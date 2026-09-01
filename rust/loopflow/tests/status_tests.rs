//! `lf status` is an audit surface, so its contract is user-facing: the JSON it
//! promises must be the JSON it emits, and the wave you are standing in must be
//! the wave it reports. Drives the real binary against a seeded `LF_HOME`.

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use loopflow::child::ChildRef;
use loopflow::controller::wave::metrics::{
    load_metric_contract, MetricObservation, ObservationAcceptance,
};
use loopflow::id::WaveId;
use loopflow::planning::{LinearIssueId, LinearProjectId, ProjectPlan, TaskPlan};
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::{open_store, PmSnapshotRow, StorageConfig};
use loopflow::work::project::{Project, ProjectEventKind, ProjectId};
use loopflow::work::task::{
    Observation, PmWritebackState, PrMergeMode, Task, TaskId, TaskPr, TaskPrId,
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
            id: LinearProjectId::new(format!("linear-{slug}")).expect("Linear Project id"),
            slug: slug.to_string(),
            name: slug.replace('-', " "),
            prompt_context: "Keep status truthful.".to_string(),
            pm_snapshot_synced_at: updated_at.unix_timestamp(),
        },
        wave_id: wave.id().clone(),
        abandon_intent: None,
        created_at: updated_at,
        updated_at,
    }
}

fn put_project_snapshot(home: &Path, wave: &Wave, project: &Project) {
    let payload = serde_json::json!({
        "projects": [{
            "id": project.plan.id.as_str(),
            "slug": project.plan.slug,
            "name": project.plan.name,
            "summary": "Keep status truthful.",
            "definition": "Historical failures never impersonate current state.",
            "flows": {"first": null, "loop": null, "finally": null},
            "krs": [{"text": "Current state and history stay distinct", "holds": false}],
            "initiative_ids": ["initiative-infrastructure"],
            "team_ids": ["team-infrastructure"]
        }],
        "items": []
    });
    SqliteStore::new(&home.join("loopflow.db"))
        .expect("open status store")
        .put_pm_snapshot(&PmSnapshotRow {
            wave_id: wave.id().clone(),
            provider: "linear".to_string(),
            initiative: "initiative-infrastructure".to_string(),
            synced_at: OffsetDateTime::now_utc().unix_timestamp(),
            payload: serde_json::to_string(&payload).expect("serialize PM snapshot"),
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

/// `lf status --json` in a clean environment, optionally standing inside a wave.
fn status_json(home: &Path, args: &[&str], ambient_wave_id: Option<&str>) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .arg("status")
        .args(args)
        .arg("--json")
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_TRACE_ID")
        .env_remove("LF_WAVE_ID")
        .current_dir(home.join("repo"));
    if let Some(id) = ambient_wave_id {
        command.env("LF_WAVE_ID", id);
    }
    prepend_test_bin(&mut command, home);
    let output = command.output().expect("lf status runs");
    assert!(
        output.status.success(),
        "lf status failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    serde_json::from_str(stdout.trim()).unwrap_or_else(|err| panic!("not JSON: {err}\n{stdout}"))
}

fn status_human(home: &Path, wave: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["status", wave])
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .current_dir(home.join("repo"))
        .output()
        .expect("lf status runs");
    assert!(
        output.status.success(),
        "lf status failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("status is utf8")
}

fn roadmap_json(home: &Path, wave: &str) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .args(["roadmap", "--wave", wave, "--json"])
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
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

fn seed_stale_project_work(home: &Path, abandon_stale_project: bool) {
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
            id: LinearProjectId::new(STALE_PROJECT_ID).expect("recorded PM Project id"),
            slug: "technical-architecture".to_string(),
            name: "Technical Architecture".to_string(),
            prompt_context: "Keep the system legible and minimally simple.".to_string(),
            pm_snapshot_synced_at: now.unix_timestamp() - 1,
        },
        wave_id: wave.id().clone(),
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    };
    store.insert_project(&stale).expect("seed stale Project");
    let stale_task = Task {
        id: TaskId::parse(STALE_TASK_WORK_ID).expect("recorded Task Work id"),
        plan: TaskPlan {
            id: LinearIssueId::new("linear-task-w2-127").expect("recorded PM Task id"),
            identifier: "W2-127".to_string(),
            title: "Preserve historical architecture evidence".to_string(),
            description: "This Task outlived its retired Linear Project.".to_string(),
            pm_snapshot_synced_at: now.unix_timestamp() - 1,
        },
        pm_writeback: PmWritebackState::Current,
        wave_id: wave.id().clone(),
        project_id: stale.id.clone(),
        worktree: home.join("repo.w2-127"),
        workspace_slug: "w2-127".to_string(),
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
    store
        .insert_task(&stale_task, &stale_pr)
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
            id: LinearProjectId::new("95159066-9098-4d0b-8903-01459dc7ec14")
                .expect("current PM Project id"),
            slug: "auditability".to_string(),
            name: "Auditability".to_string(),
            prompt_context: "Every claim points to its receipt.".to_string(),
            pm_snapshot_synced_at: now.unix_timestamp(),
        },
        wave_id: wave.id().clone(),
        abandon_intent: None,
        created_at: now,
        updated_at: now,
    };
    store
        .insert_project(&current)
        .expect("seed current Project");

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
                "definition": "Every product surface shows enough truth to trust the system.",
                "flows": {"first": null, "loop": null, "finally": null},
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
            payload: serde_json::to_string(&payload).expect("serialize PM snapshot"),
        })
        .expect("seed PM snapshot");
}

