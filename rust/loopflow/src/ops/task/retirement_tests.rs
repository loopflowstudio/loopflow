use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use loopflow_test_support::TestRepo;
use rusqlite::params;

use super::{advance, now, sweep};
use crate::durable::TaskId;
use crate::id::{LfProcessId, TerminalId};
use crate::store::sqlite::ViewerHold;
use crate::store::{SharedStore, StorageConfig};
use crate::work::task::Task;

struct Workspace {
    repo: TestRepo,
    _directory: tempfile::TempDir,
    checkout: PathBuf,
    remote: PathBuf,
    branch: String,
    store: SharedStore,
    task: Task,
}

struct Child(std::process::Child);

impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn retirement_open_viewers_hold_completed_checkout_and_an_ordinary_shell_does_not() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let fixture = Workspace::new().await;
        // An ordinary shell with cwd in the checkout is neither a hold nor ours to close.
        let mut shell = Child(
            Command::new("/bin/sleep")
                .arg("60")
                .current_dir(&fixture.checkout)
                .spawn()
                .unwrap(),
        );
        let first = fixture.session_terminal("first", Some(ViewerHold::Viewed(now())));
        let second = fixture.session_terminal("second", Some(ViewerHold::Viewed(now())));
        advance(&fixture.store, &fixture.task).await.unwrap();
        let intent = fixture.intent();
        assert_eq!(
            (intent.due_at, fixture.diagnostics().0.as_deref()),
            (None, Some("Retained by an open Task Session viewer"))
        );
        // A passive or hidden viewer is still an attached viewer: its exit alone releases.
        fixture
            .store
            .sqlite
            .record_viewer_hold(&first, None)
            .unwrap();
        advance(&fixture.store, &fixture.task).await.unwrap();
        assert_eq!(fixture.intent().due_at, None);
        fixture.assert_retained();
        fixture
            .store
            .sqlite
            .record_viewer_hold(&second, None)
            .unwrap();
        advance(&fixture.store, &fixture.task).await.unwrap();
        let released = fixture.intent();
        assert!(released.due_at.unwrap() >= now() + 899);
        fixture.assert_retained();
        // A renewed viewer cancels the queued deadline; leaving starts a fresh one.
        fixture
            .store
            .sqlite
            .record_viewer_hold(&first, Some(ViewerHold::Viewed(now())))
            .unwrap();
        advance(&fixture.store, &fixture.task).await.unwrap();
        assert_eq!(fixture.intent().due_at, None);
        fixture
            .store
            .sqlite
            .record_viewer_hold(&first, None)
            .unwrap();
        fixture
            .sql()
            .execute("UPDATE task_retirements SET due_at=NULL", [])
            .unwrap();
        fixture.due();
        advance(&fixture.store, &fixture.task).await.unwrap();
        assert!(!fixture.checkout.exists());
        assert!(
            shell.0.try_wait().unwrap().is_none(),
            "LF closed an ordinary shell"
        );
        assert!(fixture
            .store
            .get_task(&fixture.task.id)
            .await
            .unwrap()
            .is_some());
    });
}

#[test]
fn retirement_lost_viewers_and_ended_terminals_expire_without_reopening() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        for hold in [
            ViewerHold::Absent(now() - 901),
            ViewerHold::Viewed(now() - 931),
        ] {
            let fixture = Workspace::new().await;
            // Completion initially observed the viewer, then the terminal ended.
            fixture
                .store
                .sqlite
                .reconcile_task_retirement(
                    &fixture.task.id,
                    now() - 1000,
                    Some(ViewerHold::Viewed(now() - 1000)),
                )
                .unwrap();
            let terminal = fixture.session_terminal("lost", Some(hold));
            fixture.store.sqlite.end_terminal(&terminal, None).unwrap();
            advance(&fixture.store, &fixture.task).await.unwrap();
            assert!(!fixture.checkout.exists());
        }
    });
}

