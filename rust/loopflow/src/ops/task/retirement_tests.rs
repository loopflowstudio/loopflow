use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use loopflow_test_support::TestRepo;
use rusqlite::params;

use super::{advance, now, sweep};
use crate::durable::TaskId;
use crate::id::LfProcessId;
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
        let first = fixture.session_owner("first", Some(ViewerHold::Viewed(now())));
        let second = fixture.session_owner("second", Some(ViewerHold::Viewed(now())));
        advance(&fixture.store, &fixture.task).await.unwrap();
        let intent = fixture.intent();
        assert_eq!(
            (intent.due_at, intent.reason.as_deref()),
            (None, Some("Retained by an open Task Session viewer"))
        );
        // A passive or hidden viewer is still an attached viewer: its exit alone releases.
        fixture
            .store
            .sqlite
            .record_owner_viewer_hold(&first, None)
            .unwrap();
        advance(&fixture.store, &fixture.task).await.unwrap();
        assert_eq!(fixture.intent().due_at, None);
        fixture.assert_retained();
        fixture
            .store
            .sqlite
            .record_owner_viewer_hold(&second, None)
            .unwrap();
        advance(&fixture.store, &fixture.task).await.unwrap();
        let released = fixture.intent();
        assert!(released.due_at.unwrap() >= now() + 899);
        fixture.assert_retained();
        // A renewed viewer cancels the queued deadline; leaving starts a fresh one.
        fixture
            .store
            .sqlite
            .record_owner_viewer_hold(&first, Some(ViewerHold::Viewed(now())))
            .unwrap();
        advance(&fixture.store, &fixture.task).await.unwrap();
        assert_eq!(fixture.intent().due_at, None);
        fixture
            .store
            .sqlite
            .record_owner_viewer_hold(&first, None)
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
fn retirement_lost_viewers_and_crashed_owners_expire_without_reopening() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        for hold in [
            ViewerHold::Absent(now() - 901),
            ViewerHold::Viewed(now() - 931),
        ] {
            let fixture = Workspace::new().await;
            // Completion initially observed the viewer, then the owner died.
            fixture
                .store
                .sqlite
                .reconcile_task_retirement(
                    &fixture.task.id,
                    now() - 1000,
                    Some(ViewerHold::Viewed(now() - 1000)),
                )
                .unwrap();
            let owner = fixture.session_owner("lost", Some(hold));
            fixture
                .sql()
                .execute("UPDATE processes SET completed_at=1 WHERE id=?1", [&owner])
                .unwrap();
            advance(&fixture.store, &fixture.task).await.unwrap();
            assert!(!fixture.checkout.exists());
        }
    });
}

#[test]
fn retirement_waits_for_final_work_drafts_and_shell_job_settlement() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let fixture = Workspace::new().await;
        let shell = fixture.store.sqlite.test_lf_process();
        fixture
            .store
            .sqlite
            .create_desktop_shell(
                "shell",
                &fixture.task.id,
                fixture.checkout.to_str().unwrap(),
            )
            .unwrap();
        fixture
            .store
            .sqlite
            .bind_desktop_shell("shell", &shell)
            .unwrap();
        fixture
            .sql()
            .execute("UPDATE processes SET completed_at=1 WHERE id=?1", [&shell])
            .unwrap();
        fixture.due();
        // The agent's own exit says nothing about viewers: it simply holds no viewer.
        let agent = fixture.session_owner("final-turn", None);
        let mut worker = Child(Command::new("/bin/sleep").arg("60").spawn().unwrap());
        let process = crate::journal::OsProcess::read(worker.0.id()).unwrap().unwrap();
        fixture.sql().execute("UPDATE processes SET started_at=?2,pid=?3,os_started_at=?2 WHERE id=?1",
            params![agent,process.started_at,process.pid]).unwrap();
        fixture.sql().execute("UPDATE agent_sessions SET agent_process_id=?1 WHERE id='final-turn'", [&agent]).unwrap();
        fixture.sql().execute("INSERT INTO session_activity(session_id,agent_process_id,observed_at,open_tools,pending_input,yielded) VALUES('final-turn',?1,1,1,0,0)", [&agent]).unwrap();
        for (tools, pending, yielded) in [(1, 0, 0), (0, 1, 1), (0, 0, 0)] {
            fixture.sql().execute("UPDATE session_activity SET open_tools=?1,pending_input=?2,yielded=?3", params![tools,pending,yielded]).unwrap();
            advance(&fixture.store, &fixture.task).await.unwrap();
            fixture.assert_retained();
        }
        fixture.sql().execute("UPDATE session_activity SET open_tools=0,pending_input=0,yielded=1", []).unwrap();
        assert!(fixture.store.sqlite.try_record_owner_input(&agent, b"unfinished draft").unwrap());
        advance(&fixture.store, &fixture.task).await.unwrap();
        fixture.store.sqlite.clear_owner_input(&agent).unwrap();
        worker.0.kill().unwrap();
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

    /// A Task Session whose live serving owner records its viewers' hold.
    fn session_owner(&self, session: &str, hold: Option<ViewerHold>) -> LfProcessId {
        let owner = self.store.sqlite.test_lf_process();
        let live = crate::journal::OsProcess::read(std::process::id())
            .unwrap()
            .unwrap();
        self.sql()
            .execute(
                "UPDATE processes SET pid=?2,started_at=?3,os_started_at=?3 WHERE id=?1",
                params![owner, live.pid, live.started_at],
            )
            .unwrap();
        self.store
            .sqlite
            .test_session(session, &crate::session_record::new_artifact_key());
        self.sql().execute("UPDATE agent_sessions SET task_id=?1,interactive=1,transport_lf_process_id=?2 WHERE id=?3",
            params![self.task.id.as_str(), owner, session]).unwrap();
        self.store
            .sqlite
            .record_owner_viewer_hold(&owner, hold)
            .unwrap();
        owner
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
                fixture.session_owner("returned", Some(ViewerHold::Viewed(now())));
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
        let error = sweep(&fixture.store, &tasks).await.unwrap_err();
        assert!(
            !other.checkout.exists(),
            "one failure starved the next Task"
        );
        let failed = fixture.intent();
        assert_eq!(failed.failures, 1);
        assert!(failed.retry_at.unwrap() >= now() + 59);
        assert!(failed.reason.unwrap().contains("locked"));
        // The next invocation observes backoff without changing the absence deadline.
        sweep(&fixture.store, &tasks).await.unwrap();
        assert_eq!(fixture.intent().failures, 1);
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
