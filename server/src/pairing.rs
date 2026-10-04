//! One-line install of the runner on a PC (F6): the web app asks for a short
//! pairing code, the PC runs `curl -fsSL <server>/install.sh | sh -s -- CODE`,
//! and the script trades the code for a machine token. Codes live 15 minutes,
//! work once, and only their hash is stored.

use axum::{Extension, Json, extract::{Path, State}, http::{HeaderMap, StatusCode, header}, response::{IntoResponse, Response}};
use serde::Deserialize;
use serde_json::{Value, json};
use crate::{AppState, auth::User, error::{ApiError, ApiResult}, util};

/// Allowed characters for pairing codes. Module-level so tests can use it.
const CHARS: &[u8] = b"ABCDEFGHJKMNPQRSTUVWXYZ23456789";

/// Generates a pairing code: 8 chars from allowed set, formatted `XXXX-XXXX`.
pub fn new_code() -> String {
    let mut s = String::with_capacity(9);
    for _ in 0..8 {
        s.push(CHARS[(rand::random::<u32>() as usize) % CHARS.len()] as char);
    }
    format!("{}-{}", &s[..4], &s[4..])
}

/// Normalizes a code: uppercase, keep only ASCII letters and digits.
pub fn normalize(code: &str) -> String {
    code.chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Helper to get an RFC3339 timestamp for "now + m minutes".
fn in_minutes(m: i64) -> String {
    let now = time::OffsetDateTime::now_utc();
    let then = now + time::Duration::minutes(m);
    then.format(&time::format_description::well_known::Rfc3339).unwrap_or_default()
}

#[derive(Deserialize)]
pub struct CodeBody {
    #[serde(default)]
    pub name: String,
}

/// Creates a new pairing code for the authenticated user.
pub async fn create_code(State(s): State<AppState>, Extension(u): Extension<User>, Json(b): Json<CodeBody>) -> ApiResult<Json<Value>> {
    let now = util::now();
    
    // Delete expired codes for this user
    sqlx::query("DELETE FROM pair_codes WHERE user_id = ? AND expires_at < ?")
        .bind(&u.id)
        .bind(&now)
        .execute(&s.db)
        .await?;

    let code = new_code();
    let normalized = normalize(&code);
    let hash = util::sha256_hex(&normalized);
    
    // Insert the new code
    let expires = in_minutes(15);
    sqlx::query("INSERT INTO pair_codes (code_hash, user_id, name, expires_at) VALUES (?, ?, ?, ?)")
        .bind(&hash)
        .bind(&u.id)
        .bind(b.name.trim().chars().take(60).collect::<String>())
        .bind(&expires)
        .execute(&s.db)
        .await?;

    Ok(Json(json!({
        "code": code,
        "expiresAt": expires
    })))
}

#[derive(Deserialize)]
pub struct PairBody {
    pub code: String,
    #[serde(default)]
    pub hostname: String,
}

/// Pairs a machine using a pairing code. No authentication required.
pub async fn pair(State(s): State<AppState>, Json(b): Json<PairBody>) -> ApiResult<(StatusCode, Json<Value>)> {
    let normalized = normalize(&b.code);
    let hash = util::sha256_hex(&normalized);
    let now = util::now();

    // Look up the code
    let row: Option<(String, String)> = sqlx::query_as::<_, (String, String)>(
        "SELECT user_id, name FROM pair_codes WHERE code_hash = ? AND expires_at > ?"
    )
    .bind(&hash)
    .bind(&now)
    .fetch_optional(&s.db)
    .await?;

    let Some((user_id, code_name)) = row else {
        return Err(ApiError::BadRequest("That pairing code is wrong or has expired. Make a new one in Kompanion.".into()));
    };

    // Delete the used code
    sqlx::query("DELETE FROM pair_codes WHERE code_hash = ?")
        .bind(&hash)
        .execute(&s.db)
        .await?;

    // Determine machine name: web app hostname wins, then code name, then default
    let name = if !b.hostname.is_empty() {
        b.hostname.trim().chars().take(60).collect::<String>()
    } else if !code_name.is_empty() {
        code_name
    } else {
        "My computer".to_string()
    };

    // Create machine
    let id = util::new_id();
    let token = format!("kkr_{}", util::random_token());
    let token_hash = util::sha256_hex(&token);
    
    sqlx::query("INSERT INTO machines (id, user_id, name, token_hash, created_at) VALUES (?, ?, ?, ?, ?)")
        .bind(&id)
        .bind(&user_id)
        .bind(&name)
        .bind(token_hash)
        .bind(&now)
        .execute(&s.db)
        .await?;

    s.bus.send(&user_id, crate::events::Event::Changed { what: "machines", machine_id: None });

    Ok((StatusCode::CREATED, Json(json!({
        "machineId": id,
        "name": name,
        "token": token
    }))))
}

/// Finds the newest runner binary in a directory.
pub fn newest_runner(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let mut candidates: Vec<_> = std::fs::read_dir(dir).ok()?.flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_file() { return None; }
            let name = path.file_name()?.to_str()?;
            // Match kompanion-runner-<version>-x86_64-linux-musl
            if name.starts_with("kompanion-runner-") && name.ends_with("-x86_64-linux-musl") {
                let version_part = &name["kompanion-runner-".len()..name.len()-"-x86_64-linux-musl".len()];
                // Split by dots and parse as u64
                let parts: Vec<u64> = version_part.split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect();
                Some((path, parts))
            } else {
                None
            }
        })
        .collect();

    if candidates.is_empty() {
        return None;
    }

    // Sort by version descending and take the first
    candidates.sort_by(|a, b| b.1.cmp(&a.1));
    Some(candidates.first()?.0.clone())
}