#[test]
fn retirement_waits_for_final_work_and_shell_job_settlement() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let fixture = Workspace::new().await;
        let shell = crate::id::TerminalId::new();
        fixture.store.sqlite.create_terminal(&shell, "build").unwrap();
        fixture
            .store
            .sqlite
            .create_desktop_shell(
                "shell",
                &fixture.task.id,
                fixture.checkout.to_str().unwrap(),
                None,
            )
            .unwrap();
        fixture
            .store
            .sqlite
            .bind_desktop_shell("shell", &shell)
            .unwrap();
        // The shell's terminal ended; its jobs are not yet confirmed gone.
        fixture.store.sqlite.end_terminal(&shell, Some(0)).unwrap();
        fixture.due();
        // The agent's own exit says nothing about viewers: it simply holds no viewer.
        let terminal = fixture.session_terminal("final-turn", None);
        let agent = fixture.store.sqlite.test_lf_process();
        let mut worker = Child(Command::new("/bin/sleep").arg("60").spawn().unwrap());
        let process = crate::journal::OsProcess::read(worker.0.id()).unwrap().unwrap();
        fixture.sql().execute("UPDATE processes SET started_at=?2,pid=?3,os_started_at=?2,held_terminal_id=?4 WHERE id=?1",
            params![agent,process.started_at,process.pid,terminal]).unwrap();
        fixture.sql().execute("UPDATE agent_sessions SET agent_process_id=?1 WHERE id='final-turn'", [&agent]).unwrap();
        fixture.sql().execute("INSERT INTO session_activity(session_id,agent_process_id,observed_at,open_tools,pending_input,yielded) VALUES('final-turn',?1,1,1,0,0)", [&agent]).unwrap();
        for (tools, pending, yielded) in [(1, 0, 0), (0, 1, 1), (0, 0, 0)] {
            fixture.sql().execute("UPDATE session_activity SET open_tools=?1,pending_input=?2,yielded=?3", params![tools,pending,yielded]).unwrap();
            advance(&fixture.store, &fixture.task).await.unwrap();
            fixture.assert_retained();
        }
        fixture.sql().execute("UPDATE session_activity SET open_tools=0,pending_input=0,yielded=1", []).unwrap();
        let error = advance(&fixture.store, &fixture.task).await.unwrap_err();
        assert!(error.to_string().contains("without confirmed job cleanup"));
        assert_eq!(fixture.intent().phase, "closing");
        let _ = worker.0.kill();
        worker.0.wait().unwrap();
        fixture.sql().execute("UPDATE agent_sessions SET completed_at=1 WHERE id='final-turn'", []).unwrap();
        fixture.sql().execute("UPDATE processes SET completed_at=1 WHERE id=?1", [&agent]).unwrap();
        let error = advance(&fixture.store, &fixture.task).await.unwrap_err();
        assert!(error.to_string().contains("without confirmed job cleanup"));
        fixture.assert_retained();
        fixture.store.sqlite.settle_shell_jobs(&shell).unwrap();
        advance(&fixture.store, &fixture.task).await.unwrap();
        assert!(!fixture.checkout.exists());
        assert_eq!(fixture.intent().phase, "retired");
        advance(&fixture.store, &fixture.task).await.unwrap();
    });
}

