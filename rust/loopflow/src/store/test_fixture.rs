//! Prepared storage inputs, independent of application acquisition.

use std::path::{Path, PathBuf};

use loopflow_model::durable::{ProjectId, RepositoryId, TaskId};
use loopflow_model::id::{LfProcessId, TerminalId, WaveId};
use rusqlite::OptionalExtension;

use super::migration_runner::{MigrationBuild, MigrationObservers};
use super::sqlite::scope_defaults::TaskScope;
use super::sqlite::task_ownership::{OwnershipFacts, Person};
use super::sqlite::SqliteStore;
use super::{ProcessIdentityEvidence, StoreResult};
use crate::store::linear_records::{LinearIssueId, LinearProjectId};
use crate::store::session::{LfSession, TitleSource, WorkSource};
use crate::store::steer::SteerAdapter;
use crate::store::work::project::Project;
use crate::store::work::task::{LinearWritebackState, Task, TaskPr, TaskPrId};

pub(crate) fn build() -> MigrationBuild<'static> {
    MigrationBuild {
        provenance: "development",
        source_identity: "storage-fixture".into(),
        source_revision: "storage-fixture",
        version: "storage-fixture",
        package_version: env!("CARGO_PKG_VERSION"),
    }
}

pub(crate) fn observers() -> MigrationObservers<'static> {
    MigrationObservers {
        export_contract: &|_, _| Ok(()),
        process_evidence: &|_, _| ProcessIdentityEvidence::Unknown,
    }
}

pub(crate) fn key() -> Result<[u8; 32], super::token_crypto::TokenCryptoError> {
    Ok([7; 32])
}

pub(crate) fn open(path: &Path) -> StoreResult<SqliteStore> {
    SqliteStore::open_ephemeral_prepared(path, &build(), &observers(), key)
}

pub(crate) fn repository(store: &SqliteStore, path: &str) -> RepositoryId {
    let mut conn = store.conn.lock().unwrap();
    let snapshot = conn.savepoint().unwrap();
    if let Some(id) = snapshot
        .query_row(
            "SELECT id FROM repository_plans WHERE repo=?1 AND selected=1",
            [path],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .unwrap()
    {
        return RepositoryId::from_raw(id);
    }
    let id = RepositoryId::new();
    snapshot
        .execute(
            "INSERT INTO repository_plans(id,repo,selected) VALUES(?1,?2,1)",
            rusqlite::params![id.as_str(), path],
        )
        .unwrap();
    snapshot.commit().unwrap();
    id
}

pub(crate) fn ownership(store: &SqliteStore, scope: TaskScope) -> OwnershipFacts {
    let local: String = store
        .conn
        .lock()
        .unwrap()
        .query_row("SELECT id FROM machines WHERE is_local=1", [], |row| {
            row.get(0)
        })
        .unwrap();
    OwnershipFacts {
        person: Person {
            name: Some("Fixture".into()),
            key: Some("fixture@example.com".into()),
            id: scope.placement.person_id.clone(),
        },
        scope,
        planning_connected: false,
        declared_machines: std::collections::HashMap::new(),
        local_label: "fixture".into(),
        owning_machine: loopflow_model::durable::MachineId::from_raw(local),
    }
}

pub(crate) fn steer() -> (tempfile::TempDir, SqliteStore, SteerAdapter) {
    let home = tempfile::tempdir().unwrap();
    let store = crate::store::test_fixture::open(&home.path().join("store.db")).unwrap();
    store.test_session("session", &crate::store::session_record::new_artifact_key());
    let process = LfProcessId::new();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
            [process.as_str()],
        )
        .unwrap();
    let claim = store
        .attach_session(
            "session",
            None,
            &process,
            false,
            None,
            &crate::store::sqlite::SessionCaller {
                agent: None,
                capture_key: None,
            },
        )
        .unwrap();
    store
        .record_agent_process_identity("session", &claim, std::process::id(), 1)
        .unwrap();
    let adapter = store.reserve_steer_adapter("session", &claim).unwrap();
    store
        .with_provider_context(
            &adapter,
            "native",
            crate::store::steer::NativeBoundary::Activate { source: "startup" },
            || Ok(()),
        )
        .unwrap();
    (home, store, adapter)
}

/// Host the fixture's Session in a held terminal.
pub(crate) fn hold(store: &SqliteStore) -> TerminalId {
    let terminal = TerminalId::new();
    store.create_terminal(&terminal, "build").unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "UPDATE agent_sessions SET interactive=1,held_terminal_id=?1 WHERE id='session'",
            [&terminal],
        )
        .unwrap();
    terminal
}

pub(crate) fn native_steer() -> (tempfile::TempDir, SqliteStore, SteerAdapter, TerminalId) {
    let (home, store, adapter) = steer();
    let terminal = hold(&store);
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "UPDATE processes SET agent_interactive=1 WHERE id=?1",
            [&adapter.agent_process_id],
        )
        .unwrap();
    }
    (home, store, adapter, terminal)
}
pub(crate) fn unpublished_conversation(
    task_id: Option<TaskId>,
    wave_id: Option<WaveId>,
    created_at: i64,
) -> LfSession {
    LfSession {
        captured: None,
        id: uuid::Uuid::new_v4().to_string().into(),
        artifact_key: Some(crate::store::session_record::new_artifact_key()),
        caller_artifact_key: None,
        input_published: false,
        cwd: "/repo".into(),
        skill: None,
        provider: None,
        model: None,
        node: None,
        iterations: None,
        task_id,
        wave_id,
        graph_lf_process_id: None,
        work_source: Some(WorkSource::Declared),
        bound_at: None,
        interactive: false,
        repo: None,
        title: "Investigation".into(),
        title_source: TitleSource::Generated,
        request: None,
        ready_summary: None,
        completed_at: None,
        created_at,
    }
}

