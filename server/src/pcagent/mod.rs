//! The PC agent (F6): with a computer picked in the chat, the orchestrator model
//! gets the runner's tools. Every tool call waits for the user's decision in an
//! approval card (Approve, Always allow for 24 h, Deny); approved calls run as
//! runner jobs, which still check the computer's grants.
//! (The model drafted this twice; Claude rewrote it from the same spec.)
pub mod tools;

use std::time::Duration;

use axum::{Extension, Json, extract::{Path, State}, http::StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{AppState, access, api::RoleAssignment, auth::User, error::{ApiError, ApiResult}, events::Event, llm, util};

const MAX_STEPS: usize = 8;
/// How long a card waits for a decision, and a job for the computer (seconds).
const WAIT_SECS: u32 = 1800;

fn system_prompt(machine: &str) -> String {
    format!(
        "You are Kreative Kompanion. You can act on the user's computer \"{machine}\" with the tools you have. \
         Every tool call is shown to the user, who approves or declines it, and the computer only allows what \
         was granted. Prefer small, safe steps: look before you change (read a file before editing it, search \
         before installing). Explain in one short sentence what you will do before calling a tool. When done, \
         say plainly what you changed. Never ask for passwords; the computer asks for them itself."
    )
}

/// Cuts text to at most `max` bytes on a character boundary.
fn cut(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

async fn say(s: &AppState, user_id: &str, chat_id: &str, text: &str) {
    match crate::api::insert_message(s, chat_id, "orchestrator", text).await {
        Ok(message) => s.bus.send(user_id, Event::Message { message }),
        Err(e) => tracing::warn!("pc agent: can't store a message: {e}"),
    }
}

fn changed(s: &AppState, user_id: &str) {
    s.bus.send(user_id, Event::Changed { what: "actions", machine_id: None });
}

pub async fn run(s: AppState, user_id: String, chat_id: String, machine_id: String, machine_name: String, role: RoleAssignment) {
    let Some(p) = s.config.provider(&role.provider_id) else {
        return say(&s, &user_id, &chat_id, "The orchestrator's provider is not configured.").await;
    };
    let mut messages = vec![json!({ "role": "system", "content": system_prompt(&machine_name) })];
    let history: Vec<(String, String)> = sqlx::query_as(
        "SELECT author, text FROM (SELECT author, text, at, rowid FROM messages WHERE chat_id = ?
         ORDER BY at DESC, rowid DESC LIMIT 20) ORDER BY at, rowid",
    )
    .bind(&chat_id)
    .fetch_all(&s.db)
    .await
    .unwrap_or_default();
    for (author, text) in history {
        let role = if author == "user" { "user" } else { "assistant" };
        messages.push(json!({ "role": role, "content": text }));
    }
    let tools = tools::schema();

    for _ in 0..MAX_STEPS {
        let msg = match llm::chat_with_tools(&s.http, p, &role.model_id, &messages, &tools).await {
            Ok(m) => m,
            Err(e) => return say(&s, &user_id, &chat_id, &format!("The model failed: {e:#}")).await,
        };
        let calls = msg["tool_calls"].as_array().cloned().unwrap_or_default();
        if calls.is_empty() {
            let text = msg["content"].as_str().unwrap_or("").trim();
            return say(&s, &user_id, &chat_id, if text.is_empty() { "Done." } else { text }).await;
        }
        // A short "what I'll do" sentence before the tools, when the model wrote one.
        if let Some(text) = msg["content"].as_str().map(str::trim).filter(|t| !t.is_empty()) {
            say(&s, &user_id, &chat_id, text).await;
        }
        messages.push(msg.clone());
        for call in &calls {
            let name = call["function"]["name"].as_str().unwrap_or("");
            let args: Value = call["function"]["arguments"]
                .as_str()
                .and_then(|a| serde_json::from_str(a).ok())
                .unwrap_or_else(|| json!({}));
            let result = match tools::to_job(name, &args) {
                None => "Unknown tool or wrong arguments.".to_string(),
                Some(job) => step(&s, &user_id, &chat_id, &machine_id, &job).await,
            };
            messages.push(json!({ "role": "tool", "tool_call_id": call["id"], "content": result }));
        }
    }
    say(&s, &user_id, &chat_id, "I stopped after 8 steps. Tell me how to go on.").await;
}

async fn set_action(s: &AppState, id: &str, state: &str, result: &str) {
    let _ = sqlx::query("UPDATE pc_actions SET state = ?, result = ? WHERE id = ?")
        .bind(state)
        .bind(cut(result, 4000))
        .bind(id)
        .execute(&s.db)
        .await;
}

/// One tool call: an approval card, then (if approved) a runner job. Returns the
/// text the model gets back.
async fn step(s: &AppState, user_id: &str, chat_id: &str, machine_id: &str, job: &Value) -> String {
    let id = util::new_id();
    let stored = sqlx::query(
        "INSERT INTO pc_actions (id, chat_id, user_id, machine_id, tool, summary, state, created_at)
         VALUES (?, ?, ?, ?, ?, ?, 'pending', ?)",
    )
    .bind(&id)
    .bind(chat_id)
    .bind(user_id)
    .bind(machine_id)
    .bind(job.to_string())
    .bind(tools::summary(job))
    .bind(util::now())
    .execute(&s.db)
    .await;
    if let Err(e) = stored {
        return format!("Could not ask the user: {e}");
    }
    changed(s, user_id);

    let mut state = "pending".to_string();
    for _ in 0..WAIT_SECS {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let row: Option<(String,)> = sqlx::query_as("SELECT state FROM pc_actions WHERE id = ?")
            .bind(&id)
            .fetch_optional(&s.db)
            .await
            .unwrap_or(None);
        state = row.map(|r| r.0).unwrap_or_else(|| "denied".into());
        if state != "pending" {
            break;
        }
    }
    match state.as_str() {
        "pending" => {
            set_action(s, &id, "denied", "No answer within 30 minutes.").await;
            changed(s, user_id);
            return "The user did not answer; nothing was done.".into();
        }
        "denied" => return "The user declined this step; nothing was done.".into(),
        _ => {}
    }

    let job_id = match access::queue_job(&s.db, machine_id, user_id, job, None).await {
        Ok(j) => j,
        Err(e) => {
            set_action(s, &id, "failed", &e.to_string()).await;
            changed(s, user_id);
            return format!("failed: {e}");
        }
    };
    let (mut state, mut result) = ("failed".to_string(), "The computer did not answer within 30 minutes.".to_string());
    for _ in 0..WAIT_SECS {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let row: Option<(String, Option<String>)> = sqlx::query_as("SELECT state, result FROM machine_jobs WHERE id = ?")
            .bind(&job_id)
            .fetch_optional(&s.db)
            .await
            .unwrap_or(None);
        if let Some((st, res)) = row
            && matches!(st.as_str(), "done" | "failed" | "refused")
        {
            (state, result) = (st, res.unwrap_or_default());
            break;
        }
    }
    set_action(s, &id, &state, &result).await;
    changed(s, user_id);
    cut(&format!("{state}: {result}"), 8000).to_string()
}

#[derive(Deserialize)]
pub struct Decision {
    pub decision: String,
}

/// The user's answer to an approval card.
pub async fn decide(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
    Json(b): Json<Decision>,
) -> ApiResult<StatusCode> {
    let row: Option<(String, String, String)> =
        sqlx::query_as("SELECT machine_id, tool, state FROM pc_actions WHERE id = ? AND user_id = ?")
            .bind(&id)
            .bind(&u.id)
            .fetch_optional(&s.db)
            .await?;
    let Some((machine_id, tool, state)) = row else { return Err(ApiError::NotFound) };
    if state != "pending" {
        return Err(ApiError::BadRequest("This step was already decided.".into()));
    }
    let new_state = match b.decision.as_str() {
        "deny" => "denied",
        "approve" => "approved",
        "always" => {
            let job: Value = serde_json::from_str(&tool).unwrap_or(Value::Null);
            if let Some((target, rights)) = tools::grant_for(&job) {
                let grant = json!({ "tool": "add_grant", "grant": {
                    "target": target, "rights": rights, "granted_by": u.name,
                    "granted_at": util::now(), "expires": util::in_hours(24),
                } });
                access::queue_job(&s.db, &machine_id, &u.id, &grant, None).await?;
                sqlx::query("INSERT INTO access_log (machine_id, user_id, at, kind, target, detail) VALUES (?, ?, ?, 'granted', ?, ?)")
                    .bind(&machine_id)
                    .bind(&u.id)
                    .bind(util::now())
                    .bind(&target)
                    .bind("always allow, 24 h")
                    .execute(&s.db)
                    .await?;
            }
            "approved"
        }
        _ => return Err(ApiError::BadRequest("Unknown decision.".into())),
    };
    sqlx::query("UPDATE pc_actions SET state = ?, decided_at = ? WHERE id = ? AND state = 'pending'")
        .bind(new_state)
        .bind(util::now())
        .bind(&id)
        .execute(&s.db)
        .await?;
    changed(&s, &u.id);
    Ok(StatusCode::NO_CONTENT)
}

/// The approval cards of a chat, oldest first.
pub async fn list(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(chat_id): Path<String>,
) -> ApiResult<Json<Vec<Value>>> {
    let rows: Vec<(String, String, String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT id, machine_id, summary, state, result, created_at FROM (
           SELECT * FROM pc_actions WHERE chat_id = ? AND user_id = ? ORDER BY created_at DESC LIMIT 50
         ) ORDER BY created_at",
    )
    .bind(&chat_id)
    .bind(&u.id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, machine_id, summary, state, result, created_at)| {
                json!({ "id": id, "machineId": machine_id, "summary": summary, "state": state, "result": result, "createdAt": created_at })
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_names_the_machine() {
        let p = system_prompt("soucouyant");
        assert!(p.contains("soucouyant") && p.contains("approves"));
    }

    #[test]
    fn cuts_on_char_boundaries() {
        assert_eq!(cut("aé", 2), "a");
        assert_eq!(cut("abc", 10), "abc");
    }
}