fn seed_persisted_merge_request_without_copy(home: &Path) {
    seed_stale_project_work(home, true);
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
        home.to_str().expect("fixture Home path is utf8"),
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
    drop(connection);

    let store = SqliteStore::new(&database).expect("migrate previous release store");
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
fn status_surfaces_keep_credential_failure_in_history() {
    let home = tempfile::tempdir().expect("tempdir");
    let project = seed_credential_history(home.path());

    let status = status_json(home.path(), &["infrastructure"], None);
    let runtime = &status["projects"][0]["runtime"];
    assert_eq!(runtime["status"], "ready");
    assert_eq!(runtime["reason"], "ready");
    assert!(runtime.get("current").is_none());
    assert_eq!(
        runtime["last_failure"]["message"],
        "project runner failed: credential is missing"
    );
    let human = status_human(home.path(), "infrastructure");
    let current_line = human
        .lines()
        .find(|line| line.contains(&project.plan.slug))
        .expect("Project current-state line");
    assert!(current_line.contains("ready"));
    assert!(!current_line.contains("credential"));
    assert!(human.contains("last failure at "));
    assert!(human.contains("project runner failed: credential is missing"));
}

/// The reproduced break: inside a resident wave, `LF_WAVE_ID` is a wave id, and
/// bare `lf status` read it as a name.
#[test]
fn ambient_wave_id_resolves_the_wave_it_names() {
    let home = tempfile::tempdir().expect("tempdir");
    let wave = seed(home.path(), "audit-b");

    let status = status_json(home.path(), &[], Some(wave.id().as_str()));

    assert_eq!(status["wave"]["id"], wave.id().as_str());
    assert_eq!(status["wave"]["name"], "audit-b");
    assert_eq!(status["runs"]["state"], "ok");
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
project_id: d19956b2-9955-437d-aea6-d91766231c77
stage: graduated
instrument: lifecycle-scorecard
unit: ratio
target:
  at_least: 1
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
            "definition": "Keep the API coherent across every surface.",
            "flows": {"first": null, "loop": null, "finally": null},
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
            payload: serde_json::to_string(&project_payload).expect("serialize PM snapshot"),
        })
        .expect("seed PM snapshot");
    drop(sqlite);
    install_metric_schema_if_draft(&home.path().join("loopflow.db"));

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
        let store = open_store(&StorageConfig::sqlite(home.path().join("loopflow.db")))
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
    assert_eq!(
        status_metric["project_id"],
        "d19956b2-9955-437d-aea6-d91766231c77"
    );
    assert_eq!(
        status_metric["target"],
        serde_json::json!({"kind": "at_least", "value": 1.0})
    );
    assert_eq!(status_metric["window"], "7d");
    assert_eq!(status_metric["freshness"]["kind"], "fresh");
    assert_eq!(status_metric["evidence"]["kind"], "met");
    assert_eq!(status_metric["evidence"]["value"], 1.0);
    assert_eq!(status["projects"][0]["project"]["krs"][0]["holds"], false);
    assert_eq!(
        roadmap["waves"][0]["metric_portfolio"],
        status["metric_portfolio"]
    );

    let human = status_human(home.path(), "product");
    assert!(human.contains("Loopflow API\n      Task loops earn trust  [met]"));
    assert!(
        human.contains("Value 100.00% · Target >= 100.00% over 7d"),
        "{human}"
    );
}

