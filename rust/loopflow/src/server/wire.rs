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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClientIdentity {
    pub kind: String,
    pub platform: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Operation {
    SessionOwner {
        session: crate::id::LfSessionId,
    },
    Read {
        query: Read,
    },
    /// Answers `subscribed`, then `reading` messages until cancelled.
    Subscribe {
        query: Read,
    },
    Configuration {
        request: crate::config::api::Request,
    },
    Command {
        change: Write,
    },
    /// Switch this connection to the existing owner protocol after `bridged`.
    Attach {
        owner: LfProcessId,
    },
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
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Welcome {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Incompatible,
    Invalid,
    Unavailable,
    OutcomeUnknown,
    Busy,
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
            r#"{"type":"request","id":"b","operation":{"kind":"bootstrap"}}"#
        )
        .is_err());
    }
}
