use super::delivery::apply_merged_task_landing;
use super::{
    checkout_execution_boundary, lock_task_pr_mutation, resolve_task_create_input,
    task_event_process_refusal,
};
use crate::child::ChildRef;
use crate::durable::{TaskState, WorkRef, WorkStatus};
use crate::linear::{LinearIssueId, LinearProjectId};
use crate::ops::pr_landing::{reconcile_pr_landing, wait_with_io, LandingIo, LandingObservation};
use crate::pr_landing::{NewPrLanding, PrLanding, PrLandingState};
use crate::store::{SharedStore, StorageConfig};
use crate::test_linear_input::SeedLinear;
use crate::work::project::{Project, ProjectId};
use crate::work::task::{
    GithubPr, Observation, PrMergeMode, PrMergeRequest, PrPresentation, PrPublication, Task,
    TaskEventKind, TaskId, TaskPr, TaskPrId,
};
use crate::work::wave::Wave;
use std::ffi::OsString;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

struct TaskFixture {
    _database: tempfile::TempDir,
    database_path: std::path::PathBuf,
    store: SharedStore,
    task: Task,
    work: WorkRef,
}

struct EnvRestore(Vec<(&'static str, Option<OsString>)>);

impl EnvRestore {
    fn capture(names: &[&'static str]) -> Self {
        Self(
            names
                .iter()
                .map(|name| (*name, std::env::var_os(name)))
                .collect(),
        )
    }
}

impl Drop for EnvRestore {
    fn drop(&mut self) {
        for (name, value) in &self.0 {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

async fn task_fixture(identifier: &str) -> TaskFixture {
    let repository =
        std::fs::canonicalize(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.."))
            .unwrap();
    task_fixture_at(identifier, repository).await
}

#[test]
fn paused_snapshot_never_recommends_running_work() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let repo = loopflow_test_support::TestRepo::new();
    let fixture = runtime.block_on(task_fixture_at("DEFER-1", repo.path().into()));
    fixture
        .store
        .sqlite
        .set_task_deferred(&fixture.task.id, true)
        .unwrap();
    let snapshot = runtime
        .block_on(super::read_task_snapshot(&fixture.store, fixture.task))
        .unwrap();
    assert_eq!(snapshot.status, crate::durable::TaskState::Deferred);
    assert_eq!(
        snapshot.actions.recommended,
        Some(crate::ops::task_actions::TaskAction::NoAction)
    );
    assert!(snapshot.actions.reason.contains("lf task resume DEFER-1"));
}

#[test]
fn task_decision_preserves_unknown_history_and_live_process_protection() {
    let ledger = crate::journal::TestLedgerGuard::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let repo = loopflow_test_support::TestRepo::new();
    let fixture = runtime.block_on(task_fixture_at("ACCEPT-HISTORY", repo.path().into()));
    let mut pr = runtime
        .block_on(fixture.store.task_prs(&fixture.task.id))
        .unwrap()
        .remove(0);
    repo.create_branch(&pr.branch);
    pr.base_commit = repo.head_sha();
    runtime
        .block_on(fixture.store.sync_task_placement(
            &pr.task_id,
            &pr.base_commit,
            false,
            pr.updated_at,
        ))
        .unwrap();

    rusqlite::Connection::open(&fixture.database_path)
        .unwrap()
        .execute(
            "DELETE FROM task_prs WHERE task_id=?1",
            [fixture.task.id.as_str()],
        )
        .unwrap();
    let process = crate::process::LfProcess {
        kind: crate::process::ProcessKind::Lf,
        agent_session_id: None,
        os_started_at: None,
        id: crate::id::LfProcessId::new(),
        pid: None,
        trace_id: crate::id::TraceId::new(),
        parent_lf_process_id: None,
        via_agent: None,
        caller_session_id: None,
        caller_agent_process_id: None,
        command: Some("historical diagnostic".into()),
        repo: None,
        cwd: Some(repo.path().to_string_lossy().into_owned()),
        started_at: time::OffsetDateTime::now_utc().unix_timestamp(),
        completed_at: None,
        outcome: None,
        exit_code: None,
        signal: None,
        error: None,
    };
    fixture.store.sqlite.record_process(&process).unwrap();
    let session = fixture.store.sqlite.test_session(
        "reserved-session",
        &crate::session_record::new_artifact_key(),
    );
    let conn = rusqlite::Connection::open(&fixture.database_path).unwrap();
    conn.execute("UPDATE agent_sessions SET cwd=?1,task_id=?2,wave_id=?3,input_published=0,interactive=0 WHERE id=?4",
            rusqlite::params![repo.path().to_str().unwrap(), fixture.task.id.as_str(), fixture.task.wave_id.as_ref().unwrap().as_str(), session.id]).unwrap();
    conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES(?1,'captured',?2,1,'{}')",
            rusqlite::params![session.id, crate::session_record::new_artifact_key()]).unwrap();
    conn.execute(
        "UPDATE agent_sessions SET current_capture=?1 WHERE id=?2",
        rusqlite::params![conn.last_insert_rowid(), session.id],
    )
    .unwrap();
    fixture
        .store
        .sqlite
        .record_session_event(
            &session.id,
            &"thread".into(),
            "turn",
            crate::session::SessionEventKind::Started,
            &serde_json::json!({}),
        )
        .unwrap();
    let before = fixture.store.sqlite.session(&session.id).unwrap();
    // A reserved input and an unanswered turn are history; only the
    // Process with unknown evidence blocks.
    let blockers = crate::ops::task_execution::task_execution_blockers(
        &fixture.store.sqlite,
        &fixture.task.id,
    )
    .unwrap();
    assert_eq!(blockers.len(), 1, "{blockers:?}");
    assert!(blockers[0].contains(&format!("lf history show {}", process.id)));
    let gate = runtime
        .block_on(super::task_completion_refusal(
            &fixture.store,
            &fixture.task,
        ))
        .unwrap();
    assert!(gate.is_none(), "{:?}", gate);

    // Exact live identity does not change the decision or confer stop authority.
    let root = ledger.home().join(crate::journal::PROCESS_RECEIPT_ROOT);
    std::fs::create_dir_all(&root).unwrap();
    let pid = std::process::id();
    let receipt = crate::journal::ProcessReceipt {
        schema_version: 1,
        trace_id: process.trace_id.to_string(),
        lf_process_id: process.id.to_string(),
        pid,
        started_at: crate::journal::process_started_at(pid).unwrap().unwrap(),
    };
    let receipt_path = root.join(format!("{}.json", process.id));
    let receipt_bytes = serde_json::to_vec(&receipt).unwrap();
    std::fs::write(&receipt_path, &receipt_bytes).unwrap();
    assert!(runtime
        .block_on(super::task_completion_refusal(
            &fixture.store,
            &fixture.task
        ))
        .unwrap()
        .is_none());
    for _ in 0..2 {
        runtime
            .block_on(
                fixture.store.complete_task(
                    &fixture.task,
                    fixture
                        .store
                        .sqlite
                        .request_task_completion(&fixture.task.id, None)
                        .unwrap()
                        .unwrap_or(0),
                ),
            )
            .unwrap();
        runtime
            .block_on(super::cleanup_completed_task(&fixture.store, &fixture.task))
            .unwrap();
    }
    assert!(repo.path().exists());
    assert_eq!(
        runtime
            .block_on(super::task_work_status(&fixture.store, &fixture.task))
            .unwrap(),
        WorkStatus::Done
    );
    assert_eq!(
        fixture.store.sqlite.process(&process.id).unwrap(),
        Some(process)
    );
    assert_eq!(fixture.store.sqlite.session(&session.id).unwrap(), before);
    assert_eq!(std::fs::read(receipt_path).unwrap(), receipt_bytes);
    assert!(crate::ops::task_execution::task_execution_blockers(
        &fixture.store.sqlite,
        &fixture.task.id,
    )
    .unwrap()
    .iter()
    .any(|reason| reason.contains("live or unresolved")));
}

#[test]
fn prepared_placement_consumes_accepted_facts_before_checkout_or_execution() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    {
        let repo = loopflow_test_support::TestRepo::new();
        let fixture = runtime.block_on(task_fixture_at("EXISTING", repo.path().to_path_buf()));
        std::env::set_var("LF_HOME", fixture._database.path());
        std::fs::create_dir_all(repo.path().join(".lf/wave/task-recovery")).unwrap();
        std::fs::write(
            repo.path().join(".lf/wave/task-recovery/GOAL.md"),
            "Recover work.",
        )
        .unwrap();
        let mut snapshot: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        snapshot.projects.truncate(1);
        snapshot.items.truncate(1);
        snapshot.projects[0].id = "999bdbdd-c045-41a6-8ffc-a97c4a40b0b3".into();
        snapshot.projects[0].status = crate::work::project::ProjectStatus::Started;
        snapshot.items[0].id = "new-issue".into();
        snapshot.items[0].completed = false;
        snapshot.items[0].completed_at = None;
        snapshot.items[0].state = Some("unstarted".into());
        snapshot.items[0].project_id = Some("999bdbdd-c045-41a6-8ffc-a97c4a40b0b3".into());
        snapshot.items[0].revision = Some("2026-10-05T12:00:00Z".into());
        snapshot.items[0].name = "Accepted after resolution".into();
        snapshot.items[0].description = "Preserve this direction".into();
        snapshot.items[0].revision = Some("2026-10-05T13:00:00Z".into());
        runtime
            .block_on(fixture.store.seed_inventory(
                crate::test_linear_input::InventoryFixture {
                    wave_id: fixture.task.wave_id.clone().unwrap(),
                    provider: "linear".into(),
                    initiative: snapshot.projects[0].initiative_ids[0].clone(),
                    synced_at: 7,
                    snapshot,
                },
                None,
            ))
            .unwrap();
        let checkout = repo.path().join("reserved-checkout");
        let prepared = |path| super::TaskPlacement {
            plan: crate::git::worktrees::PlacementPlan {
                base_ref: repo.head_sha(),
                branch: "retained-branch".into(),
                worktree_path: path,
                strategy: crate::git::worktrees::PlacementStrategy::UseExistingWorktree,
            },
            workspace_slug: "reserved-checkout".into(),
            stack_parent: None,
            github: None,
            requested_agent: None,
            directive: None,
        };
        let saved = runtime
            .block_on(fixture.store.get_task_by_issue("new-issue"))
            .unwrap()
            .unwrap();
        let task = runtime
            .block_on(super::place_prepared_task(
                &fixture.store,
                &saved.id,
                prepared(checkout.clone()),
            ))
            .unwrap();
        {
            assert_eq!(task.title, "Accepted after resolution");
            assert_eq!(task.description, "Preserve this direction");
            assert_eq!(task.pm_snapshot_synced_at, Some(7));
            assert_eq!(task.worktree.as_ref(), Some(&checkout));
            assert_eq!(
                runtime.block_on(fixture.store.get_task(&task.id)).unwrap(),
                Some(task.clone())
            );
            assert!(runtime
                .block_on(fixture.store.active_task_pr(&task.id))
                .unwrap()
                .is_none());
            assert_eq!(task.branch, "retained-branch");
            assert_eq!(task.base_commit, repo.head_sha());
        }
        assert!(runtime
            .block_on(fixture.store.get_task(&fixture.task.id))
            .unwrap()
            .is_some());
    }
}
#[test]
fn explicit_reconciliation_recovers_merged_delivery_without_completing_work() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    let _environment = EnvRestore::capture(&["PATH"]);
    let repo = loopflow_test_support::TestRepo::new();
    assert!(std::process::Command::new("git")
        .current_dir(repo.path())
        .args([
            "remote",
            "set-url",
            "origin",
            "https://github.com/example/repo.git"
        ])
        .output()
        .unwrap()
        .status
        .success());
    let bin = tempfile::tempdir().unwrap();
    let gh = bin.path().join("gh");
    std::fs::write(
            &gh,
            "#!/bin/sh\nfixture=$(dirname \"$0\")\ncase \"$1\" in\npr) cat \"$fixture/discovery.json\" ;;\napi) cat \"$fixture/pr.json\" ;;\nesac\n",
        )
        .unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let prior_path = std::env::var_os("PATH").unwrap_or_default();
    std::env::set_var(
        "PATH",
        std::env::join_paths(
            std::iter::once(bin.path().to_path_buf()).chain(std::env::split_paths(&prior_path)),
        )
        .unwrap(),
    );
    std::fs::write(
        bin.path().join("pr.json"),
        serde_json::json!({
            "number":1283,"html_url":"https://github.com/example/repo/pull/1283",
            "state":"closed","merged":true,"mergeable_state":null,
            "merge_commit_sha":"merged-commit","merged_at":"2026-09-25T21:56:42Z",
            "head":{"sha":"authored-head"}
        })
        .to_string(),
    )
    .unwrap();
    let viewer = repo.path().join("unrelated-viewer.html");
    std::fs::write(&viewer, "keep this authored artifact").unwrap();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    for (discovery, merged) in [
        ("[]", false),
        ("[{\"number\":1283},{\"number\":1284}]", false),
        ("invalid response", false),
        ("[{\"number\":1283}]", true),
    ] {
        std::fs::write(bin.path().join("discovery.json"), discovery).unwrap();
        let mut fixture = runtime.block_on(task_fixture_at("RECOVER-PR", repo.path().into()));
        let before = runtime
            .block_on(fixture.store.task_prs(&fixture.task.id))
            .unwrap();
        let cached = runtime
            .block_on(super::reconcile_task_pr(&fixture.store, &mut fixture.task))
            .unwrap()
            .unwrap();
        assert!(cached.publication.is_none());
        let pr = runtime
            .block_on(super::reconcile_task_pr_observation(
                &fixture.store,
                &mut fixture.task,
                crate::ops::pr::PrReadFreshness::Fresh,
            ))
            .unwrap()
            .unwrap();
        assert_eq!(pr.id, before[0].id);
        assert_eq!(
            pr.merge_commit.as_deref(),
            merged.then_some("merged-commit")
        );
        assert_eq!(
            pr.github().map(|github| github.number),
            merged.then_some(1283)
        );
        assert_eq!(
            runtime
                .block_on(fixture.store.task_prs(&fixture.task.id))
                .unwrap()
                .len(),
            1
        );
        assert_ne!(
            runtime
                .block_on(super::task_work_status(&fixture.store, &fixture.task))
                .unwrap(),
            WorkStatus::Done
        );
        assert_eq!(
            std::fs::read_to_string(&viewer).unwrap(),
            "keep this authored artifact"
        );
        if merged {
            let saved = runtime
                .block_on(fixture.store.task_prs(&fixture.task.id))
                .unwrap()
                .remove(0);
            let repeated = runtime
                .block_on(super::reconcile_task_pr_observation(
                    &fixture.store,
                    &mut fixture.task,
                    crate::ops::pr::PrReadFreshness::Fresh,
                ))
                .unwrap()
                .unwrap();
            assert_eq!(repeated, saved);
        } else {
            assert!(pr.publication.is_none());
        }
    }
}

#[test]
fn a_dead_graph_is_history_while_a_live_graph_process_retains_the_checkout() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(task_fixture("WORK-1"));
    let worktree = fixture
        .task
        .worktree
        .as_ref()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    let blockers = || {
        crate::ops::task_execution::task_execution_blockers(&fixture.store.sqlite, &fixture.task.id)
            .unwrap()
    };
    fixture.store.sqlite.test_graph(
        "code",
        &worktree,
        &[("implement", Some("failed"))],
        Some("failed"),
    );
    assert!(
        blockers().is_empty(),
        "a Graph whose process exited is history"
    );
    let live = fixture
        .store
        .sqlite
        .test_graph("code", &worktree, &[("implement", None)], None);
    assert!(blockers()
        .iter()
        .any(|reason| reason.contains(live.as_str())));
    let conn = rusqlite::Connection::open(&fixture.database_path).unwrap();
    conn.execute(
            "UPDATE processes SET completed_at=started_at,outcome='interrupted' WHERE completed_at IS NULL",
            [],
        )
        .unwrap();
    assert!(blockers().is_empty());

    // A reboot proves stale execution exited without settling the Graph.
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let process = crate::process::LfProcess {
        kind: crate::process::ProcessKind::Lf,
        agent_session_id: None,
        os_started_at: None,
        id: crate::id::LfProcessId::new(),
        pid: None,
        trace_id: crate::id::TraceId::new(),
        parent_lf_process_id: None,
        via_agent: None,
        caller_session_id: None,
        caller_agent_process_id: None,
        command: Some("implement".into()),
        repo: None,
        cwd: Some(
            fixture
                .task
                .worktree
                .as_ref()
                .unwrap()
                .to_string_lossy()
                .into_owned(),
        ),
        started_at: now - 100,
        completed_at: None,
        outcome: None,
        exit_code: None,
        signal: None,
        error: None,
    };
    fixture.store.sqlite.record_process(&process).unwrap();
    let session = fixture
        .store
        .sqlite
        .test_session("stalled", &crate::session_record::new_artifact_key());
    conn.execute(
        "UPDATE agent_sessions SET cwd=?1 WHERE id=?2",
        rusqlite::params![
            fixture.task.worktree.as_ref().unwrap().to_str().unwrap(),
            session.id
        ],
    )
    .unwrap();
    conn.execute(
            "INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,observed_at,payload)
             VALUES(?1,'thread','turn','started','turn',?2,'{}')",
            rusqlite::params![session.id, now - 100],
        )
        .unwrap();
    crate::journal::set_test_machine_booted_at(Some(now - 200));
    let unresolved = blockers();
    crate::journal::set_test_machine_booted_at(Some(now - 50));
    let after_boot = blockers();
    crate::journal::set_test_machine_booted_at(None);
    // The stalled Session's unanswered turn is history, not execution.
    assert_eq!(unresolved.len(), 1, "{unresolved:?}");
    assert!(unresolved[0].contains(process.id.as_str()));
    assert!(after_boot.is_empty(), "{after_boot:?}");
    assert_eq!(
        fixture.store.sqlite.process(&process.id).unwrap(),
        Some(process)
    );
    assert_eq!(
        fixture
            .store
            .sqlite
            .task_graphs(&fixture.task.id)
            .unwrap()
            .len(),
        2
    );
}

async fn task_fixture_at(identifier: &str, repository: std::path::PathBuf) -> TaskFixture {
    let database = tempfile::tempdir().unwrap();
    let database_path = database.path().join("loopflow.db");
    let store = std::sync::Arc::new(
        crate::store::open_ephemeral_store(&StorageConfig::sqlite(database_path.clone()))
            .await
            .unwrap(),
    );
    let now = time::OffsetDateTime::now_utc();
    let wave = Wave::new(
        crate::id::WaveId::new(),
        "task-recovery".to_string(),
        repository.display().to_string(),
    );
    let project = Project {
        id: ProjectId::new(),

        summary: String::new(),
        status: crate::work::project::ProjectStatus::Started,
        linear_id: Some(LinearProjectId::new("999bdbdd-c045-41a6-8ffc-a97c4a40b0b3").unwrap()),
        slug: "task-recovery".to_string(),
        name: "Task recovery".to_string(),
        description: "Keep automatic Task recovery bounded.".to_string(),
        krs: Vec::new(),
        metric_targets: Vec::new(),
        pm_snapshot_synced_at: Some(now.unix_timestamp()),

        wave_id: wave.id().clone(),
        created_at: now,
        updated_at: now,
    };
    let task = Task {
        repository_id: store.sqlite.ensure_repository(wave.repo()).unwrap(),
        id: TaskId::new(),

        revision: 0,
        linear_id: Some(LinearIssueId::new(format!("{identifier}-issue")).unwrap()),
        identifier: identifier.to_string(),
        title: "Task recovery fixture".to_string(),
        description: String::new(),
        pm_snapshot_synced_at: Some(now.unix_timestamp()),

        pm_writeback: crate::work::task::LinearWritebackState::Current,
        wave_id: Some(wave.id().clone()),
        project_id: Some(project.id.clone()),
        worktree: Some(repository),
        workspace_slug: "task-recovery-fixture".to_string(),
        branch: "test/task-recovery-fixture".to_string(),
        base_commit: "deadbeef".to_string(),
        parent_pr_id: None,
        agent: None,
        created_at: now,
        updated_at: now,
        rank: 0,
        assignee: None,
        due_date: None,
        state: None,
        completed_at: None,
        observation: Observation::NotRequired,
    };
    let pr = TaskPr {
        id: TaskPrId::new(),
        task_id: task.id.clone(),
        sequence: 1,
        slug: task.workspace_slug.clone(),
        branch: format!("test/{}", task.workspace_slug),
        base_commit: "deadbeef".to_string(),
        parent_pr_id: None,
        publication: None,
        merge_commit: None,
        abandoned_at: None,
        ci_observation: None,
        github_observation: None,
        linear_attachment_id: None,
        linear_comment_id: None,
        linear_link_error: None,
        created_at: now,
        updated_at: now,
    };
    store.create_wave(&wave).await.unwrap();
    store.create_project(&project).await.unwrap();
    crate::store::sqlite::project_selection::write_project_binding(
        &store.sqlite,
        wave.id(),
        None,
        project.linear_id.as_ref().unwrap().as_str(),
        &crate::store::PlanningLocks::new(tempfile::tempfile().unwrap()),
    )
    .unwrap();
    store.seed_task(&task, &pr).await.unwrap();
    let work = store
        .work_for_child(&ChildRef::Task(task.id.clone()))
        .await
        .unwrap();
    TaskFixture {
        _database: database,
        database_path,
        store,
        task,
        work,
    }
}

#[test]
fn task_checkout_machine_survives_without_lifecycle_reconciliation() {
    let repo = loopflow_test_support::TestRepo::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(task_fixture_at("FILES-1", repo.path().to_path_buf()));
    let local = fixture.store.sqlite.local_machine().unwrap().id;
    let remote = crate::durable::MachineId::new();
    fixture
        .store
        .sqlite
        .add_machine(&remote, "worker", "worker")
        .unwrap();
    let mut pr = runtime
        .block_on(fixture.store.active_task_pr(&fixture.task.id))
        .unwrap()
        .unwrap();
    repo.create_branch(&pr.branch);
    pr.base_commit = super::git_output(repo.path(), &["rev-parse", "HEAD"])
        .unwrap()
        .trim()
        .into();
    rusqlite::Connection::open(&fixture.database_path)
        .unwrap()
        .execute(
            "UPDATE tasks SET base_commit=?2 WHERE id=?1",
            rusqlite::params![fixture.task.id.as_str(), pr.base_commit],
        )
        .unwrap();
    std::fs::create_dir_all(repo.path().join("scratch")).unwrap();
    std::fs::write(repo.path().join("scratch/note.md"), "local note\n").unwrap();
    let original_task = runtime
        .block_on(fixture.store.get_task(&fixture.task.id))
        .unwrap()
        .unwrap();
    let connection = rusqlite::Connection::open(&fixture.database_path).unwrap();
    connection
        .execute(
            "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd)
             VALUES('checkout-session','Retained','human',1,0,?1)",
            [repo.path().to_str().unwrap()],
        )
        .unwrap();
    assert_eq!(
        fixture
            .store
            .sqlite
            .task_work(&fixture.task.id)
            .unwrap()
            .sessions
            .len(),
        1
    );
    connection.execute_batch(
            "CREATE TRIGGER no_file_task_update BEFORE UPDATE ON tasks BEGIN SELECT RAISE(FAIL,'file access cannot update Task'); END;
             CREATE TRIGGER no_file_pr_update BEFORE UPDATE ON task_prs BEGIN SELECT RAISE(FAIL,'file access cannot reconcile PR'); END;"
        ).unwrap();
    crate::ops::linear::LINEAR_TEST_CONTEXT.sync_scope(
        crate::ops::linear::LinearTestContext {
            path: fixture.database_path.clone(),
            store: fixture.store.clone(),
            graphql_url: "http://127.0.0.1:1".into(),
        },
        || {
            let snapshot = super::task_snapshot(&fixture.task).unwrap();
            assert_eq!(snapshot.machine_id, Some(local.clone()));
            assert_eq!(snapshot.task_id, fixture.task.id.as_str());
            assert_eq!(snapshot.pr.as_ref().map(|pr| &pr.id), Some(&pr.id));
            for issue in [
                "FILES-1",
                fixture.task.id.as_str(),
                &fixture.task.id.as_str()[5..9],
                &fixture.task.id.as_str()[..9],
                fixture.task.linear_id.as_ref().unwrap().as_str(),
            ] {
                let file = super::task_file(issue, "scratch/note.md", false).unwrap();
                assert_eq!(file.content.as_deref(), Some("local note\n"));
                let changes = super::task_changes(issue, "parent").unwrap();
                assert_eq!(changes.base_commit, pr.base_commit);
                assert_eq!(changes.scratch, ["scratch/note.md"]);
                let diff = super::task_diff(issue, Some("scratch/note.md"), "head", None).unwrap();
                assert!(diff.patch.contains("+local note"));
            }
            let file = super::task_file("FILES-1", "scratch/note.md", false).unwrap();
            let saved = super::file_save::task_save(
                "FILES-1",
                "scratch/note.md",
                file.revision.as_deref().unwrap(),
                "saved\n",
            )
            .unwrap();
            assert!(saved.published);
            assert_eq!(saved.file.content.as_deref(), Some("saved\n"));
        },
    );
    assert_eq!(
        runtime
            .block_on(fixture.store.get_task(&fixture.task.id))
            .unwrap()
            .unwrap(),
        original_task
    );
    assert_eq!(
        runtime
            .block_on(fixture.store.active_task_pr(&fixture.task.id))
            .unwrap()
            .unwrap(),
        pr
    );
    connection
        .execute_batch("PRAGMA foreign_keys=OFF; DELETE FROM task_prs; DELETE FROM projects;")
        .unwrap();
    crate::ops::linear::LINEAR_TEST_CONTEXT.sync_scope(
        crate::ops::linear::LinearTestContext {
            path: fixture.database_path.clone(),
            store: fixture.store.clone(),
            graphql_url: "http://127.0.0.1:1".into(),
        },
        || {
            let file = super::task_file("FILES-1", "scratch/note.md", false).unwrap();
            let saved = super::task_save(
                "FILES-1",
                "scratch/note.md",
                file.revision.as_deref().unwrap(),
                "no PR needed\n",
            )
            .unwrap();
            assert!(saved.published);
            assert_eq!(saved.file.content.as_deref(), Some("no PR needed\n"));
            let directory = super::task_files("FILES-1", "scratch", None, false).unwrap();
            assert_eq!(directory.entries[0].path, "scratch/note.md");
            let changes = super::task_changes("FILES-1", "parent").unwrap();
            assert_eq!(changes.base_commit, pr.base_commit);
            assert_eq!(changes.scratch, ["scratch/note.md"]);
            let conflict = super::task_save(
                "FILES-1",
                "scratch/note.md",
                file.revision.as_deref().unwrap(),
                "stale",
            )
            .unwrap();
            assert!(!conflict.published);
            assert_eq!(conflict.file.content, saved.file.content);
        },
    );
}

#[test]
fn task_checkout_machine_unknown_or_remote_never_reads_a_local_path() {
    let repo = loopflow_test_support::TestRepo::new();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(task_fixture_at("LOCATION-1", repo.path().into()));
    std::fs::write(repo.path().join("note.md"), "local file").unwrap();
    let remote = crate::durable::MachineId::new();
    fixture
        .store
        .sqlite
        .add_machine(&remote, "remote", "remote")
        .unwrap();
    let connection = rusqlite::Connection::open(&fixture.database_path).unwrap();
    connection
        .execute(
            "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd)
             VALUES('unbound','Local conversation','human',1,0,?1),
                   ('bound','Explicit conversation','human',1,0,?1)",
            [repo.path().to_str().unwrap()],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE agent_sessions SET task_id=?1,wave_id=?2 WHERE id='bound'",
            rusqlite::params![
                fixture.task.id.as_str(),
                fixture.task.wave_id.as_ref().unwrap().as_str()
            ],
        )
        .unwrap();
    for machine in [None, Some(remote.as_str())] {
        connection
            .execute(
                "UPDATE tasks SET checkout_machine_id=?1 WHERE id=?2",
                rusqlite::params![machine, fixture.task.id.as_str()],
            )
            .unwrap();
        crate::ops::linear::LINEAR_TEST_CONTEXT.sync_scope(
            crate::ops::linear::LinearTestContext {
                path: fixture.database_path.clone(),
                store: fixture.store.clone(),
                graphql_url: "http://127.0.0.1:1".into(),
            },
            || {
                let expected = if machine.is_none() {
                    "Machine is unknown"
                } else {
                    "belongs to Machine"
                };
                assert!(super::task_file("LOCATION-1", "note.md", false)
                    .unwrap_err()
                    .to_string()
                    .contains(expected));
                assert!(super::task_changes("LOCATION-1", "head")
                    .unwrap_err()
                    .to_string()
                    .contains(expected));
                assert!(super::task_files("LOCATION-1", "", None, false)
                    .unwrap_err()
                    .to_string()
                    .contains(expected));
                let work = fixture.store.sqlite.task_work(&fixture.task.id).unwrap();
                assert_eq!(
                    work.sessions
                        .iter()
                        .map(|s| s.id.as_str())
                        .collect::<Vec<_>>(),
                    ["bound"]
                );
            },
        );
    }
    assert_eq!(
        std::fs::read_to_string(repo.path().join("note.md")).unwrap(),
        "local file"
    );
}

#[test]
fn task_files_compare_net_parent_head_and_revisioned_drafts() {
    let repo = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        super::git_output(repo.path(), args)
            .unwrap()
            .trim()
            .to_string()
    };
    git(&["init", "-q", "-b", "main"]);
    let commit = || {
        git(&[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-qm",
            "fixture",
        ]);
    };
    std::fs::write(repo.path().join("cancelled.txt"), "base\n").unwrap();
    std::fs::write(repo.path().join("old.txt"), "rename me\n").unwrap();
    git(&["add", "."]);
    commit();
    let parent = git(&["rev-parse", "HEAD"]);
    std::fs::write(repo.path().join("cancelled.txt"), "committed\n").unwrap();
    git(&["mv", "old.txt", "new.txt"]);
    git(&["add", "."]);
    commit();
    std::fs::write(repo.path().join("cancelled.txt"), "base\n").unwrap();
    std::fs::create_dir_all(repo.path().join("scratch/nested")).unwrap();
    std::fs::write(
        repo.path().join("scratch/nested/notes.md"),
        "\u{feff}notes\r\n",
    )
    .unwrap();
    let id = TaskId::new();
    let workspace = super::TaskComparison {
        checkout: super::TaskWorkspace {
            issue_identifier: "TEST-1",
            task_id: &id,
            worktree: repo.path(),
        },
        base_commit: &parent,
    };
    let changes = super::snapshot::changes_snapshot(workspace).unwrap();
    assert!(!changes
        .files
        .iter()
        .any(|file| file.path == "cancelled.txt"));
    let renamed = changes
        .files
        .iter()
        .find(|file| file.path == "new.txt")
        .unwrap();
    assert_eq!(renamed.old_path.as_deref(), Some("old.txt"));
    assert_eq!(changes.scratch, ["scratch/nested/notes.md"]);
    let patch = super::snapshot::diff_snapshot(workspace, Some("new.txt")).unwrap();
    assert!(patch.patch.contains("rename from old.txt"));
    let head = super::resolve_file_base(repo.path(), &parent, "head").unwrap();
    let head_changes = super::snapshot::changes_snapshot(super::TaskComparison {
        base_commit: &head,
        ..workspace
    })
    .unwrap();
    assert!(head_changes
        .files
        .iter()
        .any(|file| file.path == "cancelled.txt"));
    assert!(!head_changes.files.iter().any(|file| file.path == "new.txt"));
    let file = super::file_snapshot(workspace.checkout, "scratch/nested/notes.md").unwrap();
    assert_eq!(file.content.as_deref(), Some("\u{feff}notes\r\n"));
    assert_eq!(file.revision.as_ref().unwrap().len(), 64);
    let draft = super::snapshot::draft_snapshot(workspace, "new.txt", "local draft\n").unwrap();
    assert!(draft.patch.contains("--- a/old.txt\n+++ b/new.txt\n"));
    assert!(draft.patch.contains("-rename me"));
    assert!(draft.patch.contains("+local draft"));
    let quoted =
        super::snapshot::draft_snapshot(workspace, "new.txt", "Binary files differ\n").unwrap();
    assert!(!quoted.binary);
    std::fs::write(repo.path().join("new.txt"), "Binary files differ\n").unwrap();
    assert!(
        !super::snapshot::diff_snapshot(workspace, Some("new.txt"))
            .unwrap()
            .binary
    );
    std::fs::write(repo.path().join("new.txt"), "rename me\n").unwrap();
    assert_eq!(
        std::fs::read_to_string(repo.path().join("new.txt")).unwrap(),
        "rename me\n"
    );
    assert_eq!(
        super::file_snapshot(workspace.checkout, "gone")
            .unwrap()
            .state,
        super::TaskFileState::Missing
    );
    for (bytes, state) in [
        (vec![0], super::TaskFileState::Binary),
        (vec![255], super::TaskFileState::UnsupportedEncoding),
        (vec![b'a'; 1_000_001], super::TaskFileState::Truncated),
    ] {
        std::fs::write(repo.path().join("edge"), bytes).unwrap();
        let file = super::file_snapshot(workspace.checkout, "edge").unwrap();
        assert_eq!(file.state, state);
        assert!(file.content.is_none());
        assert!(file.revision.is_none());
    }
    assert!(super::file_snapshot(workspace.checkout, "../outside").is_err());
    #[cfg(unix)]
    {
        let before = super::file_snapshot(workspace.checkout, "new.txt").unwrap();
        assert!(before.read_only_reason.is_none());
        std::fs::rename(repo.path().join("new.txt"), repo.path().join("target.txt")).unwrap();
        std::os::unix::fs::symlink("target.txt", repo.path().join("new.txt")).unwrap();
        let after = super::file_snapshot(workspace.checkout, "new.txt").unwrap();
        assert_eq!(before.revision, after.revision);
        assert_eq!(before.content, after.content);
        assert!(after.read_only_reason.is_some());
        std::os::unix::fs::symlink("nested", repo.path().join("scratch/alias")).unwrap();
        let nested = super::file_snapshot(workspace.checkout, "scratch/alias/notes.md").unwrap();
        assert_eq!(nested.content, file.content);
        assert!(nested.read_only_reason.is_some());
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("private"), "outside").unwrap();
        std::os::unix::fs::symlink(outside.path(), repo.path().join("scratch/link")).unwrap();
        assert_eq!(
            super::snapshot::scratch_paths(repo.path()).unwrap().0,
            ["scratch/nested/notes.md"]
        );
        assert!(super::file_snapshot(workspace.checkout, "scratch/link/private").is_err());
    }
}

