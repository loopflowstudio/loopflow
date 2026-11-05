//! Carrier-independent server protocol, revision 2: one connection per client,
//! negotiated revision, requests multiplexed by id. Local admission is
//! same-user only. Each carrier delimits messages its own way; the Unix socket
//! carries one JSON object per line.
use crate::id::LfProcessId;
use crate::lf::commands::{read::Read, write::Write};
use serde::{Deserialize, Serialize};

/// The newest revision this build speaks.
pub const REVISION: u32 = 2;
/// The oldest revision this build speaks.
pub const MIN_REVISION: u32 = 2;
pub const MAX_MESSAGE: usize = 32 * 1024 * 1024;
pub const MAX_CONNECTIONS: usize = 64;

/// One message from the client.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientMessage {
    /// The connection's first message; the server answers `welcome`.
    Hello(Hello),
    /// A pre-hello readiness probe; the server answers `status` and closes.
    Status,
    /// `id` is 1–128 bytes and unique among this connection's unanswered requests.
    Request { id: String, operation: Operation },
    /// A read with a watch; answers `subscribed`, then `reading` messages
    /// until cancelled. The id follows the same rules as a request's.
    Subscribe { id: String, operation: Read },
    /// Ask the live subscription `id` for a reading now; the next reading's
    /// `answers` carries `checkpoint`, changed or not.
    Checkpoint { id: String, checkpoint: u64 },
    /// Forget a request's answer, or end a subscription.
    Cancel { id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub revision: u32,
    pub min_revision: u32,
    pub client: ClientIdentity,
}

/// Who connected, for diagnostics; it grants nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientIdentity {
    pub kind: String,
    pub platform: String,
    pub name: String,
    pub version: String,
}

/// What a request asks for, named by its `kind`: a dotted resource and verb
/// (`task.complete`, `plan.get`). `desktop.*` kinds are Desktop's own
/// arrangement state, not part of what other clients are offered.
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Operation {
    Server(ServerOperation),
    Read(Read),
    Write(Write),
}

/// Operations the server owns rather than a shared read or write.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum ServerOperation {
    /// The owner Process of a Session's terminal, if one is recorded.
    #[serde(rename = "session.owner")]
    SessionOwner { session: crate::id::LfSessionId },
    /// Settings files: catalogue, declaration, explanation and theme.
    #[serde(rename = "config.file")]
    Configuration {
        request: crate::config::api::Request,
    },
    /// Switch this connection to the existing owner protocol after `bridged`.
    #[serde(rename = "terminal.attach")]
    Attach { owner: LfProcessId },
}

/// One message from the server.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServerMessage {
    Welcome(Welcome),
    Status(Status),
    Response {
        id: String,
        result: Reply,
    },
    Reading {
        id: String,
        frame: serde_json::Value,
    },
    /// `id` is null before a request exists. An error for a subscription ends it.
    Error {
        id: Option<String>,
        code: ErrorCode,
        message: String,
        /// Structured detail a client can act on, such as the missing resource.
        data: Option<serde_json::Value>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Welcome {
    /// The server's name for this connection, as `lf server status` lists it.
    pub client_id: String,
    /// The revision both sides speak for this connection.
    pub revision: u32,
    pub server_revision: u32,
    pub min_revision: u32,
    /// Changes with every server start.
    pub instance: String,
    pub build: String,
    pub executable_sha256: String,
    /// Null while the Machine registry is unavailable or uninitialized.
    pub machine: Option<serde_json::Value>,
    pub capabilities: Vec<String>,
    pub limits: Limits,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Status {
    pub revision: u32,
    pub min_revision: u32,
    pub instance: String,
    pub executable_sha256: String,
    /// Every welcomed connection still open.
    pub clients: Vec<ConnectedClient>,
}

/// One welcomed connection, from `welcome` until it closes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ConnectedClient {
    pub id: String,
    pub client: ClientIdentity,
    /// RFC 3339.
    pub connected_at: String,
    /// Requests the connection has sent, subscriptions included.
    pub requests: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_message: usize,
    pub max_connections: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Reply {
    Read {
        value: serde_json::Value,
    },
    Subscribed,
    Command {
        trace_id: crate::id::TraceId,
        code: i32,
        /// Stdout parsed as JSON when the command's spelling prints JSON.
        value: Option<serde_json::Value>,
        output: String,
        stderr: String,
    },
    SessionOwner {
        owner: Option<LfProcessId>,
    },
    Bridged,
}

/// What a client can branch on. Nothing ran for any code but `outcome_unknown`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    /// No protocol revision in common; `data` names the side to update.
    Incompatible,
    /// The message or request is malformed or its arguments are refused.
    Invalid,
    /// The named Task, Session, owner or subscription does not exist; `data` names it.
    NotFound,
    /// The request contradicts current state: a reused id or a stale revision.
    Conflict,
    /// A dependency is missing or failed: the Machine registry, a watch, a provider.
    Unavailable,
    /// The operation may have taken effect; inspect state before retrying.
    OutcomeUnknown,
    /// The server is at capacity or the client fell behind; retry shortly.
    Busy,
}

/// A refused request, as an `error` message carries it. Shared operations
/// return it inside `anyhow::Error` to name their code; the server sends it as is.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{code}: {message}")]
pub struct ServerError {
    pub code: ErrorCode,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

impl ServerError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: None,
        }
    }

    pub fn with(mut self, data: serde_json::Value) -> Self {
        self.data = Some(data);
        self
    }

    pub fn into_message(self, id: Option<String>) -> ServerMessage {
        ServerMessage::Error {
            id,
            code: self.code,
            message: self.message,
            data: self.data,
        }
    }
}

