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
//! the primary channel is the wave's name; work-line channels are addressed
//! by their ownership names (`goals.148e0e02`). It holds the pen for every
//! child journal; doors are name-addressed.
//!
//! Wire contract (snake_case, stable — a Loopflow worker builds against it):
//! - `GET /health` → `{status, loop_state, wave, turns, workers, uptime_seconds}`;
//!   `status` is CHANNEL liveness — always `serving` while this process
//!   answers; `loop_state` is the resident's state (`idle | turning | interrupting
//!   | failed`), or null before any resident has attached; a served channel whose resident died reads
//!   `status: "serving", loop_state: "failed"`. `workers` counts this wave's
//!   observed in-flight worker runs.
//! - `GET /conversation` → `{turns: [Turn]}`; includes the open turn (status
//!   `running`), if one is in progress, after the finalized thread. Optional
//!   `?limit=N` tails the last N turns (open turn included) — `wave_context`
//!   passes 12; absent means the whole thread. Primary channel only.
//! - `GET /events` → SSE, the family's one unified stream. Scope by query:
//!   `?channel=<name>` (exactly one channel), `?prefix=<name>` (that subtree),
//!   default = the whole family. A name outside this wave's family is a 404.
//!   Three event names:
//!   - `state`: data is the loop-state name (`idle | turning | interrupting |
//!     failed`), sent once on subscribe (before the turn replay) and again on
//!     every transition — the composer keys its verb off it. Primary channel
//!     only — child channels have no loop, so a child-only subscription
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
//!   - `op`: data is an [`OpFrame`] — this wave's operational motion (a worker
//!     run starting or finishing, observed by the [`StoreObserver`]), `kind`
//!     mirroring the `run_events` ledger vocabulary 1:1 (`run.started`,
//!     `run.completed`, `run.errored`). Live-only, no replay — history is a
//!     `lf runs` query, the durable ledger the frame mirrors. Primary channel
//!     only (workers are the wave's, not a child channel's).
//!   - `memory-add`: data is the full added fact. Replays on connect for the
//!     facts since the last curation, then streams live. Primary channel only.
//!   - `inbox` (only with `?inbox=true`, the resident's subscription): data
//!     is an [`InboxFrame`] — a resident-directed op. The pending queue
//!     (journaled messages not yet named in any `answers`) replays on
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
//!   - `GET /resident/context` → `{in_flight}` — the pre-turn
//!     snapshot; serving it freshens the store observations (one poll).
//! - `POST /messages {op, text, from?, channel?}` → `{turn, state}`. `op` is
//!   required — `"message"` (human speech steers a live steer-capable turn;
//!   otherwise queued, the next turn answers it), `"steer"`
//!   (into the live turn when the harness supports it, else degrades to a
//!   queued message), `"interrupt"` (cancel the open turn; non-empty text
//!   becomes the next turn — "interrupt & send"; while idle, an interrupt is
//!   a no-op success), or `"say"` (an attributed emission — `lf chat`: a
//!   worker report, child-wave escalation, or CLI FYI; lands in the thread
//!   with its byline AND queues for the loop like a message). `text` may be
//!   empty only for `interrupt` (400 otherwise). `from {session_id?, label}`
//!   is required for `say` and rejected for every other op (400) — human
//!   turns are unattributed by convention. `channel` is explicitly Optional:
//!   null targets the wave channel (unchanged); a child name lands the
//!   message in THAT channel's journal (404 outside the family or when the
//!   work line's worktree is gone). On a child channel there is no resident:
//!   steer degrades to a plain message, a bare interrupt is a no-op. `turn`
//!   is the appended user `Turn`, or null for a bare interrupt (nothing was
//!   said); `state` is the loop-state name when the request was accepted —
//!   ops are applied by the loop asynchronously, so watch the stream's
//!   `state` events for the outcome.
//! - `POST /channels {name, run_id}` → `{turn}` — the dispatch notification
//!   door: placed `lf` minted a work-line worktree and its channel
//!   journal, and knocks here so the PARENT channel's thread shows
//!   "work line <name> opened" (journaled as `ChannelOpened`, idempotent on
//!   `run_id` — a repeated knock returns `{turn: null}`). 404 outside the
//!   family.
//! - `POST /loops {flow, seed, caps…}` → `{session}` — capability-gated
//!   detached loop launch. The listener starts a headless `lf loop` in a
//!   named tmux session; callers may inspect it with `tmux attach -r`, never
//!   write through its stdin. The blocking form never crosses this door.
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
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use crate::chat::turns::ChatTurn;
use crate::lfd::executor::helpers::{resolve_lf_binary, spawn_detached_lf, tmux_session_slug};
use crate::lfd::http::routes::exec::{ExecRequest, ExecResponse};
use crate::lfd::lf_exec::{exec_lf, validate_lf_argv};
use crate::wave::channel::tagged_turn_json;
use crate::wave::journal::{Attribution, MessageOp, PendingMessage};
use crate::wave::playhead::PlayheadView;
use crate::wave::registry::{process_alive, StoreObserver};
use crate::wave::runtime::{InboxItem, WaveRuntime};
use crate::wave::state::LoopState;
use crate::wave::supervisor::SupervisorHandle;
use crate::wave::wire::{
    AttachRequest, AttachResponse, ContextResponse, DetachedLoopRequest, DetachedLoopResponse,
    InFlightWorker, InboxFrame, OpFrame, PostDeltasRequest, PostDeltasResponse,
    RESIDENT_TOKEN_FILE, RESIDENT_TOKEN_HEADER, SUBAGENT_TOKEN_HEADER,
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
/// log. This mirrors [`crate::lfd::auth`], the machine lfd's bearer door.
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
/// door's only equality check. Length inequality short-circuits (inherent, as
/// in [`crate::lfd::auth`]); equal-length inputs compare in constant time.
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

/// The exec door's authority: the set of per-subagent capability tokens this
/// boot accepts on `/v0/exec`. A distinct principal from [`ResidentDoor`] —
/// `/exec` accepts a minted subagent token and never the resident token, so a
/// least-privilege subagent (a sandboxed process spawned inside the wave) can
/// run `lf` unsandboxed in the outwave without holding the resident's pen.
///
/// In-memory, per boot — no store, no schema. The listener mints a token when
/// it spawns the resident (injected into the child env, inherited by every
/// sandboxed descendant) and validates presented tokens against this set. A
/// respawn reuses the boot's token, the same trust domain as the resident
/// token, which is also per-boot.
///
/// Tokens are held as [`SecretString`]s (redacted in `Debug`, never logged)
/// and membership is a constant-time scan ([`subtle::ConstantTimeEq`]): every
/// accepted token is compared, results folded without an early return, so a
/// presented token leaks neither its value nor its position in the set.
#[derive(Debug, Clone, Default)]
pub struct SubagentDoor {
    accepted: Arc<Mutex<Vec<SecretString>>>,
}

impl SubagentDoor {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mint a fresh capability token and register it as accepted.
    pub fn mint(&self) -> String {
        let token = generate_resident_token();
        self.accepted
            .lock()
            .expect("subagent token set lock poisoned")
            .push(SecretString::new(token.clone()));
        token
    }

    fn authorize(&self, headers: &HeaderMap) -> Result<(), (StatusCode, String)> {
        let presented = headers
            .get(SUBAGENT_TOKEN_HEADER)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        if !presented.is_empty() && self.accepts(presented) {
            return Ok(());
        }
        Err((
            StatusCode::UNAUTHORIZED,
            format!("missing or wrong {SUBAGENT_TOKEN_HEADER}"),
        ))
    }

    /// Constant-time set membership: compare the presented token against every
    /// accepted token, folding matches with a non-short-circuiting bit-or so
    /// timing reveals neither which token matched nor whether an early one did.
    fn accepts(&self, presented: &str) -> bool {
        let accepted = self
            .accepted
            .lock()
            .expect("subagent token set lock poisoned");
        let presented = presented.as_bytes();
        let mut matched = subtle::Choice::from(0u8);
        for token in accepted.iter() {
            matched |= token.expose_secret().as_bytes().ct_eq(presented);
        }
        matched.into()
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
    /// Workers observed in flight for this wave (dispatch is daemonless —
    /// placed `lf` — so the store fold, not a task registry, is truth).
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

#[derive(Debug, Deserialize)]
struct EnqueueFlowRequest {
    flow: String,
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
    subagent: SubagentDoor,
    observer: Option<Arc<StoreObserver>>,
    supervisor: Option<SupervisorHandle>,
    started_at: OffsetDateTime,
}

/// Start the blocking `lf loop` the request names, detached in its own tmux
/// session. Tmux keeps the child inspectable without granting stdin
/// (`tmux attach -r`); the loop's own run row is its durable supervision view.
async fn launch_detached_loop(
    repo_root: &Path,
    wave: &str,
    request: &DetachedLoopRequest,
) -> Result<String, String> {
    let session = detached_loop_session_name(wave);
    let argv = detached_loop_argv(&resolve_lf_binary(), request, wave);
    spawn_detached_lf(&session, repo_root, &argv)
        .await
        .map_err(|err| format!("failed to launch detached loop: {err}"))?;
    tracing::info!(session, wave, flow = request.flow, "detached loop launched");
    Ok(session)
}

fn detached_loop_session_name(wave: &str) -> String {
    let run = uuid::Uuid::new_v4().simple().to_string();
    format!("lf-loop-{}-{}", tmux_session_slug(wave), &run[..8])
}

fn detached_loop_argv(executable: &Path, request: &DetachedLoopRequest, wave: &str) -> Vec<String> {
    let mut argv = vec![
        executable.display().to_string(),
        "loop".to_string(),
        request.flow.clone(),
        request.seed.clone(),
        "--wave".to_string(),
        wave.to_string(),
        "--max-passes".to_string(),
        request.max_passes.to_string(),
        "--pass-timeout-secs".to_string(),
        request.pass_timeout_secs.to_string(),
        "--wall-clock-secs".to_string(),
        request.wall_clock_secs.to_string(),
        "--poll-secs".to_string(),
        request.poll_secs.to_string(),
    ];
    if let Some(max_turns) = request.max_turns {
        argv.push("--max-turns".to_string());
        argv.push(max_turns.to_string());
    }
    argv
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
    subagent: SubagentDoor,
    observer: Option<Arc<StoreObserver>>,
    supervisor: Option<SupervisorHandle>,
) -> Router {
    let state = ServerState {
        runtime,
        resident,
        subagent,
        observer,
        supervisor,
        started_at: OffsetDateTime::now_utc(),
    };
    Router::new()
        .route("/health", get(health_handler))
        .route("/conversation", get(conversation_handler))
        .route("/playhead", get(playhead_handler))
        .route("/playhead/enqueue", post(playhead_enqueue_handler))
        .route("/playhead/skip", post(playhead_skip_handler))
        .route("/events", get(events_handler))
        .route("/messages", post(messages_handler))
        .route("/channels", post(channels_handler))
        .route("/loops", post(loops_handler))
        .route("/memory", get(memory_handler).post(memory_write_handler))
        .route("/v0/exec", post(exec_handler))
        .route("/memory/log", get(memory_log_handler))
        .route("/resident/attach", post(resident_attach_handler))
        .route("/resident/deltas", post(resident_deltas_handler))
        .route("/resident/context", get(resident_context_handler))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
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

async fn playhead_enqueue_handler(
    State(state): State<ServerState>,
    Json(request): Json<EnqueueFlowRequest>,
) -> Result<Json<PlayheadView>, (StatusCode, String)> {
    let flow = request.flow.trim();
    if flow.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "flow is required".to_string()));
    }
    state
        .runtime
        .enqueue_flow(flow)
        .map(Json)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))
}

