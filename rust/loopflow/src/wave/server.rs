//! The wave server's HTTP surface — a thin view over in-process state.
//!
//! Every endpoint reads or nudges [`WaveRuntime`]; none of them own logic. The
//! timeline is served as-is, live events stream over SSE, and a POSTed message
//! is journaled and broadcast to the resident's subscription. Discovery is a
//! dumb pointer file, not a transport: `wave/<name>/.wave-endpoint` holds
//! `127.0.0.1:<port>` and nothing else; `.wave-resident-token` beside it holds
//! this boot's resident token (see [`crate::wave::wire`]).
//!
//! This module is VENDOR-FREE: the loop lives in the resident process
//! ([`crate::wave::resident`]), which publishes through the resident door
//! (`/resident/attach`, `/resident/deltas`, `/resident/context` — token-gated)
//! and listens on its own wave's `/events?inbox=true` subscription. The
//! listener holds every pen; the resident holds the vendor.
//!
//! The server serves the wave's CHANNEL FAMILY (see [`crate::wave::channel`]):
//! the primary channel is the wave's name and the only journaled one — the
//! served mind. Work-line channels (`goals.148e0e02`) are live bus topics: no
//! journal, no worktree binding. Doors are name-addressed.
//!
//! Wire contract (snake_case, stable — a Loopflow worker builds against it):
//! - `GET /health` → `{status, loop_state, wave, turns, paused, uptime_seconds}`;
//!   `status` is CHANNEL liveness — always `serving` while this process
//!   answers; `loop_state` is the resident's state (`idle | turning | interrupting
//!   | failed`), or null before any resident has attached; a served channel whose resident died reads
//!   `status: "serving", loop_state: "failed"`.
//! - `GET /conversation` → `{turns: [Turn]}`; includes the open turn (status
//!   `running`), if one is in progress, after the finalized thread. Optional
//!   `?limit=N` tails the last N turns (open turn included) — `wave_context`
//!   passes 12; absent means the whole thread. Primary channel only.
//! - `GET /events` → SSE, the served mind's thread. It carries the PRIMARY
//!   channel and nothing else: agent-to-agent traffic is the bus, a table in
//!   the shared store that no server sits in front of (`crate::wave::bus`).
//!   Event names:
//!   - `state`: data is the loop-state name (`idle | turning | interrupting |
//!     failed`), sent once on subscribe (before the turn replay) and again on
//!     every transition — the composer keys its verb off it.
//!   - `turn`: data is a `Turn` JSON; the thread replays on connect (including
//!     the open turn), then streams live. Turn ids repeat: an in-progress turn
//!     is re-sent whole as it grows and finalization sends the terminal turn
//!     under the same id — each frame replaces the client's previous state
//!     for that id (upsert, never append-if-seen).
//!   - `memory`: data is the `MemoryUpdated` summary string, fired on every
//!     curation. Live-only, no replay — MEMORY.md itself is the durable
//!     state. Primary channel only (memory is wave identity; work lines have
//!     none).
//!   - `memory-add`: data is the full added fact. Replays on connect for the
//!     facts since the last curation, then streams live. Primary channel only.
//!   - `inbox` (only with `?inbox=true`, the resident's subscription): data
//!     is an [`InboxFrame`] — a resident-directed message, typed Task
//!     observation, or control op. The pending queue (journaled inputs not yet named in any `answers`) replays on
//!     connect, then live ops stream; a bare interrupt rides live-only with
//!     `id: null` (nothing journaled). Primary channel only. The default
//!     stream is byte-identical to the pre-resident wire.
//! - The resident door (token-gated via the `x-lf-resident-token` header —
//!   401 without this boot's token):
//!   - `POST /resident/attach {pid}` → `{wave}` — registers the
//!     resident's pid for liveness and revives a `failed` loop (a fresh
//!     resident IS the revival).
//!   - `POST /resident/deltas {deltas: [...]}` → `{accepted}` — ordered turn
//!     deltas, applied to the journal fold
//!     ([`WaveRuntime::apply_resident_delta`]).
//!   - `GET /resident/context` → `{playhead, provider_session}` — the
//!     pre-turn snapshot and optional typed provider thread; serving it drains
//!     pending child observations first.
//! - `POST /messages {op, text}` → `{turn, state}`. `op` is
//!   required — `"message"` (queued; the next turn answers it), `"steer"`
//!   (into the live turn when the harness supports it, else degrades to a
//!   queued message), `"interrupt"` (cancel the open turn; non-empty text
//!   becomes the next turn — "interrupt & send"; while idle, an interrupt is
//!   a no-op success). `text` may be empty only for `interrupt` (400
//!   otherwise). The thread is human and unattributed: `say` and `from` are
//!   rejected. Machine speech uses `lf radio pub`, an INSERT on the bus that this
//!   server later reads back. `turn` is the appended user `Turn`,
//!   or null for a bare interrupt (nothing was said); `state` is the
//!   loop-state name when the request was accepted — ops are applied by the
//!   loop asynchronously, so watch the stream's `state` events for the
//!   outcome.
//! - `POST /observations` drains the Wave's authoritative child-observation
//!   outbox, journals each pending Project/Task event idempotently, and wakes
//!   or queues the resident with typed inbox items. Loopback-only internal
//!   door; child sessions never write the Wave journal.
//! - `POST /stop` → 202 and requests graceful listener shutdown. The listener
//!   remains the sole owner of resident, registry, and discovery-file cleanup.
//! - `GET /memory` → `{content}` — the wave's MEMORY.md, read from the
//!   origin repo. Wave-level only: memory is wave identity, channels don't
//!   have it.
//! - `GET /memory/log` → `{facts}` — add-stream facts since the last
//!   curation, oldest first. Wave-level only.
//! - `POST /memory {op, content, summary}` → `{summary}`. `op` is `"update"`
//!   (full replacement) or `"add"` (publish one fact; `content` must be
//!   non-empty). `summary` is explicitly Optional — null falls back to the
//!   content's first non-empty line. The server is the sole writer of the
//!   origin repo's `wave/<name>/MEMORY.md` and journals `MemoryUpdated`;
//!   add-only facts journal `MemoryAdded` and broadcast `memory-add`.
//!
//! `Turn` is [`crate::chat::turns::ChatTurn`].

