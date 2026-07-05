//! `lf q` — daemonless dispatch against the shared run registry.
//!
//! `lf q worker run <wave> --flow F --task T` does what lfd's worker endpoint
//! used to do, with no daemon in the path: resolve placement (fresh worktree
//! by default, `--pool` for the wave's shared tree, `--stack <run-id>` for a
//! branch forked from an unlanded run), write the Run + Session rows to the
//! shared store, and launch `lf <flow>: <task>` in a detached tmux session.
//!
//! # The work line's channel
//!
//! Dispatch mints the work line's CHANNEL alongside its worktree: the channel
//! name is exactly the worktree basename minus the repo prefix
//! (`goals.148e0e02`), its journal is initialized IN the worktree
//! (`.lf/journal/waves/<channel>/journal.jsonl` — it travels with the branch
//! and dies with it), and the parent wave's live server is knocked
//! (`POST /channels`) so the wave's thread shows "work line <name> opened".
//! The knock is best-effort — dispatch stays daemonless; with no live
//! listener the opening simply isn't narrated (the observer still journals
//! `RunObserved` when a server next looks). Pool placement shares the wave's
//! own worktree, so its channel IS the wave channel — nothing minted.
//!
//! # Env contract (matches the lfd executor byte for byte)
//!
//! The worker's tmux environment carries `LFD_WAVE_ID`, `LFD_RUN_ID`,
//! `LF_RUN_ID`, `LFD_SESSION_ID` (the session row created here),
//! `LFD_AGENT_ROLE=worker`, and `LFD_CHANNEL` (the work line's channel — a
//! bare `lf chat` inside the worker speaks on its own channel).
//! `LFD_SESSION_INHERITED` is deliberately absent:
//! a session id without the marker means "this very process owns the row"
//! (see `lf::session`), so the child does NOT self-register a second row.
//! The shared tmux wrapper (`helpers::tmux_shell_command`) explicitly
//! `unset`s the marker, because a fresh tmux server inherits the
//! dispatcher's environment and would otherwise leak `LFD_SESSION_INHERITED=1`
//! into the worker. The child's parentage is the dispatcher's:
//! `LFD_SESSION_ID` in *this* process's env (the mind's session, when the
//! mind dispatches) becomes the worker session's `parent_session_id`.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use time::OffsetDateTime;

use crate::engine::wave_context::read_endpoint_pointer;
use crate::engine::worktrees::wave_name_from_worktree_and_main;
use crate::lf::{QCommand, WorkerCommand};
use crate::lfd::executor::helpers::{
    build_lf_dispatch_command, launch_session_in_tmux, worker_dispatch_task, worker_exit_tail,
};
use crate::lfd::executor::{create_run_for_placement, Placement};
use crate::lfd::id::LfdId;
use crate::lfd::types::{
    tmux_session_name, Run, Session, SessionStatus, SessionUse, TMUX_TERMINAL_SOURCE,
};
use crate::lfdb::{open_existing_store, SharedStore};

pub fn run(cmd: &QCommand) -> Result<()> {
    match cmd {
        QCommand::Worker {
            cmd:
                WorkerCommand::Run {
                    wave,
                    flow,
                    task,
                    pool,
                    stack,
                    no_pr,
                },
        } => worker_run(wave, flow, task, *pool, stack.as_deref(), *no_pr),
    }
}