/// A held terminal is no process. The conversation's lf process names it, so
/// its work is excused, and cleanup names the terminal for the server to end.
#[test]
fn cleanup_ends_a_done_tasks_held_conversation() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let fixture = Workspace::new().await;
        fixture.due();
        let terminal = crate::id::TerminalId::new();
        fixture
            .sql()
            .execute(
                "INSERT INTO held_terminals(id,holder_build,created_at) VALUES(?1,'fixture',1)",
                [&terminal],
            )
            .unwrap();
        fixture
            .store
            .sqlite
            .test_session("held", &crate::session_record::new_artifact_key());
        fixture
            .sql()
            .execute(
                "UPDATE agent_sessions SET task_id=?1,interactive=1,held_terminal_id=?2 WHERE id='held'",
                params![fixture.task.id.as_str(), terminal],
            )
            .unwrap();
        // The conversation's lf process, a child of the holder, which records no Process.
        let live = crate::journal::OsProcess::read(std::process::id())
            .unwrap()
            .unwrap();
        let conversation = crate::process::LfProcess {
            id: LfProcessId::new(),
            pid: Some(live.pid),
            os_started_at: Some(live.started_at),
            trace_id: crate::id::TraceId::new(),
            parent_lf_process_id: None,
            via_agent: Some(false),
            caller_session_id: None,
            caller_agent_process_id: None,
            command: None,
            repo: None,
            cwd: Some(fixture.checkout.display().to_string()),
            started_at: live.started_at,
            completed_at: None,
            outcome: None,
            exit_code: None,
            signal: None,
            error: None,
        };
        fixture
            .store
            .sqlite
            .record_process_within(&conversation, Some(&terminal), std::time::Duration::ZERO)
            .unwrap();
        // Its provider, attached to it, binds the conversation to the Task.
        let agent = crate::id::AgentProcessId::new();
        fixture.sql().execute(
            "INSERT INTO processes(id,kind,trace_id,parent_lf_process_id,started_at,pid,os_started_at,
                agent_session_id,agent_provider,agent_interactive,attached_lf_process_id)
             VALUES(?1,'agent',?2,?3,?4,?5,?4,'held','claude',1,?3)",
            params![agent, conversation.trace_id, conversation.id, live.started_at, live.pid],
        ).unwrap();
        fixture
            .sql()
            .execute("UPDATE agent_sessions SET agent_process_id=?1 WHERE id='held'", [&agent])
            .unwrap();
        advance(&fixture.store, &fixture.task).await.unwrap();
        let reason = fixture.diagnostics().0.unwrap_or_default();
        assert!(!reason.contains("still has live"), "{reason}");
        assert_eq!(fixture.intent().phase, "closing");
        // Past the viewing grace since completion, cleanup names the terminal.
        let later = now() + crate::store::sqlite::WORKSPACE_GRACE;
        assert_eq!(
            fixture
                .store
                .sqlite
                .task_close_candidates(&fixture.task.id, later)
                .unwrap(),
            vec![(terminal.clone(), false)]
        );
        fixture.assert_retained();
        // The server ends the terminal; its conversation settles with it.
        fixture.sql().execute_batch(&format!(
            "UPDATE held_terminals SET ended_at=2,exit_code=0 WHERE id='{terminal}';
             UPDATE agent_sessions SET completed_at=2 WHERE id='held';
             UPDATE processes SET completed_at=2,outcome='succeeded' WHERE id IN ('{}','{agent}');",
            conversation.id
        )).unwrap();
        advance(&fixture.store, &fixture.task).await.unwrap();
        assert!(!fixture.checkout.exists());
        assert_eq!(fixture.intent().phase, "retired");
    });
}

#[test]
fn retirement_ends_a_pr_less_tasks_conversations_and_keeps_its_checkout() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let fixture = Workspace::new().await;
        fixture
            .sql()
            .execute(
                "DELETE FROM task_prs WHERE task_id=?1",
                [fixture.task.id.as_str()],
            )
            .unwrap();
        let terminal = fixture.session_terminal("pr-less", None);
        fixture.due();
        let error = advance(&fixture.store, &fixture.task).await.unwrap_err();
        assert!(error.to_string().contains("Retained PR-less checkout"));
        assert_eq!(fixture.intent().phase, "closing");
        fixture
            .sql()
            .execute(
                "UPDATE tasks SET planning_completed_at='2000-01-01T00:00:00Z' WHERE id=?1",
                [fixture.task.id.as_str()],
            )
            .unwrap();
        fixture
            .store
            .sqlite
            .record_viewer_hold(&terminal, Some(ViewerHold::Absent(0)))
            .unwrap();
        assert_eq!(
            super::conversation_closes(&fixture.store.sqlite, &fixture.task.id).unwrap(),
            vec![(terminal, false)]
        );
        fixture.assert_retained();
    });
}

