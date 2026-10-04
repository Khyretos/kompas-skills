//! Per-game picks (milestone 4). A game has a profile (genre, art style, setting, sold or
//! not) and a needs list ("footsteps on grass"). For each need the library is searched by
//! words (FTS5) and by meaning (sqlite-vec), the hits are ordered by `Reranker` (ovms-cpu),
//! and `Coder` (A770) picks from those candidates only, with a reason each. An id the
//! model makes up is dropped. Kees keeps a pick as a candidate or rejects it; a rejected
//! asset is never suggested to that game again.
//!
//! Games come from the game repos in `GAME_REPOS` (mounted read-only; kk-engine's
//! `games/<name>/game.json`) or are added by hand. Licences are linked to packs by an
//! admin, never guessed: a game that will be sold only gets packs whose licence allows
//! it, and picks from packs with no licence linked say so.

use std::{
    collections::{HashMap, HashSet},
    path::{Path as FsPath, PathBuf},
    sync::{LazyLock, Mutex},
};

use anyhow::{Context, Result, bail};
use axum::{
    Extension, Json, Router,
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post, put},
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tokio::sync::Semaphore;

use super::{ITEM_COLUMNS, Item, ai, classify, item_json, require_admin};
use crate::{
    AppState,
    auth::User,
    error::{ApiError, ApiResult},
    events::{Bus, Event},
    util,
};

/// Candidates handed to the Reranker, and how many of its best go to the Coder.
const POOL: usize = 60;
const SHORTLIST: usize = 12;
const MAX_PICKS: usize = 4;
const MAX_NEEDS: usize = 30;

/// Styles that don't sit well together in one game (from the AI's style tags).
const STYLIZED: &[&str] = &["low-poly", "pixel-art", "hand-painted", "stylized", "cartoon", "flat", "line-art", "painterly"];
const REALISTIC: &[&str] = &["realistic", "photo"];

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/assets/games", get(list_games).post(add_game))
        .route("/assets/games/{id}", get(game_detail).put(set_profile).delete(remove_game))
        .route("/assets/games/{id}/draft", post(draft))
        .route("/assets/games/{id}/needs", post(add_need))
        .route("/assets/games/{id}/pick", post(pick_all))
        .route("/assets/needs/{id}", put(edit_need).delete(remove_need))
        .route("/assets/needs/{id}/pick", post(pick_need))
        .route("/assets/needs/{id}/picks/{asset}", put(set_pick).delete(remove_pick))
        .route("/assets/licences", get(list_licences).post(add_licence))
        .route("/assets/licences/{id}", put(edit_licence).delete(remove_licence))
        .route("/assets/packs/{id}/licence", put(link_licence))
}

// ---------- games from the repos ----------

/// Game repos from `GAME_REPOS` (comma separated, mounted read-only).
pub fn repos() -> Vec<PathBuf> {
    std::env::var("GAME_REPOS")
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect()
}

pub fn repo_name(repo: &FsPath) -> String {
    repo.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "repo".into())
}

/// A game found in a repo: (key, name, folder inside the repo, its own description).
#[derive(Debug, PartialEq)]
pub struct Found {
    pub key: String,
    pub name: String,
    pub path: String,
    pub about: String,
}

/// `games/<dir>/game.json` in a kk-engine style repo. The template isn't a game.
pub fn find_games(repo: &FsPath) -> Vec<Found> {
    let name = repo_name(repo);
    let Ok(rd) = std::fs::read_dir(repo.join("games")) else { return vec![] };
    let mut out: Vec<Found> = rd
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter_map(|e| {
            let dir = e.file_name().to_string_lossy().into_owned();
            if dir == "template" || dir.starts_with('.') {
                return None;
            }
            let meta: Value = serde_json::from_str(&std::fs::read_to_string(e.path().join("game.json")).ok()?).ok()?;
            Some(Found {
                key: format!("{name}/{dir}"),
                name: meta["title"].as_str().filter(|t| !t.trim().is_empty()).unwrap_or(&dir).trim().to_string(),
                path: format!("games/{dir}"),
                about: meta["description"].as_str().unwrap_or_default().chars().take(600).collect(),
            })
        })
        .collect();
    // Scenes outside a game folder (kk-engine's scenes/) count as the repo's shared scenes.
    if super::scenes::scene_files(repo).iter().any(|s| !s.starts_with("games/")) {
        out.push(Found {
            key: format!("{name}/scenes"),
            name: format!("{name} scenes"),
            path: "scenes".into(),
            about: "Scenes outside a game folder.".into(),
        });
    }
    out.sort_by(|a, b| a.key.cmp(&b.key));
    out
}

/// Brings the game list in line with the repos. Never deletes: a game gone from its repo
/// gets `missing_since` and keeps its profile and picks.
pub async fn discover(db: &SqlitePool) -> Result<usize> {
    let mut seen = vec![];
    for repo in repos() {
        if !repo.is_dir() {
            tracing::warn!(path = %repo.display(), "game repo not mounted");
            // A repo that is gone for now must not mark its games missing.
            seen.push(format!("{}/%", repo_name(&repo)));
            continue;
        }
        let source = repo_name(&repo);
        for g in find_games(&repo) {
            sqlx::query(
                "INSERT INTO asset_game (key, name, source, path, about, created_at) VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT(key) DO UPDATE SET name = excluded.name, path = excluded.path, about = excluded.about, missing_since = NULL",
            )
            .bind(&g.key)
            .bind(&g.name)
            .bind(&source)
            .bind(&g.path)
            .bind(&g.about)
            .bind(util::now())
            .execute(db)
            .await?;
            seen.push(g.key);
        }
    }
    // Repo games no longer found (games added by hand are 'own').
    let rows: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, key FROM asset_game WHERE source <> 'own' AND missing_since IS NULL").fetch_all(db).await?;
    for (id, key) in rows {
        let kept = seen.iter().any(|k| k == &key || k.strip_suffix('%').is_some_and(|p| key.starts_with(p)));
        if !kept {
            sqlx::query("UPDATE asset_game SET missing_since = ? WHERE id = ?").bind(util::now()).bind(id).execute(db).await?;
        }
    }
    Ok(seen.len())
}

/// The game's own docs for the AI's profile draft: game.json's description and README.md.
fn game_docs(source: &str, path: Option<&str>) -> String {
    let Some(path) = path else { return String::new() };
    let Some(repo) = repos().into_iter().find(|r| repo_name(r) == source) else { return String::new() };
    let dir = repo.join(path);
    // Stay inside the repo (the path comes from the database, but be strict).
    if !dir.starts_with(&repo) || path.contains("..") {
        return String::new();
    }
    let readme = ["README.md", "readme.md", "DESIGN.md"]
        .iter()
        .find_map(|f| std::fs::read_to_string(dir.join(f)).ok())
        .unwrap_or_default();
    readme.chars().take(6000).collect()
}

