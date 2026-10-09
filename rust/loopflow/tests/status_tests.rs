//! `lf wave status` is an audit surface, so its contract is user-facing: the JSON it
//! promises must be the JSON it emits, and the wave you are standing in must be
//! the wave it reports. Drives the real binary against a seeded `LF_HOME`.

#[path = "support/planning.rs"]
mod planning;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use loopflow::child::ChildRef;
use loopflow::id::WaveId;
use loopflow::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
use loopflow::store::migrations;
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::{PmSnapshotRow, StorageConfig};
use loopflow::work::project::{Project, ProjectEventKind, ProjectId};
use loopflow::work::task::{
    Observation, PmWritebackState, PrMergeMode, Task, TaskId, TaskPr, TaskPrId,
};
use loopflow::work::wave::metrics::{
    load_metric_contract, MetricObservation, ObservationAcceptance,
};
use loopflow::work::wave::Wave;
use time::OffsetDateTime;

const PREVIOUS_RELEASE_TASK_PR_FIXTURE: &str = include_str!("fixtures/store_0_12_8_task_pr.sql");
const PERSISTED_TASK_ID: &str = "task_40fbeeaadfbca5367aa7391432ae84ff";

/// A machine home holding one Wave.
fn seed(home: &Path, wave_name: &str) -> Wave {
    std::fs::create_dir_all(home).expect("home");
    let repo = home.join("repo");
    std::fs::create_dir_all(&repo).expect("repo");
    let db = home.join("loopflow.db");
    let store = SqliteStore::new(&db).expect("open store");
    let wave = Wave::new(
        WaveId::new(),
        wave_name.to_string(),
        repo.display().to_string(),
    );
    store.create_wave(&wave).expect("register wave");
    wave
}

fn test_project(wave: &Wave, slug: &str, updated_at: OffsetDateTime) -> Project {
    Project {
        id: ProjectId::new(),
        plan: ProjectPlan {
            summary: String::new(),
            workflow: "feature".into(),
            status: loopflow::pm::ProjectStatus::Started,
            linear_id: Some(
                LinearProjectId::new(uuid::Uuid::new_v4().to_string()).expect("Linear Project id"),
            ),
            slug: slug.to_string(),
            name: slug.replace('-', " "),
            prompt_context: "Keep status truthful.".to_string(),
            pm_snapshot_synced_at: Some(updated_at.unix_timestamp()),
        },
        wave_id: wave.id().clone(),
        iteration: 0,
        abandon_intent: None,
        created_at: updated_at,
        updated_at,
    }
}

fn select_project(home: &Path, wave: &Wave, project_id: &str) {
    let connection = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    connection.execute(
        "UPDATE waves SET current_project_id=(SELECT id FROM projects WHERE wave_id=?1 AND external_project_id=?2) WHERE id=?1",
        rusqlite::params![wave.id().as_str(), project_id],
    ).unwrap();
    connection.execute(
        "UPDATE projects SET status=CASE WHEN external_project_id=?2 THEN 'started' ELSE 'completed' END WHERE wave_id=?1",
        rusqlite::params![wave.id().as_str(), project_id],
    ).unwrap();
}

fn put_project_snapshot(home: &Path, wave: &Wave, project: &Project) {
    let payload = serde_json::json!({
        "projects": [{
            "id": project.plan.linear_id.as_ref().unwrap().as_str(),
            "slug": project.plan.slug,
            "name": project.plan.name,
            "summary": "Keep status truthful.",
            "metric_targets": [],
            "workflow": "feature", "status": "started",
            "krs": [{"text": "Current state and history stay distinct", "holds": false}],
            "initiative_ids": ["initiative-infrastructure"],
            "team_ids": ["team-infrastructure"]
        }],
        "items": []
    });
    let store = SqliteStore::new(&home.join("loopflow.db")).expect("open status store");
    select_project(
        home,
        wave,
        project.plan.linear_id.as_ref().unwrap().as_str(),
    );
    store
        .put_pm_snapshot(&PmSnapshotRow {
            wave_id: wave.id().clone(),
            provider: "linear".to_string(),
            initiative: "initiative-infrastructure".to_string(),
            synced_at: OffsetDateTime::now_utc().unix_timestamp(),
            snapshot: serde_json::from_value(payload).expect("parse PM snapshot"),
        })
        .expect("seed PM snapshot");
}

fn seed_credential_history(home: &Path) -> Project {
    let wave = seed(home, "infrastructure");
    let store = SqliteStore::new(&home.join("loopflow.db")).expect("open status store");
    let now = OffsetDateTime::now_utc();
    let project = test_project(&wave, "stability-security", now);
    store.insert_project(&project).expect("seed Project");
    let failure = store
        .append_project_event(
            &project.id,
            &ProjectEventKind::Failed {
                error: "project runner failed: credential is missing".to_string(),
                resumable: true,
            },
        )
        .expect("record failure history");
    rusqlite::Connection::open(home.join("loopflow.db"))
        .expect("open status store")
        .execute(
            "UPDATE project_events SET created_at=created_at-60 WHERE id=?1",
            [failure.id],
        )
        .expect("age historical failure");
    put_project_snapshot(home, &wave, &project);
    project
}

