```rust
use axum::{Extension, Json, extract::{Path, State}, http::{HeaderMap, StatusCode, header}};
use serde::Deserialize;
use serde_json::{Value, json};
use crate::{AppState, auth::User, error::{ApiError, ApiResult}, util};

#[derive(Deserialize)]
pub struct TargetBody {
    pub target: String,
}

#[derive(Deserialize)]
pub struct AddBody {
    pub target: String,
    pub rights: Vec<String>,
}

fn valid_grant(target: &str, rights: &[String]) -> bool {
    if rights.is_empty() {
        return false;
    }
    if target == "system" {
        return true;
    }
    if !target.starts_with('/') {
        return false;
    }
    // Check for ".." in any path component
    let components: Vec<&str> = target.split('/').collect();
    for comp in components {
        if comp == ".." {
            return false;
        }
    }
    for r in rights {
        match r.as_str() {
            "read" | "write" | "shell" => continue,
            _ => return false,
        }
    }
    true
}

async fn owned(s: &AppState, machine_id: &str, u: &User) -> ApiResult<()> {
    let rows: Vec<(i64,)> = sqlx::query_as("SELECT 1 FROM machines WHERE id = ? AND user_id = ?")
        .bind(machine_id)
        .bind(&u.id)
        .fetch_all(&s.db)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    if rows.is_empty() {
        return Err(ApiError::NotFound);
    }
    Ok(())
}

pub async fn list_grants(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<Value>>> {
    owned(&s, &id, &u).await?;

    let rows: Vec<(String, Value, String, String, Option<String>)> = sqlx::query_as(
        "SELECT target, rights, granted_by, granted_at, expires FROM machine_grants WHERE machine_id = ?"
    )
    .bind(&id)
    .fetch_all(&s.db)
    .await
    .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    let results = rows
        .into_iter()
        .map(|(target, rights, granted_by, granted_at, expires)| {
            let rights_vec: Vec<String> = rights.as_array()
                .map(|arr| arr.iter().filter_map(|v| v.as_str().map(|s| s.to_string())).collect())
                .unwrap_or_default();

            json!({
                "target": target,
                "rights": rights_vec,
                "grantedBy": granted_by,
                "grantedAt": granted_at,
                "expires": expires.unwrap_or_default()
            })
        })
        .collect();

    Ok(Json(results))
}

pub async fn revoke_grant(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
    Json(b): Json<TargetBody>,
) -> ApiResult<StatusCode> {
    owned(&s, &id, &u).await?;

    let tool_json = json!({
        "tool": "revoke_grant",
        "target": b.target
    });

    super::runner::queue_job(s.db.clone(), &id, &u.id, &tool_json, None)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    sqlx::query("INSERT INTO access_log (machine_id, user_id, at, kind, target, detail) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(&u.id)
        .bind(&util::now())
        .bind("revoked")
        .bind(&b.target)
        .bind("requested")
        .execute(&s.db)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    Ok(StatusCode::ACCEPTED)
}

pub async fn add_grant(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
    Json(b): Json<AddBody>,
) -> ApiResult<StatusCode> {
    owned(&s, &id, &u).await?;

    if !valid_grant(&b.target, &b.rights) {
        return Err(ApiError::BadRequest("That grant isn't valid: use an absolute folder or \"system\", and rights read, write or shell.".to_string()));
    }

    let tool_json = json!({
        "tool": "add_grant",
        "grant": {
            "target": b.target,
            "rights": b.rights,
            "granted_by": u.name,
            "granted_at": util::now()
        }
    });

    super::runner::queue_job(s.db.clone(), &id, &u.id, &tool_json, None)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    sqlx::query("INSERT INTO access_log (machine_id, user_id, at, kind, target, detail) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(&u.id)
        .bind(&util::now())
        .bind("granted")
        .bind(&b.target)
        .bind("added")
        .execute(&s.db)
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?;

    Ok(StatusCode::ACCEPTED)
}

pub async fn history(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
) -> ApiResult<Json<Vec<Value>>> {
    let rows: Vec<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT a.at, a.kind, a.target, a.detail, COALESCE(m.name, '') FROM access_log a LEFT JOIN machines m ON m.id = a.machine_id WHERE a.user_id = ? ORDER BY a.id DESC LIMIT 200"
    )
    .bind(&u.id)
    .fetch_all(&s.db)
    .await
    .map_err(|e| ApiError::BadRequest(e.to
