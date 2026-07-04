//! Per-wave chat server, hosted in-process by `lf wave`.
//!
//! `lf wave <name>` owns the wave's runtime. This module is the chat surface of
//! that runtime: a small HTTP server that Concerto observes to render the wave's
//! live conversation. It is deliberately per-wave and ephemeral — there is no
//! central conversation daemon.
//!
//! ## Discovery
//!
//! On start the server binds an ephemeral port on `127.0.0.1` and writes
//! `wave/<name>/.chat-endpoint` containing a single line, `127.0.0.1:<port>`.
//! Concerto reads that file to learn where to connect; it is removed on shutdown.
//!
//! ## Endpoints
//!
//! - `GET  /health` → `{ "status", "wave", "pass", "turns" }`
//! - `GET  /chat` → `{ "wave", "turns": [ChatTurn, …] }` (snapshot)
//! - `GET  /chat/stream` → SSE of `ChatTurn`s (event name `turn`): replays the
//!   current turns, then streams new/updated turns live.
//! - `POST /chat` `{ "text": "…" }` → enqueues a human message; it is appended to
//!   `wave/<name>/MAILBOX.md` (which the next `lf goal --once` pass folds into its
//!   prompt) and recorded as a `user` turn. Returns the created `ChatTurn`.
//!
//! ## Turn source
//!
//! [`ChatState::ingest_line`] takes raw agent stream-json lines (the wave's inner
//! `codex exec --json` pass), parses them through [`crate::engine::stream`], and
//! folds the events into [`ChatTurn`]s via [`TurnBuilder`].

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::engine::stream::{ParseResult, StreamParser};
use crate::lfd::conversations::turns::{ChatTurn, TurnBuilder};

const ENDPOINT_FILE: &str = ".chat-endpoint";
const MAILBOX_FILE: &str = "MAILBOX.md";
const UPDATE_BUFFER: usize = 256;

/// Live chat state for one wave. Shared between the HTTP handlers and the tailer
/// that ingests the inner pass's agent stream.
#[derive(Debug)]
pub struct ChatState {
    wave: String,
    wave_dir: PathBuf,
    inner: Mutex<ChatInner>,
    updates: broadcast::Sender<ChatTurn>,
}

#[derive(Debug)]
struct ChatInner {
    builder: TurnBuilder,
    /// Finalized assistant turns and human `user` turns, in arrival order.
    turns: Vec<ChatTurn>,
    /// Number of inner passes started so far.
    pass: u32,
    user_seq: u64,
}

impl ChatState {
    pub fn new(wave: String, wave_dir: PathBuf) -> Arc<Self> {
        let (updates, _rx) = broadcast::channel(UPDATE_BUFFER);
        Arc::new(Self {
            wave,
            wave_dir,
            inner: Mutex::new(ChatInner {
                builder: TurnBuilder::new(),
                turns: Vec::new(),
                pass: 0,
                user_seq: 0,
            }),
            updates,
        })
    }

    /// Mark the start of a new inner pass. Any turn still open from a prior pass
    /// (a pass that ended without a clean result) is finalized as failed.
    pub fn begin_pass(&self) {
        let mut inner = self.inner.lock().expect("chat state poisoned");
        inner.pass += 1;
        if let Some(turn) = inner.builder.finish_open() {
            inner.turns.push(turn.clone());
            let _ = self.updates.send(turn);
        }
    }

    /// Ingest one raw agent stream-json line from the inner pass.
    pub fn ingest_line(&self, parser: &mut StreamParser, line: &str) {
        match parser.feed_line(line) {
            ParseResult::Events(events) => {
                for event in &events {
                    self.ingest_event(event);
                }
            }
            ParseResult::Skipped | ParseResult::Passthrough => {}
        }
    }

