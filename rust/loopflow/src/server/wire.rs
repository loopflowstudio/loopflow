//! Carrier-independent server protocol, revision 2: one connection per client,
//! negotiated revision, requests multiplexed by id. Local admission is
//! same-user only; the remote carrier admits the Machine owner's other
//! Tailscale devices. Each carrier delimits messages its own way: the Unix
//! socket carries one JSON object per line, the WebSocket one per text frame.
use crate::id::LfProcessId;
use crate::lf::commands::{read::Read, write::Write};
use serde::{Deserialize, Serialize};

/// The newest revision this build speaks.
pub const REVISION: u32 = 2;
/// The oldest revision this build speaks.
pub const MIN_REVISION: u32 = 2;
/// The largest message a control connection carries, so a reading never waits
/// behind a large body. Larger messages travel on a ticketed connection.
pub const MAX_MESSAGE: usize = 1024 * 1024;
/// The largest message a ticketed connection carries in either direction.
pub const MAX_BULK: usize = 64 * 1024 * 1024;
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
    /// A ticketed connection's first and only protocol message. The server
    /// answers `admitted`, then the connection carries what the ticket names.
    Ticket { ticket: String },
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
    /// Mobile access readiness; same-user local socket only.
    #[serde(rename = "mobile.status")]
    MobileStatus,
    /// Turn mobile access on with `configuration`, or off with null; same-user
    /// local socket only. Answers `{configuration}`.
    #[serde(rename = "mobile.configure")]
    ConfigureRemote {
        configuration: Option<super::mobile::Configuration>,
    },
    /// A ticket for a terminal connection to this owner's existing protocol;
    /// answers `{ticket}`.
    #[serde(rename = "terminal.attach")]
    Attach { owner: LfProcessId },
    /// A ticket for a connection that carries one request of `bytes` bytes,
    /// larger than `limits.max_message`; answers `{ticket}`. The uploaded
    /// request is answered here under its own id.
    #[serde(rename = "request.upload")]
    Upload { bytes: usize },
}

/// One message from the server.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServerMessage {
    Welcome(Welcome),
    Status(Status),
    /// What the request returned: a read's or write's value, `{owner}` for
    /// `session.owner`, null once a subscription or attachment is confirmed.
    /// `trace_id` correlates the server's diagnostics for it.
    Response {
        id: String,
        trace_id: crate::id::TraceId,
        result: serde_json::Value,
    },
    Reading {
        id: String,
        frame: serde_json::Value,
    },
    /// The answer for `id` is `bytes` long, more than `limits.max_message`:
    /// a ticketed connection delivers it as one message.
    Bulk {
        id: String,
        ticket: String,
        bytes: usize,
    },
    /// A ticketed connection's answer; what follows is what the ticket names.
    Admitted,
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
/// return it inside `anyhow::Error` to name their code; the server sends it as
/// is, and the CLI prints its message.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{message}")]
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
            include_str!("../../../../tests/fixtures/dto/server/task_start.json"),
            include_str!("../../../../tests/fixtures/dto/server/session_start.json"),
            include_str!("../../../../tests/fixtures/dto/server/session_page.json"),
            include_str!("../../../../tests/fixtures/dto/server/view_select.json"),
            include_str!("../../../../tests/fixtures/dto/server/configure_remote.json"),
            include_str!("../../../../tests/fixtures/dto/server/mobile_status_request.json"),
            include_str!("../../../../tests/fixtures/dto/server/upload.json"),
            include_str!("../../../../tests/fixtures/dto/server/ticket.json"),
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
            include_str!("../../../../tests/fixtures/dto/server/response.json"),
            include_str!("../../../../tests/fixtures/dto/server/owner.json"),
            include_str!("../../../../tests/fixtures/dto/server/reading.json"),
            include_str!("../../../../tests/fixtures/dto/server/error.json"),
            include_str!("../../../../tests/fixtures/dto/server/not_found.json"),
            include_str!("../../../../tests/fixtures/dto/server/remote.json"),
            include_str!("../../../../tests/fixtures/dto/server/mobile_status.json"),
            include_str!("../../../../tests/fixtures/dto/server/ticket_reply.json"),
            include_str!("../../../../tests/fixtures/dto/server/bulk.json"),
            include_str!("../../../../tests/fixtures/dto/server/admitted.json"),
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