async fn playhead_skip_handler(
    State(state): State<ServerState>,
) -> Result<Json<PlayheadView>, (StatusCode, String)> {
    let current = state
        .runtime
        .ensure_playhead()
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    if current.active.is_some() {
        state.runtime.deliver_skip();
        Ok(Json(current))
    } else {
        state
            .runtime
            .skip_current("skipped by user")
            .map(Json)
            .map_err(|err| (StatusCode::CONFLICT, err.to_string()))
    }
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
    // Seat exclusivity: one loop per wave. A live seat already probed alive
    // refuses the attach naming it — a second resident would split-brain the
    // wire. A dead/absent seat is free (takeover after a crash rides the same
    // door; the supervisor's own seat probe frees a dead pid on its cadence).
    // `--force` is `lf loop`'s boot flag, not the door's business.
    if let Some(seated) = state.resident.seat_pid() {
        if seated != body.pid && process_alive(seated).await {
            return Err((
                StatusCode::CONFLICT,
                format!(
                    "wave '{}' already has a live resident on the seat (pid {seated}); \
                     stop it before attaching, or use `lf loop <name> --force` to take over",
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
    let playhead = state
        .runtime
        .ensure_playhead()
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;
    Ok(Json(ContextResponse {
        in_flight,
        playhead,
    }))
}

/// The verb policy's ruling on one argv: run it, or refuse it naming the verb.
#[derive(Debug, PartialEq, Eq)]
enum ExecVerdict {
    Allow,
    Deny(String),
}

/// The wave `/v0/exec` door's verb allowlist — the F1 containment fix.
///
/// The door is the sandboxed subagent's escape hatch: it exists so a worker
/// can COMMIT and DELEGATE in the outwave despite its own worktree's
/// `.git`-write lock. It is NOT a general remote `lf`. `validate_lf_argv`
/// only proves an argv *parses*, so without this a leaked subagent token (or
/// a prompt-injected LLM holding it) could run ANY verb — rotate credentials,
/// tear down a wave. Allowlist over denylist: permit exactly the escape
/// hatch's needs, refuse everything else.
///
/// Permitted:
/// - Git/GitHub/pm/release/queue commands EXCEPT `auth` — the commit-and-land path.
/// - `chat`, `memory` — a worker reporting up and curating wave memory.
/// - the read verbs `ls`/`status`/`runs`/`sub`/`trace`/`usage` — inspection.
/// - `loop … --detach`, whose only execution path is a server-owned fresh
///   worktree.
///
/// Rejected:
/// - `auth` — credential rotation is never the escape hatch's job.
/// - `loop <wave>` — wave lifecycle (start / `--force` take-over).
/// - blocking `loop …` — the door process would itself become the long-lived
///   owner, bypassing the listener's supervision.
/// - every direct flow / skill / inline prompt — those would run an arbitrary
///   LLM prompt unsandboxed in the outwave, the exact power this door must not
///   hand a leaked token.
fn wave_exec_verdict(argv: &[String]) -> ExecVerdict {
    use crate::lf::{Cli, Commands};
    use clap::Parser;

    let full = std::iter::once("lf".to_string()).chain(argv.iter().cloned());
    // `validate_lf_argv` runs first and already rejected anything that does
    // not parse — save help/version, which it lets through to print. So a
    // parse error here is that harmless help/version case: nothing to police.
    let Ok(cli) = Cli::try_parse_from(full) else {
        return ExecVerdict::Allow;
    };
    match &cli.command {
        Some(
            Commands::Pr { .. }
            | Commands::Wt { .. }
            | Commands::Rebase { .. }
            | Commands::Commit { .. }
            | Commands::Release { .. }
            | Commands::Pm { .. },
        ) => ExecVerdict::Allow,
        Some(Commands::Auth { .. }) => ExecVerdict::Deny("auth".to_string()),
        Some(Commands::Chat { .. })
        | Some(Commands::Memory { .. })
        | Some(Commands::Ls { .. })
        | Some(Commands::Status { .. })
        | Some(Commands::Runs { .. })
        | Some(Commands::Sub { .. })
        | Some(Commands::Trace { .. })
        | Some(Commands::Usage) => ExecVerdict::Allow,
        Some(Commands::Loop {
            detach: true,
            seed: Some(_),
            ..
        }) => ExecVerdict::Allow,
        Some(Commands::Loop { detach: false, .. }) => ExecVerdict::Deny("loop".to_string()),
        Some(Commands::Loop {
            detach: true,
            seed: None,
            ..
        }) => ExecVerdict::Deny("loop".to_string()),
        Some(Commands::External(parts)) => {
            ExecVerdict::Deny(parts.first().cloned().unwrap_or_else(|| "flow".to_string()))
        }
        Some(Commands::Flow { name, .. }) | Some(Commands::Skill { name, .. }) => {
            ExecVerdict::Deny(name.clone())
        }
        Some(Commands::Inline { .. }) => ExecVerdict::Deny(":".to_string()),
        Some(Commands::Enqueue { .. }) => ExecVerdict::Deny("enqueue".to_string()),
        Some(Commands::Skip) => ExecVerdict::Deny("skip".to_string()),
        Some(Commands::FlowStep { .. }) => ExecVerdict::Deny("__flow-step".to_string()),
        Some(Commands::Project { .. }) => ExecVerdict::Deny("project".to_string()),
        // `lf ssh` forwards the local credential bundle to a remote host and
        // runs an arbitrary command there — the exact power a leaked token
        // must not reach.
        Some(Commands::Ssh { .. }) => ExecVerdict::Deny("ssh".to_string()),
        // `lf cron` schedules recurring execution — a persistence/escalation
        // vector, not part of the commit/dispatch escape hatch.
        Some(Commands::Cron { .. }) => ExecVerdict::Deny("cron".to_string()),
        Some(Commands::SyncSkills { .. }) => ExecVerdict::Deny("sync-skills".to_string()),
        // Bare `lf` (interactive launch) has no verb the door can run.
        None => ExecVerdict::Deny("lf".to_string()),
    }
}

/// A subagent capability belongs to one wave. The exec door runs outside the
/// worker sandbox, where another wave's resident token file is readable, so an
/// explicit detached-loop target must stay pinned to this server's wave.
fn detached_loop_targets_other_wave(argv: &[String], wave: &str) -> bool {
    use crate::lf::{Cli, Commands};
    use clap::Parser;

    let full = std::iter::once("lf".to_string()).chain(argv.iter().cloned());
    let Ok(cli) = Cli::try_parse_from(full) else {
        return false;
    };
    matches!(cli.command, Some(Commands::Loop { detach: true, .. }))
        && cli.wave.as_deref().is_some_and(|target| target != wave)
}

/// `POST /v0/exec` — the wave's exec door: "a wave HAS an lfd" in one route.
/// A subagent (a sandboxed process spawned inside this wave) presents its
/// per-subagent token and runs an arbitrary `lf` argv **unsandboxed in the
/// outwave** (`runtime.repo_root()`), escaping the `.git`-write restriction of
/// its own worktree so it can commit / dispatch through the wave.
///
/// Two gates in front of the state-free [`crate::lfd::lf_exec`] engine: the
/// generic shape check ([`validate_lf_argv`] — garbage argv → 400, no exec),
/// then this door's own verb allowlist ([`wave_exec_verdict`] — a parsed but
/// forbidden verb like `auth` or `wave` → 400). Only then exec and
/// capture. The door pins execution to the outwave, so a client-supplied
/// `cwd` on the shared [`ExecRequest`] shape is ignored here — the machine
/// lfd's `/v0/exec` honors it; the wave's does not, by design.
async fn exec_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(payload): Json<ExecRequest>,
) -> Result<Json<ExecResponse>, (StatusCode, String)> {
    state.subagent.authorize(&headers)?;
    // Shape gate (generic engine): does the argv parse as an `lf` command?
    validate_lf_argv(&payload.argv).map_err(|err| (StatusCode::BAD_REQUEST, err))?;
    // Verb gate (this door's policy): is the command one the escape hatch is
    // allowed to run? A parsed-but-forbidden verb is a 400, not an exec.
    if let ExecVerdict::Deny(verb) = wave_exec_verdict(&payload.argv) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!("command '{verb}' is not permitted through the wave exec door"),
        ));
    }
    if detached_loop_targets_other_wave(&payload.argv, state.runtime.name()) {
        return Err((
            StatusCode::BAD_REQUEST,
            format!(
                "detached loops through this exec door must target wave '{}'",
                state.runtime.name()
            ),
        ));
    }
    let cwd = state.runtime.repo_root().display().to_string();
    let result = exec_lf(&payload.argv, Some(&cwd), &[])
        .await
        .map_err(|err| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                crate::lfd::redaction::sanitize_operator_message(&err),
            )
        })?;
    Ok(Json(ExecResponse {
        exit_code: result.exit_code,
        stdout: result.stdout,
        stderr: result.stderr,
    }))
}