#[tokio::test]
async fn task_workspace_context_covers_committed_staged_unstaged_and_untracked_bytes() {
    let repository = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let output = std::process::Command::new("git")
            .current_dir(repository.path())
            .args(args)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout).unwrap().trim().to_string()
    };
    git(&["init", "-q", "-b", "main"]);
    std::fs::write(repository.path().join("tracked.txt"), "base\n").unwrap();
    git(&["add", "tracked.txt"]);
    git(&[
        "-c",
        "user.email=test@loopflow.dev",
        "-c",
        "user.name=Loopflow Test",
        "commit",
        "-q",
        "-m",
        "base",
    ]);
    let base = git(&["rev-parse", "HEAD"]);
    let TaskFixture { mut task, .. } = task_fixture_at(
        "TEST-WORKSPACE",
        std::fs::canonicalize(repository.path()).unwrap(),
    )
    .await;
    task.worktree = Some(std::fs::canonicalize(repository.path()).unwrap());
    task.base_commit = base;

    std::fs::write(repository.path().join("tracked.txt"), "committed\n").unwrap();
    git(&["add", "tracked.txt"]);
    git(&[
        "-c",
        "user.email=test@loopflow.dev",
        "-c",
        "user.name=Loopflow Test",
        "commit",
        "-q",
        "-m",
        "committed",
    ]);
    std::fs::write(repository.path().join("staged.txt"), "staged bytes\n").unwrap();
    git(&["add", "staged.txt"]);
    std::fs::write(repository.path().join("tracked.txt"), "unstaged bytes\n").unwrap();
    std::fs::write(repository.path().join("untracked.txt"), "untracked bytes\n").unwrap();

    let context = super::task_workspace_context(&task).unwrap();

    for expected in [
        "\"tracked.txt\": committed, unstaged",
        "\"staged.txt\": staged",
        "\"untracked.txt\": untracked",
        "unstaged bytes",
        "staged bytes",
        "untracked bytes",
    ] {
        assert!(context.contains(expected), "missing {expected:?}");
    }

    // A large branch keeps every path and state without filling the goal
    // budget with per-file hashes, nulls and repeated JSON field names.
    for index in 0..300 {
        std::fs::write(repository.path().join(format!("file-{index:03}.bin")), [0]).unwrap();
    }
    let unusual = "quoted\"\npath.bin";
    std::fs::write(repository.path().join(unusual), [0]).unwrap();
    let context = super::task_workspace_context(&task).unwrap();
    for index in 0..300 {
        assert!(context.contains(&format!("\"file-{index:03}.bin\": untracked")));
    }
    assert!(context.contains(&format!(
        "{}: untracked",
        serde_json::to_string(unusual).unwrap()
    )));
    assert!(context.contains("Patch omitted"));
    assert!(!context.contains("content_sha256"));
    assert!(crate::prompt::count_tokens(&context) < 5_000);
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // the guard serializes process-wide capture environment
async fn parent_run_cannot_override_task_worktree_resolution() {
    let _lock = crate::journal::test_env_lock();
    let _environment = EnvRestore::capture(&[crate::session_record::CAPTURE_KEY_ENV, "LF_RUN_DIR"]);
    let repository = loopflow_test_support::TestRepo::new();
    repository.create_branch("test/task-recovery-fixture");
    repository.push_new_branch("test/task-recovery-fixture");
    let TaskFixture { store, task, .. } =
        task_fixture_at("TEST-PARENT", repository.path().to_path_buf()).await;
    let parent_run_id = crate::session_record::new_artifact_key();
    std::env::set_var(
        crate::session_record::CAPTURE_KEY_ENV,
        parent_run_id.as_str(),
    );
    std::env::remove_var("LF_RUN_DIR");

    let resolved = super::task_for_checkout(&store, task.worktree.as_ref().unwrap())
        .await
        .unwrap()
        .unwrap();

    assert_eq!(resolved.id, task.id);
}

