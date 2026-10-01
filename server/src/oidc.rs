//! Single sign-on with OpenID Connect (Keycloak and others).
//!
//! The server is the OIDC client (backend-for-frontend): authorization code
//! flow with PKCE, state and nonce, a confidential client secret, and the ID
//! token checked against the provider's keys. The browser only ever gets our
//! own session cookie, never a provider token.
//!
//! Accounts are linked, not duplicated: a sign-in matches an account by its
//! OIDC subject, then by verified email, then (if allowed) by username. A new
//! account is only created for people listed in `oidc.allow_new` (or for
//! everyone when it contains `"*"`).

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

use anyhow::{Context, anyhow};
use axum::{
    extract::{Query, State},
    http::{HeaderMap, header},
    response::{AppendHeaders, IntoResponse, Redirect, Response},
};
use openidconnect::{
    AuthorizationCode, ClientId, ClientSecret, CsrfToken, IssuerUrl, Nonce, PkceCodeChallenge,
    PkceCodeVerifier, RedirectUrl, Scope, TokenResponse,
    core::{CoreAuthenticationFlow, CoreClient, CoreProviderMetadata},
    reqwest,
};
use serde::Deserialize;
use tokio::sync::OnceCell;

use crate::{
    AppState, auth,
    config::OidcConfig,
    error::{ApiError, ApiResult},
    util,
};

const FLOW_COOKIE: &str = "kk_oidc";
const FLOW_TTL: Duration = Duration::from_secs(600);

struct Pending {
    verifier: PkceCodeVerifier,
    nonce: Nonce,
    started: Instant,
}

/// Sign-ins in progress, keyed by their `state` value, and the provider's
/// discovery document (fetched once).
#[derive(Default)]
pub struct Oidc {
    pending: Mutex<HashMap<String, Pending>>,
    metadata: OnceCell<CoreProviderMetadata>,
}

fn http() -> reqwest::Client {
    // No redirects: the provider must answer directly (prevents SSRF via redirect).
    reqwest::ClientBuilder::new()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(15))
        .build()
        .expect("oidc http client")
}

fn config(state: &AppState) -> ApiResult<&OidcConfig> {
    state.config.oidc.as_ref().ok_or_else(|| ApiError::NotFound)
}

async fn metadata<'a>(
    state: &'a AppState,
    cfg: &OidcConfig,
) -> anyhow::Result<&'a CoreProviderMetadata> {
    state
        .oidc
        .metadata
        .get_or_try_init(|| async {
            CoreProviderMetadata::discover_async(IssuerUrl::new(cfg.issuer.clone())?, &http())
                .await
                .context("reading the provider's discovery document")
        })
        .await
}

fn flow_cookie(state: &AppState, value: &str, max_age: u64) -> String {
    let secure = if state.config.secure_cookies {
        "; Secure"
    } else {
        ""
    };
    // Lax, not Strict: it must come back on the provider's redirect to us.
    format!(
        "{FLOW_COOKIE}={value}; Path=/api/auth/oidc; HttpOnly; SameSite=Lax; Max-Age={max_age}{secure}"
    )
}

/// GET /api/auth/oidc/start: send the browser to the provider.
pub async fn start(State(state): State<AppState>) -> ApiResult<Response> {
    let cfg = config(&state)?;
    let secret = cfg
        .client_secret()
        .ok_or_else(|| anyhow!("client secret missing"))?;
    let client = CoreClient::from_provider_metadata(
        metadata(&state, cfg).await?.clone(),
        ClientId::new(cfg.client_id.clone()),
        Some(ClientSecret::new(secret)),
    )
    .set_redirect_uri(RedirectUrl::new(cfg.redirect_url.clone()).map_err(anyhow::Error::from)?);

    let (challenge, verifier) = PkceCodeChallenge::new_random_sha256();
    let (url, csrf, nonce) = client
        .authorize_url(
            CoreAuthenticationFlow::AuthorizationCode,
            CsrfToken::new_random,
            Nonce::new_random,
        )
        .add_scope(Scope::new("email".into()))
        .add_scope(Scope::new("profile".into()))
        .set_pkce_challenge(challenge)
        .url();

    {
        let mut pending = state.oidc.pending.lock().unwrap();
        pending.retain(|_, p| p.started.elapsed() < FLOW_TTL);
        if pending.len() > 1000 {
            return Err(ApiError::TooMany);
        }
        pending.insert(
            csrf.secret().clone(),
            Pending {
                verifier,
                nonce,
                started: Instant::now(),
            },
        );
    }
    let cookie = flow_cookie(&state, csrf.secret(), FLOW_TTL.as_secs());
    Ok(([(header::SET_COOKIE, cookie)], Redirect::to(url.as_str())).into_response())
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

/// GET /api/auth/oidc/callback: the provider sends the browser back here.
/// Problems go back to the sign-in screen as `/?signin=<reason>`; the details
/// stay in the server log.
pub async fn callback(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<CallbackQuery>,
) -> Response {
    let clear = flow_cookie(&state, "", 0);
    let to = match finish(&state, &headers, q).await {
        Ok(session_cookie) => {
            return (
                AppendHeaders([
                    (header::SET_COOKIE, clear),
                    (header::SET_COOKIE, session_cookie),
                ]),
                Redirect::to("/"),
            )
                .into_response();
        }
        Err(Fail::NoAccount) => "/?signin=no-account",
        Err(Fail::Other(e)) => {
            tracing::warn!(error = %format!("{e:#}"), "single sign-on failed");
            "/?signin=failed"
        }
    };
    ([(header::SET_COOKIE, clear)], Redirect::to(to)).into_response()
}

enum Fail {
    NoAccount,
    Other(anyhow::Error),
}

impl<E: Into<anyhow::Error>> From<E> for Fail {
    fn from(e: E) -> Self {
        Fail::Other(e.into())
    }
}

fn flow_cookie_value(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|c| {
            c.trim()
                .strip_prefix(&format!("{FLOW_COOKIE}="))
                .map(str::to_string)
        })
}

