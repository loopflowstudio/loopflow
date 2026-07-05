//! The wave server's HTTP surface — a thin view over in-process state.
//!
//! Every endpoint reads or nudges [`WaveRuntime`]; none of them own logic. The
//! timeline is served as-is, live events stream over SSE, and a POSTed message
//! is journaled and broadcast to the resident's subscription. Discovery is a
//! dumb pointer file, not a transport: `wave/<name>/.wave-endpoint` holds
//! `127.0.0.1:<port>` and nothing else; `.wave-resident-token` beside it holds
//! this boot's resident token (see [`crate::wave::wire`]).
//!
//! This module is VENDOR-FREE: the mind lives in the resident process
//! ([`crate::wave::resident`]), which publishes through the resident door
//! (`/resident/attach`, `/resident/deltas`, `/resident/context` — token-gated)
//! and listens on its own wave's `/events?inbox=true` subscription. The
//! listener holds every pen; the resident holds the vendor.
//!
//! The server serves the wave's CHANNEL FAMILY (see [`crate::wave::channel`]):
//! the primary channel is the wave's name; work-line channels are addressed
//! by their ownership names (`goals.148e0e02`). It holds the pen for every
//! child journal; doors are name-addressed.
//!
//! Wire contract (snake_case, stable — a Concerto worker builds against it):
//! - `GET /health` → `{status, mind, wave, turns, workers, uptime_seconds}`;
//!   `status` is CHANNEL liveness — always `serving` while this process
//!   answers; `mind` is the resident's state (`idle | turning | interrupting
//!   | failed`), or null while no resident has ever been spawned or attached
//!   (`--no-mind` serves dormant); a served channel whose resident died reads
//!   `status: "serving", mind: "failed"`. `workers` counts this wave's
//!   observed in-flight worker runs.
//! - `GET /conversation` → `{turns: [Turn]}`; includes the open turn (status
//!   `running`), if one is in progress, after the finalized thread. Optional
//!   `?limit=N` tails the last N turns (open turn included) — `wave_context`
//!   passes 12; absent means the whole thread. Primary channel only.
//! - `GET /events` → SSE, the family's one unified stream. Scope by query:
//!   `?channel=<name>` (exactly one channel), `?prefix=<name>` (that subtree),
//!   default = the whole family. A name outside this wave's family is a 404.
//!   Three event names:
//!   - `state`: data is the mind-state name (`idle | turning | interrupting |
//!     failed`), sent once on subscribe (before the turn replay) and again on
//!     every transition — the composer keys its verb off it. Primary channel
//!     only — child channels have no mind, so a child-only subscription
//!     carries no `state` frames.
//!   - `turn`: data is a `Turn` JSON; the thread replays on connect
//!     (including the open turn), then streams live. A turn from a CHILD
//!     channel carries one extra key, `"channel": "<name>"`; the primary
//!     channel's turns ride untagged (absent `channel` = the wave's own
//!     channel), so a family of one is byte-identical to the pre-family wire.
//!     Turn ids repeat — and repeat across channels: an in-progress turn is
//!     re-sent whole as it grows and finalization sends the terminal turn
//!     under the same id — each frame replaces the client's previous state
//!     for that (channel, id) pair (upsert, never append-if-seen).
//!   - `memory`: data is the `MemoryUpdated` summary string, fired on every
//!     curation. Live-only, no replay — MEMORY.md itself is the durable
//!     state. Primary channel only (memory is wave identity; work lines have
//!     none).
//!   - `inbox` (only with `?inbox=true`, the resident's subscription): data
//!     is an [`InboxFrame`] — a resident-directed op. The pending queue
//!     (journaled messages not yet named in any `answers`) replays on
//!     connect, then live ops stream; a bare interrupt rides live-only with
//!     `id: null` (nothing journaled). Primary channel only. The default
//!     stream is byte-identical to the pre-resident wire.
//! - The resident door (token-gated via the `x-lf-resident-token` header —
//!   401 without this boot's token):
//!   - `POST /resident/attach {pid}` → `{wave, thread_id}` — registers the
//!     resident's pid for liveness and revives a `failed` mind (a fresh
//!     resident IS the revival).
//!   - `POST /resident/deltas {deltas: [...]}` → `{accepted}` — ordered turn
//!     deltas, applied to the journal fold
//!     ([`WaveRuntime::apply_resident_delta`]).
//!   - `GET /resident/context` → `{thread_id, in_flight}` — the pre-turn
//!     snapshot; serving it freshens the store observations (one poll).
//! - `POST /messages {op, text, from?, channel?}` → `{turn, state}`. `op` is
//!   required — `"message"` (human speech steers a live steer-capable turn;
//!   otherwise queued, the next turn answers it), `"steer"`
//!   (into the live turn when the harness supports it, else degrades to a
//!   queued message), `"interrupt"` (cancel the open turn; non-empty text
//!   becomes the next turn — "interrupt & send"; while idle, an interrupt is
//!   a no-op success), or `"say"` (an attributed emission — `lf chat`: a
//!   worker report, child-wave escalation, or CLI FYI; lands in the thread
//!   with its byline AND queues for the mind like a message). `text` may be
//!   empty only for `interrupt` (400 otherwise). `from {session_id?, label}`
//!   is required for `say` and rejected for every other op (400) — human
//!   turns are unattributed by convention. `channel` is explicitly Optional:
//!   null targets the wave channel (unchanged); a child name lands the
//!   message in THAT channel's journal (404 outside the family or when the
//!   work line's worktree is gone). On a child channel there is no resident:
//!   steer degrades to a plain message, a bare interrupt is a no-op. `turn`
//!   is the appended user `Turn`, or null for a bare interrupt (nothing was
//!   said); `state` is the mind-state name when the request was accepted —
//!   ops are applied by the mind asynchronously, so watch the stream's
//!   `state` events for the outcome.
//! - `POST /channels {name, run_id}` → `{turn}` — the dispatch notification
//!   door: `lf q worker run` minted a work-line worktree and its channel
//!   journal, and knocks here so the PARENT channel's thread shows
//!   "work line <name> opened" (journaled as `ChannelOpened`, idempotent on
//!   `run_id` — a repeated knock returns `{turn: null}`). 404 outside the
//!   family.
//! - `GET /memory` → `{content}` — the wave's MEMORY.md, read from the
//!   origin repo. Wave-level only: memory is wave identity, channels don't
//!   have it.
//! - `POST /memory {op, content, summary}` → `{summary}`. `op` is `"update"`
//!   (full replacement) or `"add"` (append one curated bullet; `content` must
//!   be non-empty). `summary` is explicitly Optional — null falls back to the
//!   content's first non-empty line. The server is the sole writer of the
//!   origin repo's `wave/<name>/MEMORY.md` and journals `MemoryUpdated`.
//!
//! `Turn` is [`crate::lfd::conversations::turns::ChatTurn`].