use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use axum::extract::{DefaultBodyLimit, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::stream::{self, Stream, StreamExt};
use secrecy::{ExposeSecret, SecretString};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;
use time::OffsetDateTime;
use tokio::sync::{broadcast, watch};
use tokio_stream::wrappers::BroadcastStream;

use crate::chat::turns::ChatTurn;
use crate::wave::journal::{MessageOp, PendingMessage};
use crate::wave::playhead::PlayheadView;
use crate::wave::registry::{process_alive, StoreObserver};
use crate::wave::runtime::{InboxItem, WaveRuntime};
use crate::wave::state::LoopState;
use crate::wave::supervisor::SupervisorHandle;
use crate::wave::wire::{
    AttachRequest, AttachResponse, ContextResponse, InboxFrame, PostDeltasRequest,
    PostDeltasResponse, RESIDENT_TOKEN_FILE, RESIDENT_TOKEN_HEADER,
};

/// Basename of the discovery pointer under `wave/<name>/`.
pub const ENDPOINT_FILE: &str = ".wave-endpoint";

/// The resident door's server-side state: this boot's token and the seat —
/// the attached resident's pid, for liveness probing. Shared with the
/// supervisor ([`crate::wave::supervisor`]), which probes attached pids and
/// clears the seat when the resident dies.
///
/// The token is held as a [`SecretString`] and compared in constant time
/// ([`subtle::ConstantTimeEq`]) — never `==`, never surfaced in `Debug` or a
/// log.
#[derive(Debug, Clone)]
pub struct ResidentDoor {
    token: SecretString,
    seat: Arc<Mutex<Option<u32>>>,
}

impl ResidentDoor {
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: SecretString::new(token.into()),
            seat: Arc::new(Mutex::new(None)),
        }
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
        if token_matches(&self.token, presented) {
            return Ok(());
        }
        Err((
            StatusCode::UNAUTHORIZED,
            format!("missing or wrong {RESIDENT_TOKEN_HEADER}"),
        ))
    }
}

