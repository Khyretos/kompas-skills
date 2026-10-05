//! UnifiedPush to the Android app; endpoints only on the push servers listed in `kompanion.toml` `[push] servers` (no user-chosen URLs: no SSRF); a push carries the task title, its new state and a link, never descriptions or prompts.

use std::sync::{LazyLock, OnceLock};
use axum::{Extension, Json, extract::State, http::StatusCode};
use serde::Deserialize;
use sqlx::SqlitePool;
use crate::{AppState, auth::User, error::{ApiError, ApiResult}, util};

/// Push servers endpoints may live on ("https://ntfy.example.com"), set at start from the config.
pub static SERVERS: OnceLock<Vec<String>> = OnceLock::new();
static CLIENT: LazyLock<reqwest::Client> = LazyLock::new(|| reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build().expect("http client"));

/// True only for an https URL on one of `servers` whose path is a UnifiedPush topic
/// ("up" + at least 8 letters or digits), optionally followed by "?up=1".
pub fn endpoint_allowed(endpoint: &str, servers: &[String]) -> bool {
    let e = endpoint;
    if e.len() > 500 || !e.starts_with("https://") || e.chars().any(char::is_whitespace) {
        return false;
    }
    let (path_end, query) = match e.find('?') {
        Some(i) => (&e[..i], Some(&e[i + 1..])),
        None => (e, None),
    };
    if query.is_some_and(|q| q != "up=1") {
        return false;
    }
    servers.iter().any(|s| {
        let base = format!("{}/", s.trim_end_matches('/'));
        path_end
            .strip_prefix(base.as_str())
            .and_then(|topic| topic.strip_prefix("up"))
            .is_some_and(|rest| rest.len() >= 8 && rest.chars().all(|c| c.is_ascii_alphanumeric()))
    })
}

/// The push body: title, new state and link only.
pub fn payload(title: &str, label: &str, link: &str) -> String {
    serde_json::json!({"title": title, "state": label, "url": link}).to_string()
}

#[derive(Deserialize)]
pub struct Register { 
    pub endpoint: String, 
    #[serde(default)] 
    pub device: String 
}

pub async fn register(State(s): State<AppState>, Extension(u): Extension<User>, Json(r): Json<Register>) -> ApiResult<StatusCode> {
    let servers = SERVERS.get().map(Vec::as_slice).unwrap_or(&[]);
    
    if !endpoint_allowed(r.endpoint.trim(), servers) {
        return Err(ApiError::BadRequest("That push address is not on an allowed push server.".into()));
    }
    
    let device = r.device.trim().chars().take(80).collect::<String>();
    
    sqlx::query("INSERT INTO push_endpoints (id, user_id, endpoint, device, created_at) VALUES (?, ?, ?, ?, ?) ON CONFLICT(user_id, endpoint) DO UPDATE SET device = excluded.device")
        .bind(util::new_id())
        .bind(&u.id)
        .bind(r.endpoint.trim())
        .bind(device)
        .bind(util::now())
        .execute(&s.db)
        .await?;
    
    Ok(StatusCode::NO_CONTENT)
}

pub async fn unregister(State(s): State<AppState>, Extension(u): Extension<User>, Json(r): Json<Register>) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM push_endpoints WHERE user_id = ? AND endpoint = ?")
        .bind(&u.id)
        .bind(r.endpoint.trim())
        .execute(&s.db)
        .await?;
    
    Ok(StatusCode::NO_CONTENT)
}

/// Sends to every endpoint of the user in the background; never fails the caller.
pub fn notify(db: SqlitePool, user_id: String, title: String, label: &'static str, link: String) {
    tokio::spawn(async move {
        let rows: Result<Vec<(String, String)>, _> = sqlx::query_as("SELECT id, endpoint FROM push_endpoints WHERE user_id = ?")
            .bind(&user_id)
            .fetch_all(&db)
            .await;
        
        let rows = match rows {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!("push endpoints: {e}");
                return;
            }
        };
        
        let body = payload(&title, label, &link);
        
        for (id, endpoint) in rows {
            // Never log the endpoint itself
            match CLIENT.post(&endpoint)
                .header("Content-Type", "application/json")
                .body(body.clone())
                .send()
                .await
            {
                Ok(resp) if resp.status().is_success() => {
                    let _ = sqlx::query("UPDATE push_endpoints SET last_ok_at = ? WHERE id = ?")
                        .bind(util::now())
                        .bind(id)
                        .execute(&db)
                        .await;
                },
                Ok(resp) if resp.status().as_u16() == 404 || resp.status().as_u16() == 410 => {
                    let _ = sqlx::query("DELETE FROM push_endpoints WHERE id = ?")
                        .bind(id)
                        .execute(&db)
                        .await;
                    tracing::info!("push endpoint gone, removed");
                },
                Ok(resp) => {
                    tracing::warn!("push failed: {}", resp.status());
                },
                Err(e) => {
                    tracing::warn!("push failed: {e}");
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn endpoints() {
        let servers = vec!["https://ntfy.example.com/".to_string()];
        
        // allowed
        assert!(endpoint_allowed("https://ntfy.example.com/upAbC12345xyz", &servers));
        assert!(endpoint_allowed("https://ntfy.example.com/upAbC12345xyz?up=1", &servers));
        
        // refused
        assert!(!endpoint_allowed("http://ntfy.example.com/upAbC12345xyz", &servers));
        assert!(!endpoint_allowed("https://evil.example.com/upAbC12345xyz", &servers));
        assert!(!endpoint_allowed("https://ntfy.example.com.evil.com/upAbC12345xyz", &servers));
        assert!(!endpoint_allowed("https://ntfy.example.com/secret", &servers));
        assert!(!endpoint_allowed("https://ntfy.example.com/upshort", &servers));
        assert!(!endpoint_allowed("https://ntfy.example.com/upAbC12345xyz/../x", &servers));
        assert!(!endpoint_allowed("https://ntfy.example.com/upAbC12345xyz?up=1&x=2", &servers));
        assert!(!endpoint_allowed("https://ntfy.example.com/up AbC12345xyz", &servers));
    }
    
    #[test]
    fn payload_has_no_extras() {
        let json = payload("T", "done", "https://k/#task=1");
        let map: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&json).unwrap();
        
        assert_eq!(map.len(), 3);
        assert!(map.contains_key("title"));
        assert!(map.contains_key("state"));
        assert!(map.contains_key("url"));
        
        assert!(!map.contains_key("description"));
        assert!(!map.contains_key("prompt"));
    }
}
