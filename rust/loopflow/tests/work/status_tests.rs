//! `lf wave status` is an audit surface, so its contract is user-facing: the JSON it
//! promises must be the JSON it emits, and the wave you are standing in must be
//! the wave it reports. Drives the real binary against a seeded `LF_HOME`.

use crate::test_linear_input::{InventoryFixture, SeedLinearSqlite};
use crate::test_support::planning;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use loopflow_execution::linear::{LinearIssueId, LinearProjectId};
use loopflow_execution::store::migrations;
use loopflow_execution::work::project::{Project, ProjectId};
use loopflow_execution::work::task::{
    LinearWritebackState, Observation, PrMergeMode, Task, TaskId, TaskPr, TaskPrId,
};
use loopflow_execution::work::wave::Wave;
use loopflow_model::id::WaveId;
use time::OffsetDateTime;

const PREVIOUS_RELEASE_TASK_PR_FIXTURE: &str = include_str!("../fixtures/store_0_12_8_task_pr.sql");
const PERSISTED_TASK_ID: &str = "task_40fbeeaadfbca5367aa7391432ae84ff";

/// A machine home holding one Wave.
fn seed(home: &Path, wave_name: &str) -> Wave {
    std::fs::create_dir_all(home).expect("home");
    let repo = home.join("repo");
    std::fs::create_dir_all(&repo).expect("repo");
    let db = home.join("loopflow.db");
    let store = loopflow_execution::store::open_sqlite(&db).expect("open store");
    let wave = Wave::new(
        WaveId::new(),
        wave_name.to_string(),
        repo.canonicalize().unwrap().display().to_string(),
    );
    loopflow_execution::work::wave::bootstrap::upsert_wave(&store, &wave).expect("register wave");
    wave
}

fn test_project(wave: &Wave, slug: &str, updated_at: OffsetDateTime) -> Project {
    Project {
        id: ProjectId::for_wave(wave.id()),

        summary: String::new(),
        status: loopflow_execution::work::project::ProjectStatus::Started,
        tracker_id: Some(
            LinearProjectId::new(uuid::Uuid::new_v4().to_string()).expect("Linear Project id"),
        ),
        slug: slug.to_string(),
        name: slug.replace('-', " "),
        description: "Keep status truthful.".to_string(),
        tracker_synced_at: Some(updated_at.unix_timestamp()),

        wave_id: wave.id().clone(),
        created_at: updated_at,
        updated_at,
    }
}

fn start_only(home: &Path, wave: &Wave, project_id: &str) {
    let connection = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    connection.execute(
        "UPDATE projects SET status=CASE WHEN external_project_id=?2 THEN 'started' ELSE 'completed' END WHERE wave_id=?1",
        rusqlite::params![wave.id().as_str(), project_id],
    ).unwrap();
}

fn put_project_snapshot(home: &Path, wave: &Wave, project: &Project) {
    let payload = serde_json::json!({
        "project": {
            "id": project.tracker_id.as_ref().unwrap().as_str(),
            "slug": project.slug,
            "name": project.name,
            "summary": "Keep status truthful.",
            "status": "started",
            "sync": {"actions": []},
            "team_ids": ["team-infrastructure"]
        },
        "items": []
    });
    let store = loopflow_execution::store::open_sqlite(&home.join("loopflow.db"))
        .expect("open status store");
    start_only(home, wave, project.tracker_id.as_ref().unwrap().as_str());
    store
        .seed_inventory(&InventoryFixture {
            wave_id: wave.id().clone(),
            synced_at: OffsetDateTime::now_utc().unix_timestamp(),
            snapshot: serde_json::from_value(payload).expect("parse PM snapshot"),
        })
        .expect("seed PM snapshot");
}

fn seed_started_project(home: &Path) -> Project {
    let wave = seed(home, "infrastructure");
    let store = loopflow_execution::store::open_sqlite(&home.join("loopflow.db"))
        .expect("open status store");
    let now = OffsetDateTime::now_utc();
    let project = test_project(&wave, "stability-security", now);
    store.insert_project(&project).expect("seed Project");
    put_project_snapshot(home, &wave, &project);
    project
}

