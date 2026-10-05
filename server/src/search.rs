//! Global search (Ctrl+K): one FTS5 index (`search_fts`, migration 0022) over the
//! signed-in user's tasks, chats, chat messages and projects.

use axum::{Extension, Json, extract::{Query, State}};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{AppState, auth::User, error::ApiResult};

#[derive(Deserialize)]
pub struct SearchQuery { q: String }

/// FTS5 query from what the user typed: every word must match, as a prefix.
/// Quotes keep FTS syntax (AND, NEAR, *, :) from being read as operators.
fn fts_query(q: &str) -> Option<String> {
    let words: Vec<String> = q
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .take(8)
        .map(|w| format!("\"{w}\"*"))
        .collect();
    (!words.is_empty()).then(|| words.join(" "))
}

/// Above this many matches the newest come first instead of the best ranked (bm25 over 50,000 matches took ~100 ms).
const RANK_LIMIT: i64 = 5000;

pub async fn find(db: &sqlx::SqlitePool, q: &str, user_id: &str) -> sqlx::Result<Vec<Value>> {
    let Some(fts) = fts_query(q) else {
        return Ok(Vec::new());
    };

    // Count matches first to decide ranking strategy
    let (matches,): (i64,) = sqlx::query_as("SELECT count(*) FROM search_fts WHERE search_fts MATCH ?1 AND user_id = ?2")
        .bind(&fts)
        .bind(user_id)
        .fetch_one(db)
        .await?;

    let order = if matches <= RANK_LIMIT {
        "bm25(search_fts, 4.0, 1.0)"
    } else {
        "rowid DESC"
    };

    let rows: Vec<(String, String, Option<String>, String, String)> = sqlx::query_as(
        format!(
            r#"
            SELECT kind, ref, parent, title, snippet(search_fts, 1, char(2), char(3), '…', 12)
            FROM search_fts
            WHERE search_fts MATCH ?1 AND user_id = ?2
            ORDER BY {order}
            LIMIT 40
            "#,
        ).as_str(),
    )
    .bind(&fts)
    .bind(user_id)
    .fetch_all(db)
    .await?;

    // Keep at most 5 messages per chat.
    let mut per_chat: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    let mut filtered = Vec::with_capacity(rows.len());
    for row in rows {
        if row.0 == "message"
            && let Some(parent) = &row.2
        {
            let n = per_chat.entry(parent.clone()).or_default();
            *n += 1;
            if *n > 5 {
                continue;
            }
        }
        filtered.push(row);
    }

    // Keep at most 25 results in all
    let results: Vec<Value> = filtered
        .into_iter()
        .take(25)
        .map(|(kind, id, parent, title, snippet)| {
            json!({
                "kind": kind,
                "id": id,
                "parent": parent,
                "title": title,
                "snippet": snippet
            })
        })
        .collect();

    Ok(results)
}

pub async fn search(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Query(p): Query<SearchQuery>,
) -> ApiResult<Json<Value>> {
    let results = find(&s.db, &p.q, &u.id).await?;
    Ok(Json(json!({ "results": results })))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fix_login() {
        let result = fts_query("fix login");
        assert_eq!(result, Some("\"fix\"* \"login\"*".to_string()));
    }

    #[test]
    fn test_semicolon_space() {
        assert_eq!(fts_query("  ;; "), None);
    }

    #[tokio::test]
    async fn finds_only_own_items_and_follows_changes() {
        let db = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!().run(&db).await.unwrap();

        sqlx::query("INSERT INTO users (id, name, password_hash, created_at) VALUES ('u1','kees','x','2026')")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO users (id, name, password_hash, created_at) VALUES ('u2','ana','x','2026')")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO projects (id, name, description, updated_at, user_id) VALUES ('p1','Website','Landing page work','2026','u1')")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO tasks (id, project_id, title, description, updated_at, user_id) VALUES ('t1','p1','Fix login button','Safari only','2026','u1')")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO chats (id, project_id, title, updated_at, user_id) VALUES ('c1','p1','Planning','2026','u1')")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO chats (id, project_id, title, updated_at, user_id) VALUES ('c2',NULL,'Secret','2026','u2')")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO messages (id, chat_id, author, text, at) VALUES ('m1','c1','user','The invoice template needs a logo','2026')")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO messages (id, chat_id, author, text, at) VALUES ('m2','c2','user','invoice for ana','2026')")
            .execute(&db)
            .await
            .unwrap();

        let r = find(&db, "invoice", "u1").await.unwrap();
        assert_eq!(r.len(), 1);
        assert_eq!(r[0]["kind"], "message");
        assert_eq!(r[0]["id"], "m1");
        assert_eq!(r[0]["parent"], "c1");
        assert_eq!(r[0]["title"], "Planning");
        assert!(r[0]["snippet"].as_str().unwrap().contains("\u{2}invoice\u{3}"));

        let r = find(&db, "log", "u1").await.unwrap();
        let ids: Vec<_> = r.iter().map(|v| v["id"].as_str().unwrap()).collect();
        assert!(ids.contains(&"t1"));
        assert!(ids.contains(&"m1"));

        sqlx::query("UPDATE chats SET title = 'Billing' WHERE id = 'c1'")
            .execute(&db)
            .await
            .unwrap();
        let r = find(&db, "invoice", "u1").await.unwrap();
        assert_eq!(r[0]["title"], "Billing");

        sqlx::query("UPDATE tasks SET title = 'Fix signup button' WHERE id = 't1'")
            .execute(&db)
            .await
            .unwrap();
        let r = find(&db, "login", "u1").await.unwrap();
        assert!(r.is_empty());
        let r = find(&db, "signup", "u1").await.unwrap();
        assert_eq!(r.len(), 1);

        sqlx::query("DELETE FROM tasks WHERE id = 't1'")
            .execute(&db)
            .await
            .unwrap();
        let r = find(&db, "signup", "u1").await.unwrap();
        assert!(r.is_empty());

        let r = find(&db, " ", "u1").await.unwrap();
        assert!(r.is_empty());
    }
}