// ---------- live updates ----------

/// Picking and drafting state per need / game, for the page while it waits.
static BUSY: LazyLock<Mutex<HashMap<String, &'static str>>> = LazyLock::new(Default::default);
/// One Coder request at a time from this feature (the A770 is shared).
static CODER: LazyLock<Semaphore> = LazyLock::new(|| Semaphore::new(1));

fn busy_set(key: String, state: Option<&'static str>) {
    let mut b = BUSY.lock().unwrap();
    match state {
        Some(s) => b.insert(key, s),
        None => b.remove(&key),
    };
}

fn publish(bus: &Bus, game: i64) {
    bus.send_all(Event::Assets { scan: None, previews: None, ai: None, games: Some(json!({ "game": game })) });
}

// ---------- licences ----------

#[derive(Deserialize)]
pub struct LicenceIn {
    name: String,
    #[serde(default)]
    commercial: bool,
    #[serde(default)]
    attribution: bool,
    url: Option<String>,
    notes: Option<String>,
}

fn licence_json((id, name, commercial, attribution, url, notes, packs): (i64, String, bool, bool, Option<String>, Option<String>, i64)) -> Value {
    json!({ "id": id, "name": name, "commercial": commercial, "attribution": attribution, "url": url, "notes": notes, "packs": packs })
}

async fn list_licences(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let rows: Vec<(i64, String, bool, bool, Option<String>, Option<String>, i64)> = sqlx::query_as(
        "SELECT l.id, l.name, l.commercial, l.attribution, l.url, l.notes,
                (SELECT COUNT(*) FROM asset_pack p WHERE p.licence_id = l.id) FROM asset_licence l ORDER BY l.name COLLATE NOCASE",
    )
    .fetch_all(&s.db)
    .await?;
    Ok(Json(Value::from(rows.into_iter().map(licence_json).collect::<Vec<_>>())))
}

fn check_licence(l: &LicenceIn) -> ApiResult<(String, Option<String>, Option<String>)> {
    let name = l.name.trim().to_string();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(ApiError::BadRequest("Give the licence a name (up to 80 characters).".into()));
    }
    let url = l.url.as_deref().map(str::trim).filter(|u| !u.is_empty()).map(String::from);
    if url.as_deref().is_some_and(|u| !(u.starts_with("https://") || u.starts_with("http://"))) {
        return Err(ApiError::BadRequest("The licence link must start with https://.".into()));
    }
    let notes = l.notes.as_deref().map(str::trim).filter(|n| !n.is_empty()).map(|n| n.chars().take(500).collect());
    Ok((name, url, notes))
}

async fn add_licence(State(s): State<AppState>, Extension(u): Extension<User>, Json(l): Json<LicenceIn>) -> ApiResult<Json<Value>> {
    require_admin(&s, &u).await?;
    let (name, url, notes) = check_licence(&l)?;
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO asset_licence (name, commercial, attribution, url, notes) VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(&name)
    .bind(l.commercial)
    .bind(l.attribution)
    .bind(&url)
    .bind(&notes)
    .fetch_one(&s.db)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(d) if d.is_unique_violation() => ApiError::BadRequest("A licence with that name exists.".into()),
        e => e.into(),
    })?;
    super::changed(&s, 0);
    Ok(Json(licence_json((id, name, l.commercial, l.attribution, url, notes, 0))))
}

