//! Carrier-independent server protocol, revision 8: one connection per client,
//! negotiated revision, requests multiplexed by id. Local admission is
//! same-user only; the remote carrier admits the Machine owner's other
//! Tailscale devices. Each carrier delimits messages its own way: the Unix
//! socket carries one JSON object per line, the WebSocket one per text frame.
use super::schema::Untyped;
use crate::engine::{Error, ErrorCode};
use crate::read_stream::SubscriptionFrame;
pub use loopflow_storage::sqlite::Revisions;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The newest revision this build speaks.
pub const REVISION: u32 = 8;
/// The oldest revision this build speaks. Revision 8 requires `sender` on
/// `request` and `subscribe`; it keeps 7's write revisions and request IDs.
pub const MIN_REVISION: u32 = 8;
/// The largest message a control connection carries, so a reading never waits
/// behind a large body. Larger messages travel on a ticketed connection.
pub const MAX_MESSAGE: usize = 1024 * 1024;
/// The largest message a ticketed connection carries in either direction.
pub const MAX_BULK: usize = 32 * 1024 * 1024;
/// Welcomed client connections; terminal bridges are not among them.
pub const MAX_CONNECTIONS: usize = 64;
/// How long a `session.attach` ticket opens a bridge.
pub const TICKET_LIFETIME: std::time::Duration = std::time::Duration::from_secs(60);

/// One message from the client.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
#[schemars(rename = "ClientMessage")]
pub enum ClientMessage<O = serde_json::Value, R = serde_json::Value> {
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
        operation: O,
        select: Option<Vec<String>>,
        /// What the Machine that sent the request runs with; null from a
        /// client on this Machine.
        #[serde(deserialize_with = "Option::deserialize")]
        sender: Option<Sender>,
        /// Names one write across connections and restarts: for a day, a
        /// repeat under the same ID gets the first answer and runs nothing; a
        /// different operation under it is `invalid`. Null for reads and for
        /// writes the client never repeats.
        #[serde(deserialize_with = "Option::deserialize")]
        request_id: Option<String>,
    },
    /// A read with a subscription; answers a null `response`, then `reading` messages
    /// until cancelled. The id follows the same rules as a request's. `select`
    /// is as for a request; a change outside it sends no reading.
    Subscribe {
        id: String,
        operation: R,
        select: Option<Vec<String>>,
        /// The last applied reading's opaque checkpoint; null starts fresh.
        resume: Option<String>,
        /// As for a request.
        #[serde(deserialize_with = "Option::deserialize")]
        sender: Option<Sender>,
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

/// Who connected; it grants nothing. A `kind` of [`MACHINE_CLIENT`] is
/// another Machine: every operation it sends runs here and is never sent on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClientIdentity {
    pub kind: String,
    pub platform: String,
    pub name: String,
    pub version: String,
}

/// The [`ClientIdentity::kind`] of another Machine's `lf`.
pub const MACHINE_CLIENT: &str = "machine";

/// What the Machine that sent a request runs with. An operation names the
/// sender's own directory; it runs in this Machine's checkout of `repository`.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Sender {
    pub repository: loopflow_model::durable::RepositoryId,
    pub selection: Selection,
    pub secrets: Secrets,
}

/// Who asks, and how: what a launch the request starts runs with.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    /// The sender's personal settings and definitions.
    #[schemars(with = "Untyped")]
    pub profile: crate::config::remote::Profile,
    /// The name its person's Task claims carry.
    #[serde(deserialize_with = "Option::deserialize")]
    pub participant: Option<String>,
    /// `--agent` as the sender selected it.
    #[serde(deserialize_with = "Option::deserialize")]
    pub agent: Option<String>,
    /// Command-line settings (`LF_SETTINGS`), as YAML.
    #[serde(deserialize_with = "Option::deserialize")]
    pub settings: Option<String>,
    /// A shipped Task's branch and Linear mapping, which resolve here.
    #[serde(deserialize_with = "Option::deserialize")]
    #[schemars(with = "Option<Untyped>")]
    pub source: Option<crate::ops::task::remote::TaskSource>,
}

/// The values `forward.env` names, by name, for the launch a request starts.
/// Nothing prints them: `Debug` shows names, and only the wire carries values.
#[derive(Clone, Default)]
pub struct Secrets(pub Vec<(String, secrecy::SecretString)>);

