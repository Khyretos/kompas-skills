//! AI tags and captions for the asset library, through the models set in `[assets]` (kompanion.toml):
//! `Coder` looks at a picture's preview (PNG) or reads sound and model names with their
//! probe numbers, `Whisper` writes down voice lines, `Embedder` (ovms-cpu) turns each
//! description into a vector for "similar" and "by meaning" search (sqlite-vec).
//!
//! Off by default. An admin picks Off / Nightly / Always in the Assets header (settings key
//! `assets.ai`); deploying this starts nothing. While on, one request at a time, followed by
//! an equal pause, so tagging uses at most about half of the GPU's time.
//!
//! Answers are checked against fixed word lists: a word not on the list is dropped, and a
//! category the AI disagrees with goes to a review list instead of replacing the rule's.

use std::{
    collections::HashMap,
    path::Path,
    process::Stdio,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tokio::sync::Notify;

use super::{classify, preview};
use crate::events::{Bus, Event};

/// Embedding size kept per asset (Qwen3-Embedding gives 1024; its first 512 work on their own).
pub const DIMS: usize = 512;
const SETTINGS_KEY: &str = "assets.ai";
const SOUND_BATCH: usize = 12;
const NAME_BATCH: usize = 20;

pub const STYLES_VISUAL: &[&str] =
    &["low-poly", "pixel-art", "hand-painted", "realistic", "stylized", "cartoon", "flat", "line-art", "photo", "painterly"];
pub const STYLES_AUDIO: &[&str] = &[
    "orchestral", "electronic", "chiptune", "synthwave", "rock", "ambient", "acoustic", "piano", "choir", "foley",
    "lo-fi", "cinematic", "retro",
];
pub const MOODS: &[&str] = &[
    "calm", "tense", "dark", "happy", "sad", "epic", "mysterious", "playful", "aggressive", "eerie", "heroic",
    "romantic", "energetic", "melancholic",
];
pub const SETTINGS: &[&str] = &[
    "fantasy", "sci-fi", "horror", "medieval", "modern", "western", "pirate", "viking", "war", "post-apocalyptic",
    "cyberpunk", "nature", "urban", "space", "underwater", "desert", "winter", "dungeon", "ui",
];

/// SQL for the AI text in search (`asset_fts.ai`): caption, subject, transcript and tags.
pub const FTS_AI_TEXT: &str = "COALESCE(a.ai_caption, '') || ' ' || COALESCE(a.ai_subject, '') || ' ' || \
     COALESCE(a.ai_transcript, '') || ' ' || COALESCE((SELECT group_concat(t.name, ' ') FROM asset_tag x \
     JOIN asset_tagname t ON t.id = x.tag_id WHERE x.asset_id = a.id), '')";

// ---------- settings and progress ----------

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Off,
    /// 23:00-06:00 UTC (01:00-08:00 in the Netherlands in summer).
    Night,
    Always,
}

pub fn night(hour_utc: u8) -> bool {
    !(6..23).contains(&hour_utc)
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub mode: Mode,
    /// Working right now (on, inside its hours, with work to do).
    pub running: bool,
    /// On, but waiting for the night window.
    pub waiting: bool,
    pub todo: i64,
    pub done: i64,
    pub failed: i64,
    pub last_error: Option<String>,
}

pub static PROGRESS: LazyLock<Mutex<Progress>> = LazyLock::new(Default::default);
static WAKE: LazyLock<Notify> = LazyLock::new(Notify::new);
static PRIORITY: LazyLock<Mutex<Vec<i64>>> = LazyLock::new(Default::default);

pub fn wake() {
    WAKE.notify_one();
}

pub async fn mode(db: &SqlitePool) -> Mode {
    let v: Option<(String,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = ?").bind(SETTINGS_KEY).fetch_optional(db).await.ok().flatten();
    v.and_then(|(v,)| serde_json::from_str(&v).ok()).unwrap_or_default()
}

pub async fn set_mode(db: &SqlitePool, m: Mode) -> Result<()> {
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(SETTINGS_KEY)
        .bind(serde_json::to_string(&m)?)
        .execute(db)
        .await?;
    PROGRESS.lock().unwrap().mode = m;
    wake();
    Ok(())
}

/// Describe this one next (the "Describe now" button). Only while tagging is on.
pub fn prioritise(id: i64) {
    let mut q = PRIORITY.lock().unwrap();
    if !q.contains(&id) {
        q.insert(0, id);
        q.truncate(100);
    }
    drop(q);
    wake();
}

// ---------- sqlite-vec ----------

/// Registers sqlite-vec for every SQLite connection opened afterwards. Call before the pool.
pub fn register_sqlite_vec() {
    // SAFETY: sqlite3_vec_init has the extension entry-point signature SQLite expects;
    // sqlite3_auto_extension only stores the pointer.
    unsafe {
        libsqlite3_sys::sqlite3_auto_extension(Some(std::mem::transmute::<*const (), unsafe extern "C" fn(
            *mut libsqlite3_sys::sqlite3,
            *mut *mut std::os::raw::c_char,
            *const libsqlite3_sys::sqlite3_api_routines,
        ) -> std::os::raw::c_int>(sqlite_vec::sqlite3_vec_init as *const ())));
    }
}

/// The vector table lives outside the migrations: it needs sqlite-vec on the connection.
pub async fn ensure_vectors(db: &SqlitePool) -> Result<()> {
    sqlx::query(&format!("CREATE VIRTUAL TABLE IF NOT EXISTS asset_vec USING vec0(embedding float[{DIMS}])"))
        .execute(db)
        .await?;
    Ok(())
}

fn to_blob(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|f| f.to_le_bytes()).collect()
}