/// A running held terminal, created long enough ago to be past any grace.
fn held_terminal(conn: &rusqlite::Connection) -> TerminalId {
    let terminal = TerminalId::new();
    conn.execute(
        "INSERT INTO held_terminals(id,holder_build,created_at) VALUES(?1,'fixture',1)",
        [&terminal],
    )
    .unwrap();
    terminal
}

fn git(repo: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{args:?}: {output:?}");
    String::from_utf8(output.stdout).unwrap().trim().to_owned()
}

impl Workspace {
    async fn new() -> Self {
        Self::with_store(None).await
    }

    async fn with_store(existing: Option<SharedStore>) -> Self {
        let id = TaskId::new();
        let branch = format!("completed-{id}");
        let repo = TestRepo::new();
        let directory = tempfile::tempdir().unwrap();
        let checkout = directory.path().canonicalize().unwrap().join("checkout");
        let remote = directory.path().join("remote.git");
        git(repo.path(), &["init", "--bare", remote.to_str().unwrap()]);
        git(
            repo.path(),
            &["remote", "set-url", "origin", remote.to_str().unwrap()],
        );
        git(
            repo.path(),
            &["worktree", "add", "-b", &branch, checkout.to_str().unwrap()],
        );
        git(repo.path(), &["push", "origin", &branch]);
        let head = repo.head_sha();
        let database = directory.path().join("loopflow.db");
        let store = match existing {
            Some(store) => store,
            None => Arc::new(
                crate::store::open_ephemeral_store(&StorageConfig::sqlite(database))
                    .await
                    .unwrap(),
            ),
        };
        let repository = store
            .sqlite
            .ensure_repository(repo.path().to_str().unwrap())
            .unwrap();
        {
            let conn = store.sqlite.conn.lock().unwrap();
            conn.execute("INSERT INTO tasks(id,repository_id,issue_identifier,issue_title,issue_description,created_at,updated_at,worktree,branch,workspace_slug,base_commit,planning_completed,planning_completed_at) VALUES(?1,?2,?1,'Retire workspace','',1,1,?3,?5,?5,?4,1,'2026-11-03T00:00:00Z')", params![id.as_str(),repository.as_str(),checkout.to_str().unwrap(),head,branch]).unwrap();
            conn.execute("INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,publication_requested_at,github_number,github_url,github_head_sha,merge_commit,created_at,updated_at) VALUES(?1,?1,1,?3,?3,?2,1,1,'https://example.test/pull/1',?2,?2,1,1)", params![id.as_str(),head,branch]).unwrap();
        }
        let task = store.get_task(&id).await.unwrap().unwrap();
        std::fs::write(checkout.join("discard-me"), "uncommitted work").unwrap();
        Self {
            repo,
            _directory: directory,
            checkout,
            remote,
            branch,
            store,
            task,
        }
    }

