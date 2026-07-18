//! `lf wave <name>` — one mind implemented as a listener and resident pair.
//!
//! The listener (this module's [`run_listener`]) is the channel made durable — pure
//! hear / check / fold / tell, vendor-free:
//!
//! - holds the mind's one journal pen;
//! - serves the doors: `/messages`, `/events`, `/memory`, `/health`,
//!   and the token-gated resident door ([`server`]);
//! - drains typed Project/Task observations ([`registry::StoreObserver`]) and
//!   its hands' broadcasts off the shared-store bus ([`bus::BusListener`]);
//! - keeps the Wave row and discovery pointer current;
//! - supervises the resident ([`supervisor`]): process liveness, the respawn
//!   ladder, the interrupt janitor.
//!
//! The resident ([`resident`]) owns the pass scheduler, runs from the clean
//! canonical main checkout as a read-and-coordinate control plane, consumes
//! its own wave's `/events?inbox=true` subscription, and publishes ordered
//! turn deltas back through the resident door. The wire between them is
//! [`wire`].
//!
//! ```text
//!   lf wave <name>                      lf __resident <name>
//!   ┌───────────────────────┐  spawns   ┌──────────────────────────┐
//!   │ LISTENER (origin repo)│──────────▶│ RESIDENT (clean main)    │
//!   │ pens · folds · doors  │           │ pass scheduler           │
//!   │ observer · supervisor │◀──deltas──│ seed · queue             │
//!   └──────────┬────────────┘           └────────────▲─────────────┘
//!              └────────── /events?inbox=true ───────┘
//! ```
//!
//! `lf wave <name>` boots the listener, which spawns `lf __resident <name>`
//! carrying private endpoint/token environment. The split is runtime plumbing,
//! not a second product surface.
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
//! The listener also uses the local [`registry`] for a store-polling observer
//! that carries typed Project/Task events into the Wave journal. The endpoint
//! file enforces one live listener per Wave. No registry store on the machine
//! means no child observations; the listener remains otherwise functional.

pub mod bus;
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
mod types;
pub mod wire;

pub use types::Wave;

use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::engine::repo::find_repo_root;
use crate::engine::worktrees::main_repo_root;
use crate::ops::util::normalize_wave_name;
use crate::store::{open_existing_store, SharedStore};
use crate::wave::runtime::WaveRuntime;

/// The hidden subcommand a listener spawns for its own resident body. Named
/// here so the spawner and the CLI cannot drift apart silently.
pub(crate) const RESIDENT_SUBCOMMAND: &str = "__resident";
pub(crate) const WAVE_SERVER_ENDPOINT_ENV: &str = "LF_WAVE_SERVER_ENDPOINT";

/// `lf wave <name>` — boot the named mind's listener and supervise its
/// resident. The steerable half: an endpoint, a thread, a cadence.
///
/// The listener spawns its resident body as `lf __resident <name>`. That is an
/// explicit command, not an ambient one: an earlier design branched here on
/// whether the resident endpoint/token were present in env, which meant any
/// process holding a parent's env — a tmux child, a promoted Wave — booted
/// the wrong half by accident.
pub fn run(name: &str, force: bool) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root).unwrap_or_else(|_| repo_root.clone());
    let wave = normalize_wave_name(name).ok_or_else(|| anyhow!("invalid wave name: '{name}'"))?;
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let registry_config = resolve_registry(&main_repo, &wave).await;
        run_listener(
            main_repo,
            wave,
            registry_config,
            force,
            true,
            shutdown_signal(),
        )
        .await
    })
}

/// `lf stop <name>` — ask the named wave's listener to shut down gracefully.
/// A missing or stale endpoint is already stopped, so this is idempotent.
pub fn stop(name: &str) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root).unwrap_or_else(|_| repo_root.clone());
    let wave = normalize_wave_name(name).ok_or_else(|| anyhow!("invalid wave name: '{name}'"))?;
    let rt = tokio::runtime::Runtime::new()?;
    let requested = rt.block_on(request_stop(&main_repo, &wave))?;
    if requested {
        println!("stopped wave {wave}");
    } else {
        println!("wave {wave} is already stopped");
    }
    Ok(())
}

