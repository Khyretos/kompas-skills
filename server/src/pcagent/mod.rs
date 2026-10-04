//! The PC agent (F6): with a computer picked in the chat, the orchestrator model
//! gets the runner's tools. Every tool call waits for the user's decision in an
//! approval card (Approve, Always allow for 24 h, Deny); approved calls run as
//! runner jobs, which still check the computer's grants.
pub mod tools;

use std::time::Duration;

use axum::{Extension, Json, extract::{Path, State}, http::StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::Row;
use tracing;

use crate::{AppState, access, api::RoleAssignment, auth::User, error::{ApiError, ApiResult}, events::Event, llm, util};

const MAX_STEPS: usize = 8;

fn system_prompt(machine: &str) -> String {
    format!(
        "You are Kreative Kompanion. You can act on the user's computer \"{machine}\" with the tools you have. Every tool call is shown to the user, who approves or declines it, and the computer only allows what was granted. Prefer small, safe steps: look before you change (read a file before editing it, search before installing). Explain in one short sentence what you will do before calling a tool. When done, say plainly what you changed. Never ask for passwords; the computer asks for them itself."
    )
}

async fn say(s: &AppState, user_id: &str, chat_id: &str, text: &str) {
    let _ = sqlx::query("INSERT INTO messages (author, text, chat_id, at) VALUES (?, ?, ?, ?)")
        .bind("orchestrator")
        .bind(text)
        .bind(chat_id)
        .bind(&util::now())
        .execute(&s.db)
        .await;
    if let Err(e) = s.bus.send(user_id, Event::Message { message: text.to_string() }) {
        tracing::warn!("Failed to send event: {}", e);
    }
}

fn changed(s: &AppState, user_id: &str) {
    let _ = s.bus.send(user_id, Event::Changed { what: "actions", machine_id: None });
}

pub async fn run(
    s: AppState,
    user_id: String,
    chat_id: String,
    machine_id: String,
    machine_name: String,
    role: RoleAssignment,
) -> ApiResult<()> {
    let provider = s.config.provider(&role.provider_id);
    if provider.is_none() {
        say(&s, &user_id, &chat_id, "The orchestrator's provider is not configured.").await;
        return Ok(());
    }

    let mut messages: Vec<Value> = vec![json!({"role":"system","content": system_prompt(&machine_name)})];

    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT author, text FROM (SELECT author, text, at, rowid FROM messages WHERE chat_id = ? ORDER BY at DESC, rowid DESC LIMIT 20) ORDER BY at, rowid"
    )
    .bind(&chat_id)
    .fetch_all(&s.db)
    .await?;

    for (author, text) in rows {
        messages.push(json!({"role": if author == "user" { "user" } else { "assistant" }, "content": text}));
    }

    let p = provider.unwrap();
    let client = &s.http;
    
    for _ in 0..MAX_STEPS {
        match llm::chat_with_tools(client, p, &role.role, &messages, &tools::schema()).await {
            Ok(msg) => {
                messages.push(msg.clone());
                
                let calls = msg["tool_calls"].as_array().cloned().unwrap_or_default();
                if calls.is_empty() {
                    let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
                    say(&s, &user_id, &chat_id, if content.is_empty() { "Done." } else { content }).await;
                    return Ok(());
                }

                messages.push(msg.clone());

                for call in calls {
                    let name = call["function"]["name"].as_str().unwrap_or("");
                    let args_str = call["function"]["arguments"].as_str();
                    let args: Value = args_str.and_then(|a| serde_json::from_str(a).ok()).unwrap_or(json!({}));
                    
                    let result_text = match tools::to_job(name, &args) {
                        None => "Unknown tool or wrong arguments.".to_string(),
                        Some(job) => step(&s, &user_id, &chat_id, &machine_id, &job).await,
                    };

                    messages.push(json!({"role":"tool","tool_call_id": call["id"], "content": result_text}));
                }
            }
            Err(e) => {
                say(&s, &user_id, &chat_id, &format!("The model failed: {e:#}")).await;
                return Ok(());
            }
        }
    }

    say(&s, &user_id, &chat_id, "I stopped after 8 steps. Tell me how to go on.").await;
    Ok(())
}

