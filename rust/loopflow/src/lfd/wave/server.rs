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
//! - `GET /conversation/stream` → SSE; each event named `turn`, data a `Turn`
//!   JSON; replays the thread on connect (including the open turn), then
//!   streams live. Turn ids repeat: an in-progress turn is re-sent whole as it
//!   grows and finalization sends the terminal turn under the same id — each
//!   frame replaces the client's previous state for that id (upsert, never
//!   append-if-seen).
//! - `POST /messages {text}` → appends a user `Turn` and returns it.
//!
//! `Turn` is [`crate::lfd::conversations::turns::ChatTurn`].

use std::convert::Infallible;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::stream::{self, Stream, StreamExt};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio_stream::wrappers::BroadcastStream;

use crate::lfd::conversations::turns::ChatTurn;
use crate::lfd::wave::runtime::WaveRuntime;

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

/// `POST /messages` request body.
#[derive(Debug, Deserialize)]
struct PostMessage {
    text: String,
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
) -> Json<ChatTurn> {
    Json(state.runtime.deliver_user_message(body.text))
}

/// SSE: replay the thread on connect (open turn included, status `running`),
/// then stream live frames as-is. Ids repeat by design — every frame replaces
/// the client's state for that id, so an in-progress turn updates in place and
/// its terminal frame lands under the same id. Snapshot and subscription are
/// atomic in the runtime (broadcasts share the append lock), so no live frame
/// is ever older than the replayed snapshot.
async fn conversation_stream_handler(
    State(state): State<ServerState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let (snapshot, rx) = state.runtime.subscribe_with_snapshot();

    let replay = stream::iter(snapshot.into_iter().map(|t| Ok(turn_event(&t))));
    let live = BroadcastStream::new(rx).filter_map(move |res| {
        let out = match res {
            Ok(turn) => Some(Ok(turn_event(&turn))),
            // Lagged: the client fell behind. Skip; it resyncs from /conversation.
            Err(_) => None,
        };
        async move { out }
    });

    Sse::new(replay.chain(live)).keep_alive(KeepAlive::default())
}

fn turn_event(turn: &ChatTurn) -> Event {
    Event::default()
        .event("turn")
        .data(serde_json::to_string(turn).unwrap_or_default())
}

/// Path to the discovery pointer for a wave.
pub fn endpoint_path(repo_root: &Path, wave: &str) -> PathBuf {
    repo_root.join("wave").join(wave).join(ENDPOINT_FILE)
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

/// Remove the discovery pointer on shutdown. Best-effort.
pub fn remove_endpoint(repo_root: &Path, wave: &str) {
    let _ = std::fs::remove_file(endpoint_path(repo_root, wave));
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

        remove_endpoint(tmp.path(), "ship");
        assert!(!path.exists());
    }
}