impl std::fmt::Debug for Secrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Secrets")
            .field(&self.0.iter().map(|(name, _)| name).collect::<Vec<_>>())
            .finish()
    }
}

impl Serialize for Secrets {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use secrecy::ExposeSecret;
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (name, value) in &self.0 {
            map.serialize_entry(name, value.expose_secret())?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for Secrets {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let values = std::collections::BTreeMap::<String, String>::deserialize(deserializer)?;
        Ok(Self(
            values
                .into_iter()
                .map(|(name, value)| (name, secrecy::SecretString::new(value)))
                .collect(),
        ))
    }
}

impl JsonSchema for Secrets {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Secrets".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "Values by environment variable name; never logged.",
            "type": "object",
            "additionalProperties": { "type": "string" },
        })
    }
}

/// Operations the server owns rather than a shared read or write.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", deny_unknown_fields)]
pub enum ServerOperation {
    /// A ticket for the Session's held terminal, answered `{ticket,
    /// terminal_id, expires_in_ms}`. A new connection presents the ticket as
    /// its first message (`attach`) and becomes the bridge; this connection
    /// stays a client. An orderly terminal sends a final `0x07` frame with
    /// JSON `{reason: replaced|finished|retired}` (see `x-terminal-close`).
    /// After `replaced`, attaching by the same Session reaches its next terminal.
    /// Bare EOF leaves the reason unknown. A Session no terminal
    /// runs is `not_found`; attaching never starts a provider.
    #[serde(rename = "session.attach")]
    #[schemars(extend("x-result" = generator.subschema_for::<SessionAttached>()))]
    SessionAttach {
        session: loopflow_model::id::LfSessionId,
        /// Attach only to this held terminal.
        terminal_id: Option<loopflow_model::id::TerminalId>,
    },
    /// Start the Session's agent in this build's holder and answer
    /// `{terminal_id}`. `env` is the caller's whole environment; the server's
    /// is not used. Repeating `request` answers the terminal it started.
    /// This is the only operation here that creates a process.
    #[serde(rename = "session.start")]
    #[schemars(extend("x-result" = generator.subschema_for::<SessionStarted>()))]
    SessionStart {
        session: Option<loopflow_model::id::LfSessionId>,
        request: String,
        argv: Vec<String>,
        cwd: std::path::PathBuf,
        env: Vec<(String, String)>,
        rows: u16,
        cols: u16,
        /// The Desktop pane (`LF_TERMINAL_ID`) the caller's own terminal was
        /// validated as; the held child continues it on its PTY. Null outside
        /// a pane.
        #[serde(deserialize_with = "Option::deserialize")]
        placement: Option<String>,
    },
    /// Continue an existing Session in a held terminal: `lf session resume`
    /// of exactly `session`, in its directory, with the server's environment.
    /// Answers `{terminal_id}`; repeating `request` answers the terminal it
    /// started. For a client with no environment of its own to send.
    #[serde(rename = "session.continue")]
    #[schemars(extend("x-result" = generator.subschema_for::<SessionStarted>()))]
    SessionContinue {
        session: loopflow_model::id::LfSessionId,
        request: String,
        rows: u16,
        cols: u16,
    },
    /// Mobile access readiness; same-user local socket only.
    #[serde(rename = "mobile.status")]
    #[schemars(extend("x-result" = generator.subschema_for::<super::mobile::Status>()))]
    MobileStatus,
    /// Turn mobile access on with `configuration`, or off with null; same-user
    /// local socket only. Answers `{configuration}`.
    #[serde(rename = "mobile.configure")]
    #[schemars(extend("x-result" = schemars::json_schema!({"type": "object", "properties": {"configuration": generator.subschema_for::<Option<super::mobile::Configuration>>()}, "required": ["configuration"], "additionalProperties": false}))) ]
    MobileConfigure {
        configuration: Option<super::mobile::Configuration>,
    },
    /// A ticket for a connection that carries one request of `bytes` bytes,
    /// larger than `limits.max_message`; answers `{ticket}`. The uploaded
    /// request is answered here under its own id.
    #[serde(rename = "request.upload")]
    #[schemars(extend("x-result" = schemars::json_schema!({"type": "object", "properties": {"ticket": {"type": "string"}}, "required": ["ticket"], "additionalProperties": false}))) ]
    RequestUpload { bytes: usize },
    /// A provider hook at a turn or tool boundary. The server records what the
    /// boundary says (the native turn, the acknowledged input, `gap`) and
    /// answers the Session's pending steers. At `stop` it answers first, then
    /// refreshes planning and applies a pending Session move.
    #[serde(rename = "session.boundary")]
    #[schemars(extend("x-result" = generator.subschema_for::<SessionBoundaryAnswer>()))]
    SessionBoundary(SessionBoundary),
}