/// First DIMS values, scaled to length 1 (cosine = dot product).
pub fn shorten(v: &[f32]) -> Vec<f32> {
    let v = &v[..v.len().min(DIMS)];
    let n = v.iter().map(|x| x * x).sum::<f32>().sqrt().max(1e-9);
    v.iter().map(|x| x / n).collect()
}

pub async fn store_vector(db: &SqlitePool, id: i64, v: &[f32]) -> Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM asset_vec WHERE rowid = ?").bind(id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO asset_vec (rowid, embedding) VALUES (?, ?)").bind(id).bind(to_blob(v)).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Nearest assets to a vector: (asset id, distance), closest first.
pub async fn nearest(db: &SqlitePool, v: &[f32], k: i64) -> Result<Vec<(i64, f64)>> {
    Ok(sqlx::query_as("SELECT rowid, distance FROM asset_vec WHERE embedding MATCH ? AND k = ? ORDER BY distance")
        .bind(to_blob(v))
        .bind(k)
        .fetch_all(db)
        .await?)
}

pub async fn vector_of(db: &SqlitePool, id: i64) -> Result<Option<Vec<f32>>> {
    let b: Option<(Vec<u8>,)> = sqlx::query_as("SELECT embedding FROM asset_vec WHERE rowid = ?").bind(id).fetch_optional(db).await?;
    Ok(b.map(|(b,)| b.chunks_exact(4).map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]])).collect()))
}

// ---------- OVMS client ----------

/// The asset AI settings from `[assets]` in kompanion.toml, set once at start (main.rs).
pub static CONFIG: std::sync::OnceLock<crate::config::AssetsConfig> = std::sync::OnceLock::new();

/// `[assets]` with the worker role's provider and model filling what is not set.
pub fn settings_from(c: &crate::config::Config) -> crate::config::AssetsConfig {
    let mut a = c.assets.clone();
    if let Some(w) = c.roles.get("worker") {
        if let Some(p) = c.provider(&w.provider) {
            if a.chat_url.is_none() { a.chat_url = Some(p.base_url.clone()); }
            if a.api_key_env.is_none() { a.api_key_env = p.api_key_env.clone(); }
        }
        if a.model.is_none() { a.model = Some(w.model.clone()); }
    }
    a
}

pub struct Ai {
    http: reqwest::Client,
    chat_url: String,
    embed_url: String,
    key: Option<String>,
    pub model: String,
    embed_model: String,
    rerank_model: String,
    audio_model: String,
}

impl Ai {
    /// Environment first (ASSET_AI_URL, ASSET_AI_MODEL, ASSET_EMBED_URL, ASSET_AI_KEY_ENV), then
    /// `[assets]` (see `CONFIG`), then OVMS's model names as a last default.
    pub fn from_env(http: reqwest::Client) -> Self {
        let s = CONFIG.get().cloned().unwrap_or_default();
        let env = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
        let chat_url = env("ASSET_AI_URL").or(s.chat_url).unwrap_or_default();
        let key_env = env("ASSET_AI_KEY_ENV").or(s.api_key_env).unwrap_or_else(|| "OVMS_API_KEY".to_string());
        Ai {
            http,
            embed_url: env("ASSET_EMBED_URL").or(s.embed_url).unwrap_or_else(|| chat_url.clone()),
            chat_url,
            key: std::env::var(&key_env).ok().filter(|k| !k.is_empty()),
            model: env("ASSET_AI_MODEL").or(s.model).unwrap_or_default(),
            embed_model: s.embed_model.unwrap_or_else(|| "Embedder".to_string()),
            rerank_model: s.rerank_model.unwrap_or_else(|| "Reranker".to_string()),
            audio_model: s.audio_model.unwrap_or_else(|| "Whisper".to_string()),
        }
    }

    /// An OVMS stand-in for tests (chat and embeddings at the same URL).
    #[cfg(test)]
    pub fn fake(url: &str) -> Self {
        Ai { http: reqwest::Client::new(), chat_url: url.into(), embed_url: url.into(), key: None, model: "Fake".into(), embed_model: "Fake".into(), rerank_model: "Fake".into(), audio_model: "Fake".into() }
    }

    fn auth(&self, r: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.key {
            Some(k) => r.bearer_auth(k),
            None => r,
        }
    }

    async fn ok_json(r: reqwest::Response) -> Result<Value> {
        let status = r.status();
        let body: Value = r.json().await.unwrap_or(Value::Null);
        if !status.is_success() {
            bail!("OVMS answered {status}: {}", body["error"].as_str().unwrap_or("no details"));
        }
        Ok(body)
    }

    pub async fn chat(&self, content: Value, max_tokens: u32) -> Result<String> {
        let body = json!({
            "model": self.model, "max_tokens": max_tokens, "temperature": 0.1,
            "chat_template_kwargs": { "enable_thinking": false },
            "messages": [{ "role": "user", "content": content }],
        });
        let r = self.auth(self.http.post(format!("{}/chat/completions", self.chat_url)).json(&body)).send().await?;
        let v = Self::ok_json(r).await?;
        Ok(v["choices"][0]["message"]["content"].as_str().unwrap_or_default().to_string())
    }

