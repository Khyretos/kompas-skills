//! Lessons from task runs. The reviewer proposes one per finding in the project thread; the user
//! accepts (optionally edited), or dismisses it. Accepted lessons go into a writable folder layered
//! over the read-only skills folder, so the next run's worker and reviewer read them.
use std::{fs::OpenOptions, io::Write, path::{Path, PathBuf}};
use axum::{Extension, Json, extract::{self, State}, http::StatusCode};
use serde::{Deserialize, Serialize};
use crate::{AppState, auth::User, error::{ApiError, ApiResult}, events::Event, util};

/// Returns the data directory for skills, using KOMPANION_SKILLS_DATA if set.
pub fn data_dir(s: &AppState) -> PathBuf {
    std::env::var("KOMPANION_SKILLS_DATA")
        .ok()
        .map(PathBuf::from)
        .unwrap_or_else(|| s.config.database.parent().unwrap_or(Path::new(".")).join("skills"))
}

/// Checks if a card with the given name exists in the skills directory.
fn known_card(name: &str) -> bool {
    crate::skills::layered(&crate::skills::dir()).iter().any(|c| c.name == name)
}

/// Proposes a lesson to the database and posts a notification.
pub async fn propose(
    s: &AppState,
    user_id: &str,
    project_id: &str,
    run_id: &str,
    task_id: &str,
    card: &str,
    text: &str,
    finding: &str,
) {
    if text.trim().is_empty() {
        return;
    }

    let chat_id = match crate::thread::ensure(s, user_id, project_id).await {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!("lesson: {e}");
            return;
        }
    };

    let stored = sqlx::query(
        "INSERT INTO lessons (id, user_id, project_id, chat_id, run_id, task_id, card, text, finding, state, created_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, 'proposed', ?)"
    )
    .bind(util::new_id())
    .bind(user_id)
    .bind(project_id)
    .bind(chat_id)
    .bind(run_id)
    .bind(task_id)
    .bind(card)
    .bind(text.trim())
    .bind(finding)
    .bind(util::now())
    .execute(&s.db)
    .await;
    if let Err(e) = stored {
        tracing::warn!("lesson: {e}");
        return;
    }

    crate::thread::post(s, user_id, project_id, &format!("Lesson proposed for `{card}`: {}", text.trim())).await;
    s.bus.send(user_id, Event::Changed { what: "lessons", machine_id: None });
}

/// Adds a lesson entry to the card's markdown file and the global log.
pub fn add_to_card(dir: &Path, card: &str, text: &str, finding: &str) -> ApiResult<PathBuf> {
    let io = |e: std::io::Error| ApiError::BadRequest(format!("Could not save the lesson: {e}"));

    let date: String = util::now().chars().take(10).collect();
    let path = dir.join(format!("{card}.md"));

    if let Some(p) = path.parent() {
        std::fs::create_dir_all(p).map_err(io)?;
    }

    {
        let mut file = OpenOptions::new().create(true).append(true).open(&path).map_err(io)?;
        writeln!(file, "- ({date}) {text}").map_err(io)?;
    }

    let log = dir.join("lessons-learned.md");
    let new = !log.exists();
    {
        let mut file = OpenOptions::new().create(true).append(true).open(&log).map_err(io)?;
        if new {
            writeln!(file, "| Date | Card | Finding | Lesson |\n|---|---|---|---|").map_err(io)?;
        }
        writeln!(file, "| {date} | {card} | {} | {} |", finding.replace('|', "/"), text.replace('|', "/")).map_err(io)?;
    }

    Ok(path)
}

/// Reads the current content of a card's markdown file.
pub fn overlay(dir: &Path, card: &str) -> String {
    std::fs::read_to_string(dir.join(format!("{card}.md"))).unwrap_or_default()
}

/// A lesson proposal stored in the database.
#[derive(Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Lesson {
    pub id: String,
    pub chat_id: String,
    pub card: String,
    pub text: String,
    pub finding: String,
    pub state: String,
    pub created_at: String,
}