/// `lf wave status --json` in a clean environment, optionally standing inside a wave.
fn status_json(home: &Path, args: &[&str], ambient_wave_id: Option<&str>) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .args(["wave", "status"])
        .args(args)
        .arg("--json")
        .env("LF_HOME", home)
        .env_remove("LF_TRACE_ID")
        .env_remove("LF_WAVE_ID")
        .current_dir(home.join("repo"));
    if let Some(id) = ambient_wave_id {
        command.env("LF_WAVE_ID", id);
    }
    prepend_test_bin(&mut command, home);
    let output = command.output().expect("lf wave status runs");
    assert!(
        output.status.success(),
        "lf wave status failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    serde_json::from_str(stdout.trim()).unwrap_or_else(|err| panic!("not JSON: {err}\n{stdout}"))
}

fn status_human(home: &Path, wave: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["wave", "status", wave])
        .env("LF_HOME", home)
        .current_dir(home.join("repo"))
        .output()
        .expect("lf wave status runs");
    assert!(
        output.status.success(),
        "lf wave status failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("status is utf8")
}

fn roadmap_json(home: &Path, wave: &str) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .args(["roadmap", "--wave", wave, "--json"])
        .env("LF_HOME", home)
        .env_remove("LF_TRACE_ID")
        .env_remove("LF_WAVE_ID")
        .current_dir(home.join("repo"));
    prepend_test_bin(&mut command, home);
    let output = command.output().expect("lf roadmap runs");
    assert!(
        output.status.success(),
        "lf roadmap failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("lf roadmap emits JSON")
}

fn prepend_test_bin(command: &mut Command, home: &Path) {
    let bin = home.join("bin");
    if !bin.is_dir() {
        return;
    }
    let inherited = std::env::var_os("PATH").unwrap_or_default();
    let paths = std::iter::once(bin).chain(std::env::split_paths(&inherited));
    command.env("PATH", std::env::join_paths(paths).expect("test PATH"));
}

