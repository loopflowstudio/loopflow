//! The wave server's HTTP surface — a thin view over in-process state.
//!
//! Every endpoint reads or nudges [`WaveRuntime`]; none of them own logic. The
//! timeline is served as-is, live turns stream over SSE, and a POSTed message
//! is dropped into the in-process inbox. Discovery is a dumb pointer file, not
//! a transport: `wave/<name>/.wave-endpoint` holds `127.0.0.1:<port>` and
//! nothing else.
//!
//! Wire contract (snake_case, stable — a Concerto worker builds against it):
//! - `GET /health` → `{status, wave, turns, subagents, uptime_seconds}`;
//!   `status` is the mind state (`idle | turning | interrupting | failed`).
//! - `GET /conversation` → `{turns: [Turn]}`; includes the open turn (status
//!   `running`), if one is in progress, after the finalized thread.
//! - `GET /conversation/stream` → SSE, two event names and no more:
//!   - `state`: data is the mind-state name (`idle | turning | interrupting |
//!     failed`), sent once on subscribe (before the turn replay) and again on
//!     every transition — the composer keys its verb off it.
//!   - `turn`: data is a `Turn` JSON; the thread replays on connect
//!     (including the open turn), then streams live. Turn ids repeat: an
//!     in-progress turn is re-sent whole as it grows and finalization sends
//!     the terminal turn under the same id — each frame replaces the client's
//!     previous state for that id (upsert, never append-if-seen).
//! - `POST /messages {op, text, from?}` → `{turn, state}`. `op` is required —
//!   `"message"` (queued; the next turn answers it), `"steer"` (into the live
//!   turn when the harness supports it, else degrades to a queued message),
//!   `"interrupt"` (cancel the open turn; non-empty text becomes the next
//!   turn — "interrupt & send"; while idle, an interrupt is a no-op success),
//!   or `"say"` (an attributed emission — `lf chat`: a worker report,
//!   child-wave escalation, or CLI FYI; lands in the thread with its byline
//!   AND queues for the mind like a message). `text` may be empty only for
//!   `interrupt` (400 otherwise). `from {session_id?, label}` is required for
//!   `say` and rejected for every other op (400) — human turns are
//!   unattributed by convention. `turn` is the appended user `Turn`, or null
//!   for a bare interrupt (nothing was said); `state` is the mind-state name
//!   when the request was accepted — ops are applied by the mind
//!   asynchronously, so watch the stream's `state` events for the outcome.
//! - `GET /memory` → `{content}` — the wave's MEMORY.md, read from the
//!   origin repo.
//! - `POST /memory {op, content, summary}` → `{summary}`. `op` is `"update"`
//!   (full replacement) or `"add"` (append one curated bullet; `content` must
//!   be non-empty). `summary` is explicitly Optional — null falls back to the
//!   content's first non-empty line. The server is the sole writer of the
//!   origin repo's `wave/<name>/MEMORY.md` and journals `MemoryUpdated`.
//!
//! `Turn` is [`crate::lfd::conversations::turns::ChatTurn`].