async fn step(s: &AppState, user_id: &str, chat_id: &str, machine_id: &str, job: &Value) -> String {
    let id = util::new_id();
    let summary = tools::summary(job);
    
    let _ = sqlx::query("INSERT INTO pc_actions (id, chat_id, user_id, machine_id, tool, summary, state, created_at) VALUES (?, ?, ?, ?, ?, ?, 'pending', ?)")
        .bind(&id)
        .bind(chat_id)
        .bind(user_id)
        .bind(machine_id)
        .bind(job.to_string())
        .bind(&summary)
        .bind(&util::now())
        .execute(&s.db)
        .await;
    
    changed(s, user_id);

    for _ in 0..1800 {
        tokio::time::sleep(Duration::from_secs(1)).await;
        
        let row = sqlx::query_as::<_, (String,)>("SELECT state FROM pc_actions WHERE id = ?")
            .bind(&id)
            .fetch_optional(&s.db)
            .await.ok().flatten();
            
        match row {
            Some((state,)) => {
                if state != "pending" {
                    break;
                }
            }
            None => break,
        }
    }

    let row = sqlx::query_as::<_, (String, Option<String>)>("SELECT state, result FROM pc_actions WHERE id = ?")
        .bind(&id)
        .fetch_optional(&s.db)
        .await.ok().flatten();

    match row {
        Some((state, result)) => {
            if state == "denied" {
                changed(s, user_id);
                return "The user declined this step; nothing was done.".to_string();
            }
            
            if state == "pending" {
                changed(s, user_id);
                return "The user did not answer; nothing was done.".to_string();
            }

            let job_result = result.unwrap_or_else(|| "No result returned.".to_string());
            
            let _ = sqlx::query("UPDATE pc_actions SET state = ?, result = ?, decided_at = ? WHERE id = ?")
                .bind(&state)
                .bind(&job_result.chars().take(4000).collect::<String>())
                .bind(&util::now())
                .bind(&id)
                .execute(&s.db)
                .await;
                
            changed(s, user_id);
            format!("{state}: {job_result}")
        }
        None => {
            let _ = sqlx::query("UPDATE pc_actions SET state = 'failed', result = ?, decided_at = ? WHERE id = ?")
                .bind("The computer did not answer within 30 minutes.")
                .bind(&util::now())
                .bind(&id)
                .execute(&s.db)
                .await;
            changed(s, user_id);
            "The computer did not answer within 30 minutes.".to_string()
        }
    }
}

#[derive(Deserialize)]
pub struct Decision {
    pub decision: String
}

pub async fn decide(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
    Json(b): Json<Decision>
) -> ApiResult<StatusCode> {
    let row = sqlx::query_as::<_, (String, String, String)>(
        "SELECT machine_id, tool, state FROM pc_actions WHERE id = ? AND user_id = ?"
    )
    .bind(&id)
    .bind(&u.id)
    .fetch_optional(&s.db)
    .await.ok().flatten();

    let Some((machine_id, job, state)) = row else {
        return Err(ApiError::NotFound("Action not found"));
    };

    if state != "pending" {
        return Err(ApiError::BadRequest("This step was already decided.".to_string()));
    }

    let new_state = match b.decision.as_str() {
        "deny" => "denied",
        "approve" => "approved",
        "always" => {
            if let Some((target, rights)) = tools::grant_for(&job) {
                let grant_job = json!({"tool": "add_grant", "grant": {"target": target, "rights": rights, "granted_by": u.name, "granted_at": util::now(), "expires": util::in_hours(24)}});
                
                let _ = access::queue_job(&s.db, &machine_id, &u.id, &grant_job, None).await;
                
                let _ = sqlx::query("INSERT INTO access_log (machine_id, user_id, at, kind, target, detail) VALUES (?, ?, ?, ?, ?, ?)")
                    .bind(&machine_id)
                    .bind(&u.id)
                    .bind(&util::now())
                    .bind("granted")
                    .bind(&target)
                    .bind("always allow, 24 h")
                    .execute(&s.db)
                    .await;
            }
            "approved"
        }
        _ => {
            return Err(ApiError::BadRequest("Unknown decision.".to_string()));
        }
    };

    let _ = sqlx::query("UPDATE pc_actions SET state = ?, decided_at = ? WHERE id = ?")
        .bind(&new_state)
        .bind(&util::now())
        .bind(&id)
        .execute(&s.db)
        .await;

    changed(&s, &u.id);
    Ok(StatusCode::NO_CONTENT)
}

pub async fn list(
    State(s): State<AppState>,
    Extension(_u): Extension<User>,
    Path(chat_id): Path<String>
) -> ApiResult<Json<Vec<Value>>> {
    let rows: Vec<(String, String, String, String, Option<String>, String)> = sqlx::query_as(
        "SELECT id, machine_id, summary, state, result, created_at FROM pc_actions WHERE chat_id = ? ORDER BY created_at ASC LIMIT 50"
    )
    .bind(&chat_id)
    .fetch_all(&s.db)
    .await?;

    let mut result: Vec<Value> = Vec::with_capacity(rows.len());
    for (id, machine_id, summary, state, result_opt, created_at) in rows {
        result.push(json!({
            "id": id,
            "machineId": machine_id,
            "summary": summary,
            "state": state,
            "result": result_opt,
            "createdAt": created_at
        }));
    }

    Ok(Json(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_prompt_contains_machine_and_approves() {
        let prompt = system_prompt("TestMachine");
        assert!(prompt.contains("TestMachine"));
        assert!(prompt.contains("approves"));
    }
}