/// `POST /loops` — launch one generic loop in a fresh worktree and return
/// immediately. Both resident and subagent credentials are accepted: a human
/// shell beside the wave reads the resident token file, while sandboxed hands
/// inherit only the narrower subagent capability. Either route can launch only
/// this worktree-forcing primitive.
async fn loops_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    Json(request): Json<DetachedLoopRequest>,
) -> Result<Json<DetachedLoopResponse>, (StatusCode, String)> {
    if state.subagent.authorize(&headers).is_err() && state.resident.authorize(&headers).is_err() {
        return Err((
            StatusCode::UNAUTHORIZED,
            "missing or wrong loop-launch credential".to_string(),
        ));
    }
    if request.flow.trim().is_empty() || request.seed.trim().is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            "flow and seed are required".to_string(),
        ));
    }
    if request.max_passes == 0
        || request.pass_timeout_secs == 0
        || request.wall_clock_secs == 0
        || request.poll_secs == 0
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "loop caps and poll interval must be positive".to_string(),
        ));
    }
    crate::flowloop::driver::require_loop_flow(state.runtime.repo_root(), &request.flow)
        .map_err(|err| (StatusCode::BAD_REQUEST, err.to_string()))?;
    let session = launch_detached_loop(state.runtime.repo_root(), state.runtime.name(), &request)
        .await
        .map_err(|err| (StatusCode::INTERNAL_SERVER_ERROR, err))?;
    Ok(Json(DetachedLoopResponse { session }))
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
        state: state.runtime.loop_state().name().to_string(),
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