use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::stream::{self, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio_stream::wrappers::BroadcastStream;

use crate::lfd::conversations::turns::ChatTurn;
use crate::wave::channel::tagged_turn_json;
use crate::wave::journal::{Attribution, MessageOp, PendingMessage};
use crate::wave::registry::{process_alive, StoreObserver};
use crate::wave::runtime::{InboxItem, WaveRuntime};
use crate::wave::state::MindState;
use crate::wave::supervisor::SupervisorHandle;
use crate::wave::wire::{
    AttachRequest, AttachResponse, ContextResponse, InFlightWorker, InboxFrame, PostDeltasRequest,
    PostDeltasResponse, RESIDENT_TOKEN_FILE, RESIDENT_TOKEN_HEADER,
};

/// Basename of the discovery pointer under `wave/<name>/`.
pub const ENDPOINT_FILE: &str = ".wave-endpoint";

/// The resident door's server-side state: this boot's token and the seat —
/// the attached resident's pid, for liveness probing. Shared with the
/// supervisor ([`crate::wave::supervisor`]), which probes attached pids and
/// clears the seat when the resident dies.
#[derive(Debug, Clone)]
pub struct ResidentDoor {
    token: String,
    seat: Arc<Mutex<Option<u32>>>,
}

impl ResidentDoor {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: token.into(),
            seat: Arc::new(Mutex::new(None)),
        }
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    /// The attached resident's pid, if one has attached and not been cleared.
    pub fn seat_pid(&self) -> Option<u32> {
        *self.seat.lock().expect("resident seat lock poisoned")
    }

    /// Record the resident occupying the seat (attach, or spawn).
    pub fn record_pid(&self, pid: u32) {
        *self.seat.lock().expect("resident seat lock poisoned") = Some(pid);
    }

    /// The resident died: free the seat.
    pub fn clear_seat(&self) {
        *self.seat.lock().expect("resident seat lock poisoned") = None;
    }

    fn authorize(&self, headers: &HeaderMap) -> Result<(), (StatusCode, String)> {
        let presented = headers
            .get(RESIDENT_TOKEN_HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if presented == self.token {
            return Ok(());
        }
        Err((
            StatusCode::UNAUTHORIZED,
            format!("missing or wrong {RESIDENT_TOKEN_HEADER}"),
        ))
    }
}

/// A fresh per-boot resident token.
pub fn generate_resident_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

#[derive(Debug, Serialize)]
struct HealthBody {
    /// Channel liveness: always `"serving"` while this process answers. The
    /// resident's condition is `mind` — a served channel whose resident died
    /// is `status: "serving", mind: "failed"`.
    status: String,
    /// Resident (mind) state name, or null for a channel with no resident
    /// (a dormant `--no-mind` channel, or before any resident attaches).
    mind: Option<String>,
    wave: String,
    turns: usize,
    /// Workers observed in flight for this wave (dispatch is daemonless —
    /// `lf q worker run` — so the store fold, not a task registry, is truth).
    workers: usize,
    /// Whether the wave is paused (GOAL.md `paused: true`): the listener
    /// refuses to start turns while set, though it keeps serving and queueing.
    paused: bool,
    uptime_seconds: i64,
}

#[derive(Debug, Serialize)]
struct ConversationBody {
    turns: Vec<ChatTurn>,
}

/// `GET /conversation` query. `limit` is explicitly Optional: `None` serves
/// the whole thread, `Some(n)` tails the last n turns.
#[derive(Debug, Deserialize)]
struct ConversationQuery {
    limit: Option<usize>,
}

/// `POST /messages` request body. `op` is required — explicit, never inferred
/// (no serde default; an op-less body is a 422). `from` is explicitly
/// Optional: required for `say`, rejected otherwise. `channel` is explicitly
/// Optional: null = the wave channel; a child name addresses that channel's
/// journal (404 outside the family).
#[derive(Debug, Deserialize)]
struct PostMessage {
    op: MessageOp,
    text: String,
    from: Option<Attribution>,
    channel: Option<String>,
}

/// `POST /channels` request body — the dispatch notification (see module
/// doc). Both fields required.
#[derive(Debug, Deserialize)]
struct PostChannel {
    name: String,
    run_id: String,
}

/// `POST /channels` response: the thread-visible opening turn, or null when
/// the run's opening was already journaled (idempotent knock).
#[derive(Debug, Serialize)]
struct PostChannelResponse {
    turn: Option<ChatTurn>,
}

/// `GET /events` scope query. `channel`/`prefix` are explicitly Optional;
/// setting both is a 400; absent = the whole family. `inbox` is explicitly
/// Optional: `true` adds the resident's `inbox` frames (pending replay +
/// live ops) to a primary-scope subscription; absent/false leaves the wire
/// byte-identical to the pre-resident stream.
#[derive(Debug, Deserialize)]
struct EventsQuery {
    channel: Option<String>,
    prefix: Option<String>,
    inbox: Option<bool>,
}

/// `GET /memory` response.
#[derive(Debug, Serialize)]
struct MemoryBody {
    content: String,
}

/// `POST /memory` op — full replacement or one appended bullet.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum MemoryOp {
    Update,
    Add,
}

