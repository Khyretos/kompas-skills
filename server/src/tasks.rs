//! Task editing: create, change, reorder, delete. Every query is scoped to the
//! signed-in user, and every change is written to `task_events` (history).
//! Changes to tasks of a Windshift project are queued for the sync adapter.

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use serde::Deserialize;
use serde_json::{Value, json};
use sqlx::SqlitePool;

use crate::{
    AppState,
    auth::User,
    error::{ApiError, ApiResult},
    util,
};

pub const STATES: &[&str] = &[
    "queued",
    "waiting_resources",
    "running",
    "needs_input",
    "in_review",
    "done",
    "failed",
];

/// Shown in the editor for a new task: a title alone is never enough.
pub const TEMPLATE: &str = "**Goal:** \n\n**Steps**\n1. \n\n**Done when:** ";

#[derive(sqlx::FromRow)]
struct Row {
    id: String,
    project_id: String,
    title: String,
    description: String,
    state: String,
    position: f64,
    progress: f64,
    step: String,
    role: String,
    model: String,
    source: Option<String>,
    updated_at: String,
}

fn to_json(r: Row) -> Value {
    json!({
        "id": r.id, "projectId": r.project_id, "title": r.title, "description": r.description,
        "state": r.state, "position": r.position, "progress": r.progress, "step": r.step,
        "role": r.role, "model": r.model, "source": r.source, "updatedAt": r.updated_at, "events": [],
    })
}

const COLUMNS: &str = "id, project_id, title, description, state, position, progress, step, role, model, source, updated_at";

#[derive(Deserialize)]
pub struct ListQuery {
    project: Option<String>,
}

pub async fn list(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Vec<Value>>> {
    let rows: Vec<Row> = sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM tasks WHERE user_id = ?2 AND (?1 IS NULL OR project_id = ?1)
         ORDER BY project_id, position, updated_at DESC LIMIT 2000"
    ))
    .bind(q.project)
    .bind(&u.id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows.into_iter().map(to_json).collect()))
}

async fn one(db: &SqlitePool, id: &str, user_id: &str) -> ApiResult<Row> {
    sqlx::query_as(&format!("SELECT {COLUMNS} FROM tasks WHERE id = ? AND user_id = ?"))
        .bind(id)
        .bind(user_id)
        .fetch_optional(db)
        .await?
        .ok_or(ApiError::NotFound)
}

pub(crate) async fn history(db: &SqlitePool, task_id: &str, user_id: &str, kind: &str, detail: Value) -> sqlx::Result<()> {
    sqlx::query("INSERT INTO task_events (task_id, user_id, at, kind, detail) VALUES (?, ?, ?, ?, ?)")
        .bind(task_id)
        .bind(user_id)
        .bind(util::now())
        .bind(kind)
        .bind(detail.to_string())
        .execute(db)
        .await?;
    Ok(())
}

/// Marks a task of a Windshift project as changed here, for the sync adapter.
pub(crate) async fn mark_dirty(db: &SqlitePool, task_id: &str) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE tasks SET sync_dirty = 1 WHERE id = ?
           AND project_id IN (SELECT id FROM projects WHERE kind = 'windshift')",
    )
    .bind(task_id)
    .execute(db)
    .await?;
    Ok(())
}

fn clean_title(t: &str) -> ApiResult<String> {
    let t: String = t.trim().chars().take(200).collect();
    if t.is_empty() {
        return Err(ApiError::BadRequest("Give the task a title.".into()));
    }
    Ok(t)
}

fn clean_description(d: &str) -> ApiResult<String> {
    let d = d.trim();
    if d.len() > 20_000 {
        return Err(ApiError::BadRequest("The description is too long (20,000 characters at most).".into()));
    }
    let lower = d.to_lowercase();
    // A title alone is never enough: a goal or steps must be written down.
    let has_body = d.lines().filter(|l| !l.trim().is_empty()).count() >= 2
        && (lower.contains("goal") || lower.contains("steps") || lower.contains("1."));
    if !has_body || d == TEMPLATE.trim() {
        return Err(ApiError::BadRequest(
            "Describe the task: a goal, the steps, and when it is done.".into(),
        ));
    }
    Ok(d.to_string())
}