    pub async fn transcribe(&self, wav: Vec<u8>) -> Result<String> {
        let form = reqwest::multipart::Form::new()
            .text("model", self.audio_model.clone())
            .part("file", reqwest::multipart::Part::bytes(wav).file_name("clip.wav").mime_str("audio/wav")?);
        let r = self.auth(self.http.post(format!("{}/audio/transcriptions", self.chat_url)).multipart(form)).send().await?;
        Ok(Self::ok_json(r).await?["text"].as_str().unwrap_or_default().trim().to_string())
    }

    pub async fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>> {
        let r = self
            .auth(self.http.post(format!("{}/embeddings", self.embed_url)).json(&json!({ "model": self.embed_model, "input": texts })))
            .send()
            .await?;
        let v = Self::ok_json(r).await?;
        let data = v["data"].as_array().context("no embeddings in the answer")?;
        Ok(data
            .iter()
            .map(|d| shorten(&d["embedding"].as_array().map(|a| a.iter().filter_map(|x| x.as_f64()).map(|x| x as f32).collect::<Vec<_>>()).unwrap_or_default()))
            .collect())
    }

    /// Orders documents by how well they answer the query (`Reranker` on ovms-cpu):
    /// (index into `docs`, score), best first.
    pub async fn rerank(&self, query: &str, docs: &[String]) -> Result<Vec<(usize, f64)>> {
        let body = json!({ "model": self.rerank_model, "query": query, "documents": docs, "top_n": docs.len() });
        let r = self.auth(self.http.post(format!("{}/rerank", self.embed_url)).json(&body)).send().await?;
        let v = Self::ok_json(r).await?;
        let results = v["results"].as_array().context("no results in the rerank answer")?;
        let mut out: Vec<(usize, f64)> = results
            .iter()
            .filter_map(|x| Some((x["index"].as_u64()? as usize, x["relevance_score"].as_f64()?)))
            .filter(|(i, _)| *i < docs.len())
            .collect();
        out.sort_by(|a, b| b.1.total_cmp(&a.1));
        Ok(out)
    }
}

// ---------- prompts and answers ----------

/// What the AI said about one asset, already checked against the word lists.
#[derive(Debug, Default, PartialEq, Serialize)]
pub struct Tagged {
    pub category: Option<String>,
    pub style: Vec<String>,
    pub mood: Vec<String>,
    pub setting: Vec<String>,
    pub subject: Option<String>,
    pub caption: Option<String>,
}

fn list(words: &[&str]) -> String {
    words.join(", ")
}

/// The fixed answer format, shared by every prompt.
fn format_rules(styles: &[&str], categories: &[&str]) -> String {
    format!(
        "Answer with JSON only, no other text. Fields:\n\
         - \"category\": one of: {}\n\
         - \"style\": up to 2 of: {}\n\
         - \"mood\": up to 2 of: {} (only for music, ambience, scenes and characters; [] for UI, icons, screenshots and surface maps)\n\
         - \"setting\": up to 2 of: {} (only when the asset clearly shows or names one, else [])\n\
         - \"subject\": a string of 1 to 4 plain words naming what it is\n\
         - \"caption\": one short sentence a game developer would search for\n\
         Use only words from these lists; leave a list empty when nothing fits. Never guess a licence or a brand.",
        list(categories),
        list(styles),
        list(MOODS),
        list(SETTINGS)
    )
}

const PICTURE_CATEGORIES: &[&str] = &["texture", "sprite", "vfx", "image"];
const SOUND_CATEGORIES: &[&str] = &["sound-effect", "music", "ambience", "voice"];
const NAME_CATEGORIES: &[&str] = &["3d-model", "animation", "texture", "material", "vfx", "font", "shader"];

pub struct Item<'a> {
    pub id: i64,
    pub name: &'a str,
    pub path: &'a str,
    pub pack: &'a str,
    pub category: &'a str,
    pub facts: String,
}

pub fn picture_prompt(a: &Item) -> String {
    format!(
        "This is a preview of a game asset file. File: \"{}\" in pack \"{}\" (path {}). {}\n\
         texture = a surface map (albedo, normal, roughness...), sprite = a 2D game graphic or UI icon, \
         vfx = a particle or effect sheet, image = anything else.\n{}",
        a.name,
        a.pack,
        a.path,
        a.facts,
        format_rules(STYLES_VISUAL, PICTURE_CATEGORIES)
    )
}

pub fn batch_prompt(items: &[Item], sound: bool) -> String {
    let lines: Vec<String> = items
        .iter()
        .map(|a| format!("{{\"id\": {}, \"file\": {:?}, \"pack\": {:?}, \"path\": {:?}, \"facts\": {:?}}}", a.id, a.name, a.pack, a.path, a.facts))
        .collect();
    let (what, styles, cats, hint) = if sound {
        (
            "sound files",
            STYLES_AUDIO,
            SOUND_CATEGORIES,
            "sound-effect = short (under ~10 s), ambience = a background loop, music = a composed track (often over 60 s), voice = speech.",
        )
    } else {
        ("3D and other asset files, known only by name", STYLES_VISUAL, NAME_CATEGORIES, "Judge from the names only; keep captions modest.")
    };
    format!(
        "Describe each of these {what} from a game asset library. {hint}\n{}\n\n\
         Answer with a JSON array: one object per file, in the same order, each with \"id\" (copied) and the fields below.\n{}",
        lines.join("\n"),
        format_rules(styles, cats)
    )
}

/// The first JSON object or array in a model answer (models like ```json fences).
pub fn json_in(answer: &str) -> Option<Value> {
    let start = answer.find(['{', '['])?;
    let open = answer.as_bytes()[start];
    let close = if open == b'{' { '}' } else { ']' };
    let end = answer.rfind(close)?;
    serde_json::from_str(answer.get(start..=end)?).ok()
}