    fn ingest_event(&self, event: &crate::engine::stream::StreamEvent) {
        let mut inner = self.inner.lock().expect("chat state poisoned");
        if let Some(finished) = inner.builder.feed(event) {
            inner.turns.push(finished.clone());
            let _ = self.updates.send(finished);
        } else if let Some(open) = inner.builder.snapshot().cloned() {
            // Broadcast the in-progress snapshot so subscribers see partial text.
            let _ = self.updates.send(open);
        }
    }

    /// Record a human message: append to the wave mailbox and add a `user` turn.
    pub fn record_user_message(&self, text: &str) -> Result<ChatTurn> {
        append_to_mailbox(&self.wave_dir, text)?;
        let mut inner = self.inner.lock().expect("chat state poisoned");
        inner.user_seq += 1;
        let turn = ChatTurn::user(format!("user-{}", inner.user_seq), text.to_string());
        inner.turns.push(turn.clone());
        let _ = self.updates.send(turn.clone());
        Ok(turn)
    }

    /// Snapshot: finalized turns plus any in-progress assistant turn.
    fn turns_snapshot(&self) -> Vec<ChatTurn> {
        let inner = self.inner.lock().expect("chat state poisoned");
        let mut turns = inner.turns.clone();
        if let Some(open) = inner.builder.snapshot() {
            turns.push(open.clone());
        }
        turns
    }

    fn pass(&self) -> u32 {
        self.inner.lock().expect("chat state poisoned").pass
    }

    /// Number of turns currently visible (finalized + any in-progress turn).
    pub fn turn_count(&self) -> usize {
        self.turns_snapshot().len()
    }
}

/// Append a human message to `wave/<name>/MAILBOX.md`, timestamped. The next
/// `lf goal --once` pass reads and clears this file (see `goal::drain_mailbox`).
fn append_to_mailbox(wave_dir: &Path, text: &str) -> Result<()> {
    use std::io::Write;

    std::fs::create_dir_all(wave_dir)
        .with_context(|| format!("create wave dir {}", wave_dir.display()))?;
    let path = wave_dir.join(MAILBOX_FILE);
    let stamp = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap_or_default();
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("open mailbox {}", path.display()))?;
    writeln!(file, "- [{stamp}] {}", text.replace('\n', " "))
        .with_context(|| format!("append mailbox {}", path.display()))?;
    Ok(())
}

// ── HTTP ────────────────────────────────────────────────────────────────────

pub fn router(state: Arc<ChatState>) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/chat", get(get_chat).post(post_chat))
        .route("/chat/stream", get(stream_chat))
        .with_state(state)
}

async fn health(State(state): State<Arc<ChatState>>) -> impl IntoResponse {
    let turns = state.turns_snapshot().len();
    Json(json!({
        "status": "ok",
        "wave": state.wave,
        "pass": state.pass(),
        "turns": turns,
    }))
}

async fn get_chat(State(state): State<Arc<ChatState>>) -> impl IntoResponse {
    Json(json!({
        "wave": state.wave,
        "turns": state.turns_snapshot(),
    }))
}

#[derive(Debug, Deserialize)]
struct PostChat {
    text: String,
}

async fn post_chat(
    State(state): State<Arc<ChatState>>,
    Json(body): Json<PostChat>,
) -> impl IntoResponse {
    match state.record_user_message(&body.text) {
        Ok(turn) => (axum::http::StatusCode::OK, Json(json!(turn))).into_response(),
        Err(err) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": { "message": err.to_string() } })),
        )
            .into_response(),
    }
}

