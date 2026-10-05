use axum::{Extension, Json, extract::{Path, Query, State}, http::header, response::{IntoResponse, Response}};
use serde::Deserialize;
use serde_json::{Value, json};
use time::{OffsetDateTime, format_description::well_known::Rfc3339};
use std::collections::HashSet;
use crate::{AppState, auth::User, error::{ApiError, ApiResult}};

/// Extracts the exit code from a result string.
/// Looks for the last line starting with "exit: ".
pub fn exit_code(result: &str) -> Option<i64> {
    result.lines().rev().find_map(|line| {
        if let Some(stripped) = line.strip_prefix("exit:") {
            stripped.trim().parse::<i64>().ok()
        } else {
            None
        }
    })
}

/// Calculates seconds between two RFC 3339 timestamps.
pub fn secs_between(a: &str, b: &str) -> Option<f64> {
    let a_ts = OffsetDateTime::parse(a, &Rfc3339).ok()?;
    let b_ts = OffsetDateTime::parse(b, &Rfc3339).ok()?;
    Some((b_ts - a_ts).whole_milliseconds() as f64 / 1000.0)
}

/// Returns an excerpt of text, max 800 chars.
/// If longer: first 400 + "\n…\n" + last 400.
pub fn excerpt(text: &str) -> String {
    let c: Vec<char> = text.chars().collect();
    if c.len() <= 800 {
        return text.to_string();
    }
    format!(
        "{}\n…\n{}",
        c[..400].iter().collect::<String>(),
        c[c.len() - 400..].iter().collect::<String>()
    )
}

/// Builds a summary JSON object for a run/chat.
pub fn summary(steps: &[Value], calls: &[Value], rounds: &[Value], status: &str) -> Value {
    let failures: Vec<String> = steps.iter()
        .filter(|s| s.get("success").and_then(|v| v.as_bool()) == Some(false) 
            && s.get("state").and_then(|v| v.as_str()) != Some("denied"))
        .map(|s| s.get("summary").and_then(|v| v.as_str()).unwrap_or("").to_string())
        .collect();
    
    let mut timed: Vec<(f64, &Value)> = steps.iter()
        .filter_map(|s| s.get("durationS").and_then(|d| d.as_f64()).map(|dur| (dur, s)))
        .collect();
    timed.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    let slowest: Vec<Value> = timed.into_iter()
        .take(3)
        .map(|(_, s)| json!({ "summary": s.get("summary").and_then(|v| v.as_str()).unwrap_or(""), "durationS": s.get("durationS").and_then(|d| d.as_f64()).unwrap_or(0.0) }))
        .collect();
    
    let mut seen_summaries = HashSet::new();
    let retries = steps.iter()
        .map(|s| {
            let sum = s.get("summary").and_then(|v| v.as_str()).unwrap_or("");
            if !seen_summaries.insert(sum) {
                1
            } else {
                0
            }
        })
        .sum::<usize>();
    
    let mut text = format!("{status}: {} steps, {} failed, {} model calls, {} review round(s).", 
        steps.len(), failures.len(), calls.len(), rounds.len());
    
    if !slowest.is_empty() {
        let first = &slowest[0];
        text.push_str(&format!(" Slowest: {} ({:.0} s).", 
            first["summary"].as_str().unwrap_or(""), 
            first["durationS"].as_f64().unwrap_or(0.0)));
    }
    
    json!({
        "text": text,
        "failures": failures,
        "slowest": slowest,
        "retries": retries
    })
}