#[tokio::test]
async fn checkout_resolution_respects_machine_identity() {
    let repo = loopflow_test_support::TestRepo::new();
    let fixture = task_fixture_at("LOCATION-1", repo.path().canonicalize().unwrap()).await;
    let peer = crate::durable::MachineId::new();
    fixture
        .store
        .add_machine(&peer, "peer", "peer")
        .await
        .unwrap();
    let conn = rusqlite::Connection::open(&fixture.database_path).unwrap();
    conn.execute(
        "UPDATE tasks SET checkout_machine_id=?2 WHERE id=?1",
        rusqlite::params![fixture.task.id.as_str(), peer.as_str()],
    )
    .unwrap();
    assert!(super::task_for_checkout(&fixture.store, repo.path())
        .await
        .unwrap()
        .is_none());
    conn.execute(
        "UPDATE tasks SET checkout_machine_id=NULL WHERE id=?1",
        [fixture.task.id.as_str()],
    )
    .unwrap();
    assert!(super::task_for_checkout(&fixture.store, repo.path())
        .await
        .unwrap_err()
        .to_string()
        .contains("Machine is unknown"));
}

#[tokio::test(start_paused = true)]
async fn waited_landing_survives_pending_and_repair_then_completes_requested_task() {
    let ledger = crate::journal::TestLedgerGuard::new();
    let restore = EnvRestore::capture(&["PATH"]);
    let repository = loopflow_test_support::TestRepo::new();
    let fixture = task_fixture_at("LOO-503", repository.path().canonicalize().unwrap()).await;
    ledger.set_db_path(fixture.database_path.clone());
    let store = &fixture.store;
    let task = &fixture.task;
    repository.create_branch(&task.branch);
    assert!(std::process::Command::new("git")
        .args([
            "config",
            &format!("url.{}.insteadOf", repository.bare_path().display()),
            "https://github.com/loopflowstudio/loopflow.git",
        ])
        .current_dir(repository.path())
        .status()
        .unwrap()
        .success());
    assert!(std::process::Command::new("git")
        .args([
            "remote",
            "set-url",
            "origin",
            "https://github.com/loopflowstudio/loopflow.git"
        ])
        .current_dir(repository.path())
        .status()
        .unwrap()
        .success());

    // Task settlement independently reads GitHub. No test writes a merged
    // PR or completed Task into the database.
    let github = ledger.home().join("github.json");
    std::fs::write(&github, "{}").unwrap();
    let gh = ledger.home().join("gh");
    std::fs::write(
        &gh,
        format!(
            "#!/bin/sh\nif [ \"$1\" = --version ]; then exit 0; fi\ncat '{}'\n",
            github.display()
        ),
    )
    .unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var(
        "PATH",
        format!(
            "{}:{}",
            ledger.home().display(),
            std::env::var("PATH").unwrap()
        ),
    );

    let now = time::OffsetDateTime::now_utc();
    let mut pr = store.active_task_pr(&task.id).await.unwrap().unwrap();
    pr.publication = Some(PrPublication {
        requested_at: now,
        presentation: Some(PrPresentation {
            title: "Wait through repair".into(),
            body: "Complete only after verified delivery.".into(),
            head_sha: "original".into(),
        }),
        github: Some(GithubPr {
            number: 503,
            url: "https://github.com/loopflowstudio/loopflow/pull/503".into(),
            head_sha: Some("original".into()),
        }),
        merge: Some(PrMergeRequest {
            mode: PrMergeMode::Auto,
            requested_at: now,
            head_sha: "original".into(),
        }),
    });
    store.update_task_pr(&pr).await.unwrap();
    store
        .sqlite
        .request_task_completion(&task.id, Some("Accepted delivery"))
        .unwrap();
    let landing = PrLanding::new(
        NewPrLanding {
            repo: "loopflowstudio/loopflow".into(),
            pr_number: 503,
            worktree: task.worktree.clone().unwrap(),
            branch: task.branch.clone(),
            task_id: Some(task.id.clone()),
            requested_head_sha: "original".into(),
        },
        now,
    )
    .unwrap();
    let landing = store.start_or_join_pr_landing(&landing).await.unwrap();

    let phase = Arc::new(AtomicUsize::new(0));
    let (observed, mut observations) = tokio::sync::mpsc::unbounded_channel();
    let observe: crate::ops::pr_landing::ObservePr = {
        let phase = phase.clone();
        Arc::new(move |_| {
            let phase = phase.load(Ordering::SeqCst);
            observed.send(phase).unwrap();
            Ok(match phase {
                0 => LandingObservation::Pending {
                    head_sha: "original".into(),
                },
                1 => LandingObservation::Failing {
                    head_sha: "original".into(),
                    failing_checks: vec![crate::work::task::CiCheck {
                        name: "rust".into(),
                        url: None,
                    }],
                },
                2 => LandingObservation::Pending {
                    head_sha: "repaired".into(),
                },
                _ => LandingObservation::Merged {
                    head_sha: "repaired".into(),
                    merge_commit: "merge".into(),
                },
            })
        })
    };
    let repairs = Arc::new(AtomicUsize::new(0));
    let (started, running) = tokio::sync::oneshot::channel();
    let (release, held) = std::sync::mpsc::channel();
    let repair: crate::ops::pr_landing::RepairCi = {
        let started = Mutex::new(Some(started));
        let held = Mutex::new(held);
        let repairs = repairs.clone();
        let store = store.clone();
        let phase = phase.clone();
        Arc::new(move |landing, _, _| {
            repairs.fetch_add(1, Ordering::SeqCst);
            started.lock().unwrap().take().unwrap().send(()).unwrap();
            held.lock().unwrap().recv().map_err(super::task_error)?;
            // Model the repair provider's publication through the same
            // persisted head/request boundary used by arm.
            let mut pr = store
                .sqlite
                .active_task_pr(landing.task_id.as_ref().unwrap())
                .unwrap()
                .unwrap();
            let publication = pr.publication.as_mut().unwrap();
            publication.presentation.as_mut().unwrap().head_sha = "repaired".into();
            publication.github.as_mut().unwrap().head_sha = Some("repaired".into());
            publication.merge.as_mut().unwrap().head_sha = "repaired".into();
            store.sqlite.update_task_pr(&pr).unwrap();
            let mut requested = landing.clone();
            requested.requested_head_sha = "repaired".into();
            store.sqlite.start_or_join_pr_landing(&requested).unwrap();
            phase.store(2, Ordering::SeqCst);
            Ok(())
        })
    };
    let mut waiter = tokio::spawn({
        let store = store.clone();
        let id = landing.id.clone();
        let observe = observe.clone();
        let repair = repair.clone();
        async move { wait_with_io(&store, &id, LandingIo::scripted(observe, Some(repair))).await }
    });

    assert_eq!(
        tokio::select! {
            observation = observations.recv() => observation,
            result = &mut waiter => panic!("waiter stopped before pending observation: {result:?}"),
        },
        Some(0)
    );
    tokio::time::advance(Duration::from_secs(24 * 60 * 60)).await;
    assert_eq!(
        tokio::select! {
            observation = observations.recv() => observation,
            result = &mut waiter => panic!("waiter stopped during pending CI: {result:?}"),
        },
        Some(0)
    );
    assert!(!waiter.is_finished());
    assert_eq!(repairs.load(Ordering::SeqCst), 0);
    assert!(store
        .ci_incidents_since(time::OffsetDateTime::UNIX_EPOCH, None, None)
        .await
        .unwrap()
        .is_empty());
    assert_ne!(store.task_state(&task.id).await.unwrap(), TaskState::Done);

    phase.store(1, Ordering::SeqCst);
    tokio::select! {
        result = running => result.unwrap(),
        result = &mut waiter => panic!("waiter stopped before repair: {result:?}"),
    }
    tokio::time::advance(Duration::from_secs(24 * 60 * 60)).await;
    assert!(!waiter.is_finished());
    let competing = reconcile_pr_landing(
        store.clone(),
        landing.clone(),
        LandingIo::scripted(observe, Some(repair)),
    )
    .await
    .unwrap();
    assert_eq!(
        competing, landing,
        "the competing observer cannot claim the landing"
    );
    assert_eq!(
        store
            .get_pr_landing(&landing.id)
            .await
            .unwrap()
            .unwrap()
            .state,
        PrLandingState::Repairing
    );
    assert_eq!(repairs.load(Ordering::SeqCst), 1);
    release.send(()).unwrap();
    loop {
        let observation = tokio::select! {
            observation = observations.recv() => observation,
            result = &mut waiter => panic!("waiter stopped before observing publication: {result:?}"),
        };
        match observation {
            Some(2) => break,
            Some(_) => {}
            None => panic!("waiter stopped before observing the repaired head"),
        }
    }
    assert!(!waiter.is_finished());
    assert_ne!(store.task_state(&task.id).await.unwrap(), TaskState::Done);
    let pr = store.active_task_pr(&task.id).await.unwrap().unwrap();
    assert_eq!(pr.head_sha(), Some("repaired"));
    assert_eq!(pr.merge_request().unwrap().head_sha, "repaired");

    std::fs::write(&github, r#"{"merged":true,"state":"closed","draft":false,"merge_commit_sha":"merge","merged_at":"2026-10-17T00:00:00Z","number":503,"html_url":"https://github.com/loopflowstudio/loopflow/pull/503","head":{"sha":"repaired"}}"#).unwrap();
    phase.store(3, Ordering::SeqCst);
    waiter.await.unwrap().unwrap();
    assert_eq!(store.task_state(&task.id).await.unwrap(), TaskState::Done);
    let saved = store.get_pr_landing(&landing.id).await.unwrap().unwrap();
    assert_eq!(saved.state, PrLandingState::Merged);
    assert_eq!(saved.requested_head_sha, "repaired");
    assert_eq!(saved.observed_head_sha, "repaired");
    assert_eq!(saved.merge_commit.as_deref(), Some("merge"));
    let incidents = store
        .ci_incidents_since(time::OffsetDateTime::UNIX_EPOCH, None, None)
        .await
        .unwrap();
    assert_eq!(incidents.len(), 1);
    assert_eq!(incidents[0].incident.failure_set, ["rust"]);
    assert!(incidents[0].incident.merged_at.is_some());
    assert_eq!(repairs.load(Ordering::SeqCst), 1);
    drop(restore);
}

#[tokio::test]
async fn watched_landing_completes_task_only_from_merged_pr_evidence() {
    let repository = loopflow_test_support::TestRepo::new();
    let TaskFixture {
        _database,
        store,
        mut task,
        work,
        database_path,
        ..
    } = task_fixture_at("LOO-248", repository.path().canonicalize().unwrap()).await;
    let now = time::OffsetDateTime::now_utc();
    let mut pr = store.active_task_pr(&task.id).await.unwrap().unwrap();
    pr.branch = "HEAD".to_string();
    rusqlite::Connection::open(database_path)
        .unwrap()
        .execute(
            "UPDATE tasks SET branch=?2 WHERE id=?1",
            rusqlite::params![task.id.as_str(), pr.branch],
        )
        .unwrap();
    let landed_head = crate::git::rev_parse(task.worktree.as_ref().unwrap(), "HEAD").unwrap();
    pr.publication = Some(PrPublication {
        requested_at: now,
        presentation: Some(PrPresentation {
            title: "Watch the landing".to_string(),
            body: "Finish only after GitHub confirms merge.".to_string(),
            head_sha: landed_head.clone(),
        }),
        github: Some(GithubPr {
            number: 248,
            url: "https://github.com/loopflowstudio/loopflow/pull/248".to_string(),
            head_sha: Some(landed_head.clone()),
        }),
        merge: Some(PrMergeRequest {
            mode: PrMergeMode::Auto,
            requested_at: now,
            head_sha: landed_head.clone(),
        }),
    });
    store.update_task_pr(&pr).await.unwrap();
    let landing = crate::pr_landing::PrLanding::new(
        crate::pr_landing::NewPrLanding {
            repo: "loopflowstudio/loopflow".to_string(),
            pr_number: 248,
            worktree: task.worktree.as_ref().unwrap().clone(),
            branch: pr.branch.clone(),
            task_id: Some(task.id.clone()),
            requested_head_sha: landed_head.clone(),
        },
        now,
    )
    .unwrap();

    assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Ready);
    let error = apply_merged_task_landing(&store, &mut task, &pr, &landing)
        .await
        .expect_err("an armed but unmerged PR cannot complete the Task");
    assert!(error.to_string().contains("did not confirm"));
    assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Ready);

    pr.merge_commit = Some(landed_head);
    store.settle_task_pr(&pr).await.unwrap();
    let follow_up = crate::work::task::TaskFollowUp {
        outcome: "Installed latency meets budget".into(),
        evidence: "20 samples below 1s p95".into(),
        check_at: now.unix_timestamp() - 1,
    };
    store
        .append_task_event(
            &task.id,
            &TaskEventKind::FollowUp {
                remaining: Some(follow_up.clone()),
                reason: "Accepted production check".into(),
            },
        )
        .await
        .unwrap();
    for _ in 0..2 {
        apply_merged_task_landing(&store, &mut task, &pr, &landing)
            .await
            .unwrap();
        assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Ready);
        assert!(super::task_completion_refusal(&store, &task)
            .await
            .unwrap()
            .is_none());
        assert!(store.task_events_after(&task.id, 0).await.unwrap().iter().any(|event|
                matches!(&event.kind, TaskEventKind::FollowUp { remaining: Some(saved), .. } if saved == &follow_up)
            ));
    }
    apply_merged_task_landing(&store, &mut task, &pr, &landing)
        .await
        .unwrap();
    assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Ready);
    store
        .sqlite
        .request_task_completion(&task.id, Some("Accepted outcome complete"))
        .unwrap();
    apply_merged_task_landing(&store, &mut task, &pr, &landing)
        .await
        .unwrap();
    assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Done);
    apply_merged_task_landing(&store, &mut task, &pr, &landing)
        .await
        .unwrap();
    assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Done);

    let current = store.active_task_pr(&task.id).await.unwrap().unwrap();
    let task = store.get_task(&task.id).await.unwrap().unwrap();
    let mut next = super::delivery::task_pr_for_publication(&task);
    next.sequence = current.sequence + 1;
    next.branch = "wrong-placement".into();
    assert!(store.insert_task_pr(&next).await.is_err());
    assert_eq!(store.active_task_pr(&task.id).await.unwrap(), Some(current));
    assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Done);

    next.branch = task.branch.clone();
    store.insert_task_pr(&next).await.unwrap();
    // A new PR respects completion; only an explicit reopen uncompletes.
    assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Done);
    store
        .sqlite
        .reopen_task(&task.id, Some("New delivery"))
        .unwrap();
    store
        .sqlite
        .request_task_completion(&task.id, Some("New delivery"))
        .unwrap();
    super::settle_task_landing(&store, &landing).await.unwrap();
    assert_eq!(store.work_status(&work).await.unwrap(), WorkStatus::Ready);
    assert_eq!(
        store.active_task_pr(&task.id).await.unwrap().unwrap().id,
        next.id
    );
    assert!(store
        .sqlite
        .task_completion_pending(&task.id)
        .unwrap()
        .is_some());
}

