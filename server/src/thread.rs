//! Project thread: a single chat per project where the orchestrator posts short updates.

use axum::{Extension, Json, extract::{Path, State}};
use serde_json::{Value, json};
use crate::{AppState, api, auth::User, error::{ApiError, ApiResult}, events::Event, util};

pub const TITLE: &str = "Project thread";

pub async fn ensure(s: &AppState, user_id: &str, project_id: &str) -> ApiResult<String> {
    // Try to find existing thread
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM chats WHERE project_id = ? AND user_id = ? AND thread = 1"
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_optional(&s.db)
    .await?;

    if let Some((id,)) = row {
        return Ok(id);
    }

    // Create new thread (INSERT OR IGNORE handles race conditions)
    sqlx::query(
        "INSERT OR IGNORE INTO chats (id, project_id, title, updated_at, user_id, thread) 
         VALUES (?, ?, ?, ?, ?, 1)"
    )
    .bind(util::new_id())
    .bind(project_id)
    .bind(TITLE)
    .bind(util::now())
    .bind(user_id)
    .execute(&s.db)
    .await?;

    // Fetch the created/confirmed ID
    let row = sqlx::query_as::<_, (String,)>(
        "SELECT id FROM chats WHERE project_id = ? AND user_id = ? AND thread = 1"
    )
    .bind(project_id)
    .bind(user_id)
    .fetch_one(&s.db)
    .await?;

    s.bus.send(user_id, Event::Changed { what: "chats", machine_id: None });
    Ok(row.0)
}

pub async fn post(s: &AppState, user_id: &str, project_id: &str, text: &str) -> Option<api::Message> {
    let chat_id = match ensure(s, user_id, project_id).await {
        Ok(id) => id,
        Err(e) => {
            tracing::warn!("project thread ensure failed: {}", e);
            return None;
        }
    };

    let message = match api::insert_message(s, &chat_id, "orchestrator", text).await {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("project thread insert_message failed: {}", e);
            return None;
        }
    };

    s.bus.send(user_id, Event::Message { message: message.clone() });
    Some(message)
}

pub async fn post_run(s: &AppState, run_id: &str, text: &str) -> Option<api::Message> {
    let row = match sqlx::query_as::<_, (String, String)>(
        "SELECT r.user_id, t.project_id 
         FROM runs r 
         JOIN tasks t ON t.id = r.task_id 
         WHERE r.id = ?"
    )
    .bind(run_id)
    .fetch_optional(&s.db)
    .await
    {
        Ok(Some((user_id, project_id))) => (user_id, project_id),
        Ok(None) | Err(_) => return None,
    };

    post(s, &row.0, &row.1, text).await
}

/// A steering word the user wrote in the thread: "pause" or "resume", or None for anything else
/// (a question goes to the orchestrator as usual). Short messages only, so a sentence that happens
/// to contain "wait" is still a question.
pub fn command(text: &str) -> Option<&'static str> {
    let t: String = text.trim().to_lowercase().chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect();
    match t.split_whitespace().collect::<Vec<_>>().join(" ").as_str() {
        "pause" | "pause please" | "hold on" | "wait" | "stop for now" => Some("pause"),
        "go on" | "continue" | "resume" | "go ahead" | "carry on" => Some("resume"),
        _ => None,
    }
}

/// Carries out a steering word for the project's running tasks; the orchestrator's answer.
pub async fn steer(s: &AppState, user_id: &str, project_id: &str, cmd: &str) -> String {
    let running: Vec<(String, String)> = sqlx::query_as("SELECT id, title FROM tasks WHERE project_id = ? AND user_id = ? AND state = 'running'")
        .bind(project_id)
        .bind(user_id)
        .fetch_all(&s.db)
        .await
        .unwrap_or_default();
    if running.is_empty() {
        return "Nothing is running in this project.".into();
    }
    let titles = running.iter().map(|(_, t)| format!("**{t}**")).collect::<Vec<_>>().join(", ");
    if cmd == "pause" {
        for (id, _) in &running {
            crate::taskrun::pause(id);
        }
        format!("Paused {titles}: the current step finishes, then it waits. Write \"go on\" to continue.")
    } else {
        let resumed: Vec<&String> = running.iter().filter(|(id, _)| crate::taskrun::resume(id)).map(|(_, t)| t).collect();
        if resumed.is_empty() {
            format!("{titles} wasn't paused; it is still running.")
        } else {
            format!("Going on with {}.", resumed.iter().map(|t| format!("**{t}**")).collect::<Vec<_>>().join(", "))
        }
    }
}

pub async fn open(State(s): State<AppState>, Extension(u): Extension<User>, Path(project_id): Path<String>) -> ApiResult<Json<Value>> {
    // Check project ownership
    let _ = sqlx::query_as::<_, (i64,)>(
        "SELECT 1 FROM projects WHERE id = ? AND user_id = ?"
    )
    .bind(&project_id)
    .bind(&u.id)
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| ApiError::NotFound)?;

    let chat_id = ensure(&s, &u.id, &project_id).await?;
    
    Ok(Json(json!({ "chatId": chat_id })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    async fn db() -> SqlitePool {
        let opts = sqlx::sqlite::SqliteConnectOptions::new().in_memory(true).foreign_keys(true);
        let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect_with(opts).await.unwrap();
        sqlx::migrate!().run(&db).await.unwrap();
        for q in [
            "INSERT INTO users (id, name, password_hash, created_at) VALUES ('u1', 'u1', 'x', '2026')",
            "INSERT INTO projects (id, name, updated_at, user_id) VALUES ('p1', 'Game', '2026', 'u1')",
        ] { sqlx::query(q).execute(&db).await.unwrap(); }
        db
    }
    fn state(db: SqlitePool) -> AppState { AppState::for_tests(toml::from_str("").unwrap(), db) }

    #[tokio::test]
    async fn ensure_makes_one_thread() {
        let db = db().await;
        let s = state(db.clone());

        let id1 = ensure(&s, "u1", "p1").await.unwrap();
        let id2 = ensure(&s, "u1", "p1").await.unwrap();
        
        assert_eq!(id1, id2);

        let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM chats WHERE thread = 1")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn post_stores_an_orchestrator_message() {
        let db = db().await;
        let s = state(db.clone());

        let chat_id = ensure(&s, "u1", "p1").await.unwrap();
        post(&s, "u1", "p1", "Started").await;

        let row: (String, String) = sqlx::query_as("SELECT author, text FROM messages WHERE chat_id = ?")
            .bind(&chat_id)
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(row, ("orchestrator".to_string(), "Started".to_string()));
    }

    #[tokio::test]
    async fn open_refuses_someone_elses_project() {
        let db = db().await;
        let s = state(db.clone());

        // Insert u2
        sqlx::query("INSERT INTO users (id, name, password_hash, created_at) VALUES ('u2', 'u2', 'x', '2026')")
            .execute(&db)
            .await
            .unwrap();

        let result = open(
            State(s.clone()),
            Extension(User { id: "u2".into(), name: "u2".into() }),
            Path("p1".into())
        ).await;

        assert!(result.is_err());
    }

    #[test]
    fn steering_words_are_short_commands() {
        assert_eq!(command("Pause"), Some("pause"));
        assert_eq!(command("  go on! "), Some("resume"));
        assert_eq!(command("Continue."), Some("resume"));
        assert_eq!(command("wait, why did step 2 fail?"), None);
        assert_eq!(command("How far is it?"), None);
    }
}