/// List lessons for a specific chat.
pub async fn list(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    extract::Path(chat_id): extract::Path<String>,
) -> ApiResult<Json<Vec<Lesson>>> {
    let rows: Vec<Lesson> = sqlx::query_as(
        "SELECT id, chat_id, card, text, finding, state, created_at FROM lessons WHERE chat_id = ? AND user_id = ? ORDER BY created_at, rowid"
    )
    .bind(&chat_id)
    .bind(&u.id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

/// The decision payload from the client.
#[derive(Deserialize)]
pub struct Decision {
    pub decision: String,
    #[serde(default)]
    pub text: Option<String>,
}

/// Accept or dismiss a lesson proposal.
pub async fn decide(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    extract::Path(id): extract::Path<String>,
    Json(b): Json<Decision>,
) -> ApiResult<StatusCode> {
    let row: Option<(String, String, String, String, String)> = sqlx::query_as(
        "SELECT card, text, finding, state, project_id FROM lessons WHERE id = ? AND user_id = ?"
    )
    .bind(&id)
    .bind(&u.id)
    .fetch_optional(&s.db)
    .await?;

    let Some((card, text, finding, state, project_id)) = row else {
        return Err(ApiError::NotFound);
    };

    if state != "proposed" {
        return Err(ApiError::BadRequest("This lesson was already decided.".into()));
    }

    match b.decision.as_str() {
        "dismiss" => {
            sqlx::query(
                "UPDATE lessons SET state = 'dismissed', decided_at = ? WHERE id = ?"
            )
            .bind(util::now())
            .bind(&id)
            .execute(&s.db)
            .await?;
            crate::thread::post(&s, &u.id, &project_id, &format!("Lesson for `{card}` dismissed."))
                .await;
        }
        "accept" => {
            let final_text = b.text.as_deref().map(str::trim).filter(|t| !t.is_empty()).unwrap_or(text.as_str()).to_string();
            
            if final_text.chars().count() > 2000 {
                return Err(ApiError::BadRequest("Keep a lesson under 2,000 characters.".into()));
            }
            
            if !known_card(&card) {
                return Err(ApiError::BadRequest("Unknown skill card.".into()));
            }

            let path = add_to_card(&data_dir(&s), &card, &final_text, &finding)?;

            sqlx::query(
                "UPDATE lessons SET state = 'accepted', text = ?, decided_at = ? WHERE id = ?"
            )
            .bind(&final_text)
            .bind(util::now())
            .bind(&id)
            .execute(&s.db)
            .await?;

            crate::thread::post(&s, &u.id, &project_id, &format!("Lesson added to `{card}` ({}).", path.display()))
                .await;
        }
        _ => return Err(ApiError::BadRequest("Unknown decision.".into())),
    }

    Ok(StatusCode::NO_CONTENT)
}
#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::SqlitePool;

    // The data folder is a process-wide env var: tests that set it run one at a time.
    static ENV: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    async fn db() -> SqlitePool {
        let opts = sqlx::sqlite::SqliteConnectOptions::new().in_memory(true).foreign_keys(true);
        let db = sqlx::sqlite::SqlitePoolOptions::new().max_connections(1).connect_with(opts).await.unwrap();
        sqlx::migrate!().run(&db).await.unwrap();
        for q in [
            "INSERT INTO users (id, name, password_hash, created_at) VALUES ('u1', 'u1', 'x', '2026'), ('u2', 'u2', 'x', '2026')",
            "INSERT INTO projects (id, name, updated_at, user_id) VALUES ('p1', 'Game', '2026', 'u1')",
        ] {
            sqlx::query(q).execute(&db).await.unwrap();
        }
        db
    }

    fn state(db: SqlitePool) -> AppState {
        AppState::for_tests(toml::from_str("").unwrap(), db)
    }

    fn user(id: &str) -> User {
        User { id: id.into(), name: id.into() }
    }

    async fn lesson_id(db: &SqlitePool, card: &str) -> String {
        sqlx::query_as::<_, (String,)>(
            "SELECT id FROM lessons WHERE card = ?",
        )
        .bind(card)
        .fetch_one(db)
        .await
        .unwrap()
        .0
    }

    fn data_dir_for_test(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("kk-lessons-{name}-{}", std::process::id()));
        unsafe { std::env::set_var("KOMPANION_SKILLS_DATA", &dir) };
        unsafe { std::env::set_var("KOMPANION_SKILLS", concat!(env!("CARGO_MANIFEST_DIR"), "/../skills")) };
        dir
    }

    fn cleanup(data_dir: &Path) {
        if data_dir.exists() {
            let _ = std::fs::remove_dir_all(data_dir);
        }
    }

    #[tokio::test]
    async fn accept_writes_the_card_and_the_log() {
        let _env = ENV.lock().await;
        let db = db().await;
        let s = state(db.clone());
        let data_dir = data_dir_for_test("accept-writes");
        let _ = cleanup(&data_dir);

        propose(
            &s,
            "u1",
            "p1",
            "r1",
            "t1",
            "worker/rust/SKILL",
            "Bind every value.",
            "SQL was formatted",
        )
        .await;

        let id = lesson_id(&db, "worker/rust/SKILL").await;
        let decision = decide(
            State(s.clone()),
            Extension(user("u1")),
            extract::Path(id.clone()),
            Json(Decision { decision: "accept".into(), text: None }),
        )
        .await;

        assert!(decision.is_ok());

        let overlay = overlay(&data_dir, "worker/rust/SKILL");
        assert!(overlay.contains("Bind every value."));

        let log = std::fs::read_to_string(data_dir.join("lessons-learned.md")).unwrap();
        assert!(log.contains("SQL was formatted"));

        let row: Option<(String,)> = sqlx::query_as::<_, (String,)>(
            "SELECT state FROM lessons WHERE id = ?",
        )
        .bind(&id)
        .fetch_optional(&db)
        .await
        .unwrap();
        assert_eq!(row.unwrap().0, "accepted");

        cleanup(&data_dir);
    }

    #[tokio::test]
    async fn an_edited_lesson_is_saved_as_edited() {
        let _env = ENV.lock().await;
        let db = db().await;
        let s = state(db.clone());
        let data_dir = data_dir_for_test("edited-lesson");
        let _ = cleanup(&data_dir);

        propose(
            &s,
            "u1",
            "p1",
            "r1",
            "t1",
            "worker/rust/SKILL",
            "Bind every value.",
            "SQL was formatted",
        )
        .await;

        let id = lesson_id(&db, "worker/rust/SKILL").await;
        let decision = decide(
            State(s.clone()),
            Extension(user("u1")),
            extract::Path(id.clone()),
            Json(Decision {
                decision: "accept".into(),
                text: Some("Bind values, never format them.".into()),
            }),
        )
        .await;

        assert!(decision.is_ok());

        let overlay = overlay(&data_dir, "worker/rust/SKILL");
        assert!(overlay.contains("never format"));
        assert!(!overlay.contains("Bind every value."));

        cleanup(&data_dir);
    }

    #[tokio::test]
    async fn unknown_cards_and_second_decisions_are_refused() {
        let _env = ENV.lock().await;
        let db = db().await;
        let s = state(db.clone());
        let data_dir = data_dir_for_test("unknown-cards");
        let _ = cleanup(&data_dir);

        propose(
            &s,
            "u1",
            "p1",
            "r1",
            "t1",
            "../../etc/passwd",
            "x",
            "y",
        )
        .await;

        let id = lesson_id(&db, "../../etc/passwd").await;
        let result = decide(
            State(s.clone()),
            Extension(user("u1")),
            extract::Path(id.clone()),
            Json(Decision { decision: "accept".into(), text: None }),
        )
        .await;
        assert!(result.is_err());

        propose(
            &s,
            "u1",
            "p1",
            "r1",
            "t1",
            "worker/rust/SKILL",
            "x",
            "y",
        )
        .await;

        let id2 = lesson_id(&db, "worker/rust/SKILL").await;
        let dismiss = decide(
            State(s.clone()),
            Extension(user("u1")),
            extract::Path(id2.clone()),
            Json(Decision { decision: "dismiss".into(), text: None }),
        )
        .await;
        assert!(dismiss.is_ok());

        let result2 = decide(
            State(s.clone()),
            Extension(user("u1")),
            extract::Path(id2.clone()),
            Json(Decision { decision: "accept".into(), text: None }),
        )
        .await;
        assert!(result2.is_err());

        cleanup(&data_dir);
    }

    #[tokio::test]
    async fn others_cannot_decide() {
        let _env = ENV.lock().await;
        let db = db().await;
        let s = state(db.clone());
        let data_dir = data_dir_for_test("others-cannot");
        let _ = cleanup(&data_dir);

        propose(
            &s,
            "u1",
            "p1",
            "r1",
            "t1",
            "worker/rust/SKILL",
            "x",
            "y",
        )
        .await;

        let id = lesson_id(&db, "worker/rust/SKILL").await;
        let result = decide(
            State(s.clone()),
            Extension(user("u2")),
            extract::Path(id.clone()),
            Json(Decision { decision: "accept".into(), text: None }),
        )
        .await;
        assert!(result.is_err());

        cleanup(&data_dir);
    }
}
