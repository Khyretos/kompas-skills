//! `kompanion-runner ask "..."` on a paired PC: the question goes to a chat named
//! after that computer, the PC agent answers with that computer picked, and the CLI
//! polls for the answer. Approvals stay in the web app (the machine token can't
//! approve its own steps). Authenticated with the runner's bearer token.
use std::{collections::HashSet, sync::{LazyLock, Mutex}};
use axum::{Json, extract::{Path, Query, State}, http::{HeaderMap, StatusCode}};
use serde::Deserialize;
use serde_json::{Value, json};
use crate::{AppState, access, api, error::{ApiError, ApiResult}, events::Event, pcagent, util};

static RUNNING: LazyLock<Mutex<HashSet<String>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

#[derive(Deserialize)]
pub struct AskBody {
    pub text: String,
}

pub async fn ask(
    State(s): State<AppState>,
    Path(machine_id): Path<String>,
    headers: HeaderMap,
    Json(b): Json<AskBody>,
) -> ApiResult<(StatusCode, Json<Value>)> {
    let user_id = access::runner_user(&s, &machine_id, &headers).await?;
    let text = b.text.trim();
    if text.is_empty() || text.len() > 20_000 {
        return Err(ApiError::BadRequest("Ask something (up to 20,000 characters).".into()));
    }

    let machine_name = sqlx::query_scalar::<_, String>("SELECT name FROM machines WHERE id = ?")
        .bind(&machine_id)
        .fetch_one(&s.db)
        .await
        .map_err(|_| ApiError::NotFound("Machine not found.".into()))?;

    let title = format!("kompanion ask ({})", machine_name);
    let chat_id = sqlx::query_scalar::<_, String>(
        "SELECT id FROM chats WHERE user_id = ? AND title = ? AND archived = 0 ORDER BY updated_at DESC LIMIT 1"
    )
    .bind(&user_id)
    .bind(&title)
    .fetch_optional(&s.db)
    .await?
    .map(|id| id.to_string());

    let chat_id = match chat_id {
        Some(id) => id,
        None => {
            let new_id = util::new_id();
            sqlx::query("INSERT INTO chats (id, project_id, title, updated_at, user_id) VALUES (?, NULL, ?, ?, ?)")
                .bind(&new_id)
                .bind(&title)
                .bind(util::now())
                .bind(&user_id)
                .execute(&s.db)
                .await?;
            s.bus.send(&user_id, Event::Changed { what: "chats", machine_id: None });
            new_id
        }
    };

    if RUNNING.lock().unwrap().contains(&chat_id) {
        return Err(ApiError::BadRequest("Kompanion is still working on your last question.".into()));
    }

    let role = sqlx::query_as::<_, api::RoleAssignment>(
        "SELECT role, provider_id, model_id FROM user_roles WHERE user_id = ? AND role = 'orchestrator'"
    )
    .bind(&user_id)
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| ApiError::BadRequest("No model is set for the orchestrator yet (Kompanion → Settings → Models and roles).".into()))?;

    let msg = api::insert_message(&s, &chat_id, "user", text).await?;
    s.bus.send(&user_id, Event::Message { message: msg.clone() });

    let mut running = RUNNING.lock().unwrap();
    running.insert(chat_id.clone());

    tokio::spawn(async move {
        let _ = pcagent::run(s, user_id, chat_id, machine_id, machine_name, role).await;
        let mut r = running.lock().unwrap();
        r.remove(&chat_id);
    });

    Ok((StatusCode::ACCEPTED, json!({ "chatId": chat_id, "after": msg.at })))
}

#[derive(Deserialize)]
pub struct Since {
    pub after: String,
}

pub async fn poll(
    State(s): State<AppState>,
    Path((machine_id, chat_id)): Path<(String, String)>,
    headers: HeaderMap,
    Query(q): Query<Since>,
) -> ApiResult<Json<Value>> {
    let user_id = access::runner_user(&s, &machine_id, &headers).await?;

    sqlx::query_scalar::<_, i32>(
        "SELECT 1 FROM chats WHERE id = ? AND user_id = ?"
    )
    .bind(&chat_id)
    .bind(&user_id)
    .fetch_optional(&s.db)
    .await?
    .ok_or_else(|| ApiError::NotFound("Chat not found.".into()))?;

    let messages = sqlx::query_as::<_, (String, String, String)>(
        "SELECT author, text, at FROM messages WHERE chat_id = ? AND at > ? AND author <> 'user' ORDER BY at, rowid"
    )
    .bind(&chat_id)
    .bind(&q.after)
    .fetch_all(&s.db)
    .await?
    .into_iter()
    .map(|(author, text, at)| json!({ "author": author, "text": text, "at": at }))
    .collect::<Vec<_>>();

    let pending = sqlx::query_as::<_, String>(
        "SELECT summary FROM pc_actions WHERE chat_id = ? AND state = 'pending' ORDER BY created_at"
    )
    .bind(&chat_id)
    .fetch_all(&s.db)
    .await?;

    let running = RUNNING.lock().unwrap().contains(&chat_id);

    Ok(Json(json!({
        "messages": messages,
        "pending": pending,
        "running": running
    })))
}
