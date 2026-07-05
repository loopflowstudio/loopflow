//! `lf wave <name>` — the wave runtime as a long-lived REACTIVE SERVER.
//!
//! A wave is not a loop. `lf wave <name>` starts a server that stays up until
//! stopped. Its brain is THE MIND ([`mind`]): one persistent vendor thread
//! (codex app-server, driven through the conversations [`Harness`]) that
//! handles progress and chat as one context. Turns are scheduled by events —
//! a user message, a turn boundary with a non-empty queue, or a heartbeat
//! tick when quiet — never a busy-loop:
//!
//! ```text
//!                 ┌─ user messages (HTTP inbox) ─┐
//!   Wave server ──┤                              ├──▶ the mind (one thread)
//!                 └─ heartbeat when idle ────────┘
//! ```
//!
//! Truth is the per-wave append-only [`journal`] (JSONL under `.lf/journal/
//! waves/<name>/`); the in-process state ([`runtime::WaveRuntime`]) — the
//! `thread` the user sees, the mind [`state`], the vendor thread id — is a
//! fold of it, rebuilt on boot so a restart keeps the whole conversation. The
//! journal is server-owned persistence, not IPC; the only coordination file
//! is a dumb discovery pointer, `wave/<name>/.wave-endpoint` (see [`server`]).
//!
//! The server also keeps a best-effort seat in the shared session
//! [`registry`] (the same local store lfd serves from — the db IS the
//! registry): a `WaveAgent` session row registered store-direct at boot (one
//! brain per wave, enforced by a pid-probing pre-flight) and a store-polling
//! observer that journals `WorkerDispatched`/`WorkerFinished` observations,
//! which fold into the mind's `<in_flight>` heartbeat context. No registry
//! store on the machine → warn once, fully functional anyway.

pub mod journal;
pub mod memory;
pub mod mind;
pub mod registry;
pub mod runtime;
pub mod server;
pub mod state;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use tokio::sync::mpsc;

use crate::engine::worktrees::main_repo_root;
use crate::lf::commands::util::find_repo_root;
use crate::lfd::conversations::harness::{default_create_harness, ApprovalPolicy, Harness};
use crate::lfd::conversations::types::ConversationEvent;
use crate::lfd::executor::ensure_wave_worktree;
use crate::lfd::store::{open_existing_store, SharedStore};
use crate::lfd::wave::mind::MindConfig;
use crate::lfd::wave::runtime::WaveRuntime;
use crate::ops::util::resolve_wave_name;

/// Start the reactive wave server for `name` and block until it is stopped
/// (Ctrl-C). Bootstraps into the wave's worktree (`<repo>.<wave>` sibling —
/// created if missing; the mind always runs there, never the main checkout),
/// binds a loopback port, publishes the discovery pointer, registers as the
/// wave's agent session in the shared store (best-effort — see [`registry`];
/// `force` takes over an existing live wave-agent session), and runs the
/// mind on a codex app-server session. Wave state (journal, discovery
/// pointer, MEMORY.md) lives under the main repo so Concerto and a restarted
/// server agree on where it is.
pub fn run(name: &str, force: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root).unwrap_or_else(|_| repo_root.clone());
    let wave = resolve_wave_name(&main_repo, Some(name))
        .ok_or_else(|| anyhow!("invalid wave name: '{name}'"))?;

    // Self-bootstrap: ensure and enter the wave's worktree.
    let mind_cwd = wave_worktree(&main_repo, &wave)?;
    if std::env::current_dir().ok().as_deref() != Some(mind_cwd.as_path()) {
        std::env::set_current_dir(&mind_cwd)?;
        println!("lf wave · {wave} · worktree {}", mind_cwd.display());
    }

    // Every child of this server — the mind's harness and anything it shells
    // out to — must resolve `lf` to the binary running this server, not an
    // installed one (observed live: the mind's `lf` predated `lf q`).
    std::env::set_var("PATH", mind::path_for_children());

    let (events_tx, events_rx) = mpsc::unbounded_channel();
    let harness = default_create_harness("codex", ApprovalPolicy::AutoApprove, events_tx)?;

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let registry_config = resolve_registry(&main_repo, &wave, &mind_cwd, force).await;
        serve(
            main_repo,
            mind_cwd,
            wave,
            harness,
            events_rx,
            registry_config,
            force,
            shutdown_signal(),
        )
        .await
    })
}