#[test]
fn deleted_task_status_requires_explicit_history_lookup() {
    const CHILD: &str = "LOOPFLOW_DELETED_STATUS_CHILD";
    if std::env::var_os(CHILD).is_some() {
        let path =
            std::path::PathBuf::from(std::env::var_os("LF_HOME").unwrap()).join("loopflow.db");
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let store = std::sync::Arc::new(
            runtime
                .block_on(crate::store::open_ephemeral_store(&StorageConfig::sqlite(
                    path.clone(),
                )))
                .unwrap(),
        );
        // Install fault injection after schema validation opens the fixture.
        let connection = rusqlite::Connection::open(&path).unwrap();
        connection.execute_batch(
            "CREATE TRIGGER no_history_task_update BEFORE UPDATE ON tasks BEGIN SELECT RAISE(FAIL,'historical Task must remain read-only'); END;
             CREATE TRIGGER no_history_pr_update BEFORE UPDATE ON task_prs BEGIN SELECT RAISE(FAIL,'historical PR must remain read-only'); END;
             CREATE TRIGGER no_history_event BEFORE INSERT ON task_events BEGIN SELECT RAISE(FAIL,'historical Task must not gain events'); END;"
        ).unwrap();
        crate::ops::linear::LINEAR_TEST_CONTEXT.sync_scope(
            crate::ops::linear::LinearTestContext {
                path,
                store,
                graphql_url: "http://127.0.0.1:1".into(),
            },
            || {
                let repo = std::env::current_dir().unwrap();
                let error = super::task_status(&repo, None).unwrap_err();
                assert!(error.to_string().contains("Task was deleted"), "{error}");
                let status = super::task_status(&repo, Some("HISTORY-1")).unwrap();
                let snapshot = status.execution.as_ref().unwrap();
                assert_eq!(
                    super::task_status(&repo, Some(&snapshot.task_id)).unwrap(),
                    status
                );
                assert_eq!(
                    super::task_status(&repo, Some(&snapshot.task_id)).unwrap(),
                    status
                );
                assert_eq!(snapshot.status, TaskState::Done);
                assert_eq!(
                    snapshot.actions.recommended,
                    Some(crate::ops::task_actions::TaskAction::NoAction)
                );
            },
        );
        return;
    }

    let repo = loopflow_test_support::TestRepo::new();
    repo.create_branch("test/task-recovery-fixture");
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let fixture = runtime.block_on(task_fixture_at("HISTORY-1", repo.path().to_path_buf()));
    let connection = rusqlite::Connection::open(&fixture.database_path).unwrap();
    // Seed a completed historical outcome and positive removal evidence.
    // This is a reader proof, not a provider-deletion fixture.
    connection
        .execute(
            "UPDATE tasks SET planning_completed=1,planning_state='completed' WHERE id=?1",
            [fixture.task.id.as_str()],
        )
        .unwrap();
    connection.execute(
            "INSERT INTO task_deletions(wave_id,issue_id,identifier,confirmed_at) VALUES (?1,?2,?3,456)",
            rusqlite::params![fixture.task.wave_id.as_ref().unwrap().as_str(), fixture.task.linear_id.as_ref().unwrap().as_str(), fixture.task.identifier],
        ).unwrap();
    let pr = runtime
        .block_on(fixture.store.active_task_pr(&fixture.task.id))
        .unwrap();
    let events = runtime
        .block_on(fixture.store.task_events_after(&fixture.task.id, 0))
        .unwrap();
    let output = std::process::Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "ops::task::tests::deleted_task_status_requires_explicit_history_lookup",
            "--test-threads=1",
        ])
        .env_clear()
        .envs(std::env::vars().filter(|(name, _)| !name.starts_with("LF_")))
        .env(CHILD, "1")
        .env("LF_HOME", fixture._database.path())
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        runtime
            .block_on(fixture.store.active_task_pr(&fixture.task.id))
            .unwrap(),
        pr
    );
    assert_eq!(
        runtime
            .block_on(fixture.store.task_events_after(&fixture.task.id, 0))
            .unwrap(),
        events
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT planning_completed FROM tasks WHERE id=?1",
                [fixture.task.id.as_str()],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        1
    );
}