fn seed_stale_project_work(home: &Path, abandon_stale_project: bool, current_task: bool) {
    const STALE_WORK_ID: &str = "proj_e972b70272fbb5e91c096ebe657f9f9b";
    const STALE_PROJECT_ID: &str = "f56c583c-c360-4dc4-ba12-4b5a02268623";
    const STALE_TASK_WORK_ID: &str = "task_40fbeeaadfbca5367aa7391432ae84ff";

    let wave = seed(home, "product");
    let repo = home.join("repo");
    std::fs::create_dir_all(&repo).expect("repo");
    let store = SqliteStore::new(&home.join("loopflow.db")).expect("open store");
    let now = OffsetDateTime::now_utc();
    let stale = Project {
        id: ProjectId::parse(STALE_WORK_ID).expect("recorded Project Work id"),
        plan: ProjectPlan {
            summary: String::new(),
            workflow: "feature".into(),
            status: loopflow::pm::ProjectStatus::Started,
            linear_id: Some(
                LinearProjectId::new(STALE_PROJECT_ID).expect("recorded PM Project id"),
            ),
            slug: "technical-architecture".to_string(),
            name: "Technical Architecture".to_string(),
            prompt_context: "Keep the system legible and minimally simple.".to_string(),
            pm_snapshot_synced_at: Some(now.unix_timestamp() - 1),
        },
        wave_id: wave.id().clone(),
        iteration: 0,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    };
    store.insert_project(&stale).expect("seed stale Project");
    select_project(home, &wave, stale.plan.linear_id.as_ref().unwrap().as_str());
    let stale_task = Task {
        id: TaskId::parse(STALE_TASK_WORK_ID).expect("recorded Task Work id"),
        plan: TaskPlan {
            revision: 0,
            linear_id: Some(
                LinearIssueId::new(if current_task {
                    "task-prd-52"
                } else {
                    "linear-task-w2-127"
                })
                .expect("recorded PM Task id"),
            ),
            identifier: "W2-127".to_string(),
            title: "Preserve historical architecture evidence".to_string(),
            description: "This Task outlived its retired Linear Project.".to_string(),
            pm_snapshot_synced_at: Some(now.unix_timestamp() - 1),
        },
        pm_writeback: PmWritebackState::Current,
        wave_id: wave.id().clone(),
        project_id: stale.id.clone(),
        worktree: Some(home.join("repo.w2-127")),
        workspace_slug: "w2-127".to_string(),
        branch: "jack-heart/w2-127".to_string(),
        base_commit: "deadbeef".to_string(),
        parent_pr_id: None,
        agent: None,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
        observation: Observation::NotRequired,
    };
    let stale_pr = TaskPr {
        id: TaskPrId::new(),
        task_id: stale_task.id.clone(),
        sequence: 1,
        slug: stale_task.workspace_slug.clone(),
        branch: "jack-heart/w2-127".to_string(),
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
    planning::seed_unplaced_task(&home.join("loopflow.db"), &stale_task);
    store
        .place_task(
            &stale_task.id,
            stale_task.worktree.as_ref().unwrap(),
            &stale_task.workspace_slug,
            &stale_pr,
        )
        .expect("seed orphaned Task");
    if abandon_stale_project {
        let stale_work = store
            .work_for_child(&ChildRef::Project(stale.id.clone()))
            .expect("resolve stale Project Work");
        store
            .abandon(
                &stale_work,
                "Project is absent from the current PM snapshot",
            )
            .expect("retire stale Project Work");
    }

    let current = Project {
        id: ProjectId::new(),
        plan: ProjectPlan {
            summary: String::new(),
            workflow: "feature".into(),
            status: loopflow::pm::ProjectStatus::Started,
            linear_id: Some(
                LinearProjectId::new("95159066-9098-4d0b-8903-01459dc7ec14")
                    .expect("current PM Project id"),
            ),
            slug: "auditability".to_string(),
            name: "Auditability".to_string(),
            prompt_context: "Every claim points to its receipt.".to_string(),
            pm_snapshot_synced_at: Some(now.unix_timestamp()),
        },
        wave_id: wave.id().clone(),
        iteration: 0,
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    };
    store
        .insert_project(&current)
        .expect("seed current Project");
    select_project(
        home,
        &wave,
        current.plan.linear_id.as_ref().unwrap().as_str(),
    );

    let bin = home.join("bin");
    std::fs::create_dir_all(&bin).expect("test bin");
    let tmux = bin.join("tmux");
    std::fs::write(&tmux, "#!/bin/sh\nexit 0\n").expect("fake tmux");
    std::fs::set_permissions(&tmux, std::fs::Permissions::from_mode(0o755))
        .expect("make fake tmux executable");

    let payload = serde_json::json!({
        "projects": [
            {
                "id": "95159066-9098-4d0b-8903-01459dc7ec14",
                "slug": "auditability",
                "name": "Auditability",
                "summary": "Every claim points to its receipt.",
                "metric_targets": [],
                "workflow": "feature", "status": "started",
                "krs": [{"text": "Every visible state carries its reason", "holds": false}],
                "initiative_ids": ["initiative-product"],
                "team_ids": ["team-product"]
            }
        ],
        "items": [
            {
                "id": "task-prd-52",
                "identifier": "PRD-52",
                "url": "https://linear.app/loopflow/issue/PRD-52",
                "name": "Expose one fleet snapshot from Wave to raw trace",
                "description": "Keep focused reads useful through stale Work.",
                "rank": 1,
                "completed": false,
                "project_id": "95159066-9098-4d0b-8903-01459dc7ec14",
                "project": "auditability",
                "team_id": "team-product",
                "assignee": null
            }
        ]
    });
    store
        .put_pm_snapshot(&PmSnapshotRow {
            wave_id: wave.id().clone(),
            provider: "linear".to_string(),
            initiative: "initiative-product".to_string(),
            synced_at: now.unix_timestamp(),
            snapshot: serde_json::from_value(payload).expect("parse PM snapshot"),
        })
        .expect("seed PM snapshot");
}

fn seed_persisted_merge_request_without_copy(home: &Path) {
    seed_stale_project_work(home, true, true);
    let connection =
        rusqlite::Connection::open(home.join("loopflow.db")).expect("open seeded Task registry");
    let now = OffsetDateTime::now_utc().unix_timestamp();
    connection
        .execute(
            "UPDATE tasks SET
                project_id=(SELECT id FROM projects WHERE project_slug='auditability'),
                external_issue_id='task-prd-52', issue_identifier='PRD-52',
                issue_title='Expose one fleet snapshot from Wave to raw trace'
             WHERE id=?1",
            [PERSISTED_TASK_ID],
        )
        .expect("move persisted Task into the current PM Project");
    connection.execute(
        "INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at)
         SELECT 'pr-copy-fixture',id,1,workspace_slug,branch,base_commit,?2,?2 FROM tasks WHERE id=?1",
        rusqlite::params![PERSISTED_TASK_ID, now],
    ).unwrap();
    connection
        .execute(
            "UPDATE task_prs SET
                publication_requested_at=?2,
                after_merge='continue_task',
                github_number=240,
                github_url='https://github.com/loopflowstudio/loopflow/pull/240',
                github_head_sha='head-240',
                merge_mode='user',
                merge_requested_at=?2,
                merge_head_sha='head-240'
             WHERE task_id=?1",
            rusqlite::params![PERSISTED_TASK_ID, now],
        )
        .expect("seed pre-copy merge request");
}

fn seed_previous_release_task_pr(home: &Path) {
    std::fs::create_dir_all(home.join("repo")).expect("fixture repo");
    let fixture = PREVIOUS_RELEASE_TASK_PR_FIXTURE.replace(
        "__LF_HOME__",
        home.to_str().expect("fixture Machine path is utf8"),
    );
    let database = home.join("loopflow.db");
    let connection = rusqlite::Connection::open(&database).expect("open previous release store");
    connection
        .execute_batch(&fixture)
        .expect("load previous release fixture");
    let frontier: String = connection
        .query_row(
            "SELECT version FROM schema_migrations ORDER BY rowid DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("read previous release frontier");
    assert_eq!(frontier, "0.12.8.001_release");
    migrations::apply_sqlite(&connection)
        .expect("apply published migrations to historical fixture");
    for draft in loopflow::build_info::migration_draft_manifest() {
        connection
            .execute_batch(draft.sql)
            .expect("apply finished draft to historical fixture");
    }
    drop(connection);

    let store = SqliteStore::new(&database).expect("open migrated previous release store");
    let task_id = TaskId::parse(PERSISTED_TASK_ID).expect("recorded Task id");
    let pr = store
        .active_task_pr(&task_id)
        .expect("decode migrated Task PR")
        .expect("fixture has active Task PR");
    assert!(pr.presentation().is_none());
    assert_eq!(
        pr.merge_request().expect("explicit merge request").mode,
        PrMergeMode::User
    );
    let wave = store.list_waves(None).unwrap().pop().unwrap();
    select_project(home, &wave, "95159066-9098-4d0b-8903-01459dc7ec14");
    drop(store);

    let connection = rusqlite::Connection::open(&database).expect("reopen migrated store");
    let frontier: String = connection
        .query_row(
            "SELECT version FROM schema_migrations ORDER BY rowid DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .expect("read current migration frontier");
    assert_eq!(
        frontier,
        loopflow::store::migrations::latest_known_version()
    );
    drop(connection);
}

#[test]
fn project_operator_failures_remain_historical_without_reappearing_on_the_wave() {
    let home = tempfile::tempdir().expect("tempdir");
    let project = seed_credential_history(home.path());
    let status = status_json(home.path(), &["infrastructure"], None);
    let roadmap = roadmap_json(home.path(), "infrastructure");
    let expected_projects = serde_json::json!({
        "state": "ok",
        "items": [{
            "id": project.plan.linear_id.as_ref().unwrap().as_str(),
            "work_id": project.id.as_str(),
            "slug": project.plan.slug,
            "name": project.plan.name,
            "workflow": "feature",
            "status": "started",
            "current": true,
            "metric_targets": [],
            "krs": [{"text": "Current state and history stay distinct", "holds": false}]
        }],
        "truncated": false
    });
    for view in [&status, &roadmap["waves"][0]] {
        assert_eq!(view["projects"], expected_projects);
        assert_eq!(view["wave"]["status"], "ready");
        assert_eq!(view["tasks"]["state"], "ok");
        assert_eq!(view["tasks"]["items"], serde_json::json!([]));
        assert_eq!(view["unavailable_tasks"], serde_json::json!([]));
        assert!(!view.to_string().contains("credential"));
    }
    let human = status_human(home.path(), "infrastructure");
    assert!(!human.contains("credential"));
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    assert_eq!(
        store
            .latest_project_failure(&project.id)
            .unwrap()
            .unwrap()
            .message,
        "project runner failed: credential is missing"
    );
}

/// The reproduced break: inside a Wave agent's environment, `LF_WAVE_ID` is a wave id, and
/// bare `lf wave status` read it as a name.
#[test]
fn ambient_wave_id_resolves_the_wave_it_names() {
    let home = tempfile::tempdir().expect("tempdir");
    let wave = seed(home.path(), "audit-b");

    let status = status_json(home.path(), &[], Some(wave.id().as_str()));

    assert_eq!(status["wave"]["id"], wave.id().as_str());
    assert_eq!(status["wave"]["name"], "audit-b");
    assert_eq!(status["history"]["state"], "ok");
}

#[test]
fn all_roadmaps_ignore_inherited_wave_from_a_gui_launch() {
    let home = tempfile::tempdir().unwrap();
    let first = seed(home.path(), "one");
    let second = seed(home.path(), "two");
    for ambient in [first.id().as_str(), "stale-wave-id"] {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(["roadmap", "--all", "--json"])
            .env("LF_HOME", home.path())
            .env_remove("LF_TRACE_ID")
            .env("LF_WAVE_ID", ambient)
            .current_dir("/")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let ids = value["waves"]
            .as_array()
            .unwrap()
            .iter()
            .map(|wave| wave["wave"]["id"].as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(ids, [first.id().as_str(), second.id().as_str()]);
    }
}

#[test]
fn current_wave_reads_exclude_abandoned_registrations_without_deleting_history() {
    let home = tempfile::tempdir().unwrap();
    let current = seed(home.path(), "current");
    let abandoned = seed(home.path(), "accidental");
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let work = loopflow::durable::WorkRef::Wave(abandoned.id().clone());
    store.abandon(&work, "accidental registration").unwrap();
    let run = |args: &[&str]| {
        Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(args)
            .env("LF_HOME", home.path())
            .env_remove("LF_WAVE_ID")
            .env_remove("LF_TRACE_ID")
            .current_dir(home.path().join("repo"))
            .output()
            .unwrap()
    };
    let listing = run(&["wave", "list", "--all", "--current", "--json"]);
    assert!(
        listing.status.success(),
        "{}",
        String::from_utf8_lossy(&listing.stderr)
    );
    let rows: serde_json::Value = serde_json::from_slice(&listing.stdout).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["id"], current.id().as_str());
    assert!(store.get_wave(abandoned.id()).unwrap().is_some());
    assert!(store.get_wave(current.id()).unwrap().is_some());
}

#[test]
fn accepted_metric_evidence_is_identical_in_status_roadmap_and_text() {
    let home = tempfile::tempdir().expect("tempdir");
    let wave = seed(home.path(), "product");
    let repo = home.path().join("repo");
    let metrics_dir = repo.join("wave/product/metrics");
    std::fs::create_dir_all(&metrics_dir).expect("metrics directory");
    let contract_path = metrics_dir.join("task-loop-trust.md");
    std::fs::write(
        &contract_path,
        r#"---
schema: 1
id: task-loop-trust
stage: graduated
instrument: lifecycle-scorecard
unit: ratio
window: 7d
freshness: 6h
---

# Task loops earn trust

Count dispatched Task loops that settle without rescue.
"#,
    )
    .expect("metric contract");
    let project_payload = serde_json::json!({
        "projects": [{
            "id": "d19956b2-9955-437d-aea6-d91766231c77",
            "slug": "loopflow-api",
            "name": "Loopflow API",
            "summary": "One product contract.",
            "metric_targets": [{"metric_id": "task-loop-trust", "target": {"kind": "at_least", "value": 1.0}}],
            "workflow": "feature", "status": "started",
            "krs": [{"text": "Task loops earn trust for one week", "holds": false}],
            "initiative_ids": ["initiative-product"],
            "team_ids": ["team-product"]
        }],
        "items": []
    });
    let sqlite = SqliteStore::new(&home.path().join("loopflow.db")).expect("open store");
    let now = OffsetDateTime::now_utc();
    sqlite
        .put_pm_snapshot(&PmSnapshotRow {
            wave_id: wave.id().clone(),
            provider: "linear".to_string(),
            initiative: "initiative-product".to_string(),
            synced_at: now.unix_timestamp(),
            snapshot: serde_json::from_value(project_payload.clone()).expect("parse PM snapshot"),
        })
        .expect("seed PM snapshot");
    select_project(home.path(), &wave, "d19956b2-9955-437d-aea6-d91766231c77");
    drop(sqlite);

    let contract = load_metric_contract(&contract_path, wave.id().as_str()).expect("contract");
    let mut observation = MetricObservation::Observed {
        identity: contract.identity.clone(),
        contract_revision: contract.contract_revision.clone(),
        instrument: contract.instrument.clone(),
        observation_id: String::new(),
        value: 1.0,
        source_window_start: now - time::Duration::days(7),
        source_window_end: now,
        complete: true,
    };
    let id = observation
        .expected_observation_id()
        .expect("observation digest");
    let MetricObservation::Observed { observation_id, .. } = &mut observation else {
        unreachable!()
    };
    *observation_id = id;
    let runtime = tokio::runtime::Runtime::new().expect("metric runtime");
    runtime.block_on(async {
        let store = loopflow::store::open_ephemeral_store(&StorageConfig::sqlite(
            home.path().join("loopflow.db"),
        ))
        .await
        .expect("open shared store");
        store
            .register_metric_instrument(&contract.identity, &contract.instrument, now)
            .await
            .expect("register instrument");
        assert_eq!(
            store
                .accept_metric_observation(&contract, observation, now)
                .await
                .expect("accept observation"),
            ObservationAcceptance::Accepted
        );
    });

    let status = status_json(home.path(), &["product"], None);
    let roadmap = roadmap_json(home.path(), "product");
    let status_metric = &status["metric_portfolio"]["metrics"][0];
    assert_eq!(status_metric["name"], "Task loops earn trust");
    assert_eq!(status_metric["identity"]["wave_id"], wave.id().as_str());
    assert!(status_metric.get("project_id").is_none());
    assert_eq!(
        status_metric["target"],
        serde_json::json!({"kind": "at_least", "value": 1.0})
    );
    assert_eq!(status_metric["window"], "7d");
    assert_eq!(status_metric["freshness"]["kind"], "fresh");
    assert_eq!(status_metric["evidence"]["kind"], "met");
    assert_eq!(status_metric["evidence"]["value"], 1.0);
    assert_eq!(status["projects"]["state"], "ok");
    let projects = status["projects"]["items"].as_array().unwrap();
    assert_eq!(projects.len(), 1);
    assert_eq!(projects[0]["id"], project_payload["projects"][0]["id"]);
    assert_eq!(projects[0]["status"], "started");
    assert_eq!(projects[0]["krs"], project_payload["projects"][0]["krs"]);
    assert_eq!(
        projects[0]["metric_targets"],
        project_payload["projects"][0]["metric_targets"]
    );
    assert_eq!(roadmap["waves"][0]["projects"], status["projects"]);
    assert_eq!(
        roadmap["waves"][0]["metric_portfolio"],
        status["metric_portfolio"]
    );

    let human = status_human(home.path(), "product");
    assert!(
        human.contains("[ ] Task loops earn trust for one week"),
        "{human}"
    );
    assert!(human.contains("Task loops earn trust  [met]"), "{human}");
    assert!(
        human.contains("Value 100.00% · Target >= 100.00% over 7d"),
        "{human}"
    );
}

/// A wave that has done nothing reports an empty reading, not a missing one:
/// "we looked and found nothing" is a claim a client can trust.
#[test]
fn a_wave_with_no_runs_reports_an_empty_reading_not_a_missing_one() {
    let home = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(home.path()).expect("home");
    let database = home.path().join("loopflow.db");
    let store = SqliteStore::new(&database).expect("open store");
    std::fs::create_dir_all(home.path().join("repo")).expect("repo");
    let wave = Wave::new(
        WaveId::new(),
        "audit-c".to_string(),
        home.path().join("repo").display().to_string(),
    );
    store.create_wave(&wave).expect("register wave");

    let status = status_json(home.path(), &["audit-c"], None);

    assert_eq!(status["wave"]["status"], "ready");
    assert_eq!(status["history"]["state"], "ok");
    assert_eq!(status["history"]["items"], serde_json::json!([]));
    assert_eq!(status["history"]["truncated"], false);
}

#[test]
fn orphaned_task_work_preserves_status_and_roadmap_evidence() {
    for abandon_parent in [true, false] {
        let home = tempfile::tempdir().expect("tempdir");
        seed_stale_project_work(home.path(), abandon_parent, false);
        let status = status_json(home.path(), &["product"], None);
        let roadmap = roadmap_json(home.path(), "product");
        let wave = &roadmap["waves"][0];
        for view in [&status, wave] {
            assert_eq!(view["projects"]["state"], "ok");
            assert_eq!(view["projects"]["truncated"], false);
            let projects = view["projects"]["items"].as_array().unwrap();
            assert_eq!(projects.len(), 2);
            let projects: Vec<_> = projects
                .iter()
                .filter(|project| project["id"] == "95159066-9098-4d0b-8903-01459dc7ec14")
                .collect();
            assert_eq!(projects.len(), 1);
            assert_eq!(projects[0]["slug"], "auditability");
            assert_eq!(projects[0]["status"], "started");
            assert_eq!(projects[0]["workflow"], "feature");
            assert_eq!(view["tasks"]["state"], "ok");
            let tasks = view["tasks"]["items"].as_array().unwrap();
            assert_eq!(tasks.len(), 2);
            assert!(tasks
                .iter()
                .any(|task| task["task"]["identifier"] == "PRD-52"));
            assert!(tasks
                .iter()
                .any(|task| task["task"]["identifier"] == "W2-127"));
            assert!(view["unavailable_tasks"].as_array().unwrap().is_empty());
        }
        assert_eq!(wave["projects"], status["projects"]);
        assert_eq!(wave["unavailable_tasks"], status["unavailable_tasks"]);
    }
}

#[test]
fn provider_inventory_loss_preserves_saved_planning_in_both_views() {
    for payload in [None, Some("{}"), Some("not-json")] {
        let home = tempfile::tempdir().unwrap();
        seed_stale_project_work(home.path(), false, false);
        let before = status_json(home.path(), &["product"], None);
        let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        if let Some(payload) = payload {
            conn.execute("UPDATE pm_projects SET body=?1", [payload])
                .unwrap();
        } else {
            conn.execute("DELETE FROM pm_wave_sync", []).unwrap();
        }
        let status = status_json(home.path(), &["product"], None);
        let roadmap = roadmap_json(home.path(), "product");
        for view in [&status, &roadmap["waves"][0]] {
            assert_eq!(view["projects"], before["projects"]);
            assert_eq!(view["tasks"]["state"], "ok");
            let tasks = view["tasks"]["items"].as_array().unwrap();
            assert_eq!(tasks.len(), 2);
            for task in tasks {
                let previous = before["tasks"]["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|old| old["task"]["id"] == task["task"]["id"])
                    .unwrap();
                assert_eq!(task["task"], previous["task"]);
            }
        }
    }
}

#[test]
fn reserved_session_starts_task_in_status_and_roadmap_without_publication() {
    let home = tempfile::tempdir().unwrap();
    seed_persisted_merge_request_without_copy(home.path());
    let connection = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    connection.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    connection
        .execute("DELETE FROM task_prs WHERE task_id=?1", [PERSISTED_TASK_ID])
        .unwrap();

    for assigned in [false, true] {
        if assigned {
            connection
                .execute(
                    "INSERT INTO agent_sessions(id,title,title_source,task_id,wave_id,created_at,cwd,input_published)
                     SELECT ?1,'Reserved work','generated',t.id,p.wave_id,1,t.worktree,0
                     FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=?2",
                    rusqlite::params![uuid::Uuid::new_v4().simple().to_string().as_str(), PERSISTED_TASK_ID],
                )
                .unwrap();
        }
        let status = status_json(home.path(), &["product"], None);
        let roadmap = roadmap_json(home.path(), "product");
        for task in [
            &status["tasks"]["items"][0],
            &roadmap["waves"][0]["tasks"]["items"][0],
        ] {
            assert_eq!(task["task"]["identifier"], "PRD-52");
            assert_eq!(task["runtime"]["started"], assigned);
        }
    }
    assert!(
        !home.path().join("runs").exists(),
        "no provider was launched"
    );
}

#[test]
fn persisted_merge_request_without_copy_keeps_status_and_roadmap_readable() {
    for missing_provider_task in [false, true] {
        let home = tempfile::tempdir().expect("tempdir");
        seed_persisted_merge_request_without_copy(home.path());
        if missing_provider_task {
            let connection = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
            connection.execute("DELETE FROM pm_items", []).unwrap();
        }

        let status = status_json(home.path(), &["product"], None);
        let status_task = &status["tasks"]["items"][0];
        assert_eq!(status_task["task"]["identifier"], "PRD-52");
        assert_eq!(
            status_task["pr"]["publication"]["presentation"],
            serde_json::Value::Null
        );
        assert_eq!(status_task["pr"]["publication"]["merge"]["mode"], "user");

        let roadmap = roadmap_json(home.path(), "product");
        let wave = &roadmap["waves"][0];
        assert_eq!(wave["tasks"]["state"], "ok");
        let roadmap_task = &wave["tasks"]["items"][0];
        assert_eq!(roadmap_task["task"]["identifier"], "PRD-52");
        assert_eq!(
            roadmap_task["pr"]["publication"]["presentation"],
            serde_json::Value::Null
        );
        assert_eq!(roadmap_task["pr"]["publication"]["merge"]["mode"], "user");
    }
}

#[test]
fn previous_release_merge_request_migrates_into_readable_status_and_roadmap() {
    let home = tempfile::tempdir().expect("tempdir");
    seed_previous_release_task_pr(home.path());

    let status = status_json(home.path(), &["product"], None);
    let status_task = &status["tasks"]["items"][0];
    assert_eq!(status_task["task"]["identifier"], "PRD-52");
    assert_eq!(
        status_task["pr"]["publication"]["presentation"],
        serde_json::Value::Null
    );
    assert_eq!(status_task["pr"]["publication"]["merge"]["mode"], "user");

    let roadmap = roadmap_json(home.path(), "product");
    let wave = &roadmap["waves"][0];
    assert_eq!(wave["tasks"]["state"], "ok");
    let roadmap_task = &wave["tasks"]["items"][0];
    assert_eq!(roadmap_task["task"]["identifier"], "PRD-52");
    assert_eq!(
        roadmap_task["pr"]["publication"]["presentation"],
        serde_json::Value::Null
    );
    assert_eq!(roadmap_task["pr"]["publication"]["merge"]["mode"], "user");
}

#[test]
fn exact_task_roadmap_retains_history_without_starting_work() {
    let home = tempfile::tempdir().unwrap();
    seed_stale_project_work(home.path(), false, false);
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    conn.execute("DELETE FROM task_prs", []).unwrap();
    let before: (i64, i64, i64) = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM task_events), (SELECT COUNT(*) FROM agent_sessions), (SELECT COUNT(*) FROM flow_processes)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    for identifier in ["W2-127", "PRD-52", "not-a-task"] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        command
            .args(["roadmap", "--task", identifier, "--all", "--json"])
            .env("LF_HOME", home.path())
            .env("LF_WAVE_ID", "must-not-narrow-exact-lookup")
            .env_remove("LF_CAPTURE_KEY")
            .current_dir(home.path().join("repo"));
        prepend_test_bin(&mut command, home.path());
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        if identifier == "not-a-task" {
            assert_eq!(result["waves"], serde_json::json!([]));
            continue;
        }
        let tasks = result["waves"][0]["tasks"]["items"].as_array().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["task"]["identifier"], identifier);
        assert!(tasks[0]["pr"].is_null());
        if identifier == "W2-127" {
            assert_eq!(tasks[0]["runtime"]["work_id"], PERSISTED_TASK_ID);
            assert_eq!(tasks[0]["runtime"]["started"], false);
            assert_eq!(
                tasks[0]["task"]["name"],
                "Preserve historical architecture evidence"
            );
            assert!(
                result["waves"][0]["unavailable_tasks"]
                    .as_array()
                    .unwrap()
                    .is_empty(),
                "saved local planning remains readable without a current provider inventory"
            );
        }
    }
    let after: (i64, i64, i64) = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM task_events), (SELECT COUNT(*) FROM agent_sessions), (SELECT COUNT(*) FROM flow_processes)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        before, after,
        "inspection must not create Sessions or Started evidence"
    );
}