/// Builds a single step JSON object.
pub fn step_json(machine: &str, tool: &Value, summary: &str, state: &str, result: &str, started: &str, ended: Option<&str>, grant: Option<&str>) -> Value {
    let tool_obj = tool.clone();
    let cwd = tool_obj.get("cwd")
        .and_then(|v| v.as_str())
        .or_else(|| tool_obj.get("path").and_then(|v| v.as_str()))
        .map(str::to_string);
    
    let folder = cwd.or_else(|| {
        tool_obj.get("path").and_then(|v| v.as_str())
            .and_then(|p| std::path::Path::new(p).parent())
            .map(|p| p.to_string_lossy().to_string())
    });
    
    let duration = ended.and_then(|e| secs_between(started, e));
    let exit = exit_code(result);
    let success = state == "done";
    
    json!({
        "tool": tool_obj.get("tool").and_then(|v| v.as_str()).unwrap_or(""),
        "args": tool_obj,
        "computer": machine,
        "folder": folder,
        "startedAt": started,
        "endedAt": ended,
        "durationS": duration,
        "exitCode": exit,
        "success": success,
        "state": state,
        "summary": summary,
        "output": excerpt(result),
        "grant": grant
    })
}

/// Fetches steps for a run or chat by ID.
pub async fn steps_where(s: &AppState, column: &str, id: &str) -> ApiResult<Vec<Value>> {
    if column != "run_id" && column != "chat_id" && column != "chat_without_run" {
        return Ok(Vec::new());
    }
    
    let filter = if column == "chat_without_run" { "p.chat_id = ? AND p.run_id IS NULL" } else { &format!("p.{column} = ?") };
    
    let rows: Vec<(String, String, String, String, String, String, Option<String>, Option<String>, String)> = sqlx::query_as(
        &format!("SELECT p.machine_id, p.tool, p.summary, p.state, COALESCE(p.result, ''), COALESCE(p.decided_at, p.created_at), p.ended_at, p.grant_note, COALESCE(m.name, p.machine_id) FROM pc_actions p LEFT JOIN machines m ON m.id = p.machine_id WHERE {} ORDER BY p.created_at", filter)
    )
    .bind(id)
    .fetch_all(&s.db)
    .await?;
    
    let mut output = Vec::new();
    for (machine, tool_str, summary, state, result, started, ended, grant, name) in rows {
        let tool_val: Value = serde_json::from_str(&tool_str).unwrap_or(Value::Null);
        output.push(step_json(&name, &tool_val, &summary, &state, &result, &started, ended.as_deref(), grant.as_deref()));
    }
    Ok(output)
}

/// Fetches model calls for a run or chat by ID.
pub async fn calls_where(s: &AppState, column: &str, id: &str) -> ApiResult<Vec<Value>> {
    if column != "run_id" && column != "chat_id" && column != "chat_without_run" {
        return Ok(Vec::new());
    }
    
    let filter = if column == "chat_without_run" { "c.chat_id = ? AND c.run_id IS NULL" } else { &format!("c.{column} = ?") };
    
    let rows: Vec<(String, String, String, String, Option<i64>, Option<i64>, i64, Option<String>, String)> = sqlx::query_as(
        &format!("SELECT role, provider_id, model_id, reason, tokens_in, tokens_out, ms, error, at FROM calls c WHERE {} ORDER BY at", filter)
    )
    .bind(id)
    .fetch_all(&s.db)
    .await?;
    
    let mut output = Vec::new();
    for (role, provider, model, reason, tin, tout, ms, err, at) in rows {
        output.push(json!({
            "role": role,
            "provider": provider,
            "model": model,
            "reason": reason,
            "tokensIn": tin,
            "tokensOut": tout,
            "latencyMs": ms,
            "error": err,
            "at": at
        }));
    }
    Ok(output)
}

