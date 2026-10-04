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
    /// Live machine stats for users watching the Machines tab on "Live".
    Machines {
        machines: Vec<serde_json::Value>,
    },
    /// Assets section news for everyone signed in: scan progress, or previews that are ready.
    Assets {
        #[serde(skip_serializing_if = "Option::is_none")]
        scan: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        previews: Option<serde_json::Value>,
        #[serde(skip_serializing_if = "Option::is_none")]
        ai: Option<serde_json::Value>,
    },
    /// Something the user can see changed (by another tab, a runner or the server):
    /// the web app reloads that list. `what` is one of tasks, projects, chats,
    /// machines, access, settings.
    #[serde(rename_all = "camelCase")]
    Changed {
        what: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        machine_id: Option<String>,
    },
}

/// User id that reaches every listener.
pub const ALL: &str = "*";

/// One channel for everyone; each listener only passes on its own user's events.
#[derive(Clone)]
pub struct Bus(broadcast::Sender<(String, Event)>);

impl Bus {
    pub fn new() -> Self {
        Self(broadcast::channel(1024).0)
    }
    pub fn send(&self, user_id: &str, e: Event) {
        let _ = self.0.send((user_id.to_string(), e)); // no listeners is fine
    }
    /// To every signed-in user (library-wide news such as the asset scan).
    pub fn send_all(&self, e: Event) {
        self.send(ALL, e);
    }
    pub fn subscribe(&self) -> broadcast::Receiver<(String, Event)> {
        self.0.subscribe()
    }
}