/// Constant-time compare of a presented token against a stored secret — the
/// door's only equality check. Length inequality short-circuits; equal-length
/// inputs compare in constant time.
fn token_matches(expected: &SecretString, provided: &str) -> bool {
    expected
        .expose_secret()
        .as_bytes()
        .ct_eq(provided.as_bytes())
        .into()
}

/// A fresh per-boot resident token.
pub fn generate_resident_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// One-shot lifecycle door for `POST /stop`. The listener owns the receiver;
/// the HTTP surface only requests shutdown, leaving cleanup to `run_listener`.
#[derive(Debug, Clone)]
pub struct ShutdownDoor {
    requested: watch::Sender<bool>,
}

impl ShutdownDoor {
    pub fn new() -> Self {
        let (requested, _) = watch::channel(false);
        Self { requested }
    }

    fn request(&self) {
        self.requested.send_replace(true);
    }

    pub async fn wait(&self) {
        let mut receiver = self.requested.subscribe();
        if *receiver.borrow() {
            return;
        }
        let _ = receiver.changed().await;
    }
}

impl Default for ShutdownDoor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Serialize)]
struct HealthBody {
    /// Channel liveness: always `"serving"` while this process answers. The
    /// resident's condition is `loop_state` — a served channel whose resident
    /// died is `status: "serving", loop_state: "failed"`.
    status: String,
    /// Resident loop state name, or null for a channel with no resident
    /// (before any resident attaches).
    loop_state: Option<String>,
    wave: String,
    turns: usize,
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
/// (no serde default; an op-less body is a 422). `from` is accepted only so a
/// byline can be rejected with a 400 that names the bus, rather than silently
/// dropped as an unknown field.
#[derive(Debug, Deserialize)]
struct PostMessage {
    op: MessageOp,
    text: String,
    from: Option<String>,
}

/// `GET /events` query. `inbox` is explicitly Optional: `true` adds the
/// resident's `inbox` frames (pending replay + live ops) to the subscription;
/// absent/false leaves the wire byte-identical to the pre-resident stream.
#[derive(Debug, Deserialize)]
struct EventsQuery {
    inbox: Option<bool>,
}

/// `GET /memory` response.
#[derive(Debug, Serialize)]
struct MemoryBody {
    content: String,
}

/// `GET /memory/log` response.
#[derive(Debug, Serialize)]
struct MemoryLogBody {
    facts: Vec<String>,
}

/// `POST /memory` op — full replacement or one published fact.
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
/// bare interrupt, which appends nothing. `state` is the loop-state name at
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
    shutdown: ShutdownDoor,
    started_at: OffsetDateTime,
}

/// Build the router over a running [`WaveRuntime`]. `observer` is the store
/// poller when this server is registered — `GET /resident/context` freshens
/// it before serving. `supervisor` lets the attach door stand the respawn
/// ladder down (`None` in tests without a supervisor).
/// Request-body ceiling for the wave routes — parity with the machine lfd's
/// `http_security.max_json_body_bytes` default (1 MiB). Loopback + token gate
/// this, but an unbounded body is a needless same-user allocation.
const MAX_BODY_BYTES: usize = 1_048_576;

