//! Previews, made on this server's CPU, one file at a time at the lowest priority
//! (`nice -n 19`, `ionice -c 3`): images get a 256 px and a 1024 px WebP, audio a
//! 30-second Opus clip (WebM) plus 64 waveform levels. A zip entry is unpacked into
//! the preview folder's tmp/ for ffmpeg and deleted right after; the library itself
//! is only read. Previews live in Kompanion's private data folder and are served to
//! signed-in users only (see `serve`).

use std::{
    collections::{HashMap, HashSet, VecDeque},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{LazyLock, Mutex},
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use serde::Serialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use tokio::{process::Command, sync::Notify};

use super::{scan, zipindex};
use crate::events::{Bus, Event};

pub const IMAGE_EXTS: &[&str] = &["png", "jpg", "jpeg", "tga", "bmp", "gif", "webp", "tif", "tiff", "psd", "dds", "exr", "hdr"];
pub const AUDIO_EXTS: &[&str] = &["wav", "ogg", "mp3", "flac", "aif", "aiff", "m4a", "aac", "opus", "wma"];
/// Bigger entries are not unpacked for a preview.
const MAX_ENTRY: u64 = 256 * 1024 * 1024;
const CLIP_SECONDS: &str = "30";
const PEAKS: usize = 64;
const BATCH: i64 = 24;

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub running: bool,
    /// Previews still to make.
    pub todo: i64,
    /// Made since the server started.
    pub made: i64,
    pub failed: i64,
}

pub static PROGRESS: LazyLock<Mutex<Progress>> = LazyLock::new(Default::default);
/// Assets someone is looking at without a preview yet: made first.
static PRIORITY: LazyLock<Mutex<VecDeque<i64>>> = LazyLock::new(Default::default);
static WAKE: LazyLock<Notify> = LazyLock::new(Notify::new);

/// Wakes the worker (after a scan, or when someone waits for a preview).
pub fn wake() {
    WAKE.notify_one();
}

pub fn prioritise(id: i64) {
    let mut q = PRIORITY.lock().unwrap();
    if !q.contains(&id) {
        q.push_front(id);
        q.truncate(500);
    }
    drop(q);
    wake();
}

/// Where previews live: next to the database, in its private data folder.
pub fn dir(database: &Path) -> PathBuf {
    database.parent().unwrap_or(Path::new(".")).join("asset-previews")
}

/// t: 256 px image, l: 1024 px image, a: audio clip.
pub fn file(dir: &Path, id: i64, kind: char) -> PathBuf {
    let ext = if kind == 'a' { "webm" } else { "webp" };
    dir.join(format!("{:02x}", id % 256)).join(format!("{id}-{kind}.{ext}"))
}

fn sql_list(exts: &[&str]) -> String {
    exts.iter().map(|e| format!("'{e}'")).collect::<Vec<_>>().join(", ")
}

/// Rows that still need a preview (and can get one).
fn todo_where() -> String {
    format!(
        "a.preview_state IS NULL AND a.missing_since IS NULL AND a.category <> 'junk' AND a.size > 0
         AND a.ext IN ({}, {}) AND a.container NOT LIKE '%.unitypackage'",
        sql_list(IMAGE_EXTS),
        sql_list(AUDIO_EXTS)
    )
}

fn publish(bus: &Bus, ids: Vec<i64>) {
    let p = PROGRESS.lock().unwrap().clone();
    bus.send_all(Event::Assets { scan: None, previews: Some(json!({ "ids": ids, "progress": p })) });
}

/// ffmpeg and ffprobe at the lowest CPU and disk priority.
fn low(program: &str, ionice: bool) -> Command {
    let mut c = Command::new("nice");
    c.args(["-n", "19"]);
    if ionice {
        c.args(["ionice", "-c", "3"]);
    }
    c.arg(program).stdin(Stdio::null()).kill_on_drop(true);
    c
}

