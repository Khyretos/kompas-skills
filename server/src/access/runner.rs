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
        .and_then(|
