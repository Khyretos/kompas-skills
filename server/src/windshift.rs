//! Two-way sync between Kompanion tasks and Windshift items, for projects with
//! `kind = 'windshift'`. Configured only from the environment (WINDSHIFT_URL,
//! WINDSHIFT_TOKEN or WINDSHIFT_TOKEN_FILE); the app only shows whether it is
//! connected. Runs every five minutes. When both sides changed a task since the
//! last sync, the newest write wins and the losing version goes into the task's
//! history (`task_events`, kind `sync_conflict`).

use std::time::Duration;

use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;

use crate::util;

pub struct Windshift {
    http: reqwest::Client,
    base: String,
    token: String,
}

#[derive(Deserialize)]
struct Workspace {
    id: i64,
    key: String,
}

#[derive(Deserialize, Clone)]
struct Item {
    id: i64,
    key: String,
    title: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    status_builtin_key: Option<String>,
    updated_at: String,
    #[serde(default)]
    workspace_item_number: i64,
}

#[derive(sqlx::FromRow)]
struct Task {
    id: String,
    title: String,
    description: String,
    state: String,
    updated_at: String,
    remote_updated_at: Option<String>,
    sync_dirty: bool,
}

pub fn to_kompanion(status: &str) -> &'static str {
    match status {
        "done" => "done",
        "in_progress" => "running",
        _ => "queued",
    }
}

pub fn to_windshift(state: &str) -> i64 {
    match state {
        "done" => 3,
        "running" | "in_review" | "needs_input" | "waiting_resources" => 2,
        _ => 1,
    }
}

fn status_id(builtin: &str) -> i64 {
    match builtin {
        "done" => 3,
        "in_progress" => 2,
        _ => 1,
    }
}

impl Windshift {
    /// None when not configured.
    pub fn from_env(http: reqwest::Client) -> Option<Self> {
        let base = std::env::var("WINDSHIFT_URL").ok().filter(|s| !s.is_empty())?;
        let token = std::env::var("WINDSHIFT_TOKEN")
            .ok()
            .or_else(|| std::env::var("WINDSHIFT_TOKEN_FILE").ok().and_then(|p| std::fs::read_to_string(p).ok()))
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())?;
        Some(Windshift { http, base: base.trim_end_matches('/').to_string(), token })
    }

    fn url(&self, path: &str) -> String {
        format!("{}/rest/api/v2{path}", self.base)
    }

    async fn get(&self, path: &str) -> Result<Value> {
        let r = self.http.get(self.url(path)).bearer_auth(&self.token).send().await?;
        let status = r.status();
        anyhow::ensure!(status.is_success(), "GET {path}: {status}");
        Ok(r.json().await?)
    }

    async fn items(&self, workspace_id: i64) -> Result<Vec<Item>> {
        let mut out = Vec::new();
        for page in 1..=100 {
            let v = self.get(&format!("/items?workspace_id={workspace_id}&limit=200&page={page}")).await?;
            out.extend(serde_json::from_value::<Vec<Item>>(v["data"].clone()).context("items")?);
            if page >= v["pagination"]["total_pages"].as_i64().unwrap_or(1) {
                break;
            }
        }
        Ok(out)
    }

    async fn item(&self, id: i64) -> Result<Item> {
        Ok(serde_json::from_value(self.get(&format!("/items/{id}")).await?["data"].clone())?)
    }

    async fn update(&self, id: i64, title: &str, description: &str) -> Result<()> {
        let r = self
            .http
            .patch(self.url(&format!("/items/{id}")))
            .bearer_auth(&self.token)
            .header("Content-Type", "application/merge-patch+json")
            .body(json!({ "title": title, "description": description }).to_string())
            .send()
            .await?;
        anyhow::ensure!(r.status().is_success(), "PATCH item {id}: {}", r.status());
        Ok(())
    }

    async fn transition(&self, id: i64, to: i64) -> Result<()> {
        let r = self
            .http
            .post(self.url(&format!("/items/{id}/transition")))
            .bearer_auth(&self.token)
            .json(&json!({ "to_status_id": to }))
            .send()
            .await?;
        anyhow::ensure!(r.status().is_success(), "transition item {id}: {}", r.status());
        Ok(())
    }

    /// Whether the token works (for the "connected" indicator).
    pub async fn check(&self) -> bool {
        self.get("/users/me").await.is_ok()
    }
}

/// Runs the sync after 10 s and then every five minutes.
pub fn spawn(db: SqlitePool, ws: std::sync::Arc<Windshift>) {
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_secs(10)).await;
        loop {
            match sync_once(&db, &ws).await {
                Ok((pulled, pushed)) if pulled + pushed > 0 => {
                    tracing::info!(pulled, pushed, "Windshift sync")
                }
                Ok(_) => {}
                Err(e) => tracing::warn!("Windshift sync failed: {e:#}"),
            }
            tokio::time::sleep(Duration::from_secs(300)).await;
        }
    });
}

async fn history(db: &SqlitePool, task_id: &str, user_id: &str, detail: Value) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO task_events (task_id, user_id, at, kind, detail) VALUES (?, ?, ?, 'sync_conflict', ?)")
        .bind(task_id)
        .bind(user_id)
        .bind(util::now())
        .bind(detail.to_string())
        .execute(db)
        .await?;
    Ok(())
}