#[test]
fn piped_report_supplies_title_and_preserves_full_description() {
    let report = "\n  lf wave status rejects stored timestamp  \n\nstack trace\nmore evidence\n";
    let input = resolve_task_create_input(None, Some(report)).expect("resolve piped report");

    assert_eq!(input.title, "lf wave status rejects stored timestamp");
    assert_eq!(
        input.report,
        "lf wave status rejects stored timestamp  \n\nstack trace\nmore evidence"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)] // Serializes identity configuration used during publication.
async fn steering_publication_resolves_empty_participant_from_config_or_git() {
    let _lock = crate::journal::test_env_lock();
    let _restore = EnvRestore::capture(&[
        "LF_HOME",
        "LF_USER_NAME",
        "GIT_CONFIG_COUNT",
        "GIT_CONFIG_KEY_0",
        "GIT_CONFIG_VALUE_0",
    ]);
    let repo = loopflow_test_support::TestRepo::new();
    let fixture = task_fixture_at("FIX-STEER", repo.path().canonicalize().unwrap()).await;
    std::env::set_var("LF_HOME", fixture._database.path());
    std::env::set_var("GIT_CONFIG_COUNT", "1");
    std::env::set_var("GIT_CONFIG_KEY_0", "user.name");
    std::env::set_var("GIT_CONFIG_VALUE_0", "Git Person");
    for (configured, participant, expected) in [
        (Some("Configured Person"), "", "Configured Person"),
        (None, "  ", "Git Person"),
        (Some("Configured Person"), "Caller", "Caller"),
    ] {
        std::env::set_var("LF_USER_NAME", participant);
        let config = configured
            .map(|name| format!("user:\n  name: {name}\n"))
            .unwrap_or_default();
        std::fs::write(fixture._database.path().join("config.yaml"), config).unwrap();
        let id =
            super::append_task_comment(&fixture.store, &fixture.task, "Keep going", true).unwrap();
        let comments = fixture
            .store
            .sqlite
            .task_comments(&fixture.task.id)
            .unwrap()
            .comments;
        let body = &comments
            .iter()
            .find(|comment| comment.id == id)
            .unwrap()
            .body;
        assert!(body.contains("Keep going"));
        assert!(body.contains("<!-- loopflow-steer:"));
        assert!(
            body.contains(&format!("<!-- loopflow-requester:\"{expected}\" -->")),
            "{body}"
        );
        assert_eq!(
            crate::ops::linear_observe::comment_requester(body, Some("Publisher")),
            Some(expected.to_string())
        );
    }
}

