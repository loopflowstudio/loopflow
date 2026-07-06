//! The resident wire: what crosses between the listener (the wave server,
//! vendor-free — hear / check / fold / tell) and the resident (the mind, its
//! own `lf` process owning the vendor harness).
//!
//! Two directions, two transports:
//!
//! - **Resident → listener**: ordered turn deltas over `POST /resident/deltas`
//!   (the resident sends serially and awaits each response, so per-turn order
//!   is the connection's order). The old in-process `TurnSink` vocabulary —
//!   Opened / Text / Item / Usage / Finished — IS this wire, promoted to full
//!   DTO discipline, plus the consumption markers (`TurnOpened.answers`,
//!   `TurnSteered.answers` — the RESIDENT decides what a turn answers; the
//!   listener validates against its queue fold and journals), the resident's
//!   reported mind state, and the vendor thread id. The single writer stays
//!   with the listener: the resident never touches journal files.
//! - **Listener → resident**: the resident consumes its own wave's `/events`
//!   subscription with `?inbox=true` — `inbox` SSE frames ([`InboxFrame`])
//!   carry queued messages / steer / interrupt ops (replayed pending queue on
//!   connect, then live).
//!
//! DTO discipline: every field is required or explicitly `Option` — no serde
//! defaults anywhere. The round-trip fixture lives at
//! `tests/fixtures/dto/resident_deltas.json`. Carve-out: unlike the other DTO
//! fixtures this wire is Rust↔Rust only (listener and resident are the same
//! binary); Swift and Python do not consume it, so only the Rust fixture test
//! pins it.
//!
//! # Auth
//! The resident door is gated by a per-boot bearer token
//! ([`RESIDENT_TOKEN_HEADER`]): the listener generates it at bind, passes it
//! to a spawned resident via [`RESIDENT_TOKEN_ENV`], and writes it to
//! `wave/<name>/.wave-resident-token` beside the endpoint pointer for
//! attached residents (`lf wave <name> --mind-only`) — the same
//! filesystem-trust domain as the discovery file. This is a stopgap: when a
//! human (or a remote mind) can hold the resident seat, the token becomes a
//! credential the gatekeeper issues, not a file the repo trusts.

use serde::{Deserialize, Serialize};

use crate::chat::types::{ConversationItem, Lifecycle};
use crate::wave::journal::{Attribution, MessageOp};

/// Header carrying the resident token on every `/resident/*` request.
pub const RESIDENT_TOKEN_HEADER: &str = "x-lf-resident-token";

/// Env var the listener sets on a spawned resident.
pub const RESIDENT_TOKEN_ENV: &str = "LF_WAVE_RESIDENT_TOKEN";

/// Basename of the token file beside `.wave-endpoint` (attached residents).
pub const RESIDENT_TOKEN_FILE: &str = ".wave-resident-token";

/// Header carrying a per-subagent capability token on the wave's `/v0/exec`
/// door. A different principal from the resident: `/exec` accepts a minted
/// subagent token and rejects the resident token, and vice versa for the
/// `/resident/*` routes.
pub const SUBAGENT_TOKEN_HEADER: &str = "x-lf-subagent-token";

/// Env var the listener sets on the resident (and thus every sandboxed
/// process it spawns) so a subagent can reach its wave's exec door via `lfq`.
pub const SUBAGENT_TOKEN_ENV: &str = "LF_SUBAGENT_TOKEN";

/// One ordered increment from the resident's harness stream, applied by the
/// listener's fold ([`crate::wave::runtime::WaveRuntime::apply_resident_delta`]).
///
/// Per-turn order is the transport's order; turn ids never ride the wire —
/// the listener mints them from its journal seq exactly as before.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ResidentDelta {
    /// A vendor turn opened. `answers` is the consumption declaration: the
    /// queued message ids this turn's input consumed (the resident drained
    /// its queue). The listener validates each id against its pending fold —
    /// unknown or already-consumed ids are dropped with a warning — and
    /// journals the valid set in `TurnStarted.answers`.
    TurnOpened { answers: Vec<String> },
    /// A prose fragment of the open turn.
    TurnText { text: String },
    /// A non-prose item (tool / command / file / thought) of the open turn.
    TurnItem { item: ConversationItem },
    /// Token usage reported for the open turn (accumulates).
    TurnUsage {
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        cache_read_tokens: Option<u64>,
    },
    /// The open turn finalized. The listener already holds the turn's content
    /// (grown from the Text/Item deltas); only the terminal status and cost
    /// cross the wire.
    TurnFinished {
        status: Lifecycle,
        cost_usd: Option<f64>,
    },
    /// Mid-turn consumption: the harness accepted these queued messages as
    /// steering input, so the CURRENT turn answers them (journaled as
    /// `TurnSteered.answers`). Validated like `TurnOpened.answers`.
    TurnSteered { answers: Vec<String> },
    /// The undo of a consumption claim: these message ids were declared
    /// consumed (`TurnSteered`) but the vendor never received the input —
    /// the harness send failed AFTER the claim was journaled. The listener
    /// returns them to its pending fold so the next resident's replay
    /// re-delivers them. The claim rides first, the undo is explicit:
    /// at-most-once to the vendor, never a silent redelivery.
    MessagesRequeued { ids: Vec<String> },
    /// The resident's reported mind state ([`ResidentStateTo`]). `Turning`
    /// and the boundary `Idle` are DERIVED by the listener from
    /// `TurnOpened`/`TurnFinished`; only the transitions the turn deltas
    /// can't express ride here.
    MindState { to: ResidentStateTo, reason: String },
    /// The vendor thread the mind runs on — its first durable act, journaled
    /// by the listener before the first turn (borrowed-handle rule).
    ThreadStarted { vendor: String, thread_id: String },
}

