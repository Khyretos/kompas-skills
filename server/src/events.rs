//! Live events for the web app, sent over server-sent events.
//! Shapes match `ServerEvent` in web/src/api/client.ts.

use serde::Serialize;
use tokio::sync::broadcast;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Event {
    Message {
        message: crate::api::Message,
    },
    #[serde(rename_all = "camelCase")]
    MessageDelta {
        message_id: String,
        chat_id: String,
        text: String,
        done: bool,
    },
}

#[derive(Clone)]
pub struct Bus(broadcast::Sender<Event>);

impl Bus {
    pub fn new() -> Self {
        Self(broadcast::channel(1024).0)
    }
    pub fn send(&self, e: Event) {
        let _ = self.0.send(e); // no listeners is fine
    }
    pub fn subscribe(&self) -> broadcast::Receiver<Event> {
        self.0.subscribe()
    }
}