/// Returns a report for a specific run.
pub async fn run_report(s: &AppState, user_id: &str, run_id: &str) -> ApiResult<Value> {
    type RunRow = (String, String, String, String, String, Option<String>, String, Option<String>, String, Option<String>, String, String);
    let row = sqlx::query_as::<_, RunRow>(
        "SELECT r.task_id, t.title, r.chat_id, COALESCE(m.name, r.machine_id), r.folder, r.check_cmd, r.started_at, r.ended_at, r.status, r.step, r.plan, r.rounds FROM runs r JOIN tasks t ON t.id = r.task_id LEFT JOIN machines m ON m.id = r.machine_id WHERE r.id = ? AND r.user_id = ?"
    )
    .bind(run_id)
    .bind(user_id)
    .fetch_optional(&s.db)
    .await?
        .ok_or(ApiError::NotFound)?;
    
    let (task_id, title, _chat_id, computer, folder, check_cmd, started, ended, status, step, plan_str, rounds_str) = row;
    
    let plan: Value = serde_json::from_str(&plan_str).unwrap_or(json!([]));
    let rounds: Value = serde_json::from_str(&rounds_str).unwrap_or(json!([]));
    
    let steps = steps_where(s, "run_id", run_id).await?;
    let calls = calls_where(s, "run_id", run_id).await?;
    
    let duration = ended.as_deref().and_then(|e| secs_between(&started, e));
    
    let summary_val = summary(&steps, &calls, &rounds.as_array().map(|v| v.as_slice()).unwrap_or(&[]), &status);
    
    Ok(json!({
        "summary": summary_val,
        "runId": run_id,
        "task": { "id": task_id, "title": title },
        "computer": computer,
        "folder": folder,
        "check": check_cmd,
        "status": status,
        "lastStep": step,
        "startedAt": started,
        "endedAt": ended,
        "durationS": duration,
        "plan": plan,
        "reviewRounds": rounds,
        "steps": steps,
        "modelCalls": calls
    }))
}

/// A chat's report, split per run (newest first) so one run's failures never hide another's result.
pub async fn chat_report(s: &AppState, user_id: &str, chat_id: &str) -> ApiResult<Value> {
    let chat_row = sqlx::query_as::<_, (String,)>(
        "SELECT title FROM chats WHERE id = ? AND user_id = ?"
    )
    .bind(chat_id)
    .bind(user_id)
    .fetch_optional(&s.db)
    .await?
        .ok_or(ApiError::NotFound)?;
    
    let (title,) = chat_row;
    
    // All runs of this chat, newest first
    let runs_rows: Vec<(String, String, String, Option<String>, String, String, String, String)> = sqlx::query_as(
        "SELECT r.id, COALESCE(m.name, r.machine_id), r.started_at, r.ended_at, r.status, r.rounds, r.task_id, t.title FROM runs r JOIN tasks t ON t.id = r.task_id LEFT JOIN machines m ON m.id = r.machine_id WHERE r.chat_id = ? AND r.user_id = ? ORDER BY r.started_at DESC"
    )
    .bind(chat_id)
    .bind(user_id)
    .fetch_all(&s.db)
    .await?;
    
    let mut runs = Vec::new();
    for (id, computer, started, ended, status, rounds_str, task_id, title) in runs_rows {
        let steps = steps_where(s, "run_id", &id).await?;
        let calls = calls_where(s, "run_id", &id).await?;
        let rounds: Value = serde_json::from_str(&rounds_str).unwrap_or(json!([]));
        
        let duration = ended.as_deref().and_then(|e| secs_between(&started, e));
        let summary_val = summary(&steps, &calls, &rounds.as_array().map(|v| v.as_slice()).unwrap_or(&[]), &status);
        
        runs.push(json!({
            "runId": id,
            "computer": computer,
            "startedAt": started,
            "endedAt": ended,
            "durationS": duration,
            "status": status,
            "task": { "id": task_id, "title": title },
            "summary": summary_val,
            "steps": steps,
            "modelCalls": calls
        }));
    }
    
    // Steps and calls outside any run
    let loose_steps = steps_where(s, "chat_without_run", chat_id).await?;
    let loose_calls = calls_where(s, "chat_without_run", chat_id).await?;
    
    Ok(json!({
        "chat": { "id": chat_id, "title": title },
        "runs": runs,
        "outsideRuns": { "summary": summary(&loose_steps, &loose_calls, &[], "chat"), "steps": loose_steps, "modelCalls": loose_calls },
        "note": "Each run is counted on its own; outsideRuns has the chat's steps that belong to no run."
    }))
}

