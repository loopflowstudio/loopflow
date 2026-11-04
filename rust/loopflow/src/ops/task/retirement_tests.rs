use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use loopflow_test_support::TestRepo;
use rusqlite::params;

use super::{advance, now, owner_evidence, reconcile_hosts};
use crate::durable::TaskId;
use crate::store::sqlite::{HostUse, TaskHostOwner};
use crate::store::{SharedStore, StorageConfig};
use crate::work::task::Task;

struct Workspace {
    repo: TestRepo,
    _directory: tempfile::TempDir,
    checkout: PathBuf,
    remote: PathBuf,
    store: SharedStore,
    task: Task,
}

struct Host(std::process::Child);

impl Host {
    fn new() -> Self {
        Self(Command::new("/bin/sleep").arg("60").spawn().unwrap())
    }

    fn owner(&self, fixture: &Workspace, settles_on_exit: bool) -> TaskHostOwner {
        let process = crate::journal::OsProcess::read(self.0.id())
            .unwrap()
            .unwrap();
        TaskHostOwner {
            machine_id: fixture.store.sqlite.local_machine().unwrap().id,
            pid: process.pid,
            started_at: process.started_at,
            settles_on_exit,
        }
    }

    fn exit(&mut self) {
        self.0.kill().unwrap();
        self.0.wait().unwrap();
    }
}

impl Drop for Host {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

#[test]
fn retirement_confirmed_host_exit_finishes_without_desktop_and_still_waits_for_shell_jobs() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let fixture = Workspace::new().await;
        let mut desktop = Host::new();
        let owner = desktop.owner(&fixture, true);
        fixture
            .store
            .sqlite
            .observe_task_host(
                &fixture.task.id,
                "desktop",
                HostUse::Retained,
                now() - 1000,
                1,
                false,
                Some(&owner),
            )
            .unwrap();
        reconcile_hosts(&fixture.store, &fixture.task.id)
            .await
            .unwrap();
        // Even an expired lease cannot release a live process. Keep the executor watching.
        assert!(!advance(&fixture.store, &fixture.task).await.unwrap());
        fixture.assert_retained();
        desktop.exit();
        reconcile_hosts(&fixture.store, &fixture.task.id)
            .await
            .unwrap();
        assert!(!advance(&fixture.store, &fixture.task).await.unwrap());
        let intent = fixture
            .store
            .sqlite
            .task_retirement(&fixture.task.id)
            .unwrap()
            .unwrap();
        assert!(intent.due_at.unwrap() >= now() + 899);
        assert!(fixture
            .store
            .sqlite
            .retirement_hosts_settled(&fixture.task.id)
            .unwrap());
        fixture.assert_retained();

        let shell = fixture.store.sqlite.test_lf_process();
        fixture
            .store
            .sqlite
            .create_task_shell(
                "shell",
                &fixture.task.id,
                fixture.checkout.to_str().unwrap(),
            )
            .unwrap();
        fixture
            .store
            .sqlite
            .bind_task_shell("shell", &shell)
            .unwrap();
        fixture
            .sql()
            .execute("UPDATE processes SET completed_at=1 WHERE id=?1", [&shell])
            .unwrap();
        fixture
            .sql()
            .execute("UPDATE task_retirements SET due_at=?1", [now() - 1])
            .unwrap();
        let agent = fixture.store.sqlite.test_lf_process();
        let mut worker = Host::new();
        let worker_owner = worker.owner(&fixture, false);
        fixture.sql().execute("UPDATE processes SET started_at=?2,pid=?3,os_started_at=?2 WHERE id=?1",
            params![agent,worker_owner.started_at,worker_owner.pid]).unwrap();
        fixture.store.sqlite.test_session("final-turn", &crate::session_record::new_artifact_key());
        fixture.sql().execute("UPDATE agent_sessions SET task_id=?1,interactive=1,transport_lf_process_id=?2,agent_process_id=?2 WHERE id='final-turn'",
            params![fixture.task.id.as_str(),agent]).unwrap();
        fixture.sql().execute("INSERT INTO session_activity(session_id,agent_process_id,observed_at,open_tools,pending_input,yielded) VALUES('final-turn',?1,1,1,0,0)", [&agent]).unwrap();
        for (tools, pending, yielded) in [(1, 0, 0), (0, 1, 1), (0, 0, 0)] {
            fixture.sql().execute("UPDATE session_activity SET open_tools=?1,pending_input=?2,yielded=?3", params![tools,pending,yielded]).unwrap();
            assert!(!advance(&fixture.store, &fixture.task).await.unwrap());
            fixture.assert_retained();
        }
        fixture.sql().execute("UPDATE session_activity SET open_tools=0,pending_input=0,yielded=1", []).unwrap();
        fixture.store.sqlite.record_owner_input(&agent, b"unfinished draft").unwrap();
        assert!(!advance(&fixture.store, &fixture.task).await.unwrap());
        fixture.store.sqlite.clear_owner_input(&agent).unwrap();
        worker.exit();
        fixture.sql().execute("UPDATE agent_sessions SET completed_at=1 WHERE id='final-turn'", []).unwrap();
        fixture.sql().execute("UPDATE processes SET completed_at=1 WHERE id=?1", [&agent]).unwrap();
        let error = advance(&fixture.store, &fixture.task).await.unwrap_err();
        assert!(error.to_string().contains("without confirmed job cleanup"));
        fixture.assert_retained();
        fixture.store.sqlite.settle_shell_jobs(&shell).unwrap();
        assert!(advance(&fixture.store, &fixture.task).await.unwrap());
        assert!(!fixture.checkout.exists());
        assert!(fixture
            .store
            .get_task(&fixture.task.id)
            .await
            .unwrap()
            .is_some());
    });
}