/// `POST /memory` request body. `summary` is explicitly Optional — null falls
/// back to the content's first non-empty line.
#[derive(Debug, Deserialize)]
struct PostMemory {
    op: MemoryOp,
    content: String,
    summary: Option<String>,
}

/// `POST /memory` response: the summary that was journaled.
#[derive(Debug, Serialize)]
struct PostMemoryResponse {
    summary: String,
}

/// `POST /messages` response. `turn` is the appended user turn; null for a
/// bare interrupt, which appends nothing. `state` is the mind-state name at
/// acceptance time.
#[derive(Debug, Serialize)]
struct PostMessageResponse {
    turn: Option<ChatTurn>,
    state: String,
}

/// Server state: the runtime, the resident door, the store observer (for the
/// context door's freshness poll), the supervisor handle (to signal an
/// attach), and when the server started (for uptime).
#[derive(Clone)]
struct ServerState {
    runtime: Arc<WaveRuntime>,
    resident: ResidentDoor,
    observer: Option<Arc<StoreObserver>>,
    supervisor: Option<SupervisorHandle>,
    started_at: OffsetDateTime,
}

/// Build the router over a running [`WaveRuntime`]. `observer` is the store
/// poller when this server is registered — `GET /resident/context` freshens
/// it before serving. `supervisor` lets the attach door stand the respawn
/// ladder down (`None` in tests without a supervisor).
pub fn router(
    runtime: Arc<WaveRuntime>,
    resident: ResidentDoor,
    observer: Option<Arc<StoreObserver>>,
    supervisor: Option<SupervisorHandle>,
) -> Router {
    let state = ServerState {
        runtime,
        resident,
        observer,
        supervisor,
        started_at: OffsetDateTime::now_utc(),
    };
    Router::new()
        .route("/health", get(health_handler))
        .route("/conversation", get(conversation_handler))
        .route("/events", get(events_handler))
        .route("/messages", post(messages_handler))
        .route("/channels", post(channels_handler))
        .route("/memory", get(memory_handler).post(memory_write_handler))
        .route("/resident/attach", post(resident_attach_handler))
        .route("/resident/deltas", post(resident_deltas_handler))
        .route("/resident/context", get(resident_context_handler))
        .with_state(state)
}

