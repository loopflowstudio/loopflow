//! `lf wave <name>` — the wave runtime as a long-lived REACTIVE SERVER.
//!
//! A wave is not a loop. `lf wave <name>` starts a server that stays up until
//! stopped and reacts to events from two INDEPENDENT sources — one firing never
//! blocks the other:
//!
//! ```text
//!                 ┌─ subagent progress events ──┐
//!   Wave server ──┤                             ├──▶ react
//!                 └─ user messages ─────────────┘
//! ```
//!
//! - **subagent progress events** — the work. The [`progress`] arm keeps a
//!   subagent grinding at all times; every turn increment is appended to the
//!   wave's journal and each finalized turn commits to the thread.
//! - **user messages** — chat over HTTP. Answered TALK-ONLY from memory and
//!   current progress state; chat observes, it does not steer progress.
//!
//! Truth is the per-wave append-only [`journal`] (JSONL under `.lf/journal/
//! waves/<name>/`); the in-process state ([`runtime::WaveRuntime`]) — the
//! `thread` the user sees, the mind [`state`] — is a fold of it, rebuilt on
//! boot so a restart keeps the whole conversation. The journal is server-owned
//! persistence, not IPC; the only coordination file is a dumb discovery
//! pointer, `wave/<name>/.wave-endpoint` (see [`server`]).

pub mod journal;
pub mod memory;
pub mod progress;
pub mod runtime;
pub mod server;
pub mod state;
pub mod subagent;
pub mod supervisor;

use std::future::Future;
use std::path::PathBuf;

use anyhow::{anyhow, Result};

use crate::engine::worktrees::main_repo_root;
use crate::lf::commands::util::find_repo_root;
use crate::lfd::wave::runtime::{run_chat_consumer, WaveRuntime};
use crate::ops::util::resolve_wave_name;

/// Start the reactive wave server for `name` and block until it is stopped
/// (Ctrl-C). Binds a loopback port, publishes the discovery pointer, and runs
/// both reactions until shutdown.
pub fn run(name: &str) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root).unwrap_or(repo_root);
    let wave = resolve_wave_name(&main_repo, Some(name))
        .ok_or_else(|| anyhow!("invalid wave name: '{name}'"))?;

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(serve(main_repo, wave, shutdown_signal()))
}