#[test]
fn retirement_rex_client_loss_and_unreachable_origin_do_not_prove_retaining_server_exit() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let fixture = Workspace::new().await;
        let mut client = Host::new();
        let mut server = Host::new();
        let owner = server.owner(&fixture, false);
        fixture
            .store
            .sqlite
            .observe_task_host(
                &fixture.task.id,
                "rex",
                HostUse::Retained,
                now() - 1000,
                1,
                false,
                Some(&owner),
            )
            .unwrap();
        client.exit();
        reconcile_hosts(&fixture.store, &fixture.task.id)
            .await
            .unwrap();
        assert!(!advance(&fixture.store, &fixture.task).await.unwrap());
        fixture.assert_retained();
        let remote = TaskHostOwner {
            machine_id: crate::durable::MachineId::new(),
            ..owner.clone()
        };
        assert_eq!(
            owner_evidence(&fixture.store, &remote).await,
            crate::journal::ProcessIdentityEvidence::Unknown
        );
        // Reusing a PID with another start time does not identify the current process as the old owner.
        let replaced = TaskHostOwner {
            started_at: owner.started_at - 100,
            ..owner.clone()
        };
        assert_eq!(
            owner_evidence(&fixture.store, &replaced).await,
            crate::journal::ProcessIdentityEvidence::Dead
        );
        server.exit();
        reconcile_hosts(&fixture.store, &fixture.task.id)
            .await
            .unwrap();
        assert!(!advance(&fixture.store, &fixture.task).await.unwrap());
        assert!(!fixture
            .store
            .sqlite
            .retirement_hosts_settled(&fixture.task.id)
            .unwrap());
        fixture
            .sql()
            .execute("UPDATE task_retirements SET due_at=?1", [now() - 1])
            .unwrap();
        assert!(advance(&fixture.store, &fixture.task)
            .await
            .unwrap_err()
            .to_string()
            .contains("Waiting for native hosts"));
        fixture.assert_retained();
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
            &[
                "worktree",
                "add",
                "-b",
                "completed",
                checkout.to_str().unwrap(),
            ],
        );
        git(repo.path(), &["push", "origin", "completed"]);
        let head = repo.head_sha();
        let database = directory.path().join("loopflow.db");
        let store = Arc::new(
            crate::store::open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
                .await
                .unwrap(),
        );
        let repository = store
            .sqlite
            .ensure_repository(repo.path().to_str().unwrap())
            .unwrap();
        let id = TaskId::new();
        let conn = rusqlite::Connection::open(database).unwrap();
        conn.execute("INSERT INTO tasks(id,repository_id,issue_identifier,issue_title,issue_description,created_at,updated_at,worktree,branch,workspace_slug,base_commit,planning_completed,planning_completed_at) VALUES(?1,?2,'RETIRE-1','Retire workspace','',1,1,?3,'completed','completed',?4,1,'2026-11-03T00:00:00Z')", params![id.as_str(),repository.as_str(),checkout.to_str().unwrap(),head]).unwrap();
        conn.execute("INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,publication_requested_at,github_number,github_url,github_head_sha,merge_commit,created_at,updated_at) VALUES('pr-retirement',?1,1,'completed','completed',?2,1,1,'https://example.test/pull/1',?2,?2,1,1)", params![id.as_str(),head]).unwrap();
        let task = store.get_task(&id).await.unwrap().unwrap();
        std::fs::write(checkout.join("discard-me"), "uncommitted work").unwrap();
        Self {
            repo,
            _directory: directory,
            checkout,
            remote,
            store,
            task,
        }
    }

    fn sql(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(self._directory.path().join("loopflow.db")).unwrap()
    }

    fn host(&self, state: HostUse, revision: i64) {
        self.store
            .sqlite
            .observe_task_host(
                &self.task.id,
                "native:task-window",
                state,
                now(),
                revision,
                state == HostUse::Closed,
                None,
            )
            .unwrap();
    }

    fn due(&self) {
        self.store
            .sqlite
            .reconcile_task_retirement(&self.task.id, now() - 901)
            .unwrap();
    }

    fn assert_retained(&self) {
        assert!(self.checkout.join("discard-me").exists());
        assert!(!git(
            self.repo.path(),
            &["ls-remote", "--heads", "origin", "completed"]
        )
        .is_empty());
        assert_eq!(
            git(self.repo.path(), &["rev-parse", "completed"]),
            self.repo.head_sha()
        );
    }
}

