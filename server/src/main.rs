mod api;
mod auth;
mod config;
mod error;
mod events;
mod llm;
mod util;

use std::{
    str::FromStr,
    sync::{Arc, Mutex},
};

use anyhow::Context;
use axum::{
    Router,
    http::{HeaderValue, header},
    middleware,
    routing::{get, post},
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

    // Starting roles from the config file, only where none is set yet.
    for (role, d) in &config.roles {
        sqlx::query("INSERT OR IGNORE INTO roles (role, provider_id, model_id) VALUES (?, ?, ?)")
            .bind(role)
            .bind(&d.provider)
            .bind(&d.model)
            .execute(&db)
            .await?;
    }

    let state = AppState {
        config: Arc::new(config.clone()),
        db,
        http: llm::http_client(),
        bus: events::Bus::new(),
        throttle: Default::default(),
        setup_code: Default::default(),
        dummy_hash: auth::hash_password(&util::random_token())?,
    };
    auth::ensure_setup_code(&state).await?;

    let api = Router::new()
        .route("/status", get(auth::status))
        .route("/setup", post(auth::setup))
        .route("/login", post(auth::login))
        .route("/logout", post(auth::logout))
        .route("/projects", get(api::projects))
        .route("/chats", get(api::chats).post(api::create_chat))
        .route("/chats/{id}/messages", get(api::messages).post(api::send))
        .route("/providers", get(api::providers))
        .route("/roles", get(api::roles).put(api::set_role))
        .route("/calls", get(api::calls))
        .route("/tasks", get(api::tasks))
        .route("/machines", get(api::machines))
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