async fn health_handler(State(state): State<ServerState>) -> Json<HealthBody> {
    // `mind` is null until a resident has ever been spawned or attached —
    // a dormant channel (`--no-mind`) has no mind to report on.
    let mind = state
        .runtime
        .resident_expected()
        .then(|| state.runtime.mind_state().name().to_string());
    Json(HealthBody {
        status: "serving".to_string(),
        mind,
        wave: state.runtime.name().to_string(),
        turns: state.runtime.thread_len(),
        workers: state.runtime.in_flight_workers().len(),
        paused: state.runtime.paused(),
        uptime_seconds: (OffsetDateTime::now_utc() - state.started_at).whole_seconds(),
    })
}

// -- The resident door (token-gated; see crate::wave::wire) --

async fn resident_attach_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(body): Json<AttachRequest>,
) -> Result<Json<AttachResponse>, (StatusCode, String)> {
    state.resident.authorize(&headers)?;
    // Seat exclusivity: one mind per wave. A live seat already probed alive
    // refuses the attach naming it — a second resident would split-brain the
    // wire. A dead/absent seat is free (takeover after a crash rides the same
    // door; the supervisor's own seat probe frees a dead pid on its cadence).
    // `--force` is `lf wave`'s boot flag, not the door's business.
    if let Some(seated) = state.resident.seat_pid() {
        if seated != body.pid && process_alive(seated).await {
            return Err((
                StatusCode::CONFLICT,
                format!(
                    "wave '{}' already has a live resident on the seat (pid {seated}); \
                     stop it before attaching, or use `lf wave <name> --force` to take over",
                    state.runtime.name()
                ),
            ));
        }
    }
    state.resident.record_pid(body.pid);
    state.runtime.set_resident_expected();
    // Tell the keeper: an attached resident stands the respawn ladder down
    // (the fresh resident IS the revival) and is watched by pid probe.
    if let Some(supervisor) = &state.supervisor {
        supervisor.on_attach(body.pid);
    }
    // A fresh resident IS the revival: a failed mind goes idle on attach.
    if matches!(state.runtime.mind_state(), MindState::Failed { .. }) {
        state
            .runtime
            .transition(MindState::Idle, "resident attached");
    }
    tracing::info!(pid = body.pid, "resident attached");
    Ok(Json(AttachResponse {
        wave: state.runtime.name().to_string(),
        thread_id: state.runtime.last_thread_id(),
    }))
}

async fn resident_deltas_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(body): Json<PostDeltasRequest>,
) -> Result<Json<PostDeltasResponse>, (StatusCode, String)> {
    state.resident.authorize(&headers)?;
    let accepted = body.deltas.len() as u64;
    for delta in body.deltas {
        state.runtime.apply_resident_delta(delta);
    }
    Ok(Json(PostDeltasResponse { accepted }))
}

async fn resident_context_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
) -> Result<Json<ContextResponse>, (StatusCode, String)> {
    state.resident.authorize(&headers)?;
    // Freshen the store fold so the resident's next turn sees current
    // workers, not a poll cadence's stale view.
    if let Some(observer) = &state.observer {
        observer.poll_once().await;
    }
    let in_flight = state
        .runtime
        .in_flight_workers()
        .into_iter()
        .map(|worker| InFlightWorker {
            run_id: worker.run_id,
            flow: worker.flow,
            task: worker.task,
        })
        .collect();
    Ok(Json(ContextResponse {
        thread_id: state.runtime.last_thread_id(),
        in_flight,
    }))
}

async fn conversation_handler(
    State(state): State<ServerState>,
    Query(query): Query<ConversationQuery>,
) -> Json<ConversationBody> {
    // The tail is taken inside the runtime lock: a `?limit=N` request clones
    // only the N turns it serves, not the whole thread.
    Json(ConversationBody {
        turns: state.runtime.thread_tail(query.limit),
    })
}

