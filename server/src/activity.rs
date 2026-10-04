//! The Activity tab: one history across chats and computers (PC steps, grants,
//! runs and refusals), newest first.
use axum::{Extension, Json, extract::State};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use crate::{AppState, auth::User, error::ApiResult};

#[derive(sqlx::FromRow)]
struct StepRow {
    id: String,
    created_at: String,
    decided_at: Option<String>,
    state: String,
    summary: String,
    machine_id: String,
    machine_name: Option<String>,
    chat_id: String,
    chat_title: Option<String>,
    tool: Option<String>,
    result: Option<String>,
    grant_note: Option<String>,
}

#[derive(sqlx::FromRow)]
struct AccessLogRow {
    at: String,
    kind: String,
    target: Option<String>,
    detail: Option<String>,
    machine_id: String,
    machine_name: Option<String>,
}

pub async fn list(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
) -> ApiResult<Json<Vec<Value>>> {
    let pc_actions = sqlx::query_as::<_, StepRow>(
        r#"
        SELECT a.id, a.created_at, a.decided_at, a.state, a.summary, a.machine_id, COALESCE(m.name, ''), a.chat_id, COALESCE(c.title, ''), a.tool, a.result, a.grant_note 
        FROM pc_actions a 
        LEFT JOIN machines m ON m.id = a.machine_id 
        LEFT JOIN chats c ON c.id = a.chat_id 
        WHERE a.user_id = ? 
        ORDER BY a.created_at DESC 
        LIMIT 200
        "#,
    )
    .bind(&u.id)
    .fetch_all(&s.db)
    .await?;

    let access_log = sqlx::query_as::<_, AccessLogRow>(
        r#"
        SELECT l.at, l.kind, l.target, l.detail, l.machine_id, COALESCE(m.name, '') 
        FROM access_log l 
        LEFT JOIN machines m ON m.id = l.machine_id 
        WHERE l.user_id = ? 
        AND l.kind != 'used'
        AND l.detail NOT IN ('one step, 10 min', 'one step done')
        ORDER BY l.id DESC 
        LIMIT 200
        "#,
    )
    .bind(&u.id)
    .fetch_all(&s.db)
    .await?;

    let mut combined: Vec<(String, Value)> = Vec::with_capacity(pc_actions.len() + access_log.len());

    for row in pc_actions {
        let at = &row.created_at;
        let decided_at = row.decided_at.as_ref().map(|s| s.as_str()).unwrap_or(at);
        let state = &row.state;
        let summary = &row.summary;
        let machine_id = &row.machine_id;
        let machine_name = row.machine_name.as_deref().unwrap_or("");
        let chat_id = &row.chat_id;
        let chat_title = row.chat_title.as_deref().unwrap_or("");

        let tool_value = serde_json::from_str::<Value>(&row.tool).unwrap_or(Value::Null);

        let mut item = json!({
            "id": row.id,
            "at": at,
            "kind": "step",
            "state": state,
            "text": summary,
            "machineId": machine_id,
            "machine": machine_name,
            "chatId": chat_id,
            "chat": chat_title,
            "decidedAt": decided_at,
            "tool": tool_value,
            "result": row.result,
            "grant": row.grant_note
        });

        combined.push((at.to_string(), item));
    }

    for row in access_log {
        let at = &row.at;
        let kind = &row.kind;
        let target = row.target.as_deref().unwrap_or("");
        let detail = row.detail.as_deref().unwrap_or("");

        let text = if !target.is_empty() && !detail.is_empty() {
            format!("{} · {}", target, detail)
        } else if !target.is_empty() {
            target.to_string()
        } else if !detail.is_empty() {
            detail.to_string()
        } else {
            String::new()
        };

        let mut item = json!({
            "at": at,
            "kind": kind,
            "text": text,
            "machineId": row.machine_id,
            "machine": row.machine_name.as_deref().unwrap_or(""),
            "target": target,
            "detail": detail,
        });

        combined.push((at.to_string(), item));
    }

    combined.sort_by(|a, b| b.0.cmp(&a.0));
    let result: Vec<Value> = combined.into_iter().take(200).map(|(_, v)| v).collect();

    Ok(Json(result))
}
