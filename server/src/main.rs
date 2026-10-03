mod admin;
mod api;
mod auth;
mod config;
mod contrast;
mod error;
mod events;
mod hoststats;
mod import;
mod llm;
mod mail;
mod notify;
mod oidc;
mod tasks;
mod util;
mod windshift;

use std::{
    str::FromStr,
    sync::{Arc, Mutex},
};

use anyhow::Context;
use axum::{
    Router,
    http::{HeaderValue, header},
    middleware,
    routing::{get, patch, post},
};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqlitePoolOptions};
use tower_http::{
    services::{ServeDir, ServeFile},
    set_header::SetResponseHeaderLayer,
    trace::TraceLayer,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<config::Config>,
    pub db: SqlitePool,
    pub http: reqwest::Client,
    pub bus: events::Bus,
    pub throttle: Arc<auth::Throttle>,
    pub setup_code: Arc<Mutex<Option<String>>>,
    pub dummy_hash: String,
    pub oidc: Arc<oidc::Oidc>,
    pub host: Arc<hoststats::HostStats>,
    /// Windshift sync configured from the environment (WINDSHIFT_URL, WINDSHIFT_TOKEN[_FILE]).
    pub windshift: bool,
}

#[cfg(test)]
impl AppState {
    pub fn for_tests(config: config::Config, db: SqlitePool) -> Self {
        AppState {
            config: Arc::new(config),
            db,
            http: llm::http_client(),
            bus: events::Bus::new(),
            throttle: Default::default(),
            setup_code: Default::default(),
            dummy_hash: String::new(),
            oidc: Default::default(),
            host: Default::default(),
            windshift: false,
        }
    }
}

const CSP: &str = "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data: blob:; \
connect-src 'self'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'; \
require-trusted-types-for 'script'; trusted-types app dompurify";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    let config = config::Config::load()?;
    let db = SqlitePoolOptions::new()
        .max_connections(8)
        .connect_with(
            SqliteConnectOptions::from_str(&format!("sqlite://{}", config.database.display()))?
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal)
                .foreign_keys(true),
        )
        .await
        .with_context(|| format!("opening {}", config.database.display()))?;
    sqlx::migrate!().run(&db).await?;

    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("import") {
        let path = args
            .get(2)
            .context("usage: kompanion-server import <file.json> [user name]")?;
        return import::run(&db, path, args.get(3).map(String::as_str)).await;
    }

    let windshift = windshift::Windshift::from_env(llm::http_client()).map(Arc::new);
    let state = AppState {
        config: Arc::new(config.clone()),
        db,
        http: llm::http_client(),
        bus: events::Bus::new(),
        throttle: Default::default(),
        setup_code: Default::default(),
        dummy_hash: auth::hash_password(&util::random_token())?,
        oidc: Default::default(),
        host: Default::default(),
        windshift: windshift.is_some(),
    };
    hoststats::HostStats::spawn_live(state.clone());
    notify::spawn_daily(state.db.clone());
    if let Some(ws) = windshift {
        tracing::info!("Windshift sync on");
        windshift::spawn(state.db.clone(), ws);
    }
    admin::ensure_admin(&state.db).await?;
    auth::ensure_setup_code(&state).await?;

    let api = Router::new()
        .route("/status", get(auth::status))
        .route("/theme.css", get(admin::theme_css))
        .route("/me/theme", axum::routing::put(admin::set_my_theme))
        .route("/admin/settings", get(admin::get_settings).put(admin::put_settings))
        .route("/admin/test-mail", post(admin::test_mail))
        .route("/setup", post(auth::setup))
        .route("/login", post(auth::login))
        .route("/logout", post(auth::logout))
        .route("/auth/oidc/start", get(oidc::start))
        .route("/auth/oidc/callback", get(oidc::callback))
        .route("/projects", get(api::projects))
        .route("/chats", get(api::chats).post(api::create_chat))
        .route("/chats/{id}", patch(api::update_chat).delete(api::delete_chat))
        .route("/chats/{id}/messages", get(api::messages).post(api::send))
        .route("/providers", get(api::providers))
        .route("/roles", get(api::roles).put(api::set_role))
        .route("/calls", get(api::calls))
        .route("/tasks", get(tasks::list).post(tasks::create))
        .route("/tasks/order", axum::routing::put(tasks::reorder))
        .route("/tasks/{id}", patch(tasks::update).delete(tasks::delete))
        .route("/tasks/{id}/events", get(tasks::events))
        .route("/projects/{id}", patch(tasks::set_project_kind))
        .route("/machines", get(hoststats::list).post(hoststats::create))
        .route("/machines/live", post(hoststats::live))
        .route("/machines/{id}", axum::routing::delete(hoststats::delete))
        .route(
            "/machines/{id}/stats",
            post(hoststats::report).layer(axum::extract::DefaultBodyLimit::max(64 * 1024)),
        )
        .route("/me/notifications", get(notify::get_prefs).put(notify::put_prefs))
        .route("/me/prefs", axum::routing::put(admin::set_prefs))
        .route("/events", get(api::events))
        .layer(middleware::from_fn_with_state(state.clone(), auth::guard))
        .layer(SetResponseHeaderLayer::overriding(
            header::CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ));

    let web =
        ServeDir::new(&config.web_dir).fallback(ServeFile::new(config.web_dir.join("index.html")));

    let app = Router::new()
        .nest("/api", api)
        .fallback_service(web)
        .layer(SetResponseHeaderLayer::overriding(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(CSP),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            header::REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&config.bind)
        .await
        .with_context(|| format!("binding {}", config.bind))?;
    tracing::info!("Kreative Kompanion listening on {}", config.bind);
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    Ok(())
}
