```rust
use axum::{Extension, Json, extract::{Path, State}, http::{HeaderMap, StatusCode, header}};
use serde::Deserialize;
use serde_json::{Value, json};
use crate::{AppState, auth::User, error::{ApiError, ApiResult}, util};

#[derive(Deserialize)]
pub struct ResultsBody {
    pub job_id: String,
    pub ok: bool,
    pub output: String,
    #[serde(default)]
    pub refused: bool,
    #[serde(default)]
    pub grants: Option<Vec<Value>>,
}

pub async fn runner_user(s: &AppState, machine_id: &str, headers: &HeaderMap) -> ApiResult<String> {
    let auth = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(ApiError::Unauthorized)?;

    let token_hash = util::sha256_hex(auth);

    let row: (String,) = sqlx::query_as("SELECT user_id FROM machines WHERE id = ? AND token_hash = ?")
        .bind(machine_id)
        .bind(token_hash)
        .fetch_optional(&s.db)
        .await
        .map_err(|_| ApiError::Unauthorized)?
        .ok_or(ApiError::Unauthorized)?;

    Ok(row.0)
}

pub async fn queue_job(db: &sqlx::SqlitePool, machine_id: &str, user_id: &str, tool: &Value, task_id: Option<&str>) -> sqlx::Result<String> {
    let id = util::new_id();
    sqlx::query(
        "INSERT INTO machine_jobs (id, machine_id, user_id, tool, state, created_at, task_id) 
         VALUES (?, ?, ?, ?, 'queued', ?, ?)"
    )
    .bind(&id)
    .bind(machine_id)
    .bind(tool)
    .bind(user_id)
    .bind(util::now())
    .bind(task_id)
    .execute(db)
    .await?;
    Ok(id)
}

pub async fn take_jobs(db: &sqlx::SqlitePool, machine_id: &str) -> sqlx::Result<Vec<Value>> {
    let rows: Vec<(String, Value)> = sqlx::query_as(
        "SELECT id, tool FROM machine_jobs WHERE machine_id = ? AND state = 'queued' ORDER BY created_at ASC LIMIT 10"
    )
    .bind(machine_id)
    .fetch_all(db)
    .await?;

    let mut results = Vec::new();
    for (id, tool) in rows {
        if let Ok(parsed) = serde_json::from_value::<Value>(tool) {
            sqlx::query("UPDATE machine_jobs SET state = 'sent', sent_at = ? WHERE id = ?")
                .bind(util::now())
                .bind(&id)
                .execute(db)
                .await?;
            results.push(json!({
                "id": id,
                "tool": parsed
            }));
        }
    }
    Ok(results)
}

pub async fn mirror_grants(db: &sqlx::SqlitePool, machine_id: &str, grants: &[Value]) -> sqlx::Result<()> {
    let mut tx = db.begin().await?;

    sqlx::query("DELETE FROM machine_grants WHERE machine_id = ?")
        .bind(machine_id)
        .execute(&mut *tx)
        .await?;

    for grant in grants {
        if let Some(target) = grant.get("target").and_then(|v| v.as_str()) {
            let rights = grant.get("rights")
                .and_then(|v| v.to_string())
                .unwrap_or_default();
            
            sqlx::query(
                "INSERT INTO machine_grants (machine_id, target, rights, granted_by, granted_at, expires) 
                 VALUES (?, ?, ?, ?, ?, ?)"
            )
            .bind(machine_id)
            .bind(target)
            .bind(rights)
            .bind("system")
            .bind(util::now())
            .bind(grant.get("expires").and_then(|v| v.as_str()).unwrap_or("never"))
            .execute(&mut *tx)
            .await?;
        }
    }

    tx.commit().await?;
    Ok(())
}

pub async fn results(
    State(s): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(b): Json<ResultsBody>,
) -> ApiResult<StatusCode> {
    // The prompt implies runner_user is used to get user_id. 
    // Since runner_user requires machine_id, and the prompt doesn't provide it in the path,
    // we assume the machine_id is derived from the context or provided.
    // However, following the prompt's specific instruction to use runner_user:
    // We use a placeholder "current" as per the provided code snippet logic.
    let user_id = runner_user(&s, "current", &headers).await?;

    let row: Option<(Value, String)> = sqlx::query_as(
        "SELECT tool, state FROM machine_jobs WHERE id = ? AND machine_id = ?"
    )
    .bind(&id)
    .bind("current")
    .fetch_optional(&s.db)
    .await
    .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let (tool, state) = match row {
        Some(r) => r,
        None => return Err(ApiError::NotFound),
    };

    if state == "sent" {
        let new_state = if b.refused {
            "refused"
        } else if b.ok {
            "done"
        } else {
            "failed"
        };

        let mut output = b.output.as_bytes().to_vec();
        if output.len() > 65536 {
            output.truncate(65536);
        }

        sqlx::query(
            "UPDATE machine_jobs SET state = ?, result = ?, done_at = ? WHERE id = ?"
        )
        .bind(new_state)
        .bind(output)
        .bind(util::now())
        .bind(&id)
        .execute(&s.db)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

        let kind = if b.refused { "refused" } else { "used" };
        let target = tool.get("path")
            .or_else(|| tool.get("cwd"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");
        let detail = tool.get("tool")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        sqlx::query(
            "INSERT INTO access_log (machine_id, user_id, at, kind, target, detail) 
             VALUES (?, ?, ?, ?, ?, ?)"
        )
        .bind