/// The wave's own worktree — `<repo>.<wave>`, a sibling of the main repo —
/// created on first boot, reused after. The server registers under this
/// directory's name and the mind runs in it.
fn wave_worktree(main_repo: &Path, wave: &str) -> Result<PathBuf> {
    let (path, _branch) = ensure_wave_worktree(main_repo, wave)?;
    Ok(PathBuf::from(path))
}

/// Open the machine's shared registry and resolve this wave's row, creating
/// the row when the store has never seen the wave — the db IS the registry,
/// so a reachable store always yields a registered boot (see
/// [`registry::ensure_wave_row`]). `None` (with one warning) only when the
/// store itself is missing or unusable: the server runs unregistered — no
/// one-brain enforcement, no worker observations — the pre-registry status
/// quo.
async fn resolve_registry(
    main_repo: &Path,
    wave: &str,
    cwd: &Path,
    force: bool,
) -> Option<registry::RegistryConfig> {
    let Some(store) = open_existing_store().await else {
        tracing::warn!(
            wave,
            "no session registry on this machine; running unregistered \
             (no one-brain enforcement, no worker observations)"
        );
        return None;
    };
    let store: SharedStore = Arc::new(store);
    match registry::ensure_wave_row(&store, main_repo, wave).await {
        Ok(row) => Some(registry::RegistryConfig {
            store,
            wave: row,
            cwd: cwd.display().to_string(),
            pid: std::process::id(),
            force,
        }),
        Err(err) => {
            tracing::warn!(wave, error = %err, "session registry unusable; running unregistered");
            None
        }
    }
}