pub fn router(
    runtime: Arc<WaveRuntime>,
    resident: ResidentDoor,
    observer: Option<Arc<StoreObserver>>,
    supervisor: Option<SupervisorHandle>,
    shutdown: ShutdownDoor,
) -> Router {
    let state = ServerState {
        runtime,
        resident,
        observer,
        supervisor,
        shutdown,
        started_at: OffsetDateTime::now_utc(),
    };
    Router::new()
        .route("/health", get(health_handler))
        .route("/stop", post(stop_handler))
        .route("/conversation", get(conversation_handler))
        .route("/playhead", get(playhead_handler))
        .route("/events", get(events_handler))
        .route("/messages", post(messages_handler))
        .route("/observations", post(observations_handler))
        .route("/memory", get(memory_handler).post(memory_write_handler))
        .route("/memory/log", get(memory_log_handler))
        .route("/resident/attach", post(resident_attach_handler))
        .route("/resident/deltas", post(resident_deltas_handler))
        .route("/resident/context", get(resident_context_handler))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}

async fn observations_handler(
    State(state): State<ServerState>,
) -> Result<StatusCode, (StatusCode, String)> {
    let observer = state.observer.as_ref().ok_or_else(|| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "child observations require the shared Loopflow registry".to_string(),
        )
    })?;
    observer.poll_once().await;
    Ok(StatusCode::NO_CONTENT)
}

async fn stop_handler(State(state): State<ServerState>) -> StatusCode {
    state.shutdown.request();
    StatusCode::ACCEPTED
}

async fn playhead_handler(
    State(state): State<ServerState>,
) -> Result<Json<PlayheadView>, (StatusCode, String)> {
    state
        .runtime
        .ensure_playhead()
        .map(Json)
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))
}