/// Runs the worker forever. Waits while a scan runs and sleeps when there is nothing to do.
pub fn spawn(db: SqlitePool, bus: Bus, root: PathBuf, dir: PathBuf) {
    tokio::spawn(async move {
        let ionice = Command::new("ionice").args(["-c", "3", "true"]).status().await.is_ok_and(|s| s.success());
        let have_ffmpeg = Command::new("ffmpeg").arg("-version").stdout(Stdio::null()).status().await.is_ok_and(|s| s.success());
        if !have_ffmpeg {
            tracing::warn!("ffmpeg not found: asset previews off");
            return;
        }
        let tmp = dir.join("tmp");
        let _ = std::fs::remove_dir_all(&tmp);
        if let Err(e) = std::fs::create_dir_all(&tmp) {
            tracing::error!(error = %e, path = %tmp.display(), "asset previews: no folder");
            return;
        }
        loop {
            if scan::PROGRESS.lock().unwrap().running {
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
            match batch(&db, &bus, &root, &dir, ionice).await {
                Ok(0) => {
                    PROGRESS.lock().unwrap().running = false;
                    publish(&bus, vec![]);
                    let _ = tokio::time::timeout(Duration::from_secs(600), WAKE.notified()).await;
                }
                Ok(_) => {}
                Err(e) => {
                    tracing::error!(error = ?e, "asset previews");
                    tokio::time::sleep(Duration::from_secs(60)).await;
                }
            }
        }
    });
}

#[derive(sqlx::FromRow, Clone)]
struct Todo {
    id: i64,
    container: String,
    path: String,
    ext: String,
}

/// Makes previews for up to BATCH assets: the wanted ones first, then in grid order.
async fn batch(db: &SqlitePool, bus: &Bus, root: &Path, dir: &Path, ionice: bool) -> Result<usize> {
    let wanted: Vec<i64> = PRIORITY.lock().unwrap().drain(..).collect();
    let mut rows: Vec<Todo> = vec![];
    if !wanted.is_empty() {
        let ids = wanted.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
        rows = sqlx::query_as(&format!("SELECT a.id, a.container, a.path, a.ext FROM asset a WHERE a.id IN ({ids}) AND {}", todo_where()))
            .fetch_all(db)
            .await?;
    }
    let todo: i64 = sqlx::query_scalar(&format!("SELECT COUNT(*) FROM asset a WHERE {}", todo_where())).fetch_one(db).await?;
    {
        let mut p = PROGRESS.lock().unwrap();
        p.todo = todo;
        p.running = todo > 0;
    }
    if rows.is_empty() {
        rows = sqlx::query_as(&format!(
            "SELECT a.id, a.container, a.path, a.ext FROM asset a JOIN asset_pack p ON p.id = a.pack_id
             WHERE {} ORDER BY p.name COLLATE NOCASE, a.pack_id, a.path COLLATE NOCASE LIMIT {BATCH}",
            todo_where()
        ))
        .fetch_all(db)
        .await?;
    }
    if rows.is_empty() {
        return Ok(0);
    }
    // One directory read per pack file in the batch.
    let mut entries: HashMap<String, Result<HashMap<String, zipindex::Entry>, String>> = HashMap::new();
    for c in rows.iter().map(|r| r.container.clone()).filter(|c| !c.is_empty()).collect::<HashSet<_>>() {
        let path = root.join(&c);
        let listed = tokio::task::spawn_blocking(move || zipindex::zip_entries(&path)).await?;
        entries.insert(c, listed.map(|l| l.into_iter().map(|e| (e.name.clone(), e)).collect()).map_err(|e| e.to_string()));
    }
    let n = rows.len();
    let mut done = vec![];
    let mut last_publish = Instant::now();
    for r in rows {
        let result = make(&r, root, dir, &entries, ionice).await;
        let ok = result.is_ok();
        save(db, r.id, result).await?;
        {
            let mut p = PROGRESS.lock().unwrap();
            if ok { p.made += 1 } else { p.failed += 1 }
            p.todo = (p.todo - 1).max(0);
        }
        done.push(r.id);
        if last_publish.elapsed() > Duration::from_secs(2) {
            publish(bus, std::mem::take(&mut done));
            last_publish = Instant::now();
        }
        // A scan or a wanted asset goes first.
        if scan::PROGRESS.lock().unwrap().running || !PRIORITY.lock().unwrap().is_empty() {
            break;
        }
    }
    publish(bus, done);
    Ok(n)
}

/// What a preview run found out.
#[derive(Debug, Default, PartialEq)]
pub struct Made {
    pub kind: &'static str,
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub has_alpha: Option<bool>,
    pub duration: Option<f64>,
    pub sample_rate: Option<i64>,
    pub channels: Option<i64>,
    pub peaks: Option<String>,
}

/// Can't be previewed at all (not an error worth showing as one).
#[derive(Debug)]
struct NotPreviewable(String);
impl std::fmt::Display for NotPreviewable {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for NotPreviewable {}

async fn save(db: &SqlitePool, id: i64, result: Result<Made>) -> Result<()> {
    match result {
        Ok(m) => {
            sqlx::query(
                "UPDATE asset SET preview_state = 'ok', preview_kind = ?, preview_v = preview_v + 1, preview_error = NULL,
                 width = ?, height = ?, has_alpha = ?, duration_s = ?, sample_rate = ?, channels = ?, peaks = ? WHERE id = ?",
            )
            .bind(m.kind)
            .bind(m.width)
            .bind(m.height)
            .bind(m.has_alpha)
            .bind(m.duration)
            .bind(m.sample_rate)
            .bind(m.channels)
            .bind(m.peaks)
            .bind(id)
            .execute(db)
            .await?;
        }
        Err(e) => {
            let state = if e.downcast_ref::<NotPreviewable>().is_some() { "none" } else { "error" };
            let msg: String = format!("{e:#}").chars().take(300).collect();
            sqlx::query("UPDATE asset SET preview_state = ?, preview_error = ? WHERE id = ?")
                .bind(state)
                .bind(msg)
                .bind(id)
                .execute(db)
                .await?;
        }
    }
    Ok(())
}

async fn make(
    r: &Todo,
    root: &Path,
    dir: &Path,
    entries: &HashMap<String, Result<HashMap<String, zipindex::Entry>, String>>,
    ionice: bool,
) -> Result<Made> {
    let out = file(dir, r.id, 't');
    std::fs::create_dir_all(out.parent().unwrap())?;
    // The input: the loose file itself, or the zip entry unpacked into tmp/.
    let (input, temp) = if r.container.is_empty() {
        (root.join(&r.path), None)
    } else {
        let list = entries.get(&r.container).context("pack file not listed")?;
        let list = list.as_ref().map_err(|e| anyhow::anyhow!("pack file unreadable: {e}"))?;
        let e = list.get(&r.path).context("entry no longer in its pack file")?.clone();
        if e.size > MAX_ENTRY {
            bail!(NotPreviewable(format!("too big to preview ({} MB)", e.size / 1024 / 1024)));
        }
        let tmp = dir.join("tmp").join(format!("{}.{}", r.id, r.ext));
        let zip = root.join(&r.container);
        let t2 = tmp.clone();
        tokio::task::spawn_blocking(move || -> std::io::Result<u64> {
            let mut f = std::io::BufWriter::new(std::fs::File::create(&t2)?);
            let n = zipindex::copy_entry(&zip, &e, &mut f, MAX_ENTRY)?;
            std::io::Write::flush(&mut f)?;
            Ok(n)
        })
        .await?
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::Unsupported { anyhow::Error::new(NotPreviewable(e.to_string())) } else { e.into() }
        })?;
        (tmp.clone(), Some(tmp))
    };
    let result = if IMAGE_EXTS.contains(&r.ext.as_str()) {
        image(&input, dir, r.id, ionice).await
    } else {
        audio(&input, dir, r.id, ionice).await
    };
    if let Some(t) = temp {
        let _ = std::fs::remove_file(t);
    }
    result
}