/// What a hook says at a turn or tool boundary.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionBoundary {
    pub moment: BoundaryMoment,
    /// The Session's context delivery file.
    pub delivery: std::path::PathBuf,
    /// The provider context the launch gave the agent; null for a
    /// conversation lf did not start (a Codex app-server thread), whose
    /// `stop` only refreshes planning.
    #[serde(deserialize_with = "Option::deserialize")]
    pub adapter: Option<crate::steer::SteerAdapter>,
    /// The JSON the provider sent the hook, unchanged.
    #[schemars(with = "Untyped")]
    pub callback: serde_json::Value,
    /// The hook's open recording gap; the server writes it into the
    /// Session's history as one `store-gap` observation.
    #[serde(deserialize_with = "Option::deserialize")]
    pub gap: Option<BoundaryGap>,
    /// Unused: hooks claim their own steers from the store. Conversations
    /// started before then still send it. TODO(LOO-838): delete it with
    /// `ContextMoment::TaskUpdates`, once none of them is live.
    pub printed: Vec<String>,
    /// Where an idle conversation takes steers and Task updates: Claude's
    /// inbox key, or an OpenCode or Pi work session's socket. Null from
    /// Codex, a worker and a Claude with no inbox.
    #[serde(deserialize_with = "Option::deserialize")]
    pub inbox: Option<crate::steer::Inbox>,
}

/// Where in a native conversation a hook fired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryMoment {
    Start,
    Compact,
    Tool,
    Stop,
    /// A Pi event the extension reports (`callback` is the event): the server
    /// records the turn from it and, while a turn runs, answers steers.
    Event,
}

/// Boundaries the hook could not record, in Unix seconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BoundaryGap {
    pub first: i64,
    pub last: i64,
}

/// What `session.boundary` answers: nothing. Hooks claim their own steers and
/// Task update from the store. The three fields are what conversations
/// started before then still parse. TODO(LOO-838): delete them with
/// `ContextMoment::TaskUpdates`, once none of those conversations is live.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionBoundaryAnswer {
    #[serde(deserialize_with = "Option::deserialize")]
    #[schemars(with = "Untyped")]
    pub steers: Option<()>,
    #[serde(deserialize_with = "Option::deserialize")]
    #[schemars(with = "Untyped")]
    pub update: Option<()>,
    pub waiting: bool,
}

/// What `session.start` answers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionStarted {
    pub terminal_id: loopflow_model::id::TerminalId,
}

/// What `session.attach` answers: a ticket that opens one bridge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionAttached {
    /// Opaque random ticket; single use.
    pub ticket: String,
    /// The held terminal the bridge reaches.
    pub terminal_id: loopflow_model::id::TerminalId,
    /// How long the ticket opens a bridge, from when it was issued.
    pub expires_in_ms: u64,
}