/// The door is opaque on resident ops: this handler validates SHAPE only —
/// `from` rides `say` and nothing else; `text` may be empty only for
/// `interrupt` — then hands the op to the runtime uninterpreted
/// ([`WaveRuntime::deliver`]). What steer or interrupt *means* lives with the
/// resident, not the ear. Honest partial: the `{turn, state}` echo still
/// leaks that a bare interrupt appends nothing (`turn: null`), but that fact
/// comes back from the runtime's return, not from the door interpreting.
async fn messages_handler(
    State(state): State<ServerState>,
    Json(body): Json<PostMessage>,
) -> Result<Json<PostMessageResponse>, (StatusCode, String)> {
    if body.from.is_some() && !matches!(body.op, MessageOp::Say) {
        return Err((
            StatusCode::BAD_REQUEST,
            "`from` is only valid for the say op".to_string(),
        ));
    }
    if matches!(body.op, MessageOp::Say) && body.from.is_none() {
        return Err((
            StatusCode::BAD_REQUEST,
            "`from` is required for the say op".to_string(),
        ));
    }
    if body.text.trim().is_empty() && !matches!(body.op, MessageOp::Interrupt) {
        return Err((
            StatusCode::BAD_REQUEST,
            "text is required for every op but interrupt".to_string(),
        ));
    }
    let channel = body
        .channel
        .unwrap_or_else(|| state.runtime.name().to_string());
    if !state.runtime.in_family(&channel) {
        return Err((
            StatusCode::NOT_FOUND,
            format!(
                "channel '{channel}' is not in wave '{}''s family",
                state.runtime.name()
            ),
        ));
    }
    let turn = state
        .runtime
        .deliver_to_channel(&channel, body.op, body.text, body.from)
        .map_err(|err| (StatusCode::NOT_FOUND, err.to_string()))?;
    Ok(Json(PostMessageResponse {
        turn,
        state: state.runtime.mind_state().name().to_string(),
    }))
}

/// The dispatch notification door: journal `ChannelOpened` on the primary
/// channel (idempotent on run id) so the wave's thread shows the work line
/// opening. The child channel itself materializes lazily on first delivery
/// or subscription — the journal file was already minted by the dispatcher.
async fn channels_handler(
    State(state): State<ServerState>,
    Json(body): Json<PostChannel>,
) -> Result<Json<PostChannelResponse>, (StatusCode, String)> {
    if state.runtime.is_primary(&body.name) || !state.runtime.in_family(&body.name) {
        return Err((
            StatusCode::NOT_FOUND,
            format!(
                "'{}' is not a child channel of wave '{}'",
                body.name,
                state.runtime.name()
            ),
        ));
    }
    let turn = state
        .runtime
        .journal_channel_opened(&body.name, &body.run_id);
    Ok(Json(PostChannelResponse { turn }))
}

async fn memory_handler(State(state): State<ServerState>) -> Json<MemoryBody> {
    Json(MemoryBody {
        content: state.runtime.memory().read(),
    })
}

async fn memory_write_handler(
    State(state): State<ServerState>,
    Json(body): Json<PostMemory>,
) -> Result<Json<PostMemoryResponse>, (StatusCode, String)> {
    let summary = body
        .summary
        .filter(|s| !s.trim().is_empty())
        .or_else(|| first_line(&body.content))
        .unwrap_or_else(|| "memory cleared".to_string());
    let result = match body.op {
        MemoryOp::Update => state.runtime.update_memory(&body.content, &summary),
        MemoryOp::Add => {
            let fact = body.content.trim();
            if fact.is_empty() {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "content is required for the add op".to_string(),
                ));
            }
            state.runtime.append_memory(fact, &summary)
        }
    };
    match result {
        Ok(()) => Ok(Json(PostMemoryResponse { summary })),
        Err(err) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("memory write failed: {err}"),
        )),
    }
}