#[test]
fn exact_task_roadmap_scopes_duplicate_identifiers_to_registered_repositories() {
    let home = tempfile::tempdir().unwrap();
    seed_stale_project_work(home.path(), false, false);
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let original = store.list_waves(None).unwrap().remove(0);
    let other_repo = home.path().join("other-repo");
    std::fs::create_dir_all(&other_repo).unwrap();
    // Registered repositories without Git metadata remain valid cached readers.
    let other = Wave::new(
        WaveId::new(),
        "other".into(),
        other_repo.display().to_string(),
    );
    store.create_wave(&other).unwrap();
    let mut snapshot = store.pm_snapshot(original.id()).unwrap().unwrap();
    snapshot.snapshot.projects[0].id = "other-project".into();
    snapshot.snapshot.projects[0].initiative_ids = vec!["other-initiative".into()];
    snapshot.snapshot.items[0].id = "other-task".into();
    snapshot.snapshot.items[0].project_id = Some("other-project".into());
    snapshot.snapshot.items[0].completed = true;
    snapshot.wave_id = other.id().clone();
    snapshot.initiative = "other-initiative".into();
    store.put_pm_snapshot(&snapshot).unwrap();
    assert!(store
        .task_by_issue("PRD-52")
        .unwrap_err()
        .to_string()
        .contains("multiple stable Tasks"));

    for all in [true, false] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
        command.args(["roadmap", "--task", "PRD-52", "--json"]);
        if all {
            command.arg("--all");
        }
        command
            .env("LF_HOME", home.path())
            .env_remove("LF_CAPTURE_KEY")
            .env("LF_WAVE_ID", original.id().to_string())
            .current_dir(&other_repo);
        prepend_test_bin(&mut command, home.path());
        let output = command.output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let waves = result["waves"].as_array().unwrap();
        assert_eq!(waves.len(), if all { 2 } else { 1 });
        let retained = waves
            .iter()
            .find(|row| row["wave"]["id"] == other.id().to_string())
            .unwrap();
        assert_eq!(retained["tasks"]["items"][0]["task"]["id"], "other-task");
        assert_eq!(retained["tasks"]["items"][0]["task"]["completed"], true);
    }
}

