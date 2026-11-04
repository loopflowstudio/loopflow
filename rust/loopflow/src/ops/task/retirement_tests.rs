use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;

use loopflow_test_support::TestRepo;
use rusqlite::params;

use super::{advance, now};
use crate::durable::TaskId;
use crate::store::sqlite::HostUse;
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
            .observe_task_host(&self.task.id, "native:task-window", state, now(), revision)
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
                        1
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
