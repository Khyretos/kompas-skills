//! Assets section: an index of the game asset library, which is mounted read-only
//! (`ASSET_LIBRARY`, e.g. /library). Every signed-in user can browse it; only an
//! admin can start a scan. Pack files are listed from their index, never unpacked.

mod classify;
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

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/assets", get(list))
        .route("/assets/status", get(status))
        .route("/assets/facets", get(facets))
        .route("/assets/scan", post(start_scan))
        .route("/assets/previews/want", post(want_previews))
        .route("/assets/{id}", get(detail))
}

/// Scans a minute after start, then every hour. Unchanged pack files are skipped,
/// so a scan of an unchanged library only walks the folders (seconds).
pub fn spawn(state: AppState) {
    let Some(root) = root() else {
        tracing::info!("ASSET_LIBRARY not set: Assets section off");
        return;
    };
    preview::spawn(state.db.clone(), state.bus.clone(), root.clone(), preview::dir(&state.config.database));
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(60)).await;
        loop {
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

/// WHERE clause and its binds. `skip` leaves one filter out (for its own facet counts).
fn where_clause(f: &Filter, skip: &str) -> (String, Vec<String>) {
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
    if let Some(q) = f.q.as_deref().and_then(fts_query) {
        sql.push("a.id IN (SELECT rowid FROM asset_fts WHERE asset_fts MATCH ?)".into());
        binds.push(q);
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
}

fn item_json(i: Item) -> Value {
    json!({
        "id": i.id, "packId": i.pack_id, "pack": i.pack, "container": i.container, "path": i.path,
        "name": i.name, "ext": i.ext, "size": i.size, "category": i.category, "meta": i.is_meta,
        "dupOf": i.dup_of,
        // Only finished previews; `pv` is part of the preview URL (a new preview, a new URL).
        "preview": if i.preview_state.as_deref() == Some("ok") { i.preview_kind } else { None },
        "pv": i.preview_v, "duration": i.duration_s, "peaks": i.peaks, "width": i.width, "height": i.height,
    })
}

const ITEM_COLUMNS: &str = "a.id, a.pack_id, p.name AS pack, a.container, a.path, a.name, a.ext, a.size, a.category, \
     a.is_meta, a.dup_of, a.preview_state, a.preview_kind, a.preview_v, a.duration_s, a.peaks, a.width, a.height";

async fn list(State(s): State<AppState>, Query(f): Query<Filter>) -> ApiResult<Json<Value>> {
    let (wh, binds) = where_clause(&f, "");
    let limit = f.limit.unwrap_or(200).clamp(1, 500);
    let offset = f.offset.unwrap_or(0).max(0);
    let total: (i64,) = bind_all(sqlx::query_as(&format!("SELECT COUNT(*) FROM asset a WHERE {wh}")), &binds)
        .fetch_one(&s.db)
        .await?;
    let rows: Vec<Item> = bind_all(
        sqlx::query_as(&format!(
            "SELECT {ITEM_COLUMNS} FROM asset a JOIN asset_pack p ON p.id = a.pack_id WHERE {wh}
             ORDER BY p.name COLLATE NOCASE, a.pack_id, a.path COLLATE NOCASE LIMIT {limit} OFFSET {offset}"
        )),
        &binds,
    )
    .fetch_all(&s.db)
    .await?;
    Ok(Json(json!({ "total": total.0, "offset": offset, "items": rows.into_iter().map(item_json).collect::<Vec<_>>() })))
}

/// Counts per category and per pack for the current filters (each facet ignores its own).
async fn facets(State(s): State<AppState>, Query(f): Query<Filter>) -> ApiResult<Json<Value>> {
    let (wh, binds) = where_clause(&f, "category");
    let cats: Vec<(String, i64, i64)> = bind_all(
        sqlx::query_as(&format!(
            "SELECT a.category, COUNT(*), COALESCE(SUM(a.size), 0) FROM asset a WHERE {wh} GROUP BY a.category"
        )),
        &binds,
    )
    .fetch_all(&s.db)
    .await?;
    let (wh, binds) = where_clause(&f, "pack");
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
    let mut out = item_json(item);
    let extra = json!({
        "packKind": pack_kind, "mtime": mtime, "rule": rule, "missingSince": missing, "sampleRate": rate,
        "channels": channels, "hasAlpha": alpha, "previewState": state, "previewError": error,
        "copies": copies.into_iter().map(|(id, c, p)| json!({ "id": id, "container": c, "path": p })).collect::<Vec<_>>(),
        "packDocs": docs.into_iter().map(|(id, p)| json!({ "id": id, "path": p })).collect::<Vec<_>>(),
    });
    if let (Some(o), Value::Object(e)) = (out.as_object_mut(), extra) {
        o.extend(e);
    }
    Ok(Json(out))
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let (wh, binds) = where_clause(&f("track"), "");
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
