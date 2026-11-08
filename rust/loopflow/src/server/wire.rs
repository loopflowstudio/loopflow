//! Carrier-independent server protocol, revision 3: one connection per client,
//! negotiated revision, requests multiplexed by id. Local admission is
//! same-user only. Each carrier delimits messages its own way; the Unix socket
//! carries one JSON object per line.
use crate::engine::{Error, ErrorCode};
use crate::id::LfProcessId;
use crate::lf::commands::{read::Read, subscribe::SubscriptionFrame, write::Write};
use crate::server::schema::Untyped;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The newest revision this build speaks.
pub const REVISION: u32 = 3;
/// The oldest revision this build speaks.
pub const MIN_REVISION: u32 = 3;
pub const MAX_MESSAGE: usize = 32 * 1024 * 1024;
/// Welcomed client connections; terminal bridges are not among them.
pub const MAX_CONNECTIONS: usize = 64;
/// How long a `session.attach` ticket opens a bridge.
pub const TICKET_LIFETIME: std::time::Duration = std::time::Duration::from_secs(60);

/// One message from the client.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientMessage {
    /// The connection's first message; the server answers `welcome`.
    Hello(Hello),
    /// A pre-hello readiness probe; the server answers `status` and closes.
    Status,
    /// A bridge's first and only message: the ticket `session.attach` issued.
    /// The server answers `bridged`, and the connection then speaks the owner
    /// protocol. A ticket opens one bridge, within [`TICKET_LIFETIME`], for the
    /// user who requested it; otherwise the answer is `error` and the connection closes.
    Attach { ticket: String },
    /// `id` is 1–128 bytes and unique among this connection's unanswered requests.
    /// `select` keeps only those dotted paths of a read's answer (`task.title`;
    /// a path through an array applies to each element, a missing path is
    /// absent); null keeps it whole, and a write refuses one.
    Request {
        id: String,
        operation: Operation,
        select: Option<Vec<String>>,
    },
    /// A read with a subscription; answers a null `response`, then `reading` messages
    /// until cancelled. The id follows the same rules as a request's. `select`
    /// is as for a request; a change outside it sends no reading.
    Subscribe {
        id: String,
        operation: Read,
        select: Option<Vec<String>>,
    },
    /// Ask the live subscription `id` for a reading now; the next reading's
    /// `answers` carries `checkpoint`, changed or not.
    Checkpoint { id: String, checkpoint: u64 },
    /// Forget a request's answer, or end a subscription.
    Cancel { id: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Hello {
    pub revision: u32,
    pub min_revision: u32,
    pub client: ClientIdentity,
}

/// Who connected, for diagnostics; it grants nothing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClientIdentity {
    pub kind: String,
    pub platform: String,
    pub name: String,
    pub version: String,
}

/// What a request asks for, named by its `kind`: a dotted resource and verb
/// (`task.complete`, `plan.list`). `desktop.*` kinds are Desktop's own
/// arrangement state, not part of what other clients are offered.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Operation {
    Server(ServerOperation),
    Read(Read),
    Write(Write),
}

/// Operations the server owns rather than a shared read or write.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum ServerOperation {
    /// A ticket for the Session's terminal: the server resolves the Session's
    /// current owner LfProcess and answers `{ticket, owner_lf_process_id,
    /// expires_in_ms}`. A new connection presents the ticket as its first
    /// message (`attach`) and becomes the bridge; this connection stays a
    /// client. When the owner ends, the bridge closes; attaching again by the
    /// same Session reaches its next owner. With `owner_lf_process_id`, the
    /// Session must still be owned by exactly that LfProcess, for a client that
    /// reconnects only to the owner it had.
    #[serde(rename = "session.attach")]
    #[schemars(extend("x-result" = generator.subschema_for::<SessionAttached>()))]
    SessionAttach {
        session: crate::id::LfSessionId,
        owner_lf_process_id: Option<LfProcessId>,
    },
}

/// What `session.attach` answers: a ticket that opens one bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionAttached {
    /// 16 random bytes as 22 base64url characters; single use.
    pub ticket: String,
    pub owner_lf_process_id: LfProcessId,
    /// How long the ticket opens a bridge, from when it was issued.
    pub expires_in_ms: u64,
}