fn first_line(content: &str) -> Option<String> {
    content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

/// The unified `/events` SSE, scoped to one channel, a subtree, or (default)
/// the whole family.
///
/// The primary channel's replay-then-live shape is unchanged: the mind state,
/// the thread on connect (open turn included, status `running`), then live
/// frames — `state` on every transition, `turn` ids repeating by design
/// (every frame replaces the client's state for that (channel, id), so an
/// in-progress turn updates in place and its terminal frame lands under the
/// same id), `memory` on every curation (live-only; the file is the durable
/// state). Snapshot and subscription are atomic in the runtime (broadcasts
/// share the append lock), so no primary live frame is ever older than the
/// replayed snapshot.
///
/// Child channels replay their folded threads (turn frames tagged with
/// `channel`) and stream live off the family bus, subscribed BEFORE the
/// snapshots — a frame can repeat across the boundary, never go missing.
/// A subscription that names a child channel with no journal yet just waits:
/// the channel may open later.
async fn events_handler(
    State(state): State<ServerState>,
    Query(query): Query<EventsQuery>,
) -> Result<axum::response::Response, (StatusCode, String)> {
    let wave = state.runtime.name().to_string();
    let include_inbox = query.inbox == Some(true);
    let (scope, primary) = match (query.channel, query.prefix) {
        (Some(_), Some(_)) => {
            return Err((
                StatusCode::BAD_REQUEST,
                "pass channel or prefix, not both".to_string(),
            ));
        }
        (Some(channel), None) => {
            let primary = state.runtime.is_primary(&channel);
            (Scope::Channel(channel), primary)
        }
        (None, Some(prefix)) => {
            let primary = state.runtime.is_primary(&prefix);
            (Scope::Prefix(prefix), primary)
        }
        (None, None) => (
            Scope::Prefix(state.runtime.channel_name().to_string()),
            true,
        ),
    };
    let name = scope.name();
    if !state.runtime.in_family(name) {
        return Err((
            StatusCode::NOT_FOUND,
            format!("'{name}' is not in wave '{wave}''s family"),
        ));
    }

    // Child channels: live bus first, snapshots second (see method doc). A
    // `?channel=` scope replays STRICTLY the one named channel (no
    // descendants), matching how it streams (strict equality); a `?prefix=`
    // scope replays the whole subtree. The primary scope carries no child
    // snapshots — its own thread rides the primary subscription below.
    let (child_snapshots, family_rx) = match &scope {
        Scope::Channel(channel) if state.runtime.is_primary(channel) => (Vec::new(), None),
        Scope::Channel(channel) => {
            let (snapshot, rx) = state.runtime.subscribe_child(channel);
            let snapshots = snapshot
                .map(|turns| vec![(channel.clone(), turns)])
                .unwrap_or_default();
            (snapshots, Some(rx))
        }
        Scope::Prefix(prefix) => {
            let (snapshots, rx) = state.runtime.subscribe_children(prefix);
            (snapshots, Some(rx))
        }
    };
    let child_replay = stream::iter(child_snapshots.into_iter().flat_map(|(channel, turns)| {
        turns
            .into_iter()
            .map(move |turn| Ok(tagged_turn_event(&channel, &turn)))
            .collect::<Vec<_>>()
    }));
    let live_children = family_rx.map(|rx| {
        let scope = scope.clone();
        BroadcastStream::new(rx).filter_map(move |res| {
            let out = match res {
                // The frame carries its tagged JSON, serialized once at the
                // send site — every subscriber reuses it.
                Ok(frame) if scope.matches(&frame.channel) => {
                    Some(Ok(Event::default().event("turn").data(frame.json.as_ref())))
                }
                // Out of scope, or lagged (the journal has it; a client
                // that fell behind resyncs on reconnect).
                _ => None,
            };
            async move { out }
        })
    });

    let mut streams: Vec<BoxedEventStream> = Vec::new();
    if primary {
        let sub = state.runtime.subscribe_with_snapshot();
        // The resident's subscription replays the pending queue after the
        // thread — its boot inbox. Consumption is validated at the resident
        // door, so a stale replay can never double-consume.
        let inbox_replay: Vec<Result<Event, Infallible>> = if include_inbox {
            sub.pending
                .iter()
                .map(|message| Ok(inbox_event(&pending_inbox_frame(message))))
                .collect()
        } else {
            Vec::new()
        };
        let replay = stream::iter(
            std::iter::once(Ok(state_event(&sub.state)))
                .chain(sub.turns.into_iter().map(|t| Ok(turn_event(&t))))
                .chain(inbox_replay),
        );
        let live_turns = BroadcastStream::new(sub.turn_rx).filter_map(move |res| {
            let out = match res {
                // The frame's wire JSON was serialized once at the send site.
                Ok(frame) => Some(Ok(Event::default().event("turn").data(frame.json.as_str()))),
                // Lagged: the client fell behind. Skip; it resyncs from /conversation.
                Err(_) => None,
            };
            async move { out }
        });
        let live_states = BroadcastStream::new(sub.state_rx).filter_map(move |res| {
            let out = match res {
                Ok(mind_state) => Some(Ok(state_event(&mind_state))),
                // Lagged: fine — the next transition carries the current state.
                Err(_) => None,
            };
            async move { out }
        });
        let live_memory = BroadcastStream::new(sub.memory_rx).filter_map(move |res| {
            let out = match res {
                Ok(summary) => Some(Ok(memory_event(&summary))),
                // Lagged: fine — MEMORY.md itself is the durable state.
                Err(_) => None,
            };
            async move { out }
        });
        let mut live: BoxedEventStream = Box::pin(stream::select(
            live_turns,
            stream::select(live_states, live_memory),
        ));
        if include_inbox {
            let live_inbox = BroadcastStream::new(sub.inbox_rx).filter_map(move |res| {
                let out = match res {
                    Ok(item) => Some(Ok(inbox_event(&inbox_item_frame(&item)))),
                    // Lagged: the pending fold is the durable queue; a
                    // resident that falls behind resubscribes.
                    Err(_) => None,
                };
                async move { out }
            });
            live = Box::pin(stream::select(live, live_inbox));
        }
        streams.push(Box::pin(replay.chain(live)));
    }
    match live_children {
        Some(live) => streams.push(Box::pin(child_replay.chain(live))),
        None => streams.push(Box::pin(child_replay)),
    }
    let merged: BoxedEventStream = match streams.len() {
        1 => streams.pop().expect("one stream"),
        _ => {
            let children = streams.pop().expect("child stream");
            let primary = streams.pop().expect("primary stream");
            Box::pin(stream::select(primary, children))
        }
    };
    Ok(axum::response::IntoResponse::into_response(
        Sse::new(merged).keep_alive(KeepAlive::default()),
    ))
}

type BoxedEventStream =
    std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send + 'static>>;

/// The scope one `/events` subscription covers.
#[derive(Debug, Clone)]
enum Scope {
    /// Exactly one channel.
    Channel(String),
    /// A subtree: the named channel and every dot-descendant.
    Prefix(String),
}

impl Scope {
    fn name(&self) -> &str {
        match self {
            Self::Channel(name) | Self::Prefix(name) => name,
        }
    }

    fn matches(&self, channel: &str) -> bool {
        match self {
            Self::Channel(name) => channel == name,
            Self::Prefix(prefix) => crate::wave::channel::matches_prefix(channel, prefix),
        }
    }
}

fn turn_event(turn: &ChatTurn) -> Event {
    Event::default()
        .event("turn")
        .data(serde_json::to_string(turn).unwrap_or_default())
}

/// A child channel's turn frame for the REPLAY path: the `Turn` JSON plus one
/// extra key, `"channel"` (the live path reuses the frame's pre-serialized
/// `json`). Additive — the primary channel's frames stay untagged, so a
/// family of one is byte-identical to the pre-family wire.
fn tagged_turn_event(channel: &str, turn: &ChatTurn) -> Event {
    Event::default()
        .event("turn")
        .data(tagged_turn_json(channel, turn))
}

fn state_event(state: &MindState) -> Event {
    Event::default().event("state").data(state.name())
}

fn memory_event(summary: &str) -> Event {
    Event::default().event("memory").data(summary)
}

fn inbox_event(frame: &InboxFrame) -> Event {
    Event::default()
        .event("inbox")
        .data(serde_json::to_string(frame).unwrap_or_default())
}

fn pending_inbox_frame(message: &PendingMessage) -> InboxFrame {
    InboxFrame {
        id: Some(message.id.0.clone()),
        op: message.op,
        text: message.text.clone(),
        from: message.from.clone(),
    }
}

fn inbox_item_frame(item: &InboxItem) -> InboxFrame {
    match item {
        InboxItem::Message(message) => pending_inbox_frame(message),
        InboxItem::Interrupt => InboxFrame {
            id: None,
            op: MessageOp::Interrupt,
            text: String::new(),
            from: None,
        },
    }
}

/// Path to the discovery pointer for a wave.
pub fn endpoint_path(repo_root: &Path, wave: &str) -> PathBuf {
    repo_root.join("wave").join(wave).join(ENDPOINT_FILE)
}

/// How long the boot-time probe waits for an existing endpoint to answer.
const ENDPOINT_PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);

