//! The library scan: walk with stat only, list pack files from their index, upsert
//! by path, skip pack files whose size and mtime didn't change, and mark vanished
//! files `missing_since` instead of deleting them. Nothing in the library is written.

use std::{
    collections::HashMap,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::{LazyLock, Mutex},
    time::Instant,
};

use anyhow::Result;
use serde::Serialize;
use sqlx::SqlitePool;

use super::{classify, zipindex};
use crate::{events::Bus, util};

/// Bump when classifier rules change: the next scan re-classifies every row.
pub const RULES_VERSION: i64 = 1;

/// Live scan state, sent to the web app as it changes.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub running: bool,
    /// walking | packs | saving | done | failed
    pub phase: &'static str,
    pub done: u64,
    pub total: u64,
    /// The pack file being listed.
    pub current: String,
    pub error: Option<String>,
}

pub static PROGRESS: LazyLock<Mutex<Progress>> = LazyLock::new(Default::default);

fn set(bus: &Bus, f: impl FnOnce(&mut Progress)) {
    let snapshot = {
        let mut p = PROGRESS.lock().unwrap();
        f(&mut p);
        p.clone()
    };
    bus.send_all(crate::events::Event::Assets { scan: serde_json::to_value(snapshot).ok(), previews: None });
}

/// A file on disk, relative to the library root.
#[derive(Debug)]
pub struct Found {
    pub rel: String,
    pub size: u64,
    pub mtime: i64,
}

/// Walks `root` without following symlinks. Returns files and unreadable entries.
pub fn walk(root: &Path) -> (Vec<Found>, Vec<String>) {
    let (mut files, mut errors) = (vec![], vec![]);
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rd = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(e) => {
                errors.push(format!("{}: {e}", rel_of(root, &dir)));
                continue;
            }
        };
        for ent in rd.flatten() {
            let Ok(ft) = ent.file_type() else { continue };
            if ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                stack.push(ent.path());
                continue;
            }
            match ent.metadata() {
                Ok(m) => files.push(Found { rel: rel_of(root, &ent.path()), size: m.len(), mtime: m.mtime() }),
                Err(e) => errors.push(format!("{}: {e}", rel_of(root, &ent.path()))),
            }
        }
    }
    files.sort_by(|a, b| a.rel.cmp(&b.rel));
    (files, errors)
}

fn rel_of(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).to_string_lossy().into_owned()
}

/// The pack a loose file belongs to: its top-level folder, or '' at the root.
fn folder_pack(rel: &str) -> &str {
    rel.split_once('/').map(|(top, _)| top).unwrap_or("")
}

fn file_name(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

/// "SILENT HILL SFX/sh sfx.zip" -> "sh sfx"
fn stem(rel: &str) -> &str {
    let name = file_name(rel);
    name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name)
}

fn pack_kind(ext: &str) -> Option<&'static str> {
    match ext {
        "zip" => Some("zip"),
        "unitypackage" => Some("unitypackage"),
        _ => None,
    }
}

/// One row to upsert.
struct Row {
    container: String,
    path: String,
    size: i64,
    mtime: Option<i64>,
    offset: Option<i64>,
}

/// Classify as the survey does: entries as `<pack file without extension>/<entry>`.
fn classified(container: &str, path: &str) -> (String, String, &'static str, String, bool) {
    let name = file_name(path).to_string();
    let ext = classify::ext_of(&name);
    let rel = if container.is_empty() {
        path.to_string()
    } else {
        let parent = container.rsplit_once('/').map(|(d, _)| format!("{d}/")).unwrap_or_default();
        format!("{parent}{}/{path}", stem(container))
    };
    let (cat, why) = classify::classify_why(&rel, &ext);
    let meta = classify::is_meta(&name);
    (name, ext, cat, why, meta)
}

/// Runs one scan unless one is running. Returns false when it was already running.
pub async fn run(db: &SqlitePool, bus: &Bus, root: &Path) -> Result<bool> {
    {
        let mut p = PROGRESS.lock().unwrap();
        if p.running {
            return Ok(false);
        }
        *p = Progress { running: true, phase: "walking", ..Default::default() };
    }
    let result = scan(db, bus, root).await;
    match &result {
        Ok(()) => set(bus, |p| {
            p.running = false;
            p.phase = "done";
            p.current.clear();
        }),
        Err(e) => {
            tracing::error!(error = ?e, "asset scan failed");
            set(bus, |p| {
                p.running = false;
                p.phase = "failed";
                p.error = Some("The scan stopped with an error; the server log has the details.".into());
            })
        }
    }
    result.map(|_| true)
}

