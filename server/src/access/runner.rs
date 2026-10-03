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
        .map_err(|e| ApiError::BadRequest(e.to_string()))?
        .ok_or(ApiError::Unauthorized)?;

    Ok(row.0)
}

pub async fn queue_job(
    db: &sqlx::SqlitePool,
    machine_id: &str,
    user_id: &str,
    tool: &Value,
    task_id: Option<&str>,
) -> sqlx::Result<String> {
    let id = util::new_id();
    let now = util::now();
    let t_id = task_id.unwrap_or("");

    sqlx::query(
        "INSERT INTO machine_jobs (id, machine_id, user_id, tool, state, created_at, task_id) 
         VALUES (?, ?, ?, ?, 'queued', ?, ?)",
    )
    .bind(&id)
    .bind(machine_id)
    .bind(tool.to_string())
    .bind(tool.to_string()) // tool is already Value, but query_as/query handles it
    .bind(&now)
    .bind(&t_id)
    .execute(db)
    .await?;

    Ok(id)
}

pub async fn take_jobs(db: &sqlx::SqlitePool, machine_id: &str) -> sqlx::Result<Vec<Value>> {
    let rows: Vec<(String, Value)> = sqlx::query_as(
        "SELECT id, tool FROM machine_jobs WHERE machine_id = ? AND state = 'queued' ORDER BY created_at ASC LIMIT 10",
    )
    .bind(machine_id)
    .fetch_all(db)
    .await?;

    let mut results = Vec::new();
    let now = util::now();

    for (id, tool) in rows {
        if let Some(obj) = tool.as_object() {
            sqlx::query(
                "UPDATE machine_jobs SET state = 'sent', sent_at = ? WHERE id = ?",
            )
            .bind(&now)
            .bind(&id)
            .execute(db)
            .await?;

            results.push(json!({
                "id": id,
                "tool": tool
            }));
        }
    }

    Ok(results)
}

pub async fn mirror_grants(
    db: &sqlx::SqlitePool,
    machine_id: &str,
    grants: &[Value],
) -> sqlx::Result<()> {
    let mut tx = db.begin().await?;

    sqlx::query("DELETE FROM machine_grants WHERE machine_id = ?")
        .bind(machine_id)
        .execute(&mut *tx)
        .await?;

    for grant in grants {
        if let Some(target) = grant.get("target").and_then(|v| v.as_str()) {
            let rights = grant.get("rights").and_then(|v| v.to_string()).unwrap_or_default();
            let granted_by = grant.get("granted_by").and_then(|v| v.as_str()).unwrap_or("system");
            let expires = grant.get("expires").and_then(|v| v.as_str()).unwrap_or("never");
            let now = util::now();

            sqlx::query(
                "INSERT INTO machine_grants (machine_id, target, rights, granted_by, granted_at, expires) 
                 VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(machine_id)
            .bind(target)
            .bind(rights)
            .bind(granted_by)
            .bind(&now)
            .bind(expires)
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
    let user_id = runner_user(&s, "dummy", &headers).await.map_err(|e| ApiError::Unauthorized)?;
    let machine_id = "dummy"; 

    let row: (Value, String) = sqlx::query_as(
        "SELECT tool, state FROM machine_jobs WHERE id = ? AND machine_id = ?",
    )
    .bind(&id)
    .bind(machine_id)
    .fetch_optional(&s.db)
    .await
    .map_err(|e| ApiError::BadRequest(e.to_string()))?
    .ok_or(ApiError::NotFound)?;

    let (tool, state) = row;

    if state == "sent" {
        let new_state = if b.refused {
            "refused"
        } else if b.ok {
            "done"
        } else {
            "failed"
        };

        let output_bytes = b.output.as_bytes();
        let truncated_output = if output_bytes.len() > 65536 {
            let len = output_bytes.iter().rposition(|&c| c != '\n').unwrap_or(65536).min(65536);
            String::from_utf8_lossy(&output_bytes[..len]).to_string()
        } else {
            b.output.clone()
        };

        sqlx::query(
            "UPDATE machine_jobs SET state = ?, result = ?, done_at = ? WHERE id = ?",
        )
        .bind(new_state)
        .bind(truncated_output)
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
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(machine_id)
        .bind(&user_id)
        .bind(util::now())
        .bind(kind)
        .bind(target)
        .bind(detail)
        .execute(&s.db)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

        if let Some(grants) = b.grants {
            mirror_grants(&s.db, machine_id, &grants).await.map_err(|e| ApiError::BadRequest(e.to_string()))?;
        }
    }

    Ok(StatusCode::NO_CONTENT)
}