pub use crate::read_stream::events::SubscriptionEvent;

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
        trace_id: loopflow_model::id::TraceId,
        result: serde_json::Value,
        /// For a write, each store domain it advanced, at its value after the
        /// commit: a reading at or past every one shows the write. Null for
        /// reads and server operations.
        #[serde(deserialize_with = "Option::deserialize")]
        revisions: Option<Revisions>,
    },
    /// The answer to `attach`: the connection now speaks the viewer protocol
    /// with this held terminal, and nothing more arrives from the server.
    Bridged {
        terminal_id: loopflow_model::id::TerminalId,
    },
    /// One subscription line; its `body` has the shape of the read's `x-result`.
    Reading {
        id: String,
        /// Resume this reading on another connection to the same server.
        checkpoint: String,
        #[schemars(with = "SubscriptionFrame<'static, Untyped>")]
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
    /// Connection-local ordering; never a replay or reading checkpoint.
    Event {
        id: String,
        seq: u64,
        event: SubscriptionEvent,
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
        /// The domains that advanced while a failed write ran, by it or by
        /// another write; null when none did.
        #[serde(deserialize_with = "Option::deserialize")]
        revisions: Option<Revisions>,
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
    #[schemars(with = "Option<loopflow_model::durable::Machine>")]
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
        Self::written_error(id, error, None)
    }

    /// The `error` that answers a write during which `revisions` advanced.
    pub fn written_error(id: Option<String>, error: Error, revisions: Option<Revisions>) -> Self {
        Self::Error {
            id,
            code: error.code,
            message: error.message,
            data: error.data,
            revisions,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ServerMessage;
    #[test]
    fn unknown_events_keep_their_subscription_envelope() {
        let message: ServerMessage = serde_json::from_str(r#"{"type":"event","id":"task","seq":2,"event":{"kind":"future.event","payload":true}}"#).unwrap();
        assert!(
            matches!(message, ServerMessage::Event { id, seq:2, event: super::SubscriptionEvent::Unknown } if id == "task")
        );
    }

    #[test]
    fn shared_server_fixtures_decode_and_round_trip_without_defaults() {
        for raw in [
            include_str!("../../tests/fixtures/dto/server/welcome.json"),
            include_str!("../../tests/fixtures/dto/server/status.json"),
            include_str!("../../tests/fixtures/dto/server/response.json"),
            include_str!("../../tests/fixtures/dto/server/attached.json"),
            include_str!("../../tests/fixtures/dto/server/bridged.json"),
            include_str!("../../tests/fixtures/dto/server/session_started.json"),
            include_str!("../../tests/fixtures/dto/server/reading.json"),
            include_str!("../../tests/fixtures/dto/server/event_shipped.json"),
            include_str!("../../tests/fixtures/dto/server/event_steer.json"),
            include_str!("../../tests/fixtures/dto/server/event_step_finished.json"),
            include_str!("../../tests/fixtures/dto/server/error.json"),
            include_str!("../../tests/fixtures/dto/server/error_written.json"),
            include_str!("../../tests/fixtures/dto/server/not_found.json"),
            include_str!("../../tests/fixtures/dto/server/remote.json"),
            include_str!("../../tests/fixtures/dto/server/mobile_status.json"),
            include_str!("../../tests/fixtures/dto/server/ticket_reply.json"),
            include_str!("../../tests/fixtures/dto/server/bulk.json"),
            include_str!("../../tests/fixtures/dto/server/admitted.json"),
            include_str!("../../tests/fixtures/dto/server/task_running.json"),
            include_str!("../../tests/fixtures/dto/server/task_shipping.json"),
            include_str!("../../tests/fixtures/dto/server/task_held.json"),
            include_str!("../../tests/fixtures/dto/server/session_steered.json"),
            include_str!("../../tests/fixtures/dto/server/session_boundary_answer.json"),
            include_str!("../../tests/fixtures/dto/server/wave_plan_connected.json"),
            include_str!("../../tests/fixtures/dto/server/wave_plan_reteamed.json"),
            include_str!("../../tests/fixtures/dto/server/repo_ci_report.json"),
            include_str!("../../tests/fixtures/dto/server/launched.json"),
            include_str!("../../tests/fixtures/dto/server/checkout_committed.json"),
            include_str!("../../tests/fixtures/dto/server/checkout_sync_planned.json"),
            include_str!("../../tests/fixtures/dto/server/checkout_syncing.json"),
            include_str!("../../tests/fixtures/dto/server/checkout_created.json"),
            include_str!("../../tests/fixtures/dto/server/account_report.json"),
            include_str!("../../tests/fixtures/dto/server/account.json"),
            include_str!("../../tests/fixtures/dto/server/skill_entries.json"),
            include_str!("../../tests/fixtures/dto/server/pr_published.json"),
            include_str!("../../tests/fixtures/dto/server/pr_prepared.json"),
            include_str!("../../tests/fixtures/dto/server/pr_abandoned.json"),
            include_str!("../../tests/fixtures/dto/server/pr_delivery_check.json"),
            include_str!("../../tests/fixtures/dto/server/pr_review_requested.json"),
            include_str!("../../tests/fixtures/dto/server/pr_checks_report.json"),
            include_str!("../../tests/fixtures/dto/server/pr_reviews_report.json"),
            include_str!("../../tests/fixtures/dto/server/ended.json"),
        ] {
            let message: ServerMessage = serde_json::from_str(raw).unwrap();
            assert_eq!(
                serde_json::to_value(message).unwrap(),
                serde_json::from_str::<serde_json::Value>(raw).unwrap()
            );
        }
        assert!(
            serde_json::from_str::<ServerMessage>(r#"{"type":"reading","id":"tasks-1"}"#).is_err()
        );
    }
}