/// Destination of a reported [`ResidentDelta::MindState`] transition. The
/// listener supplies the turn id for `Interrupting` (the current open turn —
/// the resident never learns journal-minted ids).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResidentStateTo {
    /// Cancel fired for the open turn (cooperative interrupt in flight).
    Interrupting,
    /// The mind itself died (harness terminal error, failure cap). The
    /// resident reports this and exits; the listener's supervisor owns the
    /// respawn ladder from there.
    Failed,
}

/// `POST /resident/deltas` request. Deltas apply in order; the resident sends
/// batches serially (one in flight at a time), so order is total.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostDeltasRequest {
    pub deltas: Vec<ResidentDelta>,
}

/// `POST /resident/deltas` response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PostDeltasResponse {
    /// Number of deltas applied (dropped-by-guard deltas still count as
    /// accepted — the guard is the listener's business, not a retry signal).
    pub accepted: u64,
}

/// `POST /resident/attach` request: the resident's first call. Registers the
/// resident's pid for liveness (the listener probes attached residents; a
/// spawned child is watched by process exit) and revives a `failed` mind
/// state — a fresh resident IS the revival.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttachRequest {
    pub pid: u32,
}

/// `POST /resident/attach` response: the bootstrap the resident needs before
/// its first turn. The queued messages arrive via the `/events?inbox=true`
/// subscription replay, not here — one door per direction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AttachResponse {
    /// The wave this listener serves — the resident refuses a mismatch.
    pub wave: String,
    /// Last journaled vendor thread id — the mind's resume handle.
    pub thread_id: Option<String>,
}

/// `GET /resident/context` response: the pre-turn snapshot the resident folds
/// into its prompts. Serving it freshens the listener's store observations
/// (one poll), so the `<in_flight>` view is current rather than one poll
/// cadence stale.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextResponse {
    pub thread_id: Option<String>,
    pub in_flight: Vec<InFlightWorker>,
}

/// One dispatched-not-finished worker, for the heartbeat's `<in_flight>`
/// section.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InFlightWorker {
    pub run_id: String,
    pub flow: String,
    pub task: String,
}

/// One `inbox` SSE frame on `/events?inbox=true` — a resident-directed op.
/// `id` is the journaled message id (`"msg-<seq>"`), or `None` for a bare
/// interrupt (nothing journaled — the op is control, not content).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InboxFrame {
    pub id: Option<String>,
    pub op: MessageOp,
    pub text: String,
    pub from: Option<Attribution>,
}

