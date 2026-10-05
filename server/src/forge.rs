//! TEN-03: a Forgejo (or Gitea) webhook keeps the task board true: a pull request whose
//! title carries a task id (`[TEN-03] ...`, or a branch `ten-03-...`) moves that task to
//! "in review" when opened, "done" when merged and back to "queued" when closed unmerged.
//! Only requests signed with FORGE_WEBHOOK_SECRET (HMAC-SHA256 of the body) are accepted;
//! only the tasks of FORGE_TASK_USER (a user name) are touched. Without both: 404.

use axum::{body::Bytes, extract::State, http::{HeaderMap, StatusCode}};
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use crate::{AppState, events::Event, util};

/// pure: hex text to bytes; None when not valid hex.
fn from_hex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 { return None; }
    (0..s.len()).step_by(2).map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok()).collect()
}

/// pure: true when `sig_hex` is the HMAC-SHA256 of `body` with `secret` (constant-time compare).
pub fn signature_ok(secret: &[u8], body: &[u8], sig_hex: &str) -> bool {
    let Some(sig) = from_hex(sig_hex.trim()) else { return false };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret) else { return false };
    mac.update(body);
    mac.verify_slice(&sig).is_ok()
}

/// pure: the task key a pull request names: "[ABC-12]" anywhere in the title (letters and digits,
/// one dash, letters and digits), else a branch that starts with such a key ("m6-05-x" -> "M6-05").
pub fn task_key(title: &str, branch: &str) -> Option<String> {
    let is_key = |k: &str| {
        let mut parts = k.split('-');
        let (Some(a), Some(b), None) = (parts.next(), parts.next(), parts.next()) else { return false };
        !a.is_empty() && !b.is_empty() && a.chars().all(|c| c.is_ascii_alphanumeric()) && b.chars().all(|c| c.is_ascii_alphanumeric())
    };
    let from_title = title.split('[').skip(1).filter_map(|rest| rest.split_once(']').map(|(k, _)| k)).find(|k| is_key(k));
    let from_branch = || {
        let mut parts = branch.split('-');
        let key = format!("{}-{}", parts.next()?, parts.next()?);
        is_key(&key).then_some(key)
    };
    from_title.map(str::to_string).or_else(from_branch).map(|k| k.to_ascii_uppercase())
}

/// pure: the state a task moves to, or None when nothing changes.
pub fn next_state(action: &str, merged: bool, current: &str) -> Option<&'static str> {
    match action {
        "opened" | "reopened" => {
            if matches!(current, "queued" | "running" | "needs_input" | "waiting_resources") {
                Some("in_review")
            } else {
                None
            }
        },
        "closed" => {
            if merged {
                if current == "done" {
                    None
                } else {
                    Some("done")
                }
            } else {
                if current == "in_review" {
                    Some("queued")
                } else {
                    None
                }
            }
        },
        _ => None,
    }
}

