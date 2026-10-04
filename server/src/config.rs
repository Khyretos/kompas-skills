use std::{collections::HashMap, path::PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_db")]
    pub database: PathBuf,
    #[serde(default = "default_web")]
    pub web_dir: PathBuf,
    #[serde(default)]
    pub allowed_origins: Vec<String>,
    /// Set the Secure flag on cookies. Only turn off for plain-http testing on localhost.
    #[serde(default = "yes")]
    pub secure_cookies: bool,
    /// Name shown for this server in the Machines panel.
    #[serde(default)]
    pub machine_name: Option<String>,
    /// What each GPU of this server is used for, by PCI slot, e.g.
    /// `gpu_labels = { "0000:10:00.0" = "AI (OVMS)" }`.
    #[serde(default)]
    pub gpu_labels: std::collections::HashMap<String, String>,
    #[serde(default, rename = "provider")]
    pub providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub roles: HashMap<String, RoleDefault>,
    /// Single sign-on with an OpenID Connect provider such as Keycloak.
    pub oidc: Option<OidcConfig>,
    /// Folder with the runner binaries the one-line installer downloads
    /// (`kompanion-runner-<version>-x86_64-linux-musl` plus `.sha256`).
    #[serde(default = "default_runner_dir")]
    pub runner_dir: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OidcConfig {
    /// e.g. https://auth.example.com/realms/example
    pub issuer: String,
    /// Keycloak role (realm role, or client role of `client_id`) that makes a
    /// user admin; checked at every sign-in. Unset: admins are managed here.
    #[serde(default)]
    pub admin_role: Option<String>,
    pub client_id: String,
    /// Name of the environment variable that holds the client secret.
    pub client_secret_env: String,
    /// Must match the redirect URI registered with the provider:
    /// https://<your host>/api/auth/oidc/callback
    pub redirect_url: String,
    /// Text on the sign-in button.
    #[serde(default = "default_oidc_label")]
    pub label: String,
    /// Keep name-and-password sign-in next to single sign-on.
    #[serde(default = "yes")]
    pub password_login: bool,
    /// Link a first single sign-on to an existing account with the same
    /// name as the provider's username. Only safe when users can't pick
    /// their own username at the provider.
    #[serde(default = "yes")]
    pub link_by_username: bool,
    /// Emails or usernames that may get a new account on first sign-in.
    /// Everyone else must match an existing account. `"*"` lets everyone the
    /// provider signs in get an account (the provider decides who gets in).
    #[serde(default)]
    pub allow_new: Vec<String>,
}

impl OidcConfig {
    pub fn client_secret(&self) -> Option<String> {
        std::env::var(&self.client_secret_env)
            .ok()
            .filter(|s| !s.is_empty())
    }
}

fn default_oidc_label() -> String {
    "Single sign-on".into()
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    OpenaiCompatible,
    Anthropic,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    pub id: String,
    pub name: String,
    pub kind: ProviderKind,
    pub base_url: String,
    #[serde(default)]
    pub local: bool,
    /// Name of the environment variable that holds the API key.
    pub api_key_env: Option<String>,
    /// Extra fields merged into every chat request, e.g. to turn off long
    /// "thinking" on Qwen: `extra_body = { chat_template_kwargs = { enable_thinking = false } }`
    #[serde(default)]
    pub extra_body: Option<toml::Table>,
}

impl ProviderConfig {
    pub fn api_key(&self) -> Option<String> {
        self.api_key_env
            .as_ref()
            .and_then(|v| std::env::var(v).ok())
            .filter(|k| !k.is_empty())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct RoleDefault {
    pub provider: String,
    pub model: String,
}

fn default_bind() -> String {
    "0.0.0.0:8080".into()
}
fn default_db() -> PathBuf {
    "kompanion.db".into()
}
fn default_runner_dir() -> PathBuf {
    "/dist".into()
}
fn default_web() -> PathBuf {
    "../web/dist".into()
}
fn yes() -> bool {
    true
}

impl Config {
    pub fn load() -> Result<Self> {
        let path = std::env::var("KOMPANION_CONFIG").unwrap_or_else(|_| "kompanion.toml".into());
        let text = std::fs::read_to_string(&path).with_context(|| format!("reading {path}"))?;
        let config: Config = toml::from_str(&text).with_context(|| format!("parsing {path}"))?;
        config.validate()?;
        Ok(config)
    }

    fn validate(&self) -> Result<()> {
        for p in &self.providers {
            if !(p.base_url.starts_with("http://") || p.base_url.starts_with("https://")) {
                bail!(
                    "provider {}: base_url must start with http:// or https://",
                    p.id
                );
            }
            if let Some(env) = &p.api_key_env
                && std::env::var(env).is_err()
            {
                tracing::warn!(provider = %p.id, env = %env, "API key variable is not set");
            }
        }
        if let Some(o) = &self.oidc {
            if !o.issuer.starts_with("https://") {
                bail!("oidc.issuer must start with https://");
            }
            if !o.redirect_url.ends_with("/api/auth/oidc/callback") {
                bail!("oidc.redirect_url must end with /api/auth/oidc/callback");
            }
            if o.client_secret().is_none() {
                bail!(
                    "oidc: environment variable {} is not set",
                    o.client_secret_env
                );
            }
        }
        for (role, d) in &self.roles {
            if !["orchestrator", "worker", "reviewer"].contains(&role.as_str()) {
                bail!("unknown role {role}");
            }
            if self.provider(&d.provider).is_none() {
                bail!("role {role} uses unknown provider {}", d.provider);
            }
        }
        Ok(())
    }

    pub fn password_login(&self) -> bool {
        self.oidc.as_ref().is_none_or(|o| o.password_login)
    }

    pub fn provider(&self, id: &str) -> Option<&ProviderConfig> {
        self.providers.iter().find(|p| p.id == id)
    }
}