/// Probe an existing discovery pointer: `Some(addr)` when a live wave server
/// for `wave` answers `GET /health` at the recorded address. This is the
/// file-level one-brain floor — it works with no registry store at all
/// (observed live: a second unregistered server overwrote the pointer and,
/// on shutdown, deleted it, leaving the first server undiscoverable). A
/// missing/unreadable file, a dead address, or an answer for a different
/// wave is a stale pointer — `None`, safe to overwrite.
pub async fn live_endpoint(repo_root: &Path, wave: &str) -> Option<String> {
    let addr = std::fs::read_to_string(endpoint_path(repo_root, wave)).ok()?;
    let addr = addr.trim().to_string();
    if addr.is_empty() {
        return None;
    }
    let client = reqwest::Client::builder()
        .timeout(ENDPOINT_PROBE_TIMEOUT)
        .build()
        .ok()?;
    let body: serde_json::Value = client
        .get(format!("http://{addr}/health"))
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    (body.get("wave").and_then(serde_json::Value::as_str) == Some(wave)).then_some(addr)
}

/// Publish the loopback endpoint so Concerto can find the server. Writes ONLY
/// `127.0.0.1:<port>` — a pointer, never message content.
pub fn write_endpoint(
    repo_root: &Path,
    wave: &str,
    addr: std::net::SocketAddr,
) -> std::io::Result<()> {
    let path = endpoint_path(repo_root, wave);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, addr.to_string())
}