/// Runs a low-priority command with a time limit; returns stdout, or stderr's last line as the error.
async fn run(mut c: Command, limit: Duration) -> Result<Vec<u8>> {
    let child = c.stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().context("starting ffmpeg")?;
    let out = tokio::time::timeout(limit, child.wait_with_output()).await.context("ffmpeg took too long")??;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        bail!("{}", err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("ffmpeg failed").trim());
    }
    Ok(out.stdout)
}

async fn probe(input: &Path, entries: &str, ionice: bool) -> Result<Value> {
    let mut c = low("ffprobe", ionice);
    c.args(["-v", "error", "-show_entries", entries, "-of", "json"]).arg(input);
    Ok(serde_json::from_slice(&run(c, Duration::from_secs(60)).await?)?)
}

pub fn has_alpha(pix_fmt: &str) -> bool {
    ["rgba", "bgra", "argb", "abgr", "ya", "yuva", "gbrap", "rgba64", "bgra64"].iter().any(|p| pix_fmt.starts_with(p))
}

async fn image(input: &Path, dir: &Path, id: i64, ionice: bool) -> Result<Made> {
    let p = probe(input, "stream=width,height,pix_fmt", ionice).await?;
    let s = p["streams"].get(0).context("no picture in the file")?;
    let (t, l) = (file(dir, id, 't'), file(dir, id, 'l'));
    let fit = |n: u32| format!("scale=w='min({n},iw)':h='min({n},ih)':force_original_aspect_ratio=decrease:flags=area");
    let mut c = low("ffmpeg", ionice);
    c.args(["-v", "error", "-nostdin", "-y", "-i"]).arg(input).args([
        "-filter_complex",
        &format!("[0:v]split=2[a][b];[a]{}[t];[b]{}[l]", fit(256), fit(1024)),
        "-map", "[t]", "-frames:v", "1", "-c:v", "libwebp", "-quality", "80",
    ])
    .arg(&t)
    .args(["-map", "[l]", "-frames:v", "1", "-c:v", "libwebp", "-quality", "82"])
    .arg(&l);
    run(c, Duration::from_secs(120)).await?;
    Ok(Made {
        kind: "image",
        width: s["width"].as_i64(),
        height: s["height"].as_i64(),
        has_alpha: s["pix_fmt"].as_str().map(has_alpha),
        ..Default::default()
    })
}

