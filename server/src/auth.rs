//! Sign-in with a session cookie (HttpOnly, SameSite=Strict, Secure), the
//! first-run setup code, and the guard every API request passes through.
//! An unauthenticated agent server is remote code execution for anyone who
//! can reach it (OpenCode CVE-2026-22812), so nothing is open by default.

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use axum::{
    Json,
    extract::{Request, State},
    http::{HeaderMap, Method, header},
    middleware::Next,
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use serde_json::json;

use crate::{
    AppState,
    error::{ApiError, ApiResult},
    util,
};

pub const COOKIE: &str = "kk_session";
const SESSION_DAYS: i64 = 30;

#[derive(Clone, Debug)]
pub struct User {
    pub id: String,
    pub name: String,
}

/// Failed sign-ins per user name, to slow down password guessing.
#[derive(Default)]
pub struct Throttle(Mutex<HashMap<String, (u32, Instant)>>);

impl Throttle {
    pub fn check(&self, key: &str) -> ApiResult<()> {
        let map = self.0.lock().unwrap();
        if let Some((n, since)) = map.get(key)
            && *n >= 10
            && since.elapsed() < Duration::from_secs(600)
        {
            return Err(ApiError::TooMany);
        }
        Ok(())
    }
    pub fn fail(&self, key: &str) {
        let mut map = self.0.lock().unwrap();
        let e = map.entry(key.to_string()).or_insert((0, Instant::now()));
        if e.1.elapsed() > Duration::from_secs(600) {
            *e = (0, Instant::now());
        }
        e.0 += 1;
    }
    fn clear(&self, key: &str) {
        self.0.lock().unwrap().remove(key);
    }
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let mut bytes = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut bytes);
    let salt = SaltString::encode_b64(&bytes).map_err(|e| anyhow::anyhow!("salt: {e}"))?;
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| anyhow::anyhow!("hashing failed: {e}"))?
        .to_string())
}

fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|h| {
            Argon2::default()
                .verify_password(password.as_bytes(), &h)
                .is_ok()
        })
        .unwrap_or(false)
}

pub fn session_cookie(state: &AppState, token: &str, max_age: i64) -> String {
    let secure = if state.config.secure_cookies {
        "; Secure"
    } else {
        ""
    };
    format!("{COOKIE}={token}; Path=/; HttpOnly; SameSite=Strict; Max-Age={max_age}{secure}")
}

fn cookie_token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|c| {
            c.trim()
                .strip_prefix(&format!("{COOKIE}="))
                .map(str::to_string)
        })
}

pub async fn current_user(state: &AppState, headers: &HeaderMap) -> ApiResult<Option<User>> {
    let Some(token) = cookie_token(headers) else {
        return Ok(None);
    };
    let row: Option<(String, String)> = sqlx::query_as(
        "SELECT u.id, u.name FROM sessions s JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = ? AND s.expires_at > ?",
    )
    .bind(util::sha256_hex(&token))
    .bind(util::now())
    .fetch_optional(&state.db)
    .await?;
    Ok(row.map(|(id, name)| User { id, name }))
}