/// Serve the wave until `shutdown` resolves. The harness and its event stream
/// are parameters so tests can drive a mock instead of a real vendor process;
/// `registry_config` is `None` in tests that exercise the server without a
/// registry store. `force` rides separately because the endpoint-file floor
/// must honor it even when there is no registry config at all.
#[allow(clippy::too_many_arguments)]
async fn serve(
    repo_root: PathBuf,
    mind_cwd: PathBuf,
    wave: String,
    harness: Box<dyn Harness>,
    events_rx: mpsc::UnboundedReceiver<ConversationEvent>,
    registry_config: Option<registry::RegistryConfig>,
    force: bool,
    shutdown: impl Future<Output = ()> + Send + 'static,
) -> Result<()> {
    // File-level one-brain floor, before anything else: an existing pointer
    // that answers /health for this wave is a live server — refuse (unless
    // --force takes over and overwrites); a dead pointer is stale and gets
    // overwritten below. Works with no registry store at all.
    if let Some(live) = server::live_endpoint(&repo_root, &wave).await {
        if !force {
            return Err(anyhow!(
                "refusing to start: wave '{wave}' already has a live server at \
                 http://{live} (per wave/{wave}/{endpoint}). Stop that session (or let \
                 it finish), or rerun with --force to take over.",
                endpoint = server::ENDPOINT_FILE,
            ));
        }
        tracing::warn!(
            wave,
            live,
            "--force: taking over a live wave server endpoint"
        );
    }

    let (runtime, inbox_rx) = WaveRuntime::open(wave.clone(), repo_root.clone())?;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    // Registry seat: write the WaveAgent row (one-brain pre-flight — a live
    // brain refuses the start unless --force) and start the store-polling
    // observer. Best-effort by design: a store failure degrades to the
    // registry-less status quo.
    let mut registration: Option<registry::Registration> = None;
    let mut observer: Option<Arc<registry::StoreObserver>> = None;
    let mut observer_task: Option<tokio::task::JoinHandle<()>> = None;
    if let Some(config) = registry_config {
        match registry::register(&config, &addr.to_string()).await {
            Ok(registry::RegisterOutcome::Registered(reg)) => {
                let reg = *reg;
                tracing::info!(
                    wave,
                    session_id = %reg.session_id(),
                    "registered in the session registry as the wave's agent session"
                );
                // The mind's children attribute here: a bare `lf` inside the
                // mind self-registers under this wave with this session as
                // its parent (see lf::session for the env contract).
                std::env::set_var(
                    crate::lf::session::WAVE_ID_ENV,
                    config.wave.id().to_string(),
                );
                std::env::set_var(
                    crate::lf::session::SESSION_ID_ENV,
                    reg.session_id().to_string(),
                );
                std::env::set_var(crate::lf::session::SESSION_INHERITED_ENV, "1");
                // Ctrl+C exits the process before the graceful path below
                // runs, so deregister from the interrupt hook too; the
                // once-guard makes whichever path runs first the only one.
                let hook_registration = reg.clone();
                crate::engine::agent::register_interrupt_cleanup(move || {
                    hook_registration.deregister_blocking();
                });
                registration = Some(reg);

                let obs = Arc::new(registry::StoreObserver::new(
                    runtime.clone(),
                    config.store.clone(),
                    config.wave.id().clone(),
                ));
                observer_task = Some(tokio::spawn(Arc::clone(&obs).run(registry::POLL_CADENCE)));
                observer = Some(obs);
            }
            Ok(registry::RegisterOutcome::Refused { message }) => {
                return Err(anyhow!(
                    "refusing to start: {message}. Stop that session (or let it \
                     finish), or rerun with --force to take over."
                ));
            }
            Err(err) => {
                tracing::warn!(
                    wave,
                    error = %err,
                    "session registry write failed; running unregistered (no one-brain \
                     enforcement, no worker observations)"
                );
            }
        }
    }

    // The mind is the wave's one brain: it owns the vendor thread and reacts
    // to user messages and heartbeats. It runs as its own task so the HTTP
    // surface never blocks on a turn.
    let mind = tokio::spawn(mind::run_mind(
        runtime.clone(),
        inbox_rx,
        harness,
        events_rx,
        mind_cwd,
        MindConfig::default(),
        observer,
    ));

    server::write_endpoint(&repo_root, &wave, addr)?;
    // Ctrl+C exits the process before graceful shutdown runs, so remove the
    // discovery pointer from the interrupt handler too. Both paths remove it
    // only while it still holds OUR address — a takeover's pointer stays.
    let own_addr = addr.to_string();
    let cleanup_repo = repo_root.clone();
    let cleanup_wave = wave.clone();
    let cleanup_addr = own_addr.clone();
    crate::engine::agent::register_interrupt_cleanup(move || {
        server::remove_endpoint(&cleanup_repo, &cleanup_wave, &cleanup_addr);
    });
    println!(
        "lf wave · {wave} · reactive server on http://{addr} \
         (Ctrl-C to stop, RUST_LOG=loopflow=debug for the firehose)"
    );

    let app = server::router(runtime.clone());
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await;

    // Shutdown: stop the mind (its harness child dies with it — kill_on_drop),
    // mark the registry row terminal, drop the pointer. Workers are their own
    // tmux sessions — nothing here owns them.
    mind.abort();
    if let Some(task) = observer_task {
        task.abort();
    }
    if let Some(registration) = registration {
        registration.deregister().await;
    }
    server::remove_endpoint(&repo_root, &wave, &own_addr);

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

    use crate::engine::agent::AgentConfig;
    use crate::lfd::conversations::harness::Capabilities;
    use crate::lfd::conversations::turns::{ChatRole, ChatTurn};
    use crate::lfd::conversations::types::{ConversationItem, Lifecycle, TurnUsage};
    use crate::lfd::wave::mind::EventAdapter;
    use crate::lfd::wave::runtime::TurnSink;

    /// A harness that never speaks: `serve` tests exercise the HTTP shell,
    /// not a vendor process.
    struct StubHarness;

    #[async_trait::async_trait]
    impl Harness for StubHarness {
        async fn start(&mut self, _config: &AgentConfig) -> Result<()> {
            Ok(())
        }
        async fn send_input(&mut self, _content: &str) -> Result<()> {
            Ok(())
        }
        async fn interrupt(&mut self) -> Result<()> {
            Ok(())
        }
        async fn stop(&mut self) -> Result<()> {
            Ok(())
        }
        fn capabilities(&self) -> Capabilities {
            Capabilities {
                supports_steer: false,
                supports_interrupt: false,
            }
        }
        fn provider_session_id(&self) -> Option<String> {
            None
        }
    }

    fn progress_turn(text: &str) -> ChatTurn {
        ChatTurn {
            id: String::new(),
            role: ChatRole::Assistant,
            text: text.to_string(),
            status: Lifecycle::Completed,
            items: Vec::new(),
            created_at: "1970-01-01T00:00:00Z".to_string(),
            from: None,
        }
    }

    /// Inject a finalized assistant turn, as a completed mind turn would land.
    fn narrate(runtime: &WaveRuntime, text: &str) {
        runtime.append_finalized_turn(progress_turn(text), Vec::new());
    }

    /// Boot just the HTTP surface over a runtime we control, without a mind.
    /// Returns the bound address and the runtime so the test can inject turns
    /// directly.
    async fn boot() -> (String, std::sync::Arc<WaveRuntime>, tempfile::TempDir) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("MEMORY.md"), "Goal: ship the reactive server.\n").expect("mem");

        let (runtime, _inbox_rx) =
            WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open runtime");

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
    async fn finalized_turn_appears_in_conversation() {
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

    /// `?limit=N` tails the thread: the last N turns (open turn included),
    /// newest still last; a limit past the thread length serves everything.
    #[tokio::test]
    async fn conversation_limit_tails_the_thread() {
        let (base, runtime, _tmp) = boot().await;
        narrate(&runtime, "first");
        narrate(&runtime, "second");
        narrate(&runtime, "third");

        let body: serde_json::Value = reqwest::get(format!("{base}/conversation?limit=2"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let turns = body["turns"].as_array().unwrap();
        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0]["text"], "second");
        assert_eq!(turns[1]["text"], "third");

        // A limit past the thread length serves the whole thread; no limit
        // does too.
        for url in [
            format!("{base}/conversation?limit=99"),
            format!("{base}/conversation"),
        ] {
            let body: serde_json::Value = reqwest::get(url).await.unwrap().json().await.unwrap();
            assert_eq!(body["turns"].as_array().unwrap().len(), 3);
        }

        // The open turn counts as the newest turn in the tail.
        let mut sink = TurnSink::new(runtime.clone());
        let mut adapter = EventAdapter::new();
        feed(&mut adapter, &mut sink, ev_started());
        feed(&mut adapter, &mut sink, ev_text("in progress"));
        let body: serde_json::Value = reqwest::get(format!("{base}/conversation?limit=1"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let turns = body["turns"].as_array().unwrap();
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0]["status"], "running");
        assert_eq!(turns[0]["text"], "in progress");
    }

    #[tokio::test]
    async fn posted_message_appears_as_user_turn() {
        let (base, runtime, _tmp) = boot().await;

        let client = reqwest::Client::new();
        let body: serde_json::Value = client
            .post(format!("{base}/messages"))
            .json(&serde_json::json!({ "op": "message", "text": "how's it going?" }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let posted: ChatTurn = serde_json::from_value(body["turn"].clone()).unwrap();
        assert_eq!(posted.role, ChatRole::User);
        assert_eq!(posted.text, "how's it going?");
        assert_eq!(body["state"], "idle");

        // The message is in the thread; the mind answers it at its next turn
        // (mind scheduling is covered in mind.rs tests).
        let thread = runtime.thread_snapshot();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].role, ChatRole::User);
    }

    /// The say op: an attributed emission lands in the thread with its byline
    /// on the wire (`from`), and the attribution rules are enforced — say
    /// requires `from`, every other op rejects it.
    #[tokio::test]
    async fn posted_say_is_attributed_on_the_wire() {
        let (base, runtime, _tmp) = boot().await;
        let client = reqwest::Client::new();

        let body: serde_json::Value = client
            .post(format!("{base}/messages"))
            .json(&serde_json::json!({
                "op": "say",
                "text": "run-7 landed: PR #12",
                "from": { "session_id": "sess-7", "label": "worker" },
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(body["turn"]["from"], "worker");
        assert_eq!(body["turn"]["role"], "user");

        // The wire thread carries the byline; the mind's queue has the input.
        let conversation: serde_json::Value = reqwest::get(format!("{base}/conversation"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(conversation["turns"][0]["from"], "worker");
        // The queue fold treats the emission as consumable input.
        let (_, events) =
            journal::Journal::open(&journal::journal_path(runtime.repo_root(), "ship"))
                .expect("journal");
        assert_eq!(journal::fold_thread(&events).pending_messages.len(), 1);

        // Say without from, and from on a non-say op, are both refused.
        let missing_from = client
            .post(format!("{base}/messages"))
            .json(&serde_json::json!({ "op": "say", "text": "anon" }))
            .send()
            .await
            .unwrap();
        assert_eq!(missing_from.status(), reqwest::StatusCode::BAD_REQUEST);
        let stray_from = client
            .post(format!("{base}/messages"))
            .json(&serde_json::json!({
                "op": "message",
                "text": "hello",
                "from": { "session_id": null, "label": "cli" },
            }))
            .send()
            .await
            .unwrap();
        assert_eq!(stray_from.status(), reqwest::StatusCode::BAD_REQUEST);
    }

    /// The memory routes: GET serves the origin file; POST update/add write
    /// it and journal `MemoryUpdated` (covered in depth by the runtime and
    /// `lf memory` tests — this pins the HTTP shape).
    #[tokio::test]
    async fn memory_routes_read_and_write_through_the_server() {
        let (base, runtime, _tmp) = boot().await;
        let body: serde_json::Value = reqwest::get(format!("{base}/memory"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(body["content"], "Goal: ship the reactive server.\n");

        let client = reqwest::Client::new();
        let body: serde_json::Value = client
            .post(format!("{base}/memory"))
            .json(&serde_json::json!({
                "op": "update",
                "content": "Rewritten.\n",
                "summary": null,
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(body["summary"], "Rewritten.");
        assert_eq!(runtime.memory().read(), "Rewritten.\n");

        // An empty add is refused; a real one appends a bullet.
        let empty = client
            .post(format!("{base}/memory"))
            .json(&serde_json::json!({ "op": "add", "content": "  ", "summary": null }))
            .send()
            .await
            .unwrap();
        assert_eq!(empty.status(), reqwest::StatusCode::BAD_REQUEST);
        client
            .post(format!("{base}/memory"))
            .json(&serde_json::json!({ "op": "add", "content": "one fact", "summary": null }))
            .send()
            .await
            .unwrap();
        assert_eq!(runtime.memory().read(), "Rewritten.\n- one fact\n");
    }

    #[tokio::test]
    async fn post_without_op_or_without_text_is_rejected() {
        let (base, _runtime, _tmp) = boot().await;
        let client = reqwest::Client::new();

        // Op is required — no serde default, no inference.
        let no_op = client
            .post(format!("{base}/messages"))
            .json(&serde_json::json!({ "text": "hello" }))
            .send()
            .await
            .unwrap();
        assert_eq!(no_op.status(), reqwest::StatusCode::UNPROCESSABLE_ENTITY);

        // Text may be empty only for interrupt.
        for op in ["message", "steer"] {
            let empty = client
                .post(format!("{base}/messages"))
                .json(&serde_json::json!({ "op": op, "text": "  " }))
                .send()
                .await
                .unwrap();
            assert_eq!(
                empty.status(),
                reqwest::StatusCode::BAD_REQUEST,
                "empty text rejected for {op}"
            );
        }
    }

    #[tokio::test]
    async fn bare_interrupt_while_idle_is_a_noop_success_with_state() {
        let (base, runtime, _tmp) = boot().await;
        let client = reqwest::Client::new();
        let body: serde_json::Value = client
            .post(format!("{base}/messages"))
            .json(&serde_json::json!({ "op": "interrupt", "text": "" }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(body["turn"].is_null(), "nothing said, nothing appended");
        assert_eq!(body["state"], "idle");
        assert!(runtime.thread_snapshot().is_empty());
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

        /// Read until `pred` holds over every `state` frame received so far
        /// (panics after 5s). Returns the state names, in order.
        async fn states_until(&mut self, pred: impl Fn(&[String]) -> bool) -> Vec<String> {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
            let mut buf = [0u8; 4096];
            loop {
                let states = parse_state_frames(&dechunk(&self.raw));
                if pred(&states) {
                    return states;
                }
                match tokio::time::timeout_at(deadline, self.stream.read(&mut buf)).await {
                    Ok(Ok(0)) | Err(_) => {
                        panic!("SSE ended before condition; states so far: {states:?}")
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

    /// The `data:` of every `state` event, in order.
    fn parse_state_frames(sse_body: &str) -> Vec<String> {
        let mut states = Vec::new();
        let mut in_state_event = false;
        for line in sse_body.lines() {
            if let Some(name) = line.strip_prefix("event:") {
                in_state_event = name.trim() == "state";
            } else if let Some(data) = line.strip_prefix("data:") {
                if in_state_event {
                    states.push(data.trim().to_string());
                }
            }
        }
        states
    }

    /// Feed one harness event through the production delta pipeline.
    fn feed(adapter: &mut EventAdapter, sink: &mut TurnSink, event: ConversationEvent) {
        for delta in adapter.feed(&event) {
            sink.on_delta(delta);
        }
    }

    fn ev_started() -> ConversationEvent {
        ConversationEvent::TurnStarted {
            turn_id: "vt".into(),
        }
    }

    fn ev_text(text: &str) -> ConversationEvent {
        ConversationEvent::ItemCompleted {
            turn_id: "vt".into(),
            item: ConversationItem::Message {
                id: "m".into(),
                text: text.into(),
                phase: None,
            },
        }
    }

    fn ev_completed() -> ConversationEvent {
        ConversationEvent::TurnCompleted {
            turn_id: "vt".into(),
            status: Lifecycle::Completed,
        }
    }

    fn ev_usage() -> ConversationEvent {
        ConversationEvent::TurnUsage {
            turn_id: "vt".into(),
            usage: TurnUsage {
                input_tokens: 1,
                output_tokens: 1,
                reasoning_tokens: None,
                cache_read_tokens: None,
                cache_write_tokens: None,
                model: None,
                cost_usd: None,
            },
        }
    }

    #[tokio::test]
    async fn sse_late_subscriber_watches_the_open_turn_grow_and_finalize() {
        let (base, runtime, _tmp) = boot().await;
        narrate(&runtime, "already finalized");

        // A turn is mid-flight before the client connects.
        let mut sink = TurnSink::new(runtime.clone());
        let mut adapter = EventAdapter::new();
        feed(&mut adapter, &mut sink, ev_started());
        feed(&mut adapter, &mut sink, ev_text("thinking"));

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
            .find(|t| t.status == Lifecycle::Running && t.text == "thinking")
            .unwrap()
            .clone();

        // Re-broadcast: the same id grows in place.
        feed(&mut adapter, &mut sink, ev_text("more"));
        client
            .frames_until(|f| {
                f.iter().any(|t| {
                    t.id == open.id && t.text == "thinking\nmore" && t.status == Lifecycle::Running
                })
            })
            .await;

        // Finalization replaces it terminally, same id.
        feed(&mut adapter, &mut sink, ev_completed());
        feed(&mut adapter, &mut sink, ev_usage());
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
    async fn sse_state_events_track_the_mind_live() {
        let (base, runtime, _tmp) = boot().await;

        // Subscribe while idle: the first frame is the current state.
        let mut client = SseClient::connect(&base).await;
        let states = client.states_until(|s| !s.is_empty()).await;
        assert_eq!(states, vec!["idle"]);

        // A turn opens → `turning` arrives live; finalization → `idle`.
        let mut sink = TurnSink::new(runtime.clone());
        let mut adapter = EventAdapter::new();
        feed(&mut adapter, &mut sink, ev_started());
        let states = client.states_until(|s| s.len() >= 2).await;
        assert_eq!(states, vec!["idle", "turning"]);

        feed(&mut adapter, &mut sink, ev_completed());
        feed(&mut adapter, &mut sink, ev_usage());
        let states = client.states_until(|s| s.len() >= 3).await;
        assert_eq!(states, vec!["idle", "turning", "idle"]);
    }

    #[tokio::test]
    async fn conversation_includes_the_open_running_turn() {
        let (base, runtime, _tmp) = boot().await;
        let mut sink = TurnSink::new(runtime.clone());
        let mut adapter = EventAdapter::new();
        feed(&mut adapter, &mut sink, ev_started());
        feed(&mut adapter, &mut sink, ev_text("half a thought"));

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
        feed(&mut adapter, &mut sink, ev_completed());
        feed(&mut adapter, &mut sink, ev_usage());
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
            let mut adapter = EventAdapter::new();
            feed(&mut adapter, &mut sink, ev_started());
            feed(&mut adapter, &mut sink, ev_text("half a thought"));
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
        let cwd = repo.clone();
        // Keep the events sender alive so the stub mind doesn't see a closed
        // stream.
        let (_events_tx, events_rx) = mpsc::unbounded_channel();
        let handle = tokio::spawn(async move {
            serve(
                repo2,
                cwd,
                "ship".into(),
                Box::new(StubHarness),
                events_rx,
                None,
                false,
                async {
                    let _ = shutdown_rx.await;
                },
            )
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

    /// No registry store on the machine: the boot degrades to unregistered
    /// (warn-and-continue), never an error — the pre-registry status quo.
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn resolve_registry_without_a_store_runs_unregistered() {
        let _env = crate::lf::session::test_env_lock();
        let tmp = tempfile::tempdir().expect("tempdir");
        let previous = std::env::var_os("LFD_DB_PATH");
        std::env::set_var("LFD_DB_PATH", tmp.path().join("absent.db"));
        let config = resolve_registry(tmp.path(), "ship", tmp.path(), false).await;
        match previous {
            Some(value) => std::env::set_var("LFD_DB_PATH", value),
            None => std::env::remove_var("LFD_DB_PATH"),
        }
        assert!(config.is_none(), "missing store boots unregistered");
    }

    /// A stale pointer (its server is gone) never blocks a boot: the probe
    /// finds nothing live and the new server overwrites the file.
    #[tokio::test]
    async fn serve_overwrites_a_stale_endpoint_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("wave/ship")).unwrap();
        let repo = tmp.path().to_path_buf();

        // A dead address: bind, learn the port, drop the listener.
        let dead = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dead_addr = dead.local_addr().unwrap();
        drop(dead);
        server::write_endpoint(&repo, "ship", dead_addr).expect("stale pointer");

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let (_events_tx, events_rx) = mpsc::unbounded_channel();
        let repo2 = repo.clone();
        let cwd = repo.clone();
        let handle = tokio::spawn(async move {
            serve(
                repo2,
                cwd,
                "ship".into(),
                Box::new(StubHarness),
                events_rx,
                None,
                false,
                async {
                    let _ = shutdown_rx.await;
                },
            )
            .await
        });

        let endpoint = server::endpoint_path(&repo, "ship");
        wait_for(|| {
            std::fs::read_to_string(&endpoint)
                .is_ok_and(|contents| contents != dead_addr.to_string())
        })
        .await;

        shutdown_tx.send(()).unwrap();
        handle.await.unwrap().unwrap();
    }

    /// The file-level one-brain floor: a pointer answering /health for this
    /// wave refuses a second server, registry store or not.
    #[tokio::test]
    async fn serve_refuses_to_start_over_a_live_endpoint_file() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("wave/ship")).unwrap();
        let repo = tmp.path().to_path_buf();

        // First server, unregistered (no store): boots and writes the pointer.
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let (_events_tx, events_rx) = mpsc::unbounded_channel();
        let repo2 = repo.clone();
        let cwd = repo.clone();
        let first = tokio::spawn(async move {
            serve(
                repo2,
                cwd,
                "ship".into(),
                Box::new(StubHarness),
                events_rx,
                None,
                false,
                async {
                    let _ = shutdown_rx.await;
                },
            )
            .await
        });
        let endpoint = server::endpoint_path(&repo, "ship");
        wait_for(|| endpoint.exists()).await;
        let first_addr = std::fs::read_to_string(&endpoint).unwrap();

        // Second server: probed live, refused, pointer untouched.
        let (_shutdown_tx2, shutdown_rx2) = tokio::sync::oneshot::channel::<()>();
        let (_events_tx2, events_rx2) = mpsc::unbounded_channel();
        let err = serve(
            repo.clone(),
            repo.clone(),
            "ship".into(),
            Box::new(StubHarness),
            events_rx2,
            None,
            false,
            async {
                let _ = shutdown_rx2.await;
            },
        )
        .await
        .expect_err("live endpoint refuses a second server");
        assert!(
            err.to_string().contains("--force"),
            "error points at --force: {err}"
        );
        assert_eq!(
            std::fs::read_to_string(&endpoint).unwrap(),
            first_addr,
            "refused boot never touched the pointer"
        );

        shutdown_tx.send(()).unwrap();
        first.await.unwrap().unwrap();
        assert!(!endpoint.exists(), "first server still owns its shutdown");
    }

    fn make_wave_row(name: &str) -> crate::lfd::types::Wave {
        use crate::lfd::types::{RepoWork, WaveMode, WaveStatus};
        crate::lfd::types::Wave {
            id: crate::lfd::id::LfdId::new(),
            name: name.to_string(),
            mode: WaveMode::Loop,
            primary_flow: "ship-roadmap".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            crons: Vec::new(),
            repos: vec![RepoWork {
                repo: "/tmp/repo".to_string(),
                worktree: String::new(),
                branch: String::new(),
                status: WaveStatus::Idle,
                iteration: 0,
                cycle_start_iteration: 0,
                position: 0,
            }],
            direction: Vec::new(),
            area: Vec::new(),
            paused: false,
            created_at: Some(time::OffsetDateTime::now_utc()),
            workers: 1,
            parent_wave_id: None,
        }
    }

    /// The registry seat end to end through `serve`: the WaveAgent row is
    /// live while the server runs and marked terminal by graceful shutdown.
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn serve_registers_the_brain_and_deregisters_on_shutdown() {
        // serve() exports the wave-context env on registration; serialize
        // with everything else that touches process env.
        let _env = crate::lf::session::test_env_lock();
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("wave/ship")).unwrap();
        let repo = tmp.path().to_path_buf();
        let store: crate::lfd::store::SharedStore = Arc::new(
            crate::lfd::store::open_store(&crate::lfd::store::StorageConfig::sqlite(
                tmp.path().join("lfd.db"),
            ))
            .await
            .expect("open store"),
        );
        let wave_row = make_wave_row("ship");
        store.create_wave(&wave_row).await.expect("seed wave");
        let wave_id = wave_row.id().clone();

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let (_events_tx, events_rx) = mpsc::unbounded_channel();
        let repo2 = repo.clone();
        let cwd = repo.clone();
        let config = registry::RegistryConfig {
            store: store.clone(),
            wave: wave_row,
            cwd: repo.display().to_string(),
            pid: std::process::id(),
            force: false,
        };
        let handle = tokio::spawn(async move {
            serve(
                repo2,
                cwd,
                "ship".into(),
                Box::new(StubHarness),
                events_rx,
                Some(config),
                false,
                async {
                    let _ = shutdown_rx.await;
                },
            )
            .await
        });

        let endpoint = server::endpoint_path(&repo, "ship");
        wait_for(|| endpoint.exists()).await;

        // Registered: the row is live and carries this server's endpoint.
        let live = store
            .live_wave_agent_session(&wave_id)
            .await
            .expect("live lookup")
            .expect("brain registered");
        let addr = std::fs::read_to_string(&endpoint).unwrap();
        assert_eq!(
            live.env
                .get(crate::lfd::types::WAVE_SERVER_ENDPOINT_ENV)
                .map(String::as_str),
            Some(addr.trim())
        );

        shutdown_tx.send(()).unwrap();
        handle.await.unwrap().unwrap();
        assert!(!endpoint.exists(), "pointer removed on shutdown");
        assert!(
            store
                .live_wave_agent_session(&wave_id)
                .await
                .expect("live lookup")
                .is_none(),
            "graceful shutdown deregisters the brain"
        );
    }

    /// One brain per wave: a live registered server refuses a second serve.
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn serve_refuses_to_start_over_a_live_brain() {
        let _env = crate::lf::session::test_env_lock();
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("wave/ship")).unwrap();
        let store: crate::lfd::store::SharedStore = Arc::new(
            crate::lfd::store::open_store(&crate::lfd::store::StorageConfig::sqlite(
                tmp.path().join("lfd.db"),
            ))
            .await
            .expect("open store"),
        );
        let wave_row = make_wave_row("ship");
        store.create_wave(&wave_row).await.expect("seed wave");

        // A live brain, registered as another `lf wave` would be.
        let first = registry::RegistryConfig {
            store: store.clone(),
            wave: wave_row.clone(),
            cwd: "/tmp/repo.ship".to_string(),
            pid: std::process::id(),
            force: false,
        };
        let registry::RegisterOutcome::Registered(_live) =
            registry::register(&first, "127.0.0.1:9")
                .await
                .expect("register")
        else {
            panic!("first registration succeeds");
        };

        let (_shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let (_events_tx, events_rx) = mpsc::unbounded_channel();
        let err = serve(
            tmp.path().to_path_buf(),
            tmp.path().to_path_buf(),
            "ship".into(),
            Box::new(StubHarness),
            events_rx,
            Some(registry::RegistryConfig {
                store,
                wave: wave_row,
                cwd: "/tmp/repo.ship".to_string(),
                pid: std::process::id(),
                force: false,
            }),
            false,
            async {
                let _ = shutdown_rx.await;
            },
        )
        .await
        .expect_err("second brain refused");
        assert!(
            err.to_string().contains("--force"),
            "error points at --force: {err}"
        );
    }

    /// `lf wave` self-bootstraps: the wave's `<repo>.<wave>` sibling worktree
    /// is created on first boot and reused after — the mind never runs in the
    /// main checkout.
    #[test]
    fn wave_worktree_creates_and_reuses_the_sibling_tree() {
        let repo = loopflow_test_support::TestRepo::new();

        let created = wave_worktree(repo.path(), "ship").expect("bootstrap worktree");
        let repo_name = repo
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .expect("repo name");
        assert_eq!(
            created.file_name().and_then(|name| name.to_str()),
            Some(format!("{repo_name}.ship").as_str()),
            "wave worktree is the <repo>.<wave> sibling"
        );
        assert!(created.join(".git").exists(), "worktree is a checkout");

        let reused = wave_worktree(repo.path(), "ship").expect("reuse worktree");
        assert_eq!(reused, created, "second boot reuses the same tree");
    }
}