async fn finish(state: &AppState, headers: &HeaderMap, q: CallbackQuery) -> Result<String, Fail> {
    let cfg = state
        .config
        .oidc
        .as_ref()
        .ok_or_else(|| anyhow!("oidc not configured"))?;
    if let Some(err) = q.error {
        return Err(anyhow!("provider returned error {err:.100}").into());
    }
    let (code, csrf) = q
        .code
        .zip(q.state)
        .ok_or_else(|| anyhow!("code or state missing"))?;

    // The state must be one we issued, to this same browser.
    let cookie = flow_cookie_value(headers).unwrap_or_default();
    if !util::ct_eq(&cookie, &csrf) {
        return Err(anyhow!("state does not match this browser").into());
    }
    let pending = state
        .oidc
        .pending
        .lock()
        .unwrap()
        .remove(&csrf)
        .filter(|p| p.started.elapsed() < FLOW_TTL)
        .ok_or_else(|| anyhow!("unknown or expired sign-in"))?;

    let secret = cfg
        .client_secret()
        .ok_or_else(|| anyhow!("client secret missing"))?;
    let client = CoreClient::from_provider_metadata(
        metadata(state, cfg).await?.clone(),
        ClientId::new(cfg.client_id.clone()),
        Some(ClientSecret::new(secret)),
    )
    .set_redirect_uri(RedirectUrl::new(cfg.redirect_url.clone())?);

    let http = http();
    let tokens = client
        .exchange_code(AuthorizationCode::new(code))?
        .set_pkce_verifier(pending.verifier)
        .request_async(&http)
        .await
        .map_err(|e| anyhow!("token exchange failed: {e}"))?;
    let id_token = tokens.id_token().ok_or_else(|| anyhow!("no ID token"))?;
    let claims = id_token.claims(&client.id_token_verifier(), &pending.nonce)?;

    let who = Identity {
        issuer: claims.issuer().as_str().to_string(),
        subject: claims.subject().as_str().to_string(),
        email: claims
            .email()
            .filter(|_| claims.email_verified() == Some(true))
            .map(|e| e.as_str().to_lowercase()),
        username: claims.preferred_username().map(|u| u.as_str().to_string()),
    };
    let user_id = link_account(state, cfg, &who)
        .await?
        .ok_or(Fail::NoAccount)?;
    tracing::info!(user = %user_id, "signed in with single sign-on");
    Ok(auth::create_session(state, &user_id)
        .await
        .map_err(|e| anyhow!("{e}"))?)
}

pub struct Identity {
    pub issuer: String,
    pub subject: String,
    /// Only set when the provider says the email is verified.
    pub email: Option<String>,
    pub username: Option<String>,
}

