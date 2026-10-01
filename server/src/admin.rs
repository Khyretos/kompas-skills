//! Admin settings (app name, mail, theme colours) and per-user theme choice.
//! Admins: the first account, unless another admin already exists. Theme
//! colours are only saved when the text they carry stays readable (WCAG AA).

use std::collections::HashMap;

use axum::{
    Extension, Json,
    extract::State,
    http::{StatusCode, header},
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::SqlitePool;

use crate::{
    AppState,
    auth::User,
    contrast::{parse_hex, ratio},
    error::{ApiError, ApiResult},
    mail,
};

/// Everything an admin can change in the UI. Missing values use the defaults.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub app_name: String,
    pub smtp_host: String,
    pub smtp_port: u16,
    /// "starttls", "tls" or "none".
    pub smtp_tls: String,
    pub smtp_user: String,
    pub smtp_from: String,
    /// Where replies go, e.g. info@ while sending as kompanion@.
    pub smtp_reply_to: String,
    /// Primary fills (buttons); white text sits on it.
    pub color_brand: String,
    /// Links and highlighted text on the dark theme.
    pub color_link_dark: String,
    /// Links and highlighted text on the light theme.
    pub color_link_light: String,
    /// Focus rings and active markers.
    pub color_accent: String,
}

const KEYS: &[&str] = &[
    "appName",
    "smtpHost",
    "smtpPort",
    "smtpTls",
    "smtpUser",
    "smtpFrom",
    "smtpReplyTo",
    "colorBrand",
    "colorLinkDark",
    "colorLinkLight",
    "colorAccent",
];

impl Settings {
    fn defaults() -> Self {
        Settings {
            app_name: "Kreative Kompanion".into(),
            smtp_port: 587,
            smtp_tls: "starttls".into(),
            color_brand: "#5c398e".into(),
            color_link_dark: "#cca9ff".into(),
            color_link_light: "#7b2fb5".into(),
            color_accent: "#bf4eff".into(),
            ..Default::default()
        }
    }
}

pub async fn load(db: &SqlitePool) -> sqlx::Result<Settings> {
    let rows: Vec<(String, String)> = sqlx::query_as("SELECT key, value FROM settings")
        .fetch_all(db)
        .await?;
    let mut map: serde_json::Map<String, Value> = serde_json::to_value(Settings::defaults())
        .ok()
        .and_then(|v| v.as_object().cloned())
        .unwrap_or_default();
    for (k, v) in rows {
        let value = serde_json::from_str(&v).unwrap_or(Value::String(v));
        map.insert(k, value);
    }
    Ok(serde_json::from_value(Value::Object(map)).unwrap_or_else(|_| Settings::defaults()))
}

/// Makes the oldest account admin when no admin exists (first start, setup).
pub async fn ensure_admin(db: &SqlitePool) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE users SET is_admin = 1
         WHERE id = (SELECT id FROM users ORDER BY created_at LIMIT 1)
           AND NOT EXISTS (SELECT 1 FROM users WHERE is_admin = 1)",
    )
    .execute(db)
    .await?;
    Ok(())
}

