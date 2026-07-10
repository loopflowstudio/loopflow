//! `lf loop <name>` — one mind implemented as a listener and resident pair.
//!
//! The listener (this module's `serve`) is the channel made durable — pure
//! hear / check / fold / tell, vendor-free:
//!
//! - holds every journal pen (the wave channel + the family's work lines);
//! - serves the doors: `/messages`, `/events`, `/memory`, `/health`,
//!   `/channels`, and the token-gated resident door ([`server`]);
//! - folds the store's worker facts ([`registry::StoreObserver`]);
//! - keeps the registry seat and the discovery pointer;
//! - supervises the resident ([`supervisor`]): process liveness, the respawn
//!   ladder, the interrupt janitor.
//!
//! The resident ([`resident`]) owns the
//! pass scheduler, runs in the wave's `<repo>.<wave>` worktree, consumes its
//! own wave's `/events?inbox=true` subscription, and publishes ordered turn
//! deltas back through the resident door. The wire between them is [`wire`].
//!
//! ```text
//!   lf loop <name>                      internal resident invocation
//!   ┌───────────────────────┐  spawns   ┌──────────────────────────┐
//!   │ LISTENER (origin repo)│──────────▶│ RESIDENT (<repo>.<wave>) │
//!   │ pens · folds · doors  │           │ pass scheduler           │
//!   │ observer · supervisor │◀──deltas──│ seed · queue             │
//!   └──────────┬────────────┘           └────────────▲─────────────┘
//!              └────────── /events?inbox=true ───────┘
//! ```
//!
//! `lf loop <name>` boots the listener and spawns the resident as the same
//! command carrying private endpoint/token environment. The split is runtime
//! plumbing, not a second product surface.
//!
//! Truth is the per-wave append-only [`journal`] (JSONL under `.lf/journal/
//! waves/<name>/` in the ORIGIN repo — the listener serves from the origin
//! and no longer creates worktrees); the in-process state
//! ([`runtime::WaveRuntime`]) is a fold of it, rebuilt on boot so a restart
//! keeps the whole conversation. The journal is listener-owned persistence;
//! the resident never touches journal files. The only coordination files are
//! dumb discovery: `wave/<name>/.wave-endpoint` and, beside it, this boot's
//! `.wave-resident-token`.
//!
//! The listener also keeps a best-effort seat in the shared session
//! [`registry`] (the same local store lfd serves from — the db IS the
//! registry): a `WaveAgent` session row registered store-direct at boot (one
//! brain per wave, enforced by a pid-probing pre-flight) and a store-polling
//! observer that journals `RunObserved`/`RunCompleted` observations. No
//! registry store on the machine → warn once, fully functional anyway.

pub(crate) mod channel;
pub mod journal;
pub(crate) mod memory;
pub mod playhead;

pub(crate) mod registry;
pub mod resident;
pub mod runtime;
pub mod server;
pub mod state;
pub mod subscription;
pub(crate) mod supervisor;
pub mod wire;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::engine::repo::find_repo_root;
use crate::engine::worktrees::main_repo_root;
use crate::lfd::types::WAVE_SERVER_ENDPOINT_ENV;
use crate::lfdb::{open_existing_store, SharedStore};
use crate::ops::util::resolve_wave_name;
use crate::wave::runtime::WaveRuntime;

/// Whether the listener spawns (and supervises) a resident.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoopPolicy {
    /// Spawn the internal resident child and keep it alive
    /// (respawn ladder, immediate respawn on a human message).
    Spawn,
    /// Test-only listener with no resident.
    #[cfg(test)]
    Dormant,
}

/// The hidden subcommand a listener spawns for its own resident body. Named
/// here so the spawner and the CLI cannot drift apart silently.
pub(crate) const RESIDENT_SUBCOMMAND: &str = "__resident";

/// `lf serve <name>` — boot the named mind's listener and supervise its
/// resident. The steerable half: an endpoint, a thread, a cadence.
///
/// The listener spawns its resident body as `lf __resident <name>`. That is an
/// explicit command, not an ambient one: an earlier design branched here on
/// whether the resident endpoint/token were present in env, which meant any
/// process holding a parent's env — a tmux child, a promoted subwave — booted
/// the wrong half by accident.
pub fn serve(name: &str, force: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root).unwrap_or_else(|_| repo_root.clone());
    let wave = resolve_wave_name(&main_repo, Some(name))
        .ok_or_else(|| anyhow!("invalid wave name: '{name}'"))?;
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let registry_config = resolve_registry(&main_repo, &wave, force).await;
        run_listener(
            main_repo,
            wave,
            registry_config,
            force,
            LoopPolicy::Spawn,
            shutdown_signal(),
        )
        .await
    })
}

/// `lf __resident <name>` — the resident body a listener spawns for its own
/// wave. Attaches to the endpoint/token its parent listener put in env.
pub fn resident(name: &str) -> Result<()> {
    resident::run(name)
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
            cwd: main_repo.display().to_string(),
            pid: std::process::id(),
            force,
        }),
        Err(err) => {
            tracing::warn!(wave, error = %err, "session registry unusable; running unregistered");
            None
        }
    }
}