/// Remove the discovery pointer on shutdown — only when it still holds this
/// server's own address. A takeover that overwrote the file owns it now;
/// deleting it here would leave that live server undiscoverable. Best-effort.
pub fn remove_endpoint(repo_root: &Path, wave: &str, own_addr: &str) {
    let path = endpoint_path(repo_root, wave);
    match std::fs::read_to_string(&path) {
        Ok(contents) if contents.trim() == own_addr => {
            let _ = std::fs::remove_file(path);
        }
        _ => {}
    }
}

/// Path to the resident-token file for a wave (beside `.wave-endpoint`).
pub fn resident_token_path(repo_root: &Path, wave: &str) -> PathBuf {
    repo_root.join("wave").join(wave).join(RESIDENT_TOKEN_FILE)
}

/// Publish this boot's resident token so an attached resident (`lf wave
/// <name> --mind-only`) can present it — the same filesystem-trust domain as
/// the endpoint pointer. Owner-only on unix.
pub fn write_resident_token(repo_root: &Path, wave: &str, token: &str) -> std::io::Result<()> {
    let path = resident_token_path(repo_root, wave);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, token)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
    }
    Ok(())
}

/// Read the current resident token, for `--mind-only` attachment.
pub fn read_resident_token(repo_root: &Path, wave: &str) -> Option<String> {
    let token = std::fs::read_to_string(resident_token_path(repo_root, wave)).ok()?;
    let token = token.trim().to_string();
    (!token.is_empty()).then_some(token)
}

/// Remove the token file on shutdown — only while it still holds this boot's
/// token (a takeover owns the file now). Best-effort.
pub fn remove_resident_token(repo_root: &Path, wave: &str, own_token: &str) {
    let path = resident_token_path(repo_root, wave);
    match std::fs::read_to_string(&path) {
        Ok(contents) if contents.trim() == own_token => {
            let _ = std::fs::remove_file(path);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn write_and_remove_endpoint_roundtrips() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let addr: std::net::SocketAddr = "127.0.0.1:54321".parse().unwrap();
        write_endpoint(tmp.path(), "ship", addr).expect("write endpoint");

        let path = endpoint_path(tmp.path(), "ship");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "127.0.0.1:54321");

        remove_endpoint(tmp.path(), "ship", "127.0.0.1:54321");
        assert!(!path.exists());
    }

    /// A server taken over by `--force` must not delete the pointer the new
    /// server wrote: remove only what still holds our own address.
    #[test]
    fn remove_endpoint_leaves_a_foreign_pointer_untouched() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let addr: std::net::SocketAddr = "127.0.0.1:50000".parse().unwrap();
        write_endpoint(tmp.path(), "ship", addr).expect("write endpoint");

        remove_endpoint(tmp.path(), "ship", "127.0.0.1:50001");
        let path = endpoint_path(tmp.path(), "ship");
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "127.0.0.1:50000",
            "foreign pointer survives our shutdown"
        );
    }

    /// The token file round-trips and removal honors ownership, like the
    /// endpoint pointer.
    #[test]
    fn resident_token_file_roundtrips_and_respects_ownership() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert!(read_resident_token(tmp.path(), "ship").is_none());
        write_resident_token(tmp.path(), "ship", "tok-1").expect("write");
        assert_eq!(
            read_resident_token(tmp.path(), "ship").as_deref(),
            Some("tok-1")
        );

        // A foreign token (takeover) survives our shutdown; our own doesn't.
        remove_resident_token(tmp.path(), "ship", "tok-other");
        assert_eq!(
            read_resident_token(tmp.path(), "ship").as_deref(),
            Some("tok-1")
        );
        remove_resident_token(tmp.path(), "ship", "tok-1");
        assert!(read_resident_token(tmp.path(), "ship").is_none());
    }

    /// A pointer to a dead address is stale: the probe says no live server.
    #[tokio::test]
    async fn live_endpoint_is_none_for_a_stale_pointer() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert!(live_endpoint(tmp.path(), "ship").await.is_none(), "no file");

        // A port nothing listens on: bind, learn the address, drop it.
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dead = listener.local_addr().unwrap();
        drop(listener);
        write_endpoint(tmp.path(), "ship", dead).expect("write endpoint");
        assert!(
            live_endpoint(tmp.path(), "ship").await.is_none(),
            "dead address is stale"
        );
    }
}