    fn sql(&self) -> std::sync::MutexGuard<'_, rusqlite::Connection> {
        self.store.sqlite.conn.lock().unwrap()
    }

    /// A Task Session whose running held terminal records its viewers' hold.
    fn session_terminal(&self, session: &str, hold: Option<ViewerHold>) -> TerminalId {
        let terminal = held_terminal(&self.sql());
        self.store
            .sqlite
            .test_session(session, &crate::session_record::new_artifact_key());
        self.sql().execute("UPDATE agent_sessions SET task_id=?1,interactive=1,held_terminal_id=?2 WHERE id=?3",
            params![self.task.id.as_str(), terminal, session]).unwrap();
        self.store
            .sqlite
            .record_viewer_hold(&terminal, hold)
            .unwrap();
        terminal
    }

    fn due(&self) {
        self.store
            .sqlite
            .reconcile_task_retirement(&self.task.id, now() - 901, None)
            .unwrap();
    }

    fn intent(&self) -> crate::store::sqlite::TaskRetirement {
        self.store
            .sqlite
            .task_retirement(&self.task.id)
            .unwrap()
            .unwrap()
    }

    fn diagnostics(&self) -> (Option<String>, u32) {
        self.sql()
            .query_row(
                "SELECT reason,failures FROM task_retirements WHERE task_id=?1",
                [self.task.id.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap()
    }

    fn assert_retained(&self) {
        assert!(self.checkout.join("discard-me").exists());
        assert!(!git(
            self.repo.path(),
            &["ls-remote", "--heads", "origin", &self.branch]
        )
        .is_empty());
        assert_eq!(
            git(self.repo.path(), &["rev-parse", &self.branch]),
            self.repo.head_sha()
        );
    }
}

#[test]
fn retirement_returning_viewer_and_reopen_cancel_closing_before_git_deletion() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        for reopen in [false, true] {
            let fixture = Workspace::new().await;
            fixture.due();
            assert!(fixture
                .store
                .sqlite
                .retirement_progress(&fixture.intent(), "closing", None)
                .unwrap());
            if reopen {
                fixture
                    .sql()
                    .execute(
                        "UPDATE tasks SET planning_completed=0 WHERE id=?1",
                        [fixture.task.id.as_str()],
                    )
                    .unwrap();
            } else {
                fixture.session_terminal("returned", Some(ViewerHold::Viewed(now())));
            }
            advance(&fixture.store, &fixture.task).await.unwrap();
            fixture.assert_retained();
            let intent = fixture
                .store
                .sqlite
                .task_retirement(&fixture.task.id)
                .unwrap();
            if reopen {
                assert!(intent.is_none());
            } else {
                let intent = intent.unwrap();
                assert_eq!((intent.phase.as_str(), intent.due_at), ("waiting", None));
            }
        }
    });
}

#[test]
fn retirement_retries_after_remote_deletion_and_worktree_failure() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let fixture = Workspace::new().await;
        fixture.due();
        git(
            fixture.repo.path(),
            &["worktree", "lock", fixture.checkout.to_str().unwrap()],
        );
        let other = Workspace::with_store(Some(fixture.store.clone())).await;
        other.due();
        let tasks = vec![fixture.task.id.clone(), other.task.id.clone()];
        let before_sweep = now();
        let error = sweep(&fixture.store, &tasks).await.unwrap_err();
        assert!(
            !other.checkout.exists(),
            "one failure starved the next Task"
        );
        let failed = fixture.intent();
        let (reason, failures) = fixture.diagnostics();
        assert_eq!(failures, 1);
        assert!(failed.retry_at.unwrap() >= before_sweep + 60);
        assert!(reason.unwrap().contains("locked"));
        // The next invocation observes backoff without changing the absence deadline.
        sweep(&fixture.store, &tasks).await.unwrap();
        assert_eq!(fixture.diagnostics().1, 1);
        assert_eq!(fixture.intent().due_at, failed.due_at);
        assert!(error.to_string().contains("locked"), "{error}");
        assert!(git(
            fixture.repo.path(),
            &["ls-remote", "--heads", "origin", &fixture.branch]
        )
        .is_empty());
        assert!(fixture.checkout.join("discard-me").exists());
        assert_eq!(
            fixture
                .store
                .sqlite
                .task_retirement(&fixture.task.id)
                .unwrap()
                .unwrap()
                .phase,
            "closing"
        );
        // A failed first effect must not be replayed: reject every later push.
        let hook = fixture.remote.join("hooks/pre-receive");
        std::fs::write(&hook, "#!/bin/sh\nexit 1\n").unwrap();
        std::fs::set_permissions(&hook, std::fs::Permissions::from_mode(0o755)).unwrap();
        git(
            fixture.repo.path(),
            &["worktree", "unlock", fixture.checkout.to_str().unwrap()],
        );
        fixture
            .sql()
            .execute("UPDATE task_retirements SET retry_at=0", [])
            .unwrap();
        let pending = fixture.store.sqlite.pending_retirements().unwrap();
        sweep(&fixture.store, &pending).await.unwrap();
        assert!(!fixture.checkout.exists());
        assert!(git(fixture.repo.path(), &["branch", "--list", &fixture.branch]).is_empty());
        assert!(fixture.repo.path().join(".git").exists());
        assert!(fixture
            .store
            .get_task(&fixture.task.id)
            .await
            .unwrap()
            .is_some());
        advance(&fixture.store, &fixture.task).await.unwrap();
    });
}