pub async fn is_admin(db: &SqlitePool, user_id: &str) -> sqlx::Result<bool> {
    let row: Option<(bool,)> = sqlx::query_as("SELECT is_admin FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_optional(db)
        .await?;
    Ok(row.is_some_and(|r| r.0))
}

async fn require_admin(s: &AppState, u: &User) -> ApiResult<()> {
    if is_admin(&s.db, &u.id).await? {
        Ok(())
    } else {
        Err(ApiError::Forbidden("Only admins can do that.".into()))
    }
}

/// Readability checks for theme colours; returns what is wrong.
pub fn check_colors(s: &Settings) -> Vec<String> {
    let mut problems = Vec::new();
    let mut need = |name: &str, fg: &str, bg: &str, min: f64, what: &str| {
        let (Some(f), Some(b)) = (parse_hex(fg), parse_hex(bg)) else {
            problems.push(format!("{name}: \"{fg}\" is not a colour like #5c398e."));
            return;
        };
        let r = ratio(f, b);
        if r < min {
            problems.push(format!("{name}: {what} is {r:.1}:1, needs at least {min}:1."));
        }
    };
    // Backgrounds the app uses (styles.css): night, plum, white, mist.
    need("Brand", "#ffffff", &s.color_brand, 4.5, "white text on it");
    need("Link (dark)", &s.color_link_dark, "#0c0917", 4.5, "on the dark background");
    need("Link (dark)", &s.color_link_dark, "#2c1f3f", 4.5, "on dark panels");
    need("Link (light)", &s.color_link_light, "#ffffff", 4.5, "on the light background");
    need("Link (light)", &s.color_link_light, "#ebe2f8", 4.5, "on light panels");
    need("Accent", &s.color_accent, "#0c0917", 3.0, "as a focus ring on dark");
    problems.dedup();
    problems
}

pub async fn get_settings(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
) -> ApiResult<Json<Value>> {
    require_admin(&s, &u).await?;
    let settings = load(&s.db).await?;
    Ok(Json(json!({
        "settings": settings,
        "smtpPasswordSet": std::env::var("SMTP_PASSWORD").is_ok_and(|p| !p.is_empty()),
    })))
}

pub async fn put_settings(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Json(mut new): Json<Settings>,
) -> ApiResult<Json<Settings>> {
    require_admin(&s, &u).await?;
    new.app_name = new.app_name.trim().chars().take(60).collect();
    if new.app_name.is_empty() {
        return Err(ApiError::BadRequest("Give the app a name.".into()));
    }
    if !["starttls", "tls", "none"].contains(&new.smtp_tls.as_str()) {
        return Err(ApiError::BadRequest("Mail security must be starttls, tls or none.".into()));
    }
    for c in [
        &mut new.color_brand,
        &mut new.color_link_dark,
        &mut new.color_link_light,
        &mut new.color_accent,
    ] {
        *c = c.trim().to_lowercase();
    }
    let problems = check_colors(&new);
    if !problems.is_empty() {
        return Err(ApiError::BadRequest(format!(
            "These colours would be hard to read: {}",
            problems.join(" ")
        )));
    }
    let map: HashMap<String, Value> = serde_json::from_value(serde_json::to_value(&new).unwrap_or_default())
        .unwrap_or_default();
    let mut tx = s.db.begin().await?;
    for key in KEYS {
        if let Some(v) = map.get(*key) {
            sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
                .bind(key)
                .bind(v.to_string())
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    Ok(Json(new))
}

#[derive(Deserialize)]
pub struct TestMail {
    to: String,
}

pub async fn test_mail(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Json(b): Json<TestMail>,
) -> ApiResult<StatusCode> {
    require_admin(&s, &u).await?;
    let st = load(&s.db).await?;
    if st.smtp_host.is_empty() || st.smtp_from.is_empty() {
        return Err(ApiError::BadRequest("Fill in the mail server and sender first.".into()));
    }
    let password = std::env::var("SMTP_PASSWORD").ok().filter(|p| !p.is_empty());
    let smtp = mail::SmtpSettings {
        host: st.smtp_host,
        port: st.smtp_port,
        tls: st.smtp_tls,
        user: st.smtp_user,
        from: st.smtp_from,
        reply_to: st.smtp_reply_to,
    };
    let body = format!(
        "This is a test from {}. If you can read it, mail notifications work.\n",
        st.app_name
    );
    mail::send(&smtp, password.as_deref(), b.to.trim(), &format!("{}: test mail", st.app_name), &body)
        .await
        .map_err(|e| {
            tracing::warn!("test mail failed: {e:#}");
            ApiError::BadRequest(format!("Sending failed: {e:#}"))
        })?;
    Ok(StatusCode::NO_CONTENT)
}

/// Theme colours as CSS custom properties; loaded by index.html, no sign-in needed.
pub async fn theme_css(State(s): State<AppState>) -> ApiResult<impl IntoResponse> {
    let st = load(&s.db).await?;
    // Only colours that parse are written, so nothing else can reach the CSS.
    let c = |v: &str, fallback: &str| if parse_hex(v).is_some() { v.to_string() } else { fallback.to_string() };
    let css = format!(
        ":root{{--brand:{b};--link:{ld};--accent:{a}}}\n\
         @media (prefers-color-scheme: light){{:root:not([data-theme=\"dark\"]){{--link:{ll};--accent:{ll}}}}}\n\
         :root[data-theme=\"light\"]{{--link:{ll};--accent:{ll}}}\n",
        b = c(&st.color_brand, "#5c398e"),
        ld = c(&st.color_link_dark, "#cca9ff"),
        ll = c(&st.color_link_light, "#7b2fb5"),
        a = c(&st.color_accent, "#bf4eff"),
    );
    Ok(([(header::CONTENT_TYPE, "text/css; charset=utf-8"), (header::CACHE_CONTROL, "no-cache")], css))
}

#[derive(Deserialize)]
pub struct ThemeChoice {
    theme: String,
}

/// The signed-in user's light/dark/system choice.
pub async fn set_my_theme(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Json(b): Json<ThemeChoice>,
) -> ApiResult<StatusCode> {
    if !["system", "light", "dark"].contains(&b.theme.as_str()) {
        return Err(ApiError::BadRequest("Theme must be system, light or dark.".into()));
    }
    sqlx::query("UPDATE users SET theme = ? WHERE id = ?")
        .bind(&b.theme)
        .bind(&u.id)
        .execute(&s.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Refresh steps the Machines tab offers, in seconds; 1 is "Live".
pub const REFRESH_STEPS: &[u32] = &[1, 2, 5, 15, 30, 60, 300];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prefs {
    machines_refresh: u32,
}

/// Per-user preferences that follow the user across devices.
pub async fn set_prefs(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Json(b): Json<Prefs>,
) -> ApiResult<StatusCode> {
    if !REFRESH_STEPS.contains(&b.machines_refresh) {
        return Err(ApiError::BadRequest("Pick one of the refresh steps.".into()));
    }
    sqlx::query("UPDATE users SET machines_refresh = ? WHERE id = ?")
        .bind(b.machines_refresh)
        .bind(&u.id)
        .execute(&s.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_palette_is_readable() {
        assert!(check_colors(&Settings::defaults()).is_empty());
    }

    #[test]
    fn refuses_unreadable_and_invalid_colours() {
        let mut s = Settings::defaults();
        s.color_brand = "#f3941f".into(); // white on orange
        s.color_link_light = "#cca9ff".into(); // lilac on white
        s.color_accent = "red".into();
        let p = check_colors(&s);
        assert!(p.iter().any(|m| m.starts_with("Brand")));
        assert!(p.iter().any(|m| m.starts_with("Link (light)")));
        assert!(p.iter().any(|m| m.contains("not a colour")));
    }
}
