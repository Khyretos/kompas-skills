//! Does a folder exist on a computer, and what is in it? Asked through the runner's
//! list_dir job, which reports "no such file or folder" before it checks grants, so it
//! answers even without a grant (a grant is still needed to see what is inside).
//! Used by the Run form (check before Start, folder browser) and by W2 before it plans.
use std::time::Duration;

use axum::{Extension, Json, extract::{Path, State}};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{AppState, access, auth::User, error::{ApiError, ApiResult}};

#[derive(Debug, Clone, PartialEq)]
pub enum Folder {
    /// It exists and may be read: its entries (name, is a folder).
    Ok(Vec<(String, bool)>),
    /// It exists, but no read grant covers it (nothing listed).
    NoGrant,
    Missing,
    NotAFolder,
    /// The computer didn't answer in time (offline, or a slow report interval).
    NoAnswer,
}

/// What list_dir's answer means (state is the job state: done | failed | refused).
pub fn read(state: &str, output: &str) -> Folder {
    if state == "done" {
        let entries = output
            .lines()
            .filter(|l| !l.trim().is_empty() && !l.starts_with("… "))
            .map(|l| match l.strip_suffix('/') {
                Some(d) => (d.to_string(), true),
                None => (l.to_string(), false),
            })
            .collect();
        return Folder::Ok(entries);
    }
    if output.starts_with("no such file or folder") {
        Folder::Missing
    } else if output.starts_with("not a directory") {
        Folder::NotAFolder
    } else if output.starts_with("not granted") || state == "refused" {
        Folder::NoGrant
    } else {
        Folder::NoAnswer
    }
}

/// Ask `machine_id` about `path`, waiting up to `wait`. The runner reports every second
/// while this waits (the same speed-up as an open Machines tab).
pub async fn check(s: &AppState, user_id: &str, machine_id: &str, path: &str, wait: Duration) -> ApiResult<Folder> {
    s.host.watch(user_id);
    let job = access::queue_job(&s.db, machine_id, user_id, &json!({ "tool": "list_dir", "path": path }), None).await?;
    let deadline = tokio::time::Instant::now() + wait;
    while tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(300)).await;
        let row: Option<(String, Option<String>)> =
            sqlx::query_as("SELECT state, result FROM machine_jobs WHERE id = ?").bind(&job).fetch_optional(&s.db).await?;
        if let Some((state, out)) = row
            && matches!(state.as_str(), "done" | "failed" | "refused")
        {
            return Ok(read(&state, &out.unwrap_or_default()));
        }
    }
    // Nobody waits for it any more; don't let it run later.
    let _ = sqlx::query("DELETE FROM machine_jobs WHERE id = ? AND state = 'queued'").bind(&job).execute(&s.db).await;
    Ok(Folder::NoAnswer)
}

#[derive(Deserialize)]
pub struct Body {
    path: String,
}

/// POST /machines/{id}/folder {path}: for the Run form.
pub async fn api(State(s): State<AppState>, Extension(u): Extension<User>, Path(id): Path<String>, Json(b): Json<Body>) -> ApiResult<Json<Value>> {
    let path = b.path.trim().trim_end_matches('/');
    let path = if path.is_empty() { "/" } else { path };
    if !path.starts_with('/') || path.split('/').any(|c| c == "..") {
        return Err(ApiError::BadRequest("Use an absolute folder, like /home/you/projects/app.".into()));
    }
    let name: Option<(String,)> =
        sqlx::query_as("SELECT name FROM machines WHERE id = ? AND user_id = ?").bind(&id).bind(&u.id).fetch_optional(&s.db).await?;
    let Some((name,)) = name else { return Err(ApiError::NotFound) };
    let f = check(&s, &u.id, &id, path, Duration::from_secs(20)).await?;
    Ok(Json(match f {
        Folder::Ok(entries) => json!({
            "state": "ok", "path": path,
            "folders": entries.iter().filter(|e| e.1).map(|e| &e.0).collect::<Vec<_>>(),
            "files": entries.iter().filter(|e| !e.1).count(),
        }),
        Folder::NoGrant => json!({ "state": "nogrant", "path": path, "message": format!("{path} exists on {name}; its contents need a read grant (Access tab).") }),
        Folder::Missing => json!({ "state": "missing", "path": path, "message": format!("Folder not found on {name}: {path}") }),
        Folder::NotAFolder => json!({ "state": "notfolder", "path": path, "message": format!("{path} on {name} is a file, not a folder.") }),
        Folder::NoAnswer => json!({ "state": "noanswer", "path": path, "message": format!("{name} didn't answer within 20 s (offline?).") }),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn list_dir_answers_are_read_right() {
        assert_eq!(read("done", "src/\nnames.py\n"), Folder::Ok(vec![("src".into(), true), ("names.py".into(), false)]));
        assert_eq!(read("failed", "no such file or folder: /home/khyretos/Docker/x"), Folder::Missing);
        assert_eq!(read("refused", "not granted: Read on /home/k"), Folder::NoGrant);
        assert_eq!(read("failed", "not a directory: /etc/hosts"), Folder::NotAFolder);
        assert_eq!(read("failed", "something else"), Folder::NoAnswer);
    }
}
