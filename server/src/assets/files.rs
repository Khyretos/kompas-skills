//! The original files for the in-app viewers (milestone 6): any library file or zip entry,
//! to signed-in users only, with Range support and private caching. A zip entry is unpacked
//! once into the private cache (`<previews>/files/`, oldest dropped past CACHE_BYTES), never
//! into the library. `near` resolves a file a model refers to (a texture, an .mtl, a .bin)
//! in the same pack: by its path relative to the model, else by its file name.

use std::path::{Path as FsPath, PathBuf};

use axum::{
    extract::{Path, Query, Request, State},
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Deserialize;
use tower::ServiceExt;

use super::{preview, zipindex};
use crate::AppState;

/// Unpacked entries kept on disk for the viewers.
const CACHE_BYTES: u64 = 3 * 1024 * 1024 * 1024;
/// Bigger files aren't served to the browser.
const MAX_FILE: u64 = 512 * 1024 * 1024;

#[derive(sqlx::FromRow, Clone)]
struct Row {
    id: i64,
    pack_id: i64,
    container: String,
    path: String,
    ext: String,
    size: i64,
}

/// Content type by extension; anything else is a download-only octet stream.
pub fn content_type(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "mp4" | "m4v" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "ogv" => "video/ogg",
        "mp3" => "audio/mpeg",
        "ogg" | "oga" => "audio/ogg",
        "wav" => "audio/wav",
        "flac" => "audio/flac",
        "m4a" | "aac" => "audio/mp4",
        "opus" => "audio/opus",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "gltf" | "json" => "application/json",
        "glb" => "model/gltf-binary",
        "txt" | "md" | "cs" | "py" | "lua" | "shader" | "hlsl" | "glsl" | "cginc" | "mtl" | "obj" | "bvh" | "dae" | "ini" | "cfg"
        | "csv" | "xml" | "yaml" | "yml" | "toml" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

async fn row(s: &AppState, id: i64) -> Option<Row> {
    sqlx::query_as("SELECT id, pack_id, container, path, ext, size FROM asset WHERE id = ? AND missing_since IS NULL")
        .bind(id)
        .fetch_optional(&s.db)
        .await
        .ok()
        .flatten()
}

/// The cache file for an unpacked entry.
fn cached(dir: &FsPath, id: i64, size: i64, ext: &str) -> PathBuf {
    // The size is in the name, so a pack that changed doesn't serve its old entry.
    dir.join("files").join(format!("{id}-{size}.{}", ext.to_ascii_lowercase()))
}

/// Drops the oldest unpacked files until the cache fits.
fn trim_cache(files: &FsPath) {
    let Ok(rd) = std::fs::read_dir(files) else { return };
    let mut all: Vec<(std::time::SystemTime, u64, PathBuf)> = rd
        .flatten()
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((m.accessed().or_else(|_| m.modified()).ok()?, m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = all.iter().map(|x| x.1).sum();
    all.sort();
    for (_, len, path) in all {
        if total <= CACHE_BYTES {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}

/// The file on disk: the loose file itself, or the zip entry unpacked into the cache.
async fn on_disk(s: &AppState, r: &Row) -> Result<PathBuf, StatusCode> {
    let root = super::root().ok_or(StatusCode::NOT_FOUND)?;
    if r.size as u64 > MAX_FILE {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }
    if r.container.is_empty() {
        let p = root.join(&r.path);
        return if p.is_file() { Ok(p) } else { Err(StatusCode::NOT_FOUND) };
    }
    if !r.container.to_ascii_lowercase().ends_with(".zip") {
        // Unity packages are gzipped tars: not unpacked for the viewers.
        return Err(StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }
    let dir = preview::dir(&s.config.database);
    let out = cached(&dir, r.id, r.size, &r.ext);
    if out.is_file() {
        return Ok(out);
    }
    let zip = root.join(&r.container);
    let (name, out2, files) = (r.path.clone(), out.clone(), dir.join("files"));
    tokio::task::spawn_blocking(move || -> std::io::Result<()> {
        std::fs::create_dir_all(&files)?;
        let entry = zipindex::zip_entries(&zip)?
            .into_iter()
            .find(|e| e.name == name)
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "entry no longer in its pack file"))?;
        let tmp = files.join(format!(".{}-{}", std::process::id(), out2.file_name().unwrap_or_default().to_string_lossy()));
        {
            let mut f = std::io::BufWriter::new(std::fs::File::create(&tmp)?);
            zipindex::copy_entry(&zip, &entry, &mut f, MAX_FILE)?;
            std::io::Write::flush(&mut f)?;
        }
        std::fs::rename(&tmp, &out2)?;
        trim_cache(&files);
        Ok(())
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|e| {
        tracing::warn!(id = r.id, error = %e, "asset file");
        match e.kind() {
            std::io::ErrorKind::NotFound => StatusCode::NOT_FOUND,
            std::io::ErrorKind::Unsupported | std::io::ErrorKind::InvalidData => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            _ => StatusCode::BAD_GATEWAY,
        }
    })?;
    Ok(out)
}

async fn signed_in(s: &AppState, headers: &axum::http::HeaderMap) -> Result<(), Response> {
    match crate::auth::current_user(s, headers).await {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(StatusCode::UNAUTHORIZED.into_response()),
        Err(e) => Err(e.into_response()),
    }
}

async fn send(s: &AppState, r: &Row, req: Request) -> Response {
    let path = match on_disk(s, r).await {
        Ok(p) => p,
        Err(code) => return code.into_response(),
    };
    let mut res = match tower_http::services::ServeFile::new(path).oneshot(req).await {
        Ok(r) => r.into_response(),
        Err(_) => return StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    };
    let h = res.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type(&r.ext)));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("private, max-age=86400"));
    h.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'none'; sandbox"));
    h.insert(header::X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    res
}

/// GET /asset-file/{id}/{name}: the asset's own file. `name` is only there so loaders see
/// the file's extension in the URL.
pub async fn serve(State(s): State<AppState>, Path((id, _name)): Path<(i64, String)>, req: Request) -> Response {
    if let Err(r) = signed_in(&s, req.headers()).await {
        return r;
    }
    match row(&s, id).await {
        Some(r) => send(&s, &r, req).await,
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[derive(Deserialize)]
pub struct Near {
    name: String,
}

/// Joins `rel` to the folder of `base` inside a pack: "../Textures/a.png" from
/// "Models/SM_Tree.fbx" is "Textures/a.png". None when it climbs out of the pack.
pub fn join_rel(base: &str, rel: &str) -> Option<String> {
    let rel = rel.replace('\\', "/");
    let mut parts: Vec<&str> = base.split('/').collect();
    parts.pop();
    for seg in rel.split('/') {
        match seg {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            s => parts.push(s),
        }
    }
    Some(parts.join("/"))
}

/// The file a model refers to, from the same pack: by relative path, then by file name,
/// then by stem as a picture (a model naming "Tree.psd" gets "Tree.png"), then the first
/// numbered variant ("Tree.png" gets "Tree_01.png").
async fn resolve(s: &AppState, base: &Row, name: &str) -> Option<Row> {
    let wanted = name.replace('\\', "/");
    if let Some(p) = join_rel(&base.path, &wanted).filter(|p| !wanted.contains(':')) {
        let hit: Option<Row> = sqlx::query_as(
            "SELECT id, pack_id, container, path, ext, size FROM asset
             WHERE pack_id = ? AND container = ? AND path = ? COLLATE NOCASE AND missing_since IS NULL",
        )
        .bind(base.pack_id)
        .bind(&base.container)
        .bind(&p)
        .fetch_optional(&s.db)
        .await
        .ok()
        .flatten();
        if hit.is_some() {
            return hit;
        }
    }
    let file = wanted.rsplit('/').next().unwrap_or(&wanted).to_string();
    let by_name: Option<Row> = sqlx::query_as(
        "SELECT id, pack_id, container, path, ext, size FROM asset
         WHERE pack_id = ? AND container = ? AND name = ? COLLATE NOCASE AND missing_since IS NULL
         ORDER BY length(path) LIMIT 1",
    )
    .bind(base.pack_id)
    .bind(&base.container)
    .bind(&file)
    .fetch_optional(&s.db)
    .await
    .ok()
    .flatten();
    if by_name.is_some() {
        return by_name;
    }
    let stem = file.rsplit_once('.').map(|(s, _)| s).unwrap_or(&file).to_string();
    let same_stem: Option<Row> = sqlx::query_as(
        "SELECT id, pack_id, container, path, ext, size FROM asset
         WHERE pack_id = ? AND container = ? AND missing_since IS NULL AND ext IN ('png', 'jpg', 'jpeg', 'tga')
         AND (name = ? || '.png' COLLATE NOCASE OR name = ? || '.jpg' COLLATE NOCASE OR name = ? || '.tga' COLLATE NOCASE)
         ORDER BY ext = 'png' DESC, length(path) LIMIT 1",
    )
    .bind(base.pack_id)
    .bind(&base.container)
    .bind(&stem)
    .bind(&stem)
    .bind(&stem)
    .fetch_optional(&s.db)
    .await
    .ok()
    .flatten();
    if same_stem.is_some() {
        return same_stem;
    }
    // Synty FBX files name the artist's "PolygonNature.png"; the pack ships "PolygonNature_01.png".
    sqlx::query_as(
        "SELECT id, pack_id, container, path, ext, size FROM asset
         WHERE pack_id = ? AND container = ? AND missing_since IS NULL AND ext IN ('png', 'jpg', 'jpeg', 'tga')
         AND name LIKE ? || '!_%' ESCAPE '!'
         ORDER BY ext = 'png' DESC, length(name), path LIMIT 1",
    )
    .bind(base.pack_id)
    .bind(&base.container)
    .bind(stem.replace('!', "!!").replace('_', "!_").replace('%', "!%"))
    .fetch_optional(&s.db)
    .await
    .ok()
    .flatten()
}

/// GET /asset-file/{id}/near?name=../Textures/a.png: a file the model `id` refers to.
pub async fn near(State(s): State<AppState>, Path(id): Path<i64>, Query(q): Query<Near>, req: Request) -> Response {
    if let Err(r) = signed_in(&s, req.headers()).await {
        return r;
    }
    let Some(base) = row(&s, id).await else { return StatusCode::NOT_FOUND.into_response() };
    match resolve(&s, &base, &q.name).await {
        Some(r) => send(&s, &r, req).await,
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_paths_stay_inside_the_pack() {
        assert_eq!(join_rel("Models/SM_Tree.fbx", "../Textures/a.png").as_deref(), Some("Textures/a.png"));
        assert_eq!(join_rel("Models/SM_Tree.fbx", "tree.mtl").as_deref(), Some("Models/tree.mtl"));
        assert_eq!(join_rel("Models/SM_Tree.fbx", "..\\..\\x.png"), None);
        assert_eq!(join_rel("SM_Tree.obj", "./Tex/b.tga").as_deref(), Some("Tex/b.tga"));
    }

    #[test]
    fn types_for_the_viewers() {
        assert_eq!(content_type("TTF"), "font/ttf");
        assert_eq!(content_type("mp4"), "video/mp4");
        assert_eq!(content_type("fbx"), "application/octet-stream");
        assert!(content_type("shader").starts_with("text/plain"));
    }
}