fn words(v: &Value, allowed: &[&str], max: usize) -> Vec<String> {
    let items: Vec<&str> = match v {
        Value::Array(a) => a.iter().filter_map(Value::as_str).collect(),
        Value::String(s) => s.split(',').collect(),
        _ => vec![],
    };
    let mut out: Vec<String> = vec![];
    for w in items {
        let w = w.trim().to_lowercase().replace(' ', "-");
        if allowed.contains(&w.as_str()) && !out.contains(&w) && out.len() < max {
            out.push(w);
        }
    }
    out
}

fn short_text(v: &Value, max: usize) -> Option<String> {
    let s = v.as_str()?.trim();
    (!s.is_empty()).then(|| s.chars().take(max).collect())
}

/// Checks one answer object against the word lists. Unknown words are dropped.
pub fn check(v: &Value, styles: &[&str], categories: &[&str]) -> Tagged {
    let category = v["category"].as_str().map(|c| c.trim().to_lowercase()).filter(|c| categories.contains(&c.as_str()));
    Tagged {
        category,
        style: words(&v["style"], styles, 2),
        mood: words(&v["mood"], MOODS, 2),
        setting: words(&v["setting"], SETTINGS, 2),
        // Models often send the subject as a list of words.
        subject: short_text(&v["subject"], 60).or_else(|| {
            let words: Vec<&str> = v["subject"].as_array()?.iter().filter_map(Value::as_str).map(str::trim).filter(|w| !w.is_empty()).take(4).collect();
            short_text(&Value::from(words.join(" ")), 60)
        }),
        caption: short_text(&v["caption"], 240),
    }
}

/// Matches a batch answer to its items by "id" (falling back to order). Missing ones are None.
pub fn check_batch(answer: &Value, ids: &[i64], styles: &[&str], categories: &[&str]) -> Vec<Option<Tagged>> {
    let arr = answer.as_array().cloned().unwrap_or_default();
    let by_id: HashMap<i64, &Value> = arr.iter().filter_map(|o| Some((o["id"].as_i64()?, o))).collect();
    ids.iter()
        .enumerate()
        .map(|(i, id)| by_id.get(id).copied().or_else(|| (by_id.is_empty()).then(|| arr.get(i)).flatten()).map(|o| check(o, styles, categories)))
        .collect()
}

/// The text that is embedded for "similar" and "by meaning" search.
pub fn embed_text(name: &str, pack: &str, category: &str, t: &Tagged, transcript: Option<&str>) -> String {
    let mut parts = vec![format!("{name} ({category}, pack {pack})")];
    if let Some(c) = &t.caption {
        parts.push(c.clone());
    }
    if let Some(s) = &t.subject {
        parts.push(s.clone());
    }
    let tags: Vec<&str> = t.style.iter().chain(&t.mood).chain(&t.setting).map(String::as_str).collect();
    if !tags.is_empty() {
        parts.push(tags.join(", "));
    }
    if let Some(tr) = transcript.filter(|t| !t.is_empty()) {
        parts.push(format!("says: {tr}"));
    }
    parts.join(". ")
}

// ---------- saving ----------

async fn save(db: &SqlitePool, id: i64, current_category: &str, t: &Tagged, model: &str) -> Result<()> {
    let disagree = t.category.as_deref().filter(|c| *c != current_category);
    let mut tx = db.begin().await?;
    sqlx::query(
        "UPDATE asset SET ai_state = 'ok', ai_error = NULL, ai_caption = ?, ai_subject = ?,
         ai_category = CASE WHEN category_by = 'kees' THEN NULL ELSE ? END, ai_model = ?, ai_at = ? WHERE id = ?",
    )
    .bind(&t.caption)
    .bind(&t.subject)
    .bind(disagree)
    .bind(model)
    .bind(crate::util::now())
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM asset_tag WHERE asset_id = ? AND by = 'ai'").bind(id).execute(&mut *tx).await?;
    for (kind, names) in [("style", &t.style), ("mood", &t.mood), ("setting", &t.setting)] {
        for name in names {
            sqlx::query("INSERT INTO asset_tagname (kind, name) VALUES (?, ?) ON CONFLICT DO NOTHING")
                .bind(kind)
                .bind(name)
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                "INSERT INTO asset_tag (asset_id, tag_id, by) SELECT ?, id, 'ai' FROM asset_tagname WHERE kind = ? AND name = ?
                 ON CONFLICT DO NOTHING",
            )
            .bind(id)
            .bind(kind)
            .bind(name)
            .execute(&mut *tx)
            .await?;
        }
    }
    tx.commit().await?;
    refresh_search(db, id).await
}