#[derive(Deserialize)]
pub struct Dl {
    /// Any value (?download=1) asks for a file instead of JSON in the page.
    #[serde(default)]
    download: Option<String>,
}

/// Sends a JSON response, optionally as a file download.
fn send(v: Value, name: &str, download: bool) -> Response {
    if download {
        let body = serde_json::to_string_pretty(&v).unwrap_or_default();
        ([(header::CONTENT_TYPE, "application/json".to_string()), (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{name}.json\""))], body).into_response()
    } else {
        Json(v).into_response()
    }
}

/// Handler to get a run report.
pub async fn report(State(s): State<AppState>, Extension(u): Extension<User>, Path(id): Path<String>, Query(q): Query<Dl>) -> ApiResult<Response> {
    let v = run_report(&s, &u.id, &id).await?;
    Ok(send(v, &format!("run-{id}"), q.download.is_some()))
}

/// Handler to get a chat report.
pub async fn chat(State(s): State<AppState>, Extension(u): Extension<User>, Path(id): Path<String>, Query(q): Query<Dl>) -> ApiResult<Response> {
    let v = chat_report(&s, &u.id, &id).await?;
    Ok(send(v, &format!("chat-{id}"), q.download.is_some()))
}

/// Lists recent runs for a task.
pub async fn of_task(State(s): State<AppState>, Extension(u): Extension<User>, Path(id): Path<String>) -> ApiResult<Json<Vec<Value>>> {
    let rows: Vec<(String, Option<String>, Option<String>, String, Option<String>)> = sqlx::query_as(
        "SELECT id, started_at, ended_at, status, step FROM runs WHERE task_id = ? AND user_id = ? ORDER BY started_at DESC LIMIT 20"
    )
    .bind(&id)
    .bind(&u.id)
    .fetch_all(&s.db)
    .await?;
    
    let mut output = Vec::new();
    for (rid, started, ended, status, step) in rows {
        output.push(json!({
            "id": rid,
            "startedAt": started,
            "endedAt": ended,
            "status": status,
            "step": step
        }));
    }
    Ok(Json(output))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_exit_code_ok() {
        assert_eq!(exit_code("ok\nexit: 0"), Some(0));
    }

    #[test]
    fn test_exit_code_none() {
        assert_eq!(exit_code("x"), None);
    }

    #[test]
    fn test_exit_code_negative() {
        assert_eq!(exit_code("a\nexit: -1"), Some(-1));
    }

    #[test]
    fn test_secs_between() {
        assert_eq!(secs_between("2026-10-05T10:00:00Z", "2026-10-05T10:00:02.5Z"), Some(2.5));
    }

    #[test]
    fn test_excerpt() {
        let long = "a".repeat(1000);
        let ex = excerpt(&long);
        assert_eq!(ex.chars().count(), 803);
        assert!(ex.starts_with('a'));
        assert!(ex.contains("…"));
        assert!(ex.ends_with('a'));
    }

    #[test]
    fn test_summary() {
        let steps = vec![
            json!({"summary":"read","success":true,"durationS":1.0,"state":"done"}),
            json!({"summary":"build","success":false,"durationS":30.0,"state":"failed"}),
            json!({"summary":"build","success":true,"durationS":20.0,"state":"done"})
        ];
        let calls = vec![];
        let rounds = vec![];
        
        let s = summary(&steps, &calls, &rounds, "done");
        
        assert_eq!(s["text"].as_str().unwrap().starts_with("done: 3 steps, 1 failed"), true);
        assert_eq!(s["failures"], json!(["build"]));
        assert_eq!(s["slowest"][0]["summary"], "build");
        assert_eq!(s["slowest"][0]["durationS"], 30.0);
        assert_eq!(s["retries"], 1);
    }
}
