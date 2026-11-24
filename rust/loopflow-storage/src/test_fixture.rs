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
use crate::planning::{LinearIssueId, LinearProjectId};
use crate::session::{LfSession, TitleSource, WorkSource};
use crate::steer::SteerAdapter;
use crate::work::project::Project;
use crate::work::task::{LinearWritebackState, Task, TaskPr, TaskPrId};

pub fn build() -> MigrationBuild<'static> {
    MigrationBuild {
        provenance: "development",
        source_identity: "storage-fixture".into(),
        source_revision: "storage-fixture",
        version: "storage-fixture",
        package_version: env!("CARGO_PKG_VERSION"),
    }
}

pub fn observers() -> MigrationObservers<'static> {
    MigrationObservers {
        export_contract: &|_, _| Ok(()),
        process_evidence: &|_, _| ProcessIdentityEvidence::Unknown,
    }
}

pub fn key() -> Result<[u8; 32], super::token_crypto::TokenCryptoError> {
    Ok([7; 32])
}

pub fn open(path: &Path) -> StoreResult<SqliteStore> {
    SqliteStore::open_ephemeral_prepared(path, &build(), &observers(), key)
}

pub fn repository(store: &SqliteStore, path: &str) -> RepositoryId {
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

/// Register saved Wave facts without authored configuration or discovery.
/// One exchange as a hub carries it: `local` sends what its hub has not
/// taken, then takes in what changed there.
pub fn exchange(local: &SqliteStore, local_repo: &str, hub: &SqliteStore, hub_repo: &str) {
    use crate::sqlite::plan_carriers::Carrier;
    let held = local
        .select_carrier(local_repo, Carrier::Hub, "hub")
        .unwrap();
    let page = local
        .plan_records_since(local_repo, held.sent_seq, usize::MAX)
        .unwrap();
    // What the hub could not write stays unsent and goes again.
    if hub
        .apply_plan_records(hub_repo, &page.records, None, &Default::default())
        .unwrap()
        .is_empty()
    {
        local.plan_sent(local_repo, Carrier::Hub, page.seq).unwrap();
    }
    let page = hub
        .plan_records_since(hub_repo, held.seen_seq.unwrap_or(0), usize::MAX)
        .unwrap();
    local
        .apply_plan_records(
            local_repo,
            &page.records,
            Some((Carrier::Hub, page.seq)),
            &Default::default(),
        )
        .unwrap();
}

/// Take `edits` in as another Machine's values, newer than anything here.
pub fn arrive(
    store: &SqliteStore,
    repo: &str,
    edits: &[crate::work::edit::WorkEdit],
) -> StoreResult<()> {
    use crate::sqlite::planning_write;
    use crate::work::edit::{StampedRecord, StampedValue};
    let at = planning_write::next_at(&store.conn.lock().unwrap())?;
    let records: Vec<StampedRecord> = planning_write::grouped(edits)
        .into_iter()
        .map(|((table, id), edits)| StampedRecord {
            table: table.to_owned(),
            id: id.to_owned(),
            fields: edits
                .into_iter()
                .map(|edit| StampedValue {
                    value: edit.change,
                    at,
                    machine: "home_elsewhere".into(),
                })
                .collect(),
        })
        .collect();
    store
        .apply_plan_records(repo, &records, None, &Default::default())
        .map(drop)
}

pub fn wave(store: &SqliteStore, wave: &crate::work::wave::Wave) {
    repository(store, wave.repo());
    assert!(store.upsert_wave_prepared(wave, None).unwrap());
}

pub fn project(store: &SqliteStore, repo: &str, name: &str) -> Project {
    let wave = crate::work::wave::Wave::new(WaveId::new(), name.into(), repo.into());
    self::wave(store, &wave);
    let project = store.ensure_project(wave.id(), name).unwrap();
    store.project(&project).unwrap().unwrap()
}

pub fn ownership(store: &SqliteStore, scope: TaskScope) -> OwnershipFacts {
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

pub fn steer() -> (tempfile::TempDir, SqliteStore, SteerAdapter) {
    let home = tempfile::tempdir().unwrap();
    let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
    store.test_session("session", &crate::session_record::new_artifact_key());
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
            None,
            &crate::sqlite::SessionCaller {
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
            crate::steer::NativeBoundary::Activate { source: "startup" },
            || Ok(()),
        )
        .unwrap();
    (home, store, adapter)
}

/// Host the fixture's Session in a held terminal.
pub fn hold(store: &SqliteStore) -> TerminalId {
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

pub fn native_steer() -> (tempfile::TempDir, SqliteStore, SteerAdapter, TerminalId) {
    let (home, store, adapter) = steer();
    let terminal = hold(&store);
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "UPDATE agent_sessions SET agent_interactive=1 WHERE agent_process_id=?1",
            [&adapter.agent_process_id],
        )
        .unwrap();
    }
    (home, store, adapter, terminal)
}
pub fn unpublished_conversation(
    task_id: Option<TaskId>,
    wave_id: Option<WaveId>,
    created_at: i64,
) -> LfSession {
    LfSession {
        captured: None,
        id: uuid::Uuid::new_v4().to_string().into(),
        artifact_key: Some(crate::session_record::new_artifact_key()),
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
        completed_at: None,
        created_at,
    }
}

pub fn task() -> (tempfile::TempDir, SqliteStore, Task, TaskPr) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("loopflow.db");
    let store = crate::test_fixture::open(&path).expect("open a fresh store");
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
        status: crate::work::project::ProjectStatus::Started,
        tracker_id: Some(LinearProjectId::new("999bdbdd-c045-41a6-8ffc-a97c4a40b0b3").unwrap()),
        slug: "probe".to_string(),
        name: "Probe".to_string(),
        description: "Probe Task execution".to_string(),
        krs: Vec::new(),
        metric_targets: Vec::new(),
        tracker_synced_at: Some(now.unix_timestamp()),

        wave_id: wave_id.clone(),
        created_at: now,
        updated_at: now,
    };
    store.insert_project(&project).unwrap();
    crate::sqlite::project_selection::write_project_binding(
        &store,
        &wave_id,
        None,
        project.tracker_id.as_ref().unwrap().as_str(),
        &crate::PlanningLocks::new(tempfile::tempfile().unwrap()),
    )
    .unwrap();
    let task = Task {
        parent_task_id: None,
        repository_id: crate::test_fixture::repository(&store, "/repo"),
        id: task_id.clone(),

        revision: 0,
        tracker_id: Some(LinearIssueId::new("task-uuid").unwrap()),
        identifier: "PROBE-1".to_string(),
        title: "Probe Task execution".to_string(),
        description: "Exercise the Task Graph store".to_string(),
        tracker_synced_at: Some(now.unix_timestamp()),

        tracker_writeback: LinearWritebackState::Current,
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
        observation: crate::work::task::Observation::NotRequired,
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

pub fn store_with_task() -> (tempfile::TempDir, SqliteStore, TaskId) {
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

pub fn conversation(
    graph_lf_process_id: Option<String>,
    task_id: Option<TaskId>,
    wave_id: Option<WaveId>,
) -> crate::session::LfSession {
    crate::session::LfSession {
        captured: None,
        id: uuid::Uuid::new_v4().to_string().into(),
        artifact_key: Some(crate::session_record::new_artifact_key()),
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
        title_source: crate::session::TitleSource::Generated,
        request: None,
        completed_at: None,
        created_at: 100,
    }
}

pub fn unregistered_task(
    store: &SqliteStore,
    existing: &TaskId,
    checkout: PathBuf,
) -> (Task, TaskPr) {
    let mut task = store.task(existing).unwrap().unwrap();
    let mut pr = store.task_prs(existing).unwrap().remove(0);
    task.id = TaskId::new();
    task.tracker_id = Some(LinearIssueId::new("later-issue").unwrap());
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

pub fn unowned_task() -> (tempfile::TempDir, SqliteStore, TaskId) {
    let dir = tempfile::tempdir().unwrap();
    let store = crate::test_fixture::open(&dir.path().join("db")).unwrap();
    let repo = dir.path().canonicalize().unwrap();
    let task = store
        .create_task(&crate::work::task::NewTask {
            parent_task_id: None,
            id: TaskId::new(),
            repository_id: crate::test_fixture::repository(&store, repo.to_str().unwrap()),
            project_id: None,
            title: "Private Machines".into(),
            description: String::new(),
            due_date: None,
        })
        .unwrap();
    (dir, store, task.id)
}

pub fn process(store: &SqliteStore, number: u32, started_at: i64) -> LfProcessId {
    let id = LfProcessId::parse(&format!("00000000-0000-0000-0000-{number:012x}")).unwrap();
    store
        .conn
        .lock()
        .unwrap()
        .execute(
            "INSERT INTO processes(id,trace_id,started_at,command,repo,cwd)
         VALUES(?1,?2,?3,?4,'/repo','/missing-checkout')",
            rusqlite::params![
                id,
                loopflow_model::id::TraceId::new(),
                started_at,
                serde_json::to_string(&["lf", "inspect"]).unwrap()
            ],
        )
        .unwrap();
    id
}

pub fn record_captured_start(store: &SqliteStore, session: &str, turn: &str) {
    let attachment = store
        .session_attachment(session)
        .unwrap()
        .unwrap_or_else(|| {
            let process = loopflow_model::id::LfProcessId::new();
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                    [&process],
                )
                .unwrap();
            store
                .attach_session(
                    session,
                    None,
                    &process,
                    None,
                    &crate::sqlite::SessionCaller {
                        agent: None,
                        capture_key: None,
                    },
                )
                .unwrap()
        });
    let origin = store.session_turn_origin(session, &attachment).unwrap();
    store.record_session_turn_origin(&"thread".into(), turn, &origin);
}

pub fn retirement() -> (
    tempfile::TempDir,
    SqliteStore,
    TaskId,
    loopflow_model::id::TerminalId,
) {
    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("loopflow.db");
    let store = crate::test_fixture::open(&path).unwrap();
    let repo = repository(&store, "/repo");
    let task = TaskId::new();
    let viewer = loopflow_model::id::TerminalId::new();
    store.create_terminal(&viewer, "build").unwrap();
    store.test_session("viewed", &crate::session_record::new_artifact_key());
    {
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO tasks(id,repository_id,issue_identifier,issue_title,issue_description,
                 workspace_slug,created_at,updated_at,planning_state,planning_completed_at)
             VALUES(?1,?2,'PLAN-1','Plan','','plan',1,1,'completed','done')",
            rusqlite::params![task.as_str(), repo.as_str()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO task_checkouts(task_id,worktree,branch) VALUES(?1,'/repo.task','task')",
            [task.as_str()],
        )
        .unwrap();
        conn.execute(
            "UPDATE agent_sessions SET cwd='/repo.task/src',task_id=?2,interactive=1,held_terminal_id=?1
             WHERE id='viewed'",
            rusqlite::params![viewer, task.as_str()],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO task_retirements(task_id,completion,checkout,branch,phase,due_at)
             VALUES(?1,'done','/repo.task','task','waiting',100)",
            [task.as_str()],
        )
        .unwrap();
    }
    (home, store, task, viewer)
}

/// Open an upgrade fence without exposing transaction internals to application fixtures.
pub fn session_fence_file(database: &Path, locator: &str) -> StoreResult<std::fs::File> {
    crate::canonical_ids::session_fence_file(database, locator)
}

pub fn validate_persisted_json(conn: &rusqlite::Connection) -> StoreResult<()> {
    crate::migration_runner::validate_persisted_json(conn)
}