fn check_state(state: &str) -> ApiResult<()> {
    if STATES.contains(&state) {
        Ok(())
    } else {
        Err(ApiError::BadRequest(format!("Unknown state {state}.")))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewTask {
    project_id: String,
    title: String,
    description: String,
    #[serde(default)]
    state: Option<String>,
}

pub async fn create(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Json(b): Json<NewTask>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let title = clean_title(&b.title)?;
    let description = clean_description(&b.description)?;
    let state = b.state.unwrap_or_else(|| "queued".into());
    check_state(&state)?;
    let owns: Option<(i64,)> = sqlx::query_as("SELECT 1 FROM projects WHERE id = ? AND user_id = ?")
        .bind(&b.project_id)
        .bind(&u.id)
        .fetch_optional(&s.db)
        .await?;
    if owns.is_none() {
        return Err(ApiError::NotFound);
    }
    let id = util::new_id();
    sqlx::query(
        "INSERT INTO tasks (id, project_id, title, description, state, position, updated_at, user_id, sync_dirty)
         VALUES (?, ?, ?, ?, ?, (SELECT COALESCE(MAX(position), 0) + 1 FROM tasks WHERE project_id = ?), ?, ?,
                 (SELECT kind = 'windshift' FROM projects WHERE id = ?))",
    )
    .bind(&id)
    .bind(&b.project_id)
    .bind(&title)
    .bind(&description)
    .bind(&state)
    .bind(&b.project_id)
    .bind(util::now())
    .bind(&u.id)
    .bind(&b.project_id)
    .execute(&s.db)
    .await?;
    history(&s.db, &id, &u.id, "created", json!({ "title": title })).await?;
    Ok((StatusCode::CREATED, Json(to_json(one(&s.db, &id, &u.id).await?))))
}

#[derive(Deserialize)]
pub struct Change {
    title: Option<String>,
    description: Option<String>,
    state: Option<String>,
}

pub async fn update(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
    Json(b): Json<Change>,
) -> ApiResult<Json<Value>> {
    let before = one(&s.db, &id, &u.id).await?;
    let title = b.title.as_deref().map(clean_title).transpose()?;
    let description = b.description.as_deref().map(clean_description).transpose()?;
    if let Some(st) = &b.state {
        check_state(st)?;
    }
    sqlx::query(
        "UPDATE tasks SET title = COALESCE(?, title), description = COALESCE(?, description),
         state = COALESCE(?, state), updated_at = ? WHERE id = ? AND user_id = ?",
    )
    .bind(&title)
    .bind(&description)
    .bind(&b.state)
    .bind(util::now())
    .bind(&id)
    .bind(&u.id)
    .execute(&s.db)
    .await?;
    let mut changed = serde_json::Map::new();
    if let Some(t) = title.filter(|t| *t != before.title) {
        changed.insert("title".into(), json!({ "from": before.title, "to": t }));
    }
    if description.as_ref().is_some_and(|d| *d != before.description) {
        changed.insert("description".into(), json!({ "from": before.description }));
    }
    if let Some(st) = b.state.filter(|st| *st != before.state) {
        changed.insert("state".into(), json!({ "from": before.state, "to": st }));
        crate::notify::task_changed(s.db.clone(), u.id.clone(), id.clone(), before.title.clone(), before.state.clone(), st);
    }
    if !changed.is_empty() {
        history(&s.db, &id, &u.id, "changed", Value::Object(changed)).await?;
        mark_dirty(&s.db, &id).await?;
    }
    Ok(Json(to_json(one(&s.db, &id, &u.id).await?)))
}

pub async fn delete(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let t = one(&s.db, &id, &u.id).await?;
    // Windshift items are closed there by the sync adapter; keep a tombstone.
    if t.source.as_deref().is_some_and(|src| src.starts_with("windshift:")) {
        sqlx::query("INSERT INTO sync_deletes (task_id, user_id, source, at) VALUES (?, ?, ?, ?)")
            .bind(&id)
            .bind(&u.id)
            .bind(&t.source)
            .bind(util::now())
            .execute(&s.db)
            .await?;
    }
    sqlx::query("DELETE FROM tasks WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&u.id)
        .execute(&s.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Order {
    project_id: String,
    ids: Vec<String>,
}

/// Sets the order of a project's tasks; ids not listed keep their place after.
pub async fn reorder(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Json(b): Json<Order>,
) -> ApiResult<StatusCode> {
    let mut tx = s.db.begin().await?;
    for (i, id) in b.ids.iter().enumerate() {
        let done = sqlx::query("UPDATE tasks SET position = ? WHERE id = ? AND project_id = ? AND user_id = ?")
            .bind(i as f64)
            .bind(id)
            .bind(&b.project_id)
            .bind(&u.id)
            .execute(&mut *tx)
            .await?;
        if done.rows_affected() == 0 {
            return Err(ApiError::NotFound);
        }
    }
    sqlx::query(
        "UPDATE tasks SET position = position + ? WHERE project_id = ? AND user_id = ? AND id NOT IN (SELECT value FROM json_each(?))",
    )
    .bind(b.ids.len() as f64)
    .bind(&b.project_id)
    .bind(&u.id)
    .bind(serde_json::to_string(&b.ids).unwrap_or_default())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(StatusCode::NO_CONTENT)
}

/// A task's change history, newest first.
pub async fn events(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
) -> ApiResult<Json<Vec<Value>>> {
    one(&s.db, &id, &u.id).await?;
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT at, kind, detail FROM task_events WHERE task_id = ? AND user_id = ? ORDER BY id DESC LIMIT 200",
    )
    .bind(&id)
    .bind(&u.id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(at, kind, detail)| {
                json!({ "at": at, "kind": kind, "detail": serde_json::from_str::<Value>(&detail).unwrap_or(Value::Null) })
            })
            .collect(),
    ))
}

#[derive(Deserialize)]
pub struct ProjectChange {
    kind: String,
}

/// Turns a Windshift project into an internal one (keeps everything, stops syncing).
pub async fn set_project_kind(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
    Json(b): Json<ProjectChange>,
) -> ApiResult<StatusCode> {
    if b.kind != "internal" {
        return Err(ApiError::BadRequest(
            "A project can only be turned into an internal one here.".into(),
        ));
    }
    let done = sqlx::query("UPDATE projects SET kind = 'internal' WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&u.id)
        .execute(&s.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    sqlx::query("UPDATE tasks SET sync_dirty = 0 WHERE project_id = ?")
        .bind(&id)
        .execute(&s.db)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptions_need_a_body() {
        assert!(clean_description("Fix it").is_err());
        assert!(clean_description(TEMPLATE).is_err());
        assert!(clean_description("**Goal:** make backups.\n\n**Steps**\n1. Run restic.\n\n**Done when:** a snapshot exists.").is_ok());
    }

    async fn db() -> SqlitePool {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&db).await.unwrap();
        for (u, p) in [("u1", "p1"), ("u2", "p2")] {
            sqlx::query("INSERT INTO users (id, name, password_hash, created_at) VALUES (?, ?, 'x', '2026')")
                .bind(u)
                .bind(u)
                .execute(&db)
                .await
                .unwrap();
            sqlx::query("INSERT INTO projects (id, name, updated_at, user_id) VALUES (?, ?, '2026', ?)")
                .bind(p)
                .bind(p)
                .bind(u)
                .execute(&db)
                .await
                .unwrap();
        }
        for (id, pos) in [("a", 0.0), ("b", 1.0), ("c", 2.0)] {
            sqlx::query("INSERT INTO tasks (id, project_id, title, position, updated_at, user_id) VALUES (?, 'p1', ?, ?, '2026', 'u1')")
                .bind(id)
                .bind(id)
                .bind(pos)
                .execute(&db)
                .await
                .unwrap();
        }
        db
    }

    #[tokio::test]
    async fn other_users_task_is_not_found() {
        let db = db().await;
        assert!(one(&db, "a", "u1").await.is_ok());
        assert!(matches!(one(&db, "a", "u2").await, Err(ApiError::NotFound)));
    }

    #[tokio::test]
    async fn reorder_is_stable() {
        let db = db().await;
        let mut config: crate::config::Config = toml::from_str("").unwrap();
        config.roles.clear();
        let s = AppState::for_tests(config, db.clone());
        let u = User { id: "u1".into(), name: "u1".into() };
        reorder(
            State(s.clone()),
            Extension(u.clone()),
            Json(Order { project_id: "p1".into(), ids: vec!["c".into(), "a".into()] }),
        )
        .await
        .unwrap();
        let order: Vec<(String,)> = sqlx::query_as("SELECT id FROM tasks WHERE project_id = 'p1' ORDER BY position")
            .fetch_all(&db)
            .await
            .unwrap();
        assert_eq!(order.into_iter().map(|r| r.0).collect::<Vec<_>>(), ["c", "a", "b"]);
        // Someone else's project: refused.
        let other = User { id: "u2".into(), name: "u2".into() };
        assert!(
            reorder(State(s), Extension(other), Json(Order { project_id: "p1".into(), ids: vec!["a".into()] }))
                .await
                .is_err()
        );
    }
}