#[test]
fn retirement_return_and_reopen_cancel_closing_before_git_deletion() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        for reopen in [false, true] {
            let fixture = Workspace::new().await;
            fixture.host(HostUse::Away, 1);
            fixture.due();
            let error = advance(&fixture.store, &fixture.task).await.unwrap_err();
            assert!(
                error.to_string().contains("Waiting for native hosts"),
                "{error}"
            );
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
            fixture.assert_retained();
            if reopen {
                fixture
                    .sql()
                    .execute(
                        "UPDATE tasks SET planning_completed=0 WHERE id=?1",
                        [fixture.task.id.as_str()],
                    )
                    .unwrap();
            } else {
                fixture.host(HostUse::Retained, 2);
            }
            assert!(advance(&fixture.store, &fixture.task).await.unwrap());
            fixture.assert_retained();
            let intent = fixture
                .store
                .sqlite
                .task_retirement(&fixture.task.id)
                .unwrap();
            if reopen {
                assert!(intent.is_none());
            } else {
                assert!(intent.unwrap().due_at.is_none());
                // A delayed old close cannot undo the return.
                assert!(fixture
                    .store
                    .sqlite
                    .observe_task_host(
                        &fixture.task.id,
                        "native:task-window",
                        HostUse::Closed,
                        now(),
                        1,
                        true,
                        None
                    )
                    .is_err());
                fixture.host(HostUse::Closed, 3);
                assert!(advance(&fixture.store, &fixture.task).await.unwrap());
                assert!(!fixture.checkout.exists());
                assert!(fixture
                    .store
                    .get_task(&fixture.task.id)
                    .await
                    .unwrap()
                    .is_some());
            }
        }
    });
}

#[test]
fn retirement_acknowledged_quit_finishes_without_another_host_report() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let fixture = Workspace::new().await;
        fixture.host(HostUse::Away, 1);
        let departure = now() - 300;
        let intent = fixture
            .store
            .sqlite
            .reconcile_task_retirement(&fixture.task.id, departure)
            .unwrap()
            .unwrap();
        assert_eq!(intent.due_at, Some(departure + 900));
        fixture
            .store
            .sqlite
            .observe_task_host(
                &fixture.task.id,
                "native:task-window",
                HostUse::Away,
                now(),
                2,
                true,
                None,
            )
            .unwrap();
        assert!(!advance(&fixture.store, &fixture.task).await.unwrap());
        fixture.assert_retained();
        assert_eq!(
            fixture
                .store
                .sqlite
                .task_retirement(&fixture.task.id)
                .unwrap()
                .unwrap()
                .due_at,
            intent.due_at
        );

        // Advance the persisted deadline; the quit host never reports again.
        fixture
            .sql()
            .execute("UPDATE task_retirements SET due_at=?1", [now() - 1])
            .unwrap();
        assert!(advance(&fixture.store, &fixture.task).await.unwrap());
        assert!(!fixture.checkout.exists());
        assert!(git(
            fixture.repo.path(),
            &["ls-remote", "--heads", "origin", "completed"]
        )
        .is_empty());
        assert!(fixture
            .store
            .get_task(&fixture.task.id)
            .await
            .unwrap()
            .is_some());
    });
}

