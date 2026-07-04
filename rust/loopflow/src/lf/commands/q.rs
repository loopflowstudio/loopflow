//! `lf q` — daemonless dispatch against the shared run registry.
//!
//! `lf q worker run <wave> --flow F --task T` does what lfd's worker endpoint
//! used to do, with no daemon in the path: resolve placement (fresh worktree
//! by default, `--pool` for the wave's shared tree, `--stack <run-id>` for a
//! branch forked from an unlanded run), write the Run + Session rows to the
//! shared store, and launch `lf <flow>: <task>` in a detached tmux session.
//!
//! # Env contract (matches the lfd executor byte for byte)
//!
//! The worker's tmux environment carries `LFD_WAVE_ID`, `LFD_RUN_ID`,
//! `LF_RUN_ID`, `LFD_SESSION_ID` (the session row created here) and
//! `LFD_AGENT_ROLE=worker`. `LFD_SESSION_INHERITED` is deliberately absent:
//! a session id without the marker means "this very process owns the row"
//! (see `lf::session`), so the child does NOT self-register a second row.
//! The child's parentage is the dispatcher's: `LFD_SESSION_ID` in *this*
//! process's env (the mind's session, when the mind dispatches) becomes the
//! worker session's `parent_session_id`.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;

use anyhow::{anyhow, Result};
use time::OffsetDateTime;

use crate::lf::{QCommand, WorkerCommand};
use crate::lfd::executor::helpers::{build_lf_dispatch_command, shell_escape, tmux_exit_file};
use crate::lfd::executor::{create_run_for_placement, Placement};
use crate::lfd::id::LfdId;
use crate::lfd::store::{open_existing_store, SharedStore};
use crate::lfd::types::{
    tmux_session_name, Run, Session, SessionStatus, SessionUse, TMUX_TERMINAL_SOURCE,
};

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
                },
        } => worker_run(wave, flow, task, *pool, stack.as_deref()),
    }
}

fn worker_run(wave: &str, flow: &str, task: &str, pool: bool, stack: Option<&str>) -> Result<()> {
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
        launch_worker_tmux(&store, dispatched.session.clone()).await?;
        println!("dispatched {flow} worker for wave '{wave}'");
        println!("  run       {}", dispatched.run.id);
        println!("  session   {}", dispatched.session.id);
        println!("  tmux      {}", dispatched.session.tmux_name);
        println!("  worktree  {}", dispatched.run.worktree);
        Ok(())
    })
}

/// A recorded dispatch: the Run row (placement resolved, worktree created)
/// and the Session row the worker will own.
#[derive(Debug)]
pub(crate) struct Dispatch {
    pub run: Run,
    pub session: Session,
}

/// Write the dispatch into the registry: resolve placement into a worktree +
/// branch, create the Run row, then the worker's Session row carrying the
/// env contract. No process is spawned here.
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

    let session_id = LfdId::new();
    let env = BTreeMap::from([
        ("LFD_WAVE_ID".to_string(), wave.id().to_string()),
        ("LFD_RUN_ID".to_string(), run.id.to_string()),
        ("LF_RUN_ID".to_string(), run.id.to_string()),
        ("LFD_SESSION_ID".to_string(), session_id.to_string()),
        (
            "LFD_AGENT_ROLE".to_string(),
            SessionUse::Worker.as_str().to_string(),
        ),
        // LFD_SESSION_INHERITED deliberately absent: the child owns this row.
    ]);
    let cmd =
        build_lf_dispatch_command(flow, task, run.direction.as_slice(), &run.area, wave.name());
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
    Ok(Dispatch { run, session })
}

/// Launch the worker in a detached tmux session — the same wrapper the lfd
/// executor uses: env exported inline, exit code written to the session's
/// exit file so any running lfd's reconciliation can close the row.
async fn launch_worker_tmux(store: &SharedStore, session: Session) -> Result<()> {
    let exit_file = tmux_exit_file(Path::new(&session.cwd), &session.id);
    let exit_dir = exit_file
        .parent()
        .expect("tmux exit file always has a parent");
    let env_prefix = session
        .env
        .iter()
        .map(|(key, value)| format!("{key}={} ", shell_escape(value)))
        .collect::<String>();
    let command = session
        .argv
        .iter()
        .map(|arg| shell_escape(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let shell_command = format!(
        "mkdir -p {exit_dir}; rm -f {exit_file}; {env_prefix}{command}; EXIT_CODE=$?; printf '%s' \"$EXIT_CODE\" > {exit_file}; exit \"$EXIT_CODE\"",
        exit_dir = shell_escape(&exit_dir.display().to_string()),
        exit_file = shell_escape(&exit_file.display().to_string()),
    );

    let status = tokio::process::Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            &session.tmux_name,
            "-c",
            &session.cwd,
            "/bin/zsh",
            "-lc",
            &shell_command,
        ])
        .status()
        .await;
    let launched = status.map(|status| status.success());
    let mut session = session;
    match launched {
        Ok(true) => {
            let _ = tokio::process::Command::new("tmux")
                .args(["set-option", "-t", &session.tmux_name, "mouse", "on"])
                .status()
                .await;
            session.start();
            store.update_control_session(&session).await?;
            Ok(())
        }
        Ok(false) | Err(_) => {
            session.complete(1);
            store.update_control_session(&session).await?;
            Err(anyhow!("tmux failed to launch the worker session"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use loopflow_test_support::TestRepo;

    use crate::lf::session::classify_run_context;
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{RepoWork, RunStatus, Wave, WaveMode, WaveStatus};

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
            crons: Vec::new(),
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
        assert_eq!(
            session.argv.last().map(String::as_str),
            Some("Add the thing.")
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
                session.env.get("LFD_WAVE_ID").map(String::as_str),
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