async fn audio(input: &Path, dir: &Path, id: i64, ionice: bool) -> Result<Made> {
    let p = probe(input, "format=duration:stream=codec_type,sample_rate,channels", ionice).await?;
    let s = p["streams"]
        .as_array()
        .and_then(|a| a.iter().find(|s| s["codec_type"] == "audio"))
        .context("no sound in the file")?;
    let channels = s["channels"].as_i64();
    let a = file(dir, id, 'a');
    // One pass: the Opus clip, and 4 kHz mono samples on stdout for the waveform.
    let mut c = low("ffmpeg", ionice);
    c.args(["-v", "error", "-nostdin", "-y", "-i"]).arg(input).args([
        "-t", CLIP_SECONDS, "-map", "0:a:0", "-vn", "-ac", if channels == Some(1) { "1" } else { "2" },
        "-c:a", "libopus", "-b:a", "96k", "-f", "webm",
    ])
    .arg(&a)
    .args(["-t", CLIP_SECONDS, "-map", "0:a:0", "-ac", "1", "-ar", "4000", "-f", "s16le", "pipe:1"]);
    let raw = run(c, Duration::from_secs(180)).await?;
    let samples: Vec<i16> = raw.chunks_exact(2).map(|b| i16::from_le_bytes([b[0], b[1]])).collect();
    Ok(Made {
        kind: "audio",
        duration: p["format"]["duration"].as_str().and_then(|d| d.parse().ok()),
        sample_rate: s["sample_rate"].as_str().and_then(|r| r.parse().ok()),
        channels,
        peaks: Some(peaks(&samples, PEAKS)),
        ..Default::default()
    })
}

/// `n` levels, 0..=35 as base-36 characters, loudest bucket = "z". Square root, so quiet
/// parts still show.
pub fn peaks(samples: &[i16], n: usize) -> String {
    if samples.is_empty() {
        return "0".repeat(n);
    }
    let per = samples.len().div_ceil(n).max(1);
    let maxes: Vec<f64> = samples.chunks(per).map(|c| c.iter().map(|s| s.unsigned_abs() as f64).fold(0.0, f64::max)).collect();
    let top = maxes.iter().cloned().fold(1.0, f64::max);
    let mut out: String = maxes
        .iter()
        .map(|m| std::char::from_digit(((m / top).sqrt() * 35.0).round() as u32, 36).unwrap_or('0'))
        .collect();
    while out.len() < n {
        out.push('0');
    }
    out
}