pub async fn webhook(State(s): State<AppState>, headers: HeaderMap, body: Bytes) -> StatusCode {
    let secret = std::env::var("FORGE_WEBHOOK_SECRET").unwrap_or_default();
    let owner = std::env::var("FORGE_TASK_USER").unwrap_or_default();
    
    if secret.is_empty() || owner.is_empty() {
        return StatusCode::NOT_FOUND;
    }
    
    let sig = headers.get("x-forgejo-signature").or_else(|| headers.get("x-gitea-signature"))
        .and_then(|v| v.to_str().ok()).unwrap_or("");
    
    if !signature_ok(secret.as_bytes(), &body, sig) {
        tracing::warn!("forge webhook: bad signature");
        return StatusCode::UNAUTHORIZED;
    }
    
    let event = headers.get("x-forgejo-event").or_else(|| headers.get("x-gitea-event"))
        .and_then(|v| v.to_str().ok()).unwrap_or("");
    
    if event != "pull_request" {
        return StatusCode::NO_CONTENT;
    }
    
    let Ok(v) = serde_json::from_slice::<Value>(&body) else { 
        return StatusCode::BAD_REQUEST; 
    };
    
    let pr = &v["pull_request"];
    let action = v["action"].as_str().unwrap_or("");
    let merged = pr["merged"].as_bool().unwrap_or(false);
    let title = pr["title"].as_str().unwrap_or("");
    let branch = pr["head"]["ref"].as_str().unwrap_or("");
    let url = pr["html_url"].as_str().unwrap_or("");
    
    let Some(key) = task_key(title, branch) else { 
        return StatusCode::NO_CONTENT; 
    };
    
    let user: Option<(String,)> = sqlx::query_as("SELECT id FROM users WHERE name = ?")
        .bind(&owner)
        .fetch_optional(&s.db)
        .await
        .ok()
        .flatten();
    
    let Some((user_id,)) = user else { 
        tracing::warn!("forge webhook: FORGE_TASK_USER not found");
        return StatusCode::NO_CONTENT; 
    };
    
    let task: Option<(String, String, String)> = sqlx::query_as(
        "SELECT id, state, title FROM tasks WHERE user_id = ? AND (upper(id) = ? OR upper(id) LIKE '%-' || ? OR upper(title) LIKE ? || ' %') ORDER BY updated_at DESC LIMIT 1"
    )
    .bind(&user_id)
    .bind(&key)
    .bind(&key)
    .bind(&key)
    .fetch_optional(&s.db)
    .await
    .ok()
    .flatten();
    
    let Some((task_id, current, task_title)) = task else { 
        return StatusCode::NO_CONTENT; 
    };
    
    let Some(to) = next_state(action, merged, &current) else { 
        return StatusCode::NO_CONTENT; 
    };
    
    if sqlx::query("UPDATE tasks SET state = ?, updated_at = ? WHERE id = ? AND user_id = ?")
        .bind(to)
        .bind(util::now())
        .bind(&task_id)
        .bind(&user_id)
        .execute(&s.db)
        .await
        .is_err() {
        return StatusCode::INTERNAL_SERVER_ERROR;
    }
    
    let _ = crate::tasks::history(&s.db, &task_id, &user_id, "forge", json!({ "state": { "from": current, "to": to }, "pr": url, "action": action })).await;
    let _ = crate::tasks::mark_dirty(&s.db, &task_id).await;
    crate::notify::task_changed(s.db.clone(), user_id.clone(), task_id.clone(), task_title, current, to.to_string());
    s.bus.send(&user_id, Event::Changed { what: "tasks", machine_id: None });
    
    tracing::info!(task = %task_id, to, "forge webhook moved a task");
    
    StatusCode::NO_CONTENT
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_matches_rfc_4231() {
        assert!(signature_ok(b"Jefe", b"what do ya want for nothing?", "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"));
        assert!(!signature_ok(b"Jefe", b"what do ya want for nothing!", "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"));
        assert!(!signature_ok(b"Jefe", b"x", "not hex"));
        assert!(!signature_ok(b"Jefe", b"x", ""));
    }

    #[test]
    fn keys_from_titles_and_branches() {
        assert_eq!(task_key("[TEN-03] Webhook", "x").as_deref(), Some("TEN-03"));
        assert_eq!(task_key("Fix [m6-05] cards", "").as_deref(), Some("M6-05"));
        assert_eq!(task_key("no key [a b] [x]", "m6-05-workflows").as_deref(), Some("M6-05"));
        assert_eq!(task_key("plain", "main"), None);
        assert_eq!(task_key("[A-B-C]", "feature"), None);
    }

    #[test]
    fn states_follow_the_pr() {
        assert_eq!(next_state("opened", false, "queued"), Some("in_review"));
        assert_eq!(next_state("opened", false, "done"), None);
        assert_eq!(next_state("closed", true, "in_review"), Some("done"));
        assert_eq!(next_state("closed", true, "done"), None);
        assert_eq!(next_state("closed", false, "in_review"), Some("queued"));
        assert_eq!(next_state("closed", false, "running"), None);
        assert_eq!(next_state("synchronized", false, "queued"), None);
    }
}