async fn health_handler(State(state): State<ServerState>) -> Json<HealthBody> {
    // `loop_state` is null until a resident has ever been spawned or attached —
    // A listener-only test channel has no Loop to report on.
    let loop_state = state
        .runtime
        .resident_expected()
        .then(|| state.runtime.loop_state().name().to_string());
    Json(HealthBody {
        status: "serving".to_string(),
        loop_state,
        wave: state.runtime.name().to_string(),
        turns: state.runtime.thread_len(),
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
    // Seat exclusivity: one loop per wave. A live seat already probed alive
    // refuses the attach naming it — a second resident would split-brain the
    // wire. A dead/absent seat is free (takeover after a crash rides the same
    // door; the supervisor's own seat probe frees a dead pid on its cadence).
    // `--force` is `lf serve`'s boot flag, not the door's business.
    if let Some(seated) = state.resident.seat_pid() {
        if seated != body.pid && process_alive(seated).await {
            return Err((
                StatusCode::CONFLICT,
                format!(
                    "wave '{}' already has a live resident on the seat (pid {seated}); \
                     stop it before attaching, or use `lf serve <name> --force` to take over",
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
    // A fresh resident IS the revival: a failed loop goes idle on attach.
    if matches!(state.runtime.loop_state(), LoopState::Failed { .. }) {
        state
            .runtime
            .transition(LoopState::Idle, "resident attached");
    }
    state
        .runtime
        .ensure_playhead()
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    tracing::info!(pid = body.pid, "resident attached");
    Ok(Json(AttachResponse {
        wave: state.runtime.name().to_string(),
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
    // Drain child observations before the resident captures its next turn.
    if let Some(observer) = &state.observer {
        observer.poll_once().await;
    }
    let playhead = state
        .runtime
        .ensure_playhead()
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(Json(ContextResponse {
        playhead,
        provider_session: state.runtime.latest_provider_session(),
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
    // The thread door is the human's: unattributed message/steer/interrupt.
    // `say` is the journal's vocabulary for folded bus reports — nothing
    // posts it; agents publish with `lf radio pub` and the listener's bus sweep
    // records the attributed copy. Rejecting both here is what makes "agents
    // don't use chat" a wire property instead of doctrine.
    if matches!(body.op, MessageOp::Say) {
        return Err((
            StatusCode::BAD_REQUEST,
            "`say` is not a wire op: machine speech rides the bus (`lf radio pub`)".to_string(),
        ));
    }
    if body.from.is_some() {
        return Err((
            StatusCode::BAD_REQUEST,
            "the thread is unattributed: bylines belong to the bus (`lf radio pub --from`)"
                .to_string(),
        ));
    }
    if body.text.trim().is_empty() && !matches!(body.op, MessageOp::Interrupt) {
        return Err((
            StatusCode::BAD_REQUEST,
            "text is required for every op but interrupt".to_string(),
        ));
    }
    let turn = state.runtime.deliver(body.op, body.text);
    Ok(Json(PostMessageResponse {
        turn,
        state: state.runtime.loop_state().name().to_string(),
    }))
}

async fn memory_handler(State(state): State<ServerState>) -> Json<MemoryBody> {
    Json(MemoryBody {
        content: state.runtime.memory().read(),
    })
}

async fn memory_log_handler(State(state): State<ServerState>) -> Json<MemoryLogBody> {
    Json(MemoryLogBody {
        facts: state.runtime.memory_adds(),
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
            state.runtime.append_memory(fact)
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

/// The served mind's thread as SSE: the loop state, the thread on connect
/// (open turn included, status `running`), then live frames — `state` on every
/// transition, `turn` ids repeating by design (every frame replaces the
/// client's state for that id, so an in-progress turn updates in place and its
/// terminal frame lands under the same id), `memory` on every curation
/// (live-only; the file is the durable state), and `memory-add` for replayable
/// facts. Snapshot and subscription are atomic in the runtime (broadcasts
/// share the append lock), so no live frame is ever older than the replayed
/// snapshot.
///
/// There is no channel scoping. Agent-to-agent broadcast is the bus — a table,
/// polled from a cursor, with no server in the path (`crate::wave::bus`).
async fn events_handler(
    State(state): State<ServerState>,
    Query(query): Query<EventsQuery>,
) -> axum::response::Response {
    let include_inbox = query.inbox == Some(true);
    let sub = state.runtime.subscribe_with_snapshot();
    // The resident's subscription replays the pending queue after the
    // thread — its boot inbox. Consumption is validated at the resident
    // door, so a stale replay can never double-consume.
    let inbox_replay: Vec<Result<Event, Infallible>> = if include_inbox {
        sub.pending
            .iter()
            .map(|message| {
                let frame = if let Some(observation) = sub.tasks.get(&message.id) {
                    InboxFrame::Task {
                        observation: observation.clone(),
                    }
                } else if let Some(observation) = sub.projects.get(&message.id) {
                    InboxFrame::Project {
                        observation: observation.clone(),
                    }
                } else {
                    pending_inbox_frame(message)
                };
                Ok(inbox_event(&frame))
            })
            .collect()
    } else {
        Vec::new()
    };
    let replay = stream::iter(
        std::iter::once(Ok(state_event(&sub.state)))
            .chain(sub.playhead.into_iter().map(|p| Ok(playhead_event(&p))))
            .chain(sub.turns.into_iter().map(|t| Ok(turn_event(&t))))
            .chain(
                sub.memory_adds
                    .into_iter()
                    .map(|fact| Ok(memory_add_event(&fact))),
            )
            .chain(inbox_replay),
    );
    // The frame's wire JSON was serialized once at the send site. Lagged:
    // the client fell behind; it resyncs from /conversation.
    let live_turns = live_stream(sub.turn_rx, |frame| {
        Event::default().event("turn").data(frame.json.as_str())
    });
    // Lagged: fine — the next transition carries the current state.
    let live_states = live_stream(sub.state_rx, |s| state_event(&s));
    let live_playhead = live_stream(sub.playhead_rx, |p| playhead_event(&p));
    // Lagged: reconnect gets a fresh add snapshot.
    let live_memory_adds = live_stream(sub.memory_add_rx, |fact| memory_add_event(&fact));
    // Lagged: fine — MEMORY.md itself is the durable state.
    let live_memory = live_stream(sub.memory_rx, |summary| memory_event(&summary));
    let mut live: BoxedEventStream = Box::pin(stream::select(
        live_turns,
        stream::select(
            stream::select(live_states, live_playhead),
            stream::select(live_memory, live_memory_adds),
        ),
    ));
    if include_inbox {
        // Lagged: the pending fold is the durable queue; a resident that
        // falls behind resubscribes.
        let live_inbox = live_stream(sub.inbox_rx, |item| inbox_event(&inbox_item_frame(&item)));
        live = Box::pin(stream::select(live, live_inbox));
    }
    let merged: BoxedEventStream = Box::pin(replay.chain(live));
    axum::response::IntoResponse::into_response(Sse::new(merged).keep_alive(KeepAlive::default()))
}

type BoxedEventStream =
    std::pin::Pin<Box<dyn Stream<Item = Result<Event, Infallible>> + Send + 'static>>;

/// One live SSE stream off a broadcast receiver: each value becomes an event,
/// a lagged receiver drops silently (every stream's durable state resyncs on
/// reconnect — see each call site for what backs it).
fn live_stream<T, F>(
    rx: broadcast::Receiver<T>,
    to_event: F,
) -> impl Stream<Item = Result<Event, Infallible>> + Send + 'static
where
    T: Clone + Send + 'static,
    F: Fn(T) -> Event + Send + 'static,
{
    BroadcastStream::new(rx).filter_map(move |res| {
        let out = res.ok().map(|value| Ok(to_event(value)));
        async move { out }
    })
}

fn turn_event(turn: &ChatTurn) -> Event {
    Event::default()
        .event("turn")
        .data(serde_json::to_string(turn).expect("ChatTurn serializes to JSON"))
}

fn playhead_event(playhead: &PlayheadView) -> Event {
    Event::default()
        .event("playhead")
        .data(serde_json::to_string(playhead).expect("PlayheadView serializes to JSON"))
}

fn state_event(state: &LoopState) -> Event {
    Event::default().event("state").data(state.name())
}

fn memory_event(summary: &str) -> Event {
    Event::default().event("memory").data(summary)
}

fn memory_add_event(fact: &str) -> Event {
    Event::default().event("memory-add").data(fact)
}

fn inbox_event(frame: &InboxFrame) -> Event {
    Event::default()
        .event("inbox")
        .data(serde_json::to_string(frame).expect("InboxFrame serializes to JSON"))
}

fn pending_inbox_frame(message: &PendingMessage) -> InboxFrame {
    InboxFrame::Message {
        id: message.id.0.clone(),
        op: message.op,
        text: message.text.clone(),
        from: message.from.clone(),
    }
}

fn inbox_item_frame(item: &InboxItem) -> InboxFrame {
    match item {
        InboxItem::Message(message) => pending_inbox_frame(message),
        InboxItem::Task(observation) => InboxFrame::Task {
            observation: observation.clone(),
        },
        InboxItem::Project(observation) => InboxFrame::Project {
            observation: observation.clone(),
        },
        InboxItem::Interrupt => InboxFrame::Interrupt,
        InboxItem::Skip => InboxFrame::Skip,
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

/// Publish the loopback endpoint so Loopflow can find the server. Writes ONLY
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

/// Publish this boot's resident token so the internal resident can present it
/// — the same filesystem-trust domain as
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

/// Read the current resident token for attachment.
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

    #[tokio::test]
    async fn stop_route_requests_listener_shutdown() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let runtime = WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open");
        let shutdown = ShutdownDoor::new();
        let requested = shutdown.clone();
        let app = router(runtime, ResidentDoor::new("resident"), None, None, shutdown);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        let response = reqwest::Client::new()
            .post(format!("http://{addr}/stop"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::ACCEPTED);
        requested.wait().await;
        server.abort();
    }

    #[tokio::test]
    async fn observation_nudge_fails_loudly_without_the_shared_registry() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let runtime = WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open");
        let app = router(
            runtime,
            ResidentDoor::new("resident"),
            None,
            None,
            ShutdownDoor::new(),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        let response = reqwest::Client::new()
            .post(format!("http://{addr}/observations"))
            .json(&serde_json::json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE);
        server.abort();
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