/// GET /asset-preview/{id}-{t|l|a}.{webp|webm}: signed-in users only, cached privately
/// (the URL carries the preview version, so a new preview gets a new URL).
pub async fn serve(
    axum::extract::State(s): axum::extract::State<crate::AppState>,
    axum::extract::Path(name): axum::extract::Path<String>,
    req: axum::extract::Request,
) -> axum::response::Response {
    use axum::{http::{HeaderValue, StatusCode, header}, response::IntoResponse};
    use tower::ServiceExt;
    match crate::auth::current_user(&s, req.headers()).await {
        Ok(Some(_)) => {}
        Ok(None) => return StatusCode::UNAUTHORIZED.into_response(),
        Err(e) => return e.into_response(),
    }
    let parsed = name.split_once('.').and_then(|(stem, ext)| {
        let (id, kind) = stem.split_once('-')?;
        let kind = kind.chars().next().filter(|k| matches!((k, ext), ('t' | 'l', "webp") | ('a', "webm")))?;
        Some((id.parse::<i64>().ok()?, kind))
    });
    let Some((id, kind)) = parsed else { return StatusCode::NOT_FOUND.into_response() };
    let path = file(&dir(&s.config.database), id, kind);
    if !path.is_file() {
        prioritise(id);
        return StatusCode::NOT_FOUND.into_response();
    }
    let mut res = match tower_http::services::ServeFile::new(path).oneshot(req).await {
        Ok(r) => r.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let h = res.headers_mut();
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=31536000, immutable"));
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'none'; sandbox"));
    if kind == 'a' {
        h.insert(header::CONTENT_TYPE, HeaderValue::from_static("audio/webm"));
    }
    res
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waveform_levels() {
        let quiet_then_loud: Vec<i16> = (0..400).map(|i| if i < 200 { 100 } else { 10_000 }).collect();
        let p = peaks(&quiet_then_loud, 4);
        assert_eq!(p.len(), 4);
        assert_eq!(&p[2..], "zz");
        assert!(p.as_bytes()[0] < b'9', "{p}");
        assert_eq!(peaks(&[], 3), "000");
    }

    #[test]
    fn alpha_from_pixel_format() {
        assert!(has_alpha("rgba") && has_alpha("ya8") && has_alpha("yuva420p"));
        assert!(!has_alpha("pal8") && !has_alpha("rgb24") && !has_alpha("gray"));
    }

    #[test]
    fn preview_paths() {
        let d = Path::new("/data/asset-previews");
        assert_eq!(file(d, 258, 't'), Path::new("/data/asset-previews/02/258-t.webp"));
        assert_eq!(file(d, 7, 'a'), Path::new("/data/asset-previews/07/7-a.webm"));
        assert_eq!(dir(Path::new("/data/kompanion.db")), Path::new("/data/asset-previews"));
    }

    /// Makes real previews when ffmpeg is installed (the server image has it; CI may not).
    #[tokio::test]
    async fn ffmpeg_makes_image_and_audio_previews() {
        if std::process::Command::new("ffmpeg").arg("-version").output().is_err() {
            eprintln!("ffmpeg not installed: skipped");
            return;
        }
        let d = std::env::temp_dir().join(format!("kk-prev-{}", std::process::id()));
        std::fs::create_dir_all(d.join("01")).unwrap();
        let png = d.join("in.png");
        let wav = d.join("in.wav");
        let make_input = |args: &[&str]| assert!(std::process::Command::new("ffmpeg").args(["-v", "error", "-y"]).args(args).status().unwrap().success());
        make_input(&["-f", "lavfi", "-i", "color=c=0x5c398e:s=1600x900", "-frames:v", "1", png.to_str().unwrap()]);
        make_input(&["-f", "lavfi", "-i", "sine=frequency=440:duration=2", wav.to_str().unwrap()]);
        let m = image(&png, &d, 1, false).await.unwrap();
        assert_eq!((m.kind, m.width, m.height, m.has_alpha), ("image", Some(1600), Some(900), Some(false)));
        assert!(file(&d, 1, 't').is_file() && file(&d, 1, 'l').is_file());
        let m = audio(&wav, &d, 1, false).await.unwrap();
        assert_eq!((m.kind, m.channels), ("audio", Some(1)));
        assert!((m.duration.unwrap() - 2.0).abs() < 0.1, "{:?}", m.duration);
        assert_eq!(m.peaks.as_ref().unwrap().len(), PEAKS);
        assert!(file(&d, 1, 'a').metadata().unwrap().len() > 1000);
        std::fs::remove_dir_all(&d).unwrap();
    }
}
