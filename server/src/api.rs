//! JSON API used by the web app. Field names match web/src/api/types.ts.

use std::{convert::Infallible, time::Instant};

use axum::{
    Json,
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

pub async fn projects(State(s): State<AppState>) -> ApiResult<Json<Vec<Project>>> {
    let rows = sqlx::query_as(
        "SELECT id, name, description, updated_at FROM projects ORDER BY updated_at DESC",
    )
    .fetch_all(&s.db)
    .await?;
    Ok(Json(rows))
}

pub async fn chats(State(s): State<AppState>) -> ApiResult<Json<Vec<Chat>>> {
    let rows = sqlx::query_as(
        "SELECT id, title, project_id, updated_at FROM chats ORDER BY updated_at DESC",
    )
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
    Json(b): Json<NewChat>,
) -> ApiResult<(StatusCode, Json<Chat>)> {
    let title: String = b.title.trim().chars().take(120).collect();
    if title.is_empty() {
        return Err(ApiError::BadRequest("Give the chat a title.".into()));
    }
    let chat = Chat {
        id: util::new_id(),
        title,
        project_id: b.project_id,
        updated_at: util::now(),
    };
    sqlx::query("INSERT INTO chats (id, project_id, title, updated_at) VALUES (?, ?, ?, ?)")
        .bind(&chat.id)
        .bind(&chat.project_id)
        .bind(&chat.title)
        .bind(&chat.updated_at)
        .execute(&s.db)
        .await?;
    Ok((StatusCode::CREATED, Json(chat)))
}

pub async fn messages(
    State(s): State<AppState>,
    Path(chat_id): Path<String>,
) -> ApiResult<Json<Vec<Message>>> {
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
    Path(chat_id): Path<String>,
    Json(b): Json<SendBody>,
) -> ApiResult<StatusCode> {
    let text = b.text.trim().to_string();
    if text.is_empty() || text.len() > 100_000 {
        return Err(ApiError::BadRequest(
            "Messages must be 1 to 100,000 characters.".into(),
        ));
    }
    let exists: Option<(String,)> = sqlx::query_as("SELECT id FROM chats WHERE id = ?")
        .bind(&chat_id)
        .fetch_optional(&s.db)
        .await?;
    if exists.is_none() {
        return Err(ApiError::NotFound);
    }

    let user_msg = insert_message(&s, &chat_id, "user", &text).await?;
    s.bus.send(Event::Message { message: user_msg });

    let role: Option<RoleAssignment> =
        sqlx::query_as("SELECT role, provider_id, model_id FROM roles WHERE role = 'orchestrator'")
            .fetch_optional(&s.db)
            .await?;
    let Some(role) = role else {
        let m = insert_message(
            &s,
            &chat_id,
            "orchestrator",
            "No model is set for the orchestrator role yet. Pick one under Models and roles.",
        )
        .await?;
        s.bus.send(Event::Message { message: m });
        return Ok(StatusCode::ACCEPTED);
    };

    tokio::spawn(answer(s.clone(), chat_id, role));
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

async fn answer(s: AppState, chat_id: String, role: RoleAssignment) {
    let reply_id = util::new_id();
    let at = util::now();
    s.bus.send(Event::Message {
        message: Message {
            id: reply_id.clone(),
            chat_id: chat_id.clone(),
            author: "orchestrator".into(),
            text: String::new(),
            at: at.clone(),
            streaming: true,
        },
    });

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
                            s.bus.send(Event::MessageDelta {
                                message_id: reply_id.clone(),
                                chat_id: chat_id.clone(),
                                text: t,
                                done: false,
                            });
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
        s.bus.send(Event::MessageDelta {
            message_id: reply_id.clone(),
            chat_id: chat_id.clone(),
            text: note,
            done: false,
        });
    }
    s.bus.send(Event::MessageDelta {
        message_id: reply_id.clone(),
        chat_id: chat_id.clone(),
        text: String::new(),
        done: true,
    });

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
        "INSERT INTO calls (id, chat_id, role, provider_id, model_id, reason, request, response,
         tokens_in, tokens_out, ms, error, at) VALUES (?, ?, 'orchestrator', ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
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

pub async fn roles(State(s): State<AppState>) -> ApiResult<Json<Vec<RoleAssignment>>> {
    let rows = sqlx::query_as("SELECT role, provider_id, model_id FROM roles ORDER BY role")
        .fetch_all(&s.db)
        .await?;
    Ok(Json(rows))
}

pub async fn set_role(
    State(s): State<AppState>,
    Json(r): Json<RoleAssignment>,
) -> ApiResult<StatusCode> {
    if !["orchestrator", "worker", "reviewer"].contains(&r.role.as_str()) {
        return Err(ApiError::BadRequest("Unknown role.".into()));
    }
    if s.config.provider(&r.provider_id).is_none() {
        return Err(ApiError::BadRequest("Unknown provider.".into()));
    }
    sqlx::query(
        "INSERT INTO roles (role, provider_id, model_id) VALUES (?, ?, ?)
         ON CONFLICT(role) DO UPDATE SET provider_id = excluded.provider_id, model_id = excluded.model_id",
    )
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
             FROM calls WHERE (?1 IS NULL OR chat_id = ?1) ORDER BY at DESC LIMIT ?2",
        )
        .bind(&q.chat)
        .bind(limit)
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
    Query(q): Query<TasksQuery>,
) -> ApiResult<Json<Vec<Value>>> {
    type Row = (String, String, String, String, f64, String, String, String);
    let rows: Vec<Row> = sqlx::query_as(
        "SELECT id, project_id, title, state, progress, step, role, model FROM tasks
         WHERE ?1 IS NULL OR project_id = ?1 ORDER BY updated_at DESC LIMIT 500",
    )
    .bind(q.project)
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
pub async fn machines() -> Json<Vec<Value>> {
    Json(vec![])
}

// ---- Live events ----

pub async fn events(State(s): State<AppState>) -> impl IntoResponse {
    let stream = BroadcastStream::new(s.bus.subscribe()).filter_map(|e| async move {
        match e {
            Ok(event) => Some(Ok::<_, Infallible>(
                SseEvent::default().json_data(event).unwrap_or_default(),
            )),
            // A slow client missed events; tell it to reload instead of guessing.
            Err(BroadcastStreamRecvError::Lagged(_)) => {
                Some(Ok(SseEvent::default().event("resync").data("{}")))
            }
        }
    });
    // Tell nginx not to buffer the stream, or events arrive in bursts.
    (
        [("x-accel-buffering", "no")],
        Sse::new(stream).keep_alive(KeepAlive::default()),
    )
}
