//! JSON API used by the web app. Field names match web/src/api/types.ts.

use std::{convert::Infallible, time::Instant};

use axum::{
    Extension, Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{
        IntoResponse,
        sse::{Event as SseEvent, KeepAlive, Sse},
    },
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};

use crate::{
    AppState,
    auth::User,
    config::ProviderKind,
    error::{ApiError, ApiResult},
    events::Event,
    llm::{self, ChatMessage, Chunk},
    util,
};

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub description: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Chat {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project_id: Option<String>,
    pub updated_at: String,
    #[sqlx(default)]
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub chat_id: String,
    pub author: String,
    pub text: String,
    pub at: String,
    #[sqlx(skip)]
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub streaming: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct RoleAssignment {
    pub role: String,
    pub provider_id: String,
    pub model_id: String,
}

const ORCHESTRATOR_PROMPT: &str = "You are Kreative Kompanion's orchestrator. You talk with the user about \
their projects, plan work and split it into small, self-contained tasks. Be direct and concise. When you \
are unsure, ask one clear question. Never claim you ran a command or changed a file; tools are not \
connected yet.";

// ---- Projects and chats ----

pub async fn projects(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
) -> ApiResult<Json<Vec<Project>>> {
    let rows = sqlx::query_as(
        "SELECT id, name, description, updated_at FROM projects WHERE user_id = ? ORDER BY updated_at DESC",
    )
    .bind(&u.id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

pub async fn chats(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
) -> ApiResult<Json<Vec<Chat>>> {
    let rows = sqlx::query_as(
        "SELECT id, title, project_id, updated_at, pinned FROM chats\n         WHERE user_id = ? AND archived = 0 ORDER BY pinned DESC, updated_at DESC",
    )
    .bind(&u.id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewChat {
    title: String,
    project_id: Option<String>,
}

pub async fn create_chat(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Json(b): Json<NewChat>,
) -> ApiResult<(StatusCode, Json<Chat>)> {
    let title: String = b.title.trim().chars().take(120).collect();
    if title.is_empty() {
        return Err(ApiError::BadRequest("Give the chat a title.".into()));
    }
    if let Some(p) = &b.project_id
        && !owns(&s, "projects", p, &u).await?
    {
        return Err(ApiError::NotFound);
    }
    let chat = Chat {
        id: util::new_id(),
        title,
        project_id: b.project_id,
        updated_at: util::now(),
        pinned: false,
    };
    sqlx::query(
        "INSERT INTO chats (id, project_id, title, updated_at, user_id) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(&chat.id)
    .bind(&chat.project_id)
    .bind(&chat.title)
    .bind(&chat.updated_at)
    .bind(&u.id)
    .execute(&s.db)
    .await?;
    Ok((StatusCode::CREATED, Json(chat)))
}

pub async fn messages(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(chat_id): Path<String>,
) -> ApiResult<Json<Vec<Message>>> {
    if !owns(&s, "chats", &chat_id, &u).await? {
        return Err(ApiError::NotFound);
    }
    let rows = sqlx::query_as(
        "SELECT id, chat_id, author, text, at FROM messages WHERE chat_id = ? ORDER BY at, rowid",
    )
    .bind(chat_id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

#[derive(Deserialize)]
pub struct SendBody {
    text: String,
}

/// Stores the user's message, then streams the orchestrator's answer as
/// events. Returns right away; the reply arrives over /api/events.
pub async fn send(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(chat_id): Path<String>,
    Json(b): Json<SendBody>,
) -> ApiResult<StatusCode> {
    let text = b.text.trim().to_string();
    if text.is_empty() || text.len() > 100_000 {
        return Err(ApiError::BadRequest(
            "Messages must be 1 to 100,000 characters.".into(),
        ));
    }
    if !owns(&s, "chats", &chat_id, &u).await? {
        return Err(ApiError::NotFound);
    }

    let user_msg = insert_message(&s, &chat_id, "user", &text).await?;
    s.bus.send(&u.id, Event::Message { message: user_msg });

    let role = user_roles(&s, &u.id)
        .await?
        .into_iter()
        .find(|r| r.role == "orchestrator");
    let Some(role) = role else {
        let m = insert_message(
            &s,
            &chat_id,
            "orchestrator",
            "No model is set for the orchestrator role yet. Pick one under Models and roles.",
        )
        .await?;
        s.bus.send(&u.id, Event::Message { message: m });
        return Ok(StatusCode::ACCEPTED);
    };

    tokio::spawn(answer(s.clone(), u.id.clone(), chat_id, role));
    Ok(StatusCode::ACCEPTED)
}

async fn insert_message(
    s: &AppState,
    chat_id: &str,
    author: &str,
    text: &str,
) -> ApiResult<Message> {
    let m = Message {
        id: util::new_id(),
        chat_id: chat_id.to_string(),
        author: author.to_string(),
        text: text.to_string(),
        at: util::now(),
        streaming: false,
    };
    sqlx::query("INSERT INTO messages (id, chat_id, author, text, at) VALUES (?, ?, ?, ?, ?)")
        .bind(&m.id)
        .bind(&m.chat_id)
        .bind(&m.author)
        .bind(&m.text)
        .bind(&m.at)
        .execute(&s.db)
        .await?;
    sqlx::query("UPDATE chats SET updated_at = ? WHERE id = ?")
        .bind(&m.at)
        .bind(chat_id)
        .execute(&s.db)
        .await?;
    Ok(m)
}

async fn answer(s: AppState, user_id: String, chat_id: String, role: RoleAssignment) {
    let reply_id = util::new_id();
    let at = util::now();
    s.bus.send(
        &user_id,
        Event::Message {
            message: Message {
                id: reply_id.clone(),
                chat_id: chat_id.clone(),
                author: "orchestrator".into(),
                text: String::new(),
                at: at.clone(),
                streaming: true,
            },
        },
    );

    // Conversation so far, oldest first (last 40 messages).
    let history: Vec<(String, String)> = sqlx::query_as(
        "SELECT author, text FROM (SELECT author, text, at, rowid FROM messages WHERE chat_id = ?
         ORDER BY at DESC, rowid DESC LIMIT 40) ORDER BY at, rowid",
    )
    .bind(&chat_id)
    .fetch_all(&s.db)
    .await
    .unwrap_or_default();
    let mut convo = vec![ChatMessage {
        role: "system".into(),
        content: ORCHESTRATOR_PROMPT.into(),
    }];
    convo.extend(history.into_iter().map(|(author, text)| ChatMessage {
        role: if author == "user" {
            "user".into()
        } else {
            "assistant".into()
        },
        content: text,
    }));

    let started = Instant::now();
    let mut text = String::new();
    let (mut tokens_in, mut tokens_out) = (None, None);
    let mut error: Option<String> = None;

    match s.config.provider(&role.provider_id) {
        None => error = Some(format!("provider {} is not configured", role.provider_id)),
        Some(p) => match llm::stream_chat(&s.http, p, &role.model_id, &convo).await {
            Err(e) => error = Some(format!("{e:#}")),
            Ok(stream) => {
                tokio::pin!(stream);
                while let Some(chunk) = stream.next().await {
                    match chunk {
                        Ok(Chunk::Text(t)) => {
                            text.push_str(&t);
                            s.bus.send(
                                &user_id,
                                Event::MessageDelta {
                                    message_id: reply_id.clone(),
                                    chat_id: chat_id.clone(),
                                    text: t,
                                    done: false,
                                },
                            );
                        }
                        Ok(Chunk::Usage {
                            tokens_in: i,
                            tokens_out: o,
                        }) => {
                            tokens_in = i.or(tokens_in);
                            tokens_out = o.or(tokens_out);
                        }
                        Err(e) => {
                            error = Some(format!("{e:#}"));
                            break;
                        }
                    }
                }
            }
        },
    }

    if let Some(e) = &error {
        tracing::warn!(error = %e, model = %role.model_id, "model call failed");
        let note = format!(
            "\n\nI couldn't reach `{}`: {}",
            role.model_id,
            e.chars().take(300).collect::<String>()
        );
        text.push_str(&note);
        s.bus.send(
            &user_id,
            Event::MessageDelta {
                message_id: reply_id.clone(),
                chat_id: chat_id.clone(),
                text: note,
                done: false,
            },
        );
    }
    s.bus.send(
        &user_id,
        Event::MessageDelta {
            message_id: reply_id.clone(),
            chat_id: chat_id.clone(),
            text: String::new(),
            done: true,
        },
    );

    let saved = sqlx::query(
        "INSERT INTO messages (id, chat_id, author, text, at) VALUES (?, ?, 'orchestrator', ?, ?)",
    )
    .bind(&reply_id)
    .bind(&chat_id)
    .bind(&text)
    .bind(&at)
    .execute(&s.db)
    .await;
    if let Err(e) = saved {
        tracing::error!(error = ?e, "saving reply failed");
    }

    // Every call is recorded in full (see the call inspector in the app).
    let request = json!({ "messages": convo });
    let _ = sqlx::query(
        "INSERT INTO calls (user_id, id, chat_id, role, provider_id, model_id, reason, request, response,
         tokens_in, tokens_out, ms, error, at) VALUES (?, ?, ?, 'orchestrator', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&user_id)
    .bind(util::new_id())
    .bind(&chat_id)
    .bind(&role.provider_id)
    .bind(&role.model_id)
    .bind("You sent a message in this chat.")
    .bind(request.to_string())
    .bind(&text)
    .bind(tokens_in)
    .bind(tokens_out)
    .bind(started.elapsed().as_millis() as i64)
    .bind(&error)
    .bind(util::now())
    .execute(&s.db)
    .await;
}

// ---- Models and roles ----

pub async fn providers(State(s): State<AppState>) -> Json<Vec<Value>> {
    let probes = s.config.providers.iter().map(|p| {
        let http = s.http.clone();
        async move {
            let (models, error) = match llm::list_models(&http, p).await {
                Ok(m) => match llm::check_chat_auth(&http, p).await {
                    Ok(()) => (m, None),
                    Err(e) => (m, Some(format!("{e:#}"))),
                },
                Err(e) => (vec![], Some(format!("{e:#}"))),
            };
            json!({
                "id": p.id,
                "name": p.name,
                "kind": match p.kind { ProviderKind::OpenaiCompatible => "openai-compatible", ProviderKind::Anthropic => "anthropic" },
                "baseUrl": p.base_url,
                "local": p.local,
                "hasKey": p.api_key().is_some(),
                "error": error,
                "models": models.iter().map(|m| json!({ "id": m.id })).collect::<Vec<_>>(),
            })
        }
    });
    Json(futures::future::join_all(probes).await)
}

/// The user's roles; roles they haven't set yet come from the config file.
async fn user_roles(s: &AppState, user_id: &str) -> ApiResult<Vec<RoleAssignment>> {
    let mut rows: Vec<RoleAssignment> =
        sqlx::query_as("SELECT role, provider_id, model_id FROM user_roles WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(&s.db)
            .await?;
    for (role, d) in &s.config.roles {
        if !rows.iter().any(|r| &r.role == role) {
            rows.push(RoleAssignment {
                role: role.clone(),
                provider_id: d.provider.clone(),
                model_id: d.model.clone(),
            });
        }
    }
    rows.sort_by(|a, b| a.role.cmp(&b.role));
    Ok(rows)
}

/// Whether `id` in `table` (projects, chats or tasks) belongs to this user.
#[derive(Deserialize)]
pub struct ChatChange {
    title: Option<String>,
    pinned: Option<bool>,
    archived: Option<bool>,
}

/// Rename, pin or archive a chat (the chat menu).
pub async fn update_chat(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
    Json(b): Json<ChatChange>,
) -> ApiResult<StatusCode> {
    if !owns(&s, "chats", &id, &u).await? {
        return Err(ApiError::NotFound);
    }
    if let Some(t) = b.title {
        let t: String = t.trim().chars().take(120).collect();
        if t.is_empty() {
            return Err(ApiError::BadRequest("Give the chat a title.".into()));
        }
        sqlx::query("UPDATE chats SET title = ? WHERE id = ?")
            .bind(t)
            .bind(&id)
            .execute(&s.db)
            .await?;
    }
    if let Some(p) = b.pinned {
        sqlx::query("UPDATE chats SET pinned = ? WHERE id = ?")
            .bind(p)
            .bind(&id)
            .execute(&s.db)
            .await?;
    }
    if let Some(a) = b.archived {
        sqlx::query("UPDATE chats SET archived = ? WHERE id = ?")
            .bind(a)
            .bind(&id)
            .execute(&s.db)
            .await?;
    }
    Ok(StatusCode::NO_CONTENT)
}

/// Delete a chat and its messages.
pub async fn delete_chat(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Path(id): Path<String>,
) -> ApiResult<StatusCode> {
    let done = sqlx::query("DELETE FROM chats WHERE id = ? AND user_id = ?")
        .bind(&id)
        .bind(&u.id)
        .execute(&s.db)
        .await?;
    if done.rows_affected() == 0 {
        return Err(ApiError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn owns(s: &AppState, table: &str, id: &str, u: &User) -> ApiResult<bool> {
    let sql = match table {
        "projects" => "SELECT 1 FROM projects WHERE id = ? AND user_id = ?",
        "chats" => "SELECT 1 FROM chats WHERE id = ? AND user_id = ?",
        "tasks" => "SELECT 1 FROM tasks WHERE id = ? AND user_id = ?",
        _ => unreachable!("unknown table"),
    };
    let row: Option<(i64,)> = sqlx::query_as(sql)
        .bind(id)
        .bind(&u.id)
        .fetch_optional(&s.db)
        .await?;
    Ok(row.is_some())
}

pub async fn roles(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
) -> ApiResult<Json<Vec<RoleAssignment>>> {
    Ok(Json(user_roles(&s, &u.id).await?))
}

pub async fn set_role(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Json(r): Json<RoleAssignment>,
) -> ApiResult<StatusCode> {
    if !["orchestrator", "worker", "reviewer"].contains(&r.role.as_str()) {
        return Err(ApiError::BadRequest("Unknown role.".into()));
    }
    if s.config.provider(&r.provider_id).is_none() {
        return Err(ApiError::BadRequest("Unknown provider.".into()));
    }
    sqlx::query(
        "INSERT INTO user_roles (user_id, role, provider_id, model_id) VALUES (?, ?, ?, ?)
         ON CONFLICT(user_id, role) DO UPDATE SET provider_id = excluded.provider_id, model_id = excluded.model_id",
    )
    .bind(&u.id)
    .bind(&r.role)
    .bind(&r.provider_id)
    .bind(&r.model_id)
    .execute(&s.db)
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---- Observability ----

#[derive(Deserialize)]
pub struct CallQuery {
    limit: Option<i64>,
    chat: Option<String>,
}

pub async fn calls(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Query(q): Query<CallQuery>,
) -> ApiResult<Json<Vec<Value>>> {
    let limit = q.limit.unwrap_or(50).clamp(1, 500);
    type Row = (
        String,
        Option<String>,
        String,
        String,
        String,
        String,
        String,
        String,
        Option<i64>,
        Option<i64>,
        i64,
        Option<String>,
        String,
    );
    let rows: Vec<Row> =
        sqlx::query_as(
            "SELECT id, chat_id, role, provider_id, model_id, reason, request, response, tokens_in, tokens_out, ms, error, at
             FROM calls WHERE user_id = ?3 AND (?1 IS NULL OR chat_id = ?1) ORDER BY at DESC LIMIT ?2",
        )
        .bind(&q.chat)
        .bind(limit)
        .bind(&u.id)
        .fetch_all(&s.db)
        .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, chat_id, role, provider, model, reason, request, response, ti, to, ms, error, at)| {
                json!({
                    "id": id, "chatId": chat_id, "role": role, "provider": provider, "model": model,
                    "reason": reason, "request": serde_json::from_str::<Value>(&request).unwrap_or(Value::Null),
                    "response": response, "tokensIn": ti, "tokensOut": to, "ms": ms, "error": error, "at": at,
                })
            })
            .collect(),
    ))
}

// Tasks and machines arrive with milestones 2 and 3.
#[derive(Deserialize)]
pub struct TasksQuery {
    project: Option<String>,
}

pub async fn tasks(
    State(s): State<AppState>,
    Extension(u): Extension<User>,
    Query(q): Query<TasksQuery>,
) -> ApiResult<Json<Vec<Value>>> {
    type Row = (String, String, String, String, f64, String, String, String);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, project_id, title, state, progress, step, role, model FROM tasks
         WHERE user_id = ?2 AND (?1 IS NULL OR project_id = ?1) ORDER BY updated_at DESC LIMIT 500",
    )
    .bind(q.project)
    .bind(&u.id)
    .fetch_all(&s.db)
    .await?;
    Ok(Json(
        rows.into_iter()
            .map(|(id, project_id, title, state, progress, step, role, model)| {
                json!({ "id": id, "projectId": project_id, "title": title, "state": state,
                        "progress": progress, "step": step, "role": role, "model": model, "events": [] })
            })
            .collect(),
    ))
}
/// The server's own machine; runners on other PCs come in milestone 3.
pub async fn machines(State(s): State<AppState>) -> Json<Vec<Value>> {
    Json(vec![s.host.snapshot_now(&s)])
}

/// Heartbeat from a Machines tab set to "Live": stats are pushed over the
/// event stream every second for the next 15 s.
pub async fn machines_live(State(s): State<AppState>, Extension(u): Extension<User>) -> StatusCode {
    s.host.watch(&u.id);
    StatusCode::NO_CONTENT
}

// ---- Live events ----

pub async fn events(State(s): State<AppState>, Extension(u): Extension<User>) -> impl IntoResponse {
    let me = u.id;
    let stream = BroadcastStream::new(s.bus.subscribe()).filter_map(move |e| {
        let me = me.clone();
        async move {
            match e {
                Ok((user_id, _)) if user_id != me => None,
                Ok((_, event)) => Some(Ok::<_, Infallible>(
                    SseEvent::default().json_data(event).unwrap_or_default(),
                )),
                // A slow client missed events; tell it to reload instead of guessing.
                Err(BroadcastStreamRecvError::Lagged(_)) => {
                    Some(Ok(SseEvent::default().event("resync").data("{}")))
                }
            }
        }
    });
    // Tell nginx not to buffer the stream, or events arrive in bursts.
    (
        [("x-accel-buffering", "no")],
        Sse::new(stream).keep_alive(KeepAlive::default()),
    )
}
