use axum::{Extension, Json, extract::{Path, State}, http::StatusCode};
use serde::Deserialize;
use serde_json::{Value, json};
use crate::{AppState, auth::User, error::{ApiError, ApiResult}};
use super::runner::queue_job;

#[derive(Deserialize)]
pub struct NewJob {
    pub tool: Value,
}

pub async fn create_job(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
    Json(b): Json<NewJob>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    // Verify machine ownership
    let row: Option<(i64,)> = sqlx::query_as(
        "SELECT 1 FROM machines WHERE id = ? AND user_id = ?"
    )
    .bind(&id)
    .bind(&u.id)
    .fetch_optional(&s.db)
    .await?;

    if row.is_none() {
        return Err(ApiError::NotFound);
    }

    // Validate tool type
    let tool_name = b.tool["tool"].as_str().ok_or_else(|| {
        ApiError::BadRequest("Tool name missing.".into())
    })?;

    match tool_name {
        "read_file" | "write_file" | "list_dir" | "shell" => (),
        _ => return Err(ApiError::BadRequest("Unknown tool.".into())),
    }

    // Validate payload size
    let tool_json = b.tool.to_string();
    if tool_json.len() > 1_100_000 {
        return Err(ApiError::BadRequest("Too big.".into()));
    }

    // Queue the job
    let job_id = queue_job(&s.db, &id, &u.id, &b.tool, None)
        .await
        .map_err(ApiError::Internal)?;

    Ok((StatusCode::ACCEPTED, Json(json!({ "id": job_id }))))
}

pub async fn get_job(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path((id, machine_id)): Path<(String, String)>,
) -> ApiResult<Json<Value>> {
    let row: Option<(String, Option<String>, String, Option<String>)> =
        sqlx::query_as(
            "SELECT state, result, created_at, done_at FROM machine_jobs WHERE id = ? AND machine_id = ? AND user_id = ?"
        )
        .bind(&id)
        .bind(&machine_id)
        .bind(&u.id)
        .fetch_optional(&s.db)
        .await?;

    if let Some((state, result, created_at, done_at)) = row {
        Ok(Json(json!({
            "id": id,
            "state": state,
            "result": result.unwrap_or_default(),
            "createdAt": created_at,
            "doneAt": done_at.unwrap_or_default(),
        })))
    } else {
        Err(ApiError::NotFound)
    }
}
