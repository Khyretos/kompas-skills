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
        .map(|(target, rights, granted_by, granted_at, expires)| {
            let rights_vec: Vec<String> = rights
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            json!({
                "target": target,
                "rights": rights_vec
