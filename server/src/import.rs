//! `kompanion-server import <file.json>`: copies projects and tasks from
//! another planner into Kompanion. Running it again updates what it added
//! before (matched by id) instead of adding duplicates.
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

pub async fn run(db: &SqlitePool, path: &str) -> Result<()> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
    let file: File = serde_json::from_str(&text).with_context(|| format!("parsing {path}"))?;
    let mut tx = db.begin().await?;
    let (mut np, mut nt) = (0, 0);
    for p in &file.projects {
        if p.name.trim().is_empty() {
            bail!("project {} has no name", p.id);
        }
        sqlx::query(
            "INSERT INTO projects (id, name, description, source, updated_at) VALUES (?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, description = excluded.description,
             source = excluded.source, updated_at = excluded.updated_at",
        )
        .bind(&p.id)
        .bind(p.name.trim())
        .bind(&p.description)
        .bind(&p.source)
        .bind(util::now())
        .execute(&mut *tx)
        .await?;
        np += 1;
        for t in &p.tasks {
            if !STATES.contains(&t.state.as_str()) {
                bail!("task {}: unknown state {}", t.id, t.state);
            }
            sqlx::query(
                "INSERT INTO tasks (id, project_id, title, state, source, updated_at) VALUES (?, ?, ?, ?, ?, ?)
                 ON CONFLICT(id) DO UPDATE SET project_id = excluded.project_id, title = excluded.title,
                 state = excluded.state, source = excluded.source, updated_at = excluded.updated_at",
            )
            .bind(&t.id)
            .bind(&p.id)
            .bind(t.title.trim())
            .bind(&t.state)
            .bind(&t.source)
            .bind(util::now())
            .execute(&mut *tx)
            .await?;
            nt += 1;
        }
    }
    tx.commit().await?;
    println!("Imported {np} projects and {nt} tasks.");
    Ok(())
}