#[test]
fn due_follow_up_is_visible_before_checkout_and_requires_a_confirmed_source() {
    let home = tempfile::tempdir().unwrap();
    seed_stale_project_work(home.path(), false, false);
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let source_id = TaskId::parse(PERSISTED_TASK_ID).unwrap();
    let source = store.task(&source_id).unwrap().unwrap();
    let mut planning = store.pm_snapshot(&source.wave_id).unwrap().unwrap();
    let mut child = planning.snapshot.items[0].clone();
    child.id = "follow-up-issue".into();
    child.identifier = "W2-FOLLOW".into();
    child.name = "Verify the installed command".into();
    child.due_date = Some("2026-10-08".into());
    child.completed = false;
    child.state = Some("unstarted".into());
    child.completed_at = None;
    planning.snapshot.items.push(child.clone());
    planning.synced_at += 1;
    store.put_pm_snapshot(&planning).unwrap();
    let intent = loopflow::work::task::follow_through::FollowThroughIntent {
        key: "installed-check".into(),
        issue_id: child.id.clone(),
        relation_id: uuid::Uuid::new_v4().to_string(),
        project_id: child.project_id.clone().unwrap(),
        team_id: child.team_id.clone().unwrap(),
        state_id: None,
        wave: "product".into(),
        title: child.name.clone(),
        notes: "Check the installed release after deployment".into(),
        due: Some("2026-10-09".into()),
        existing: false,
    };
    store.reserve_follow_through(&source_id, &intent).unwrap();
    let pending = status_json(home.path(), &["product"], None);
    let pending_child = pending["tasks"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["task"]["id"] == child.id)
        .unwrap();
    assert_eq!(
        pending_child["task"]["follow_up_sources"],
        serde_json::json!([])
    );
    let link = loopflow::work::task::follow_through::FollowThroughLink {
        key: intent.key,
        issue_id: child.id.clone(),
        identifier: child.identifier,
        url: child.url,
        due: intent.due,
    };
    store.link_follow_through(&source_id, &link).unwrap();
    store.link_follow_through(&source_id, &link).unwrap();
    let status = status_json(home.path(), &["product"], None);
    let row = status["tasks"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["task"]["id"] == child.id)
        .unwrap();
    assert_eq!(row["runtime"]["started"], false);
    assert!(row["reference"]["workspace"].is_null());
    assert!(row["pr"].is_null());
    assert_eq!(
        row["task"]["due_date"], "2026-10-08",
        "current planning owns the date, not the original filing"
    );
    assert_eq!(row["task"]["completed"], false);
    assert_eq!(
        row["task"]["follow_up_sources"],
        serde_json::json!([
            {"issue_id": source.plan.linear_id.as_ref().unwrap().as_str(), "identifier": source.plan.identifier}
        ])
    );
    assert_eq!(row["follow_through"]["reason"], serde_json::Value::Null);
    let roadmap = roadmap_json(home.path(), "product");
    let roadmap_child = roadmap["waves"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|wave| wave["tasks"]["items"].as_array().into_iter().flatten())
        .find(|row| row["task"]["id"] == child.id)
        .unwrap();
    assert_eq!(roadmap_child["task"], row["task"]);
}
