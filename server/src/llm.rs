//! Model providers. Two adapters cover everything: OpenAI-compatible
//! (OVMS, Ollama, llama.cpp, vLLM, DeepSeek, OpenRouter...) and Anthropic.

use std::time::Duration;

use anyhow::{Context, Result, anyhow, bail};
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::config::{ProviderConfig, ProviderKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "system" | "user" | "assistant"
    pub content: String,
}

#[derive(Debug, Clone)]
pub enum Chunk {
    Text(String),
    Usage {
        tokens_in: Option<i64>,
        tokens_out: Option<i64>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: String,
}

pub fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(300))
        .redirect(reqwest::redirect::Policy::none())
        .user_agent(concat!("kreative-kompanion/", env!("CARGO_PKG_VERSION")))
        .build()
        .expect("http client")
}

fn join(base: &str, path: &str) -> String {
    format!(
        "{}/{}",
        base.trim_end_matches('/'),
        path.trim_start_matches('/')
    )
}

/// Lists the models a provider offers (used for setup and the settings screen).
pub async fn list_models(http: &reqwest::Client, p: &ProviderConfig) -> Result<Vec<ModelInfo>> {
    let mut req = http.get(join(
        &p.base_url,
        match p.kind {
            ProviderKind::OpenaiCompatible => "models",
            ProviderKind::Anthropic => "v1/models",
        },
    ));
    req = authorize(req, p);
    let resp = req.timeout(Duration::from_secs(10)).send().await?;
    if !resp.status().is_success() {
        bail!("{} answered {}", p.name, resp.status());
    }
    let body: Value = resp.json().await?;
    let list = body["data"].as_array().cloned().unwrap_or_default();
    Ok(list
        .into_iter()
        .filter_map(|m| m["id"].as_str().map(|id| ModelInfo { id: id.to_string() }))
        .collect())
}

/// Some servers (OVMS among them) list models without a key but refuse chats.
/// Sends an empty chat request, which a server rejects before running any
/// model, and reports when the rejection is about the key rather than the body.
pub async fn check_chat_auth(http: &reqwest::Client, p: &ProviderConfig) -> Result<()> {
    if !matches!(p.kind, ProviderKind::OpenaiCompatible) {
        return Ok(()); // Anthropic already needs the key to list models.
    }
    let resp = authorize(http.post(join(&p.base_url, "chat/completions")), p)
        .json(&json!({ "model": "", "messages": [], "max_tokens": 1 }))
        .timeout(Duration::from_secs(10))
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if is_auth_error(status, &text) {
        bail!(
            "{} refuses chats without a valid API key ({status}). Set api_key_env for this provider.",
            p.name
        );
    }
    Ok(())
}

fn is_auth_error(status: reqwest::StatusCode, body: &str) -> bool {
    let body = body.to_ascii_lowercase();
    status == reqwest::StatusCode::UNAUTHORIZED
        || status == reqwest::StatusCode::FORBIDDEN
        || body.contains("api-key")
        || body.contains("api key")
        || body.contains("unauthorized")
}

fn authorize(req: reqwest::RequestBuilder, p: &ProviderConfig) -> reqwest::RequestBuilder {
    match (p.kind.clone(), p.api_key()) {
        (ProviderKind::OpenaiCompatible, Some(key)) => req.bearer_auth(key),
        (ProviderKind::Anthropic, Some(key)) => req
            .header("x-api-key", key)
            .header("anthropic-version", "2023-06-01"),
        (ProviderKind::Anthropic, None) => req.header("anthropic-version", "2023-06-01"),
        (ProviderKind::OpenaiCompatible, None) => req,
    }
}