async fn users_exist(state: &AppState) -> ApiResult<bool> {
    let (n,): (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(&state.db)
        .await?;
    Ok(n > 0)
}

/// Prints a one-time setup code when no account exists yet. Only someone who
/// can read the server log can create the first account.
pub async fn ensure_setup_code(state: &AppState) -> anyhow::Result<()> {
    if !users_exist(state).await? {
        let code = util::random_token()[..12].to_string();
        tracing::warn!(
            "No account yet. Open the app and use this setup code to create one: {code}"
        );
        *state.setup_code.lock().unwrap() = Some(code);
    }
    Ok(())
}

pub async fn status(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<serde_json::Value>> {
    let user = current_user(&state, &headers).await?;
    let settings = crate::admin::load(&state.db).await?;
    let (admin, theme, refresh, pins) = match &user {
        Some(u) => sqlx::query_as::<_, (bool, String, i64, String)>(
            "SELECT is_admin, theme, machines_refresh, gpu_pins FROM users WHERE id = ?",
        )
        .bind(&u.id)
        .fetch_optional(&state.db)
        .await?
        .unwrap_or((false, "system".into(), 5, "[]".into())),
        None => (false, "system".into(), 5, "[]".into()),
    };
    Ok(Json(json!({
        "name": settings.app_name,
        "admin": admin,
        "theme": theme,
        "machinesRefresh": refresh,
        "gpuPins": serde_json::from_str::<serde_json::Value>(&pins).unwrap_or(serde_json::json!([])),
        "version": env!("CARGO_PKG_VERSION"),
        "setupNeeded": !users_exist(&state).await?,
        "user": user.map(|u| u.name),
        "logoVersion": crate::admin::logo_version(&state.db).await?,
        "windshift": if state.windshift { "connected" } else { "not configured" },
        "windshiftWarning": crate::windshift::LAST_ERROR.lock().unwrap().as_ref()
            .map(|(at, why)| format!("The last sync at {} failed: {why}", &at[..16.min(at.len())])),
        "signIn": {
            "password": state.config.password_login(),
            "oidc": state.config.oidc.as_ref().map(|o| o.label.clone()),
        },
    })))
}

#[derive(Deserialize)]
pub struct SetupBody {
    code: String,
    name: String,
    password: String,
}

pub async fn setup(
    State(state): State<AppState>,
    Json(body): Json<SetupBody>,
) -> ApiResult<Response> {
    if users_exist(&state).await? {
        return Err(ApiError::Forbidden(
            "Setup is already done. Sign in instead.".into(),
        ));
    }
    state.throttle.check("setup")?;
    let expected = state.setup_code.lock().unwrap().clone().unwrap_or_default();
    if expected.is_empty() || !util::ct_eq(body.code.trim(), &expected) {
        state.throttle.fail("setup");
        return Err(ApiError::BadRequest(
            "That setup code is wrong. It is printed in the server log.".into(),
        ));
    }
    validate_credentials(&body.name, &body.password)?;
    let id = util::new_id();
    sqlx::query("INSERT INTO users (id, name, password_hash, created_at) VALUES (?, ?, ?, ?)")
        .bind(&id)
        .bind(body.name.trim())
        .bind(hash_password(&body.password)?)
        .bind(util::now())
        .execute(&state.db)
        .await?;
    *state.setup_code.lock().unwrap() = None;
    tracing::info!(user = %body.name.trim(), "first account created");
    crate::admin::ensure_admin(&state.db).await?;
    start_session(&state, &id).await
}

fn validate_credentials(name: &str, password: &str) -> ApiResult<()> {
    let name = name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(ApiError::BadRequest(
            "Pick a name of 1 to 64 characters.".into(),
        ));
    }
    if password.chars().count() < 12 {
        return Err(ApiError::BadRequest(
            "Use a password of at least 12 characters.".into(),
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct LoginBody {
    name: String,
    password: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginBody>,
) -> ApiResult<Response> {
    if !state.config.password_login() {
        return Err(ApiError::Forbidden(
            "Password sign-in is off. Use single sign-on.".into(),
        ));
    }
    let key = body.name.trim().to_lowercase();
    state.throttle.check(&key)?;
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT id, password_hash FROM users WHERE name = ?")
            .bind(body.name.trim())
            .fetch_optional(&state.db)
            .await?;
    // Verify against a dummy hash when the user doesn't exist, so timing
    // doesn't reveal which names are valid.
    let (id, hash) = row.unwrap_or_else(|| (String::new(), state.dummy_hash.clone()));
    if id.is_empty() || !verify_password(&body.password, &hash) {
        state.throttle.fail(&key);
        return Err(ApiError::BadRequest(
            "That name and password don't match.".into(),
        ));
    }
    state.throttle.clear(&key);
    start_session(&state, &id).await
}

async fn start_session(state: &AppState, user_id: &str) -> ApiResult<Response> {
    let cookie = create_session(state, user_id).await?;
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({ "ok": true }))).into_response())
}

/// Stores a new session and returns the Set-Cookie value for it.
pub async fn create_session(state: &AppState, user_id: &str) -> ApiResult<String> {
    create_session_with(state, user_id, None).await
}

/// A session that remembers the provider's ID token (for signing out there too).
pub async fn create_session_with(state: &AppState, user_id: &str, id_token: Option<&str>) -> ApiResult<String> {
    let token = util::random_token();
    sqlx::query("INSERT INTO sessions (token_hash, user_id, expires_at, id_token) VALUES (?, ?, ?, ?)")
        .bind(util::sha256_hex(&token))
        .bind(user_id)
        .bind(util::in_days(SESSION_DAYS))
        .bind(id_token)
        .execute(&state.db)
        .await?;
    Ok(session_cookie(state, &token, SESSION_DAYS * 86_400))
}

pub async fn logout(State(state): State<AppState>, headers: HeaderMap) -> ApiResult<Response> {
    let mut redirect = None;
    if let Some(token) = cookie_token(&headers) {
        let hash = util::sha256_hex(&token);
        let row: Option<(Option<String>,)> = sqlx::query_as("SELECT id_token FROM sessions WHERE token_hash = ?")
            .bind(&hash)
            .fetch_optional(&state.db)
            .await?;
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?").bind(&hash).execute(&state.db).await?;
        // Signed in with single sign-on: also end the provider's session.
        if let Some(id_token) = row.and_then(|r| r.0) {
            redirect = crate::oidc::end_session_url(&state, &id_token, &headers).await;
        }
    }
    let cookie = session_cookie(&state, "", 0);
    Ok(([(header::SET_COOKIE, cookie)], Json(json!({ "ok": true, "redirect": redirect }))).into_response())
}

/// Guard for all API routes:
/// - state-changing requests must carry `X-Kompanion: 1` (a custom header no
///   cross-site form or simple request can send) and, when the browser sends
///   an Origin, it must be one we serve from;
/// - everything except status, setup, login and the OIDC redirects needs a
///   signed-in user.
pub async fn guard(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    // Inside the nested /api router the prefix is already stripped.
    let path = req.uri().path();
    let path = path.strip_prefix("/api").unwrap_or(path).to_string();
    // Runner reports carry their own bearer token and no cookie.
    if req.method() == Method::POST && path.starts_with("/machines/") && (path.ends_with("/stats") || path.ends_with("/results")) {
        return next.run(req).await;
    }
    let open = matches!(
        path.as_str(),
        "/status" | "/theme.css" | "/logo" | "/setup" | "/login" | "/auth/oidc/start" | "/auth/oidc/callback"
    );

    if !matches!(*req.method(), Method::GET | Method::HEAD | Method::OPTIONS) {
        if req
            .headers()
            .get("x-kompanion")
            .and_then(|v| v.to_str().ok())
            != Some("1")
        {
            return ApiError::Forbidden("Missing X-Kompanion header.".into()).into_response();
        }
        if let Some(origin) = req
            .headers()
            .get(header::ORIGIN)
            .and_then(|v| v.to_str().ok())
            && !origin_allowed(&state, req.headers(), origin)
        {
            return ApiError::Forbidden("Requests from this site are not allowed.".into())
                .into_response();
        }
    }

    if !open {
        match current_user(&state, req.headers()).await {
            Ok(Some(user)) => {
                req.extensions_mut().insert(user);
            }
            Ok(None) => return ApiError::Unauthorized.into_response(),
            Err(e) => return e.into_response(),
        }
    }
    next.run(req).await
}

fn origin_allowed(state: &AppState, headers: &HeaderMap, origin: &str) -> bool {
    if state.config.allowed_origins.iter().any(|o| o == origin) {
        return true;
    }
    // Same origin as the Host header (direct access without a proxy).
    let host = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    origin == format!("http://{host}") || origin == format!("https://{host}")
}