#[test]
fn retirement_unacknowledged_quit_keeps_checkout_and_recovers_on_return() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let fixture = Workspace::new().await;
        fixture
            .store
            .sqlite
            .observe_task_host(
                &fixture.task.id,
                "native:task-window",
                HostUse::Retained,
                now() - 1000,
                1,
                false,
                None,
            )
            .unwrap();
        assert!(advance(&fixture.store, &fixture.task).await.unwrap());
        fixture.assert_retained();
        let intent = fixture
            .store
            .sqlite
            .task_retirement(&fixture.task.id)
            .unwrap()
            .unwrap();
        assert!(intent.due_at.is_none());
        assert!(intent.reason.unwrap().contains("unavailable"));
        fixture.host(HostUse::Retained, 3);
        assert!(fixture
            .store
            .sqlite
            .observe_task_host(
                &fixture.task.id,
                "native:task-window",
                HostUse::Away,
                now(),
                2,
                true,
                None
            )
            .is_err());
        assert!(advance(&fixture.store, &fixture.task).await.unwrap());
        fixture.assert_retained();
    });
}

#[test]
fn retirement_requires_job_settlement_after_owner_death_and_retries() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let fixture = Workspace::new().await;
        let owner = fixture.store.sqlite.test_lf_process();
        fixture
            .store
            .sqlite
            .create_task_shell(
                "shell",
                &fixture.task.id,
                fixture.checkout.to_str().unwrap(),
            )
            .unwrap();
        fixture
            .store
            .sqlite
            .bind_task_shell("shell", &owner)
            .unwrap();
        fixture
            .sql()
            .execute("UPDATE processes SET completed_at=1 WHERE id=?1", [&owner])
            .unwrap();
        fixture.host(HostUse::Closed, 1);
        let error = advance(&fixture.store, &fixture.task).await.unwrap_err();
        assert!(
            error.to_string().contains("without confirmed job cleanup"),
            "{error}"
        );
        fixture.assert_retained();
        fixture.store.sqlite.settle_shell_jobs(&owner).unwrap();
        assert!(advance(&fixture.store, &fixture.task).await.unwrap());
        assert!(!fixture.checkout.exists());
        assert_eq!(
            fixture
                .store
                .sqlite
                .task_retirement(&fixture.task.id)
                .unwrap()
                .unwrap()
                .phase,
            "retired"
        );
        assert!(advance(&fixture.store, &fixture.task).await.unwrap());
    });
}

#[test]
fn retirement_retries_after_remote_deletion_and_worktree_failure() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let fixture = Workspace::new().await;
        fixture.host(HostUse::Closed, 1);
        git(
            fixture.repo.path(),
            &["worktree", "lock", fixture.checkout.to_str().unwrap()],
        );
        let error = advance(&fixture.store, &fixture.task).await.unwrap_err();
        assert!(error.to_string().contains("locked"), "{error}");
        assert!(git(
            fixture.repo.path(),
            &["ls-remote", "--heads", "origin", "completed"]
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
        assert!(advance(&fixture.store, &fixture.task).await.unwrap());
        assert!(!fixture.checkout.exists());
        assert!(git(fixture.repo.path(), &["branch", "--list", "completed"]).is_empty());
        assert!(fixture.repo.path().join(".git").exists());
        assert!(fixture
            .store
            .get_task(&fixture.task.id)
            .await
            .unwrap()
            .is_some());
        assert!(advance(&fixture.store, &fixture.task).await.unwrap());
    });
}