pub(crate) fn task() -> (tempfile::TempDir, SqliteStore, Task, TaskPr) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loopflow.db");
    let store = crate::store::test_fixture::open(&path).expect("open a fresh store");
    let wave_id = WaveId::new();
    let project_id = ProjectId::new();
    let task_id = TaskId::new();
    let now = time::OffsetDateTime::now_utc();
    let conn = rusqlite::Connection::open(&path).unwrap();
    conn.execute(
        "INSERT INTO waves (id, name, repo, created_at, parent_wave_id)
             VALUES (?1, 'probe', '/repo', 1700000000, NULL)",
        [wave_id.as_str()],
    )
    .unwrap();
    drop(conn);
    let project = Project {
        id: project_id.clone(),

        summary: String::new(),
        status: crate::store::work::project::ProjectStatus::Started,
        linear_id: Some(LinearProjectId::new("999bdbdd-c045-41a6-8ffc-a97c4a40b0b3").unwrap()),
        slug: "probe".to_string(),
        name: "Probe".to_string(),
        description: "Probe Task execution".to_string(),
        krs: Vec::new(),
        metric_targets: Vec::new(),
        pm_snapshot_synced_at: Some(now.unix_timestamp()),

        wave_id: wave_id.clone(),
        created_at: now,
        updated_at: now,
    };
    store.insert_project(&project).unwrap();
    crate::store::sqlite::project_selection::write_project_binding(
        &store,
        &wave_id,
        None,
        project.linear_id.as_ref().unwrap().as_str(),
        &crate::store::PlanningLocks::new(tempfile::tempfile().unwrap()),
    )
    .unwrap();
    let task = Task {
        repository_id: crate::store::test_fixture::repository(&store, "/repo"),
        id: task_id.clone(),

        revision: 0,
        linear_id: Some(LinearIssueId::new("task-uuid").unwrap()),
        identifier: "PROBE-1".to_string(),
        title: "Probe Task execution".to_string(),
        description: "Exercise the Task Graph store".to_string(),
        pm_snapshot_synced_at: Some(now.unix_timestamp()),

        pm_writeback: LinearWritebackState::Current,
        wave_id: Some(wave_id),
        project_id: Some(project_id),
        worktree: Some(PathBuf::from("/repo.probe")),
        workspace_slug: "probe".to_string(),
        branch: "probe".to_string(),
        base_commit: "deadbeef".to_string(),
        parent_pr_id: None,
        created_at: now,
        updated_at: now,
        rank: 0,
        assignee: None,
        due_date: None,
        state: None,
        completed_at: None,
        observation: crate::store::work::task::Observation::NotRequired,
    };
    let pr = TaskPr {
        id: TaskPrId::new(),
        task_id: task_id.clone(),
        sequence: 1,
        slug: "probe".to_string(),
        branch: "probe".to_string(),
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
    (dir, store, task, pr)
}

pub(crate) fn store_with_task() -> (tempfile::TempDir, SqliteStore, TaskId) {
    let (dir, store, task, pr) = task();
    let me = ownership(
        &store,
        store
            .new_task_scope(&task.repository_id, task.project_id.as_ref())
            .unwrap(),
    );
    store.seed_task(&task, &pr, Some(&me)).unwrap();
    (dir, store, task.id)
}

pub(crate) fn conversation(
    graph_lf_process_id: Option<String>,
    task_id: Option<TaskId>,
    wave_id: Option<WaveId>,
) -> crate::store::session::LfSession {
    crate::store::session::LfSession {
        captured: None,
        id: uuid::Uuid::new_v4().to_string().into(),
        artifact_key: Some(crate::store::session_record::new_artifact_key()),
        caller_artifact_key: None,
        input_published: false,
        cwd: "/repo".into(),
        skill: Some("implement".into()),
        provider: None,
        model: None,
        node: None,
        iterations: None,
        task_id,
        wave_id,
        graph_lf_process_id,
        work_source: Some(WorkSource::Declared),
        bound_at: None,
        interactive: false,
        repo: None,
        title: "Implementation".into(),
        title_source: crate::store::session::TitleSource::Generated,
        request: None,
        ready_summary: None,
        completed_at: None,
        created_at: 100,
    }
}

pub(crate) fn unregistered_task(
    store: &SqliteStore,
    existing: &TaskId,
    checkout: PathBuf,
) -> (Task, TaskPr) {
    let mut task = store.task(existing).unwrap().unwrap();
    let mut pr = store.task_prs(existing).unwrap().remove(0);
    task.id = TaskId::new();
    task.linear_id = Some(LinearIssueId::new("later-issue").unwrap());
    task.identifier = "PROBE-2".into();
    task.worktree = Some(checkout);
    task.workspace_slug = "later-checkout".into();
    pr.id = TaskPrId::new();
    pr.task_id = task.id.clone();
    pr.slug = task.workspace_slug.clone();
    task.branch = task.workspace_slug.clone();
    pr.branch = task.branch.clone();
    (task, pr)
}

pub(crate) fn unowned_task() -> (tempfile::TempDir, SqliteStore, TaskId) {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::store::test_fixture::open(&dir.path().join("db")).unwrap();
    let repo = dir.path().canonicalize().unwrap();
    let task = store
        .create_task(&crate::store::work::task::NewTask {
            id: TaskId::new(),
            repository_id: crate::store::test_fixture::repository(&store, repo.to_str().unwrap()),
            project_id: None,
            title: "Private Machines".into(),
            description: String::new(),
            due_date: None,
        })
        .unwrap();
    (dir, store, task.id)
}