fn worker_run(
    wave: &str,
    flow: &str,
    task: &str,
    pool: bool,
    stack: Option<&str>,
    no_pr: bool,
) -> Result<()> {
    let placement = match (pool, stack) {
        (true, _) => Placement::Pool,
        (false, Some(run_id)) => Placement::Stack {
            parent_run_id: run_id
                .parse()
                .map_err(|_| anyhow!("invalid --stack run id: '{run_id}'"))?,
        },
        (false, None) => Placement::Fresh,
    };
    let parent_session_id = std::env::var(crate::lf::session::SESSION_ID_ENV)
        .ok()
        .filter(|value| !value.is_empty())
        .and_then(|value| value.parse().ok());

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let store: SharedStore = Arc::new(open_existing_store().await.ok_or_else(|| {
            anyhow!("no run registry on this machine — nothing has created ~/.lf/lfd.db yet")
        })?);
        let dispatched = dispatch(&store, wave, flow, task, placement, parent_session_id).await?;
        launch_worker_tmux(&store, dispatched.session.clone(), no_pr).await?;
        // Tell the parent listener a work line opened — best effort, the
        // dispatch itself is daemonless.
        if dispatched.channel != wave {
            notify_channel_opened(&store, &dispatched).await;
        }
        println!("dispatched {flow} worker for wave '{wave}'");
        println!("  run       {}", dispatched.run.id);
        println!("  session   {}", dispatched.session.id);
        println!("  channel   {}", dispatched.channel);
        println!("  tmux      {}", dispatched.session.tmux_name);
        println!("  worktree  {}", dispatched.run.worktree);
        Ok(())
    })
}

/// A recorded dispatch: the Run row (placement resolved, worktree created),
/// the Session row the worker will own, the wave row it belongs to, and the
/// work line's channel name (the wave's own name for pool placement).
#[derive(Debug)]
pub(crate) struct Dispatch {
    pub run: Run,
    pub session: Session,
    pub wave: crate::lfd::types::Wave,
    pub channel: String,
}

/// Write the dispatch into the registry: resolve placement into a worktree +
/// branch, create the Run row, then the worker's Session row carrying the
/// env contract. No process is spawned here. This is the one worker dispatch
/// door — the lfd HTTP worker route died with the collapse.
pub(crate) async fn dispatch(
    store: &SharedStore,
    wave_name: &str,
    flow: &str,
    task: &str,
    placement: Placement,
    parent_session_id: Option<LfdId>,
) -> Result<Dispatch> {
    let flow = flow.trim();
    let task = task.trim();
    if flow.is_empty() {
        return Err(anyhow!("flow is required"));
    }
    if task.is_empty() {
        return Err(anyhow!("task is required"));
    }
    let wave = store
        .get_wave_by_name(wave_name)
        .await?
        .ok_or_else(|| anyhow!("wave '{wave_name}' not found in the registry"))?;

    let parent_session_id = match parent_session_id {
        Some(id) => match store.get_control_session(&id).await? {
            Some(parent) => {
                if parent.session_use == SessionUse::Worker {
                    return Err(anyhow!(
                        "worker sessions cannot launch worker sessions (parent {id})"
                    ));
                }
                if parent.wave_id != *wave.id() {
                    return Err(anyhow!("parent session {id} belongs to another wave"));
                }
                Some(id)
            }
            None => {
                tracing::warn!(parent = %id, "LFD_SESSION_ID names no session; dispatching unattributed");
                None
            }
        },
        None => None,
    };

    let active_runs = store.count_active_runs(wave.id()).await?;
    if wave.workers() > 0 && active_runs >= wave.workers() {
        return Err(anyhow!(
            "wave '{wave_name}' already at worker capacity ({active_runs}/{})",
            wave.workers()
        ));
    }

    let run_id = LfdId::new();
    let mut run = create_run_for_placement(store, &wave, &run_id, &placement, None).await?;
    run.flow = flow.to_string();
    run.task = Some(task.to_string());
    store.update_run(&run).await?;

    // The work line's channel: exactly the worktree basename minus the repo
    // prefix (pool placement shares the wave worktree — its channel IS the
    // wave channel). A fresh channel's journal is initialized HERE, in the
    // worktree, before anyone else knows the name — no pen race; the family
    // head's server materializes its pen on first delivery or subscription.
    let channel =
        wave_name_from_worktree_and_main(Path::new(&run.worktree), Path::new(wave.repo()))
            .unwrap_or_else(|| wave.name().clone());
    if channel != *wave.name() {
        let journal = crate::wave::journal::journal_path(Path::new(&run.worktree), &channel);
        if let Err(err) = crate::wave::journal::Journal::open(&journal) {
            tracing::warn!(channel, error = %err, "work-line channel journal init failed");
        }
    }

    // The worker's prompt closes the reporting loop through the one door
    // every process has — exec: it finishes by posting an `lf chat` report
    // into the wave's thread (attributed via LFD_SESSION_ID from its env).
    // The Run row keeps the raw task; only the dispatched prompt grows.
    let dispatch_task = worker_dispatch_task(task);

    let session_id = LfdId::new();
    let env = BTreeMap::from([
        ("LFD_WAVE_ID".to_string(), wave.id().to_string()),
        ("LFD_RUN_ID".to_string(), run.id.to_string()),
        ("LF_RUN_ID".to_string(), run.id.to_string()),
        ("LFD_SESSION_ID".to_string(), session_id.to_string()),
        (crate::lf::session::CHANNEL_ENV.to_string(), channel.clone()),
        (
            "LFD_AGENT_ROLE".to_string(),
            SessionUse::Worker.as_str().to_string(),
        ),
        // LFD_SESSION_INHERITED deliberately absent: the child owns this row.
    ]);
    let cmd = build_lf_dispatch_command(
        flow,
        &dispatch_task,
        run.direction.as_slice(),
        &run.area,
        wave.name(),
    );
    let session = Session {
        id: session_id.clone(),
        wave_id: wave.id().clone(),
        run_id: Some(run.id.clone()),
        parent_session_id,
        session_use: SessionUse::Worker,
        step: format!("dispatch:{flow}"),
        agent: "lf".to_string(),
        cwd: run.worktree.clone(),
        argv: cmd,
        env,
        source: TMUX_TERMINAL_SOURCE.to_string(),
        tmux_name: tmux_session_name(&format!("{}-{}", run.branch, session_id.as_str())),
        status: SessionStatus::Pending,
        attached_at: None,
        started_at: None,
        completed_at: None,
        created_at: OffsetDateTime::now_utc(),
        completion_token: None,
    };
    store.register_session(&session).await?;
    Ok(Dispatch {
        run,
        session,
        wave,
        channel,
    })
}

