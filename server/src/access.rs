use axum::extract::{Path, Query, State, HeaderMap};
use axum::http::StatusCode;
use axum::response::Json;
use axum::Extension;
use serde::{Deserialize, Serialize};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use sqlx::SqlitePool;
use std::fmt::Write;

use crate::AppState;
use crate::error::{ApiError, ApiResult};
use crate::util::{now, new_id, sha256_hex};

#[derive(Deserialize)]
pub struct ResultsBody {
    job_id: String,
    ok: bool,
    output: String,
    #[serde(default)]
    refused: bool,
    #[serde(default)]
    grants: Option<Vec<serde_json::Value>>,
}

#[derive(Deserialize)]
pub struct TargetBody {
    target: String,
}

#[derive(Deserialize)]
pub struct AddBody {
    target: String,
    rights: Vec<String>,
}

pub async fn runner_user(s: &AppState, machine_id: &str, headers: &HeaderMap) -> ApiResult<String> {
    let auth_header = headers.get("Authorization");
    let Some(auth_header) = auth_header else {
        return Err(ApiError::Unauthorized);
    };

    let auth_header = auth_header.to_str().ok().filter(|h| h.starts_with("Bearer "));

    let Some(token) = auth_header.map(|h| h.split_once(' ').map(|(_, t)| t).unwrap_or_default()) else {
        return Err(ApiError::Unauthorized);
    };

    let token_hash = sha256_hex(token);

    let user_id = sqlx::query_scalar::<_, String>(
        "SELECT user_id FROM machines WHERE id = ? AND token_hash = ?",
    )
    .bind(machine_id)
    .bind(token_hash)
    .fetch_optional(&s.db)
    .await?
    .ok_or(ApiError::Unauthorized)?;

    Ok(user_id)
}

pub async fn queue_job(
    db: &SqlitePool,
    machine_id: &str,
    user_id: &str,
    tool: &serde_json::Value,
    task_id: Option<&str>,
) -> sqlx::Result<String> {
    let job_id = new_id();
    let created_at = now();
    let state = "queued";

    sqlx::query!(
        "INSERT INTO machine_jobs (id, machine_id, user_id, tool, state, created_at, task_id) VALUES (?, ?, ?, ?, ?, ?, ?)",
        job_id,
        machine_id,
        user_id,
        tool,
        state,
        created_at,
        task_id.unwrap_or_default()
    )
    .execute(db)
    .await?;

    Ok(job_id)
}

pub async fn take_jobs(db: &SqlitePool, machine_id: &str) -> sqlx::Result<Vec<serde_json::Value>> {
    let jobs = sqlx::query!(
        "SELECT id, tool FROM machine_jobs WHERE machine_id = ? AND state = 'queued' ORDER BY created_at ASC LIMIT 10",
        machine_id
    )
    .fetch_all(db)
    .await?;

    let mut result = Vec::new();
    for job in jobs {
        let tool = job.tool;
        let parsed_tool: serde_json::Value = serde_json::from_str(&tool)?;
        result.push(parsed_tool);
    }

    // Mark jobs as 'sent' with sent_at
    let now = now();
    for job in jobs {
        sqlx::query!(
            "UPDATE machine_jobs SET state = 'sent', sent_at = ? WHERE id = ?",
            now,
            job.id
        )
        .execute(db)
        .await?;
    }

    Ok(result)
}