/// The production resident spawner: `lf loop <wave>`, run by this
/// same executable, endpoint + token + wave-session context in env. The
/// resident's stdout/stderr inherit — one `lf serve` terminal shows both
/// halves, today's UX.
// TODO(M1): keep this shutdown contract in the wave/supervisor owner: stand
// the respawn ladder down before terminating the resident, honor interrupt
// cleanup, and keep SIGKILL deadlines in the supervisor path.
fn resident_spawner(
    wave: String,
    repo_root: PathBuf,
    endpoint: String,
    token: String,
    subagent_token: String,
    session_env: Vec<(String, String)>,
) -> supervisor::SpawnResident {
    Box::new(move || {
        let exe = std::env::current_exe()?;
        let mut command = tokio::process::Command::new(exe);
        command
            .arg(RESIDENT_SUBCOMMAND)
            .arg(&wave)
            .current_dir(&repo_root)
            .env(WAVE_SERVER_ENDPOINT_ENV, &endpoint)
            .env(wire::RESIDENT_TOKEN_ENV, &token)
            // The per-subagent exec-door token, inherited by every sandboxed
            // process the resident spawns: a worker reads it (with the
            // endpoint above) and runs `lfq exec <lf argv>` back through the
            // wave's `/v0/exec`, unsandboxed in the outwave.
            .env(wire::SUBAGENT_TOKEN_ENV, &subagent_token)
            // The resident's children must resolve `lf` to this binary.
            .env("PATH", crate::flowloop::wave::path_for_children())
            .stdin(std::process::Stdio::null());
        // No kill_on_drop: shutdown stops the supervisor FIRST (so a TERM'd
        // resident's exit isn't journaled as a failure), then SIGTERMs the
        // resident by pid — its hooks stop the vendor process group. A
        // SIGKILL-on-drop here would orphan the codex group instead.
        for (key, value) in &session_env {
            command.env(key, value);
        }
        command.spawn()
    })
}