async fn scan(db: &SqlitePool, bus: &Bus, root: &Path) -> Result<()> {
    let started = Instant::now();
    let scan_id: i64 = sqlx::query_scalar("INSERT INTO asset_scan (started_at, rules) VALUES (?, ?) RETURNING id")
        .bind(util::now())
        .bind(RULES_VERSION)
        .fetch_one(db)
        .await?;
    set(bus, |_| {});
    let root_buf: PathBuf = root.to_path_buf();
    let (files, mut errors) = tokio::task::spawn_blocking(move || walk(&root_buf)).await?;

    // Packs already known: key -> (id, size, mtime, error).
    let known: HashMap<String, (i64, i64, i64, Option<String>)> =
        sqlx::query_as::<_, (String, i64, i64, i64, Option<String>)>("SELECT key, id, size, mtime, error FROM asset_pack")
            .fetch_all(db)
            .await?
            .into_iter()
            .map(|(k, id, s, m, e)| (k, (id, s, m, e)))
            .collect();
    let last_rules: i64 = sqlx::query_scalar(
        "SELECT COALESCE((SELECT rules FROM asset_scan WHERE finished_at IS NOT NULL ORDER BY id DESC LIMIT 1), 0)",
    )
    .fetch_one(db)
    .await?;
    let reclassify = last_rules != RULES_VERSION;

    let (pack_files, loose): (Vec<&Found>, Vec<&Found>) =
        files.iter().partition(|f| pack_kind(&classify::ext_of(&f.rel)).is_some());

    // Loose files, grouped into folder packs.
    let mut folders: HashMap<&str, (i64, Vec<Row>)> = HashMap::new();
    for f in &loose {
        let entry = folders.entry(folder_pack(&f.rel)).or_default();
        entry.0 += f.size as i64;
        entry.1.push(Row { container: String::new(), path: f.rel.clone(), size: f.size as i64, mtime: Some(f.mtime), offset: None });
    }
    set(bus, |p| {
        p.phase = "packs";
        p.total = pack_files.len() as u64;
    });
    let (mut entries, mut unity, mut packs_read) = (0i64, 0i64, 0i64);
    for (key, (size, rows)) in folders {
        let (name, kind) = if key.is_empty() { ("Loose files", "loose") } else { (key, "folder") };
        let pack_id = upsert_pack(db, key, name, kind, size, 0, None, scan_id).await?;
        save_rows(db, pack_id, "", rows, scan_id).await?;
    }

    for (i, f) in pack_files.iter().enumerate() {
        let ext = classify::ext_of(&f.rel);
        let kind = pack_kind(&ext).unwrap_or("zip");
        set(bus, |p| {
            p.done = i as u64;
            p.current = stem(&f.rel).to_string();
        });
        let unchanged = known
            .get(&f.rel)
            .is_some_and(|(_, s, m, e)| *s == f.size as i64 && *m == f.mtime && e.is_none());
        if unchanged {
            let id = known[&f.rel].0;
            sqlx::query("UPDATE asset_pack SET seen_scan = ?, missing_since = NULL WHERE id = ?")
                .bind(scan_id)
                .bind(id)
                .execute(db)
                .await?;
            let n = sqlx::query("UPDATE asset SET seen_scan = ?, missing_since = NULL WHERE container = ?")
                .bind(scan_id)
                .bind(&f.rel)
                .execute(db)
                .await?
                .rows_affected() as i64;
            if kind == "zip" { entries += n } else { unity += n }
            continue;
        }
        let path = root.join(&f.rel);
        let listed = tokio::task::spawn_blocking(move || {
            if ext == "zip" { zipindex::zip_entries(&path) } else { zipindex::unitypackage_entries(&path) }
        })
        .await?;
        packs_read += 1;
        let (rows, error) = match listed {
            Ok(list) => (
                list.into_iter()
                    .map(|e| Row {
                        container: f.rel.clone(),
                        path: e.name,
                        size: e.size as i64,
                        mtime: None,
                        offset: (kind == "zip").then_some(e.offset as i64),
                    })
                    .collect::<Vec<_>>(),
                None,
            ),
            Err(e) => {
                errors.push(format!("{}: {kind}: {e}", f.rel));
                (vec![], Some(e.to_string()))
            }
        };
        if kind == "zip" { entries += rows.len() as i64 } else { unity += rows.len() as i64 }
        let pack_id = upsert_pack(db, &f.rel, stem(&f.rel), kind, f.size as i64, f.mtime, error, scan_id).await?;
        let container = f.rel.clone();
        save_rows(db, pack_id, &container, rows, scan_id).await?;
    }

    set(bus, |p| {
        p.phase = "saving";
        p.done = p.total;
        p.current.clear();
    });
    let now = util::now();
    sqlx::query("UPDATE asset SET missing_since = ? WHERE seen_scan < ? AND missing_since IS NULL")
        .bind(&now)
        .bind(scan_id)
        .execute(db)
        .await?;
    sqlx::query("UPDATE asset_pack SET missing_since = ? WHERE seen_scan < ? AND missing_since IS NULL")
        .bind(&now)
        .bind(scan_id)
        .execute(db)
        .await?;
    if reclassify {
        reclassify_all(db).await?;
    }
    mark_duplicates(db).await?;
    rebuild_search(db).await?;

    let first: Vec<&String> = errors.iter().take(50).collect();
    sqlx::query(
        "UPDATE asset_scan SET finished_at = ?, files = ?, entries = ?, unity = ?, packs_read = ?, errors = ?, took_ms = ?
         WHERE id = ?",
    )
    .bind(util::now())
    .bind(files.len() as i64)
    .bind(entries)
    .bind(unity)
    .bind(packs_read)
    .bind(serde_json::to_string(&first)?)
    .bind(started.elapsed().as_millis() as i64)
    .bind(scan_id)
    .execute(db)
    .await?;
    // Keep the last 50 scans.
    sqlx::query("DELETE FROM asset_scan WHERE id <= ? - 50").bind(scan_id).execute(db).await?;
    tracing::info!(files = files.len(), entries, unity, packs_read, ms = started.elapsed().as_millis() as u64, "asset scan done");
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn upsert_pack(
    db: &SqlitePool,
    key: &str,
    name: &str,
    kind: &str,
    size: i64,
    mtime: i64,
    error: Option<String>,
    scan_id: i64,
) -> Result<i64> {
    Ok(sqlx::query_scalar(
        "INSERT INTO asset_pack (key, name, kind, size, mtime, error, seen_scan) VALUES (?, ?, ?, ?, ?, ?, ?)
         ON CONFLICT(key) DO UPDATE SET name = excluded.name, kind = excluded.kind, size = excluded.size,
           mtime = excluded.mtime, error = excluded.error, seen_scan = excluded.seen_scan, missing_since = NULL
         RETURNING id",
    )
    .bind(key)
    .bind(name)
    .bind(kind)
    .bind(size)
    .bind(mtime)
    .bind(error)
    .bind(scan_id)
    .fetch_one(db)
    .await?)
}

/// Upserts one pack's rows in one transaction (a short write lock per pack).
async fn save_rows(db: &SqlitePool, pack_id: i64, container: &str, rows: Vec<Row>, scan_id: i64) -> Result<()> {
    let mut tx = db.begin().await?;
    for r in rows {
        let (name, ext, cat, why, meta) = classified(container, &r.path);
        sqlx::query(
            "INSERT INTO asset (pack_id, container, path, name, ext, size, mtime, entry_offset, category, rule, is_meta, seen_scan)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(container, path) DO UPDATE SET pack_id = excluded.pack_id, size = excluded.size,
               mtime = excluded.mtime, entry_offset = excluded.entry_offset, category = excluded.category,
               rule = excluded.rule, is_meta = excluded.is_meta, seen_scan = excluded.seen_scan, missing_since = NULL,
               preview_state = CASE WHEN asset.size <> excluded.size THEN NULL ELSE asset.preview_state END",
        )
        .bind(pack_id)
        .bind(&r.container)
        .bind(&r.path)
        .bind(name)
        .bind(ext)
        .bind(r.size)
        .bind(r.mtime)
        .bind(r.offset)
        .bind(cat)
        .bind(why)
        .bind(meta)
        .bind(scan_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// After a classifier change: every row gets its category again from its path.
async fn reclassify_all(db: &SqlitePool) -> Result<()> {
    let rows: Vec<(i64, String, String)> = sqlx::query_as("SELECT id, container, path FROM asset").fetch_all(db).await?;
    let mut tx = db.begin().await?;
    for (id, container, path) in rows {
        let (_, _, cat, why, meta) = classified(&container, &path);
        sqlx::query("UPDATE asset SET category = ?, rule = ?, is_meta = ? WHERE id = ?")
            .bind(cat)
            .bind(why)
            .bind(meta)
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Same size and same name apart from " (1)": a duplicate download, shown once.
/// The copy with the shortest full path is the one kept ("X.zip" over "X (1).zip").
async fn mark_duplicates(db: &SqlitePool) -> Result<()> {
    let packs: Vec<(i64, String, i64)> =
        sqlx::query_as("SELECT id, key, size FROM asset_pack WHERE kind IN ('zip', 'unitypackage') AND missing_since IS NULL")
            .fetch_all(db)
            .await?;
    let assets: Vec<(i64, String, String, i64)> = sqlx::query_as(
        "SELECT id, container, path, size FROM asset WHERE size >= 4096 AND missing_since IS NULL AND category <> 'junk'",
    )
    .fetch_all(db)
    .await?;
    let pack_dups = keepers(packs.iter().map(|(id, key, size)| (*id, key.clone(), *size, classify::dup_name(file_name(key)))));
    let asset_dups = keepers(assets.iter().map(|(id, c, p, size)| {
        (*id, format!("{c}/{p}"), *size, classify::dup_name(file_name(p)))
    }));
    let mut tx = db.begin().await?;
    sqlx::query("UPDATE asset_pack SET duplicate_of = NULL").execute(&mut *tx).await?;
    sqlx::query("UPDATE asset SET dup_of = NULL WHERE dup_of IS NOT NULL").execute(&mut *tx).await?;
    for (id, keep) in pack_dups {
        sqlx::query("UPDATE asset_pack SET duplicate_of = ? WHERE id = ?").bind(keep).bind(id).execute(&mut *tx).await?;
    }
    for (id, keep) in asset_dups {
        sqlx::query("UPDATE asset SET dup_of = ? WHERE id = ?").bind(keep).bind(id).execute(&mut *tx).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// (id, full path, size, name) -> (copy id, kept id) for every group of two or more.
fn keepers(items: impl Iterator<Item = (i64, String, i64, String)>) -> Vec<(i64, i64)> {
    let mut groups: HashMap<(i64, String), Vec<(i64, String)>> = HashMap::new();
    for (id, full, size, name) in items {
        groups.entry((size, name)).or_default().push((id, full));
    }
    let mut out = vec![];
    for (_, mut g) in groups.into_iter().filter(|(_, g)| g.len() > 1) {
        g.sort_by(|a, b| a.1.len().cmp(&b.1.len()).then_with(|| a.1.cmp(&b.1)));
        let keep = g[0].0;
        out.extend(g[1..].iter().map(|(id, _)| (*id, keep)));
    }
    out
}

async fn rebuild_search(db: &SqlitePool) -> Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM asset_fts").execute(&mut *tx).await?;
    sqlx::query(
        "INSERT INTO asset_fts (rowid, name, path, pack, category)
         SELECT a.id, a.name, CASE a.container WHEN '' THEN a.path ELSE a.container || '/' || a.path END, p.name, a.category
         FROM asset a JOIN asset_pack p ON p.id = a.pack_id WHERE a.missing_since IS NULL",
    )
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entries_are_classified_with_the_pack_name() {
        let (_, ext, cat, _, _) = classified("Fantasy RPG Music Pack.zip", "Tracks/mp3/Action 1.mp3");
        assert_eq!((ext.as_str(), cat), ("mp3", "music"));
        // A pack file in a folder keeps the folder: "SILENT HILL SFX/<zip>/<entry>".
        let (_, _, cat, _, _) = classified("Packs/Horror SFX.zip", "Ambient/Robots/Warning High Voltage.ogg");
        assert_eq!(cat, "ambience");
        let (name, _, cat, _, meta) = classified("", "LICENSE.pdf");
        assert_eq!((name.as_str(), cat, meta), ("LICENSE.pdf", "doc", true));
    }

    #[test]
    fn copies_keep_the_shortest_path() {
        let d = keepers(
            [
                (1, "POLYGON_Generic (1).zip".to_string(), 10, "POLYGON_Generic.zip".to_string()),
                (2, "POLYGON_Generic.zip".to_string(), 10, "POLYGON_Generic.zip".to_string()),
                (3, "Other.zip".to_string(), 10, "Other.zip".to_string()),
            ]
            .into_iter(),
        );
        assert_eq!(d, vec![(1, 2)]);
    }

    #[test]
    fn walk_skips_symlinks_and_reads_nothing() {
        let dir = std::env::temp_dir().join(format!("kk-walk-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("Pack/Sub")).unwrap();
        std::fs::write(dir.join("Pack/Sub/a.wav"), b"1234").unwrap();
        std::fs::write(dir.join("top.png"), b"1").unwrap();
        std::os::unix::fs::symlink(dir.join("Pack"), dir.join("link")).unwrap();
        let (files, errors) = walk(&dir);
        let rels: Vec<&str> = files.iter().map(|f| f.rel.as_str()).collect();
        assert_eq!(rels, ["Pack/Sub/a.wav", "top.png"]);
        assert!(errors.is_empty());
        assert_eq!(files[0].size, 4);
        assert_eq!(folder_pack("Pack/Sub/a.wav"), "Pack");
        assert_eq!(folder_pack("top.png"), "");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