/// Serve the wave until `shutdown` resolves. Factored out so tests can drive a
/// deterministic shutdown instead of Ctrl-C.
async fn serve(
    repo_root: PathBuf,
    wave: String,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<()> {
    let (runtime, inbox_rx) = WaveRuntime::open(wave.clone(), repo_root.clone())?;

    // Reaction 2 (chat) and the autonomous progress arm run as independent
    // top-level tasks so neither blocks the other. Each progress *pass* is
    // tracked in the supervisor, so `/health` reflects live subagents.
    let chat = tokio::spawn(run_chat_consumer(runtime.clone(), inbox_rx));
    let progress = tokio::spawn(progress::run_progress_arm(runtime.clone()));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    server::write_endpoint(&repo_root, &wave, addr)?;
    // Ctrl+C exits the process before graceful shutdown runs, so remove the
    // discovery pointer from the interrupt handler too.
    let cleanup_repo = repo_root.clone();
    let cleanup_wave = wave.clone();
    crate::engine::agent::register_interrupt_cleanup(move || {
        server::remove_endpoint(&cleanup_repo, &cleanup_wave);
    });
    println!("lf wave · {wave} · reactive server on http://{addr} (Ctrl-C to stop)");

    let app = server::router(runtime.clone());
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await;

    // Shutdown: stop the arms and every live subagent, drop the pointer.
    progress.abort();
    chat.abort();
    runtime.supervisor().shutdown_all();
    server::remove_endpoint(&repo_root, &wave);

    result.map_err(|err| anyhow!("wave server error: {err}"))
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    use crate::engine::stream::{ResultSubtype, StreamEvent};
    use crate::lfd::conversations::turns::{ChatRole, ChatTurn, TurnBuilder};
    use crate::lfd::conversations::types::Lifecycle;
    use crate::lfd::wave::runtime::TurnSink;

    fn progress_turn(text: &str) -> ChatTurn {
        ChatTurn {
            id: String::new(),
            role: ChatRole::Assistant,
            text: text.to_string(),
            status: Lifecycle::Completed,
            items: Vec::new(),
            created_at: "1970-01-01T00:00:00Z".to_string(),
        }
    }

    /// Inject a finalized progress turn, as the progress arm's sink would.
    fn narrate(runtime: &WaveRuntime, text: &str) {
        runtime.append_finalized_turn(progress_turn(text), Vec::new());
    }

    /// Boot just the HTTP surface + chat consumer over a runtime we control,
    /// without the real-codex progress arm. Returns the bound address and the
    /// runtime so the test can inject progress turns directly.
    async fn boot() -> (String, std::sync::Arc<WaveRuntime>, tempfile::TempDir) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("MEMORY.md"), "Goal: ship the reactive server.\n").expect("mem");

        let (runtime, inbox_rx) =
            WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open runtime");
        tokio::spawn(run_chat_consumer(runtime.clone(), inbox_rx));

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = server::router(runtime.clone());
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        (format!("http://{addr}"), runtime, tmp)
    }

    async fn wait_for<F: Fn() -> bool>(cond: F) {
        for _ in 0..200 {
            if cond() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        panic!("condition not met in time");
    }

    #[tokio::test]
    async fn subagent_turn_appears_in_conversation() {
        let (base, runtime, _tmp) = boot().await;
        narrate(&runtime, "Implemented the reactive server.");

        let body = reqwest::get(format!("{base}/conversation"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(body.contains("Implemented the reactive server."));
        assert!(body.contains("\"role\":\"assistant\""));
    }

    #[tokio::test]
    async fn posted_message_appears_as_user_turn_and_gets_a_reply() {
        let (base, runtime, _tmp) = boot().await;
        narrate(&runtime, "wired the SSE stream");

        let client = reqwest::Client::new();
        let posted: ChatTurn = client
            .post(format!("{base}/messages"))
            .json(&serde_json::json!({ "text": "how's it going?" }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(posted.role, ChatRole::User);
        assert_eq!(posted.text, "how's it going?");

        // The chat consumer appends exactly one reply, drawn from progress+memory.
        wait_for(|| runtime.thread_snapshot().len() >= 3).await;
        let thread = runtime.thread_snapshot();
        let reply = thread.last().unwrap();
        assert_eq!(reply.role, ChatRole::Assistant);
        assert!(reply.text.contains("wired the SSE stream"));
        assert!(reply.text.contains("ship the reactive server"));
    }

    #[tokio::test]
    async fn health_reports_status_and_turn_count() {
        let (base, runtime, _tmp) = boot().await;
        narrate(&runtime, "first");
        let body = reqwest::get(format!("{base}/health"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(
            body.contains("\"status\":\"idle\""),
            "health status is the mind state"
        );
        assert!(body.contains("\"wave\":\"ship\""));
        assert!(body.contains("\"turns\":1"));
    }

    #[tokio::test]
    async fn sse_replays_on_connect_then_streams_live() {
        let (base, runtime, _tmp) = boot().await;
        narrate(&runtime, "replayed turn");

        let host = base.strip_prefix("http://").unwrap().to_string();
        let mut stream = tokio::net::TcpStream::connect(&host).await.unwrap();
        stream
            .write_all(
                b"GET /conversation/stream HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n",
            )
            .await
            .unwrap();

        // Read until we've seen the replayed turn, then a live one.
        narrate(&runtime, "live turn");
        let mut acc = String::new();
        let mut buf = [0u8; 2048];
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let read = tokio::time::timeout_at(deadline, stream.read(&mut buf)).await;
            match read {
                Ok(Ok(0)) | Err(_) => break,
                Ok(Ok(n)) => {
                    acc.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if acc.contains("replayed turn") && acc.contains("live turn") {
                        break;
                    }
                }
                Ok(Err(_)) => break,
            }
        }
        assert!(acc.contains("event: turn"), "SSE frames are named `turn`");
        assert!(
            acc.contains("replayed turn"),
            "replays the thread on connect"
        );
        assert!(
            acc.contains("live turn"),
            "streams turns narrated after connect"
        );
    }

    /// Raw-TCP SSE client that decodes the chunked body and parses every
    /// `data:` line into a [`ChatTurn`], in arrival order.
    struct SseClient {
        stream: tokio::net::TcpStream,
        raw: Vec<u8>,
    }

    impl SseClient {
        async fn connect(base: &str) -> Self {
            let host = base.strip_prefix("http://").unwrap();
            let mut stream = tokio::net::TcpStream::connect(host).await.unwrap();
            stream
                .write_all(
                    b"GET /conversation/stream HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n",
                )
                .await
                .unwrap();
            Self {
                stream,
                raw: Vec::new(),
            }
        }

        /// Read until `pred` holds over every turn frame received so far
        /// (panics after 5s). Returns the frames, in order.
        async fn frames_until(&mut self, pred: impl Fn(&[ChatTurn]) -> bool) -> Vec<ChatTurn> {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            let mut buf = [0u8; 4096];
            loop {
                let frames = parse_turn_frames(&dechunk(&self.raw));
                if pred(&frames) {
                    return frames;
                }
                match tokio::time::timeout_at(deadline, self.stream.read(&mut buf)).await {
                    Ok(Ok(0)) | Err(_) => {
                        panic!("SSE ended before condition; {} frames so far", frames.len())
                    }
                    Ok(Ok(n)) => self.raw.extend_from_slice(&buf[..n]),
                    Ok(Err(err)) => panic!("SSE read error: {err}"),
                }
            }
        }
    }

    /// Strip the HTTP response head and chunked transfer framing, tolerating a
    /// partial tail (the connection stays open). Test traffic is ASCII.
    fn dechunk(raw: &[u8]) -> String {
        let text = String::from_utf8_lossy(raw);
        let Some(head_end) = text.find("\r\n\r\n") else {
            return String::new();
        };
        let mut body = &text[head_end + 4..];
        let mut out = String::new();
        while let Some(size_end) = body.find("\r\n") {
            let Ok(size) = usize::from_str_radix(body[..size_end].trim(), 16) else {
                break;
            };
            let start = size_end + 2;
            if size == 0 || body.len() < start + size {
                break;
            }
            out.push_str(&body[start..start + size]);
            body = &body[(start + size + 2).min(body.len())..];
        }
        out
    }

    fn parse_turn_frames(sse_body: &str) -> Vec<ChatTurn> {
        sse_body
            .lines()
            .filter_map(|line| line.strip_prefix("data:"))
            .filter_map(|data| serde_json::from_str(data.trim()).ok())
            .collect()
    }

    /// Feed one stream event through the production delta pipeline.
    fn feed(builder: &mut TurnBuilder, sink: &mut TurnSink, event: StreamEvent) {
        for delta in builder.feed(&event) {
            sink.on_delta(delta);
        }
    }

    #[tokio::test]
    async fn sse_late_subscriber_watches_the_open_turn_grow_and_finalize() {
        let (base, runtime, _tmp) = boot().await;
        narrate(&runtime, "already finalized");

        // A turn is mid-flight before the client connects.
        let mut sink = TurnSink::new(runtime.clone());
        let mut builder = TurnBuilder::new();
        feed(
            &mut builder,
            &mut sink,
            StreamEvent::Text("thinking".into()),
        );

        // Late subscriber: replay carries the finalized thread AND the open turn.
        let mut client = SseClient::connect(&base).await;
        let frames = client
            .frames_until(|f| f.iter().any(|t| t.status == Lifecycle::Running))
            .await;
        assert!(
            frames
                .iter()
                .any(|t| t.text == "already finalized" && t.status == Lifecycle::Completed),
            "replay carries the finalized thread"
        );
        let open = frames
            .iter()
            .find(|t| t.status == Lifecycle::Running)
            .unwrap()
            .clone();
        assert_eq!(open.text, "thinking");

        // Re-broadcast: the same id grows in place.
        feed(&mut builder, &mut sink, StreamEvent::Text("more".into()));
        client
            .frames_until(|f| {
                f.iter().any(|t| {
                    t.id == open.id && t.text == "thinking\nmore" && t.status == Lifecycle::Running
                })
            })
            .await;

        // Finalization replaces it terminally, same id.
        feed(
            &mut builder,
            &mut sink,
            StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: None,
                duration_secs: None,
            },
        );
        let frames = client
            .frames_until(|f| {
                f.iter()
                    .any(|t| t.id == open.id && t.status == Lifecycle::Completed)
            })
            .await;
        let last = frames.iter().rfind(|t| t.id == open.id).unwrap();
        assert_eq!(last.status, Lifecycle::Completed, "terminal frame is last");
        assert_eq!(last.text, "thinking\nmore");
    }

    #[tokio::test]
    async fn conversation_includes_the_open_running_turn() {
        let (base, runtime, _tmp) = boot().await;
        let mut sink = TurnSink::new(runtime.clone());
        let mut builder = TurnBuilder::new();
        feed(
            &mut builder,
            &mut sink,
            StreamEvent::Text("half a thought".into()),
        );

        let body: serde_json::Value = reqwest::get(format!("{base}/conversation"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let turns = body["turns"].as_array().unwrap();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0]["status"], "running");
        assert_eq!(turns[0]["text"], "half a thought");

        // After finalization the same id is served exactly once, terminal.
        feed(
            &mut builder,
            &mut sink,
            StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: None,
                duration_secs: None,
            },
        );
        let body: serde_json::Value = reqwest::get(format!("{base}/conversation"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let turns = body["turns"].as_array().unwrap();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0]["status"], "completed");
    }

    #[tokio::test]
    async fn restart_mid_turn_never_serves_a_stale_running_turn() {
        let tmp = tempfile::tempdir().expect("tempdir");

        // First life crashes mid-turn: started + text journaled, never finished.
        {
            let (runtime, _rx) =
                WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open");
            let mut sink = TurnSink::new(runtime.clone());
            let mut builder = TurnBuilder::new();
            feed(
                &mut builder,
                &mut sink,
                StreamEvent::Text("half a thought".into()),
            );
        }

        // Second life: journal replay + boot janitor close the crash tail.
        let (runtime, _rx) =
            WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("reopen");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = server::router(runtime.clone());
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        let base = format!("http://{addr}");

        let body: serde_json::Value = reqwest::get(format!("{base}/conversation"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let turns = body["turns"].as_array().unwrap();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0]["status"], "failed", "janitor closed the turn");
        assert_eq!(turns[0]["text"], "half a thought");

        // SSE replay agrees: the turn arrives failed, never running.
        let mut client = SseClient::connect(&base).await;
        let frames = client
            .frames_until(|f| f.iter().any(|t| t.status == Lifecycle::Failed))
            .await;
        assert!(
            frames.iter().all(|t| t.status != Lifecycle::Running),
            "no stale running turn in replay"
        );
    }

    #[tokio::test]
    async fn serve_publishes_and_removes_discovery_pointer() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("wave/ship")).unwrap();
        let repo = tmp.path().to_path_buf();

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let repo2 = repo.clone();
        let handle = tokio::spawn(async move {
            serve(repo2, "ship".into(), async {
                let _ = shutdown_rx.await;
            })
            .await
        });

        let endpoint = server::endpoint_path(&repo, "ship");
        wait_for(|| endpoint.exists()).await;
        let contents = std::fs::read_to_string(&endpoint).unwrap();
        assert!(
            contents.starts_with("127.0.0.1:"),
            "pointer is just an address"
        );

        shutdown_tx.send(()).unwrap();
        handle.await.unwrap().unwrap();
        assert!(!endpoint.exists(), "pointer removed on shutdown");
    }
}
