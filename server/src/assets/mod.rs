//! Assets section: an index of the game asset library, which is mounted read-only
//! (`ASSET_LIBRARY`, e.g. /library). Every signed-in user can browse it; only an
//! admin can start a scan. Pack files are listed from their index, never unpacked.

pub mod ai;
mod classify;
mod games;
mod preview;
mod scan;
mod zipindex;

pub use preview::serve as preview_file;

use std::{path::PathBuf, time::Duration};

use axum::{
    Extension, Json, Router,
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{
    AppState,
    auth::User,
    error::{ApiError, ApiResult},
};

/// The library folder, from `ASSET_LIBRARY`. None: the Assets section says it isn't set up.
pub fn root() -> Option<PathBuf> {
    std::env::var("ASSET_LIBRARY").ok().filter(|s| !s.is_empty()).map(PathBuf::from)
}

/// Whether the library mount answers. After a crash or a remount a dead FUSE mount
/// fails every read with "not connected"; scans and previews then wait instead of
/// recording every asset as missing or broken.
pub fn library_online(root: &std::path::Path) -> bool {
    std::fs::read_dir(root).and_then(|mut d| d.next().transpose()).is_ok()
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/assets", get(list))
        .route("/assets/status", get(status))
        .route("/assets/facets", get(facets))
        .route("/assets/scan", post(start_scan))
        .route("/assets/previews/want", post(want_previews))
        .route("/assets/ai", get(ai_status).put(set_ai))
        .route("/assets/{id}", get(detail))
        .route("/assets/{id}/describe", post(describe))
        .route("/assets/{id}/category", post(set_category))
        .route("/assets/{id}/category/keep", post(keep_category))
        .route("/assets/{id}/tags", post(add_tag))
        .route("/assets/{id}/tags/{tag}", axum::routing::delete(remove_tag))
        .merge(games::routes())
}

/// sqlite-vec for every SQLite connection opened from now on (call before the pool).
pub fn register_sqlite_extensions() {
    ai::register_sqlite_vec();
}

/// Scans a minute after start, then every hour. Unchanged pack files are skipped,
/// so a scan of an unchanged library only walks the folders (seconds).
pub fn spawn(state: AppState) {
    let Some(root) = root() else {
        tracing::info!("ASSET_LIBRARY not set: Assets section off");
        return;
    };
    preview::spawn(state.db.clone(), state.bus.clone(), root.clone(), preview::dir(&state.config.database));
    // Off until an admin turns it on in the Assets header.
    ai::spawn(state.db.clone(), state.bus.clone(), ai::Ai::from_env(crate::llm::http_client()), preview::dir(&state.config.database));
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(60)).await;
        loop {
            match games::discover(&state.db).await {
                Ok(n) if n > 0 => tracing::info!(games = n, "asset games found in the repos"),
                Ok(_) => {}
                Err(e) => tracing::warn!(error = ?e, "asset games"),
            }
            if root.is_dir() {
                if let Err(e) = scan::run(&state.db, &state.bus, &root).await {
                    tracing::error!(error = ?e, "asset scan");
                }
                preview::wake();
            } else {
                tracing::warn!(path = %root.display(), "asset library not mounted");
            }
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
    });
}

async fn start_scan(State(s): State<AppState>, Extension(u): Extension<User>) -> ApiResult<StatusCode> {
    if !crate::admin::is_admin(&s.db, &u.id).await? {
        return Err(ApiError::Forbidden("Only an admin can start a scan.".into()));
    }
    let root = root().ok_or_else(|| ApiError::BadRequest("No asset library is set up (ASSET_LIBRARY).".into()))?;
    if !root.is_dir() {
        return Err(ApiError::BadRequest("The asset library folder is not mounted.".into()));
    }
    if scan::PROGRESS.lock().unwrap().running {
        return Ok(StatusCode::ACCEPTED);
    }
    tokio::spawn(async move {
        if let Err(e) = scan::run(&s.db, &s.bus, &root).await {
            tracing::error!(error = ?e, "asset scan");
        }
        preview::wake();
    });
    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize)]
pub struct Want {
    ids: Vec<i64>,
}