fn install_metric_schema_if_draft(database: &Path) {
    let connection = rusqlite::Connection::open(database).expect("open metric schema");
    let installed = connection
        .query_row(
            "SELECT EXISTS(
                SELECT 1 FROM sqlite_master
                WHERE type='table' AND name='metric_observations'
            )",
            [],
            |row| row.get::<_, bool>(0),
        )
        .expect("inspect metric schema");
    if installed {
        return;
    }

    let drafts = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/store/migrations/drafts");
    let migration = std::fs::read_dir(drafts)
        .expect("metric migration drafts")
        .map(|entry| entry.expect("metric migration draft").path())
        .find(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| {
                    name.starts_with("project_metric_observations__") && name.ends_with(".sql")
                })
        })
        .expect("project metric migration draft");
    let sql = std::fs::read_to_string(migration).expect("read metric migration draft");
    connection
        .execute_batch(&sql)
        .expect("install metric schema");
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
    assert_eq!(status["runs"]["state"], "ok");
    assert_eq!(status["runs"]["items"], serde_json::json!([]));
    assert_eq!(status["runs"]["truncated"], false);
}

#[test]
fn orphaned_task_work_preserves_status_and_roadmap_evidence() {
    let home = tempfile::tempdir().expect("tempdir");
    seed_stale_project_work(home.path(), true);

    let status = status_json(home.path(), &["product"], None);
    let status_projects = status["projects"].as_array().expect("status projects");
    assert_eq!(status_projects.len(), 1);
    assert_eq!(status_projects[0]["project"]["slug"], "auditability");
    assert_eq!(
        status_projects[0]["tasks"][0]["task"]["identifier"],
        "PRD-52"
    );
    assert_eq!(status["runs"]["state"], "ok");
    assert_eq!(status["runs"]["items"].as_array().expect("runs").len(), 0);
    let unavailable = status["unavailable_projects"]
        .as_array()
        .expect("status unavailable Projects");
    assert_eq!(unavailable.len(), 1);
    assert_eq!(
        unavailable[0]["work_id"],
        "proj_e972b70272fbb5e91c096ebe657f9f9b"
    );
    assert_eq!(
        unavailable[0]["project_id"],
        "f56c583c-c360-4dc4-ba12-4b5a02268623"
    );
    assert_eq!(unavailable[0]["project_slug"], "technical-architecture");
    assert_eq!(unavailable[0]["status"], "abandoned");
    assert_eq!(unavailable[0]["owner"], "wave");
    assert_eq!(
        unavailable[0]["reason"],
        "Project is absent from the current PM snapshot"
    );
    assert_eq!(
        unavailable[0]["recovery"],
        "Settle the listed Tasks; Project Work is already abandoned"
    );
    let orphaned_tasks = unavailable[0]["tasks"].as_array().expect("orphaned Tasks");
    assert_eq!(orphaned_tasks.len(), 1);
    assert_eq!(
        orphaned_tasks[0]["work_id"],
        "task_40fbeeaadfbca5367aa7391432ae84ff"
    );
    assert_eq!(orphaned_tasks[0]["task_id"], "linear-task-w2-127");
    assert_eq!(orphaned_tasks[0]["task_identifier"], "W2-127");
    assert_eq!(orphaned_tasks[0]["status"], "ready");
    assert_eq!(orphaned_tasks[0]["owner"], "wave");
    assert_eq!(
        orphaned_tasks[0]["reason"],
        "Task's owning Project is absent from the current PM snapshot"
    );
    assert_eq!(
        orphaned_tasks[0]["recovery"],
        "lf work abandon task task_40fbeeaadfbca5367aa7391432ae84ff --reason \"Project is absent from the current PM snapshot\""
    );

    let roadmap = roadmap_json(home.path(), "product");
    let wave = &roadmap["waves"][0];
    assert_eq!(wave["projects"]["state"], "ok");
    let roadmap_projects = wave["projects"]["items"]
        .as_array()
        .expect("roadmap projects");
    assert_eq!(roadmap_projects.len(), 1);
    assert_eq!(roadmap_projects[0]["project"]["slug"], "auditability");
    assert_eq!(
        roadmap_projects[0]["tasks"][0]["task"]["identifier"],
        "PRD-52"
    );
    assert_eq!(wave["unavailable_projects"], status["unavailable_projects"]);
}