/// Rewrites one asset's search row (after tags or a caption changed).
pub async fn refresh_search(db: &SqlitePool, id: i64) -> Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM asset_fts WHERE rowid = ?").bind(id).execute(&mut *tx).await?;
    sqlx::query(&format!(
        "INSERT INTO asset_fts (rowid, name, path, pack, category, ai)
         SELECT a.id, a.name, CASE a.container WHEN '' THEN a.path ELSE a.container || '/' || a.path END, p.name, a.category,
                {FTS_AI_TEXT}
         FROM asset a JOIN asset_pack p ON p.id = a.pack_id WHERE a.id = ? AND a.missing_since IS NULL"
    ))
    .bind(id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn fail(db: &SqlitePool, ids: &[i64], e: &anyhow::Error) -> Result<()> {
    let msg: String = format!("{e:#}").chars().take(300).collect();
    for id in ids {
        sqlx::query("UPDATE asset SET ai_state = 'error', ai_error = ? WHERE id = ?").bind(&msg).bind(id).execute(db).await?;
    }
    let mut p = PROGRESS.lock().unwrap();
    p.failed += ids.len() as i64;
    p.last_error = Some(msg);
    Ok(())
}

// ---------- the worker ----------

/// Work that can be tagged: previewed pictures, sounds, and models and other files by name.
fn todo_where() -> String {
    format!(
        "a.ai_state IS NULL AND a.missing_since IS NULL AND a.dup_of IS NULL AND a.is_meta = 0 AND a.category IN ({})",
        ["texture", "sprite", "vfx", "image", "sound-effect", "music", "ambience", "voice", "3d-model", "animation", "material", "font", "shader"]
            .iter()
            .map(|c| format!("'{c}'"))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

#[derive(sqlx::FromRow)]
struct Row {
    id: i64,
    name: String,
    container: String,
    path: String,
    pack: String,
    category: String,
    preview_kind: Option<String>,
    preview_state: Option<String>,
    duration_s: Option<f64>,
    sample_rate: Option<i64>,
    channels: Option<i64>,
    width: Option<i64>,
    height: Option<i64>,
    ai_transcript: Option<String>,
}

const ROW_COLUMNS: &str = "a.id, a.name, a.container, a.path, p.name AS pack, a.category, a.preview_kind, a.preview_state, \
     a.duration_s, a.sample_rate, a.channels, a.width, a.height, a.ai_transcript";

fn facts(r: &Row) -> String {
    let mut f = vec![];
    if let (Some(w), Some(h)) = (r.width, r.height) {
        f.push(format!("{w} x {h} px"));
    }
    if let Some(d) = r.duration_s {
        f.push(format!("{d:.1} s long"));
    }
    if let Some(rate) = r.sample_rate {
        f.push(format!("{rate} Hz"));
    }
    if let Some(c) = r.channels {
        f.push(if c == 1 { "mono".into() } else { format!("{c} channels") });
    }
    if let Some(t) = r.ai_transcript.as_deref().filter(|t| !t.is_empty()) {
        f.push(format!("the voice says: {t:?}"));
    }
    f.push(format!("currently filed as {}", r.category));
    f.join(", ")
}

fn publish(bus: &Bus, ids: Vec<i64>) {
    let p = PROGRESS.lock().unwrap().clone();
    bus.send_all(Event::Assets { scan: None, previews: None, ai: Some(json!({ "ids": ids, "progress": p })), games: None });
}

pub fn spawn(db: SqlitePool, bus: Bus, ai: Ai, previews: std::path::PathBuf) {
    tokio::spawn(async move {
        if let Err(e) = ensure_vectors(&db).await {
            tracing::error!(error = ?e, "asset vectors: sqlite-vec missing, AI search off");
        }
        loop {
            let m = mode(&db).await;
            let hour = time::OffsetDateTime::now_utc().hour();
            {
                let mut p = PROGRESS.lock().unwrap();
                p.mode = m;
                p.waiting = m == Mode::Night && !night(hour);
            }
            let allowed = m == Mode::Always || (m == Mode::Night && night(hour));
            if !allowed || super::scan::PROGRESS.lock().unwrap().running {
                PROGRESS.lock().unwrap().running = false;
                publish(&bus, vec![]);
                let _ = tokio::time::timeout(Duration::from_secs(300), WAKE.notified()).await;
                continue;
            }
            let started = Instant::now();
            match step(&db, &bus, &ai, &previews).await {
                Ok(0) => {
                    PROGRESS.lock().unwrap().running = false;
                    publish(&bus, vec![]);
                    let _ = tokio::time::timeout(Duration::from_secs(600), WAKE.notified()).await;
                }
                Ok(_) => {
                    // Half duty: rest as long as the work took, so others get the GPU too.
                    tokio::time::sleep(started.elapsed().max(Duration::from_secs(1))).await;
                }
                Err(e) => {
                    tracing::warn!(error = ?e, "asset AI");
                    PROGRESS.lock().unwrap().last_error = Some(format!("{e:#}").chars().take(300).collect());
                    tokio::time::sleep(Duration::from_secs(120)).await;
                }
            }
        }
    });
}

/// One request's worth of work. Returns how many assets it handled.
async fn step(db: &SqlitePool, bus: &Bus, ai: &Ai, previews: &Path) -> Result<usize> {
    let todo: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM asset a WHERE {}", todo_where())).fetch_one(db).await?;
    {
        let mut p = PROGRESS.lock().unwrap();
        p.todo = todo;
        p.running = todo > 0;
    }
    let wanted: Vec<i64> = std::mem::take(&mut *PRIORITY.lock().unwrap());
    let from = format!("FROM asset a JOIN asset_pack p ON p.id = a.pack_id WHERE {}", todo_where());
    let mut rows: Vec<Row> = if wanted.is_empty() {
        vec![]
    } else {
        let ids = wanted.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        sqlx::query_as(&format!("SELECT {ROW_COLUMNS} {from} AND a.id IN ({ids})")).fetch_all(db).await?
    };
    if rows.is_empty() {
        // Pictures with a preview first (most to learn), then sounds, then names.
        rows = sqlx::query_as(&format!(
            "SELECT {ROW_COLUMNS} {from} ORDER BY
               CASE WHEN a.preview_kind = 'image' AND a.preview_state = 'ok' THEN 0
                    WHEN a.category IN ('sound-effect', 'music', 'ambience', 'voice') THEN 1 ELSE 2 END,
               a.pack_id, a.path LIMIT {NAME_BATCH}"
        ))
        .fetch_all(db)
        .await?;
    }
    let Some(first) = rows.first() else { return Ok(0) };
    let picture = first.preview_kind.as_deref() == Some("image") && first.preview_state.as_deref() == Some("ok");
    let sound = SOUND_CATEGORIES.contains(&first.category.as_str());
    if picture {
        let r = rows.swap_remove(0);
        let done = match tag_picture(db, ai, previews, &r).await {
            Ok(()) => 1,
            Err(e) => {
                fail(db, &[r.id], &e).await?;
                0
            }
        };
        PROGRESS.lock().unwrap().done += done;
        publish(bus, vec![r.id]);
        return Ok(1);
    }
    let batch: Vec<Row> = rows
        .into_iter()
        .filter(|r| SOUND_CATEGORIES.contains(&r.category.as_str()) == sound && r.preview_kind.as_deref() != Some("image"))
        .take(if sound { SOUND_BATCH } else { NAME_BATCH })
        .collect();
    let ids: Vec<i64> = batch.iter().map(|r| r.id).collect();
    if let Err(e) = tag_batch(db, ai, previews, batch, sound).await {
        fail(db, &ids, &e).await?;
    }
    publish(bus, ids.clone());
    Ok(ids.len())
}

async fn tag_picture(db: &SqlitePool, ai: &Ai, previews: &Path, r: &Row) -> Result<()> {
    // The vision model takes PNG or JPEG, not WebP: a 512 px PNG from the 1024 px preview.
    let webp = preview::file(previews, r.id, 'l');
    let out = tokio::process::Command::new("nice")
        .args(["-n", "19", "ffmpeg", "-v", "error", "-nostdin", "-i"])
        .arg(&webp)
        .args(["-vf", "scale=w='min(512,iw)':h='min(512,ih)':force_original_aspect_ratio=decrease", "-frames:v", "1", "-f", "image2pipe", "-c:v", "png", "pipe:1"])
        .stdin(Stdio::null())
        .output()
        .await?;
    if !out.status.success() || out.stdout.is_empty() {
        bail!("couldn't make a PNG of the preview");
    }
    use base64::Engine;
    let png = base64::engine::general_purpose::STANDARD.encode(&out.stdout);
    let item = Item { id: r.id, name: &r.name, path: &r.path, pack: &r.pack, category: &r.category, facts: facts(r) };
    let answer = ai
        .chat(
            json!([
                { "type": "text", "text": picture_prompt(&item) },
                { "type": "image_url", "image_url": { "url": format!("data:image/png;base64,{png}") } },
            ]),
            300,
        )
        .await?;
    let v = json_in(&answer).context("the answer had no JSON")?;
    let t = check(&v, STYLES_VISUAL, PICTURE_CATEGORIES);
    save(db, r.id, &r.category, &t, &ai.model).await?;
    embed_one(db, ai, r, &t).await
}

async fn tag_batch(db: &SqlitePool, ai: &Ai, previews: &Path, mut batch: Vec<Row>, sound: bool) -> Result<()> {
    // Voice lines are written down first (Whisper), so the words help tagging and search.
    for r in batch.iter_mut().filter(|r| r.category == "voice" && r.ai_transcript.is_none() && r.preview_state.as_deref() == Some("ok")) {
        if let Ok(t) = transcript(ai, previews, r.id).await {
            sqlx::query("UPDATE asset SET ai_transcript = ? WHERE id = ?").bind(&t).bind(r.id).execute(db).await?;
            r.ai_transcript = Some(t);
        }
    }
    let items: Vec<Item> = batch
        .iter()
        .map(|r| Item { id: r.id, name: &r.name, path: &r.path, pack: &r.pack, category: &r.category, facts: facts(r) })
        .collect();
    let (styles, cats) = if sound { (STYLES_AUDIO, SOUND_CATEGORIES) } else { (STYLES_VISUAL, NAME_CATEGORIES) };
    let answer = ai.chat(json!(batch_prompt(&items, sound)), 220 * items.len() as u32).await?;
    let v = json_in(&answer).context("the answer had no JSON")?;
    let ids: Vec<i64> = batch.iter().map(|r| r.id).collect();
    let checked = check_batch(&v, &ids, styles, cats);
    let mut missing = vec![];
    for (r, t) in batch.iter().zip(&checked) {
        match t {
            Some(t) => {
                save(db, r.id, &r.category, t, &ai.model).await?;
                if let Err(e) = embed_one(db, ai, r, t).await {
                    tracing::warn!(error = ?e, "asset embedding");
                }
                PROGRESS.lock().unwrap().done += 1;
            }
            None => missing.push(r.id),
        }
    }
    if !missing.is_empty() {
        fail(db, &missing, &anyhow::anyhow!("left out of the batch answer")).await?;
    }
    Ok(())
}

async fn embed_one(db: &SqlitePool, ai: &Ai, r: &Row, t: &Tagged) -> Result<()> {
    let text = embed_text(&r.name, &r.pack, &r.category, t, r.ai_transcript.as_deref());
    let v = ai.embed(&[text]).await?.pop().context("no embedding")?;
    store_vector(db, r.id, &v).await
}

/// 16 kHz mono WAV of the preview clip, written down by Whisper.
async fn transcript(ai: &Ai, previews: &Path, id: i64) -> Result<String> {
    let out = tokio::process::Command::new("nice")
        .args(["-n", "19", "ffmpeg", "-v", "error", "-nostdin", "-i"])
        .arg(preview::file(previews, id, 'a'))
        .args(["-ac", "1", "-ar", "16000", "-f", "wav", "pipe:1"])
        .stdin(Stdio::null())
        .output()
        .await?;
    if !out.status.success() {
        bail!("couldn't read the clip");
    }
    ai.transcribe(out.stdout).await
}

/// Embeds a search phrase (for "by meaning"); None when AI search isn't set up.
pub async fn embed_query(ai: &Ai, q: &str) -> Option<Vec<f32>> {
    ai.embed(&[q.to_string()]).await.ok()?.pop()
}

/// For the category review: the categories a person may pick.
pub fn is_category(c: &str) -> bool {
    classify::CATEGORIES.contains(&c) && c != "junk"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn assets_settings_fall_back_to_the_worker_role() {
        let none: crate::config::Config = toml::from_str(
            "[[provider]]\nid = \"local\"\nname = \"L\"\nkind = \"openai-compatible\"\nbase_url = \"http://m:1/v1\"\napi_key_env = \"K\"\n[roles]\nworker = { provider = \"local\", model = \"small\" }\n",
        ).unwrap();
        let s = settings_from(&none);
        assert_eq!(s.chat_url.as_deref(), Some("http://m:1/v1"));
        assert_eq!(s.model.as_deref(), Some("small"));
        assert_eq!(s.api_key_env.as_deref(), Some("K"));
        let set: crate::config::Config = toml::from_str(
            "[roles]\nworker = { provider = \"x\", model = \"small\" }\n[assets]\nchat_url = \"http://a:2/v3\"\nmodel = \"Tagger\"\n",
        ).unwrap();
        let s = settings_from(&set);
        assert_eq!(s.chat_url.as_deref(), Some("http://a:2/v3"));
        assert_eq!(s.model.as_deref(), Some("Tagger"));
    }

    #[test]
    fn answers_are_checked_against_the_lists() {
        let v = json_in("Sure!\n```json\n{\"category\": \"Sprite\", \"style\": [\"Pixel Art\", \"vaporwave\", \"flat\", \"cartoon\"],\
            \"mood\": \"calm, epic\", \"setting\": [\"ui\"], \"subject\": \"health icon\", \"caption\": \"A red cross health icon.\"}\n```")
            .unwrap();
        let t = check(&v, STYLES_VISUAL, PICTURE_CATEGORIES);
        assert_eq!(t.category.as_deref(), Some("sprite"));
        assert_eq!(t.style, ["pixel-art", "flat"], "unknown words dropped, at most 2");
        assert_eq!(t.mood, ["calm", "epic"], "a comma list is accepted too");
        assert_eq!(t.subject.as_deref(), Some("health icon"));
        let bad = check(&json!({"category": "music", "style": "orchestral"}), STYLES_VISUAL, PICTURE_CATEGORIES);
        assert_eq!(bad.category, None, "a category outside the prompt's list is dropped");
        assert!(bad.style.is_empty());
        // Qwen3.5-9B on the A770 answers the subject as a list (seen in the first real batch).
        let listed = check(&json!({"subject": ["key", "keyboard", "icon"]}), STYLES_VISUAL, PICTURE_CATEGORIES);
        assert_eq!(listed.subject.as_deref(), Some("key keyboard icon"));
    }

    #[test]
    fn batch_answers_match_by_id() {
        let v = json_in(r#"[{"id": 9, "category": "music"}, {"id": 7, "category": "sound-effect", "mood": ["tense"]}]"#).unwrap();
        let got = check_batch(&v, &[7, 8, 9], STYLES_AUDIO, SOUND_CATEGORIES);
        assert_eq!(got[0].as_ref().unwrap().mood, ["tense"]);
        assert!(got[1].is_none(), "8 was left out");
        assert_eq!(got[2].as_ref().unwrap().category.as_deref(), Some("music"));
        // Without ids, the order counts.
        let v = json_in(r#"[{"category": "voice"}]"#).unwrap();
        assert_eq!(check_batch(&v, &[3], STYLES_AUDIO, SOUND_CATEGORIES)[0].as_ref().unwrap().category.as_deref(), Some("voice"));
        assert!(json_in("no json here").is_none());
    }

    #[test]
    fn prompts_carry_the_lists_and_the_facts() {
        let a = Item { id: 4, name: "Action 1.mp3", path: "Tracks/Action 1.mp3", pack: "Fantasy RPG Music Pack", category: "music", facts: "93.0 s long".into() };
        let p = batch_prompt(&[a], true);
        assert!(p.contains("\"id\": 4") && p.contains("93.0 s long") && p.contains("synthwave") && p.contains("Never guess a licence"));
        assert!(!p.contains("pixel-art"), "sound prompts use the sound styles");
    }

    #[test]
    fn night_window_and_vectors() {
        assert!(night(23) && night(0) && night(5) && !night(6) && !night(12) && !night(22));
        let v = shorten(&[3.0, 4.0]);
        assert!((v[0] - 0.6).abs() < 1e-6 && (v[1] - 0.8).abs() < 1e-6);
        assert_eq!(shorten(&vec![1.0; 1024]).len(), DIMS);
        let t = Tagged { caption: Some("A mossy rock.".into()), style: vec!["low-poly".into()], ..Default::default() };
        assert_eq!(embed_text("SM_Rock_01.fbx", "POLYGON_Nature", "3d-model", &t, None), "SM_Rock_01.fbx (3d-model, pack POLYGON_Nature). A mossy rock.. low-poly");
    }

    #[tokio::test]
    async fn vectors_find_the_nearest() {
        register_sqlite_vec();
        let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        ensure_vectors(&db).await.unwrap();
        let mut a = vec![0.0f32; DIMS];
        a[0] = 1.0;
        let mut b = vec![0.0f32; DIMS];
        b[1] = 1.0;
        let mut c = vec![0.0f32; DIMS];
        c[0] = 0.9;
        c[1] = 0.1;
        store_vector(&db, 1, &shorten(&a)).await.unwrap();
        store_vector(&db, 2, &shorten(&b)).await.unwrap();
        store_vector(&db, 3, &shorten(&c)).await.unwrap();
        store_vector(&db, 3, &shorten(&c)).await.unwrap(); // a second write replaces the first
        let near = nearest(&db, &shorten(&a), 2).await.unwrap();
        assert_eq!(near.iter().map(|n| n.0).collect::<Vec<_>>(), [1, 3]);
        assert_eq!(vector_of(&db, 2).await.unwrap().unwrap().len(), DIMS);
    }

    /// The worker against a fake OVMS: a sound batch and a name batch are tagged, checked,
    /// saved with tags, a disagreeing category goes to review, and vectors are stored.
    #[tokio::test]
    async fn worker_tags_against_a_fake_ovms() {
        use axum::{Json, Router, routing::post};
        register_sqlite_vec();
        let app = Router::new()
            .route("/v3/chat/completions", post(|Json(b): Json<Value>| async move {
                let prompt = b["messages"][0]["content"].as_str().unwrap_or_default().to_string();
                let ids: Vec<i64> = prompt.match_indices("{\"id\": ").map(|(i, _)| prompt[i + 7..].split(',').next().unwrap().parse().unwrap()).collect();
                let answer: Vec<Value> = ids.iter().map(|id| if prompt.contains("sound files") {
                    json!({ "id": id, "category": "music", "style": ["orchestral", "dubstep"], "mood": ["epic"], "subject": "battle theme", "caption": "An epic orchestral battle track." })
                } else {
                    json!({ "id": id, "category": "3d-model", "style": ["low-poly"], "setting": ["nature"], "subject": "rock", "caption": "A low-poly rock." })
                }).collect();
                Json(json!({ "choices": [{ "message": { "content": format!("```json\n{}\n```", Value::from(answer)) } }] }))
            }))
            .route("/v3/embeddings", post(|Json(b): Json<Value>| async move {
                let n = b["input"].as_array().map(|a| a.len()).unwrap_or(1);
                Json(json!({ "data": (0..n).map(|_| json!({ "embedding": vec![0.5f32; 1024] })).collect::<Vec<_>>() }))
            }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v3", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });

        let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!().run(&db).await.unwrap();
        ensure_vectors(&db).await.unwrap();
        sqlx::query("INSERT INTO asset_pack (id, key, name, kind) VALUES (1, 'Medieval Vol. 2.zip', 'Medieval Vol. 2', 'zip')").execute(&db).await.unwrap();
        for (id, path, cat) in [(1, "wav/Medieval 1.wav", "sound-effect"), (2, "wav/Medieval 2.wav", "sound-effect"), (3, "Models/SM_Rock.fbx", "3d-model")] {
            sqlx::query("INSERT INTO asset (id, pack_id, container, path, name, ext, size, category, rule, duration_s) VALUES (?, 1, 'Medieval Vol. 2.zip', ?, ?, 'x', 10, ?, 'test', 95.0)")
                .bind(id).bind(path).bind(path.rsplit('/').next().unwrap()).bind(cat).execute(&db).await.unwrap();
        }
        let ai = Ai { http: reqwest::Client::new(), chat_url: url.clone(), embed_url: url, key: None, model: "Fake".into(), embed_model: "Fake".into(), rerank_model: "Fake".into(), audio_model: "Fake".into() };
        let bus = Bus::new();
        let dir = std::env::temp_dir();
        assert_eq!(step(&db, &bus, &ai, &dir).await.unwrap(), 2, "the two sounds go in one batch");
        assert_eq!(step(&db, &bus, &ai, &dir).await.unwrap(), 1, "then the model, by name");
        assert_eq!(step(&db, &bus, &ai, &dir).await.unwrap(), 0, "nothing left");

        let rows: Vec<(i64, String, Option<String>, Option<String>)> =
            sqlx::query_as("SELECT id, ai_state, ai_category, ai_caption FROM asset ORDER BY id").fetch_all(&db).await.unwrap();
        assert_eq!(rows[0].1, "ok");
        assert_eq!(rows[0].2.as_deref(), Some("music"), "the AI hears music where the rule said sound effect: review");
        assert_eq!(rows[2].2, None, "agreeing categories are not flagged");
        let tags: Vec<(String, String, String)> = sqlx::query_as(
            "SELECT t.kind, t.name, x.by FROM asset_tag x JOIN asset_tagname t ON t.id = x.tag_id WHERE x.asset_id = 1 ORDER BY t.kind, t.name",
        ).fetch_all(&db).await.unwrap();
        assert_eq!(tags, [("mood".into(), "epic".into(), "ai".into()), ("style".into(), "orchestral".into(), "ai".into())], "\"dubstep\" isn't on the list");
        let hits: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM asset_fts WHERE asset_fts MATCH 'orchestral'").fetch_one(&db).await.unwrap();
        assert_eq!(hits.0, 2, "AI words are searchable");
        assert_eq!(nearest(&db, &shorten(&[0.5; 1024]), 5).await.unwrap().len(), 3, "every tagged asset has a vector");
        assert_eq!(mode(&db).await, Mode::Off, "off unless an admin turns it on");
    }
}
