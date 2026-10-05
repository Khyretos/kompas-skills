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
    /// GPUs for the M6 ledger and scheduler (`[[gpu]]`), with what may live on each.
    #[serde(default, rename = "gpu")]
    pub gpus: Vec<GpuConfig>,
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
    /// UnifiedPush servers the Android app may register endpoints on (`[push] servers = [...]`).
    #[serde(default)]
    pub push: PushConfig,
    /// `[assets]`: the models the asset library uses for tags and search; empty = the worker role's provider and model.
    #[serde(default)]
    pub assets: AssetsConfig,
    /// `[voice]`: speech to text and text to speech; off when neither URL is set.
    #[serde(default)]
    pub voice: VoiceConfig,
    /// `[features]`: switch whole areas off (all on by default).
    #[serde(default)]
    pub features: FeaturesConfig,
}

/// `[push]`: where phones may receive pushes (an ntfy server); empty = push off.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct PushConfig {
    #[serde(default)]
    pub servers: Vec<String>,
}

/// `[assets]`. Every field is optional; environment variables (ASSET_AI_URL, ASSET_AI_MODEL,
/// ASSET_EMBED_URL) still win, so older deploys keep working.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AssetsConfig {
    /// OpenAI-compatible base URL for tagging, e.g. "http://ovms:8000/v3".
    #[serde(default)]
    pub chat_url: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// Base URL with /embeddings and /rerank for search; none = the chat URL.
    #[serde(default)]
    pub embed_url: Option<String>,
    #[serde(default)]
    pub embed_model: Option<String>,
    #[serde(default)]
    pub rerank_model: Option<String>,
    /// Model for describing audio files (speech to text).
    #[serde(default)]
    pub audio_model: Option<String>,
    /// Environment variable with the API key, e.g. "OVMS_API_KEY".
    #[serde(default)]
    pub api_key_env: Option<String>,
}

/// `[voice]`. Environment variables (VOICE_STT_URL, VOICE_STT_MODEL, VOICE_TTS_URL,
/// VOICE_TTS_MODEL) still win.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct VoiceConfig {
    #[serde(default)]
    pub stt_url: Option<String>,
    #[serde(default)]
    pub stt_model: Option<String>,
    #[serde(default)]
    pub tts_url: Option<String>,
    #[serde(default)]
    pub tts_model: Option<String>,
    #[serde(default)]
    pub api_key_env: Option<String>,
}

/// `[features]`: an area that is off has no routes, no background jobs and no menu item.
#[derive(Debug, Clone, Deserialize)]
pub struct FeaturesConfig {
    #[serde(default = "yes")]
    pub assets: bool,
    #[serde(default = "yes")]
    pub gpus: bool,
    #[serde(default = "yes")]
    pub voice: bool,
    #[serde(default = "yes")]
    pub windshift: bool,
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        FeaturesConfig { assets: true, gpus: true, voice: true, windshift: true }
    }
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

/// One GPU: `[[gpu]] id = "a770", machine = "kireserver", pci = "0000:10:00.0", vram_gb = 16`.
#[derive(Debug, Clone, Deserialize)]
pub struct GpuConfig {
    pub id: String,
    /// This server's machine_name, or a paired computer's name.
    pub machine: String,
    pub pci: String,
    pub vram_gb: f64,
    /// false: never scheduled (the A580: desktop and Jellyfin).
    #[serde(default = "yes")]
    pub schedulable: bool,
    #[serde(default, rename = "holder")]
    pub holders: Vec<HolderConfig>,
}

/// Something that can hold VRAM on a GPU and how to see it:
/// probe "ovms:<url>" (model = OVMS name), "ollama:<url>" (model = name, or "*" for any),
/// "comfyui:<url>", or "studio:<url>" (an app with GET /health {loaded, busy}).
/// Its reserved peak is weights_mb + kv_mb_per_seq x max_seqs: an LLM's KV cache grows with
/// every parallel sequence (2026-10-04: Coder with 8 sequences outgrew the A770).
#[derive(Debug, Clone, Deserialize)]
pub struct HolderConfig {
    pub name: String,
    #[serde(default = "model_kind")]
    pub kind: String,
    pub probe: String,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub weights_mb: u64,
    #[serde(default)]
    pub kv_mb_per_seq: u64,
    #[serde(default = "one")]
    pub max_seqs: u64,
}

impl HolderConfig {
    pub fn peak_mb(&self) -> u64 {
        self.weights_mb + self.kv_mb_per_seq * self.max_seqs
    }
}

fn model_kind() -> String {
    "model".into()
}

fn one() -> u64 {
    1
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