async fn edit_licence(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<i64>,
    Json(l): Json<LicenceIn>,
) -> ApiResult<StatusCode> {
    require_admin(&s, &u).await?;
    let (name, url, notes) = check_licence(&l)?;
    let n = sqlx::query("UPDATE asset_licence SET name = ?, commercial = ?, attribution = ?, url = ?, notes = ? WHERE id = ?")
        .bind(name)
        .bind(l.commercial)
        .bind(l.attribution)
        .bind(url)
        .bind(notes)
        .bind(id)
        .execute(&s.db)
        .await?
        .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    super::changed(&s, 0);
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_licence(State(s): State<AppState>, Extension(u): Extension<User>, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    require_admin(&s, &u).await?;
    sqlx::query("UPDATE asset_pack SET licence_id = NULL WHERE licence_id = ?").bind(id).execute(&s.db).await?;
    sqlx::query("DELETE FROM asset_licence WHERE id = ?").bind(id).execute(&s.db).await?;
    super::changed(&s, 0);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct LinkIn {
    licence: Option<i64>,
}

/// Links a pack to a licence (or unlinks it). Admin only: this is Kees's call.
async fn link_licence(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(pack): Path<i64>,
    Json(b): Json<LinkIn>,
) -> ApiResult<StatusCode> {
    require_admin(&s, &u).await?;
    if let Some(l) = b.licence {
        let exists: Option<(i64,)> = sqlx::query_as("SELECT id FROM asset_licence WHERE id = ?").bind(l).fetch_optional(&s.db).await?;
        if exists.is_none() {
            return Err(ApiError::BadRequest("That licence doesn't exist.".into()));
        }
    }
    let n = sqlx::query("UPDATE asset_pack SET licence_id = ? WHERE id = ?").bind(b.licence).bind(pack).execute(&s.db).await?.rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    tracing::info!(pack, licence = ?b.licence, by = %u.name, "asset pack licence");
    super::changed(&s, 0);
    Ok(StatusCode::NO_CONTENT)
}

/// The licence of an asset's pack, for cards and details.
pub async fn licence_of_pack(db: &SqlitePool, pack: i64) -> Result<Option<Value>> {
    let row: Option<(i64, String, bool, bool, Option<String>)> = sqlx::query_as(
        "SELECT l.id, l.name, l.commercial, l.attribution, l.url FROM asset_pack p JOIN asset_licence l ON l.id = p.licence_id WHERE p.id = ?",
    )
    .bind(pack)
    .fetch_optional(db)
    .await?;
    Ok(row.map(|(id, name, commercial, attribution, url)| {
        json!({ "id": id, "name": name, "commercial": commercial, "attribution": attribution, "url": url })
    }))
}

// ---------- games and needs ----------

#[derive(sqlx::FromRow)]
struct Game {
    id: i64,
    key: String,
    name: String,
    source: String,
    path: Option<String>,
    about: String,
    genre: String,
    art_style: String,
    setting: String,
    commercial: bool,
    profile_by: String,
    missing_since: Option<String>,
}

const GAME_COLUMNS: &str = "id, key, name, source, path, about, genre, art_style, setting, commercial, profile_by, missing_since";

fn game_json(g: &Game) -> Value {
    json!({
        "id": g.id, "key": g.key, "name": g.name, "source": g.source, "path": g.path, "about": g.about,
        "genre": g.genre, "artStyle": g.art_style, "setting": g.setting, "commercial": g.commercial,
        "profileBy": g.profile_by, "missingSince": g.missing_since,
        "drafting": BUSY.lock().unwrap().contains_key(&format!("g{}", g.id)),
    })
}

async fn game(db: &SqlitePool, id: i64) -> ApiResult<Game> {
    sqlx::query_as(&format!("SELECT {GAME_COLUMNS} FROM asset_game WHERE id = ?"))
        .bind(id)
        .fetch_optional(db)
        .await?
        .ok_or(ApiError::NotFound)
}

async fn list_games(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let games: Vec<Game> = sqlx::query_as(&format!(
        "SELECT {GAME_COLUMNS} FROM asset_game ORDER BY missing_since IS NOT NULL, source = 'own' DESC, name COLLATE NOCASE"
    ))
    .fetch_all(&s.db)
    .await?;
    let counts: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT n.game_id, COUNT(DISTINCT n.id), COUNT(p.asset_id) FILTER (WHERE p.status = 'candidate')
         FROM asset_need n LEFT JOIN asset_pick p ON p.need_id = n.id GROUP BY n.game_id",
    )
    .fetch_all(&s.db)
    .await?;
    let counts: HashMap<i64, (i64, i64)> = counts.into_iter().map(|(g, n, c)| (g, (n, c))).collect();
    let used: HashMap<i64, i64> =
        sqlx::query_as::<_, (i64, i64)>("SELECT game_id, COUNT(DISTINCT asset_id) FROM asset_use GROUP BY game_id").fetch_all(&s.db).await?.into_iter().collect();
    Ok(Json(Value::from(
        games
            .iter()
            .map(|g| {
                let (needs, kept) = counts.get(&g.id).copied().unwrap_or_default();
                let mut v = game_json(g);
                v["needs"] = needs.into();
                v["candidates"] = kept.into();
                v["used"] = used.get(&g.id).copied().unwrap_or_default().into();
                v
            })
            .collect::<Vec<_>>(),
    )))
}

#[derive(Deserialize)]
pub struct NewGame {
    name: String,
}

async fn add_game(State(s): State<AppState>, Json(b): Json<NewGame>) -> ApiResult<Json<Value>> {
    let name = b.name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(ApiError::BadRequest("Give the game a name (up to 80 characters).".into()));
    }
    let key = format!("own/{}", util::now());
    let id: i64 = sqlx::query_scalar("INSERT INTO asset_game (key, name, source, created_at) VALUES (?, ?, 'own', ?) RETURNING id")
        .bind(&key)
        .bind(name)
        .bind(util::now())
        .fetch_one(&s.db)
        .await?;
    publish(&s.bus, id);
    Ok(Json(game_json(&game(&s.db, id).await?)))
}

/// Only games added by hand can be removed; repo games come back with the next scan.
async fn remove_game(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    let g = game(&s.db, id).await?;
    if g.source != "own" && g.missing_since.is_none() {
        return Err(ApiError::BadRequest("This game comes from its repo; it stays while the repo has it.".into()));
    }
    sqlx::query("DELETE FROM asset_game WHERE id = ?").bind(id).execute(&s.db).await?;
    publish(&s.bus, id);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    #[serde(default)]
    genre: String,
    #[serde(default)]
    art_style: String,
    #[serde(default)]
    setting: String,
    #[serde(default)]
    commercial: bool,
}

fn short(s: &str, max: usize) -> String {
    s.trim().chars().take(max).collect()
}

