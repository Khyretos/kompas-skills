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

/// The app's public address for links in mails ("https://kompanion.example.com"), set at start.
pub static PUBLIC_URL: std::sync::OnceLock<String> = std::sync::OnceLock::new();

/// A Reply-To with a display name: a bare address shows whatever name the reader's mail app
/// learned for it (info@ showed as "Watchtower", whose updates come from that address).
fn reply_to_with_name(reply_to: &str) -> String {
    let r = reply_to.trim();
    if r.is_empty() || r.contains('<') { r.to_string() } else { format!("Kreative Kompas <{r}>") }
}

static SENT: LazyLock<Mutex<HashMap<String, Instant>>> = LazyLock::new(Default::default);

/// The parts of a branded mail besides the app's own name, link, logo and colour.
struct Card<'a> {
    status: crate::mailhtml::Status,
    title: &'a str,
    intro: &'a str,
    rows: &'a [(&'a str, &'a str)],
    button: Option<(&'a str, &'a str)>,
}

/// The branded HTML of a mail, with this server's name, link, logo and colour.
async fn card_html(db: &SqlitePool, c: Card<'_>) -> anyhow::Result<String> {
    let st = crate::admin::load(db).await?;
    let url = PUBLIC_URL.get().cloned().unwrap_or_default();
    let logo = if url.is_empty() { String::new() } else { format!("{url}/api/mail-logo.png") };
    Ok(crate::mailhtml::render(&crate::mailhtml::Mail {
        app_name: &st.app_name, app_url: &url, logo_url: &logo, brand: &st.color_brand,
        status: c.status, title: c.title, intro: c.intro, rows: c.rows, button: c.button,
    }))
}

async fn send(db: &SqlitePool, to: &str, subject_tail: &str, body: &str, card: Option<Card<'_>>) -> anyhow::Result<()> {
    let st = crate::admin::load(db).await?;
    anyhow::ensure!(!st.smtp_host.is_empty() && !st.smtp_from.is_empty(), "mail is not set up");
    let smtp = mail::SmtpSettings {
        host: st.smtp_host,
        port: st.smtp_port,
        tls: st.smtp_tls,
        user: st.smtp_user,
        from: st.smtp_from,
        reply_to: reply_to_with_name(&st.smtp_reply_to),
    };
    let password = std::env::var("SMTP_PASSWORD").ok().filter(|p| !p.is_empty());
    let url = PUBLIC_URL.get().cloned().unwrap_or_default();
    let logo = if url.is_empty() { String::new() } else { format!("{url}/api/mail-logo.png") };
    let html = card.map(|c| {
        crate::mailhtml::render(&crate::mailhtml::Mail {
            app_name: &st.app_name, app_url: &url, logo_url: &logo, brand: &st.color_brand,
            status: c.status, title: c.title, intro: c.intro, rows: c.rows, button: c.button,
        })
    });
    mail::send(&smtp, password.as_deref(), to, &format!("{}: {subject_tail}", st.app_name), body, html.as_deref()).await
}

/// The text of a task mail: what happened, what is needed, and where to open it.
fn mail_body(title: &str, label: &str, step: &str, link: &str) -> String {
    let mut b = format!("{title}\nNow: {label}.\n");
    let need = match label {
        "needs you" if !step.is_empty() => format!("What's needed: {step}.\n"),
        "needs you" => "What's needed: an answer or a decision from you in Kompanion.\n".to_string(),
        "failed" if !step.is_empty() => format!("Where it stopped: {step}.\n"),
        _ => String::new(),
    };
    b.push_str(&need);
    b.push_str(&if link.is_empty() { "Open Kompanion to see the details.\n".to_string() } else { format!("Open it: {link}\n") });
    b
}

/// Call after a task's state changed; mails in the background if wanted.
pub fn task_changed(db: SqlitePool, user_id: String, task_id: String, title: String, from: String, to: String) {
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
        let step: String = sqlx::query_as::<_, (Option<String>,)>("SELECT step FROM tasks WHERE id = ?")
            .bind(&task_id)
            .fetch_optional(&db)
            .await
            .ok()
            .flatten()
            .and_then(|(s,)| s)
            .unwrap_or_default();
        let link = PUBLIC_URL.get().map(|u| format!("{u}/#task={task_id}")).unwrap_or_default();
        let status = match label { "needs you" => crate::mailhtml::Status::NeedsYou, "failed" => crate::mailhtml::Status::Failed, _ => crate::mailhtml::Status::Done };
        let intro = match label { "needs you" => "This task is waiting for you.", "failed" => "This task stopped with an error.", _ => "This task is finished." };
        let need_label = if label == "failed" { "Where it stopped" } else { "What's needed" };
        let rows_owned: Vec<(&str, &str)> = if step.is_empty() || label == "done" { vec![] } else { vec![(need_label, step.as_str())] };
        let card = Card { status, title: &title, intro, rows: &rows_owned, button: if link.is_empty() { None } else { Some(("Open the task", link.as_str())) } };
        let body = mail_body(&title, label, &step, &link);
        if let Err(e) = send(&db, &p.email, &format!("{title} — {label}"), &body, Some(card)).await {
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
                let rows: Vec<(String, String)> = counts.iter().map(|(s, n)| (s.replace('_', " "), n.to_string())).collect();
                let row_refs: Vec<(&str, &str)> = rows.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
                let url = PUBLIC_URL.get().cloned().unwrap_or_default();
                let card = Card { status: crate::mailhtml::Status::Info, title: "Your tasks today", intro: "How your tasks stand this morning.", rows: &row_refs,
                    button: if url.is_empty() { None } else { Some(("Open Kompanion", url.as_str())) } };
                let body: String = counts.iter().map(|(s, n)| format!("{}: {n}\n", s.replace('_', " "))).collect();
                if let Err(e) = send(&db, &email, "today", &body, Some(card)).await {
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

/// `kompanion-server test-mail <address>`: a sample "needs you" mail through the configured
/// mail server, to check the branded mail renders.
pub async fn send_test(db: &SqlitePool, to: &str, print: bool) -> anyhow::Result<()> {
    let url = PUBLIC_URL.get().cloned().unwrap_or_default();
    let link = format!("{url}/");
    let rows: [(&str, &str); 2] = [("What's needed", "folder not found on soucouyant"), ("Project", "Kreative Kompanion")];
    let card = Card { status: crate::mailhtml::Status::NeedsYou, title: "Add initials() to names.py (test mail)", intro: "This task is waiting for you.", rows: &rows,
        button: Some(("Open the task", link.as_str())) };
    if print {
        println!("{}", card_html(db, card).await?);
        return Ok(());
    }
    send(db, to, "test mail", "Add initials() to names.py (test mail)\nNow: needs you.\nWhat's needed: folder not found on soucouyant.\n", Some(card)).await
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

    #[test]
    fn task_mails_say_what_is_needed() {
        let b = mail_body("Add initials()", "needs you", "folder not found", "https://k.example/#task=t1");
        assert!(b.contains("What's needed: folder not found.") && b.contains("Open it: https://k.example/#task=t1"));
        assert!(mail_body("T", "needs you", "", "").contains("an answer or a decision"));
        assert!(!mail_body("T", "done", "x", "").contains("needed"));
        assert_eq!(reply_to_with_name("info@kreative-kompas.com"), "Kreative Kompas <info@kreative-kompas.com>");
        assert_eq!(reply_to_with_name("Team <a@b.c>"), "Team <a@b.c>");
        assert_eq!(reply_to_with_name(" "), "");
    }
}