/// Serve the wave until `shutdown` resolves. Vendor-free by construction:
/// no harness, no vendor process — the resident (spawned per `loop_policy`, or
/// attached by hand) owns those. `registry_config` is `None` in tests that
/// exercise the server without a registry store; `force` rides separately
/// because the endpoint-file floor must honor it even when there is no
/// registry config at all.
async fn run_listener(
    repo_root: PathBuf,
    wave: String,
    registry_config: Option<registry::RegistryConfig>,
    force: bool,
    loop_policy: LoopPolicy,
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

    // Bind before opening the journal for writing: the registry pre-flight
    // needs this boot's endpoint, and a refused start must scribble NOTHING —
    // no journal opened, no ServerStarted appended. Binding a loopback port
    // writes nothing.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;

    // Registry seat: write the WaveAgent row (one-brain pre-flight — a live
    // brain refuses the start unless --force) BEFORE the journal opens, so a
    // refusal exits having written nothing. Best-effort by design: a store
    // failure degrades to the registry-less status quo. `registered` carries
    // the store + wave id forward to build the observer once the runtime is
    // open.
    let mut registration: Option<registry::Registration> = None;
    let mut registered: Option<(SharedStore, crate::lfd::id::LfdId)> = None;
    // Wave-session context handed to the spawned resident: a bare `lf`
    // inside the loop self-registers under this wave with the listener's
    // session as its parent (see lf::session for the env contract).
    let mut session_env: Vec<(String, String)> = Vec::new();
    if let Some(config) = registry_config {
        match registry::register(&config, &addr.to_string()).await {
            Ok(registry::RegisterOutcome::Registered(reg)) => {
                let reg = *reg;
                tracing::info!(
                    wave,
                    session_id = %reg.session_id(),
                    "registered in the session registry as the wave's agent session"
                );
                session_env = vec![
                    (
                        crate::lf::session::WAVE_ID_ENV.to_string(),
                        config.wave.id().to_string(),
                    ),
                    (
                        crate::lf::session::SESSION_ID_ENV.to_string(),
                        reg.session_id().to_string(),
                    ),
                    (
                        crate::lf::session::SESSION_INHERITED_ENV.to_string(),
                        "1".to_string(),
                    ),
                ];
                // Ctrl+C exits the process before the graceful path below
                // runs, so deregister from the interrupt hook too; the
                // once-guard makes whichever path runs first the only one.
                let hook_registration = reg.clone();
                crate::engine::agent::register_interrupt_cleanup(move || {
                    hook_registration.deregister_blocking();
                });
                registration = Some(reg);
                registered = Some((config.store.clone(), config.wave.id().clone()));
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

    // Refusals are behind us: NOW open the journal for writing and mark the
    // boot. The store-polling observer starts once the runtime exists.
    let runtime = WaveRuntime::open(wave.clone(), repo_root.clone())?;
    // Boot marker, once per life, after replay: restarts are visible in the
    // journal itself (the boot janitor already leaks process lifecycle into
    // the record; make it honest and forensically legible).
    runtime.journal_server_started(std::process::id(), &addr.to_string());

    let mut observer: Option<Arc<registry::StoreObserver>> = None;
    let mut observer_task: Option<tokio::task::JoinHandle<()>> = None;
    if let Some((store, wave_id)) = registered {
        let obs = Arc::new(registry::StoreObserver::new(
            runtime.clone(),
            store,
            wave_id,
        ));
        observer_task = Some(tokio::spawn(Arc::clone(&obs).run(registry::POLL_CADENCE)));
        observer = Some(obs);
    }

    // The resident door: a per-boot token, published beside the endpoint
    // pointer so the resident can attach (same trust domain).
    let token = server::generate_resident_token();
    let door = server::ResidentDoor::new(token.clone());
    server::write_resident_token(&repo_root, &wave, &token)?;

    // The exec door's authority: a per-subagent capability set. Minted into
    // when the resident is spawned (below) and validated on `/v0/exec`. In
    // memory, per boot — no store, no schema. Listener-only tests spawn no
    // resident, so the set stays empty and `/exec` accepts nothing.
    let subagent_door = server::SubagentDoor::new();

    // The keeper's watch: resident liveness, respawn ladder, interrupt
    // janitor. Runs even without a spawner — the pen-side anti-wedges (janitor, attach
    // probe) never depend on who spawned the resident.
    let spawner = match loop_policy {
        LoopPolicy::Spawn => Some(resident_spawner(
            wave.clone(),
            repo_root.clone(),
            addr.to_string(),
            token.clone(),
            subagent_door.mint(),
            session_env,
        )),
        #[cfg(test)]
        LoopPolicy::Dormant => None,
    };
    // Build the supervisor before spawning so the attach door can hold its
    // handle: an attached resident signals the keeper to
    // stand the respawn ladder down, so the deadline never spawns a second
    // loop over it.
    let supervisor = supervisor::Supervisor::new(
        runtime.clone(),
        door.clone(),
        spawner,
        supervisor::SupervisorConfig::default(),
    );
    let supervisor_handle = supervisor.handle();
    let supervisor_task = tokio::spawn(supervisor.run());

    server::write_endpoint(&repo_root, &wave, addr)?;
    // Ctrl+C exits the process before graceful shutdown runs, so clean up
    // from the interrupt handler too: SIGTERM the resident (its hooks stop
    // the vendor process group) and remove the discovery files — only while
    // they still hold OUR address/token (a takeover's stay).
    let own_addr = addr.to_string();
    let cleanup_repo = repo_root.clone();
    let cleanup_wave = wave.clone();
    let cleanup_addr = own_addr.clone();
    let cleanup_token = token.clone();
    let cleanup_door = door.clone();
    crate::engine::agent::register_interrupt_cleanup(move || {
        if let Some(pid) = cleanup_door.seat_pid() {
            supervisor::terminate_resident_blocking(pid);
        }
        server::remove_endpoint(&cleanup_repo, &cleanup_wave, &cleanup_addr);
        server::remove_resident_token(&cleanup_repo, &cleanup_wave, &cleanup_token);
    });
    println!(
        "lf serve · {wave} · listener on http://{addr}{} \
         (Ctrl-C to stop, RUST_LOG=loopflow=debug for the firehose)",
        match loop_policy {
            LoopPolicy::Spawn => " · spawning resident",
            #[cfg(test)]
            LoopPolicy::Dormant => " · dormant test listener",
        }
    );

    let app = server::router(
        runtime.clone(),
        door.clone(),
        subagent_door,
        observer,
        Some(supervisor_handle),
    );
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await;

    // Shutdown: stand the keeper down FIRST (so the resident's exit below
    // is not journaled as a failure), then ask the resident to leave
    // (SIGTERM → its interrupt hooks stop the harness; SIGKILL after a
    // grace), mark the registry row terminal, drop the discovery files.
    // Workers are their own tmux sessions — nothing here owns them.
    supervisor_task.abort();
    if let Some(pid) = door.seat_pid() {
        supervisor::terminate_resident(pid).await;
    }
    if let Some(task) = observer_task {
        task.abort();
    }
    if let Some(registration) = registration {
        registration.deregister().await;
    }
    server::remove_endpoint(&repo_root, &wave, &own_addr);
    server::remove_resident_token(&repo_root, &wave, &token);

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

    use crate::chat::turns::{ChatRole, ChatTurn};
    use crate::chat::types::Lifecycle;
    use crate::wave::journal::MessageOp;
    use crate::wave::server::ResidentDoor;
    use crate::wave::wire::{ResidentDelta, RESIDENT_TOKEN_HEADER};

    /// Serving a mind and looping a flow are different entrypoints, and the
    /// listener spawns its resident body by name. Nothing here reads the
    /// environment: an `lf` process inheriting `WAVE_SERVER_ENDPOINT` and
    /// `RESIDENT_TOKEN` — a tmux child, a promoted subwave — becomes whichever
    /// half its argv says, not whichever half its parent happened to be.
    #[test]
    fn the_listener_spawns_its_resident_body_by_name() {
        use crate::lf::{Cli, Commands};
        use clap::Parser;

        let cli = Cli::try_parse_from(["lf", RESIDENT_SUBCOMMAND, "goals"])
            .expect("the spawner's subcommand must be one the CLI accepts");
        assert!(matches!(
            cli.command,
            Some(Commands::Resident { name }) if name == "goals"
        ));

        // The batch verb cannot name a wave, so the spawner could not have
        // reached the resident half through `lf loop` even by accident.
        assert!(Cli::try_parse_from(["lf", "loop", "goals"]).is_err());
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
            body: None,
        }
    }

    /// Inject a finalized assistant turn, as a completed loop turn would land.
    fn narrate(runtime: &WaveRuntime, text: &str) {
        runtime.append_finalized_turn(progress_turn(text), Vec::new());
    }

    /// Boot just the HTTP surface over a runtime we control, without a
    /// resident. Returns the bound address and the runtime so the test can
    /// inject turns directly.
    async fn boot() -> (String, Arc<WaveRuntime>, tempfile::TempDir) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("MEMORY.md"), "Goal: ship the reactive server.\n").expect("mem");

        let runtime = WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open");

        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = server::router(
            runtime.clone(),
            ResidentDoor::new("test-token"),
            server::SubagentDoor::new(),
            None,
            None,
        );
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
        runtime.apply_resident_delta(ResidentDelta::TurnOpened {
            answers: Vec::new(),
        });
        runtime.apply_resident_delta(ResidentDelta::TurnText {
            text: "in progress".into(),
        });
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

        // The message is in the thread; the resident answers it at its next
        // turn (loop scheduling is covered in loop/wave.rs tests).
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

        // The wire thread carries the byline; the pending queue has the input.
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

    /// The memory routes: GET serves the origin file; POST update writes it,
    /// and POST add publishes one replayable fact without mutating the compiled
    /// file (covered in depth by the runtime and `lf memory` tests — this pins
    /// the HTTP shape).
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

        // An empty add is refused; a real one echoes and leaves MEMORY.md alone.
        let empty = client
            .post(format!("{base}/memory"))
            .json(&serde_json::json!({ "op": "add", "content": "  ", "summary": null }))
            .send()
            .await
            .unwrap();
        assert_eq!(empty.status(), reqwest::StatusCode::BAD_REQUEST);

        let body: serde_json::Value = client
            .post(format!("{base}/memory"))
            .json(&serde_json::json!({ "op": "add", "content": "one fact", "summary": null }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(body["summary"], "one fact");
        assert_eq!(runtime.memory().read(), "Rewritten.\n");
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

    /// `/health` splits channel liveness from the resident: `status` says
    /// the channel serves; `loop_state` is null while no resident was ever spawned
    /// or attached, then carries the resident's
    /// state — a dead resident on a live channel reads `serving` + `failed`.
    #[tokio::test]
    async fn health_splits_channel_liveness_from_the_loop() {
        let (base, runtime, _tmp) = boot().await;
        narrate(&runtime, "first");

        // Dormant: no resident ever — loop is null, the channel serves.
        let body: serde_json::Value = reqwest::get(format!("{base}/health"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(body["status"], "serving", "status is channel liveness");
        assert!(body["loop_state"].is_null(), "dormant channel has no loop");
        assert_eq!(body["wave"], "ship");
        assert_eq!(body["turns"], 1);

        // A resident exists: loop reports its state.
        runtime.set_resident_expected();
        let body: serde_json::Value = reqwest::get(format!("{base}/health"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(body["loop_state"], "idle", "loop is the resident's state");

        // The resident dies; the channel keeps serving.
        runtime.transition(
            crate::wave::state::LoopState::Failed {
                reason: "vendor gone".into(),
            },
            "test",
        );
        let body: serde_json::Value = reqwest::get(format!("{base}/health"))
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(body["status"], "serving");
        assert_eq!(body["loop_state"], "failed");
    }

    /// The resident door end to end over HTTP: auth gates on the token,
    /// attach registers + revives, deltas fold into the journal and the
    /// thread, and the context door serves the pre-turn snapshot.
    #[tokio::test]
    async fn resident_door_gates_attaches_and_applies_deltas() {
        let (base, runtime, _tmp) = boot().await;
        let client = reqwest::Client::new();

        // No token (or a wrong one): 401, and nothing changes.
        for request in [
            client
                .post(format!("{base}/resident/attach"))
                .json(&serde_json::json!({ "pid": 1234 })),
            client
                .post(format!("{base}/resident/attach"))
                .header(RESIDENT_TOKEN_HEADER, "wrong")
                .json(&serde_json::json!({ "pid": 1234 })),
        ] {
            let denied = request.send().await.unwrap();
            assert_eq!(denied.status(), reqwest::StatusCode::UNAUTHORIZED);
        }
        assert!(!runtime.resident_expected());

        // A failed loop + attach: the fresh resident IS the revival.
        runtime.set_resident_expected();
        runtime.transition(
            crate::wave::state::LoopState::Failed {
                reason: "old resident died".into(),
            },
            "test",
        );
        let attach: serde_json::Value = client
            .post(format!("{base}/resident/attach"))
            .header(RESIDENT_TOKEN_HEADER, "test-token")
            .json(&serde_json::json!({ "pid": std::process::id() }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(attach["wave"], "ship");
        assert_eq!(runtime.loop_state().name(), "idle", "attach revives");

        // Deltas through the door: a whole turn, in order, one batch.
        let deltas: serde_json::Value = client
            .post(format!("{base}/resident/deltas"))
            .header(RESIDENT_TOKEN_HEADER, "test-token")
            .json(&serde_json::json!({ "deltas": [
                { "kind": "turn_opened", "answers": [] },
                { "kind": "turn_text", "text": "over the wire" },
                { "kind": "turn_usage", "input_tokens": 7, "output_tokens": 3, "cache_read_tokens": null },
                { "kind": "turn_finished", "status": "completed", "cost_usd": 0.01 },
            ] }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(deltas["accepted"], 4);
        let thread = runtime.thread_snapshot();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].text, "over the wire");
        assert_eq!(thread[0].status, Lifecycle::Completed);

        // The context door serves the in-flight fold.
        runtime.journal_run_observed("run-1", "sess-1", "implement", "wire it");
        let context: serde_json::Value = client
            .get(format!("{base}/resident/context"))
            .header(RESIDENT_TOKEN_HEADER, "test-token")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

        assert_eq!(context["in_flight"][0]["run_id"], "run-1");
        assert_eq!(context["in_flight"][0]["flow"], "implement");
    }

    #[tokio::test]
    async fn sse_replays_on_connect_then_streams_live() {
        let (base, runtime, _tmp) = boot().await;
        narrate(&runtime, "replayed turn");

        let host = base.strip_prefix("http://").unwrap().to_string();
        let mut stream = tokio::net::TcpStream::connect(&host).await.unwrap();
        stream
            .write_all(b"GET /events HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n")
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
        assert!(
            !acc.contains("event: inbox"),
            "the default stream carries no inbox frames"
        );
    }

    /// The resident's subscription scope: `?inbox=true` replays the pending
    /// queue as `inbox` frames and streams live ops (bare interrupts
    /// included, as their own `kind`-tagged control frame).
    #[tokio::test]
    async fn events_inbox_scope_replays_pending_and_streams_ops() {
        let (base, runtime, _tmp) = boot().await;
        runtime.deliver_user_message("queued before".into(), MessageOp::Message);

        let host = base.strip_prefix("http://").unwrap().to_string();
        let mut stream = tokio::net::TcpStream::connect(&host).await.unwrap();
        stream
            .write_all(
                b"GET /events?inbox=true HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n",
            )
            .await
            .unwrap();

        // Wait for the replay first — it proves the subscription is live —
        // THEN deliver the (live-only, unjournaled) bare interrupt.
        let mut acc = String::new();
        let mut buf = [0u8; 4096];
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        let mut interrupt_sent = false;
        loop {
            if acc.contains("queued before") && !interrupt_sent {
                interrupt_sent = true;
                runtime.deliver_interrupt();
            }
            let read = tokio::time::timeout_at(deadline, stream.read(&mut buf)).await;
            match read {
                Ok(Ok(0)) | Err(_) => break,
                Ok(Ok(n)) => {
                    acc.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if acc.contains("queued before") && acc.contains(r#""kind":"interrupt""#) {
                        break;
                    }
                }
                Ok(Err(_)) => break,
            }
        }
        assert!(acc.contains("event: inbox"), "inbox frames are named");
        assert!(
            acc.contains("queued before") && acc.contains(r#""op":"message""#),
            "the pending queue replays: {acc}"
        );
        assert!(
            acc.contains(r#""kind":"interrupt""#),
            "a live bare interrupt rides as a tagged control frame: {acc}"
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
                    b"GET /events HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n",
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

    #[tokio::test]
    async fn sse_late_subscriber_watches_the_open_turn_grow_and_finalize() {
        let (base, runtime, _tmp) = boot().await;
        narrate(&runtime, "already finalized");

        // A turn is mid-flight before the client connects.
        runtime.apply_resident_delta(ResidentDelta::TurnOpened {
            answers: Vec::new(),
        });
        runtime.apply_resident_delta(ResidentDelta::TurnText {
            text: "thinking".into(),
        });

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
        runtime.apply_resident_delta(ResidentDelta::TurnText {
            text: "more".into(),
        });
        client
            .frames_until(|f| {
                f.iter().any(|t| {
                    t.id == open.id && t.text == "thinkingmore" && t.status == Lifecycle::Running
                })
            })
            .await;

        // Finalization replaces it terminally, same id.
        runtime.apply_resident_delta(ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
            cost_usd: None,
            reason: None,
        });
        let frames = client
            .frames_until(|f| {
                f.iter()
                    .any(|t| t.id == open.id && t.status == Lifecycle::Completed)
            })
            .await;
        let last = frames.iter().rfind(|t| t.id == open.id).unwrap();
        assert_eq!(last.status, Lifecycle::Completed, "terminal frame is last");
        // Stream fragments concatenate exactly; no newline is welded between them.
        assert_eq!(last.text, "thinkingmore");
    }

    #[tokio::test]
    async fn sse_state_events_track_the_loop_live() {
        let (base, runtime, _tmp) = boot().await;

        // Subscribe while idle: the first frame is the current state.
        let mut client = SseClient::connect(&base).await;
        let states = client.states_until(|s| !s.is_empty()).await;
        assert_eq!(states, vec!["idle"]);

        // A turn opens → `turning` arrives live; finalization → `idle`.
        runtime.apply_resident_delta(ResidentDelta::TurnOpened {
            answers: Vec::new(),
        });
        let states = client.states_until(|s| s.len() >= 2).await;
        assert_eq!(states, vec!["idle", "turning"]);

        runtime.apply_resident_delta(ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
            cost_usd: None,
            reason: None,
        });
        let states = client.states_until(|s| s.len() >= 3).await;
        assert_eq!(states, vec!["idle", "turning", "idle"]);
    }

    #[tokio::test]
    async fn conversation_includes_the_open_running_turn() {
        let (base, runtime, _tmp) = boot().await;
        runtime.apply_resident_delta(ResidentDelta::TurnOpened {
            answers: Vec::new(),
        });
        runtime.apply_resident_delta(ResidentDelta::TurnText {
            text: "half a thought".into(),
        });

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
        runtime.apply_resident_delta(ResidentDelta::TurnFinished {
            status: Lifecycle::Completed,
            cost_usd: None,
            reason: None,
        });
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
            let runtime = WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("open");
            runtime.apply_resident_delta(ResidentDelta::TurnOpened {
                answers: Vec::new(),
            });
            runtime.apply_resident_delta(ResidentDelta::TurnText {
                text: "half a thought".into(),
            });
        }

        // Second life: journal replay + boot janitor close the crash tail.
        let runtime = WaveRuntime::open("ship".into(), tmp.path().to_path_buf()).expect("reopen");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = server::router(
            runtime.clone(),
            ResidentDoor::new("test-token"),
            server::SubagentDoor::new(),
            None,
            None,
        );
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

    /// Boot the HTTP surface over a wave. Child channels need no setup: they
    /// are names on the bus, not places on disk.
    async fn boot_family() -> (String, Arc<WaveRuntime>, tempfile::TempDir) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let origin = tmp.path().join("repo");
        std::fs::create_dir_all(origin.join("wave/ship")).expect("wave dir");
        let runtime = WaveRuntime::open("ship".into(), origin).expect("open runtime");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = server::router(
            runtime.clone(),
            ResidentDoor::new("test-token"),
            server::SubagentDoor::new(),
            None,
            None,
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        (format!("http://{addr}"), runtime, tmp)
    }

    /// Name-addressed chat: a `say` on a child channel is a report — the
    /// served wave records ONE attributed copy in its own journal and nowhere
    /// else. The byline is stamped from the channel, so a forged `from` in the
    /// body never survives. Only foreign names 404; a work line with no
    /// worktree is just a name.
    #[tokio::test]
    async fn a_child_say_is_recorded_once_in_the_waves_journal() {
        let (base, runtime, tmp) = boot_family().await;
        let client = reqwest::Client::new();

        let body: serde_json::Value = client
            .post(format!("{base}/messages"))
            .json(&serde_json::json!({
                "op": "say",
                "text": "landed the parser",
                "from": { "session_id": null, "label": "ship" },
                "channel": "ship.148e",
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(body["turn"]["text"], "landed the parser");

        // The report reached the wave's thread, bylined with the channel —
        // the forged "ship" byline did not survive.
        let thread = runtime.thread_snapshot();
        assert_eq!(thread.len(), 1, "the report reached the wave thread");
        assert_eq!(thread[0].text, "landed the parser");
        assert_eq!(thread[0].from.as_deref(), Some("ship.148e"));

        // Exactly one journal on disk: the served wave's, with exactly one row.
        let wave_journal = journal::journal_path(runtime.repo_root(), "ship");
        assert_eq!(journal_files_under(tmp.path()), vec![wave_journal.clone()]);
        assert_eq!(
            journal::read_events(&wave_journal)
                .iter()
                .filter(|e| matches!(e.kind, journal::EventKind::UserMessage { .. }))
                .count(),
            1,
            "one copy of the report, in the wave's journal",
        );

        // Addressing the wave channel by name = the unaddressed path.
        client
            .post(format!("{base}/messages"))
            .json(&serde_json::json!({ "op": "message", "text": "to the wave", "channel": "ship" }))
            .send()
            .await
            .unwrap();
        assert_eq!(runtime.thread_snapshot().len(), 2);

        // A work line with no worktree is a name like any other: it publishes.
        let ghost = client
            .post(format!("{base}/messages"))
            .json(&serde_json::json!({ "op": "message", "text": "?", "channel": "ship.ghost" }))
            .send()
            .await
            .unwrap();
        assert!(ghost.status().is_success(), "a topic needs no worktree");

        // Outside the family: 404.
        let refused = client
            .post(format!("{base}/messages"))
            .json(&serde_json::json!({ "op": "message", "text": "?", "channel": "concerto" }))
            .send()
            .await
            .unwrap();
        assert_eq!(refused.status(), reqwest::StatusCode::NOT_FOUND);
    }

    /// Every `.jsonl` journal file anywhere under `root`, sorted.
    fn journal_files_under(root: &std::path::Path) -> Vec<std::path::PathBuf> {
        let mut found = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            let Ok(entries) = std::fs::read_dir(&dir) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else if path.extension().is_some_and(|ext| ext == "jsonl") {
                    found.push(path);
                }
            }
        }
        found.sort();
        found
    }

    /// `POST /channels` (the dispatch knock): the wave's thread shows the
    /// opening, idempotent on run id; foreign names 404.
    #[tokio::test]
    async fn channels_door_journals_the_opening_once() {
        let (base, runtime, _tmp) = boot_family().await;
        let client = reqwest::Client::new();

        let body: serde_json::Value = client
            .post(format!("{base}/channels"))
            .json(&serde_json::json!({ "name": "ship.148e", "run_id": "run-1" }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(body["turn"]["text"], "work line ship.148e opened");
        assert_eq!(body["turn"]["from"], "dispatch");

        let again: serde_json::Value = client
            .post(format!("{base}/channels"))
            .json(&serde_json::json!({ "name": "ship.148e", "run_id": "run-1" }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert!(again["turn"].is_null(), "repeated knock appends nothing");
        assert_eq!(runtime.thread_snapshot().len(), 1);

        let foreign = client
            .post(format!("{base}/channels"))
            .json(&serde_json::json!({ "name": "concerto.x", "run_id": "run-2" }))
            .send()
            .await
            .unwrap();
        assert_eq!(foreign.status(), reqwest::StatusCode::NOT_FOUND);
    }

    /// The family subscription is live-only: `/events` streams the primary
    /// (untagged) plus every child (tagged with its channel) from the instant
    /// of connection. Topics have no past, so a frame published before connect
    /// never arrives; `?channel=` narrows to one; a foreign name 404s.
    #[tokio::test]
    async fn events_family_subscription_carries_channel_tagged_frames() {
        let (base, runtime, _tmp) = boot_family().await;
        // Published before connect: nobody was listening, so it is gone.
        runtime
            .deliver_to_channel("ship.a", journal::MessageOp::Message, "a lost".into(), None)
            .unwrap();

        let host = base.strip_prefix("http://").unwrap().to_string();
        let mut stream = tokio::net::TcpStream::connect(&host).await.unwrap();
        stream
            .write_all(b"GET /events HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n")
            .await
            .unwrap();
        // Let the subscription establish before publishing live frames.
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Live frames after connect, from the wave and both children.
        narrate(&runtime, "wave turn");
        runtime
            .deliver_to_channel("ship.a", journal::MessageOp::Message, "a live".into(), None)
            .unwrap();
        runtime
            .deliver_to_channel("ship.b", journal::MessageOp::Message, "b live".into(), None)
            .unwrap();
        let mut acc = String::new();
        let mut buf = [0u8; 4096];
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let read = tokio::time::timeout_at(deadline, stream.read(&mut buf)).await;
            match read {
                Ok(Ok(0)) | Err(_) => break,
                Ok(Ok(n)) => {
                    acc.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if acc.contains("wave turn")
                        && acc.contains("a live")
                        && acc.contains("b live")
                    {
                        break;
                    }
                }
                Ok(Err(_)) => break,
            }
        }
        assert!(acc.contains("a live"), "child live frame arrives: {acc}");
        assert!(acc.contains("b live"), "child live frame arrives");
        assert!(
            !acc.contains("a lost"),
            "a frame published before connect is gone: {acc}"
        );
        assert!(
            acc.contains(r#""channel":"ship.a""#) && acc.contains(r#""channel":"ship.b""#),
            "child frames carry their channel tag"
        );
        // The primary's frames stay untagged (family-of-one wire, unchanged).
        let wave_frame = acc
            .lines()
            .find(|line| line.contains("wave turn"))
            .expect("wave frame");
        assert!(
            !wave_frame.contains(r#""channel""#),
            "primary frames are untagged: {wave_frame}"
        );

        // ?channel=ship.a serves only that channel — no wave turn, no b.
        let mut one = tokio::net::TcpStream::connect(&host).await.unwrap();
        one.write_all(
            b"GET /events?channel=ship.a HTTP/1.1\r\nHost: localhost\r\nConnection: keep-alive\r\n\r\n",
        )
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(200)).await;
        runtime
            .deliver_to_channel("ship.a", journal::MessageOp::Message, "a solo".into(), None)
            .unwrap();
        narrate(&runtime, "wave turn two");
        let mut acc_one = String::new();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        loop {
            let read = tokio::time::timeout_at(deadline, one.read(&mut buf)).await;
            match read {
                Ok(Ok(0)) | Err(_) => break,
                Ok(Ok(n)) => {
                    acc_one.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if acc_one.contains("a solo") {
                        break;
                    }
                }
                Ok(Err(_)) => break,
            }
        }
        assert!(acc_one.contains("a solo"));
        assert!(!acc_one.contains("wave turn"), "no primary frames");
        assert!(
            !acc_one.contains("event: state"),
            "no loop on a child channel"
        );

        // A name outside the family is a 404.
        let refused = reqwest::get(format!("{base}/events?channel=concerto"))
            .await
            .unwrap();
        assert_eq!(refused.status(), reqwest::StatusCode::NOT_FOUND);
        let both = reqwest::get(format!("{base}/events?channel=ship&prefix=ship"))
            .await
            .unwrap();
        assert_eq!(both.status(), reqwest::StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn serve_publishes_and_removes_discovery_pointer_and_token() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("wave/ship")).unwrap();
        let repo = tmp.path().to_path_buf();

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let repo2 = repo.clone();
        let handle = tokio::spawn(async move {
            run_listener(
                repo2,
                "ship".into(),
                None,
                false,
                LoopPolicy::Dormant,
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
        assert!(
            server::read_resident_token(&repo, "ship").is_some(),
            "the resident token publishes beside the pointer"
        );

        shutdown_tx.send(()).unwrap();
        handle.await.unwrap().unwrap();
        assert!(!endpoint.exists(), "pointer removed on shutdown");
        assert!(
            server::read_resident_token(&repo, "ship").is_none(),
            "token removed on shutdown"
        );
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
        let config = resolve_registry(tmp.path(), "ship", false).await;
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
        let repo2 = repo.clone();
        let handle = tokio::spawn(async move {
            run_listener(
                repo2,
                "ship".into(),
                None,
                false,
                LoopPolicy::Dormant,
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
        let repo2 = repo.clone();
        let first = tokio::spawn(async move {
            run_listener(
                repo2,
                "ship".into(),
                None,
                false,
                LoopPolicy::Dormant,
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
        let err = run_listener(
            repo.clone(),
            "ship".into(),
            None,
            false,
            LoopPolicy::Dormant,
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
        use crate::lfd::types::WaveStatus;
        crate::lfd::types::Wave {
            id: crate::lfd::id::LfdId::new(),
            name: name.to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repo: "/tmp/repo".to_string(),
            worktree: String::new(),
            branch: String::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
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
    async fn serve_registers_the_brain_and_deregisters_on_shutdown() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("wave/ship")).unwrap();
        let repo = tmp.path().to_path_buf();
        let store: crate::lfdb::SharedStore = Arc::new(
            crate::lfdb::open_store(&crate::lfdb::StorageConfig::sqlite(
                tmp.path().join("lfd.db"),
            ))
            .await
            .expect("open store"),
        );
        let wave_row = make_wave_row("ship");
        store.create_wave(&wave_row).await.expect("seed wave");
        let wave_id = wave_row.id().clone();

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let repo2 = repo.clone();
        let config = registry::RegistryConfig {
            store: store.clone(),
            wave: wave_row,
            cwd: repo.display().to_string(),
            pid: std::process::id(),
            force: false,
        };
        let handle = tokio::spawn(async move {
            run_listener(
                repo2,
                "ship".into(),
                Some(config),
                false,
                LoopPolicy::Dormant,
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
    async fn serve_refuses_to_start_over_a_live_brain() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("wave/ship")).unwrap();
        let store: crate::lfdb::SharedStore = Arc::new(
            crate::lfdb::open_store(&crate::lfdb::StorageConfig::sqlite(
                tmp.path().join("lfd.db"),
            ))
            .await
            .expect("open store"),
        );
        let wave_row = make_wave_row("ship");
        store.create_wave(&wave_row).await.expect("seed wave");

        // A live brain, registered as another `lf loop` would be.
        let first = registry::RegistryConfig {
            store: store.clone(),
            wave: wave_row.clone(),
            cwd: "/tmp/repo".to_string(),
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
        let err = run_listener(
            tmp.path().to_path_buf(),
            "ship".into(),
            Some(registry::RegistryConfig {
                store,
                wave: wave_row,
                cwd: "/tmp/repo".to_string(),
                pid: std::process::id(),
                force: false,
            }),
            false,
            LoopPolicy::Dormant,
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
}