#[tokio::test]
async fn stacking_is_idempotent_and_preserves_history() {
    let _ledger = crate::journal::TestLedgerGuard::new();
    let repo = loopflow_test_support::TestRepo::new();
    let fixture = task_fixture_at("STACK-1", repo.path().canonicalize().unwrap()).await;
    let store = &fixture.store;
    let child = store
        .active_task_pr(&fixture.task.id)
        .await
        .unwrap()
        .unwrap();
    let mut parent_task = fixture.task.clone();
    parent_task.id = TaskId::new();
    parent_task.linear_id = Some(LinearIssueId::new("stack-parent-issue").unwrap());
    parent_task.identifier = "STACK-2".into();
    parent_task.worktree = Some(repo.path().join("parent"));
    parent_task.workspace_slug = "stack-parent".into();
    let mut parent = child.clone();
    parent.id = TaskPrId::new();
    parent.task_id = parent_task.id.clone();
    parent_task.branch = "test/stack-parent".into();
    parent.branch = parent_task.branch.clone();
    parent.slug = "stack-parent".into();
    store.seed_task(&parent_task, &parent).await.unwrap();
    parent.publication = Some(PrPublication {
        requested_at: parent.created_at,
        presentation: None,
        github: Some(GithubPr {
            number: 42,
            url: "https://github.com/fixture/repo/pull/42".into(),
            head_sha: None,
        }),
        merge: None,
    });
    store.update_task_pr(&parent).await.unwrap();
    let events = store.task_events_after(&fixture.task.id, 0).await.unwrap();
    store
        .stack_task_placement(&fixture.task, &parent.id)
        .await
        .unwrap();
    store
        .stack_task_placement(&fixture.task, &parent.id)
        .await
        .unwrap();
    assert_eq!(
        store.task_events_after(&fixture.task.id, 0).await.unwrap(),
        events
    );
    // Both self-parenting and a reverse edge preserve the accepted dependency.
    assert!(store
        .stack_task_placement(&parent_task, &parent.id)
        .await
        .is_err());
    let mut published_child = store
        .active_task_pr(&fixture.task.id)
        .await
        .unwrap()
        .unwrap();
    published_child.publication = parent.publication.clone();
    store.update_task_pr(&published_child).await.unwrap();
    assert!(store
        .stack_task_placement(&parent_task, &child.id)
        .await
        .unwrap_err()
        .to_string()
        .contains("cycle"));
    assert_eq!(
        store.get_task_pr(&parent.id).await.unwrap(),
        Some(parent.clone())
    );
    assert_eq!(
        store.get_task_pr(&child.id).await.unwrap(),
        Some(published_child)
    );

    let current = store.get_task(&fixture.task.id).await.unwrap().unwrap();
    let mut next_task = parent_task.clone();
    next_task.id = TaskId::new();
    next_task.identifier = "STACK-3".into();
    next_task.linear_id = None;
    next_task.branch = "test/next-parent".into();
    next_task.workspace_slug = "next-parent".into();
    next_task.worktree = Some(repo.path().join("next-parent"));
    let mut next = store.get_task_pr(&parent.id).await.unwrap().unwrap();
    next.id = TaskPrId::new();
    next.task_id = next_task.id.clone();
    next.branch = next_task.branch.clone();
    next.slug = next_task.workspace_slug.clone();
    store.seed_task(&next_task, &next).await.unwrap();
    let selected = super::select_existing_stack_parent(store, &current, "STACK-3")
        .await
        .unwrap();
    store
        .stack_task_placement(&current, &selected.id)
        .await
        .unwrap();
    let moved = store.get_task_pr(&child.id).await.unwrap().unwrap();
    assert_eq!(moved.parent_pr_id, Some(next.id));
    assert_eq!(moved.base_commit, child.base_commit);
    assert_eq!(moved.id, child.id);
    assert_eq!(
        store.task_events_after(&fixture.task.id, 0).await.unwrap(),
        events
    );
    assert!(store
        .stack_task_placement(&current, &parent.id)
        .await
        .is_err());
}