/// One message from the server.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum ServerMessage {
    Welcome(Welcome),
    Status(Status),
    /// What the request returned: the value its operation's `x-result` names,
    /// null once a subscription is confirmed. `trace_id` correlates the server's
    /// diagnostics for it.
    Response {
        id: String,
        trace_id: crate::id::TraceId,
        result: serde_json::Value,
    },
    /// The answer to `attach`: the connection now speaks the owner protocol
    /// with this LfProcess, and nothing more arrives from the server.
    Bridged {
        owner_lf_process_id: LfProcessId,
    },
    /// One subscription line; its `body` has the shape of the read's `x-result`.
    Reading {
        id: String,
        #[schemars(with = "SubscriptionFrame<'static, Untyped>")]
        frame: serde_json::Value,
    },
    /// The subscription's last reading was its answer (`task.wait` reached
    /// its state); nothing follows for `id`.
    Ended {
        id: String,
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
    #[schemars(with = "Option<crate::durable::Machine>")]
    pub machine: Option<serde_json::Value>,
    pub limits: Limits,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConnectedClient {
    pub id: String,
    pub client: ClientIdentity,
    /// RFC 3339.
    pub connected_at: String,
    /// Requests the connection has sent, subscriptions included.
    pub requests: u64,
    pub open: usize,
    pub retained_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub max_message: usize,
    pub max_connections: usize,
    pub max_open: usize,
    pub max_retained_bytes: usize,
}

impl ServerMessage {
    /// The `error` message that answers request `id` (null: the connection) with `error`.
    pub fn error(id: Option<String>, error: Error) -> Self {
        Self::Error {
            id,
            code: error.code,
            message: error.message,
            data: error.data,
        }
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
            include_str!("../../../../tests/fixtures/dto/server/attach_ticket.json"),
            include_str!("../../../../tests/fixtures/dto/server/view_plan.json"),
            include_str!("../../../../tests/fixtures/dto/server/configuration.json"),
            include_str!("../../../../tests/fixtures/dto/server/task_from_session.json"),
            include_str!("../../../../tests/fixtures/dto/server/task_run.json"),
            include_str!("../../../../tests/fixtures/dto/server/task_ship.json"),
            include_str!("../../../../tests/fixtures/dto/server/task_hold.json"),
            include_str!("../../../../tests/fixtures/dto/server/task_steer.json"),
            include_str!("../../../../tests/fixtures/dto/server/task_report.json"),
            include_str!("../../../../tests/fixtures/dto/server/task_interrupt.json"),
            include_str!("../../../../tests/fixtures/dto/server/task_wait.json"),
            include_str!("../../../../tests/fixtures/dto/server/session_steer.json"),
            include_str!("../../../../tests/fixtures/dto/server/wave_plan_bind.json"),
            include_str!("../../../../tests/fixtures/dto/server/wave_plan_connect.json"),
            include_str!("../../../../tests/fixtures/dto/server/wave_plan_reteam.json"),
            include_str!("../../../../tests/fixtures/dto/server/repo_ci.json"),
            include_str!("../../../../tests/fixtures/dto/server/repo_release.json"),
            include_str!("../../../../tests/fixtures/dto/server/checkout_commit.json"),
            include_str!("../../../../tests/fixtures/dto/server/checkout_sync.json"),
            include_str!("../../../../tests/fixtures/dto/server/checkout_create.json"),
            include_str!("../../../../tests/fixtures/dto/server/account_list.json"),
            include_str!("../../../../tests/fixtures/dto/server/account_route.json"),
            include_str!("../../../../tests/fixtures/dto/server/account_disconnect.json"),
            include_str!("../../../../tests/fixtures/dto/server/account_connect.json"),
            include_str!("../../../../tests/fixtures/dto/server/account_set.json"),
            include_str!("../../../../tests/fixtures/dto/server/account_token.json"),
            include_str!("../../../../tests/fixtures/dto/server/account_redeem_reset.json"),
            include_str!("../../../../tests/fixtures/dto/server/account_use.json"),
            include_str!("../../../../tests/fixtures/dto/server/graph_run.json"),
            include_str!("../../../../tests/fixtures/dto/server/skill_run.json"),
            include_str!("../../../../tests/fixtures/dto/server/skill_list.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_publish.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_open.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_submit.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_arm.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_land.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_abandon.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_reconcile.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_request_review.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_checks.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_reviews.json"),
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
            include_str!("../../../../tests/fixtures/dto/server/attached.json"),
            include_str!("../../../../tests/fixtures/dto/server/bridged.json"),
            include_str!("../../../../tests/fixtures/dto/server/reading.json"),
            include_str!("../../../../tests/fixtures/dto/server/error.json"),
            include_str!("../../../../tests/fixtures/dto/server/not_found.json"),
            include_str!("../../../../tests/fixtures/dto/server/task_running.json"),
            include_str!("../../../../tests/fixtures/dto/server/task_shipping.json"),
            include_str!("../../../../tests/fixtures/dto/server/task_held.json"),
            include_str!("../../../../tests/fixtures/dto/server/session_steered.json"),
            include_str!("../../../../tests/fixtures/dto/server/wave_plan_connected.json"),
            include_str!("../../../../tests/fixtures/dto/server/wave_plan_reteamed.json"),
            include_str!("../../../../tests/fixtures/dto/server/repo_ci_report.json"),
            include_str!("../../../../tests/fixtures/dto/server/launched.json"),
            include_str!("../../../../tests/fixtures/dto/server/checkout_committed.json"),
            include_str!("../../../../tests/fixtures/dto/server/checkout_sync_planned.json"),
            include_str!("../../../../tests/fixtures/dto/server/checkout_syncing.json"),
            include_str!("../../../../tests/fixtures/dto/server/checkout_created.json"),
            include_str!("../../../../tests/fixtures/dto/server/account_report.json"),
            include_str!("../../../../tests/fixtures/dto/server/account_routes.json"),
            include_str!("../../../../tests/fixtures/dto/server/account.json"),
            include_str!("../../../../tests/fixtures/dto/server/account_redeemed.json"),
            include_str!("../../../../tests/fixtures/dto/server/skill_entries.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_published.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_prepared.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_abandoned.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_delivery_check.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_review_requested.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_checks_report.json"),
            include_str!("../../../../tests/fixtures/dto/server/pr_reviews_report.json"),
            include_str!("../../../../tests/fixtures/dto/server/ended.json"),
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