/// One `op` SSE frame on `/events` — this wave's operational motion: a worker
/// run starting or finishing, observed by the wave server's [`StoreObserver`]
/// (`crate::wave::registry`). `kind` reuses the `run_events` ledger vocabulary
/// 1:1 (`<node>.<event>`) so the durable ledger row and the live frame carry
/// the same shape — a client folds `lf runs` history and live `op` frames with
/// one code path. The base model emits run-grain kinds (`run.started`,
/// `run.completed`, `run.errored`); step/flow granularity is future work, so
/// `kind` stays a free-form string like the ledger's `node`/`event` columns.
///
/// Live-only: the past is a ledger query (`lf runs`), never a stream — nothing
/// replays `op` frames on connect. DTO discipline: `flow`/`task`/`summary` are
/// explicitly Optional (a finish carries a summary but no flow; a start the
/// reverse); `kind`/`run_id`/`ts` are always present.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpFrame {
    /// `<node>.<event>` from the ledger vocabulary: `run.started`,
    /// `run.completed`, `run.errored`.
    pub kind: String,
    pub run_id: String,
    /// The run's flow, on a `run.started` frame; absent on a finish.
    pub flow: Option<String>,
    /// The run's task text, on a `run.started` frame; absent on a finish.
    pub task: Option<String>,
    /// What the registry knows about a finish (session status, PR url, error);
    /// absent on a start.
    pub summary: Option<String>,
    /// RFC3339 emission time.
    pub ts: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resident_delta_round_trips_every_variant() {
        let deltas = vec![
            ResidentDelta::TurnOpened {
                answers: vec!["msg-1".into(), "msg-2".into()],
            },
            ResidentDelta::TurnText {
                text: "thinking".into(),
            },
            ResidentDelta::TurnItem {
                item: ConversationItem::Tool {
                    id: "t-1".into(),
                    name: "Bash".into(),
                    status: Lifecycle::Completed,
                    input: None,
                    output: Some("ok".into()),
                },
            },
            ResidentDelta::TurnUsage {
                input_tokens: Some(10),
                output_tokens: Some(4),
                cache_read_tokens: None,
            },
            ResidentDelta::TurnFinished {
                status: Lifecycle::Completed,
                cost_usd: Some(0.02),
            },
            ResidentDelta::TurnSteered {
                answers: vec!["msg-3".into()],
            },
            ResidentDelta::MessagesRequeued {
                ids: vec!["msg-3".into()],
            },
            ResidentDelta::MindState {
                to: ResidentStateTo::Failed,
                reason: "harness disconnected".into(),
            },
            ResidentDelta::ThreadStarted {
                vendor: "codex".into(),
                thread_id: "thread-abc".into(),
            },
        ];
        for delta in deltas {
            let value = serde_json::to_value(&delta).expect("serialize");
            let decoded: ResidentDelta = serde_json::from_value(value).expect("deserialize");
            assert_eq!(decoded, delta);
        }
    }

    /// No serde defaults: an absent REQUIRED field is a parse error, never a
    /// silent fill-in. Absent `Option` fields decode as `None` — explicitly
    /// Optional is the one sanctioned absence.
    #[test]
    fn absent_required_fields_are_parse_errors() {
        for bad in [
            serde_json::json!({ "kind": "turn_opened" }),
            serde_json::json!({ "kind": "turn_finished", "cost_usd": null }),
            serde_json::json!({ "kind": "turn_text" }),
            serde_json::json!({ "kind": "mind_state", "to": "failed" }),
            serde_json::json!({ "kind": "messages_requeued" }),
            serde_json::json!({ "kind": "thread_started", "vendor": "codex" }),
        ] {
            assert!(
                serde_json::from_value::<ResidentDelta>(bad.clone()).is_err(),
                "must reject {bad}"
            );
        }
        assert!(serde_json::from_value::<InboxFrame>(
            serde_json::json!({ "id": null, "text": "hi", "from": null })
        )
        .is_err());

        // Explicitly Optional fields may be absent: they decode as None.
        let frame: InboxFrame = serde_json::from_value(
            serde_json::json!({ "op": "message", "text": "hi", "id": null, "from": null }),
        )
        .expect("explicit nulls parse");
        assert_eq!(frame.id, None);
        assert_eq!(frame.from, None);
    }

    /// The `op` frame round-trips and holds DTO discipline: `kind`/`run_id`/
    /// `ts` are required (an absent one is a parse error), while
    /// `flow`/`task`/`summary` are explicitly Optional (a finish carries a
    /// summary but no flow; a start the reverse).
    #[test]
    fn op_frame_round_trips_and_enforces_required_fields() {
        let started = OpFrame {
            kind: "run.started".into(),
            run_id: "run-1".into(),
            flow: Some("implement".into()),
            task: Some("wire it".into()),
            summary: None,
            ts: "2026-07-06T00:00:00Z".into(),
        };
        let decoded: OpFrame =
            serde_json::from_value(serde_json::to_value(&started).unwrap()).unwrap();
        assert_eq!(decoded, started);

        // A required field absent is a parse error, never a silent default.
        assert!(serde_json::from_value::<OpFrame>(
            serde_json::json!({ "run_id": "run-1", "ts": "t" })
        )
        .is_err());

        // A finish: no flow/task, a summary present. Absent Optionals decode
        // as None.
        let finished: OpFrame = serde_json::from_value(serde_json::json!({
            "kind": "run.errored",
            "run_id": "run-1",
            "summary": "session failed",
            "ts": "t"
        }))
        .expect("optionals may be absent");
        assert_eq!(finished.flow, None);
        assert_eq!(finished.task, None);
        assert_eq!(finished.summary.as_deref(), Some("session failed"));
    }
}