#[test]
fn persisted_merge_request_without_copy_keeps_status_and_roadmap_readable() {
    let home = tempfile::tempdir().expect("tempdir");
    seed_persisted_merge_request_without_copy(home.path());

    let status = status_json(home.path(), &["product"], None);
    let status_task = &status["projects"][0]["tasks"][0];
    assert_eq!(status_task["task"]["identifier"], "PRD-52");
    assert_eq!(
        status_task["prs"][0]["publication"]["presentation"],
        serde_json::Value::Null
    );
    assert_eq!(
        status_task["prs"][0]["publication"]["merge"]["mode"],
        "user"
    );

    let roadmap = roadmap_json(home.path(), "product");
    let wave = &roadmap["waves"][0];
    assert_eq!(wave["projects"]["state"], "ok");
    let roadmap_task = &wave["projects"]["items"][0]["tasks"][0];
    assert_eq!(roadmap_task["task"]["identifier"], "PRD-52");
    assert_eq!(
        roadmap_task["active_pr"]["publication"]["presentation"],
        serde_json::Value::Null
    );
    assert_eq!(
        roadmap_task["active_pr"]["publication"]["merge"]["mode"],
        "user"
    );
}

#[test]
fn previous_release_merge_request_migrates_into_readable_status_and_roadmap() {
    let home = tempfile::tempdir().expect("tempdir");
    seed_previous_release_task_pr(home.path());

    let status = status_json(home.path(), &["product"], None);
    let status_task = &status["projects"][0]["tasks"][0];
    assert_eq!(status_task["task"]["identifier"], "PRD-52");
    assert_eq!(
        status_task["prs"][0]["publication"]["presentation"],
        serde_json::Value::Null
    );
    assert_eq!(
        status_task["prs"][0]["publication"]["merge"]["mode"],
        "user"
    );

    let roadmap = roadmap_json(home.path(), "product");
    let wave = &roadmap["waves"][0];
    assert_eq!(wave["projects"]["state"], "ok");
    let roadmap_task = &wave["projects"]["items"][0]["tasks"][0];
    assert_eq!(roadmap_task["task"]["identifier"], "PRD-52");
    assert_eq!(
        roadmap_task["active_pr"]["publication"]["presentation"],
        serde_json::Value::Null
    );
    assert_eq!(
        roadmap_task["active_pr"]["publication"]["merge"]["mode"],
        "user"
    );
}

#[test]
fn active_project_removed_from_planning_reports_truthful_recovery() {
    let home = tempfile::tempdir().expect("tempdir");
    seed_stale_project_work(home.path(), false);

    let status = status_json(home.path(), &["product"], None);
    let unavailable = status["unavailable_projects"]
        .as_array()
        .expect("status unavailable Projects");

    assert_eq!(unavailable.len(), 1);
    assert_eq!(unavailable[0]["project_slug"], "technical-architecture");
    assert_eq!(unavailable[0]["status"], "ready");
    assert_eq!(unavailable[0]["owner"], "wave");
    assert_eq!(
        unavailable[0]["reason"],
        "Project is absent from the current PM snapshot"
    );
    assert_eq!(
        unavailable[0]["recovery"],
        "lf project abandon technical-architecture --reason \"Project is absent from the current PM snapshot\""
    );
    assert_eq!(
        unavailable[0]["tasks"]
            .as_array()
            .expect("preserved Task evidence")
            .len(),
        1
    );
}
