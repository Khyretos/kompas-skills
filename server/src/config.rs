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
    #[serde(default, rename = "provider")]
    pub providers: Vec<ProviderConfig>,
    #[serde(default)]
    pub roles: HashMap<String, RoleDefault>,
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

    pub fn provider(&self, id: &str) -> Option<&ProviderConfig> {
        self.providers.iter().find(|p| p.id == id)
    }
}
