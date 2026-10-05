//! TEN-05: what a task cost, Coder (local model) versus Claude, from `tools/qwen/summary.py`.
//! Stored as `task_events` of kind "costs" (counts only, never prompts or code).

use anyhow::{Context, Result, bail};
use axum::{Extension, Json, extract::{Path, State}};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use crate::{AppState, auth::User, error::ApiResult, util};

/// pure: the totals of several cost lines: {"tasks", "coderOutput", "claudeOutput", "coderShare"}.
pub fn totals(lines: &[Value]) -> Value {
    let coder: i64 = lines.iter().map(|l| l["coder"]["output"].as_i64().unwrap_or(0)).sum();
    let claude: i64 = lines.iter().map(|l| l["claude"]["output"].as_i64().unwrap_or(0)).sum();
    let share = if coder + claude > 0 { (coder as f64 / (coder + claude) as f64 * 100.0).round() / 100.0 } else { 0.0 };
    json!({ "tasks": lines.len(), "coderOutput": coder, "claudeOutput": claude, "coderShare": share })
}

/// `kompanion-server costs <line.json> <user name>`: stores a summary line on its task.
pub async fn record(db: &SqlitePool, path: &str, user: &str) -> Result<()> {
    let line: Value = serde_json::from_str(&std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?)?;
    let Some(task) = line["task"].as_str() else { bail!("the cost line has no task") };
    let row: Option<(String,)> = sqlx::query_as("SELECT t.user_id FROM tasks t JOIN users u ON u.id = t.user_id WHERE t.id = ? AND u.name = ?")
        .bind(task).bind(user).fetch_optional(db).await?;
    let Some((user_id,)) = row else { bail!("task {task} not found for user {user}") };
    sqlx::query("INSERT INTO task_events (task_id, user_id, at, kind, detail) VALUES (?, ?, ?, 'costs', ?)")
        .bind(task).bind(&user_id).bind(util::now()).bind(line.to_string()).execute(db).await?;
    println!("cost line stored on {task}");
    Ok(())
}

/// GET /api/tasks/{id}/costs: the newest cost line of the task, or null.
pub async fn of_task(State(s): State<AppState>, Extension(u): Extension<User>, Path(id): Path<String>) -> ApiResult<Json<Value>> {
    let row: Option<(String,)> = sqlx::query_as("SELECT detail FROM task_events WHERE task_id = ? AND user_id = ? AND kind = 'costs' ORDER BY id DESC LIMIT 1")
        .bind(&id).bind(&u.id).fetch_optional(&s.db).await?;
    Ok(Json(row.and_then(|(d,)| serde_json::from_str(&d).ok()).unwrap_or(Value::Null)))
}

/// GET /api/costs/weekly: totals of the user's cost lines from the last 7 days.
pub async fn weekly(State(s): State<AppState>, Extension(u): Extension<User>) -> ApiResult<Json<Value>> {
    let since = (time::OffsetDateTime::now_utc() - time::Duration::days(7)).format(&time::format_description::well_known::Rfc3339).unwrap_or_default();
    let rows: Vec<(String,)> = sqlx::query_as("SELECT detail FROM task_events WHERE user_id = ? AND kind = 'costs' AND at >= ?")
        .bind(&u.id).bind(&since).fetch_all(&s.db).await?;
    let lines: Vec<Value> = rows.iter().filter_map(|(d,)| serde_json::from_str(d).ok()).collect();
    Ok(Json(totals(&lines)))
}

#[cfg(test)] mod tests { use super::*;
    #[test] fn totals_add_up() {
        let a = json!({"coder": {"output": 300}, "claude": {"output": 700}});
        let b = json!({"coder": {"output": 100}, "claude": {"output": 900}});
        assert_eq!(totals(&[a, b]), json!({"tasks": 2, "coderOutput": 400, "claudeOutput": 1600, "coderShare": 0.2}));
        assert_eq!(totals(&[])["coderShare"], json!(0.0));
    }
}