/// The cards someone is looking at: their previews are made first.
async fn want_previews(Json(w): Json<Want>) -> StatusCode {
    for id in w.ids.into_iter().take(200).rev() {
        preview::prioritise(id);
    }
    StatusCode::NO_CONTENT
}

async fn status(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let last: Option<(String, Option<String>, i64, i64, i64, i64, String, Option<i64>)> = sqlx::query_as(
        "SELECT started_at, finished_at, files, entries, unity, packs_read, errors, took_ms
         FROM asset_scan WHERE finished_at IS NOT NULL ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(&s.db)
    .await?;
    let (assets, bytes, packs): (i64, i64, i64) = sqlx::query_as(
        "SELECT COUNT(*), COALESCE(SUM(size), 0), (SELECT COUNT(*) FROM asset_pack WHERE missing_since IS NULL)
         FROM asset WHERE missing_since IS NULL",
    )
    .fetch_one(&s.db)
    .await?;
    let progress = scan::PROGRESS.lock().unwrap().clone();
    let previews = preview::PROGRESS.lock().unwrap().clone();
    Ok(Json(json!({
        "configured": root().is_some(),
        "mounted": root().is_some_and(|r| r.is_dir()),
        "assets": assets, "bytes": bytes, "packs": packs,
        "scan": progress,
        "previews": previews,
        "ai": ai::PROGRESS.lock().unwrap().clone(),
        "lastScan": last.map(|(started, finished, files, entries, unity, read, errors, ms)| json!({
            "startedAt": started, "finishedAt": finished, "files": files, "entries": entries, "unity": unity,
            "packsRead": read, "errors": serde_json::from_str::<Value>(&errors).unwrap_or(json!([])), "tookMs": ms,
        })),
        "categories": classify::CATEGORIES,
    })))
}

#[derive(Deserialize, Default)]
pub struct Filter {
    q: Option<String>,
    /// One category, or several separated by commas.
    category: Option<String>,
    pack: Option<i64>,
    /// Also show copies (same size and name as another asset).
    #[serde(default)]
    dups: bool,
    /// Only assets whose category the AI disagrees with.
    #[serde(default)]
    review: bool,
    /// "kind:name", e.g. "mood:calm".
    tag: Option<String>,
    /// Assets most like this one (needs AI vectors).
    similar: Option<i64>,
    /// Search `q` by meaning (vectors) instead of by words.
    #[serde(default)]
    meaning: bool,
    offset: Option<i64>,
    limit: Option<i64>,
}

/// FTS5 query from what the user typed: every word must match, as a prefix.
/// Quotes keep FTS syntax (AND, NEAR, *, :) from being read as operators.
fn fts_query(q: &str) -> Option<String> {
    let words: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(8)
        .map(|w| format!("\"{w}\"*"))
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

/// For "similar" and "by meaning": the nearest assets as ",id,id,...," (closest first).
async fn near(s: &AppState, f: &Filter) -> ApiResult<Option<String>> {
    let v = if let Some(id) = f.similar {
        ai::vector_of(&s.db, id).await?
    } else if f.meaning && let Some(q) = f.q.as_deref().filter(|q| !q.trim().is_empty()) {
        if ai::mode(&s.db).await == ai::Mode::Off {
            return Err(ApiError::BadRequest("Search by meaning needs AI tagging turned on.".into()));
        }
        ai::embed_query(&ai::Ai::from_env(s.http.clone()), q).await
    } else {
        return Ok(None);
    };
    let Some(v) = v else {
        // No vector yet (not described by the AI): nothing is "near".
        return Ok(Some(",".into()));
    };
    let ids = ai::nearest(&s.db, &v, 500).await?;
    let mut out = String::from(",");
    for (id, _) in ids.into_iter().filter(|(id, _)| Some(*id) != f.similar) {
        out.push_str(&format!("{id},"));
    }
    Ok(Some(out))
}

/// WHERE clause and its binds. `skip` leaves one filter out (for its own facet counts).
fn where_clause(f: &Filter, skip: &str, near: Option<&str>) -> (String, Vec<String>) {
    let mut sql = vec!["a.missing_since IS NULL".to_string()];
    let mut binds = vec![];
    let cats: Vec<&str> =
        f.category.as_deref().unwrap_or("").split(',').map(str::trim).filter(|c| !c.is_empty()).collect();
    if skip != "category" && !cats.is_empty() {
        sql.push(format!("a.category IN ({})", vec!["?"; cats.len()].join(", ")));
        binds.extend(cats.iter().map(|c| c.to_string()));
    }
    // Junk only when asked for by name.
    if !cats.contains(&"junk") {
        sql.push("a.category <> 'junk'".into());
    }
    if !f.dups {
        sql.push("a.dup_of IS NULL".into());
    }
    if skip != "pack" && let Some(p) = f.pack {
        sql.push("a.pack_id = ?".into());
        binds.push(p.to_string());
    }
    if let Some(q) = f.q.as_deref().and_then(fts_query).filter(|_| !f.meaning) {
        sql.push("a.id IN (SELECT rowid FROM asset_fts WHERE asset_fts MATCH ?)".into());
        binds.push(q);
    }
    if skip != "review" && f.review {
        sql.push("a.ai_category IS NOT NULL".into());
    }
    if let Some((kind, name)) = f.tag.as_deref().and_then(|t| t.split_once(':')) {
        sql.push("a.id IN (SELECT x.asset_id FROM asset_tag x JOIN asset_tagname t ON t.id = x.tag_id WHERE t.kind = ? AND t.name = ?)".into());
        binds.push(kind.to_string());
        binds.push(name.to_string());
    }
    if let Some(n) = near {
        sql.push("instr(?, ',' || a.id || ',') > 0".into());
        binds.push(n.to_string());
    }
    (sql.join(" AND "), binds)
}

fn bind_all<'q, O>(
    mut query: sqlx::query::QueryAs<'q, sqlx::Sqlite, O, sqlx::sqlite::SqliteArguments<'q>>,
    binds: &'q [String],
) -> sqlx::query::QueryAs<'q, sqlx::Sqlite, O, sqlx::sqlite::SqliteArguments<'q>> {
    for b in binds {
        query = query.bind(b);
    }
    query
}

#[derive(sqlx::FromRow)]
struct Item {
    id: i64,
    pack_id: i64,
    pack: String,
    container: String,
    path: String,
    name: String,
    ext: String,
    size: i64,
    category: String,
    is_meta: bool,
    dup_of: Option<i64>,
    preview_state: Option<String>,
    preview_kind: Option<String>,
    preview_v: i64,
    duration_s: Option<f64>,
    peaks: Option<String>,
    width: Option<i64>,
    height: Option<i64>,
    ai_category: Option<String>,
}

fn item_json(i: Item) -> Value {
    json!({
        "id": i.id, "packId": i.pack_id, "pack": i.pack, "container": i.container, "path": i.path,
        "name": i.name, "ext": i.ext, "size": i.size, "category": i.category, "meta": i.is_meta,
        "dupOf": i.dup_of,
        // Only finished previews; `pv` is part of the preview URL (a new preview, a new URL).
        "preview": if i.preview_state.as_deref() == Some("ok") { i.preview_kind } else { None },
        "pv": i.preview_v, "duration": i.duration_s, "peaks": i.peaks, "width": i.width, "height": i.height,
        "aiCategory": i.ai_category,
    })
}

const ITEM_COLUMNS: &str = "a.id, a.pack_id, p.name AS pack, a.container, a.path, a.name, a.ext, a.size, a.category, \
     a.is_meta, a.dup_of, a.preview_state, a.preview_kind, a.preview_v, a.duration_s, a.peaks, a.width, a.height, \
     a.ai_category";

async fn list(State(s): State<AppState>, Query(f): Query<Filter>) -> ApiResult<Json<Value>> {
    let near = near(&s, &f).await?;
    let (wh, mut binds) = where_clause(&f, "", near.as_deref());
    let limit = f.limit.unwrap_or(200).clamp(1, 500);
    let offset = f.offset.unwrap_or(0).max(0);
    let total: (i64,) = bind_all(sqlx::query_as(&format!("SELECT COUNT(*) FROM asset a WHERE {wh}")), &binds)
        .fetch_one(&s.db)
        .await?;
    // Nearest first when searching by vectors, else in pack and path order.
    let order = match &near {
        Some(n) => {
            binds.push(n.clone());
            "instr(?, ',' || a.id || ',')"
        }
        None => "p.name COLLATE NOCASE, a.pack_id, a.path COLLATE NOCASE",
    };
    let rows: Vec<Item> = bind_all(
        sqlx::query_as(&format!(
            "SELECT {ITEM_COLUMNS} FROM asset a JOIN asset_pack p ON p.id = a.pack_id WHERE {wh}
             ORDER BY {order} LIMIT {limit} OFFSET {offset}"
        )),
        &binds,
    )
    .fetch_all(&s.db)
    .await?;
    Ok(Json(json!({ "total": total.0, "offset": offset, "items": rows.into_iter().map(item_json).collect::<Vec<_>>() })))
}

/// Counts per category and per pack for the current filters (each facet ignores its own).
async fn facets(State(s): State<AppState>, Query(f): Query<Filter>) -> ApiResult<Json<Value>> {
    let near = near(&s, &f).await?;
    let (wh, binds) = where_clause(&f, "review", near.as_deref());
    let review: (i64,) = bind_all(sqlx::query_as(&format!("SELECT COUNT(*) FROM asset a WHERE {wh} AND a.ai_category IS NOT NULL")), &binds)
        .fetch_one(&s.db)
        .await?;
    let (wh, binds) = where_clause(&f, "category", near.as_deref());
    let cats: Vec<(String, i64, i64)> = bind_all(
        sqlx::query_as(&format!(
            "SELECT a.category, COUNT(*), COALESCE(SUM(a.size), 0) FROM asset a WHERE {wh} GROUP BY a.category"
        )),
        &binds,
    )
    .fetch_all(&s.db)
    .await?;
    let (wh, binds) = where_clause(&f, "pack", near.as_deref());
    let packs: Vec<(i64, String, String, i64, Option<i64>, Option<String>, i64, i64)> = bind_all(
        sqlx::query_as(&format!(
            "SELECT p.id, p.name, p.kind, p.size, p.duplicate_of, p.error, COUNT(a.id), COALESCE(SUM(a.size), 0)
             FROM asset_pack p JOIN asset a ON a.pack_id = p.id
             WHERE p.missing_since IS NULL AND {wh} GROUP BY p.id ORDER BY p.name COLLATE NOCASE"
        )),
        &binds,
    )
    .fetch_all(&s.db)
    .await?;
    Ok(Json(json!({
        "review": review.0,
        "categories": cats.into_iter().map(|(name, files, bytes)| json!({ "name": name, "files": files, "bytes": bytes })).collect::<Vec<_>>(),
        "packs": packs.into_iter().map(|(id, name, kind, size, dup, error, files, bytes)| json!({
            "id": id, "name": name, "kind": kind, "size": size, "duplicateOf": dup, "error": error,
            "files": files, "bytes": bytes,
        })).collect::<Vec<_>>(),
    })))
}

async fn detail(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<Json<Value>> {
    let item: Option<Item> = sqlx::query_as(&format!(
        "SELECT {ITEM_COLUMNS} FROM asset a JOIN asset_pack p ON p.id = a.pack_id WHERE a.id = ?"
    ))
    .bind(id)
    .fetch_optional(&s.db)
    .await?;
    let Some(item) = item else { return Err(ApiError::NotFound) };
    #[allow(clippy::type_complexity)]
    let (mtime, rule, missing, pack_kind, rate, channels, alpha, state, error): (
        Option<i64>, String, Option<String>, String, Option<i64>, Option<i64>, Option<bool>, Option<String>, Option<String>,
    ) = sqlx::query_as(
        "SELECT a.mtime, a.rule, a.missing_since, p.kind, a.sample_rate, a.channels, a.has_alpha, a.preview_state, a.preview_error
         FROM asset a JOIN asset_pack p ON p.id = a.pack_id WHERE a.id = ?",
    )
    .bind(id)
    .fetch_one(&s.db)
    .await?;
    let (pack_id, dup_of) = (item.pack_id, item.dup_of);
    let (ai_state, ai_error, caption, subject, transcript, ai_model, category_by): (
        Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, Option<String>, String,
    ) = sqlx::query_as(
        "SELECT ai_state, ai_error, ai_caption, ai_subject, ai_transcript, ai_model, category_by FROM asset WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&s.db)
    .await?;
    let tags: Vec<(i64, String, String, String)> = sqlx::query_as(
        "SELECT t.id, t.kind, t.name, x.by FROM asset_tag x JOIN asset_tagname t ON t.id = x.tag_id WHERE x.asset_id = ?
         ORDER BY t.kind, t.name",
    )
    .bind(id)
    .fetch_all(&s.db)
    .await?;
    let has_vector = ai::vector_of(&s.db, id).await.ok().flatten().is_some();
    // Its copies (or the original and its other copies).
    let keep = dup_of.unwrap_or(id);
    let copies: Vec<(i64, String, String)> = sqlx::query_as(
        "SELECT id, container, path FROM asset WHERE (id = ? OR dup_of = ?) AND id <> ? ORDER BY id LIMIT 20",
    )
    .bind(keep)
    .bind(keep)
    .bind(id)
    .fetch_all(&s.db)
    .await?;
    // Licence and readme files of the same pack.
    let docs: Vec<(i64, String)> =
        sqlx::query_as("SELECT id, path FROM asset WHERE pack_id = ? AND is_meta = 1 AND missing_since IS NULL ORDER BY path LIMIT 20")
            .bind(pack_id)
            .fetch_all(&s.db)
            .await?;
    let licence = games::licence_of_pack(&s.db, pack_id).await?;
    let mut out = item_json(item);
    let extra = json!({
        "licence": licence,
        "packKind": pack_kind, "mtime": mtime, "rule": rule, "missingSince": missing, "sampleRate": rate,
        "channels": channels, "hasAlpha": alpha, "previewState": state, "previewError": error,
        "aiState": ai_state, "aiError": ai_error, "aiCaption": caption, "aiSubject": subject, "transcript": transcript,
        "aiModel": ai_model, "categoryBy": category_by, "similar": has_vector,
        "tags": tags.into_iter().map(|(id, kind, name, by)| json!({ "id": id, "kind": kind, "name": name, "by": by })).collect::<Vec<_>>(),
        "copies": copies.into_iter().map(|(id, c, p)| json!({ "id": id, "container": c, "path": p })).collect::<Vec<_>>(),
        "packDocs": docs.into_iter().map(|(id, p)| json!({ "id": id, "path": p })).collect::<Vec<_>>(),
    });
    if let (Some(o), Value::Object(e)) = (out.as_object_mut(), extra) {
        o.extend(e);
    }
    Ok(Json(out))
}

async fn require_admin(s: &AppState, u: &User) -> ApiResult<()> {
    if crate::admin::is_admin(&s.db, &u.id).await? {
        Ok(())
    } else {
        Err(ApiError::Forbidden("Only an admin can change this.".into()))
    }
}

/// Tell every signed-in user that this asset changed (their grid and details update).
fn changed(s: &AppState, id: i64) {
    let p = ai::PROGRESS.lock().unwrap().clone();
    s.bus.send_all(crate::events::Event::Assets { scan: None, previews: None, ai: Some(json!({ "ids": [id], "progress": p })), games: None });
}

async fn ai_status(State(s): State<AppState>) -> ApiResult<Json<Value>> {
    let mut p = ai::PROGRESS.lock().unwrap().clone();
    p.mode = ai::mode(&s.db).await;
    let (tagged, review): (i64, i64) = sqlx::query_as(
        "SELECT COUNT(*) FILTER (WHERE ai_state = 'ok'), COUNT(*) FILTER (WHERE ai_category IS NOT NULL) FROM asset WHERE missing_since IS NULL",
    )
    .fetch_one(&s.db)
    .await?;
    Ok(Json(json!({ "progress": p, "tagged": tagged, "review": review })))
}

#[derive(Deserialize)]
pub struct SetAi {
    mode: ai::Mode,
}

/// Off / Nightly / Always. Admin only; "off" stops after the request in flight.
async fn set_ai(State(s): State<AppState>, Extension(u): Extension<User>, Json(b): Json<SetAi>) -> ApiResult<StatusCode> {
    require_admin(&s, &u).await?;
    ai::set_mode(&s.db, b.mode).await?;
    tracing::info!(mode = ?b.mode, by = %u.name, "asset AI tagging");
    changed(&s, 0);
    Ok(StatusCode::NO_CONTENT)
}

/// "Describe now": this asset goes first, while tagging is on.
async fn describe(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    if ai::mode(&s.db).await == ai::Mode::Off {
        return Err(ApiError::BadRequest("AI tagging is off. An admin can turn it on in the Assets header.".into()));
    }
    sqlx::query("UPDATE asset SET ai_state = NULL WHERE id = ?").bind(id).execute(&s.db).await?;
    ai::prioritise(id);
    Ok(StatusCode::ACCEPTED)
}

#[derive(Deserialize)]
pub struct SetCategory {
    category: String,
}

/// A person's category: kept by every later scan and AI run.
async fn set_category(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<i64>,
    Json(b): Json<SetCategory>,
) -> ApiResult<StatusCode> {
    if !ai::is_category(&b.category) {
        return Err(ApiError::BadRequest("Unknown category.".into()));
    }
    let n = sqlx::query(
        "UPDATE asset SET category = ?, category_by = 'kees', rule = ?, ai_category = NULL WHERE id = ?",
    )
    .bind(&b.category)
    .bind(format!("set by {}", u.name))
    .bind(id)
    .execute(&s.db)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(ApiError::NotFound);
    }
    ai::refresh_search(&s.db, id).await?;
    changed(&s, id);
    Ok(StatusCode::NO_CONTENT)
}

/// The rule's category stays; the AI's other opinion is dismissed.
async fn keep_category(State(s): State<AppState>, Path(id): Path<i64>) -> ApiResult<StatusCode> {
    sqlx::query("UPDATE asset SET ai_category = NULL WHERE id = ?").bind(id).execute(&s.db).await?;
    changed(&s, id);
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
pub struct NewTag {
    #[serde(default)]
    kind: Option<String>,
    name: String,
}

/// A tag set by a person ("custom", or one of the AI's kinds).
async fn add_tag(State(s): State<AppState>, Path(id): Path<i64>, Json(b): Json<NewTag>) -> ApiResult<Json<Value>> {
    let name = b.name.trim().to_lowercase();
    let kind = b.kind.as_deref().unwrap_or("custom");
    if name.is_empty() || name.chars().count() > 40 || !["custom", "style", "mood", "setting"].contains(&kind) {
        return Err(ApiError::BadRequest("A tag is 1 to 40 characters.".into()));
    }
    sqlx::query("INSERT INTO asset_tagname (kind, name) VALUES (?, ?) ON CONFLICT DO NOTHING").bind(kind).bind(&name).execute(&s.db).await?;
    let tag: (i64,) = sqlx::query_as("SELECT id FROM asset_tagname WHERE kind = ? AND name = ?").bind(kind).bind(&name).fetch_one(&s.db).await?;
    sqlx::query("INSERT INTO asset_tag (asset_id, tag_id, by) VALUES (?, ?, 'kees') ON CONFLICT(asset_id, tag_id) DO UPDATE SET by = 'kees'")
        .bind(id)
        .bind(tag.0)
        .execute(&s.db)
        .await
        .map_err(|_| ApiError::NotFound)?;
    ai::refresh_search(&s.db, id).await?;
    changed(&s, id);
    Ok(Json(json!({ "id": tag.0, "kind": kind, "name": name, "by": "kees" })))
}

async fn remove_tag(State(s): State<AppState>, Path((id, tag)): Path<(i64, i64)>) -> ApiResult<StatusCode> {
    sqlx::query("DELETE FROM asset_tag WHERE asset_id = ? AND tag_id = ?").bind(id).bind(tag).execute(&s.db).await?;
    ai::refresh_search(&s.db, id).await?;
    changed(&s, id);
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_online_needs_a_readable_root() {
        assert!(library_online(&std::env::temp_dir()));
        assert!(!library_online(std::path::Path::new("/no/such/library")));
    }

    #[test]
    fn search_words_are_quoted_prefixes() {
        assert_eq!(fts_query("forest  amb").as_deref(), Some("\"forest\"* \"amb\"*"));
        assert_eq!(fts_query("NEAR(a b) OR \"x\"").as_deref(), Some("\"NEAR\"* \"a\"* \"b\"* \"OR\"* \"x\"*"));
        assert_eq!(fts_query(" - * "), None);
    }

    #[tokio::test]
    async fn scan_then_browse() {
        let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!().run(&db).await.unwrap();
        let dir = std::env::temp_dir().join(format!("kk-lib-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("LOFI")).unwrap();
        std::fs::write(dir.join("LOFI/track 1.mp3"), vec![0u8; 5000]).unwrap();
        std::fs::write(dir.join("LOFI/track 1 (1).mp3"), vec![0u8; 5000]).unwrap();
        std::fs::write(dir.join("broken.zip"), b"not a zip").unwrap();
        std::fs::write(dir.join(".DS_Store"), b"x").unwrap();
        let bus = crate::events::Bus::new();
        assert!(scan::run(&db, &bus, &dir).await.unwrap());

        let f = |q: &str| Filter { q: Some(q.into()), ..Default::default() };
        let (wh, binds) = where_clause(&f("track"), "", None);
        let n: (i64,) = bind_all(sqlx::query_as(&format!("SELECT COUNT(*) FROM asset a WHERE {wh}")), &binds)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(n.0, 1, "the \" (1)\" copy is hidden");
        let cats: Vec<(String,)> = sqlx::query_as("SELECT category FROM asset ORDER BY path").fetch_all(&db).await.unwrap();
        assert_eq!(cats.iter().map(|c| c.0.as_str()).collect::<Vec<_>>(), ["junk", "music", "music"]);
        let err: (Option<String>,) = sqlx::query_as("SELECT error FROM asset_pack WHERE kind = 'zip'").fetch_one(&db).await.unwrap();
        assert!(err.0.is_some(), "an unreadable zip is a pack with an error");

        // Second scan: the file is gone -> missing, not deleted.
        std::fs::remove_file(dir.join("LOFI/track 1.mp3")).unwrap();
        assert!(scan::run(&db, &bus, &dir).await.unwrap());
        let missing: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM asset WHERE missing_since IS NOT NULL").fetch_one(&db).await.unwrap();
        assert_eq!(missing.0, 1);
        std::fs::remove_dir_all(&dir).unwrap();
    }
}

/// Parity with survey_assets.py on a real library (read-only):
/// `KK_LIBRARY=/media/Kees/GameDev cargo test --release survey_parity -- --ignored --nocapture`
#[cfg(test)]
mod parity {
    #[tokio::test]
    #[ignore]
    async fn survey_parity() {
        let Ok(root) = std::env::var("KK_LIBRARY") else { return };
        let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!().run(&db).await.unwrap();
        let t = std::time::Instant::now();
        super::scan::run(&db, &crate::events::Bus::new(), std::path::Path::new(&root)).await.unwrap();
        println!("scan took {:?}", t.elapsed());
        let rows: Vec<(String, String, i64, i64)> = sqlx::query_as(
            "SELECT CASE WHEN container = '' THEN 'disk' WHEN container LIKE '%.zip' THEN 'zip' ELSE 'unity' END,
                    category, COUNT(*), SUM(size) FROM asset GROUP BY 1, 2 ORDER BY 1, 3 DESC",
        )
        .fetch_all(&db)
        .await
        .unwrap();
        for r in rows {
            println!("{}\t{}\t{}\t{}", r.0, r.1, r.2, r.3);
        }
        let s: (i64, i64, i64, i64) = sqlx::query_as("SELECT files, entries, unity, packs_read FROM asset_scan").fetch_one(&db).await.unwrap();
        println!("files {} entries {} unity {} packs_read {}", s.0, s.1, s.2, s.3);
        let d: (i64, i64) = sqlx::query_as("SELECT (SELECT COUNT(*) FROM asset_pack WHERE duplicate_of IS NOT NULL), (SELECT COUNT(*) FROM asset WHERE dup_of IS NOT NULL)").fetch_one(&db).await.unwrap();
        println!("duplicate packs {} duplicate assets {}", d.0, d.1);
    }
}
