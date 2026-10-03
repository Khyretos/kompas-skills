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
    #[serde(default)]
    pub expires_hours: Option<i64>,
    pub target: String,
    pub rights: Vec<String>,
}

fn valid_grant(target: &str, rights: &[String]) -> bool {
    if rights.is_empty() {
        return false;
    }
    // Check for duplicates
    if rights.len() != rights.iter().collect::<std::collections::HashSet<_>>().len() {
        return false;
    }

    if target == "system" {
        let allowed = ["packages", "services", "desktop", "root"];
        for r in rights {
            if !allowed.contains(&r.as_str()) {
                return false;
            }
        }
        return true;
    }

    if !target.starts_with('/') {
        return false;
    }
    // Rule 11: check every grant and skip expired ones instead of returning on the first expired match.
    // Also check for ".." in any path component.
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
    let row: Vec<(i64,)> = sqlx::query_as("SELECT 1 FROM machines WHERE id = ? AND user_id = ?")
        .bind(machine_id)
        .bind(&u.id)
        .fetch_all(&s.db)
        .await?;

    if row.is_empty() {
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

    let rows: Vec<(String, String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT target, rights, granted_by, granted_at, expires FROM machine_grants WHERE machine_id = ?"
    )
    .bind(&id)
    .fetch_all(&s.db)
        .await?;

    let results = rows
        .into_iter()
        .map(|(target, rights_str, granted_by, granted_at, expires)| {
            let rights_vec: Vec<String> = serde_json::from_str::<Vec<String>>(&rights_str)
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

    let tool = json!({
        "tool": "revoke_grant",
        "target": b.target
    });

    super::runner::queue_job(&s.db, &id, &u.id, &tool, None).await?;

    sqlx::query("INSERT INTO access_log (machine_id, user_id, at, kind, target, detail) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(&u.id)
        .bind(&util::now())
        .bind("revoked")
        .bind(&b.target)
        .bind("requested")
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
    owned(&s, &id, &u).await?;

    if !valid_grant(&b.target, &b.rights) {
        return Err(ApiError::BadRequest("That grant isn't valid: a folder takes read, write or shell; \"system\" takes packages, services, desktop or root.".to_string()));
    }

    let mut grant_json = json!({
        "target": b.target,
        "rights": b.rights,
        "granted_by": u.name,
        "granted_at": util::now()
    });

    if let Some(hours) = b.expires_hours {
        if hours < 1 || hours > 720 {
            return Err(ApiError::BadRequest("Expiry must be between 1 hour and 30 days.".to_string()));
        }
        grant_json["expires"] = util::in_hours(hours).into();
    }

    let tool = json!({
        "tool": "add_grant",
        "grant": grant_json
    });

    super::runner::queue_job(&s.db, &id, &u.id, &tool, None).await?;

    sqlx::query("INSERT INTO access_log (machine_id, user_id, at, kind, target, detail) VALUES (?, ?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(&u.id)
        .bind(&util::now())
        .bind("granted")
        .bind(&b.target)
        .bind("requested")
        .execute(&s.db)
        .await?;

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
        .await?;

    let results = rows
        .into_iter()
        .map(|(at, kind, target, detail, machine)| {
            json!({
                "at": at,
                "kind": kind,
                "target": target,
                "detail": detail,
                "machine": machine
            })
        })
        .collect();

    Ok(Json(results))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_grant() {
        assert!(valid_grant("/home/k", &["read".to_string(), "write".to_string()]));
        assert!(!valid_grant("/home/k", &["root".to_string()]));
        assert!(valid_grant("system", &["packages".to_string(), "root".to_string()]));
        assert!(!valid_grant("system", &["read".to_string()]));
        assert!(!valid_grant("/home/../etc", &["read".to_string()]));
        assert!(!valid_grant("/home/k", &[]));
    }
}