async fn stream_chat(State(state): State<Arc<ChatState>>) -> impl IntoResponse {
    let rx = state.updates.subscribe();
    // Replay the current turns first so a late subscriber gets full context,
    // then follow live updates.
    let initial = state.turns_snapshot();
    let replay = tokio_stream::iter(initial.into_iter().map(Ok));
    let live = BroadcastStream::new(rx).filter_map(|item| item.ok().map(Ok));
    let stream = replay.chain(live).map(|turn: Result<ChatTurn, _>| {
        turn.and_then(|turn| Event::default().event("turn").json_data(turn))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

// ── Discovery ────────────────────────────────────────────────────────────────

/// Bind the chat server's listener on an ephemeral loopback port and publish the
/// `.chat-endpoint` discovery file under the wave directory.
pub async fn bind(wave_dir: &Path) -> Result<TcpListener> {
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .context("bind wave chat listener")?;
    let addr = listener
        .local_addr()
        .context("resolve chat listener addr")?;
    write_endpoint_file(wave_dir, addr)?;
    Ok(listener)
}

fn write_endpoint_file(wave_dir: &Path, addr: SocketAddr) -> Result<()> {
    std::fs::create_dir_all(wave_dir)
        .with_context(|| format!("create wave dir {}", wave_dir.display()))?;
    let path = wave_dir.join(ENDPOINT_FILE);
    std::fs::write(&path, format!("{addr}\n"))
        .with_context(|| format!("write endpoint file {}", path.display()))?;
    Ok(())
}

/// Remove the discovery file. Best-effort; called on shutdown.
pub fn remove_endpoint_file(wave_dir: &Path) {
    let _ = std::fs::remove_file(wave_dir.join(ENDPOINT_FILE));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::stream::{ResultSubtype, StreamEvent};

    fn tmp_state() -> (tempfile::TempDir, Arc<ChatState>) {
        let dir = tempfile::tempdir().expect("tempdir");
        let state = ChatState::new("demo".to_string(), dir.path().to_path_buf());
        (dir, state)
    }

    #[test]
    fn ingest_builds_turns_from_codex_stream() {
        let (_dir, state) = tmp_state();
        let mut parser = StreamParser::new();
        state.begin_pass();
        state.ingest_line(
            &mut parser,
            r#"{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"Done."}}"#,
        );
        state.ingest_line(
            &mut parser,
            r#"{"type":"turn.completed","usage":{"input_tokens":1,"output_tokens":1}}"#,
        );
        let turns = state.turns_snapshot();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].text, "Done.");
    }

    #[test]
    fn user_message_writes_mailbox_and_records_turn() {
        let (dir, state) = tmp_state();
        let turn = state
            .record_user_message("check the tests")
            .expect("record");
        assert_eq!(turn.text, "check the tests");
        let mailbox = std::fs::read_to_string(dir.path().join(MAILBOX_FILE)).expect("mailbox");
        assert!(mailbox.contains("check the tests"));
        assert_eq!(state.turns_snapshot().len(), 1);
    }

    #[test]
    fn begin_pass_finalizes_dangling_turn_as_failed() {
        let (_dir, state) = tmp_state();
        state.begin_pass();
        // Open a turn but never finish it, then start a new pass.
        state.ingest_event(&StreamEvent::Text("half".into()));
        assert_eq!(state.turns_snapshot().len(), 1); // in-progress snapshot
        state.begin_pass();
        let turns = state.turns_snapshot();
        assert_eq!(turns.len(), 1);
        assert_eq!(
            turns[0].status,
            crate::lfd::conversations::turns::ChatTurnStatus::Failed
        );
    }

    #[test]
    fn bind_writes_discovery_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let rt = tokio::runtime::Runtime::new().expect("rt");
        let listener = rt.block_on(bind(dir.path())).expect("bind");
        let contents = std::fs::read_to_string(dir.path().join(ENDPOINT_FILE)).expect("endpoint");
        assert!(contents.trim().starts_with("127.0.0.1:"));
        assert_eq!(
            contents.trim(),
            listener.local_addr().expect("addr").to_string()
        );
    }

    #[test]
    fn result_event_reference_compiles() {
        // Guards against StreamEvent::Result field drift used by TurnBuilder.
        let _ = StreamEvent::Result {
            subtype: ResultSubtype::Success,
            cost_usd: None,
            duration_secs: None,
        };
    }
}