/// Streams a chat completion as text chunks plus a final usage chunk.
pub async fn stream_chat(
    http: &reqwest::Client,
    p: &ProviderConfig,
    model: &str,
    messages: &[ChatMessage],
) -> Result<impl Stream<Item = Result<Chunk>> + Send + 'static> {
    let (url, body) = match p.kind {
        ProviderKind::OpenaiCompatible => (
            join(&p.base_url, "chat/completions"),
            json!({
                "model": model,
                "messages": messages,
                "stream": true,
                "stream_options": { "include_usage": true },
            }),
        ),
        ProviderKind::Anthropic => {
            let system: Vec<&str> = messages
                .iter()
                .filter(|m| m.role == "system")
                .map(|m| m.content.as_str())
                .collect();
            let rest: Vec<&ChatMessage> = messages.iter().filter(|m| m.role != "system").collect();
            (
                join(&p.base_url, "v1/messages"),
                json!({
                    "model": model,
                    "max_tokens": 4096,
                    "system": system.join("\n\n"),
                    "messages": rest,
                    "stream": true,
                }),
            )
        }
    };

    let mut body = body;
    if let (Some(extra), Some(obj)) = (&p.extra_body, body.as_object_mut()) {
        for (k, v) in serde_json::to_value(extra)?
            .as_object()
            .into_iter()
            .flatten()
        {
            obj.insert(k.clone(), v.clone());
        }
    }
    let resp = authorize(http.post(url), p)
        .json(&body)
        .send()
        .await
        .context("model request failed")?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        bail!(
            "{} answered {status}: {}",
            p.name,
            text.chars().take(300).collect::<String>()
        );
    }

    let kind = p.kind.clone();
    let mut bytes = resp.bytes_stream();
    let stream = async_stream(move |tx| async move {
        let mut buf = String::new();
        while let Some(chunk) = bytes.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    let _ = tx.send(Err(anyhow!(e))).await;
                    return;
                }
            };
            buf.push_str(&String::from_utf8_lossy(&chunk));
            // Server-sent events are separated by a blank line.
            while let Some(end) = buf.find("\n\n") {
                let event: String = buf.drain(..end + 2).collect();
                for line in event.lines() {
                    let Some(data) = line.strip_prefix("data:") else {
                        continue;
                    };
                    let data = data.trim();
                    if data == "[DONE]" || data.is_empty() {
                        continue;
                    }
                    let Ok(v) = serde_json::from_str::<Value>(data) else {
                        continue;
                    };
                    for c in parse_event(&kind, &v) {
                        if tx.send(Ok(c)).await.is_err() {
                            return;
                        }
                    }
                }
            }
        }
    });
    Ok(stream)
}

fn parse_event(kind: &ProviderKind, v: &Value) -> Vec<Chunk> {
    let mut out = Vec::new();
    match kind {
        ProviderKind::OpenaiCompatible => {
            if let Some(text) = v["choices"][0]["delta"]["content"].as_str()
                && !text.is_empty()
            {
                out.push(Chunk::Text(text.to_string()));
            }
            if v["usage"].is_object() {
                out.push(Chunk::Usage {
                    tokens_in: v["usage"]["prompt_tokens"].as_i64(),
                    tokens_out: v["usage"]["completion_tokens"].as_i64(),
                });
            }
        }
        ProviderKind::Anthropic => match v["type"].as_str() {
            Some("content_block_delta") => {
                if let Some(text) = v["delta"]["text"].as_str() {
                    out.push(Chunk::Text(text.to_string()));
                }
            }
            Some("message_start") => out.push(Chunk::Usage {
                tokens_in: v["message"]["usage"]["input_tokens"].as_i64(),
                tokens_out: None,
            }),
            Some("message_delta") => out.push(Chunk::Usage {
                tokens_in: None,
                tokens_out: v["usage"]["output_tokens"].as_i64(),
            }),
            _ => {}
        },
    }
    out
}

/// Small helper: run a producer task and expose its output as a Stream.
fn async_stream<T, F, Fut>(f: F) -> impl Stream<Item = T> + Send + 'static
where
    T: Send + 'static,
    F: FnOnce(tokio::sync::mpsc::Sender<T>) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let (tx, rx) = tokio::sync::mpsc::channel(64);
    tokio::spawn(f(tx));
    tokio_stream::wrappers::ReceiverStream::new(rx)
}

#[cfg(test)]
mod tests {
    #[test]
    fn spots_key_errors() {
        use reqwest::StatusCode;
        let ovms = r#"{"error":"Unauthorized request due to invalid or missing api-key"}"#;
        assert!(super::is_auth_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ovms
        ));
        assert!(super::is_auth_error(StatusCode::UNAUTHORIZED, ""));
        assert!(!super::is_auth_error(
            StatusCode::BAD_REQUEST,
            r#"{"error":"model not found"}"#
        ));
    }

    use super::*;

    #[test]
    fn parses_openai_delta_and_usage() {
        let v: Value = serde_json::from_str(r#"{"choices":[{"delta":{"content":"Hi"}}]}"#).unwrap();
        assert!(
            matches!(&parse_event(&ProviderKind::OpenaiCompatible, &v)[..], [Chunk::Text(t)] if t == "Hi")
        );
        let v: Value = serde_json::from_str(
            r#"{"choices":[],"usage":{"prompt_tokens":5,"completion_tokens":2}}"#,
        )
        .unwrap();
        assert!(matches!(
            &parse_event(&ProviderKind::OpenaiCompatible, &v)[..],
            [Chunk::Usage {
                tokens_in: Some(5),
                tokens_out: Some(2)
            }]
        ));
    }

    #[test]
    fn parses_anthropic_delta() {
        let v: Value = serde_json::from_str(
            r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"Yo"}}"#,
        )
        .unwrap();
        assert!(
            matches!(&parse_event(&ProviderKind::Anthropic, &v)[..], [Chunk::Text(t)] if t == "Yo")
        );
    }
}