/// `lf wave status --json` in a clean environment, optionally standing inside a wave.
fn status_json(home: &Path, args: &[&str], ambient_wave_id: Option<&str>) -> serde_json::Value {
    let mut command = Command::new(loopflow_test_support::lf_binary());
    command
        .args(["desktop", "wave"])
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

/// A roadmap Wave holding its share of the roadmap's Tasks, as `wave status`
/// shows them; the roadmap writes each Task once.
fn roadmap_wave(roadmap: &serde_json::Value, wave: &serde_json::Value) -> serde_json::Value {
    let mut view = wave.clone();
    let mut tasks = roadmap["tasks"].clone();
    if let Some(items) = tasks["items"].as_array_mut() {
        items.retain(|task| task["task"]["wave_id"] == wave["wave"]["id"]);
    }
    view["tasks"] = tasks;
    view
}

fn roadmap_json(home: &Path, wave: &str) -> serde_json::Value {
    let mut command = Command::new(loopflow_test_support::lf_binary());
    command
        .args(["desktop", "plan", wave, "--json"])
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

fn seed_stale_project_work(home: &Path, current_task: bool) -> InventoryFixture {
    const STALE_TASK_WORK_ID: &str = "task_40fbeeaadfbca5367aa7391432ae84ff";

    let wave = seed(home, "product");
    let repo = home.join("repo");
    std::fs::create_dir_all(&repo).expect("repo");
    let store =
        loopflow_execution::store::open_sqlite(&home.join("loopflow.db")).expect("open store");
    let now = OffsetDateTime::now_utc();
    // The Wave's one Project holds both Tasks.
    let current = Project {
        id: ProjectId::for_wave(wave.id()),

        summary: String::new(),
        status: loopflow_execution::work::project::ProjectStatus::Started,
        tracker_id: Some(
            LinearProjectId::new("95159066-9098-4d0b-8903-01459dc7ec14")
                .expect("current PM Project id"),
        ),
        slug: "auditability".to_string(),
        name: "Auditability".to_string(),
        description: "Every claim points to its receipt.".to_string(),
        tracker_synced_at: Some(now.unix_timestamp()),

        wave_id: wave.id().clone(),
        created_at: now,
        updated_at: now,
    };
    store.insert_project(&current).expect("seed Project");
    start_only(home, &wave, current.tracker_id.as_ref().unwrap().as_str());
    let stale_task = Task {
        parent_task_id: None,
        repository_id: loopflow_execution::repository::location::ensure_repository(
            &store,
            wave.repo(),
        )
        .unwrap(),
        id: TaskId::parse(STALE_TASK_WORK_ID).expect("recorded Task Work id"),

        revision: 0,
        tracker_id: Some(
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
        tracker_synced_at: Some(now.unix_timestamp() - 1),

        tracker_writeback: LinearWritebackState::Current,
        wave_id: Some(wave.id().clone()),
        project_id: Some(current.id.clone()),
        worktree: Some(home.join("repo.w2-127")),
        workspace_slug: "w2-127".to_string(),
        branch: "jack-heart/w2-127".to_string(),
        base_commit: "deadbeef".to_string(),
        parent_pr_id: None,
        created_at: now,
        updated_at: now,
        rank: 0,
        assignee: None,
        due_date: None,
        state: None,
        completed_at: None,
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
    loopflow_execution::store::ownership::place_task(
        &store,
        &stale_task.id,
        stale_task.worktree.as_ref().unwrap(),
        &stale_task.workspace_slug,
        &stale_pr,
    )
    .expect("seed orphaned Task");

    let bin = home.join("bin");
    std::fs::create_dir_all(&bin).expect("test bin");
    let tmux = bin.join("tmux");
    std::fs::write(&tmux, "#!/bin/sh\nexit 0\n").expect("fake tmux");
    std::fs::set_permissions(&tmux, std::fs::Permissions::from_mode(0o755))
        .expect("make fake tmux executable");

    let payload = serde_json::json!({
        "project": {
            "id": "95159066-9098-4d0b-8903-01459dc7ec14",
            "slug": "auditability",
            "name": "Auditability",
            "summary": "Every claim points to its receipt.",
            "status": "started",
            "team_ids": ["team-product"]
        },
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
    let snapshot = InventoryFixture {
        wave_id: wave.id().clone(),
        synced_at: now.unix_timestamp(),
        snapshot: serde_json::from_value(payload).unwrap(),
    };
    store.seed_inventory(&snapshot).expect("seed PM snapshot");
    snapshot
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
    connection.execute(
        "INSERT INTO task_prs(id,task_id,sequence,slug,branch,base_commit,created_at,updated_at)
         SELECT 'pr-copy-fixture',tasks.id,1,workspace_slug,coalesce(c.branch,''),coalesce(c.base_commit,''),?2,?2
         FROM tasks LEFT JOIN task_checkouts c ON c.task_id=tasks.id WHERE tasks.id=?1",
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
    migrations::apply_sqlite(&connection).expect("apply migrations to historical fixture");
    drop(connection);

    let store = loopflow_execution::store::open_sqlite(&database)
        .expect("open migrated previous release store");
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
    start_only(home, &wave, "95159066-9098-4d0b-8903-01459dc7ec14");
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
        loopflow_execution::store::migrations::latest_known_version()
    );
    drop(connection);
}

#[test]
fn wave_status_and_roadmap_list_a_started_project_without_tasks() {
    let home = tempfile::tempdir().expect("tempdir");
    let project = seed_started_project(home.path());
    let status = status_json(home.path(), &["infrastructure"], None);
    let roadmap = roadmap_json(home.path(), "infrastructure");
    let expected_project = serde_json::json!({
        "state": "ok",
        "value": {
            "id": project.id.as_str(),
            "tracker_id": project.tracker_id.as_ref().unwrap().as_str(),
            "slug": project.slug,
            "name": project.name,
            "status": "started",
            "sync": {"actions": []},
            "url": null
        }
    });
    for view in [&status, &roadmap_wave(&roadmap, &roadmap["waves"][0])] {
        assert_eq!(view["project"], expected_project);
        assert_eq!(view["wave"]["status"], "ready");
        assert_eq!(view["tasks"]["state"], "ok");
        assert_eq!(view["tasks"]["items"], serde_json::json!([]));
    }
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
        let output = Command::new(loopflow_test_support::lf_binary())
            .args(["wave", "show", "--all", "--json"])
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
fn wave_list_keeps_abandoned_registrations_until_they_are_removed() {
    let home = tempfile::tempdir().unwrap();
    let current = seed(home.path(), "current");
    let abandoned = seed(home.path(), "accidental");
    let store = loopflow_execution::store::open_sqlite(&home.path().join("loopflow.db")).unwrap();
    let work = loopflow_execution::durable::WorkRef::Wave(abandoned.id().clone());
    store.abandon(&work, "accidental registration").unwrap();
    let run = |args: &[&str]| {
        Command::new(loopflow_test_support::lf_binary())
            .args(args)
            .env("LF_HOME", home.path())
            .env_remove("LF_WAVE_ID")
            .env_remove("LF_TRACE_ID")
            .current_dir(home.path().join("repo"))
            .output()
            .unwrap()
    };
    let listing = run(&["wave", "list", "--all", "--json"]);
    assert!(
        listing.status.success(),
        "{}",
        String::from_utf8_lossy(&listing.stderr)
    );
    let rows: serde_json::Value = serde_json::from_slice(&listing.stdout).unwrap();
    let mut ids: Vec<&str> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    ids.sort();
    let mut expected = vec![current.id().as_str(), abandoned.id().as_str()];
    expected.sort();
    assert_eq!(
        ids, expected,
        "a stray abandoned Wave stays listed for Remove Wave"
    );
    assert!(store.get_wave(abandoned.id()).unwrap().is_some());
}

#[test]
fn a_wave_with_no_runs_reports_an_empty_reading_not_a_missing_one() {
    let home = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(home.path()).expect("home");
    let database = home.path().join("loopflow.db");
    let store = loopflow_execution::store::open_sqlite(&database).expect("open store");
    std::fs::create_dir_all(home.path().join("repo")).expect("repo");
    let wave = Wave::new(
        WaveId::new(),
        "audit-c".to_string(),
        home.path().join("repo").display().to_string(),
    );
    loopflow_execution::work::wave::bootstrap::upsert_wave(&store, &wave).expect("register wave");

    let status = status_json(home.path(), &["audit-c"], None);

    assert_eq!(status["wave"]["status"], "ready");
    assert_eq!(status["history"]["state"], "ok");
    assert_eq!(status["history"]["items"], serde_json::json!([]));
    assert_eq!(status["history"]["truncated"], false);
}

#[test]
fn orphaned_task_work_preserves_status_and_roadmap_evidence() {
    {
        let home = tempfile::tempdir().expect("tempdir");
        seed_stale_project_work(home.path(), false);
        let status = status_json(home.path(), &["product"], None);
        let roadmap = roadmap_json(home.path(), "product");
        let wave = &roadmap_wave(&roadmap, &roadmap["waves"][0]);
        for view in [&status, wave] {
            assert_eq!(view["project"]["state"], "ok");
            let project = &view["project"]["value"];
            assert_eq!(
                project["tracker_id"],
                "95159066-9098-4d0b-8903-01459dc7ec14"
            );
            assert_eq!(project["slug"], "auditability");
            assert_eq!(project["status"], "started");
            assert!(project.get("graph").is_none());
            assert_eq!(view["tasks"]["state"], "ok");
            let tasks = view["tasks"]["items"].as_array().unwrap();
            assert_eq!(tasks.len(), 2);
            assert!(tasks
                .iter()
                .any(|task| task["task"]["identifier"] == "PRD-52"));
            assert!(tasks
                .iter()
                .any(|task| task["task"]["identifier"] == "W2-127"));
        }
        assert_eq!(wave["project"], status["project"]);
    }
}

#[test]
fn provider_inventory_loss_preserves_saved_planning_in_both_views() {
    for payload in [None, Some("{}"), Some("not-json")] {
        let home = tempfile::tempdir().unwrap();
        seed_stale_project_work(home.path(), false);
        let before = status_json(home.path(), &["product"], None);
        let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        if let Some(payload) = payload {
            conn.execute("UPDATE tracker_projects SET team_ids=?1", [payload])
                .unwrap();
        } else {
            conn.execute("DELETE FROM tracker_wave_sync", []).unwrap();
        }
        let status = status_json(home.path(), &["product"], None);
        let roadmap = roadmap_json(home.path(), "product");
        for view in [&status, &roadmap_wave(&roadmap, &roadmap["waves"][0])] {
            assert_eq!(view["project"], before["project"]);
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
                let mut content = task["task"].clone();
                let mut previous_content = previous["task"].clone();
                content.as_object_mut().unwrap().remove("tracker_state");
                previous_content
                    .as_object_mut()
                    .unwrap()
                    .remove("tracker_state");
                assert_eq!(content, previous_content);
                if payload.is_some() && task["task"]["tracker_id"] == "task-prd-52" {
                    assert_eq!(
                        task["task"]["tracker_state"],
                        serde_json::json!({
                            "state": "invalid", "synced_at": previous["task"]["tracker_state"]["synced_at"]
                        })
                    );
                } else {
                    assert_eq!(
                        task["task"]["tracker_state"],
                        previous["task"]["tracker_state"]
                    );
                }
                assert_eq!(task["runtime"], previous["runtime"]);
                assert_eq!(task["reference"], previous["reference"]);
            }
        }
    }
}

#[test]
fn reserved_session_does_not_claim_task_in_status_and_roadmap() {
    let home = tempfile::tempdir().unwrap();
    seed_persisted_merge_request_without_copy(home.path());
    let connection = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    connection.execute_batch("PRAGMA foreign_keys=ON").unwrap();
    connection
        .execute("DELETE FROM task_prs WHERE task_id=?1", [PERSISTED_TASK_ID])
        .unwrap();

    let mut owners = Vec::new();
    for assigned in [false, true] {
        if assigned {
            connection
                .execute(
                    "INSERT INTO agent_sessions(id,title,title_source,task_id,wave_id,created_at,cwd,input_published)
                     SELECT ?1,'Reserved work','generated',t.id,p.wave_id,1,c.worktree,0
                     FROM tasks t JOIN projects p ON p.id=t.project_id
                     LEFT JOIN task_checkouts c ON c.task_id=t.id WHERE t.id=?2",
                    rusqlite::params![uuid::Uuid::new_v4().simple().to_string().as_str(), PERSISTED_TASK_ID],
                )
                .unwrap();
            // Admission alone does not start a Task; reserving its first input does.
            let session: String = connection
                .query_row(
                    "SELECT id FROM agent_sessions WHERE task_id=?1",
                    [PERSISTED_TASK_ID],
                    |row| row.get(0),
                )
                .unwrap();
            connection
                .execute(
                    "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES(?1,'captured',?2,1,'{}')",
                    rusqlite::params![session, uuid::Uuid::new_v4().simple().to_string()],
                )
                .unwrap();
            connection
                .execute(
                    "UPDATE agent_sessions SET current_capture=?2 WHERE id=?1",
                    rusqlite::params![session, connection.last_insert_rowid()],
                )
                .unwrap();
        }
        let status = status_json(home.path(), &["product"], None);
        let roadmap = roadmap_json(home.path(), "product");
        for task in [&status["tasks"]["items"][0], &roadmap["tasks"]["items"][0]] {
            assert_eq!(task["task"]["identifier"], "PRD-52");
            owners.push(task["runtime"]["owner"].clone());
        }
    }
    // A bound conversation is not a claim; it leaves the recorded owner as it was.
    assert!(
        owners.windows(2).all(|pair| pair[0] == pair[1]),
        "{owners:?}"
    );
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
            connection.execute("DELETE FROM tracker_items", []).unwrap();
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
        assert_eq!(roadmap["tasks"]["state"], "ok");
        let roadmap_task = &roadmap["tasks"]["items"][0];
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
    assert_eq!(roadmap["tasks"]["state"], "ok");
    let roadmap_task = &roadmap["tasks"]["items"][0];
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
    seed_stale_project_work(home.path(), false);
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    conn.execute("DELETE FROM task_prs", []).unwrap();
    let before: (i64, i64, i64) = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM task_events), (SELECT COUNT(*) FROM agent_sessions), (SELECT COUNT(*) FROM graph_processes)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    for identifier in ["W2-127", "PRD-52", "not-a-task"] {
        let mut command = Command::new(loopflow_test_support::lf_binary());
        command
            .args(["desktop", "plan", "--task", identifier, "--all", "--json"])
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
        let tasks = result["tasks"]["items"].as_array().unwrap();
        assert_eq!(tasks.len(), 1);
        assert_eq!(tasks[0]["task"]["identifier"], identifier);
        assert!(tasks[0]["pr"].is_null());
        if identifier == "W2-127" {
            assert_eq!(tasks[0]["runtime"]["work_id"], PERSISTED_TASK_ID);
            // The seeded checkout claimed the Task; reading the roadmap leaves it owned.
            assert!(tasks[0]["runtime"]["owner"].is_object());
            assert_eq!(
                tasks[0]["task"]["name"],
                "Preserve historical architecture evidence"
            );
        }
    }
    let after: (i64, i64, i64) = conn
        .query_row(
            "SELECT (SELECT COUNT(*) FROM task_events), (SELECT COUNT(*) FROM agent_sessions), (SELECT COUNT(*) FROM graph_processes)",
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
    let mut snapshot = seed_stale_project_work(home.path(), false);
    let store = loopflow_execution::store::open_sqlite(&home.path().join("loopflow.db")).unwrap();
    let original = store.list_waves(None).unwrap().remove(0);
    let other_repo = home.path().join("other-repo");
    std::fs::create_dir_all(&other_repo).unwrap();
    // Registered repositories without Git metadata remain valid cached readers.
    let other = Wave::new(
        WaveId::new(),
        "other".into(),
        other_repo.canonicalize().unwrap().display().to_string(),
    );
    loopflow_execution::work::wave::bootstrap::upsert_wave(&store, &other).unwrap();
    snapshot.snapshot.project.id = "other-project".into();
    snapshot.snapshot.items[0].id = "other-task".into();
    snapshot.snapshot.items[0].project_id = Some("other-project".into());
    snapshot.snapshot.items[0].completed = true;
    snapshot.wave_id = other.id().clone();
    store.seed_inventory(&snapshot).unwrap();
    assert!(store
        .task_by_issue("PRD-52")
        .unwrap_err()
        .to_string()
        .contains("multiple stable Tasks"));

    for all in [true, false] {
        let mut command = Command::new(loopflow_test_support::lf_binary());
        command.args(["desktop", "plan", "--task", "PRD-52", "--json"]);
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
        let retained = roadmap_wave(
            &result,
            waves
                .iter()
                .find(|row| row["wave"]["id"] == other.id().to_string())
                .unwrap(),
        );
        let task = store.task_by_issue("other-task").unwrap().unwrap();
        assert_eq!(
            retained["tasks"]["items"][0]["task"]["id"],
            task.id.as_str()
        );
        assert_eq!(
            retained["tasks"]["items"][0]["task"]["tracker_id"],
            "other-task"
        );
        assert_eq!(retained["tasks"]["items"][0]["task"]["state"], "completed");
    }
}

#[test]
fn ordinary_task_due_date_is_visible_before_checkout() {
    let home = tempfile::tempdir().unwrap();
    let mut planning = seed_stale_project_work(home.path(), false);
    let store = loopflow_execution::store::open_sqlite(&home.path().join("loopflow.db")).unwrap();
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
    store.seed_inventory(&planning).unwrap();
    let status = status_json(home.path(), &["product"], None);
    let row = status["tasks"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["task"]["tracker_id"] == child.id)
        .unwrap();
    assert!(row["runtime"]["owner"].is_null());
    assert!(row["reference"]["workspace"].is_null());
    assert!(row["pr"].is_null());
    assert_eq!(
        row["task"]["due_date"], "2026-10-08",
        "current planning owns the date, not the original filing"
    );
    assert_eq!(row["task"]["state"], "unstarted");
    let roadmap = roadmap_json(home.path(), "product");
    let roadmap_child = roadmap["tasks"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["task"]["tracker_id"] == child.id)
        .unwrap();
    assert_eq!(roadmap_child["task"], row["task"]);
}
