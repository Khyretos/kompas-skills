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
         say plainly what you changed. Never ask for passwords; the computer asks for them itself. \
         Do every part of the request: after each result, call the next tool until all parts are done \
         (for \"install X and show its version\": install, then run the version command). Only say a step \
         worked when its result is \"done\" with \"exit: 0\", and quote the key output line. When a step \
         failed, was refused or declined, say so plainly and stop. \
         Never route around a refused or declined step with another tool (for example reading a refused \
         file with shell); report it instead. Never create a config or system file the user did not ask \
         you to create: if the file you were asked to change does not exist, stop, say what is there \
         (list the folder) and ask. Before changing desktop or app settings, call system_info and use the \
         config it reports (Hyprland: hyprland.lua or hyprland.conf, whichever exists). After a change, \
         check its effect with a read-only command (Hyprland: `hyprctl getoption general:gaps_out`) before \
         saying it worked; if you can't check, say it is not verified."
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

/// The start of the error `Agent::run` returns when it used all its steps.
const STEP_LIMIT: &str = "I stopped after";

/// Whether an `Agent::run` error only means it ran out of steps (the work may well be done).
pub fn hit_step_limit(err: &str) -> bool {
    err.starts_with(STEP_LIMIT)
}

pub async fn run(s: AppState, user_id: String, chat_id: String, machine_id: String, machine_name: String, role: RoleAssignment) {
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
    let agent = Agent { s: s.clone(), user_id: user_id.clone(), chat_id: chat_id.clone(), machine_id, role, auto: false, max_steps: MAX_STEPS, folder: None };
    let text = match agent.run(messages).await {
        Ok(t) => t,
        Err(e) => e,
    };
    say(&s, &user_id, &chat_id, &text).await;
}

/// One agent working on one computer for one chat: the model calls the runner's
/// tools until it answers in text (or runs out of steps).
pub struct Agent {
    pub s: AppState,
    pub user_id: String,
    pub chat_id: String,
    pub machine_id: String,
    pub role: RoleAssignment,
    /// Steps already covered by a grant run without a card (tasks under standing
    /// grants); anything else still waits for the user's decision.
    pub auto: bool,
    pub max_steps: usize,
    /// W2 tasks: every path and working folder must be inside this folder, so the check
    /// and the diff always see the folder that was worked in (never two mixed folders).
    pub folder: Option<String>,
}

/// The paths a runner job touches (path, or the working folder of a command).
fn job_paths(job: &Value) -> Vec<&str> {
    ["path", "cwd"].iter().filter_map(|k| job[*k].as_str()).collect()
}

/// Inside `folder` (the folder itself or below it), compared as clean absolute paths.
pub fn inside(folder: &str, path: &str) -> bool {
    let f = folder.trim_end_matches('/');
    path.starts_with('/') && !path.split('/').any(|c| c == "..") && (path == f || path.starts_with(&format!("{f}/")))
}

impl Agent {
    /// Runs the tool loop on `messages` (system prompt first). Ok(final text), or
    /// Err(text to tell the user) when the model failed or ran out of steps.
    pub async fn run(&self, mut messages: Vec<Value>) -> Result<String, String> {
        let s = &self.s;
        let Some(p) = s.config.provider(&self.role.provider_id) else {
            return Err(format!("The provider {} is not configured.", self.role.provider_id));
        };
        let tools = tools::schema();
        // Paths a read found missing during this answer (see the write_file guardrail).
        let mut missing = std::collections::HashSet::<String>::new();
        for _ in 0..self.max_steps {
            let msg = llm::chat_with_tools(&s.http, p, &self.role.model_id, &messages, &tools)
                .await
                .map_err(|e| format!("The model failed: {e:#}"))?;
            let calls = msg["tool_calls"].as_array().cloned().unwrap_or_default();
            if calls.is_empty() {
                let text = msg["content"].as_str().unwrap_or("").trim();
                return Ok(if text.is_empty() { "Done.".into() } else { text.to_string() });
            }
            // A short "what I'll do" sentence before the tools, when the model wrote one.
            if let Some(text) = msg["content"].as_str().map(str::trim).filter(|t| !t.is_empty()) {
                say(s, &self.user_id, &self.chat_id, text).await;
            }
            messages.push(msg.clone());
            for call in &calls {
                let name = call["function"]["name"].as_str().unwrap_or("");
                let args: Value = call["function"]["arguments"]
                    .as_str()
                    .and_then(|a| serde_json::from_str(a).ok())
                    .unwrap_or_else(|| json!({}));
                let result = match tools::to_job(name, &args) {
                    // Answered here, not by a computer.
                    None if name == "capabilities" => crate::capabilities::summary(s, &self.user_id).await,
                    None => "Unknown tool or wrong arguments.".to_string(),
                    // Guardrail: a file found missing earlier in this answer is not created
                    // behind the user's back.
                    Some(job) if self.folder.as_deref().is_some_and(|f| job_paths(&job).iter().any(|p| !inside(f, p))) => {
                        format!(
                            "Blocked: outside the task's folder {}. Work only there; if the work belongs in another folder, stop and say which.",
                            self.folder.as_deref().unwrap_or("")
                        )
                    }
                    Some(job) if job["tool"] == "write_file" && job["path"].as_str().is_some_and(|p| missing.contains(p)) => {
                        "Blocked: that file did not exist when you looked. Ask the user before creating a new file.".to_string()
                    }
                    Some(job) => {
                        let r = step(s, &self.user_id, &self.chat_id, &self.machine_id, &job, self.auto).await;
                        if r.contains("no such file") && let Some(p) = job["path"].as_str() {
                            missing.insert(p.to_string());
                        }
                        r
                    }
                };
                messages.push(json!({ "role": "tool", "tool_call_id": call["id"], "content": result }));
            }
        }
        Err(format!("{STEP_LIMIT} {} steps. Tell me how to go on.", self.max_steps))
    }
}

/// Whether the computer's current grants (the server's mirror) already allow this
/// job, so it can run without asking.
pub async fn covered(s: &AppState, machine_id: &str, job: &Value) -> bool {
    let Some((target, rights)) = tools::grant_for(job) else { return job["tool"] == "system_info" };
    let rows: Vec<(String, String, Option<String>)> =
        sqlx::query_as("SELECT target, rights, expires FROM machine_grants WHERE machine_id = ?")
            .bind(machine_id)
            .fetch_all(&s.db)
            .await
            .unwrap_or_default();
    let now = util::now();
    rows.iter().any(|(t, r, expires)| {
        let target_ok = if target == "system" {
            t == "system"
        } else {
            t != "system" && std::path::Path::new(&target).starts_with(t)
        };
        let have: Vec<String> = serde_json::from_str(r).unwrap_or_default();
        target_ok && rights.iter().all(|x| have.iter().any(|h| h == x)) && expires.as_deref().is_none_or(|e| now.as_str() < e)
    })
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
async fn step(s: &AppState, user_id: &str, chat_id: &str, machine_id: &str, job: &Value, auto: bool) -> String {
    let id = util::new_id();
    // Under a standing grant (tasks): no card, the step runs right away.
    let preapproved = auto && covered(s, machine_id, job).await;
    let stored = sqlx::query(
        "INSERT INTO pc_actions (id, chat_id, user_id, machine_id, tool, summary, state, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(chat_id)
    .bind(user_id)
    .bind(machine_id)
    .bind(job.to_string())
    .bind(tools::summary(job))
    .bind(if preapproved { "running" } else { "pending" })
    .bind(util::now())
    .execute(&s.db)
    .await;
    if let Err(e) = stored {
        return format!("Could not ask the user: {e}");
    }
    changed(s, user_id);

    let mut state = if preapproved { "covered".to_string() } else { "pending".to_string() };
    for _ in 0..if preapproved { 0 } else { WAIT_SECS } {
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
    let always = state == "always";

    // The computer only runs what is granted. Approve adds a grant for this one
    // step (10 minutes, removed again afterwards, the earlier grant on that target
    // restored); Always allow adds one for 24 hours.
    // Already covered by a standing grant: nothing to add or remove.
    let needed = if preapproved { None } else { tools::grant_for(job) };
    let mut restore: Option<Option<Value>> = None;
    if let Some((target, rights)) = &needed {
        set_action(s, &id, "granting", "").await;
        changed(s, user_id);
        let before = current_grant(s, machine_id, target).await;
        let expires = if always { util::in_hours(24) } else { in_minutes(10) };
        let grant = json!({ "tool": "add_grant", "grant": {
            "target": target, "rights": rights, "granted_by": user_name(s, user_id).await,
            "granted_at": util::now(), "expires": expires,
        } });
        let ok = match access::queue_job(&s.db, machine_id, user_id, &grant, None).await {
            Ok(job_id) => wait_job(s, &job_id).await.0 == "done",
            Err(_) => false,
        };
        if !ok {
            set_action(s, &id, "failed", "The computer did not confirm the grant.").await;
            changed(s, user_id);
            return "failed: the computer did not confirm the grant, so nothing was run.".into();
        }
        log_access(s, machine_id, user_id, "granted", target, if always { "always allow, 24 h" } else { "one step, 10 min" }).await;
        if !always {
            restore = Some(before);
        }
    }

    let grant_note = if preapproved {
        "standing grant"
    } else if needed.is_none() {
        "no grant needed"
    } else if always {
        "always allowed · 24 h"
    } else {
        "allowed once · 10 min · removed after"
    };
    let _ = sqlx::query("UPDATE pc_actions SET grant_note = ? WHERE id = ?").bind(grant_note).bind(&id).execute(&s.db).await;
    set_action(s, &id, "running", "").await;
    changed(s, user_id);
    let (state, result) = match access::queue_job(&s.db, machine_id, user_id, job, None).await {
        Ok(job_id) => wait_job(s, &job_id).await,
        Err(e) => ("failed".to_string(), e.to_string()),
    };

    // A one-step grant goes away again; an earlier grant on that target comes back.
    if let (Some(before), Some((target, _))) = (restore, &needed) {
        let _ = access::queue_job(&s.db, machine_id, user_id, &json!({ "tool": "revoke_grant", "target": target }), None).await;
        if let Some(g) = before {
            let _ = access::queue_job(&s.db, machine_id, user_id, &json!({ "tool": "add_grant", "grant": g }), None).await;
        }
        log_access(s, machine_id, user_id, "revoked", target, "one step done").await;
    }
    set_action(s, &id, &state, &result).await;
    changed(s, user_id);
    model_result(&state, &result)
}

/// What the model gets back from a step: the outcome first, then the end of the
/// output (where the exit code and errors are).
fn model_result(state: &str, result: &str) -> String {
    let tail_start = result.len().saturating_sub(6000);
    let mut start = tail_start;
    while !result.is_char_boundary(start) {
        start += 1;
    }
    let outcome = match state {
        "done" => "The step ran.",
        "refused" => "The computer refused the step (not granted).",
        _ => "The step failed.",
    };
    format!("{outcome} State: {state}.\nOutput{}:\n{}", if start > 0 { " (last part)" } else { "" }, &result[start..])
}

fn in_minutes(m: i64) -> String {
    (time::OffsetDateTime::now_utc() + time::Duration::minutes(m))
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default()
}

async fn user_name(s: &AppState, user_id: &str) -> String {
    sqlx::query_as::<_, (String,)>("SELECT name FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(&s.db)
        .await
        .ok()
        .flatten()
        .map(|r| r.0)
        .unwrap_or_else(|| "kompanion".into())
}

/// The computer's current grant on `target`, as the runner's grant JSON, from the
/// server's mirror of its grants.json.
async fn current_grant(s: &AppState, machine_id: &str, target: &str) -> Option<Value> {
    let row: Option<(String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT rights, granted_by, granted_at, expires FROM machine_grants WHERE machine_id = ? AND target = ?",
    )
    .bind(machine_id)
    .bind(target)
    .fetch_optional(&s.db)
    .await
    .ok()
    .flatten();
    row.map(|(rights, by, at, expires)| {
        json!({ "target": target, "rights": serde_json::from_str::<Value>(&rights).unwrap_or(json!([])),
                "granted_by": by, "granted_at": at, "expires": expires })
    })
}

async fn log_access(s: &AppState, machine_id: &str, user_id: &str, kind: &str, target: &str, detail: &str) {
    let _ = sqlx::query("INSERT INTO access_log (machine_id, user_id, at, kind, target, detail) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(machine_id)
        .bind(user_id)
        .bind(util::now())
        .bind(kind)
        .bind(target)
        .bind(detail)
        .execute(&s.db)
        .await;
}

/// Waits for a runner job to finish: (state, result).
async fn wait_job(s: &AppState, job_id: &str) -> (String, String) {
    for _ in 0..WAIT_SECS {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let row: Option<(String, Option<String>)> = sqlx::query_as("SELECT state, result FROM machine_jobs WHERE id = ?")
            .bind(job_id)
            .fetch_optional(&s.db)
            .await
            .unwrap_or(None);
        if let Some((st, res)) = row
            && matches!(st.as_str(), "done" | "failed" | "refused")
        {
            return (st, res.unwrap_or_default());
        }
    }
    ("failed".into(), "The computer did not answer within 30 minutes.".into())
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
        "always" => "always",
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
    let rows: Vec<(String, String, String, String, Option<String>, String, String)> = sqlx::query_as(
        "SELECT id, machine_id, summary, state, result, created_at, tool FROM (
           SELECT * FROM pc_actions WHERE chat_id = ? AND user_id = ? ORDER BY created_at DESC LIMIT 50
         ) ORDER BY created_at",
    )
    .bind(&chat_id)
    .bind(&u.id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, machine_id, summary, state, result, created_at, tool)| {
                let job: Value = serde_json::from_str(&tool).unwrap_or(Value::Null);
                json!({ "id": id, "machineId": machine_id, "summary": summary, "state": state, "result": result,
                        "createdAt": created_at, "needs": needs_text(&job), "tool": job })
            })
            .collect(),
    ))
}

/// What a step needs on the computer, for the card: "packages + root (asks for the password on the PC)".
fn needs_text(job: &Value) -> Option<String> {
    let (target, rights) = tools::grant_for(job)?;
    let mut text = rights.join(" + ");
    if target != "system" {
        text = format!("{text} on {target}");
    }
    if rights.contains(&"root") {
        text.push_str(" (asks for the password on the PC)");
    }
    Some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_task_folder_confines_paths() {
        assert!(inside("/home/k/demo", "/home/k/demo"));
        assert!(inside("/home/k/demo/", "/home/k/demo/names.py"));
        assert!(!inside("/home/k/demo", "/home/k/demo2/x"));
        assert!(!inside("/home/k/demo", "/home/k/demo/../etc"));
        assert!(!inside("/home/k/demo", "names.py"));
        assert_eq!(job_paths(&json!({ "tool": "shell", "cwd": "/a", "command": "ls" })), ["/a"]);
    }

    #[test]
    fn prompt_names_the_machine() {
        let p = system_prompt("soucouyant");
        assert!(p.contains("soucouyant") && p.contains("approves"));
    }

    #[test]
    fn model_result_says_the_outcome_first() {
        let r = model_result("done", "installed\nexit: 0");
        assert!(r.starts_with("The step ran. State: done."));
        assert!(r.ends_with("exit: 0"));
        assert!(model_result("refused", "not granted: packages").contains("refused"));
    }

    #[test]
    fn cuts_on_char_boundaries() {
        assert_eq!(cut("aé", 2), "a");
        assert_eq!(cut("abc", 10), "abc");
    }
}