#[tokio::test]
async fn task_placement_accepts_shared_worktree_names() {
    let repo = loopflow_test_support::TestRepo::new();
    let fixture = task_fixture_at("FIX-2", repo.path().to_path_buf()).await;
    for (name, expected) in [
        ("fix", "fix"),
        ("Fix Workspace", "fix-workspace"),
        ("one-two-three-four-five-six", "one-two-three-four-five-six"),
    ] {
        let (plan, slug) = super::plan_task_placement(
            repo.path(),
            &fixture.task,
            None,
            &super::TaskProcessOptions {
                name: Some(name.into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(slug, expected);
        assert!(plan.worktree_path.ends_with(format!(
            "{}.{}",
            repo.path().file_name().unwrap().to_str().unwrap(),
            expected
        )));
        assert!(!plan.worktree_path.exists());
    }
}

#[tokio::test]
async fn task_preparation_rejects_invalid_inputs_before_allocating_placement() {
    let repo = loopflow_test_support::TestRepo::new();
    let fixture = task_fixture_at("FIX-2", repo.path().to_path_buf()).await;
    for (options, message) in [
        (
            super::TaskProcessOptions {
                name: Some("bad.name".into()),
                ..Default::default()
            },
            "flat worktree name",
        ),
        (
            super::TaskProcessOptions {
                directive: Some("  ".into()),
                ..Default::default()
            },
            "directive cannot be empty",
        ),
    ] {
        let error = super::prepare_task_placement(
            &fixture.store,
            repo.path(),
            &fixture.task,
            None,
            &options,
        )
        .await
        .unwrap_err();
        assert!(error.to_string().contains(message), "{error}");
    }
    assert_eq!(
        crate::git::worktrees::list_worktrees(repo.path())
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn task_preparation_preserves_occupied_placement_and_resolves_base_without_creating() {
    let repo = loopflow_test_support::TestRepo::new();
    let fixture = task_fixture_at("FIX-2", repo.path().to_path_buf()).await;
    let options = super::TaskProcessOptions {
        name: Some("existing-task".into()),
        ..Default::default()
    };
    let planned =
        super::prepare_task_placement(&fixture.store, repo.path(), &fixture.task, None, &options)
            .await
            .unwrap();
    assert_eq!(planned.plan.base_ref, repo.head_sha());
    assert!(!planned.plan.worktree_path.exists());
    std::fs::create_dir_all(&planned.plan.worktree_path).unwrap();
    let authored = planned.plan.worktree_path.join("authored.txt");
    std::fs::write(&authored, "retain these bytes").unwrap();
    let result =
        super::prepare_task_placement(&fixture.store, repo.path(), &fixture.task, None, &options)
            .await;
    assert!(result.unwrap_err().to_string().contains("already exists"));
    assert_eq!(
        std::fs::read_to_string(&authored).unwrap(),
        "retain these bytes"
    );
    std::fs::remove_dir_all(planned.plan.worktree_path).unwrap();
}

#[tokio::test]
async fn task_checkout_pins_upstream_without_requiring_clean_canonical_main() {
    let repo = loopflow_test_support::TestRepo::new();
    let fixture = task_fixture_at("FIX-2", repo.path().to_path_buf()).await;
    let upstream = repo.head_sha();
    repo.create_file("local.txt", "unpublished main work");
    repo.stage_all();
    repo.commit("Local main work");
    repo.create_branch("local-feature");
    repo.create_file("dirty.txt", "uncommitted work");
    let prepared = super::prepare_task_placement(
        &fixture.store,
        repo.path(),
        &fixture.task,
        None,
        &super::TaskProcessOptions::default(),
    )
    .await
    .unwrap();
    assert_eq!(prepared.plan.base_ref, upstream);
    assert_eq!(
        crate::git::current_branch(repo.path()).unwrap().as_deref(),
        Some("local-feature")
    );
    assert_eq!(
        std::fs::read_to_string(repo.path().join("dirty.txt")).unwrap(),
        "uncommitted work"
    );
    assert!(!prepared.plan.worktree_path.exists());
}

#[tokio::test]
async fn task_preparation_keeps_the_resolved_base_when_the_remote_ref_advances() {
    let repo = loopflow_test_support::TestRepo::new();
    let fixture = task_fixture_at("FIX-2", repo.path().to_path_buf()).await;
    let prepared = super::prepare_task_placement(
        &fixture.store,
        repo.path(),
        &fixture.task,
        None,
        &super::TaskProcessOptions::default(),
    )
    .await
    .unwrap();
    let original = repo.head_sha();
    repo.create_file("later.txt", "arrived before placement committed");
    repo.stage_all();
    repo.commit("Advance the remote after preparation");
    repo.push();
    assert_ne!(
        crate::git::rev_parse(repo.path(), "origin/main").unwrap(),
        original
    );

    let placement =
        crate::git::worktrees::create_from_placement_plan(repo.path(), &prepared.plan).unwrap();
    let placed_head = crate::git::rev_parse(&placement.path, "HEAD").unwrap();
    let has_later_file = placement.path.join("later.txt").exists();
    std::fs::remove_dir_all(&placement.path).unwrap();

    assert_eq!(placed_head, original);
    assert_eq!(placed_head, prepared.plan.base_ref);
    assert!(!has_later_file);
}

#[tokio::test]
async fn task_preparation_refuses_an_unresolvable_base_before_allocating_placement() {
    let repo = loopflow_test_support::TestRepo::new();
    let fixture = task_fixture_at("FIX-2", repo.path().to_path_buf()).await;
    let output = std::process::Command::new("git")
        .current_dir(repo.path())
        .args([
            "remote",
            "set-url",
            "origin",
            "/nonexistent/loopflow-fixture-remote",
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    let error = super::prepare_task_placement(
        &fixture.store,
        repo.path(),
        &fixture.task,
        None,
        &super::TaskProcessOptions {
            name: Some("missing-base".into()),
            ..Default::default()
        },
    )
    .await
    .unwrap_err();
    assert!(
        error.to_string().contains("failed to fetch task base"),
        "{error}"
    );
    assert_eq!(
        crate::git::worktrees::list_worktrees(repo.path())
            .unwrap()
            .len(),
        1
    );
}

#[tokio::test]
async fn task_preparation_rejects_unpublished_parent_before_allocating_child() {
    let repo = loopflow_test_support::TestRepo::new();
    let parent = task_fixture_at("FIX-1", repo.path().canonicalize().unwrap()).await;
    for selector in ["FIX-1", parent.task.id.as_str(), "FIX-1-issue"] {
        let error = super::prepare_task_placement(
            &parent.store,
            repo.path(),
            &parent.task,
            None,
            &super::TaskProcessOptions {
                stack_on: Some(selector.into()),
                ..Default::default()
            },
        )
        .await
        .unwrap_err();
        assert!(
            error.to_string().contains("cannot stack on itself"),
            "{error}"
        );
    }
    let child = Task {
        id: TaskId::new(),
        ..parent.task.clone()
    };
    let result = super::prepare_task_placement(
        &parent.store,
        repo.path(),
        &child,
        None,
        &super::TaskProcessOptions {
            name: Some("child-task".into()),
            stack_on: Some("FIX-1".into()),
            ..Default::default()
        },
    )
    .await;
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("open the parent PR"));
    let retained = parent
        .store
        .get_task(&parent.task.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.worktree, parent.task.worktree);
    assert_eq!(parent.store.list_tasks(None).await.unwrap().len(), 1);
    assert_eq!(
        parent.store.task_prs(&parent.task.id).await.unwrap().len(),
        1
    );
}

#[test]
fn execution_boundary_resolves_linked_git_and_control_roots() {
    let _env_lock = crate::journal::test_env_lock();
    let _restore = EnvRestore::capture(&["LF_BIN", "LF_HOME"]);
    let directory = tempfile::tempdir().unwrap();
    let main = directory.path().join("repo");
    let worktree = directory.path().join("repo.task");
    std::fs::create_dir(&main).unwrap();
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "user.email", "test@example.com"],
        vec!["config", "user.name", "Loopflow Test"],
    ] {
        assert!(std::process::Command::new("git")
            .current_dir(&main)
            .args(args)
            .status()
            .unwrap()
            .success());
    }
    std::fs::write(main.join("README.md"), "proof\n").unwrap();
    for args in [vec!["add", "."], vec!["commit", "-m", "proof"]] {
        assert!(std::process::Command::new("git")
            .current_dir(&main)
            .args(args)
            .status()
            .unwrap()
            .success());
    }
    assert!(std::process::Command::new("git")
        .current_dir(&main)
        .args(["worktree", "add", "-b", "task", worktree.to_str().unwrap()])
        .status()
        .unwrap()
        .success());
    let control = directory.path().join("control");
    std::fs::create_dir(&control).unwrap();
    std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
    std::env::set_var("LF_HOME", &control);

    let boundary = checkout_execution_boundary(&worktree, "codex").unwrap();

    assert_eq!(boundary.writable_roots.len(), 2);
    assert!(boundary
        .writable_roots
        .contains(&main.join(".git").canonicalize().unwrap()));
    assert!(boundary.writable_roots.contains(&control));
    assert_eq!(
        checkout_execution_boundary(&worktree, "pi:fixture/model").unwrap(),
        boundary
    );
}

#[tokio::test]
async fn nonresumable_execution_blocker_remains_a_launch_refusal() {
    let TaskFixture { store, task, .. } = task_fixture("TEST-BOUNDARY").await;
    let blocker = "Task execution boundary is blocked: linked Git index.lock is not writable";
    store
        .append_task_event(
            &task.id,
            &TaskEventKind::Failed {
                error: blocker.to_string(),
                resumable: false,
            },
        )
        .await
        .unwrap();
    let settled = store.latest_task_event(&task.id).await.unwrap().unwrap();

    let event = store.latest_task_event(&task.id).await.unwrap().unwrap();
    assert_eq!(event, settled);
    assert_eq!(task_event_process_refusal(Some(&event)), Some(blocker));
    assert!(matches!(
        event.kind,
        TaskEventKind::Failed {
            resumable: false,
            ..
        }
    ));
}

#[test]
fn pr_mutation_lock_refuses_a_concurrent_writer() {
    let repo = tempfile::tempdir().expect("temporary repository");
    let status = std::process::Command::new("git")
        .args(["init", "--quiet"])
        .current_dir(repo.path())
        .status()
        .expect("initialize repository");
    assert!(status.success());

    let _first = lock_task_pr_mutation(repo.path()).expect("first mutation lock");
    let error = lock_task_pr_mutation(repo.path()).expect_err("second writer must be refused");
    assert!(error.to_string().contains("already running"));
}