/// One full sync; returns (pulled, pushed).
pub async fn sync_once(db: &SqlitePool, ws: &Windshift) -> Result<(usize, usize)> {
    let workspaces: Vec<Workspace> = serde_json::from_value(ws.get("/workspaces").await?["data"].clone())?;
    let projects: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT id, source, user_id FROM projects WHERE kind = 'windshift' AND source LIKE 'windshift:%'",
    )
    .fetch_all(db)
    .await?;
    let (mut pulled, mut pushed) = (0, 0);

    for (project_id, source, user_id) in projects {
        let key = source.trim_start_matches("windshift:");
        let Some(w) = workspaces.iter().find(|w| w.key == key) else {
            tracing::warn!(workspace = key, "Windshift workspace not found");
            continue;
        };
        let items = ws.items(w.id).await?;

        for item in &items {
            let task_id = format!("windshift-{}", item.key);
            let desc = item.description.clone().unwrap_or_default();
            let status = item.status_builtin_key.clone().unwrap_or_default();
            let task: Option<Task> = sqlx::query_as(
                "SELECT id, title, description, state, updated_at, remote_updated_at, sync_dirty FROM tasks
                 WHERE id = ? AND user_id = ?",
            )
            .bind(&task_id)
            .bind(&user_id)
            .fetch_optional(db)
            .await?;
            match task {
                None => {
                    sqlx::query(
                        "INSERT INTO tasks (id, project_id, user_id, title, description, state, position, source,
                         sync_dirty, remote_updated_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, ?, ?)",
                    )
                    .bind(&task_id)
                    .bind(&project_id)
                    .bind(&user_id)
                    .bind(&item.title)
                    .bind(&desc)
                    .bind(to_kompanion(&status))
                    .bind(item.workspace_item_number as f64)
                    .bind(format!("windshift:{}", item.key))
                    .bind(&item.updated_at)
                    .bind(util::now())
                    .execute(db)
                    .await?;
                    pulled += 1;
                }
                Some(t) if t.remote_updated_at.as_deref() == Some(item.updated_at.as_str()) => {}
                Some(t) if !t.sync_dirty || item.updated_at > t.updated_at => {
                    if t.sync_dirty {
                        history(db, &t.id, &user_id, json!({ "kept": "windshift",
                            "lost": { "title": t.title, "description": t.description, "state": t.state } }))
                        .await?;
                    }
                    crate::notify::task_changed(
                        db.clone(), user_id.clone(), item.title.clone(), t.state.clone(), to_kompanion(&status).into(),
                    );
                    sqlx::query(
                        "UPDATE tasks SET title = ?, description = ?, state = ?, remote_updated_at = ?,
                         sync_dirty = 0, updated_at = ? WHERE id = ?",
                    )
                    .bind(&item.title)
                    .bind(&desc)
                    .bind(to_kompanion(&status))
                    .bind(&item.updated_at)
                    .bind(util::now())
                    .bind(&t.id)
                    .execute(db)
                    .await?;
                    pulled += 1;
                }
                Some(t) => {
                    // Both changed and Kompanion is newer: keep ours, note theirs; pushed below.
                    if t.remote_updated_at.is_some() {
                        history(db, &t.id, &user_id, json!({ "kept": "kompanion",
                            "lost": { "title": item.title, "description": desc, "status": status } }))
                        .await?;
                    }
                }
            }
        }

        // Push what changed here.
        let dirty: Vec<Task> = sqlx::query_as(
            "SELECT id, title, description, state, updated_at, remote_updated_at, sync_dirty FROM tasks
             WHERE project_id = ? AND user_id = ? AND sync_dirty = 1",
        )
        .bind(&project_id)
        .bind(&user_id)
        .fetch_all(db)
        .await?;
        for t in dirty {
            let key = t.id.trim_start_matches("windshift-");
            let Some(item) = items.iter().find(|i| i.key == key) else {
                continue; // made in Kompanion; creating Windshift items comes later
            };
            ws.update(item.id, &t.title, &t.description).await?;
            let want = to_windshift(&t.state);
            if want != status_id(item.status_builtin_key.as_deref().unwrap_or("")) {
                ws.transition(item.id, want).await?;
            }
            let fresh = ws.item(item.id).await?;
            sqlx::query("UPDATE tasks SET sync_dirty = 0, remote_updated_at = ? WHERE id = ?")
                .bind(&fresh.updated_at)
                .bind(&t.id)
                .execute(db)
                .await?;
            pushed += 1;
        }

        // Tasks deleted here are closed in Windshift.
        let deletes: Vec<(String, String)> = sqlx::query_as(
            "SELECT task_id, source FROM sync_deletes WHERE user_id = ? AND source LIKE ?",
        )
        .bind(&user_id)
        .bind(format!("windshift:{}-%", w.key))
        .fetch_all(db)
        .await?;
        for (task_id, source) in deletes {
            let key = source.trim_start_matches("windshift:");
            if let Some(item) = items.iter().find(|i| i.key == key)
                && item.status_builtin_key.as_deref() != Some("done")
            {
                ws.transition(item.id, 3).await?;
            }
            sqlx::query("DELETE FROM sync_deletes WHERE task_id = ?").bind(&task_id).execute(db).await?;
            pushed += 1;
        }
    }
    Ok((pulled, pushed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn states_map_both_ways() {
        assert_eq!(to_kompanion("open"), "queued");
        assert_eq!(to_kompanion("in_progress"), "running");
        assert_eq!(to_kompanion("done"), "done");
        assert_eq!(to_windshift("done"), 3);
        assert_eq!(to_windshift("needs_input"), 2);
        assert_eq!(to_windshift("failed"), 1);
        for s in ["open", "in_progress", "done"] {
            assert_eq!(status_id(s), to_windshift(to_kompanion(s)));
        }
    }
}
