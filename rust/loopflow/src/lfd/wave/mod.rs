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
//!   subagent grinding at all times; every finalized turn is narrated into the
//!   thread and durable facts fold into MEMORY.
//! - **user messages** — chat over HTTP. Answered TALK-ONLY from memory and
//!   current progress state; chat observes, it does not steer progress.
//!
//! All state is in-process (see [`runtime::WaveRuntime`]): the `thread` the user
//! sees, a MEMORY handle, and an in-process inbox channel. There are no files as
//! IPC. The only file the server writes for coordination is a dumb discovery
//! pointer, `wave/<name>/.wave-endpoint` (see [`server`]).

pub mod memory;
pub mod progress;
pub mod runtime;
pub mod server;
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
    let (runtime, inbox_rx) = WaveRuntime::new(wave.clone(), repo_root.clone());

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

    use crate::lfd::conversations::turns::{ChatRole, ChatTurn};
    use crate::lfd::conversations::types::Lifecycle;

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

    /// Boot just the HTTP surface + chat consumer over a runtime we control,
    /// without the real-codex progress arm. Returns the bound address and the
    /// runtime so the test can inject progress turns directly.
    async fn boot() -> (String, std::sync::Arc<WaveRuntime>, tempfile::TempDir) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("MEMORY.md"), "Goal: ship the reactive server.\n").expect("mem");

        let (runtime, inbox_rx) = WaveRuntime::new("ship".into(), tmp.path().to_path_buf());
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
        runtime.narrate_progress(progress_turn("Implemented the reactive server."));

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
        runtime.narrate_progress(progress_turn("wired the SSE stream"));

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
        runtime.narrate_progress(progress_turn("first"));
        let body = reqwest::get(format!("{base}/health"))
            .await
            .unwrap()
            .text()
            .await
            .unwrap();
        assert!(body.contains("\"status\":\"ok\""));
        assert!(body.contains("\"wave\":\"ship\""));
        assert!(body.contains("\"turns\":1"));
    }

    #[tokio::test]
    async fn sse_replays_on_connect_then_streams_live() {
        let (base, runtime, _tmp) = boot().await;
        runtime.narrate_progress(progress_turn("replayed turn"));

        let host = base.strip_prefix("http://").unwrap().to_string();
        let mut stream = tokio::net::TcpStream::connect(&host).await.unwrap();
        stream
            .write_all(
                b"GET /conversation/stream HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n",
            )
            .await
            .unwrap();

        // Read until we've seen the replayed turn, then a live one.
        runtime.narrate_progress(progress_turn("live turn"));
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