/// Saving the profile confirms it (an AI draft becomes Kees's).
async fn set_profile(State(s): State<AppState>, Path(id): Path<i64>, Json(p): Json<Profile>) -> ApiResult<StatusCode> {
    game(&s.db, id).await?;
    sqlx::query("UPDATE asset_game SET genre = ?, art_style = ?, setting = ?, commercial = ?, profile_by = 'kees' WHERE id = ?")
        .bind(short(&p.genre, 60))
        .bind(short(&p.art_style, 60))
        .bind(short(&p.setting, 60))
        .bind(p.commercial)
        .bind(id)
        .execute(&s.db)
        .await?;
    sqlx::query("UPDATE asset_need SET by = 'kees' WHERE game_id = ?").bind(id).execute(&s.db).await?;
    publish(&s.bus, id);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct NeedIn {
    text: String,
    category: Option<String>,
}

fn check_need(n: &NeedIn) -> ApiResult<(String, Option<String>)> {
    let text = n.text.trim().to_string();
    if text.is_empty() || text.chars().count() > 120 {
        return Err(ApiError::BadRequest("Say what the game needs in up to 120 characters.".into()));
    }
    let category = n.category.as_deref().map(str::trim).filter(|c| !c.is_empty());
    if let Some(c) = category
        && !ai::is_category(c)
    {
        return Err(ApiError::BadRequest(format!("\"{c}\" isn't a category.")));
    }
    Ok((text, category.map(String::from)))
}

async fn add_need(State(s): State<AppState>, Path(id): Path<i64>, Json(n): Json<NeedIn>) -> ApiResult<Json<Value>> {
    game(&s.db, id).await?;
    let (text, category) = check_need(&n)?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM asset_need WHERE game_id = ?").bind(id).fetch_one(&s.db).await?;
    if count as usize >= MAX_NEEDS {
        return Err(ApiError::BadRequest(format!("A game has at most {MAX_NEEDS} needs.")));
    }
    let need: i64 = sqlx::query_scalar(
        "INSERT INTO asset_need (game_id, text, category, by, position) VALUES (?, ?, ?, 'kees', ?) RETURNING id",
    )
    .bind(id)
    .bind(&text)
    .bind(&category)
    .bind(count)
    .fetch_one(&s.db)
    .await?;
    publish(&s.bus, id);
    Ok(Json(json!({ "id": need, "text": text, "category": category, "by": "kees", "picks": [] })))
}

async fn need_game(db: &SqlitePool, need: i64) -> ApiResult<i64> {
    sqlx::query_scalar("SELECT game_id FROM asset_need WHERE id = ?").bind(need).fetch_optional(db).await?.ok_or(ApiError::NotFound)
}

async fn edit_need(State(s): State<AppState>, Path(id): Path<i64>, Json(n): Json<NeedIn>) -> ApiResult<StatusCode> {
    let g = need_game(&s.db, id).await?;
    let (text, category) = check_need(&n)?;
    sqlx::query("UPDATE asset_need SET text = ?, category = ?, by = 'kees' WHERE id = ?")
        .bind(text)
        .bind(category)
        .bind(id)
        .execute(&s.db)
        .await?;
    publish(&s.bus, g);
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_need(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    let g = need_game(&s.db, id).await?;
    sqlx::query("DELETE FROM asset_need WHERE id = ?").bind(id).execute(&s.db).await?;
    publish(&s.bus, g);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct PickIn {
    status: String,
}

/// Kees keeps a pick as a candidate, rejects it, or (from search) adds one himself.
async fn set_pick(State(s): State<AppState>, Path((need, asset)): Path<(i64, i64)>, Json(b): Json<PickIn>) -> ApiResult<StatusCode> {
    let g = need_game(&s.db, need).await?;
    if !matches!(b.status.as_str(), "candidate" | "rejected") {
        return Err(ApiError::BadRequest("A pick is kept as a candidate or rejected.".into()));
    }
    sqlx::query(
        "INSERT INTO asset_pick (need_id, asset_id, status, by, at) VALUES (?, ?, ?, 'kees', ?)
         ON CONFLICT(need_id, asset_id) DO UPDATE SET status = excluded.status, at = excluded.at",
    )
    .bind(need)
    .bind(asset)
    .bind(&b.status)
    .bind(util::now())
    .execute(&s.db)
    .await?;
    publish(&s.bus, g);
    Ok(StatusCode::NO_CONTENT)
}

async fn remove_pick(State(s): State<AppState>, Path((need, asset)): Path<(i64, i64)>) -> ApiResult<StatusCode> {
    let g = need_game(&s.db, need).await?;
    sqlx::query("DELETE FROM asset_pick WHERE need_id = ? AND asset_id = ?").bind(need).bind(asset).execute(&s.db).await?;
    publish(&s.bus, g);
    Ok(StatusCode::NO_CONTENT)
}

/// A game with its needs, each need's picks (with the asset's card data and licence),
/// and a warning when the kept and suggested picks mix clashing styles.
async fn game_detail(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let g = game(&s.db, id).await?;
    let needs: Vec<(i64, String, Option<String>, String, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT id, text, category, by, picked_at, pick_error FROM asset_need WHERE game_id = ? ORDER BY position, id",
    )
    .bind(id)
    .fetch_all(&s.db)
    .await?;
    let picks: Vec<(i64, i64, String, Option<String>, String)> = sqlx::query_as(
        "SELECT p.need_id, p.asset_id, p.status, p.reason, p.by FROM asset_pick p JOIN asset_need n ON n.id = p.need_id
         WHERE n.game_id = ? ORDER BY p.status = 'rejected', p.status <> 'candidate', p.rank, p.at",
    )
    .bind(id)
    .fetch_all(&s.db)
    .await?;
    let ids: Vec<String> = picks.iter().map(|p| p.1.to_string()).collect::<HashSet<_>>().into_iter().collect();
    let mut items: HashMap<i64, Value> = HashMap::new();
    let mut licences: HashMap<i64, Option<Value>> = HashMap::new();
    if !ids.is_empty() {
        let rows: Vec<Item> = sqlx::query_as(&format!(
            "SELECT {ITEM_COLUMNS} FROM asset a JOIN asset_pack p ON p.id = a.pack_id WHERE a.id IN ({})",
            ids.join(",")
        ))
        .fetch_all(&s.db)
        .await?;
        for r in rows {
            let pack = r.pack_id;
            if let std::collections::hash_map::Entry::Vacant(e) = licences.entry(pack) {
                e.insert(licence_of_pack(&s.db, pack).await?);
            }
            let mut v = item_json(r);
            v["licence"] = licences[&pack].clone().unwrap_or(Value::Null);
            items.insert(v["id"].as_i64().unwrap_or_default(), v);
        }
    }
    let busy = BUSY.lock().unwrap().clone();
    let needs_json: Vec<Value> = needs
        .into_iter()
        .map(|(nid, text, category, by, picked_at, error)| {
            let list: Vec<Value> = picks
                .iter()
                .filter(|p| p.0 == nid)
                .filter_map(|(_, aid, status, reason, by)| {
                    Some(json!({ "asset": items.get(aid)?.clone(), "status": status, "reason": reason, "by": by }))
                })
                .collect();
            json!({
                "id": nid, "text": text, "category": category, "by": by, "pickedAt": picked_at, "error": error,
                "state": busy.get(&format!("n{nid}")).copied(), "picks": list,
            })
        })
        .collect();
    let mut out = game_json(&g);
    out["needs"] = Value::from(needs_json);
    out["styleWarning"] = style_warning(&s.db, id).await?.map(Value::from).unwrap_or(Value::Null);
    out["scenes"] = super::scenes::summary(&s.db, id).await?;
    out["aiOn"] = (ai::mode(&s.db).await != ai::Mode::Off).into();
    Ok(Json(out))
}

/// "Low-poly (POLYGON Nature) next to realistic (Rock Scans)": packs of the game's kept and
/// suggested picks whose AI style tags clash. None when they agree or have no style tags.
pub async fn style_warning(db: &SqlitePool, game: i64) -> Result<Option<String>> {
    let rows: Vec<(String, String)> = sqlx::query_as(
        "SELECT DISTINCT pk.name, t.name FROM asset_pick p
         JOIN asset_need n ON n.id = p.need_id
         JOIN asset a ON a.id = p.asset_id JOIN asset_pack pk ON pk.id = a.pack_id
         JOIN asset_tag x ON x.asset_id = a.id JOIN asset_tagname t ON t.id = x.tag_id AND t.kind = 'style'
         WHERE n.game_id = ? AND p.status IN ('suggested', 'candidate')",
    )
    .bind(game)
    .fetch_all(db)
    .await?;
    let packs_with = |styles: &[&str]| -> Vec<String> {
        let mut v: Vec<String> = rows.iter().filter(|(_, t)| styles.contains(&t.as_str())).map(|(p, _)| p.clone()).collect();
        v.sort();
        v.dedup();
        v
    };
    let (soft, real) = (packs_with(STYLIZED), packs_with(REALISTIC));
    if soft.is_empty() || real.is_empty() {
        return Ok(None);
    }
    Ok(Some(format!(
        "Mixed styles: stylized ({}) next to realistic ({}).",
        soft.into_iter().take(3).collect::<Vec<_>>().join(", "),
        real.into_iter().take(3).collect::<Vec<_>>().join(", ")
    )))
}

// ---------- the AI's profile draft ----------

/// Drafts genre, art style, setting and a needs list from the game's own docs. Runs in the
/// background; a confirmed profile (Kees's) is never overwritten, and needs already on the
/// list are not added twice.
async fn draft(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    let g = game(&s.db, id).await?;
    let key = format!("g{id}");
    if BUSY.lock().unwrap().contains_key(&key) {
        return Ok(StatusCode::ACCEPTED);
    }
    busy_set(key.clone(), Some("drafting"));
    publish(&s.bus, id);
    tokio::spawn(async move {
        let ai = ai::Ai::from_env(s.http.clone());
        let result = async {
            let _turn = CODER.acquire().await?;
            let docs = game_docs(&g.source, g.path.as_deref());
            let answer = ai.chat(Value::from(draft_prompt(&g.name, &g.about, &docs)), 1200).await?;
            let d = check_draft(&answer).context("the model's answer had no usable profile")?;
            save_draft(&s.db, &g, &d).await
        }
        .await;
        if let Err(e) = result {
            tracing::warn!(game = id, error = %e, "asset game draft");
            let msg: String = format!("Draft failed: {e:#}").chars().take(300).collect();
            s.bus.send_all(Event::Assets { scan: None, previews: None, ai: None, games: Some(json!({ "game": id, "error": msg })) });
        }
        busy_set(key, None);
        publish(&s.bus, id);
    });
    Ok(StatusCode::ACCEPTED)
}

pub fn draft_prompt(name: &str, about: &str, docs: &str) -> String {
    let cats = classify::CATEGORIES.iter().filter(|c| ai::is_category(c)).copied().collect::<Vec<_>>().join(", ");
    format!(
        "You help plan which game assets (models, sounds, music, textures) a game needs.\n\n\
         Game: {name}\nDescription: {about}\n\nThe game's README (may be empty):\n<<<\n{docs}\n>>>\n\n\
         Reply with JSON only:\n\
         {{\"genre\": \"...\", \"artStyle\": \"...\", \"setting\": \"...\", \"needs\": [{{\"text\": \"...\", \"category\": \"...\"}}]}}\n\n\
         Rules:\n\
         1. genre, artStyle and setting: a few words each, only what the text supports; \"\" when it doesn't say.\n\
         2. needs: 4 to 12 concrete assets the game needs, each under 80 characters, like \"footsteps on grass\", \
         \"calm ambient forest loop\", \"low-poly pine trees\", \"UI click sound\".\n\
         3. category: one of {cats}, or null when several fit.\n\
         4. Base every need on the description and README. Don't invent features the game doesn't have."
    )
}

#[derive(Debug, Default, PartialEq)]
pub struct Draft {
    pub genre: String,
    pub art_style: String,
    pub setting: String,
    pub needs: Vec<(String, Option<String>)>,
}

/// The model's draft, cut to size; unknown categories become "any".
pub fn check_draft(answer: &str) -> Option<Draft> {
    let v = ai::json_in(answer)?;
    let text = |k: &str| v[k].as_str().map(|s| short(s, 60)).unwrap_or_default();
    let mut seen = HashSet::new();
    let needs: Vec<(String, Option<String>)> = v["needs"]
        .as_array()?
        .iter()
        .filter_map(|n| {
            let t = short(n["text"].as_str().or_else(|| n.as_str())?, 120);
            let c = n["category"].as_str().filter(|c| ai::is_category(c)).map(String::from);
            (!t.is_empty() && seen.insert(t.to_lowercase())).then_some((t, c))
        })
        .take(12)
        .collect();
    Some(Draft { genre: text("genre"), art_style: text("artStyle"), setting: text("setting"), needs })
}

async fn save_draft(db: &SqlitePool, g: &Game, d: &Draft) -> Result<()> {
    let mut tx = db.begin().await?;
    if g.profile_by != "kees" {
        sqlx::query("UPDATE asset_game SET genre = ?, art_style = ?, setting = ?, profile_by = 'ai' WHERE id = ?")
            .bind(&d.genre)
            .bind(&d.art_style)
            .bind(&d.setting)
            .bind(g.id)
            .execute(&mut *tx)
            .await?;
    }
    let have: Vec<(String,)> = sqlx::query_as("SELECT text FROM asset_need WHERE game_id = ?").bind(g.id).fetch_all(&mut *tx).await?;
    let have: HashSet<String> = have.into_iter().map(|(t,)| t.to_lowercase()).collect();
    let mut pos = have.len() as i64;
    for (text, cat) in &d.needs {
        if have.contains(&text.to_lowercase()) || pos as usize >= MAX_NEEDS {
            continue;
        }
        sqlx::query("INSERT INTO asset_need (game_id, text, category, by, position) VALUES (?, ?, ?, 'ai', ?)")
            .bind(g.id)
            .bind(text)
            .bind(cat)
            .bind(pos)
            .execute(&mut *tx)
            .await?;
        pos += 1;
    }
    tx.commit().await?;
    Ok(())
}

// ---------- picks ----------

/// One asset the search found, described for the Reranker and the Coder.
#[derive(sqlx::FromRow, Debug, Clone)]
pub struct Cand {
    pub id: i64,
    pub name: String,
    pub pack: String,
    pub category: String,
    pub caption: Option<String>,
    pub tags: Option<String>,
    pub licence: Option<String>,
}

impl Cand {
    fn text(&self) -> String {
        let mut t = format!("{} ({}) from the pack {}", self.name, self.category.replace('-', " "), self.pack);
        if let Some(c) = &self.caption {
            t.push_str(&format!(". {c}"));
        }
        if let Some(tags) = &self.tags {
            t.push_str(&format!(". Tags: {tags}"));
        }
        t
    }
}

/// Searchable words of a need: no little words, each as a prefix, any of them (OR).
pub fn need_query(text: &str) -> Option<String> {
    const SKIP: &[&str] = &["a", "an", "and", "the", "of", "on", "in", "for", "with", "to", "or", "some", "set", "pack"];
    let words: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|w| w.chars().count() >= 3 && !SKIP.contains(&w.as_str()))
        .take(10)
        .map(|w| format!("\"{w}\"*"))
        .collect();
    (!words.is_empty()).then(|| words.join(" OR "))
}

/// Which assets may be picked for this game and need: not junk, readmes or copies, not
/// rejected for this game, the need's category if it has one, and for a game that will be
/// sold, no pack whose licence rules that out (packs with no licence linked stay, marked).
const PICKABLE: &str = "a.missing_since IS NULL AND a.category <> 'junk' AND a.is_meta = 0 AND a.dup_of IS NULL
     AND (?1 IS NULL OR a.category = ?1)
     AND a.id NOT IN (SELECT p.asset_id FROM asset_pick p JOIN asset_need n ON n.id = p.need_id
                      WHERE n.game_id = ?2 AND p.status = 'rejected')
     AND (?3 = 0 OR NOT EXISTS (SELECT 1 FROM asset_pack pk JOIN asset_licence l ON l.id = pk.licence_id
                                WHERE pk.id = a.pack_id AND l.commercial = 0))";

const CAND_COLUMNS: &str = "a.id, a.name, p.name AS pack, a.category, a.ai_caption AS caption,
     (SELECT group_concat(t.name, ', ') FROM asset_tag x JOIN asset_tagname t ON t.id = x.tag_id WHERE x.asset_id = a.id) AS tags,
     (SELECT l.name FROM asset_licence l WHERE l.id = p.licence_id) AS licence";

/// Search by words and by meaning, alternating, without repeats: at most POOL.
pub async fn candidates(db: &SqlitePool, ai: Option<&ai::Ai>, game: i64, commercial: bool, text: &str, category: Option<&str>) -> Result<Vec<Cand>> {
    let mut by_words: Vec<Cand> = vec![];
    if let Some(q) = need_query(text) {
        by_words = sqlx::query_as(&format!(
            "SELECT {CAND_COLUMNS} FROM asset_fts f JOIN asset a ON a.id = f.rowid JOIN asset_pack p ON p.id = a.pack_id
             WHERE asset_fts MATCH ?4 AND {PICKABLE} ORDER BY bm25(asset_fts) LIMIT {POOL}"
        ))
        .bind(category)
        .bind(game)
        .bind(commercial)
        .bind(q)
        .fetch_all(db)
        .await?;
    }
    let mut by_meaning: Vec<Cand> = vec![];
    let vectors: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM asset WHERE ai_state = 'ok'").fetch_one(db).await?;
    if let Some(ai) = ai
        && vectors > 0
        && let Some(v) = ai::embed_query(ai, text).await
    {
        let near = ai::nearest(db, &v, 400).await.unwrap_or_default();
        if !near.is_empty() {
            let list = format!(",{},", near.iter().map(|(id, _)| id.to_string()).collect::<Vec<_>>().join(","));
            by_meaning = sqlx::query_as(&format!(
                "SELECT {CAND_COLUMNS} FROM asset a JOIN asset_pack p ON p.id = a.pack_id
                 WHERE instr(?4, ',' || a.id || ',') > 0 AND {PICKABLE} ORDER BY instr(?4, ',' || a.id || ',') LIMIT {POOL}"
            ))
            .bind(category)
            .bind(game)
            .bind(commercial)
            .bind(list)
            .fetch_all(db)
            .await?;
        }
    }
    let mut seen = HashSet::new();
    let mut out = vec![];
    let (mut w, mut m) = (by_words.into_iter(), by_meaning.into_iter());
    loop {
        let (a, b) = (w.next(), m.next());
        if a.is_none() && b.is_none() {
            break;
        }
        for c in [a, b].into_iter().flatten() {
            if out.len() < POOL && seen.insert(c.id) {
                out.push(c);
            }
        }
    }
    Ok(out)
}

pub fn pick_prompt(game: &str, profile: &str, need: &str, shortlist: &[Cand]) -> String {
    let lines: Vec<String> = shortlist
        .iter()
        .map(|c| {
            let mut l = format!("- id {}: {} | {} | pack {}", c.id, c.name, c.category, c.pack);
            if let Some(t) = &c.tags {
                l.push_str(&format!(" | {t}"));
            }
            if let Some(cap) = &c.caption {
                l.push_str(&format!(" | \"{cap}\""));
            }
            l
        })
        .collect();
    format!(
        "You pick game assets from a list.\n\nGame: {game}{profile}\nNeed: {need}\n\n\
         Candidates (the only ones you may pick):\n{}\n\n\
         Pick up to {MAX_PICKS} that best fit the need and the game's style, best first. \
         Prefer assets that match each other's style. For each, give one short sentence saying why.\n\
         Reply with JSON only: {{\"picks\": [{{\"id\": 123, \"reason\": \"...\"}}]}}\n\
         Use only ids from the list. If none fit, reply {{\"picks\": []}}.",
        lines.join("\n")
    )
}

/// The model's picks: only ids from the shortlist, each once, at most MAX_PICKS.
pub fn check_picks(answer: &str, shortlist: &[Cand]) -> Option<Vec<(i64, String)>> {
    let v = ai::json_in(answer)?;
    let list = v["picks"].as_array().or_else(|| v.as_array())?;
    let allowed: HashSet<i64> = shortlist.iter().map(|c| c.id).collect();
    let mut seen = HashSet::new();
    Some(
        list.iter()
            .filter_map(|p| {
                let id = p["id"].as_i64().or_else(|| p["id"].as_str()?.trim().parse().ok())?;
                let reason = short(p["reason"].as_str().unwrap_or_default(), 160);
                (allowed.contains(&id) && seen.insert(id)).then(|| (id, if reason.is_empty() { "Fits the need.".into() } else { reason }))
            })
            .take(MAX_PICKS)
            .collect(),
    )
}

/// Finds, ranks and picks assets for one need, replacing its earlier suggestions (kept and
/// rejected picks stay). Returns how many were suggested.
pub async fn pick(db: &SqlitePool, ai: &ai::Ai, need: i64) -> Result<usize> {
    let (game_id, text, category): (i64, String, Option<String>) =
        sqlx::query_as("SELECT game_id, text, category FROM asset_need WHERE id = ?").bind(need).fetch_one(db).await?;
    let g: Game = sqlx::query_as(&format!("SELECT {GAME_COLUMNS} FROM asset_game WHERE id = ?")).bind(game_id).fetch_one(db).await?;
    let mut context = vec![];
    for (label, v) in [("genre", &g.genre), ("art style", &g.art_style), ("setting", &g.setting)] {
        if !v.is_empty() {
            context.push(format!("{label}: {v}"));
        }
    }
    let profile = if context.is_empty() { String::new() } else { format!(" ({})", context.join("; ")) };
    let pool = candidates(db, Some(ai), game_id, g.commercial, &text, category.as_deref()).await?;
    if pool.is_empty() {
        save_picks(db, need, &[]).await?;
        return Ok(0);
    }
    let query = format!("{text}, for {}{profile}", g.name);
    let docs: Vec<String> = pool.iter().map(Cand::text).collect();
    // Without the Reranker, the search order stands.
    let order: Vec<usize> = match ai.rerank(&query, &docs).await {
        Ok(r) if !r.is_empty() => r.into_iter().map(|(i, _)| i).collect(),
        Ok(_) => (0..pool.len()).collect(),
        Err(e) => {
            tracing::warn!(error = %e, "asset picks: reranker");
            (0..pool.len()).collect()
        }
    };
    let shortlist: Vec<Cand> = order.into_iter().take(SHORTLIST).map(|i| pool[i].clone()).collect();
    let answer = {
        let _turn = CODER.acquire().await?;
        ai.chat(Value::from(pick_prompt(&g.name, &profile, &text, &shortlist)), 700).await?
    };
    let Some(picks) = check_picks(&answer, &shortlist) else { bail!("the model's answer had no picks list") };
    save_picks(db, need, &picks).await?;
    Ok(picks.len())
}

async fn save_picks(db: &SqlitePool, need: i64, picks: &[(i64, String)]) -> Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM asset_pick WHERE need_id = ? AND status = 'suggested'").bind(need).execute(&mut *tx).await?;
    for (rank, (id, reason)) in picks.iter().enumerate() {
        sqlx::query("INSERT OR IGNORE INTO asset_pick (need_id, asset_id, status, reason, rank, by, at) VALUES (?, ?, 'suggested', ?, ?, 'ai', ?)")
            .bind(need)
            .bind(id)
            .bind(reason)
            .bind(rank as i64)
            .bind(util::now())
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("UPDATE asset_need SET picked_at = ?, pick_error = NULL WHERE id = ?").bind(util::now()).bind(need).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

/// Queues picking for these needs (one after the other) and returns at once.
fn start_picking(s: AppState, game: i64, needs: Vec<i64>) {
    let needs: Vec<i64> = {
        let mut b = BUSY.lock().unwrap();
        needs.into_iter().filter(|n| b.insert(format!("n{n}"), "queued").is_none()).collect()
    };
    if needs.is_empty() {
        return;
    }
    publish(&s.bus, game);
    tokio::spawn(async move {
        let ai = ai::Ai::from_env(s.http.clone());
        for n in needs {
            busy_set(format!("n{n}"), Some("picking"));
            publish(&s.bus, game);
            if let Err(e) = pick(&s.db, &ai, n).await {
                tracing::warn!(need = n, error = %e, "asset picks");
                let msg: String = format!("{e:#}").chars().take(300).collect();
                let _ = sqlx::query("UPDATE asset_need SET pick_error = ? WHERE id = ?").bind(msg).bind(n).execute(&s.db).await;
            }
            busy_set(format!("n{n}"), None);
            publish(&s.bus, game);
        }
    });
}

async fn pick_need(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    let g = need_game(&s.db, id).await?;
    start_picking(s, g, vec![id]);
    Ok(StatusCode::ACCEPTED)
}

/// Picks for every need of the game that has no suggestions or kept picks yet.
async fn pick_all(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    game(&s.db, id).await?;
    let needs: Vec<(i64,)> = sqlx::query_as(
        "SELECT n.id FROM asset_need n WHERE n.game_id = ?
         AND NOT EXISTS (SELECT 1 FROM asset_pick p WHERE p.need_id = n.id AND p.status IN ('suggested', 'candidate'))
         ORDER BY n.position, n.id",
    )
    .bind(id)
    .fetch_all(&s.db)
    .await?;
    if needs.is_empty() {
        return Err(ApiError::BadRequest("Every need already has picks. Use a need's own button to pick again.".into()));
    }
    start_picking(s, id, needs.into_iter().map(|(n,)| n).collect());
    Ok(StatusCode::ACCEPTED)
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn db() -> SqlitePool {
        ai::register_sqlite_vec();
        let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!().run(&db).await.unwrap();
        ai::ensure_vectors(&db).await.unwrap();
        sqlx::query("INSERT INTO asset_licence (id, name, commercial) VALUES (1, 'Free for games', 1), (2, 'Personal use only', 0)")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO asset_pack (id, key, name, kind, licence_id) VALUES
             (1, 'Nature.zip', 'POLYGON Nature', 'zip', 1), (2, 'Steps.zip', 'Footsteps Pro', 'zip', NULL), (3, 'Private.zip', 'Private Sounds', 'zip', 2)",
        )
        .execute(&db)
        .await
        .unwrap();
        for (id, pack, name, cat) in [
            (1, 1, "SM_Tree_Pine_01.fbx", "3d-model"),
            (2, 1, "SM_Rock_01.fbx", "3d-model"),
            (3, 2, "Footstep_Grass_01.wav", "sound-effect"),
            (4, 2, "Footstep_Stone_01.wav", "sound-effect"),
            (5, 3, "Footstep_Grass_Soft.wav", "sound-effect"),
            (6, 2, "Readme_Grass.txt", "doc"),
        ] {
            sqlx::query("INSERT INTO asset (id, pack_id, container, path, name, ext, size, category, rule, is_meta) VALUES (?, ?, 'x.zip', ?, ?, 'x', 10, ?, 'test', ?)")
                .bind(id)
                .bind(pack)
                .bind(name)
                .bind(name)
                .bind(cat)
                .bind(cat == "doc")
                .execute(&db)
                .await
                .unwrap();
        }
        sqlx::query(
            "INSERT INTO asset_fts (rowid, name, path, pack, category, ai)
             SELECT a.id, a.name, a.path, p.name, a.category, '' FROM asset a JOIN asset_pack p ON p.id = a.pack_id",
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query("INSERT INTO asset_game (id, key, name, source, commercial, created_at) VALUES (1, 'own/1', 'Forest Walk', 'own', 1, 'now')")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO asset_need (id, game_id, text) VALUES (1, 1, 'footsteps on grass')").execute(&db).await.unwrap();
        db
    }

    #[test]
    fn need_words_are_any_of_them() {
        assert_eq!(need_query("Footsteps on the grass").as_deref(), Some("\"footsteps\"* OR \"grass\"*"));
        assert_eq!(need_query("a an of"), None);
    }

    #[tokio::test]
    async fn candidates_respect_licence_category_and_rejections() {
        let db = db().await;
        let ids = |c: &[Cand]| c.iter().map(|c| c.id).collect::<Vec<_>>();
        let c = candidates(&db, None, 1, true, "footsteps on grass", Some("sound-effect")).await.unwrap();
        assert_eq!(ids(&c).iter().copied().collect::<HashSet<_>>(), HashSet::from([3, 4]), "no readme, no personal-use pack for a sold game");
        assert_eq!(c[0].id, 3, "both words beat one");
        assert_eq!(c[0].licence, None, "no licence linked: the card says so");
        let c = candidates(&db, None, 1, false, "footsteps on grass", None).await.unwrap();
        assert!(ids(&c).contains(&5), "a game that isn't sold may use the personal-use pack");
        sqlx::query("INSERT INTO asset_pick (need_id, asset_id, status, by, at) VALUES (1, 3, 'rejected', 'kees', 'now')").execute(&db).await.unwrap();
        let c = candidates(&db, None, 1, true, "footsteps on grass", None).await.unwrap();
        assert!(!ids(&c).contains(&3), "rejected once, never suggested to this game again");
    }

    #[test]
    fn picks_only_from_the_shortlist() {
        let c = |id| Cand { id, name: format!("a{id}"), pack: "p".into(), category: "sound-effect".into(), caption: None, tags: None, licence: None };
        let list = vec![c(3), c(4)];
        let answer = r#"Sure! ```json
        {"picks": [{"id": 4, "reason": "Soft grass steps."}, {"id": 99, "reason": "made up"}, {"id": "3"}, {"id": 4, "reason": "again"}]}```"#;
        assert_eq!(check_picks(answer, &list).unwrap(), vec![(4, "Soft grass steps.".into()), (3, "Fits the need.".into())]);
        assert_eq!(check_picks("no json here", &list), None);
        let p = pick_prompt("Forest Walk", " (genre: walking sim)", "footsteps on grass", &list);
        assert!(p.contains("- id 3: a3 | sound-effect | pack p") && p.contains("Use only ids from the list"));
    }

    #[test]
    fn drafts_are_checked() {
        let d = check_draft(
            r#"{"genre": "Exploration", "artStyle": "low-poly", "setting": "forest",
               "needs": [{"text": "pine trees", "category": "3d-model"}, {"text": "Pine trees", "category": "3d-model"},
                         {"text": "wind ambience", "category": "weather"}, "UI clicks"]}"#,
        )
        .unwrap();
        assert_eq!(d.art_style, "low-poly");
        assert_eq!(
            d.needs,
            vec![("pine trees".into(), Some("3d-model".into())), ("wind ambience".into(), None), ("UI clicks".into(), None)],
            "no duplicates; a category not on the list becomes \"any\""
        );
    }

    #[test]
    fn games_come_from_game_json() {
        let repo = std::env::temp_dir().join(format!("kk-engine-{}", std::process::id()));
        for (dir, json) in [("showcase", r#"{"title": "KKE Showcase", "description": "Walk around."}"#), ("template", "{}"), ("bare", "{}")] {
            std::fs::create_dir_all(repo.join("games").join(dir)).unwrap();
            std::fs::write(repo.join("games").join(dir).join("game.json"), json).unwrap();
        }
        std::fs::create_dir_all(repo.join("games/no_json")).unwrap();
        let name = repo_name(&repo);
        let found = find_games(&repo);
        assert_eq!(found.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(), ["bare", "KKE Showcase"]);
        assert_eq!(found[1].key, format!("{name}/showcase"));
        assert_eq!(found[1].about, "Walk around.");
        std::fs::remove_dir_all(&repo).unwrap();
    }

    /// The whole chain against a stand-in OVMS: search, rerank (reversed), Coder picks one
    /// listed id and one made-up id; only the listed one is saved, with its reason.
    #[tokio::test]
    async fn pick_against_a_fake_ovms() {
        use axum::{Json, routing::post};
        let app = Router::new()
            .route("/v3/rerank", post(|Json(b): Json<Value>| async move {
                let n = b["documents"].as_array().map(|d| d.len()).unwrap_or(0);
                Json(json!({ "results": (0..n).map(|i| json!({ "index": i, "relevance_score": i as f64 })).collect::<Vec<_>>() }))
            }))
            .route("/v3/chat/completions", post(|Json(b): Json<Value>| async move {
                let prompt = b["messages"][0]["content"].as_str().unwrap_or_default().to_string();
                let first = prompt.split("- id ").nth(1).and_then(|l| l.split(':').next()).unwrap_or("0").to_string();
                let content = format!("{{\"picks\": [{{\"id\": {first}, \"reason\": \"Grass steps for the forest.\"}}, {{\"id\": 777, \"reason\": \"x\"}}]}}");
                Json(json!({ "choices": [{ "message": { "content": content } }] }))
            }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/v3", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let db = db().await;
        sqlx::query("INSERT INTO asset_pick (need_id, asset_id, status, by, at) VALUES (1, 3, 'candidate', 'kees', 'now')").execute(&db).await.unwrap();
        let n = pick(&db, &ai::Ai::fake(&url), 1).await.unwrap();
        assert_eq!(n, 1, "the made-up id 777 is dropped");
        let rows: Vec<(i64, String, Option<String>)> =
            sqlx::query_as("SELECT asset_id, status, reason FROM asset_pick WHERE need_id = 1 ORDER BY asset_id").fetch_all(&db).await.unwrap();
        // The reranker reversed the order, so the Coder saw the stone steps first.
        assert_eq!(rows, vec![(3, "candidate".into(), None), (4, "suggested".into(), Some("Grass steps for the forest.".into()))]);
        let picked: (Option<String>, Option<String>) = sqlx::query_as("SELECT picked_at, pick_error FROM asset_need WHERE id = 1").fetch_one(&db).await.unwrap();
        assert!(picked.0.is_some() && picked.1.is_none());
    }

    #[tokio::test]
    async fn clashing_styles_are_flagged() {
        let db = db().await;
        sqlx::query("INSERT INTO asset_tagname (id, kind, name) VALUES (1, 'style', 'low-poly'), (2, 'style', 'realistic')").execute(&db).await.unwrap();
        sqlx::query("INSERT INTO asset_tag (asset_id, tag_id, by) VALUES (1, 1, 'ai'), (4, 2, 'ai')").execute(&db).await.unwrap();
        sqlx::query("INSERT INTO asset_pick (need_id, asset_id, status, by, at) VALUES (1, 1, 'candidate', 'kees', 'now')").execute(&db).await.unwrap();
        assert_eq!(style_warning(&db, 1).await.unwrap(), None);
        sqlx::query("INSERT INTO asset_pick (need_id, asset_id, status, by, at) VALUES (1, 4, 'suggested', 'ai', 'now')").execute(&db).await.unwrap();
        assert_eq!(style_warning(&db, 1).await.unwrap().as_deref(), Some("Mixed styles: stylized (POLYGON Nature) next to realistic (Footsteps Pro)."));
    }
}