/// Knock on the parent wave's live server so its thread shows the work line
/// opening (`POST /channels {name, run_id}`, journaled as `ChannelOpened`,
/// idempotent). Best-effort by design: no live listener → one stderr note,
/// dispatch already succeeded.
async fn notify_channel_opened(store: &SharedStore, dispatched: &Dispatch) {
    let wave = &dispatched.wave;
    let mut endpoint = crate::lf::commands::chat::wave_server_endpoint(store, wave.id())
        .await
        .ok()
        .flatten();
    if endpoint.is_none() {
        endpoint = read_endpoint_pointer(Path::new(wave.repo()), wave.name());
    }
    let Some(endpoint) = endpoint else {
        eprintln!(
            "note: wave '{}' has no live server; channel '{}' opens unannounced",
            wave.name(),
            dispatched.channel
        );
        return;
    };
    let body = serde_json::json!({
        "name": dispatched.channel,
        "run_id": dispatched.run.id.to_string(),
    });
    if let Err(err) = crate::lf::commands::chat::post_json(&endpoint, "/channels", &body).await {
        eprintln!(
            "note: could not announce channel '{}' to wave '{}': {err}",
            dispatched.channel,
            wave.name()
        );
    }
}

/// Launch the worker in a detached tmux session through the shared wrapper
/// (`helpers::launch_session_in_tmux` — one authoring site for the exit-file
/// contract and the inherited-marker unset, byte-identical to the lfd
/// executor's launches). The tail carries the auto-PR guarantee: on clean
/// flow exit the wrapper runs `lf op pr` itself unless `--no-pr` was passed
/// at dispatch.
async fn launch_worker_tmux(store: &SharedStore, session: Session, no_pr: bool) -> Result<()> {
    let mut session = session;
    match launch_session_in_tmux(&session, &worker_exit_tail(no_pr)).await {
        Ok(()) => {
            session.start();
            store.update_control_session(&session).await?;
            Ok(())
        }
        Err(err) => {
            session.complete(1);
            store.update_control_session(&session).await?;
            Err(anyhow!("tmux failed to launch the worker session: {err}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::path::Path;

    use loopflow_test_support::TestRepo;

    use crate::lf::session::classify_run_context;
    use crate::lfd::executor::helpers::tmux_shell_command;
    use crate::lfd::types::{RepoWork, RunStatus, Wave, WaveMode, WaveStatus};
    use crate::lfdb::{open_store, StorageConfig};

    async fn temp_store(tmp: &Path) -> SharedStore {
        Arc::new(
            open_store(&StorageConfig::sqlite(tmp.join("lfd.db")))
                .await
                .expect("open sqlite store"),
        )
    }

    fn make_wave(repo: &Path, name: &str, workers: u32) -> Wave {
        Wave {
            id: LfdId::new(),
            name: name.to_string(),
            mode: WaveMode::Loop,
            primary_flow: "ship-roadmap".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repos: vec![RepoWork {
                repo: repo.display().to_string(),
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
            created_at: Some(OffsetDateTime::now_utc()),
            workers,
            parent_wave_id: None,
        }
    }

    #[tokio::test]
    async fn dispatch_records_run_and_session_with_the_executor_env_contract() {
        let repo = TestRepo::new();
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave(repo.path(), "ship", 2);
        store.create_wave(&wave).await.expect("seed wave");

        let dispatched = dispatch(
            &store,
            "ship",
            "implement",
            "Add the thing.",
            Placement::Fresh,
            None,
        )
        .await
        .expect("dispatch");

        // Run row: flow/task recorded, fresh three-segment worker worktree.
        let run = store
            .get_run(&dispatched.run.id)
            .await
            .expect("run lookup")
            .expect("run stored");
        assert_eq!(run.flow, "implement");
        assert_eq!(run.task.as_deref(), Some("Add the thing."));
        let repo_name = repo
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .expect("repo name");
        let worktree_name = Path::new(&run.worktree)
            .file_name()
            .and_then(|name| name.to_str())
            .expect("worktree name");
        assert!(
            worktree_name.starts_with(&format!("{repo_name}.ship.")),
            "fresh placement gets a <repo>.<wave>.<id> worktree, got {worktree_name}"
        );
        assert!(Path::new(&run.worktree).exists(), "worktree created");

        // Session row: worker, run-linked, tmux-named with the session id.
        let session = store
            .get_control_session(&dispatched.session.id)
            .await
            .expect("session lookup")
            .expect("session stored");
        assert_eq!(session.session_use, SessionUse::Worker);
        assert_eq!(session.run_id, Some(run.id.clone()));
        assert_eq!(session.cwd, run.worktree);
        assert!(session.tmux_name.contains(session.id.as_str()));
        assert_eq!(session.argv[1], "implement:");
        // The dispatched prompt is the task plus the report-back instruction
        // (the Run row keeps the raw task).
        let prompt = session.argv.last().expect("task arg");
        assert!(prompt.starts_with("Add the thing."));
        assert!(
            prompt.contains("lf chat"),
            "worker prompt closes the loop with an lf chat report: {prompt}"
        );

        // The env contract, pinned: the child sees its own session id with
        // no inherited marker → OwnSession → it must NOT register again.
        assert_eq!(
            session.env.get("LFD_WAVE_ID").map(String::as_str),
            Some(wave.id().as_str())
        );
        assert_eq!(
            session.env.get("LFD_RUN_ID").map(String::as_str),
            Some(run.id.as_str())
        );
        assert_eq!(
            session.env.get("LFD_SESSION_ID").map(String::as_str),
            Some(session.id.as_str())
        );
        assert_eq!(
            session.env.get("LFD_AGENT_ROLE").map(String::as_str),
            Some("worker")
        );
        assert!(
            !session.env.contains_key("LFD_SESSION_INHERITED"),
            "the child owns this row; the inherited marker must be absent"
        );
        assert_eq!(
            classify_run_context(
                session
                    .env
                    .get("LFD_WAVE_ID")
                    .map(|id| crate::engine::wave_context::AmbientWaveRef::Id(id.clone())),
                session.env.get("LFD_SESSION_ID").map(String::as_str),
                session.env.contains_key("LFD_SESSION_INHERITED"),
            ),
            crate::lf::session::RunContext::OwnSession,
            "a worker launched with this env classifies as OwnSession"
        );

        // The registry groups live sessions under the worktree name.
        let grouped = store
            .active_sessions_by_worktree(worktree_name)
            .await
            .expect("worktree lookup");
        assert_eq!(grouped.len(), 1);
        assert_eq!(grouped[0].id, session.id);

        // Dispatch minted the work line's CHANNEL: the ownership name (the
        // worktree basename minus the repo prefix), its journal initialized
        // IN the worktree, and the name in the worker's env so `lf chat`
        // with no args speaks locally.
        assert_eq!(
            dispatched.channel,
            worktree_name.trim_start_matches(&format!("{repo_name}."))
        );
        assert!(dispatched.channel.starts_with("ship."));
        let journal =
            crate::wave::journal::journal_path(Path::new(&run.worktree), &dispatched.channel);
        assert!(
            journal.is_file(),
            "child channel journal initialized in the worktree"
        );
        assert_eq!(
            session.env.get("LFD_CHANNEL").map(String::as_str),
            Some(dispatched.channel.as_str())
        );
    }

    /// Dispatch announces the work line to the parent's live server: the
    /// wave's thread shows "work line <name> opened" (journaled as
    /// `ChannelOpened` — idempotent, so a retry knocks harmlessly).
    #[tokio::test]
    async fn dispatch_notifies_the_parent_channel_when_a_listener_is_up() {
        let repo = TestRepo::new();
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave(repo.path(), "ship", 2);
        store.create_wave(&wave).await.expect("seed wave");

        // A live listener at the wave home (discovery-file resolution).
        let runtime =
            crate::wave::runtime::WaveRuntime::open("ship".into(), repo.path().to_path_buf())
                .expect("open runtime");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = crate::wave::server::router(
            runtime.clone(),
            crate::wave::server::ResidentDoor::new("test-token"),
            None,
        );
        tokio::spawn(async move {
            axum::serve(listener, app).await.ok();
        });
        crate::wave::server::write_endpoint(repo.path(), "ship", addr).expect("pointer");

        let dispatched = dispatch(
            &store,
            "ship",
            "implement",
            "Wire it.",
            Placement::Fresh,
            None,
        )
        .await
        .expect("dispatch");
        notify_channel_opened(&store, &dispatched).await;
        notify_channel_opened(&store, &dispatched).await; // idempotent knock

        let thread = runtime.thread_snapshot();
        assert_eq!(thread.len(), 1);
        assert_eq!(
            thread[0].text,
            format!("work line {} opened", dispatched.channel)
        );
        assert_eq!(thread[0].from.as_deref(), Some("dispatch"));
    }

    /// No listener anywhere: the announcement is skipped, dispatch stands.
    #[tokio::test]
    async fn dispatch_notification_is_best_effort_without_a_listener() {
        let repo = TestRepo::new();
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave(repo.path(), "ship", 2);
        store.create_wave(&wave).await.expect("seed wave");
        let dispatched = dispatch(
            &store,
            "ship",
            "implement",
            "Wire it.",
            Placement::Fresh,
            None,
        )
        .await
        .expect("dispatch");
        notify_channel_opened(&store, &dispatched).await; // must not error/panic
    }

    /// The wrapper `lf q` launches workers with, pinned end to end: built
    /// from a real dispatch's session, it must clear the inherited-session
    /// marker before anything runs. A fresh tmux server inherits the
    /// dispatcher's env, so without the unset a mind that is itself a
    /// registered session would poison every worker into registering a
    /// duplicate row instead of adopting the one created here.
    #[tokio::test]
    async fn dispatched_worker_wrapper_unsets_the_inherited_marker() {
        let repo = TestRepo::new();
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave(repo.path(), "ship", 2);
        store.create_wave(&wave).await.expect("seed wave");

        let dispatched = dispatch(
            &store,
            "ship",
            "implement",
            "Add the thing.",
            Placement::Fresh,
            None,
        )
        .await
        .expect("dispatch");

        let wrapper = tmux_shell_command(&dispatched.session, &worker_exit_tail(false));
        assert!(
            wrapper.starts_with("unset LFD_SESSION_INHERITED; "),
            "worker wrapper must clear the marker the tmux server inherits: {wrapper}"
        );
        assert!(wrapper.contains(&format!("LFD_SESSION_ID='{}' ", dispatched.session.id)));
        // The dispatcher's auto-PR guarantee rides the wrapper, not the prompt.
        assert!(
            wrapper.contains(r#"if [ "$EXIT_CODE" -eq 0 ]; then "#)
                && wrapper.contains(" op pr; fi; "),
            "clean exits run lf op pr: {wrapper}"
        );
        // --no-pr strips it.
        let wrapper = tmux_shell_command(&dispatched.session, &worker_exit_tail(true));
        assert!(
            !wrapper.contains(" op pr"),
            "--no-pr removes the PR step: {wrapper}"
        );
    }

    #[tokio::test]
    async fn pool_placement_dispatches_into_the_shared_wave_worktree() {
        let repo = TestRepo::new();
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave(repo.path(), "ship", 2);
        store.create_wave(&wave).await.expect("seed wave");

        let dispatched = dispatch(
            &store,
            "ship",
            "design",
            "Sketch it.",
            Placement::Pool,
            None,
        )
        .await
        .expect("dispatch");

        let repo_name = repo
            .path()
            .file_name()
            .and_then(|name| name.to_str())
            .expect("repo name");
        let worktree_name = Path::new(&dispatched.run.worktree)
            .file_name()
            .and_then(|name| name.to_str())
            .expect("worktree name");
        assert_eq!(
            worktree_name,
            format!("{repo_name}.ship"),
            "pool placement shares the wave's two-segment worktree"
        );
        // Pool shares the wave worktree, so its channel IS the wave channel:
        // nothing minted in the worktree, env still names the channel.
        assert_eq!(dispatched.channel, "ship");
        assert!(
            !crate::wave::journal::journal_path(Path::new(&dispatched.run.worktree), "ship")
                .exists(),
            "no channel journal minted in the pool worktree (the wave's lives at the origin)"
        );
        assert_eq!(
            dispatched
                .session
                .env
                .get("LFD_CHANNEL")
                .map(String::as_str),
            Some("ship")
        );
    }

    #[tokio::test]
    async fn dispatch_enforces_capacity_and_parent_rules() {
        let repo = TestRepo::new();
        let tmp = tempfile::tempdir().expect("tempdir");
        let store = temp_store(tmp.path()).await;
        let wave = make_wave(repo.path(), "ship", 1);
        store.create_wave(&wave).await.expect("seed wave");

        // A worker parent is refused before anything is created.
        let first = dispatch(&store, "ship", "implement", "One.", Placement::Fresh, None)
            .await
            .expect("first dispatch");
        let err = dispatch(
            &store,
            "ship",
            "implement",
            "Two.",
            Placement::Fresh,
            Some(first.session.id.clone()),
        )
        .await
        .expect_err("worker parent refused");
        assert!(err.to_string().contains("worker sessions cannot launch"));

        // The first run occupies the wave's single worker slot.
        let mut running = first.run.clone();
        running.status = RunStatus::Running;
        store.update_run(&running).await.expect("keep running");
        let err = dispatch(&store, "ship", "implement", "Two.", Placement::Fresh, None)
            .await
            .expect_err("capacity reached");
        assert!(err.to_string().contains("worker capacity"));
    }
}