async fn request_stop(repo_root: &Path, wave: &str) -> Result<bool> {
    let Some(endpoint) = server::live_endpoint(repo_root, wave).await else {
        return Ok(false);
    };
    let response = reqwest::Client::new()
        .post(format!("http://{endpoint}/stop"))
        .send()
        .await
        .map_err(|err| anyhow!("failed to stop wave '{wave}': {err}"))?;
    if response.status() != reqwest::StatusCode::ACCEPTED {
        return Err(anyhow!(
            "wave '{wave}' refused stop with HTTP {}",
            response.status()
        ));
    }

    // The route requests shutdown; `run_listener` still owns resident and
    // registry cleanup. Wait until this boot's pointer disappears or a newer
    // listener replaces it, without probing the server on every iteration.
    for _ in 0..100 {
        let recorded = std::fs::read_to_string(server::endpoint_path(repo_root, wave))
            .ok()
            .map(|value| value.trim().to_string());
        if recorded.as_deref() != Some(endpoint.as_str()) {
            return Ok(true);
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }

    if server::live_endpoint(repo_root, wave).await.is_none() {
        return Ok(true);
    }
    Err(anyhow!(
        "wave '{wave}' accepted stop but is still serving at http://{endpoint}"
    ))
}

/// Open the machine's shared registry and resolve this wave's row, creating
/// the row when the store has never seen the wave — the db IS the registry,
/// so a reachable store always yields a registered boot (see
/// [`registry::ensure_wave_row`]). `None` (with one warning) only when the
/// store itself is missing or unusable: the server runs without child
/// observations. Endpoint discovery still prevents a second listener.
async fn resolve_registry(main_repo: &Path, wave: &str) -> Option<registry::RegistryConfig> {
    let Some(store) = open_existing_store().await else {
        tracing::warn!(
            wave,
            "no local registry on this machine; running without child observations"
        );
        return None;
    };
    let store: SharedStore = Arc::new(store);
    match registry::ensure_wave_row(&store, main_repo, wave).await {
        Ok(row) => Some(registry::RegistryConfig { store, wave: row }),
        Err(err) => {
            tracing::warn!(wave, error = %err, "local registry unusable; running without child observations");
            None
        }
    }
}

struct WaveRunGuard(Option<(SharedStore, crate::durable::RunLease)>);

impl WaveRunGuard {
    fn as_ref(&self) -> Option<&(SharedStore, crate::durable::RunLease)> {
        self.0.as_ref()
    }

    fn take(&mut self) -> Option<(SharedStore, crate::durable::RunLease)> {
        self.0.take()
    }
}

impl Drop for WaveRunGuard {
    fn drop(&mut self) {
        let Some((store, lease)) = self.0.take() else {
            return;
        };
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            return;
        };
        runtime.spawn(async move {
            if let Err(error) = store
                .stop_run(
                    &lease,
                    crate::durable::StopCause::Requested,
                    crate::durable::ContainmentObservation::Absent,
                )
                .await
            {
                tracing::warn!(%error, "failed to roll back Wave Run authority");
            }
        });
    }
}

/// The production resident spawner: `lf __resident <wave>`, run by this
/// same executable, endpoint + token + Wave context in env. The
/// resident's stdout/stderr inherit — one `lf wave` terminal shows both
/// halves, today's UX.
// TODO(M1): keep this shutdown contract in the wave/supervisor owner: stand
// the respawn ladder down before terminating the resident, honor interrupt
// cleanup, and keep SIGKILL deadlines in the supervisor path.
fn resident_spawner(
    wave: String,
    repo_root: PathBuf,
    endpoint: String,
    token: String,
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
            // The resident's children must resolve `lf` to this binary.
            .env("PATH", crate::flowloop::wave::path_for_children())
            .env_remove(crate::durable::RUN_CONTEXT_ENV)
            .env_remove(crate::durable::RUN_LEASE_ENV)
            .env_remove("LF_PROJECT_SESSION_ID")
            .env_remove("LF_PROJECT_GENERATION")
            .env_remove("LF_PROJECT_LEASE_TOKEN")
            .env_remove("LF_TASK_SESSION_ID")
            .env_remove("LF_TASK_GENERATION")
            .env_remove("LF_TASK_LEASE_TOKEN")
            .stdin(std::process::Stdio::null());
        // No kill_on_drop: shutdown stops the supervisor FIRST (so a TERM'd
        // resident's exit isn't journaled as a failure), then SIGTERMs the
        // resident by pid — its hooks stop the vendor process group. A
        // SIGKILL-on-drop here would orphan the codex group instead.
        for (key, value) in &session_env {
            command.env(key, value);
        }
        #[cfg(unix)]
        command.process_group(0);
        command.spawn()
    })
}