use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::stream::{self, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio_stream::wrappers::BroadcastStream;

use crate::lfd::conversations::turns::ChatTurn;
use crate::lfd::wave::journal::{Attribution, MessageOp};
use crate::lfd::wave::runtime::WaveRuntime;
use crate::lfd::wave::state::MindState;

/// Basename of the discovery pointer under `wave/<name>/`.
pub const ENDPOINT_FILE: &str = ".wave-endpoint";

#[derive(Debug, Serialize)]
struct HealthBody {
    status: String,
    wave: String,
    turns: usize,
    subagents: usize,
    uptime_seconds: i64,
}

#[derive(Debug, Serialize)]
struct ConversationBody {
    turns: Vec<ChatTurn>,
}

/// `POST /messages` request body. `op` is required — explicit, never inferred
/// (no serde default; an op-less body is a 422). `from` is explicitly
/// Optional: required for `say`, rejected otherwise.
#[derive(Debug, Deserialize)]
struct PostMessage {
    op: MessageOp,
    text: String,
    from: Option<Attribution>,
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

/// Server state: the runtime plus when it started (for uptime).
#[derive(Clone)]
struct ServerState {
    runtime: Arc<WaveRuntime>,
    started_at: OffsetDateTime,
}

/// Build the router over a running [`WaveRuntime`].
pub fn router(runtime: Arc<WaveRuntime>) -> Router {
    let state = ServerState {
        runtime,
        started_at: OffsetDateTime::now_utc(),
    };
    Router::new()
        .route("/health", get(health_handler))
        .route("/conversation", get(conversation_handler))
        .route("/conversation/stream", get(conversation_stream_handler))
        .route("/messages", post(messages_handler))
        .route("/memory", get(memory_handler).post(memory_write_handler))
        .with_state(state)
}

async fn health_handler(State(state): State<ServerState>) -> Json<HealthBody> {
    Json(HealthBody {
        status: state.runtime.mind_state().name().to_string(),
        wave: state.runtime.name().to_string(),
        turns: state.runtime.thread_snapshot().len(),
        subagents: state.runtime.supervisor().reap(),
        uptime_seconds: (OffsetDateTime::now_utc() - state.started_at).whole_seconds(),
    })
}

async fn conversation_handler(State(state): State<ServerState>) -> Json<ConversationBody> {
    Json(ConversationBody {
        turns: state.runtime.thread_snapshot(),
    })
}

async fn messages_handler(
    State(state): State<ServerState>,
    Json(body): Json<PostMessage>,
) -> Result<Json<PostMessageResponse>, (StatusCode, String)> {
    let empty = body.text.trim().is_empty();
    if body.from.is_some() && !matches!(body.op, MessageOp::Say) {
        return Err((
            StatusCode::BAD_REQUEST,
            "`from` is only valid for the say op".to_string(),
        ));
    }
    let turn = match body.op {
        MessageOp::Say => {
            if empty {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "text is required for the say op".to_string(),
                ));
            }
            let Some(from) = body.from else {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "`from` is required for the say op".to_string(),
                ));
            };
            Some(state.runtime.deliver_say(body.text, from))
        }
        MessageOp::Message | MessageOp::Steer => {
            if empty {
                return Err((
                    StatusCode::BAD_REQUEST,
                    "text is required for message/steer ops".to_string(),
                ));
            }
            Some(state.runtime.deliver_user_message(body.text, body.op))
        }
        MessageOp::Interrupt => {
            if empty {
                // Bare interrupt: nothing said, nothing appended. While idle
                // this is a no-op success — the returned state says so.
                state.runtime.deliver_interrupt();
                None
            } else {
                // Interrupt & send: the text is journaled (op records intent)
                // and becomes the next turn after the cancel settles.
                Some(
                    state
                        .runtime
                        .deliver_user_message(body.text, MessageOp::Interrupt),
                )
            }
        }
    };
    Ok(Json(PostMessageResponse {
        turn,
        state: state.runtime.mind_state().name().to_string(),
    }))
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

/// SSE: send the mind state, replay the thread on connect (open turn
/// included, status `running`), then stream live frames as-is — `state` on
/// every transition, `turn` ids repeating by design (every frame replaces the
/// client's state for that id, so an in-progress turn updates in place and
/// its terminal frame lands under the same id). Snapshot and subscription are
/// atomic in the runtime (broadcasts share the append lock), so no live frame
/// is ever older than the replayed snapshot.
async fn conversation_stream_handler(
    State(state): State<ServerState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let sub = state.runtime.subscribe_with_snapshot();

    let replay = stream::iter(
        std::iter::once(Ok(state_event(&sub.state)))
            .chain(sub.turns.into_iter().map(|t| Ok(turn_event(&t)))),
    );
    let live_turns = BroadcastStream::new(sub.turn_rx).filter_map(move |res| {
        let out = match res {
            Ok(turn) => Some(Ok(turn_event(&turn))),
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

    Sse::new(replay.chain(stream::select(live_turns, live_states))).keep_alive(KeepAlive::default())
}

fn turn_event(turn: &ChatTurn) -> Event {
    Event::default()
        .event("turn")
        .data(serde_json::to_string(turn).unwrap_or_default())
}

fn state_event(state: &MindState) -> Event {
    Event::default().event("state").data(state.name())
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