/// Serves runner binaries or their checksums.
pub async fn download(State(s): State<AppState>, Path(file): Path<String>) -> Response {
    let not_found = || (StatusCode::NOT_FOUND, "Not found").into_response();
    let Some(newest) = newest_runner(&s.config.runner_dir) else { return not_found() };
    let Some(name) = newest.file_name().and_then(|n| n.to_str()).map(str::to_string) else { return not_found() };
    match file.as_str() {
        "kompanion-runner" => match tokio::fs::read(&newest).await {
            Ok(bytes) => ([(header::CONTENT_TYPE, "application/octet-stream")], bytes).into_response(),
            Err(_) => not_found(),
        },
        "kompanion-runner.sha256" => match tokio::fs::read_to_string(s.config.runner_dir.join(format!("{name}.sha256"))).await {
            Ok(text) => ([(header::CONTENT_TYPE, "text/plain")], text.replace(&name, "kompanion-runner")).into_response(),
            Err(_) => not_found(),
        },
        _ => not_found(),
    }
}

/// Serves the install script with the server URL injected.
pub async fn install_script(headers: HeaderMap) -> Response {
    let raw = headers
        .get("x-forwarded-host")
        .or_else(|| headers.get(header::HOST))
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost");
    let host: String = raw.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':')).take(253).collect();
    let script = include_str!("../assets/install.sh").replace("{{SERVER}}", &format!("https://{host}"));
    ([(header::CONTENT_TYPE, "text/x-shellscript; charset=utf-8")], script).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process;

    fn temp_dir() -> std::path::PathBuf {
        let mut d = std::env::temp_dir();
        d.push(format!("kk-pair-{}", process::id()));
        fs::create_dir_all(&d).ok();
        d
    }

    #[test]
    fn test_new_code_format() {
        let code = new_code();
        assert_eq!(code.len(), 9);
        assert_eq!(code.chars().nth(4), Some('-'));
        for c in code.chars() {
            if c != '-' {
                assert!(CHARS.contains(&(c as u8)));
            }
        }
    }

    #[test]
    fn test_normalize() {
        assert_eq!(normalize("ab cd-ef gh"), "ABCDEFGH");
        assert_eq!(normalize("ABCD-EFGH"), "ABCDEFGH");
    }

    #[test]
    fn test_newest_runner() {
        let dir = temp_dir();
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        
        let v1 = format!("kompanion-runner-0.9.2-x86_64-linux-musl");
        let v2 = format!("kompanion-runner-0.10.0-x86_64-linux-musl");
        let sha = format!("kompanion-runner-0.10.0-x86_64-linux-musl.sha256");

        fs::write(dir.join(&v1), "dummy").unwrap();
        fs::write(dir.join(&v2), "dummy").unwrap();
        fs::write(dir.join(&sha), "dummy").unwrap();

        let result = newest_runner(&dir).unwrap();
        assert_eq!(result.file_name().unwrap().to_str().unwrap(), &v2);

        // Cleanup
        let _ = fs::remove_dir_all(&dir);
    }
}