/// Serve the wave until `shutdown` resolves. Vendor-free by construction:
/// no harness, no vendor process — the resident owns those. Production always
/// spawns one; `spawn_resident` is `false` only in tests that exercise the
/// listener alone. `registry_config` is `None` in tests that exercise the
/// server without a registry store; `force` rides separately because the
/// endpoint-file floor must honor it even when there is no registry config at
/// all.
async fn run_listener(
    repo_root: PathBuf,
    wave: String,
    registry_config: Option<registry::RegistryConfig>,
    force: bool,
    spawn_resident: bool,
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

    let registered = registry_config
        .as_ref()
        .map(|config| (config.store.clone(), config.wave.id().clone()));
    let mut wave_run = WaveRunGuard(if spawn_resident {
        match registry_config.as_ref() {
            Some(config) => {
                let work = crate::durable::WorkRef::Wave(config.wave.id().clone());
                let home = config.store.home("local").await?;
                let (_, lease) = config
                    .store
                    .reserve_run(&work, &home.id, crate::durable::RunTrigger::User)
                    .await?;
                Some((config.store.clone(), lease))
            }
            None => None,
        }
    } else {
        None
    });
    let mut session_env = registry_config
        .as_ref()
        .map(|config| {
            vec![
                (
                    crate::engine::wave_context::WAVE_ID_ENV.to_string(),
                    config.wave.id().to_string(),
                ),
                (
                    crate::engine::wave_context::CHANNEL_ENV.to_string(),
                    config.wave.name().to_string(),
                ),
            ]
        })
        .unwrap_or_default();
    if let Some((_, lease)) = wave_run.as_ref() {
        session_env.extend([
            (
                crate::durable::RUN_CONTEXT_ENV.to_string(),
                "agent".to_string(),
            ),
            (
                crate::durable::RUN_LEASE_ENV.to_string(),
                lease.env_value().to_string(),
            ),
        ]);
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
    let mut bus_task: Option<tokio::task::JoinHandle<()>> = None;
    if let Some((store, wave_id)) = registered {
        let obs = Arc::new(registry::StoreObserver::new(
            runtime.clone(),
            store.clone(),
            wave_id,
        ));
        observer_task = Some(tokio::spawn(Arc::clone(&obs).run(registry::POLL_CADENCE)));
        observer = Some(obs);
        // The mind's ear: the bus is a table, so the listener subscribes to it
        // like anyone else. Unregistered boots have no store and stay deaf.
        let listener = Arc::new(bus::BusListener::new(runtime.clone(), store));
        bus_task = Some(tokio::spawn(listener.run(bus::POLL_CADENCE)));
    }

    // The resident door: a per-boot token, published beside the endpoint
    // pointer so the resident can attach (same trust domain).
    let token = server::generate_resident_token();
    let door = server::ResidentDoor::new(token.clone());
    server::write_resident_token(&repo_root, &wave, &token)?;

    // The keeper's watch: resident liveness, respawn ladder, interrupt
    // janitor. Runs even without a spawner — the pen-side anti-wedges (janitor, attach
    // probe) never depend on who spawned the resident.
    let spawner = spawn_resident.then(|| {
        resident_spawner(
            wave.clone(),
            repo_root.clone(),
            addr.to_string(),
            token.clone(),
            session_env,
        )
    });
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
        "lf wave · {wave} · listener on http://{addr}{} \
         (Ctrl-C to stop, RUST_LOG=loopflow=debug for the firehose)",
        if spawn_resident {
            " · spawning resident"
        } else {
            " · no resident"
        }
    );

    let shutdown_door = server::ShutdownDoor::new();
    let shutdown_request = shutdown_door.clone();
    let graceful_shutdown = async move {
        tokio::select! {
            _ = shutdown => {}
            _ = shutdown_request.wait() => {}
        }
    };
    let app = server::router(
        runtime.clone(),
        door.clone(),
        observer,
        Some(supervisor_handle),
        shutdown_door,
    );
    let result = axum::serve(listener, app)
        .with_graceful_shutdown(graceful_shutdown)
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
    if let Some(task) = bus_task {
        task.abort();
    }
    if let Some((store, lease)) = wave_run.take() {
        if let Err(error) = store
            .stop_run(
                &lease,
                crate::durable::StopCause::Requested,
                crate::durable::ContainmentObservation::Absent,
            )
            .await
        {
            tracing::warn!(%error, "failed to release Wave Run authority");
        }
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

    use crate::chat::turns::{ChatRole, ChatTurn, TurnDelta};
    use crate::chat::types::Lifecycle;
    use crate::wave::journal::MessageOp;
    use crate::wave::server::ResidentDoor;
    use crate::wave::wire::{ResidentDelta, RESIDENT_TOKEN_HEADER};

    /// Serving a mind and running a Task Session are different entrypoints, and the
    /// listener spawns its resident body by name. Nothing here reads the
    /// environment: an `lf` process inheriting `WAVE_SERVER_ENDPOINT` and
    /// `RESIDENT_TOKEN` — a tmux child, a promoted Wave — becomes whichever
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

        // The retired loop verb is no longer a built-in; unmatched names fall
        // through to the external skill surface.
        assert!(matches!(
            Cli::try_parse_from(["lf", "loop", "goals"])
                .expect("external fallback")
                .command,
            Some(Commands::External(parts)) if parts[0] == "loop"
        ));
    }

    #[tokio::test]
    async fn stopping_an_absent_wave_is_idempotent() {
        let tmp = tempfile::tempdir().expect("tempdir");
        assert!(!request_stop(tmp.path(), "ship").await.expect("stop"));
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
            activity: None,
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
            None,
            None,
            server::ShutdownDoor::new(),
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

    /// The thread door is the human's: `say` is not a wire op and bylines are
    /// refused on every op — attribution enters the thread only through the
    /// bus fold. "Agents don't use chat" is a wire property, not doctrine,
    /// and each refusal names the bus.
    #[tokio::test]
    async fn the_thread_door_refuses_machine_speech() {
        let (base, runtime, _tmp) = boot().await;
        let client = reqwest::Client::new();

        for body in [
            serde_json::json!({
                "op": "say",
                "text": "run-7 landed: PR #12",
                "from": "worker",
            }),
            serde_json::json!({ "op": "say", "text": "anon" }),
            serde_json::json!({ "op": "message", "text": "hello", "from": "cli" }),
        ] {
            let response = client
                .post(format!("{base}/messages"))
                .json(&body)
                .send()
                .await
                .unwrap();
            assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
            let text = response.text().await.unwrap();
            assert!(
                text.contains("lf radio pub"),
                "the refusal names the bus: {text}"
            );
        }

        assert!(runtime.thread_snapshot().is_empty(), "nothing journaled");
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
                "receipts": [],
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
            .json(&serde_json::json!({ "op": "add", "content": "  ", "summary": null, "receipts": [] }))
            .send()
            .await
            .unwrap();
        assert_eq!(empty.status(), reqwest::StatusCode::BAD_REQUEST);

        let body: serde_json::Value = client
            .post(format!("{base}/memory"))
            .json(&serde_json::json!({ "op": "add", "content": "one fact", "summary": null, "receipts": [] }))
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

        // The context door serves durable scheduler/provider state only.
        let context: serde_json::Value = client
            .get(format!("{base}/resident/context"))
            .header(RESIDENT_TOKEN_HEADER, "test-token")
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();

        assert!(context.get("playhead").is_some());
        assert!(context.get("provider_session").is_some());
        assert!(context.get("in_flight").is_none());
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
        runtime
            .deliver(MessageOp::Message, "queued before".into())
            .expect("user turn");

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

    /// Reconstruct the thread the way a real client does: a `turn` frame
    /// replaces the turn of its id, a `turn-delta` frame absorbs one increment
    /// into it. Returns the turns in first-seen order, each at its latest state.
    fn parse_turn_frames(sse_body: &str) -> Vec<ChatTurn> {
        let mut order: Vec<String> = Vec::new();
        let mut by_id: std::collections::HashMap<String, ChatTurn> =
            std::collections::HashMap::new();
        let mut event = String::new();
        for line in sse_body.lines() {
            if let Some(name) = line.strip_prefix("event:") {
                event = name.trim().to_string();
            } else if let Some(data) = line.strip_prefix("data:") {
                let data = data.trim();
                match event.as_str() {
                    "turn" => {
                        if let Ok(turn) = serde_json::from_str::<ChatTurn>(data) {
                            if !by_id.contains_key(&turn.id) {
                                order.push(turn.id.clone());
                            }
                            by_id.insert(turn.id.clone(), turn);
                        }
                    }
                    "turn-delta" => {
                        if let Ok(delta) = serde_json::from_str::<TurnDelta>(data) {
                            if let Some(turn) = by_id.get_mut(&delta.turn_id) {
                                turn.absorb_item(delta.item);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        order
            .into_iter()
            .map(|id| by_id.remove(&id).expect("id tracked"))
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
            None,
            None,
            server::ShutdownDoor::new(),
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
            None,
            None,
            server::ShutdownDoor::new(),
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        (format!("http://{addr}"), runtime, tmp)
    }

    /// The thread door is the thread's alone: a human message lands one
    /// unattributed copy in the served wave's journal, and no journal exists
    /// anywhere else on disk. Reports from hands arrive on the bus, not here.
    #[tokio::test]
    async fn a_message_is_recorded_once_in_the_waves_journal() {
        let (base, runtime, tmp) = boot_family().await;
        let client = reqwest::Client::new();

        let body: serde_json::Value = client
            .post(format!("{base}/messages"))
            .json(&serde_json::json!({ "op": "message", "text": "landed the parser" }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(body["turn"]["text"], "landed the parser");

        let thread = runtime.thread_snapshot();
        assert_eq!(thread.len(), 1);
        assert_eq!(thread[0].from, None, "human turns carry no byline");

        // Exactly one journal on disk: the served wave's, with exactly one row.
        let wave_journal = journal::journal_path(runtime.repo_root(), "ship");
        assert_eq!(
            loopflow_test_support::journal_files_under(tmp.path()),
            vec![wave_journal.clone()]
        );
        assert_eq!(
            journal::read_events(&wave_journal)
                .iter()
                .filter(|e| matches!(e.kind, journal::EventKind::UserMessage { .. }))
                .count(),
            1,
            "one copy, in the wave's journal",
        );
    }

    #[tokio::test]
    async fn serve_publishes_and_removes_discovery_pointer_and_token() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("wave/ship")).unwrap();
        let repo = tmp.path().to_path_buf();

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let repo2 = repo.clone();
        let handle = tokio::spawn(async move {
            run_listener(repo2, "ship".into(), None, false, false, async {
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

    #[tokio::test]
    async fn stop_command_path_gracefully_shuts_down_the_listener() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("wave/ship")).unwrap();
        let repo = tmp.path().to_path_buf();
        let repo2 = repo.clone();
        let handle = tokio::spawn(async move {
            run_listener(
                repo2,
                "ship".into(),
                None,
                false,
                false,
                std::future::pending(),
            )
            .await
        });

        let endpoint = server::endpoint_path(&repo, "ship");
        wait_for(|| endpoint.exists()).await;
        assert!(request_stop(&repo, "ship").await.expect("request stop"));
        handle.await.unwrap().unwrap();
        assert!(!endpoint.exists(), "stop removes the endpoint");
        assert!(
            server::read_resident_token(&repo, "ship").is_none(),
            "stop removes the resident token"
        );
    }

    #[tokio::test]
    async fn stop_closes_a_live_event_subscription() {
        let tmp = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(tmp.path().join("wave/ship")).unwrap();
        let repo = tmp.path().to_path_buf();
        let repo2 = repo.clone();
        let mut handle = tokio::spawn(async move {
            run_listener(
                repo2,
                "ship".into(),
                None,
                false,
                false,
                std::future::pending(),
            )
            .await
        });

        let endpoint = server::endpoint_path(&repo, "ship");
        wait_for(|| endpoint.exists()).await;
        let addr = std::fs::read_to_string(&endpoint).unwrap();
        let client = reqwest::Client::new();
        let events = client
            .get(format!("http://{}/events?inbox=true", addr.trim()))
            .send()
            .await
            .expect("subscribe");
        assert_eq!(events.status(), reqwest::StatusCode::OK);

        assert!(request_stop(&repo, "ship").await.expect("request stop"));
        match tokio::time::timeout(Duration::from_secs(2), &mut handle).await {
            Ok(result) => result.unwrap().unwrap(),
            Err(_) => {
                handle.abort();
                panic!("listener stayed alive behind its event subscription");
            }
        }
    }

    /// No registry store on the machine: the boot degrades to unregistered
    /// (warn-and-continue), never an error — the pre-registry status quo.
    #[tokio::test]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn resolve_registry_without_a_store_runs_unregistered() {
        let _env = crate::journal::TestLedgerGuard::new();
        let tmp = tempfile::tempdir().expect("tempdir");
        let config = resolve_registry(tmp.path(), "ship").await;
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
            run_listener(repo2, "ship".into(), None, false, false, async {
                let _ = shutdown_rx.await;
            })
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
            run_listener(repo2, "ship".into(), None, false, false, async {
                let _ = shutdown_rx.await;
            })
            .await
        });
        let endpoint = server::endpoint_path(&repo, "ship");
        wait_for(|| endpoint.exists()).await;
        let first_addr = std::fs::read_to_string(&endpoint).unwrap();

        // Second server: probed live, refused, pointer untouched.
        let (_shutdown_tx2, shutdown_rx2) = tokio::sync::oneshot::channel::<()>();
        let err = run_listener(repo.clone(), "ship".into(), None, false, false, async {
            let _ = shutdown_rx2.await;
        })
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
}
