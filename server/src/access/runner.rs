//! The runner side of F5: runner authentication, the job queue, results, and
//! the mirror of each machine's grants. (Claude rewrote gemma4's draft: it
//! authenticated against a fixed "dummy" machine and stored JSON as Value.)

use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode, header},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    error::{ApiError, ApiResult},
    util,
};

/// The user that owns the machine whose runner token is in the request.
pub async fn runner_user(s: &AppState, machine_id: &str, headers: &HeaderMap) -> ApiResult<String> {
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .ok_or(ApiError::Unauthorized)?;
    let row: Option<(String,)> = sqlx::query_as("SELECT user_id FROM machines WHERE id = ? AND token_hash = ?")
        .bind(machine_id)
        .bind(util::sha256_hex(token.trim()))
        .fetch_optional(&s.db)
        .await?;
    row.map(|r| r.0).ok_or(ApiError::Unauthorized)
}

pub async fn queue_job(
    db: &sqlx::SqlitePool,
    machine_id: &str,
    user_id: &str,
    tool: &Value,
    task_id: Option<&str>,
) -> sqlx::Result<String> {
    let id = util::new_id();
    sqlx::query(
        "INSERT INTO machine_jobs (id, machine_id, user_id, tool, state, created_at, task_id)
         VALUES (?, ?, ?, ?, 'queued', ?, ?)",
    )
    .bind(&id)
    .bind(machine_id)
    .bind(user_id)
    .bind(tool.to_string())
    .bind(util::now())
    .bind(task_id)
    .execute(db)
    .await?;
    Ok(id)
}

/// Up to 10 queued jobs, oldest first, marked as sent.
pub async fn take_jobs(db: &sqlx::SqlitePool, machine_id: &str) -> sqlx::Result<Vec<Value>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT id, tool FROM machine_jobs WHERE machine_id = ? AND state = 'queued' ORDER BY created_at LIMIT 10",
    )
    .bind(machine_id)
    .fetch_all(db)
    .await?;
    let now = util::now();
    let mut out = Vec::new();
    for (id, tool) in rows {
        let Ok(tool) = serde_json::from_str::<Value>(&tool) else { continue };
        sqlx::query("UPDATE machine_jobs SET state = 'sent', sent_at = ? WHERE id = ?")
            .bind(&now)
            .bind(&id)
            .execute(db)
            .await?;
        out.push(json!({ "id": id, "tool": tool }));
    }
    Ok(out)
}

/// Replaces the server's copy of a machine's grants.
pub async fn mirror_grants(db: &sqlx::SqlitePool, machine_id: &str, grants: &[Value]) -> sqlx::Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM machine_grants WHERE machine_id = ?")
        .bind(machine_id)
        .execute(&mut *tx)
        .await?;
    for g in grants.iter().take(500) {
        let Some(target) = g["target"].as_str() else { continue };
        sqlx::query(
            "INSERT OR REPLACE INTO machine_grants (machine_id, target, rights, granted_by, granted_at, expires)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(machine_id)
        .bind(target.chars().take(4096).collect::<String>())
        .bind(g["rights"].to_string())
        .bind(g["granted_by"].as_str().unwrap_or(""))
        .bind(g["granted_at"].as_str().unwrap_or(""))
        .bind(g["expires"].as_str())
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

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

fn cut(s: &str, max: usize) -> &str {
    if s.len() <= max {
        return s;
    }
    let mut end = max;
    while !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

/// A runner reports the outcome of a job (and its current grants).
pub async fn results(
    State(s): State<AppState>,
    Path(machine_id): Path<String>,
    headers: HeaderMap,
    Json(b): Json<ResultsBody>,
) -> ApiResult<StatusCode> {
    let user_id = runner_user(&s, &machine_id, &headers).await?;
    let job: Option<(String, String)> =
        sqlx::query_as("SELECT tool, state FROM machine_jobs WHERE id = ? AND machine_id = ?")
            .bind(&b.job_id)
            .bind(&machine_id)
            .fetch_optional(&s.db)
            .await?;
    if let Some((tool, state)) = job
        && state == "sent"
    {
        let new_state = if b.refused { "refused" } else if b.ok { "done" } else { "failed" };
        sqlx::query("UPDATE machine_jobs SET state = ?, result = ?, done_at = ? WHERE id = ?")
            .bind(new_state)
            .bind(cut(&b.output, 64 * 1024))
            .bind(util::now())
            .bind(&b.job_id)
            .execute(&s.db)
            .await?;
        let tool: Value = serde_json::from_str(&tool).unwrap_or(Value::Null);
        let target = tool["path"].as_str().or(tool["cwd"].as_str()).or(tool["target"].as_str());
        sqlx::query(
            "INSERT INTO access_log (machine_id, user_id, at, kind, target, detail) VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(&machine_id)
        .bind(&user_id)
        .bind(util::now())
        .bind(if b.refused { "refused" } else { "used" })
        .bind(target)
        .bind(tool["tool"].as_str())
        .execute(&s.db)
        .await?;
    }
    if let Some(grants) = &b.grants {
        mirror_grants(&s.db, &machine_id, grants).await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::cut;

    #[test]
    fn cuts_on_char_boundaries() {
        assert_eq!(cut("abc", 10), "abc");
        assert_eq!(cut("aé", 2), "a");
    }
}
