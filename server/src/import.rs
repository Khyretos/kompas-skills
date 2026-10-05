//! `kompanion-server import <file.json> [user name]`: copies projects and tasks from
//! another planner into Kompanion. Running it again updates what it added
//! before (matched by id) instead of adding duplicates. Everything goes to
//! the named user, or to the first account when no name is given.
//!
//! ```json
//! { "projects": [ { "id": "windshift-12", "name": "kk-engine", "description": "",
//!     "source": "https://projects.example.com/p/12",
//!     "tasks": [ { "id": "windshift-t-88", "title": "Fix shader cache",
//!                  "state": "queued", "source": "https://..." } ] } ] }
//! ```

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use sqlx::SqlitePool;

use crate::util;

#[derive(Deserialize)]
struct File {
    projects: Vec<Project>,
}

#[derive(Deserialize)]
struct Project {
    id: String,
    name: String,
    #[serde(default)]
    description: String,
    source: Option<String>,
    #[serde(default)]
    tasks: Vec<Task>,
}

#[derive(Deserialize)]
struct Task {
    id: String,
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default = "queued")]
    state: String,
    source: Option<String>,
}

fn queued() -> String {
    "queued".into()
}

const STATES: &[&str] = &[
    "queued",
    "waiting_resources",
    "running",
    "needs_input",
    "in_review",
    "done",
    "failed",
];

/// Settings row the import (a separate process) bumps after it changed projects or tasks.
const CHANGE_KEY: &str = "external_change";

async fn change_mark(db: &SqlitePool) -> Option<String> {
    sqlx::query_as::<_, (String,)>("SELECT value FROM settings WHERE key = ?")
        .bind(CHANGE_KEY)
        .fetch_optional(db)
        .await
        .ok()
        .flatten()
        .map(|(v,)| v)
}

/// True once when the mark changed since `last` (which it updates).
async fn changed_since(db: &SqlitePool, last: &mut Option<String>) -> bool {
    let now = change_mark(db).await;
    if now != *last {
        *last = now;
        true
    } else {
        false
    }
}

/// In the server: every 2 s, look for an import by another process and send the same
/// "changed" events the API sends, so every open app updates without a refresh.
pub fn watch(s: crate::AppState) {
    crate::util::supervise("import-watch", move || { let s = s.clone(); async move {
        let mut last = change_mark(&s.db).await;
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(2));
        loop {
            tick.tick().await;
            if changed_since(&s.db, &mut last).await {
                s.bus.send_all(crate::events::Event::Changed { what: "projects", machine_id: None });
                s.bus.send_all(crate::events::Event::Changed { what: "tasks", machine_id: None });
            }
        }
    } });
}

pub async fn run(db: &SqlitePool, path: &str, user: Option<&str>) -> Result<()> {
    let owner: Option<(String,)> = match user {
        Some(name) => {
            sqlx::query_as("SELECT id FROM users WHERE name = ?")
                .bind(name)
                .fetch_optional(db)
                .await?
        }
        None => {
            sqlx::query_as("SELECT id FROM users ORDER BY created_at LIMIT 1")
                .fetch_optional(db)
                .await?
        }
    };
    let Some((owner,)) = owner else {
        bail!("no such account; create one in the app first");
    };
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
    let file: File = serde_json::from_str(&text).with_context(|| format!("parsing {path}"))?;
    let mut tx = db.begin().await?;
    let (mut np, mut nt) = (0, 0);
    for p in &file.projects {
        if p.name.trim().is_empty() {
            bail!("project {} has no name", p.id);
        }
        sqlx::query(
            "INSERT INTO projects (id, name, description, source, updated_at, user_id, kind) VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, description = excluded.description,
             source = excluded.source, updated_at = excluded.updated_at
             WHERE projects.user_id = excluded.user_id",
        )
        .bind(&p.id)
        .bind(p.name.trim())
        .bind(&p.description)
        .bind(&p.source)
        .bind(util::now())
        .bind(&owner)
        .bind(if p.source.as_deref().is_some_and(|s| s.starts_with("windshift:")) { "windshift" } else { "internal" })
        .execute(&mut *tx)
        .await?;
        np += 1;
        for t in &p.tasks {
            if !STATES.contains(&t.state.as_str()) {
                bail!("task {}: unknown state {}", t.id, t.state);
            }
            sqlx::query(
                // New tasks go after the project's existing ones, in the order of the file;
                // a task imported again keeps its place.
                "INSERT INTO tasks (id, project_id, title, description, state, source, updated_at, user_id, position)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, (SELECT COALESCE(MAX(position), -1) + 1 FROM tasks WHERE project_id = ?))
                 ON CONFLICT(id) DO UPDATE SET project_id = excluded.project_id, title = excluded.title,
                 description = CASE WHEN excluded.description = '' THEN tasks.description ELSE excluded.description END,
                 state = excluded.state, source = excluded.source, updated_at = excluded.updated_at
                 WHERE tasks.user_id = excluded.user_id",
            )
            .bind(&t.id)
            .bind(&p.id)
            .bind(t.title.trim())
            .bind(t.description.trim())
            .bind(&t.state)
            .bind(&t.source)
            .bind(util::now())
            .bind(&owner)
            .bind(&p.id)
            .execute(&mut *tx)
            .await?;
            nt += 1;
        }
    }
    // Tell the running server (another process) so open apps update without a refresh.
    sqlx::query("INSERT INTO settings (key, value) VALUES (?, ?) ON CONFLICT(key) DO UPDATE SET value = excluded.value")
        .bind(CHANGE_KEY)
        .bind(format!("{} {}", util::now(), util::new_id()))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    println!("Imported {np} projects and {nt} tasks.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_import_marks_a_change_the_server_sees_once() {
        let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!().run(&db).await.unwrap();
        sqlx::query("INSERT INTO users (id, name, password_hash, created_at) VALUES ('u1', 'kees', 'x', '2026')")
            .execute(&db)
            .await
            .unwrap();
        let mut last = change_mark(&db).await;
        assert!(!changed_since(&db, &mut last).await);
        let path = std::env::temp_dir().join(format!("kk-import-{}.json", std::process::id()));
        std::fs::write(
            &path,
            r#"{"projects":[{"id":"p1","name":"P","tasks":[{"id":"t1","title":"T","description":"**Goal:** x\n1. y"}]}]}"#,
        )
        .unwrap();
        run(&db, path.to_str().unwrap(), Some("kees")).await.unwrap();
        assert!(changed_since(&db, &mut last).await); // the watcher sends its events now
        assert!(!changed_since(&db, &mut last).await); // and only once
        run(&db, path.to_str().unwrap(), Some("kees")).await.unwrap();
        assert!(changed_since(&db, &mut last).await); // every import counts, even an identical one
        std::fs::remove_file(path).ok();
    }
}