/// Finds the account for this identity, linking or creating one when allowed.
pub async fn link_account(
    state: &AppState,
    cfg: &OidcConfig,
    who: &Identity,
) -> anyhow::Result<Option<String>> {
    let db = &state.db;
    let linked: Option<(String,)> =
        sqlx::query_as("SELECT id FROM users WHERE oidc_issuer = ? AND oidc_subject = ?")
            .bind(&who.issuer)
            .bind(&who.subject)
            .fetch_optional(db)
            .await?;
    if let Some((id,)) = linked {
        return Ok(Some(id));
    }

    // Not linked yet: match an existing, unlinked account.
    let mut found: Option<(String,)> = None;
    if let Some(email) = &who.email {
        found =
            sqlx::query_as("SELECT id FROM users WHERE lower(email) = ? AND oidc_subject IS NULL")
                .bind(email)
                .fetch_optional(db)
                .await?;
    }
    if found.is_none()
        && cfg.link_by_username
        && let Some(name) = &who.username
    {
        found = sqlx::query_as("SELECT id FROM users WHERE name = ? AND oidc_subject IS NULL")
            .bind(name)
            .fetch_optional(db)
            .await?;
    }
    if let Some((id,)) = found {
        sqlx::query(
            "UPDATE users SET oidc_issuer = ?, oidc_subject = ?, email = coalesce(email, ?) WHERE id = ?",
        )
        .bind(&who.issuer)
        .bind(&who.subject)
        .bind(&who.email)
        .bind(&id)
        .execute(db)
        .await?;
        tracing::info!(user = %id, "linked existing account to single sign-on");
        return Ok(Some(id));
    }

    // A new account only for people the config allows.
    let allowed = cfg.allow_new.iter().any(|a| {
        let a = a.to_lowercase();
        a == "*"
            || who.email.as_deref() == Some(a.as_str())
            || who.username.as_deref().map(str::to_lowercase).as_deref() == Some(a.as_str())
    });
    if !allowed {
        return Ok(None);
    }
    let name = who
        .username
        .clone()
        .or_else(|| who.email.clone())
        .ok_or_else(|| anyhow!("provider sent neither username nor email"))?;
    let id = util::new_id();
    sqlx::query(
        "INSERT INTO users (id, name, password_hash, created_at, email, oidc_issuer, oidc_subject)
         VALUES (?, ?, '', ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&name)
    .bind(util::now())
    .bind(&who.email)
    .bind(&who.issuer)
    .bind(&who.subject)
    .execute(db)
    .await
    .context("creating account (is the name taken?)")?;
    *state.setup_code.lock().unwrap() = None;
    tracing::info!(user = %id, "created account from single sign-on");
    Ok(Some(id))
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn state(cfg: OidcConfig) -> AppState {
        let mut config: crate::config::Config = toml::from_str("").unwrap();
        config.oidc = Some(cfg);
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&db).await.unwrap();
        AppState::for_tests(config, db)
    }

    fn cfg(allow_new: &[&str], by_name: bool) -> OidcConfig {
        OidcConfig {
            issuer: "https://idp".into(),
            client_id: "k".into(),
            client_secret_env: "X".into(),
            redirect_url: "https://k/api/auth/oidc/callback".into(),
            label: "SSO".into(),
            password_login: true,
            link_by_username: by_name,
            allow_new: allow_new.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn who(sub: &str, email: Option<&str>, name: Option<&str>) -> Identity {
        Identity {
            issuer: "https://idp".into(),
            subject: sub.into(),
            email: email.map(Into::into),
            username: name.map(Into::into),
        }
    }

    async fn add_user(s: &AppState, name: &str) -> String {
        let id = util::new_id();
        sqlx::query(
            "INSERT INTO users (id, name, password_hash, created_at) VALUES (?, ?, 'x', ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(util::now())
        .execute(&s.db)
        .await
        .unwrap();
        id
    }

    async fn count(s: &AppState) -> i64 {
        sqlx::query_as::<_, (i64,)>("SELECT COUNT(*) FROM users")
            .fetch_one(&s.db)
            .await
            .unwrap()
            .0
    }

    #[tokio::test]
    async fn links_existing_user_by_name_without_duplicate() {
        let c = cfg(&[], true);
        let s = state(c.clone()).await;
        let kees = add_user(&s, "khyretos").await;
        let w = who("sub-1", Some("k@example.com"), Some("khyretos"));
        assert_eq!(link_account(&s, &c, &w).await.unwrap(), Some(kees.clone()));
        // Second sign-in goes by subject, even if the username changed.
        let w2 = who("sub-1", None, Some("renamed"));
        assert_eq!(link_account(&s, &c, &w2).await.unwrap(), Some(kees));
        assert_eq!(count(&s).await, 1);
    }

    #[tokio::test]
    async fn refuses_strangers_and_name_link_when_off() {
        let c = cfg(&[], false);
        let s = state(c.clone()).await;
        add_user(&s, "khyretos").await;
        let w = who("sub-2", None, Some("khyretos"));
        assert_eq!(link_account(&s, &c, &w).await.unwrap(), None);
        assert_eq!(count(&s).await, 1);
    }

    #[tokio::test]
    async fn creates_only_allowed_new_users() {
        let c = cfg(&["friend@example.com"], true);
        let s = state(c.clone()).await;
        let ok = who("sub-3", Some("friend@example.com"), Some("friend"));
        assert!(link_account(&s, &c, &ok).await.unwrap().is_some());
        let no = who("sub-4", Some("other@example.com"), Some("other"));
        assert_eq!(link_account(&s, &c, &no).await.unwrap(), None);
        assert_eq!(count(&s).await, 1);
    }

    #[tokio::test]
    async fn wildcard_creates_everyone_once() {
        let c = cfg(&["*"], true);
        let s = state(c.clone()).await;
        let a = who("sub-5", Some("a@example.com"), Some("alice"));
        let first = link_account(&s, &c, &a).await.unwrap().unwrap();
        assert_eq!(link_account(&s, &c, &a).await.unwrap(), Some(first));
        assert!(
            link_account(&s, &c, &who("sub-6", None, Some("bob")))
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(count(&s).await, 2);
    }

    #[tokio::test]
    async fn linked_account_is_not_taken_over_by_another_subject() {
        let c = cfg(&[], true);
        let s = state(c.clone()).await;
        add_user(&s, "khyretos").await;
        link_account(&s, &c, &who("sub-1", None, Some("khyretos")))
            .await
            .unwrap()
            .unwrap();
        let imposter = who("sub-9", None, Some("khyretos"));
        assert_eq!(link_account(&s, &c, &imposter).await.unwrap(), None);
    }
}