pub async fn mirror_grants(
    db: &SqlitePool,
    machine_id: &str,
    grants: &[serde_json::Value],
) -> sqlx::Result<()> {
    let mut tx = db.begin().await?;

    // Delete existing grants for this machine
    sqlx::query!(
        "DELETE FROM machine_grants WHERE machine_id = ?",
        machine_id
    )
    .execute(&mut *tx)
    .await?;

    // Insert new grants
    for grant in grants {
        let grant = grant.clone();
        let target = grant.get("target").ok_or(sqlx::Error::InvalidArgument)?;
        let rights = grant.get("rights").ok_or(sqlx::Error::InvalidArgument)?;
        let granted_by = grant.get("granted_by").ok_or(sqlx::Error::InvalidArgument)?;
        let granted_at = grant.get("granted_at").ok_or(sqlx::Error::InvalidArgument)?;
        let expires = grant.get("expires");

        sqlx::query!(
            "INSERT INTO machine_grants (machine_id, target, rights, granted_by, granted_at, expires) VALUES (?, ?, ?, ?, ?, ?)",
            machine_id,
            target,
            rights,
            granted_by,
            granted_at,
            expires
        )
        .execute(&mut *tx)
        .await?;
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
    let user_id = runner_user(&s, &id, &headers).await?;
    let job_id = b.job_id.clone();
    let output = b.output.clone();
    let ok = b.ok;
    let refused = b.refused;
    let grants = b.grants.clone();

    let job = sqlx::query!(
        "SELECT id, state, tool FROM machine_jobs WHERE id = ? AND machine_id = ? AND state = 'sent'",
        job_id,
        id
    )
    .fetch_optional(&s.db)
    .await?
    .ok_or(ApiError::NotFound)?;

    if job.id != job_id {
        return Err(ApiError::NotFound);
    }

    let state = if refused {
        "refused"
    } else if ok {
        "done"
    } else {
        "failed"
    };

    let done_at = now();
    let result = if output.len() > 65536 {
        &output[0..65536]
    } else {
        &output
    };

    sqlx::query!(
        "UPDATE machine_jobs SET state = ?, result = ?, done_at = ? WHERE id = ?",
        state,
        result,
        done_at,
        job_id
    )
    .execute(&s.db)
    .await?;

    let tool = job.tool;
    let tool: serde_json::Value = serde_json::from_str(&tool)?;
    let target = tool.get("path").or(tool.get("cwd")).and_then(|v| v.as_str()).unwrap_or_default();

    sqlx::query!(
        "INSERT INTO access_log (machine_id, user_id, at, kind, target, detail) VALUES (?, ?, ?, ?, ?, ?)",
        id,
        user_id,
        done_at,
        if refused { "refused" } else { "used" },
        target,
        tool.get("tool").and_then(|v| v.as_str()).unwrap_or_default()
    )
    .execute(&s.db)
    .await?;

    if let Some(grants) = grants {
        mirror_grants(&s.db, &id, &grants).await?;
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn list_grants(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<serde_json::Value>>> {
    let machine_id = id.as_str();
    let user_id = u.id.as_str();

    let rows = sqlx::query!(
        "SELECT target, rights, granted_by, granted_at, expires FROM machine_grants WHERE machine_id = ?",
        machine_id
    )
    .fetch_all(&s.db)
    .await?;

    if rows.is_empty() {
        return Err(ApiError::NotFound);
    }

    let mut grants = Vec::new();
    for row in rows {
        let target = row.target;
        let rights = row.rights;
        let granted_by = row.granted_by;
        let granted_at = row.granted_at;
        let expires = row.expires;

        let rights: Vec<String> = serde_json::from_str(&rights)?;
        let grant = serde_json::json!({
            "target": target,
            "rights": rights,
            "granted_by": granted_by,
            "granted_at": granted_at,
            "expires": expires
        });

        grants.push(grant);
    }

    Ok(Json(grants))
}

pub async fn revoke_grant(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
    Json(b): Json<TargetBody>,
) -> ApiResult<StatusCode> {
    let machine_id = id.as_str();
    let user_id = u.id.as_str();
    let target = b.target.clone();

    // Check if the machine belongs to the user
    let rows = sqlx::query!(
        "SELECT id FROM machines WHERE id = ? AND user_id = ?",
        machine_id,
        user_id
    )
    .fetch_all(&s.db)
    .await?;

    if rows.is_empty() {
        return Err(ApiError::NotFound);
    }

    let job_id = queue_job(
        &s.db,
        machine_id,
        user_id,
        &serde_json::json!({"tool": "revoke_grant", "target": target}),
        None,
    )
    .await?;

    sqlx::query!(
        "INSERT INTO access_log (machine_id, user_id, at, kind, detail) VALUES (?, ?, ?, ?, ?)",
        machine_id,
        user_id,
        now(),
        "revoked",
        "requested"
    )
    .execute(&s.db)
    .await?;

    Ok(StatusCode::ACCEPTED)
}

pub async fn add_grant(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
    Json(b): Json<AddBody>,
) -> ApiResult<StatusCode> {
    let machine_id = id.as_str();
    let user_id = u.id.as_str();
    let target = b.target.clone();
    let rights = b.rights.clone();

    // Check if the machine belongs to the user
    let rows = sqlx::query!(
        "SELECT id FROM machines WHERE id = ? AND user_id = ?",
        machine_id,
        user_id
    )
    .fetch_all(&s.db)
    .await?;

    if rows.is_empty() {
        return Err(ApiError::NotFound);
    }

    // Validate target and rights
    if !valid_grant(&target, &rights) {
        return Err(ApiError::BadRequest("Invalid target or rights".to_string()));
    }

    let granted_at = now();
    let grant = serde_json::json!({
        "target": target,
        "rights": rights,
        "granted_by": u.name,
        "granted_at": granted_at
    });

    let job_id = queue_job(
        &s.db,
        machine_id,
        user_id,
        &grant,
        None,
    )
    .await?;

    sqlx::query!(
        "INSERT INTO access_log (machine_id, user_id, at, kind, detail) VALUES (?, ?, ?, ?, ?)",
        machine_id,
        user_id,
        now(),
        "granted",
        "requested"
    )
    .execute(&s.db)
    .await?;

    Ok(StatusCode::ACCEPTED)
}

pub async fn history(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
) -> ApiResult<Json<Vec<serde_json::Value>>> {
    let user_id = u.id.as_str();

    let rows = sqlx::query!(
        "SELECT al.id, al.machine_id, al.user_id, al.at, al.kind, al.target, al.detail, m.name AS machine_name FROM access_log al JOIN machines m ON al.machine_id = m.id WHERE al.user_id = ? ORDER BY al.at DESC LIMIT 200",
        user_id
    )
    .fetch_all(&s.db)
    .await?;

    let mut history = Vec::new();
    for row in rows {
        let mut entry = serde_json::json!({
            "id": row.id,
            "machine_id": row.machine_id,
            "user_id": row.user_id,
            "at": row.at,
            "kind": row.kind,
            "target": row.target,
            "detail": row.detail,
            "machine_name": row.machine_name
        });
        history.push(entry);
    }

    Ok(Json(history))
}

fn valid_grant(target: &str, rights: &[String]) -> bool {
    if target == "system" || target.starts_with('/') && !target.contains("..") {
        for right in rights {
            if !["read", "write", "shell"].contains(&&*right) {
                return false;
            }
        }
        return true;
    }
    false
}