#[test]
fn retirement_sweep_empty_queue_and_interrupted_close_are_finite() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let fixture = Workspace::new().await;
        assert!(fixture
            .store
            .sqlite
            .pending_retirements()
            .unwrap()
            .is_empty());
        sweep(&fixture.store, &[]).await.unwrap();
        fixture.due();
        fixture
            .store
            .sqlite
            .retirement_progress(&fixture.intent(), "closing", None)
            .unwrap();
        let pending = fixture.store.sqlite.pending_retirements().unwrap();
        sweep(&fixture.store, &pending).await.unwrap();
        assert!(!fixture.checkout.exists());
        assert!(fixture
            .store
            .sqlite
            .pending_retirements()
            .unwrap()
            .is_empty());
    });
}

#[test]
fn conversation_cleanup_owns_grace_membership_and_work() {
    let guard = crate::journal::TestLedgerGuard::new();
    let store = crate::journal::process_ledger().unwrap();
    let conn = rusqlite::Connection::open(guard.home().join("loopflow.db")).unwrap();
    let repo = store.ensure_repository("/repo").unwrap();
    let task = TaskId::new();
    conn.execute("INSERT INTO tasks(id,repository_id,issue_identifier,created_at,planning_completed,planning_completed_at)
        VALUES(?1,?2,'CLOSE-1',1,1,'2000-01-01T00:00:00Z')", params![task.as_str(),repo.as_str()]).unwrap();
    let terminal = held_terminal(&conn);
    let agent = store.test_lf_process();
    // A live agent is needed to exercise Task-wide work retention.
    let live = crate::journal::OsProcess::read(std::process::id())
        .unwrap()
        .unwrap();
    conn.execute(
        "UPDATE processes SET pid=?2,os_started_at=?3,held_terminal_id=?4 WHERE id=?1",
        params![agent, live.pid, live.started_at, terminal],
    )
    .unwrap();
    store.test_session("conversation", &crate::session_record::new_artifact_key());
    conn.execute("UPDATE agent_sessions SET task_id=?1,interactive=1,held_terminal_id=?2,agent_process_id=?3 WHERE id='conversation'", params![task.as_str(),terminal,agent]).unwrap();
    assert!(store.pending_retirements().unwrap().contains(&task));
    assert!(store.task_retirement(&task).unwrap().is_none());
    let request = || {
        super::conversation_closes(&store, &task)
            .unwrap()
            .into_iter()
            .find(|(host, _)| host == &terminal)
            .map(|(_, abandoned)| abandoned)
    };
    assert_eq!(request(), Some(false));
    // A conversation resumed after its checkout retired still closes, and the
    // finished retirement stays finished.
    conn.execute(
        "INSERT INTO task_retirements(task_id,completion,checkout,branch,due_at,phase)
         VALUES(?1,'2000-01-01T00:00:00Z','/removed','retired',0,'retired')",
        [task.as_str()],
    )
    .unwrap();
    assert_eq!(request(), Some(false));
    let phase: String = conn
        .query_row("SELECT phase FROM task_retirements", [], |r| r.get(0))
        .unwrap();
    assert_eq!(phase, "retired");
    conn.execute("DELETE FROM task_retirements", []).unwrap();
    // Returning holds cleanup even with no checkout or retirement row.
    store
        .record_viewer_hold(&terminal, Some(ViewerHold::Viewed(now())))
        .unwrap();
    assert_eq!(request(), None);
    store
        .record_viewer_hold(&terminal, Some(ViewerHold::Absent(now())))
        .unwrap();
    assert_eq!(request(), None);
    conn.execute(
        "UPDATE held_terminals SET viewer_hold_at=0 WHERE id=?1",
        [&terminal],
    )
    .unwrap();
    assert_eq!(request(), Some(false));
    for change in [
        "UPDATE tasks SET planning_completed=0",
        "UPDATE tasks SET planning_completed_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')",
        "UPDATE agent_sessions SET primary_scope='repository'",
        "UPDATE agent_sessions SET primary_scope='wave'",
        "UPDATE agent_sessions SET held_terminal_id=NULL",
    ] {
        conn.execute_batch(change).unwrap();
        assert_eq!(request(), None, "{change}");
        conn.execute_batch("UPDATE tasks SET planning_completed=1,planning_completed_at='2000-01-01T00:00:00Z'; UPDATE agent_sessions SET primary_scope=NULL;").unwrap();
        conn.execute("UPDATE agent_sessions SET held_terminal_id=?1", [&terminal])
            .unwrap();
        assert_eq!(request(), Some(false));
    }
    // Requested work keeps execution until it yields.
    conn.execute("INSERT INTO session_activity(session_id,agent_process_id,observed_at,open_tools,pending_input,yielded)
        VALUES('conversation',?1,1,0,1,1)", [&agent]).unwrap();
    for (tools, pending, yielded) in [(1, 0, 0), (0, 1, 1), (0, 0, 0)] {
        conn.execute(
            "UPDATE session_activity SET open_tools=?1,pending_input=?2,yielded=?3",
            params![tools, pending, yielded],
        )
        .unwrap();
        assert_eq!(request(), None);
    }
    conn.execute(
        "UPDATE session_activity SET open_tools=0,pending_input=0,yielded=1",
        [],
    )
    .unwrap();
    assert_eq!(request(), Some(false));
    // Work in another conversation holds the Task even through checkout membership.
    let other = store.test_lf_process();
    let other_terminal = held_terminal(&conn);
    conn.execute("UPDATE tasks SET worktree='/repo.task',checkout_machine_id=(SELECT id FROM machines WHERE is_local=1)", []).unwrap();
    conn.execute(
        "UPDATE processes SET pid=?2,os_started_at=?3,held_terminal_id=?4 WHERE id=?1",
        params![other, live.pid, live.started_at, other_terminal],
    )
    .unwrap();
    store.test_session("other", &crate::session_record::new_artifact_key());
    conn.execute("UPDATE agent_sessions SET cwd='/repo.task/nested',interactive=1,held_terminal_id=?1,agent_process_id=?2 WHERE id='other'", params![other_terminal, other]).unwrap();
    assert_eq!(request(), Some(false));
    conn.execute("INSERT INTO session_activity(session_id,agent_process_id,observed_at,open_tools,pending_input,yielded)
        VALUES('other',?1,1,0,1,1)", [&other]).unwrap();
    assert_eq!(request(), None);
    conn.execute(
        "UPDATE agent_sessions SET completed_at=1 WHERE id='other'",
        [],
    )
    .unwrap();
    assert_eq!(request(), Some(false));
    let graph = store.test_lf_process();
    conn.execute(
        "INSERT INTO task_graphs(task_id,graph,step,run_start,lf_process_id,updated_at)
        VALUES(?1,'{\"name\":\"test\",\"steps\":[]}','start','{\"index\":0}',?2,1)",
        params![task.as_str(), graph],
    )
    .unwrap();
    assert_eq!(request(), None);
    conn.execute("UPDATE processes SET completed_at=1 WHERE id=?1", [&graph])
        .unwrap();
    assert_eq!(request(), Some(false));
    // Abandonment bypasses grace and pending work, but cleanup protects viewers.
    conn.execute("UPDATE session_activity SET yielded=0", [])
        .unwrap();
    conn.execute("UPDATE tasks SET abandoned_at=unixepoch()", [])
        .unwrap();
    assert_eq!(request(), Some(true));
    store
        .record_viewer_hold(&terminal, Some(ViewerHold::Viewed(now())))
        .unwrap();
    assert_eq!(request(), None);
    store
        .record_viewer_hold(&terminal, Some(ViewerHold::Absent(now())))
        .unwrap();
    assert_eq!(request(), Some(true));
}

#[test]
fn abandoned_checkout_retries_after_live_work_exits_and_keeps_dirty_files() {
    let ledger = crate::journal::TestLedgerGuard::new();
    let bin = ledger.home().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let gh = bin.join("gh");
    std::fs::write(
        &gh,
        "#!/bin/sh\n[ \"$1 $2\" = 'pr list' ] || exit 1\nprintf '[]\\n'\n",
    )
    .unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap());
    let _environment = crate::test_ambient::EnvGuard::clear(&["PATH"]);
    std::env::set_var("PATH", path);
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let fixture = Workspace::new().await;
        fixture
            .sql()
            .execute(
                "UPDATE tasks SET planning_completed=0,planning_completed_at=NULL,abandoned_at=1,
            checkout_machine_id=(SELECT id FROM machines WHERE is_local=1) WHERE id=?1",
                [fixture.task.id.as_str()],
            )
            .unwrap();
        fixture
            .sql()
            .execute("UPDATE task_prs SET merge_commit=NULL", [])
            .unwrap();
        let task = fixture
            .store
            .get_task(&fixture.task.id)
            .await
            .unwrap()
            .unwrap();
        let terminal = fixture.session_terminal("headless", None);
        fixture
            .sql()
            .execute(
                "UPDATE agent_sessions SET interactive=0 WHERE id='headless'",
                [],
            )
            .unwrap();
        // The Run's lf process, live in its terminal.
        let run = fixture.store.sqlite.test_lf_process();
        let mut worker = Child(Command::new("/bin/sleep").arg("60").spawn().unwrap());
        let process = crate::journal::OsProcess::read(worker.0.id())
            .unwrap()
            .unwrap();
        fixture
            .sql()
            .execute(
                "UPDATE processes SET pid=?2,os_started_at=?3,held_terminal_id=?4,agent_session_id='headless' WHERE id=?1",
                params![run, process.pid, process.started_at, terminal],
            )
            .unwrap();
        let pending = || fixture.store.sqlite.pending_retirements().unwrap();
        assert!(pending().contains(&task.id));
        assert!(sweep(&fixture.store, &pending()).await.is_err());
        fixture.assert_retained();
        worker.0.kill().unwrap();
        worker.0.wait().unwrap();
        fixture
            .sql()
            .execute("UPDATE processes SET completed_at=1 WHERE id=?1", [&run])
            .unwrap();
        fixture.store.sqlite.end_terminal(&terminal, Some(0)).unwrap();
        assert!(
            pending().contains(&task.id),
            "ending the last terminal lost cleanup"
        );
        fixture
            .sql()
            .execute("UPDATE task_retirements SET retry_at=0", [])
            .unwrap();
        let error = sweep(&fixture.store, &pending()).await.unwrap_err();
        assert!(error.to_string().contains("uncommitted changes"), "{error}");
        fixture.assert_retained();
        std::fs::remove_file(fixture.checkout.join("discard-me")).unwrap();
        fixture
            .sql()
            .execute("UPDATE task_retirements SET retry_at=0", [])
            .unwrap();
        sweep(&fixture.store, &pending()).await.unwrap();
        assert!(!fixture.checkout.exists());
        assert!(
            !pending().contains(&task.id),
            "successful cleanup must leave the queue"
        );
        fixture
            .sql()
            .execute(
                "UPDATE tasks SET abandoned_at=NULL WHERE id=?1",
                [task.id.as_str()],
            )
            .unwrap();
        assert!(fixture
            .store
            .sqlite
            .task_retirement(&task.id)
            .unwrap()
            .is_none());
        assert!(git(fixture.repo.path(), &["branch", "--list", &fixture.branch]).is_empty());
    });
}