/// The wire name, so a message reads `outcome_unknown: …` on every client.
impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = serde_json::to_value(self).expect("an error code serializes as a string");
        f.write_str(name.as_str().expect("an error code serializes as a string"))
    }
}

#[cfg(test)]
mod tests {
    use super::{ClientMessage, ServerMessage};

    #[test]
    fn shared_client_fixtures_decode_and_round_trip_without_defaults() {
        for raw in [
            include_str!("../../../../tests/fixtures/dto/server/hello.json"),
            include_str!("../../../../tests/fixtures/dto/server/status_request.json"),
            include_str!("../../../../tests/fixtures/dto/server/subscribe.json"),
            include_str!("../../../../tests/fixtures/dto/server/checkpoint.json"),
            include_str!("../../../../tests/fixtures/dto/server/cancel.json"),
            include_str!("../../../../tests/fixtures/dto/server/attach.json"),
            include_str!("../../../../tests/fixtures/dto/server/session_owner.json"),
            include_str!("../../../../tests/fixtures/dto/server/view_plan.json"),
            include_str!("../../../../tests/fixtures/dto/server/configuration.json"),
            include_str!("../../../../tests/fixtures/dto/server/task_from_session.json"),
        ] {
            let message: ClientMessage = serde_json::from_str(raw).unwrap();
            assert_eq!(
                serde_json::to_value(&message).unwrap(),
                serde_json::from_str::<serde_json::Value>(raw).unwrap()
            );
        }
        for raw in [
            include_str!("../../../../tests/fixtures/dto/server/welcome.json"),
            include_str!("../../../../tests/fixtures/dto/server/status.json"),
            include_str!("../../../../tests/fixtures/dto/server/command.json"),
            include_str!("../../../../tests/fixtures/dto/server/owner.json"),
            include_str!("../../../../tests/fixtures/dto/server/reading.json"),
            include_str!("../../../../tests/fixtures/dto/server/error.json"),
            include_str!("../../../../tests/fixtures/dto/server/not_found.json"),
        ] {
            let message: ServerMessage = serde_json::from_str(raw).unwrap();
            assert_eq!(
                serde_json::to_value(message).unwrap(),
                serde_json::from_str::<serde_json::Value>(raw).unwrap()
            );
        }
        // A missing field is an error, never a default.
        assert!(serde_json::from_str::<ClientMessage>(
            r#"{"type":"hello","revision":2,"min_revision":2}"#
        )
        .is_err());
        assert!(
            serde_json::from_str::<ServerMessage>(r#"{"type":"reading","id":"tasks-1"}"#).is_err()
        );
        assert!(serde_json::from_str::<ClientMessage>(
            r#"{"type":"request","id":"b","operation":{"kind":"task.complete","issue":"A"}}"#
        )
        .is_err());
    }
}
