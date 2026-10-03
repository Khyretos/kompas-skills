//! Task notifications by mail: per-user switches (needs you, failed, done, a
//! daily summary). Mails carry the task title and its new state only, never
//! descriptions, prompts or secrets. At most one mail per task per 10 minutes.

use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use axum::{Extension, Json, extract::State, http::StatusCode};
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::{
    AppState,
    auth::User,
    error::{ApiError, ApiResult},
    mail, util,
};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Prefs {
    pub email: String,
    pub on_needs_input: bool,
    pub on_failed: bool,
    pub on_done: bool,
    pub daily_summary: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs { email: String::new(), on_needs_input: true, on_failed: true, on_done: false, daily_summary: false }
    }
}

/// The user's switches; the address defaults to the account's email (from
/// single sign-on), so notifications work without any setup.
async fn prefs(db: &SqlitePool, user_id: &str) -> sqlx::Result<Prefs> {
    let mut p: Prefs = sqlx::query_as(
        "SELECT email, on_needs_input, on_failed, on_done, daily_summary FROM notification_prefs WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?
    .unwrap_or_default();
    if p.email.is_empty() {
        let account: Option<(Option<String>,)> = sqlx::query_as("SELECT email FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(db)
            .await?;
        p.email = account.and_then(|a| a.0).unwrap_or_default();
    }
    Ok(p)
}

pub async fn get_prefs(State(s): State<AppState>, Extension(u): Extension<User>) -> ApiResult<Json<Prefs>> {
    Ok(Json(prefs(&s.db, &u.id).await?))
}

pub async fn put_prefs(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Json(mut p): Json<Prefs>,
) -> ApiResult<StatusCode> {
    p.email = p.email.trim().to_string();
    if !p.email.is_empty()
        && (p.email.len() > 200 || p.email.matches('@').count() != 1 || p.email.contains(char::is_whitespace))
    {
        return Err(ApiError::BadRequest("That doesn't look like a mail address.".into()));
    }
    sqlx::query(
        "INSERT INTO notification_prefs (user_id, email, on_needs_input, on_failed, on_done, daily_summary)
         VALUES (?, ?, ?, ?, ?, ?)
         ON CONFLICT(user_id) DO UPDATE SET email = excluded.email, on_needs_input = excluded.on_needs_input,
           on_failed = excluded.on_failed, on_done = excluded.on_done, daily_summary = excluded.daily_summary",
    )
    .bind(&u.id)
    .bind(&p.email)
    .bind(p.on_needs_input)
    .bind(p.on_failed)
    .bind(p.on_done)
    .bind(p.daily_summary)
    .execute(&s.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Which label a move to `to` gets, if this user wants a mail for it.
fn wanted(p: &Prefs, to: &str) -> Option<&'static str> {
    match to {
        "needs_input" | "waiting_resources" if p.on_needs_input => Some("needs you"),
        "failed" if p.on_failed => Some("failed"),
        "done" if p.on_done => Some("done"),
        _ => None,
    }
}

static SENT: LazyLock<Mutex<HashMap<String, Instant>>> = LazyLock::new(Default::default);

async fn send(db: &SqlitePool, to: &str, subject_tail: &str, body: &str) -> anyhow::Result<()> {
    let st = crate::admin::load(db).await?;
    anyhow::ensure!(!st.smtp_host.is_empty() && !st.smtp_from.is_empty(), "mail is not set up");
    let smtp = mail::SmtpSettings {
        host: st.smtp_host,
        port: st.smtp_port,
        tls: st.smtp_tls,
        user: st.smtp_user,
        from: st.smtp_from,
        reply_to: st.smtp_reply_to,
    };
    let password = std::env::var("SMTP_PASSWORD").ok().filter(|p| !p.is_empty());
    mail::send(&smtp, password.as_deref(), to, &format!("{}: {subject_tail}", st.app_name), body).await
}

/// Call after a task's state changed; mails in the background if wanted.
pub fn task_changed(db: SqlitePool, user_id: String, title: String, from: String, to: String) {
    if from == to {
        return;
    }
    tokio::spawn(async move {
        let p = match prefs(&db, &user_id).await {
            Ok(p) => p,
            Err(e) => return tracing::warn!("notification prefs: {e}"),
        };
        let Some(label) = wanted(&p, &to) else { return };
        if p.email.is_empty() {
            return;
        }
        {
            let mut sent = SENT.lock().unwrap();
            let key = format!("{user_id}/{title}");
            if sent.get(&key).is_some_and(|t| t.elapsed() < Duration::from_secs(600)) {
                return;
            }
            sent.insert(key, Instant::now());
        }
        let body = format!("{title}\nNow: {label}.\nOpen Kompanion to see the details.\n");
        if let Err(e) = send(&db, &p.email, &format!("{title} — {label}"), &body).await {
            tracing::warn!("task mail failed: {e:#}");
        }
    });
}

/// Daily summary at or after 08:00 UTC, once per day per user.
pub fn spawn_daily(db: SqlitePool) {
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(900)).await;
            let now = util::now();
            if now.get(11..13).and_then(|h| h.parse::<u32>().ok()).unwrap_or(0) < 8 {
                continue;
            }
            let today = &now[..10];
            let users: Vec<(String, String)> = match sqlx::query_as(
                "SELECT user_id, email FROM notification_prefs
                 WHERE daily_summary = 1 AND email <> '' AND (last_daily IS NULL OR substr(last_daily, 1, 10) <> ?)",
            )
            .bind(today)
            .fetch_all(&db)
            .await
            {
                Ok(u) => u,
                Err(e) => {
                    tracing::warn!("daily summary: {e}");
                    continue;
                }
            };
            for (user_id, email) in users {
                let counts: Vec<(String, i64)> = sqlx::query_as(
                    "SELECT state, COUNT(*) FROM tasks WHERE user_id = ? GROUP BY state ORDER BY state",
                )
                .bind(&user_id)
                .fetch_all(&db)
                .await
                .unwrap_or_default();
                let body: String = counts.iter().map(|(s, n)| format!("{}: {n}\n", s.replace('_', " "))).collect();
                if let Err(e) = send(&db, &email, "today", &body).await {
                    tracing::warn!("daily summary mail failed: {e:#}");
                    continue;
                }
                let _ = sqlx::query("UPDATE notification_prefs SET last_daily = ? WHERE user_id = ?")
                    .bind(util::now())
                    .bind(&user_id)
                    .execute(&db)
                    .await;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_wanted_moves_mail() {
        let p = Prefs::default();
        assert_eq!(wanted(&p, "needs_input"), Some("needs you"));
        assert_eq!(wanted(&p, "failed"), Some("failed"));
        assert_eq!(wanted(&p, "done"), None);
        assert_eq!(wanted(&Prefs { on_done: true, ..p }, "done"), Some("done"));
    }
}