/// The unified `/events` SSE, scoped to one channel, a subtree, or (default)
/// the whole family.
///
/// The primary channel's replay-then-live shape is unchanged: the loop state,
/// the thread on connect (open turn included, status `running`), then live
/// frames — `state` on every transition, `turn` ids repeating by design
/// (every frame replaces the client's state for that (channel, id), so an
/// in-progress turn updates in place and its terminal frame lands under the
/// same id), `memory` on every curation (live-only; the file is the durable
/// state), and `memory-add` for replayable facts. Snapshot and subscription
/// are atomic in the runtime (broadcasts share the append lock), so no primary
/// live frame is ever older than the replayed snapshot.
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
        // Worker-run motion (`op` frames). Live-only — no replay; a client
        // that lags re-reads history from `lf runs`.
        let live_ops = live_stream(sub.op_rx, |frame| op_event(&frame));
        // Lagged: fine — the next transition carries the current state.
        let live_states = live_stream(sub.state_rx, |s| state_event(&s));
        let live_playhead = live_stream(sub.playhead_rx, |p| playhead_event(&p));
        // Lagged: reconnect gets a fresh add snapshot.
        let live_memory_adds = live_stream(sub.memory_add_rx, |fact| memory_add_event(&fact));
        // Lagged: fine — MEMORY.md itself is the durable state.
        let live_memory = live_stream(sub.memory_rx, |summary| memory_event(&summary));
        let mut live: BoxedEventStream = Box::pin(stream::select(
            stream::select(live_turns, live_ops),
            stream::select(
                stream::select(live_states, live_playhead),
                stream::select(live_memory, live_memory_adds),
            ),
        ));
        if include_inbox {
            // Lagged: the pending fold is the durable queue; a resident that
            // falls behind resubscribes.
            let live_inbox =
                live_stream(sub.inbox_rx, |item| inbox_event(&inbox_item_frame(&item)));
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

fn playhead_event(playhead: &PlayheadView) -> Event {
    Event::default()
        .event("playhead")
        .data(serde_json::to_string(playhead).unwrap_or_default())
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

fn state_event(state: &LoopState) -> Event {
    Event::default().event("state").data(state.name())
}

fn memory_event(summary: &str) -> Event {
    Event::default().event("memory").data(summary)
}

fn op_event(frame: &OpFrame) -> Event {
    Event::default()
        .event("op")
        .data(serde_json::to_string(frame).unwrap_or_default())
}

fn memory_add_event(fact: &str) -> Event {
    Event::default().event("memory-add").data(fact)
}

fn inbox_event(frame: &InboxFrame) -> Event {
    Event::default()
        .event("inbox")
        .data(serde_json::to_string(frame).unwrap_or_default())
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

    /// Boot the HTTP surface over a runtime we control, with a subagent door
    /// we can mint from. Returns the base URL and the minted token.
    async fn boot_exec() -> (String, String, tempfile::TempDir) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&dir).expect("wave dir");
        std::fs::write(dir.join("MEMORY.md"), "Goal: exercise /exec.\n").expect("memory");
        let runtime = WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open");

        let subagent = SubagentDoor::new();
        let token = subagent.mint();
        let app = router(runtime, ResidentDoor::new("resident"), subagent, None, None);
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        (format!("http://{addr}"), token, tmp)
    }

    /// The wave's `/v0/exec` door: no token → 401, the resident token → 401,
    /// and a minted subagent token clears auth (garbage argv then 400s at the
    /// validator). Proves the exec door is a distinct principal from the
    /// resident door. (The valid-argv exec path is exercised by dogfooding,
    /// not here — a unit test must not spawn the real `lf` binary.)
    #[tokio::test]
    async fn exec_door_gates_on_the_subagent_token_and_validates_argv() {
        let (base, token, _tmp) = boot_exec().await;
        let client = reqwest::Client::new();
        let url = format!("{base}/v0/exec");

        // No token: refused before any exec.
        let no_token = client
            .post(&url)
            .json(&serde_json::json!({ "argv": ["pr", "status"], "cwd": null }))
            .send()
            .await
            .unwrap();
        assert_eq!(no_token.status(), reqwest::StatusCode::UNAUTHORIZED);

        // The resident token must NOT authorize a subagent exec call.
        let resident_token = client
            .post(&url)
            .header(RESIDENT_TOKEN_HEADER, "resident")
            .json(&serde_json::json!({ "argv": ["pr", "status"], "cwd": null }))
            .send()
            .await
            .unwrap();
        assert_eq!(resident_token.status(), reqwest::StatusCode::UNAUTHORIZED);

        // A minted token clears auth; a garbage argv proves we reached the
        // validator (400, not 401).
        let bad_argv = client
            .post(&url)
            .header(SUBAGENT_TOKEN_HEADER, &token)
            .json(&serde_json::json!({ "argv": ["pr", "land", "--nonesuch"], "cwd": null }))
            .send()
            .await
            .unwrap();
        assert_eq!(bad_argv.status(), reqwest::StatusCode::BAD_REQUEST);
    }

    /// A worker dispatched while a client is subscribed to `/events` surfaces
    /// as a live `op` frame carrying the ledger-vocabulary `kind`. This is the
    /// wave's operational channel riding the same stream as `state`/`turn`.
    #[tokio::test]
    async fn op_frame_reaches_the_events_stream_for_a_run() {
        use crate::wave::subscription::{stream_events, Frame};
        use std::sync::{Arc, Mutex};

        let tmp = tempfile::tempdir().expect("tempdir");
        let runtime = WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = router(
            runtime.clone(),
            ResidentDoor::new("resident"),
            SubagentDoor::new(),
            None,
            None,
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });

        let seen: Arc<Mutex<Vec<Frame>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = seen.clone();
        let endpoint = addr.to_string();
        let task = tokio::spawn(async move {
            let mut on_frame = |frame: Frame| sink.lock().unwrap().push(frame);
            let _ = stream_events(&endpoint, "", &mut on_frame).await;
        });

        // Wait for the subscription to open (the state replay lands first),
        // then a worker is observed — the live `op` frame must arrive.
        for _ in 0..200 {
            if seen.lock().unwrap().iter().any(|f| f.event == "state") {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert!(runtime.journal_run_observed("run-42", "sess-1", "implement", "wire it"));
        for _ in 0..200 {
            if seen.lock().unwrap().iter().any(|f| f.event == "op") {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        task.abort();

        let frames = seen.lock().unwrap().clone();
        let op = frames
            .iter()
            .find(|f| f.event == "op")
            .expect("op frame arrives on /events");
        let frame: OpFrame = serde_json::from_str(&op.data).expect("op frame parses");
        assert_eq!(frame.kind, "run.started");
        assert_eq!(frame.run_id, "run-42");
        assert_eq!(frame.flow.as_deref(), Some("implement"));
    }

    fn argv(parts: &[&str]) -> Vec<String> {
        parts.iter().map(ToString::to_string).collect()
    }

    /// The escape hatch's real work passes the verb policy: committing and
    /// landing through git/PR commands, reporting via `chat`/`memory`, inspecting via the
    /// read verbs, and delegating a loop into a sandboxed worktree.
    #[test]
    fn wave_exec_policy_permits_the_escape_hatch_essentials() {
        for command in [
            argv(&["commit", "-m", "wip"]),
            argv(&["pr", "land", "--strict"]),
            argv(&["pr", "open"]),
            argv(&["chat", "worker done"]),
            argv(&["memory", "add", "learned a thing"]),
            argv(&["ls"]),
            argv(&["status"]),
            argv(&["runs"]),
            argv(&["sub"]),
            argv(&["trace", "deadbeef"]),
            argv(&["loop", "task", "ship it", "--detach"]),
            argv(&["loop", "review", "audit the diff", "--detach"]),
        ] {
            assert_eq!(
                wave_exec_verdict(&command),
                ExecVerdict::Allow,
                "{command:?} should be permitted"
            );
        }
    }

    /// Credentials and wave lifecycle are refused, and a blocking loop or
    /// direct flow (which would execute an arbitrary prompt unsandboxed in the
    /// outwave) is refused — the F1 containment the door exists to enforce.
    #[test]
    fn wave_exec_policy_rejects_dangerous_verbs() {
        let denied = [
            argv(&["auth", "login"]),
            argv(&["auth", "status"]),
            argv(&["wave", "ship"]),
            argv(&["wave", "ship", "--force"]),
            argv(&["task", "ship it"]),
            argv(&["loop", "task", "ship it"]),
            argv(&["sync-skills", "--yes"]),
            argv(&["implement", "ship it"]),
            argv(&[":", "do", "something"]),
        ];
        for command in denied {
            assert!(
                matches!(wave_exec_verdict(&command), ExecVerdict::Deny(_)),
                "{command:?} should be rejected"
            );
        }
    }

    /// The refusal names the offending verb, for a clear 400 body.
    #[test]
    fn wave_exec_policy_names_the_rejected_verb() {
        assert_eq!(
            wave_exec_verdict(&argv(&["auth", "login"])),
            ExecVerdict::Deny("auth".to_string())
        );
        assert_eq!(
            wave_exec_verdict(&argv(&["wave", "ship", "--force"])),
            ExecVerdict::Deny("wave".to_string())
        );
        assert_eq!(
            wave_exec_verdict(&argv(&["loop", "task", "ship it"])),
            ExecVerdict::Deny("loop".to_string())
        );
        assert_eq!(
            wave_exec_verdict(&argv(&["sync-skills", "--yes"])),
            ExecVerdict::Deny("sync-skills".to_string())
        );
        assert_eq!(
            wave_exec_verdict(&argv(&["implement", "ship it"])),
            ExecVerdict::Deny("implement".to_string())
        );
    }

    #[test]
    fn detached_loop_argv_forces_the_server_owned_blocking_form() {
        let request = DetachedLoopRequest {
            flow: "task".into(),
            seed: "fix 'quoted' behavior".into(),
            max_passes: 8,
            pass_timeout_secs: 1800,
            wall_clock_secs: 7200,
            poll_secs: 60,
            max_turns: Some(20),
        };
        let argv = detached_loop_argv(Path::new("/opt/lf"), &request, "platform");
        assert_eq!(
            &argv[..4],
            ["/opt/lf", "loop", "task", "fix 'quoted' behavior"]
        );
        assert!(argv.windows(2).any(|pair| pair == ["--wave", "platform"]));
        assert!(!argv.iter().any(|arg| arg == "--detach"));
    }

    #[tokio::test]
    async fn loop_door_requires_capability_before_validating_or_launching() {
        let (base, token, _tmp) = boot_exec().await;
        let request = DetachedLoopRequest {
            flow: String::new(),
            seed: "ship it".into(),
            max_passes: 8,
            pass_timeout_secs: 1800,
            wall_clock_secs: 7200,
            poll_secs: 60,
            max_turns: None,
        };
        let client = reqwest::Client::new();
        let unauthorized = client
            .post(format!("{base}/loops"))
            .json(&request)
            .send()
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);

        let invalid = client
            .post(format!("{base}/loops"))
            .header(SUBAGENT_TOKEN_HEADER, token)
            .json(&request)
            .send()
            .await
            .unwrap();
        assert_eq!(invalid.status(), reqwest::StatusCode::BAD_REQUEST);
    }

    /// A minted subagent token authorizes but a forbidden verb still 400s over
    /// HTTP — the policy runs inside the live door, not just in isolation.
    #[tokio::test]
    async fn exec_door_refuses_forbidden_verb_over_http() {
        let (base, token, _tmp) = boot_exec().await;
        let client = reqwest::Client::new();
        let url = format!("{base}/v0/exec");

        let refused = client
            .post(&url)
            .header(SUBAGENT_TOKEN_HEADER, &token)
            .json(&serde_json::json!({ "argv": ["auth", "status"], "cwd": null }))
            .send()
            .await
            .unwrap();
        assert_eq!(refused.status(), reqwest::StatusCode::BAD_REQUEST);
        let body = refused.text().await.unwrap();
        assert!(
            body.contains("not permitted through the wave exec door"),
            "body names the refusal: {body}"
        );
    }

    #[tokio::test]
    async fn exec_door_pins_detached_loops_to_its_wave() {
        let (base, token, _tmp) = boot_exec().await;
        let response = reqwest::Client::new()
            .post(format!("{base}/v0/exec"))
            .header(SUBAGENT_TOKEN_HEADER, token)
            .json(&serde_json::json!({
                "argv": ["loop", "task", "ship it", "--wave", "another-wave", "--detach"],
                "cwd": null
            }))
            .send()
            .await
            .unwrap();

        assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
        assert!(response.text().await.unwrap().contains("must target wave"));
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
