//! One real Task/Graph population, with provider side effects isolated in fixtures.
use crate::test_linear_input::SeedLinearSqlite;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use axum::{extract::State, routing::post, Json, Router};
use serde_json::{json, Value};

use crate::id::WaveId;
use crate::lf::commands::work_watch::{WorkContent, WorkFrame};
use crate::linear::{LinearIssueId, LinearProjectId};
use crate::ops::linear::{LinearRefresh, LinearTestContext, LINEAR_TEST_CONTEXT};
use crate::store::{open_ephemeral_store, CredentialType, ProviderToken, StorageConfig};
use crate::work::project::{Project, ProjectId};
use crate::work::task::{
    GithubPr, LinearWritebackState, Observation, PrPublication, Task, TaskId, TaskPr, TaskPrId,
};
use crate::work::wave::Wave;

struct Running {
    child: Child,
    release: Option<PathBuf>,
}
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(release) = &self.release {
            let _ = fs::write(release, "");
            let deadline = Instant::now() + Duration::from_secs(5);
            while self.child.try_wait().ok().flatten().is_none() && Instant::now() < deadline {
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn command(binary: &Path, repo: &Path, home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(binary);
    command
        .args(args)
        .current_dir(repo)
        .env("LF_HOME", home)
        .env("LF_BIN", binary)
        .env("HTTPS_PROXY", "http://127.0.0.1:1")
        .env("HTTP_PROXY", "http://127.0.0.1:1");
    command
}

fn read(binary: &Path, repo: &Path, home: &Path, args: &[&str]) -> Value {
    let output = command(binary, repo, home, args).output().unwrap();
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn task_row(roadmap: &Value) -> Value {
    roadmap["tasks"]["items"]
        .as_array()
        .unwrap_or_else(|| panic!("Task plan unavailable: {roadmap}"))
        .iter()
        .find(|task| task["task"]["identifier"] == "FIX-1")
        .unwrap_or_else(|| panic!("source Task missing: {roadmap}"))
        .clone()
}

fn capture(binary: &Path, repo: &Path, home: &Path) -> Value {
    let status = read(binary, repo, home, &["task", "status", "FIX-1", "--json"]);
    let roadmap = read(binary, repo, home, &["wave", "show", "--all", "--json"]);
    let row = task_row(&roadmap);
    let mut monitor = Running {
        release: None,
        child: command(
            binary,
            repo,
            home,
            &["monitor", "work", "--watch", "--json"],
        )
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap(),
    };
    let stdout = monitor.child.stdout.take().unwrap();
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            if send
                .send(serde_json::from_str::<WorkFrame>(&line.unwrap()).unwrap())
                .is_err()
            {
                break;
            }
        }
    });
    writeln!(
        monitor.child.stdin.as_mut().unwrap(),
        "{}",
        json!({"action": "scope", "id": 1, "repo": repo, "headless": true, "task": "FIX-1", "wave": null, "activity": null})
    )
    .unwrap();
    let deadline = Instant::now() + Duration::from_secs(30);
    let mut planning = None;
    let mut task = None;
    while planning.is_none() || task.is_none() {
        let frame = receive
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .unwrap();
        assert!(frame.unavailable.is_none(), "{frame:?}");
        match &frame.content {
            WorkContent::Planning(Some(part)) => {
                let projected = task_row(&serde_json::to_value(&part.roadmap).unwrap());
                assert_eq!(projected["runtime"]["status"], row["runtime"]["status"]);
                planning = Some(frame);
            }
            WorkContent::Task(Some(part)) => {
                let work = serde_json::to_value(&part.work).unwrap();
                assert_eq!(work["graph"], status["execution"]["work"]["graph"]);
                task = Some(frame);
            }
            _ => {}
        }
    }
    json!({"status": status, "row": row, "planning_frame": planning, "task_frame": task})
}

// Compare behavior, retaining generated IDs, clock readings and paths as evidence
// in the capture without making those volatile values golden expectations.
fn contract(population: &Value) -> Value {
    let mut phases = serde_json::Map::new();
    for phase in ["merged", "completed", "arrived"] {
        let capture = &population[phase];
        let execution = &capture["status"]["execution"];
        let graph = &execution["work"]["graph"];
        let graphs: Vec<_> = capture["task_frame"]["body"]["graph_processes"].as_array().unwrap().iter()
            .map(|graph| json!({"state":graph["entry"]["state"], "steps":graph["steps"].as_array().unwrap().iter()
                .map(|step| step["outcome"].clone()).collect::<Vec<_>>()})).collect();
        phases.insert(phase.into(), json!({
            "status": execution["status"], "pr": capture["row"]["pr"]["phase"],
            "position": [graph["position"]["kind"],graph["position"]["step"],graph["position"]["running"]],
            "graphs": graphs,
        }));
    }
    Value::Object(phases)
}

#[test]
fn ordinary_task_comments_and_completion_preserve_live_graph_and_arrival() {
    let _lock = crate::journal::test_env_lock();
    let path = std::env::var_os("PATH").unwrap();
    let mut keys: Vec<_> = std::env::vars_os()
        .filter_map(|(key, _)| key.into_string().ok())
        .filter(|key| key.starts_with("LF_"))
        .collect();
    keys.extend(["PATH", "LF_HOME", "LF_BIN"].map(str::to_owned));
    keys.sort();
    keys.dedup();
    let _environment =
        crate::test_ambient::EnvGuard::clear(&keys.iter().map(String::as_str).collect::<Vec<_>>());
    std::env::set_var("PATH", &path);
    let binary = PathBuf::from(loopflow_test_support::lf_binary());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let (home, store, repo, mut source, wave) = fixture(&runtime);
    std::env::set_var("LF_HOME", home.path());
    std::env::set_var("LF_BIN", &binary);
    let bin = home.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let gh = bin.join("gh");
    fs::write(&gh, r#"#!/bin/sh
if [ "$1" = --version ]; then
  echo "gh version fixture"
elif [ "$1" = api ]; then
  head=$(git rev-parse HEAD)
  printf '{"merged":true,"state":"closed","draft":false,"merge_commit_sha":"%s","merged_at":"2026-10-08T00:00:00Z","number":1,"html_url":"https://github.com/loopflowstudio/fixture/pull/1","head":{"sha":"%s"}}\n' "$head" "$head"
else
  exit 1
fi
"#).unwrap();
    fs::set_permissions(&gh, fs::Permissions::from_mode(0o755)).unwrap();
    let mut paths = vec![bin];
    paths.extend(std::env::split_paths(&path));
    std::env::set_var("PATH", std::env::join_paths(paths).unwrap());
    // The delivery run holds in an ordinary owned execution until released.
    let hold = home.path().join("hold.py");
    fs::write(
        &hold,
        r#"import os, time
from pathlib import Path
home = Path(os.environ['LF_HOME'])
(home / 'ready').touch()
while not (home / 'release').exists():
    time.sleep(.02)
"#,
    )
    .unwrap();
    repo.create_file(
        ".lf/graphs/delivery.yaml",
        &format!(
            "- cmd: session serve -- python3 {}\n- cmd: task complete FIX-1\n",
            hold.display()
        ),
    );
    let provider = Arc::new(Mutex::new(Linear::default()));
    let (url, server) = runtime.block_on(async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let app = Router::new()
            .route("/", post(respond))
            .with_state(provider.clone());
        (
            url,
            tokio::spawn(async move { axum::serve(listener, app).await.unwrap() }),
        )
    });
    LINEAR_TEST_CONTEXT.sync_scope(LinearTestContext {
        path: home.path().join("loopflow.db"), store: store.clone(), graphql_url: url,
    }, || {
        let planning = runtime.block_on(crate::ops::linear::test_fixture::refresh_task_input(repo.path(), "FIX-1", LinearRefresh::Force)).unwrap();
        store.sqlite.seed_inventory(&crate::test_linear_input::InventoryFixture {
            wave_id: wave.id().clone(), provider: "linear".into(), initiative: "initiative-1".into(),
            synced_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            snapshot: serde_json::from_value(json!({"projects": [planning.project], "items": [planning.item]})).unwrap(),
        }).unwrap();
        let log_path = home.path().join("graph.log");
        let log = fs::File::create(&log_path).unwrap();
        let mut graph = Running { release: Some(home.path().join("release")), child: command(&binary, repo.path(), home.path(), &["-b", "task", "run", "FIX-1", "delivery"])
            .stdout(log.try_clone().unwrap()).stderr(log).spawn().unwrap() };
        let deadline = Instant::now() + Duration::from_secs(30);
        while !home.path().join("ready").exists() {
            assert!(graph.child.try_wait().unwrap().is_none() && Instant::now() < deadline, "{}", fs::read_to_string(&log_path).unwrap());
            std::thread::sleep(Duration::from_millis(20));
        }
        // Authoritative simulated GitHub observation precedes any filing.
        runtime.block_on(crate::ops::task::reconcile_delivered_task(&store, &mut source)).unwrap();
        let merged = capture(&binary, repo.path(), home.path());
        assert_eq!(merged["status"]["execution"]["work"]["graph"]["position"]["running"], true);
        assert_ne!(merged["status"]["execution"]["status"], "done");
        // Exercise buffered filing while the selected planning transport is offline.
        provider.lock().unwrap().unavailable = true;
        let child = crate::ops::task::task_create(repo.path(), Some(wave.slug()),
            Some("Verify installed release".into()), Some("Run the released command; retain failures.".into())).unwrap();
        let thread = crate::ops::task::task_comment(repo.path(), "FIX-1", None,
            Some(&format!("Installed verification continues in {}.", child.identifier)), false).unwrap();
        assert!(thread.comments.iter().any(|comment| comment.body.contains(&child.identifier)));
        crate::ops::task::task_complete(repo.path(), "FIX-1", None).unwrap();
        let completed = capture(&binary, repo.path(), home.path());
        assert_eq!(completed["status"]["execution"]["status"], "done");
        assert_eq!(completed["status"]["execution"]["work"]["graph"], merged["status"]["execution"]["work"]["graph"]);
        assert!(graph.child.try_wait().unwrap().is_none());
        assert!(repo.path().exists());
        fs::write(home.path().join("release"), "").unwrap();
        assert!(graph.child.wait().unwrap().success(), "{}", fs::read_to_string(&log_path).unwrap());
        let arrived = capture(&binary, repo.path(), home.path());
        assert_eq!(arrived["status"]["execution"]["work"]["graph"]["position"], json!({"kind":"at", "step":"end"}));
        let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        let completions: i64 = db.query_row("SELECT count(*) FROM task_events WHERE json_extract(kind_json,'$.kind')='completed'", [], |row| row.get(0)).unwrap();
        assert_eq!(completions, 1);
        let graphs: i64 = db.query_row("SELECT count(*) FROM graph_processes g JOIN processes p ON p.id=g.lf_process_id WHERE p.outcome='succeeded'", [], |row| row.get(0)).unwrap();
        assert_eq!(graphs, 1);
        assert!(child.worktree.is_none());
        assert!(child.linear_id.is_none());
        assert_ne!(store.sqlite.task_state(&child.id).unwrap(), crate::durable::TaskState::Done);
        // Completion and filing are saved locally; provider delivery has its own receipt.
        assert!(provider.lock().unwrap().unavailable);
        let population = json!({"merged":merged,"completed":completed,"arrived":arrived});
        if std::env::var_os("LOOPFLOW_UPDATE_LIFECYCLE_FIXTURE").is_some() {
            let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/dto/task_lifecycle.json");
            fs::write(path, serde_json::to_string_pretty(&population).unwrap() + "\n").unwrap();
        } else {
            let frozen: Value = serde_json::from_str(include_str!("../../../../../tests/fixtures/dto/task_lifecycle.json")).unwrap();
            assert_eq!(contract(&population), contract(&frozen));
        }
    });
    server.abort();
}

#[derive(Default)]
struct Linear {
    unavailable: bool,
}

async fn respond(
    State(state): State<Arc<Mutex<Linear>>>,
    Json(request): Json<Value>,
) -> Json<Value> {
    if state.lock().unwrap().unavailable {
        return Json(json!({"errors": [{"message": "provider unavailable"}]}));
    }
    let query = request["query"].as_str().unwrap();
    let data = if query.contains("ListTeams") {
        json!({"teams": {"nodes": [{"id": "team-1", "name": "Fixture", "key": "FIX",
            "description": "<!-- loopflow-repository: loopflowstudio/fixture -->"}]}})
    } else if query.contains("IssueComments") {
        json!({"issue": {"comments": {"nodes": [], "pageInfo": {"hasNextPage": false, "endCursor": null}}}})
    } else if query.contains("IssueOwnership") {
        json!({"issue": {"id":"source-issue", "identifier":"FIX-1", "url":"https://linear.app/fixture/issue/FIX-1",
            "title":"Deliver the command", "description":"", "dueDate":null,
            "updatedAt":"2026-10-07T00:00:00Z", "completedAt":null, "branchName":null,
            "prioritySortOrder":1.0, "sortOrder":1.0, "assignee":null, "state":{"type":"unstarted"},
            "team":{"id":"team-1"}, "project":{"id":"project-1", "name":"Provider destination",
                "description":"", "content":"workflow: feature", "status":{"type":"started"},
                "updatedAt":"2026-10-07T00:00:00Z", "archivedAt":null,
                "initiatives":{"nodes":[{"id":"initiative-1"}]}, "teams":{"nodes":[{"id":"team-1"}]}}}})
    } else {
        panic!("unexpected fixture query: {query}");
    };
    Json(json!({"data":data}))
}

fn fixture(
    runtime: &tokio::runtime::Runtime,
) -> (
    tempfile::TempDir,
    crate::store::SharedStore,
    loopflow_test_support::TestRepo,
    Task,
    Wave,
) {
    let directory = tempfile::tempdir().unwrap();
    let database = directory.path().join("loopflow.db");
    let store = Arc::new(
        runtime
            .block_on(open_ephemeral_store(&StorageConfig::sqlite(
                database.clone(),
            )))
            .unwrap(),
    );
    let repo = loopflow_test_support::TestRepo::new();
    assert!(std::process::Command::new("git")
        .args([
            "remote",
            "set-url",
            "origin",
            "https://github.com/loopflowstudio/fixture.git"
        ])
        .current_dir(repo.path())
        .status()
        .unwrap()
        .success());
    std::fs::create_dir_all(repo.path().join(".lf")).unwrap();
    std::fs::write(
        repo.path().join(".lf/config.yaml"),
        "planning:\n  provider: linear\n  team: team-1\n",
    )
    .unwrap();
    std::fs::create_dir_all(repo.path().join(".lf/wave/product")).unwrap();
    std::fs::write(
        repo.path().join(".lf/wave/product/GOAL.md"),
        "---\nlinear:\n  initiative: initiative-1\n---\nProduct\n",
    )
    .unwrap();
    repo.create_file(".lf/graphs/delivery.yaml", "- cmd: sync --plan\n");
    let now = time::OffsetDateTime::now_utc();
    let wave = Wave::new(
        WaveId::new(),
        "product".into(),
        repo.path().canonicalize().unwrap().display().to_string(),
    );
    let project = Project {
        id: ProjectId::new(),

        linear_id: Some(LinearProjectId::new("project-1").unwrap()),
        summary: String::new(),
        slug: "chapter".into(),
        name: "Chapter".into(),
        status: crate::work::project::ProjectStatus::Started,
        description: String::new(),
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

        linear_id: Some(LinearIssueId::new("source-issue").unwrap()),
        revision: 0,
        identifier: "FIX-1".into(),
        title: "Deliver the command".into(),
        description: String::new(),
        pm_snapshot_synced_at: Some(now.unix_timestamp()),

        pm_writeback: LinearWritebackState::Current,
        wave_id: Some(wave.id().clone()),
        project_id: Some(project.id.clone()),
        worktree: Some(repo.path().canonicalize().unwrap()),
        workspace_slug: "delivery".into(),
        branch: "main".into(),
        base_commit: repo.head_sha(),
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
        branch: task.branch.clone(),
        base_commit: task.base_commit.clone(),
        parent_pr_id: None,
        publication: Some(PrPublication {
            requested_at: now,
            presentation: None,
            merge: None,
            github: Some(GithubPr {
                number: 1,
                url: "https://github.com/loopflowstudio/fixture/pull/1".into(),
                head_sha: Some(repo.head_sha()),
            }),
        }),
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
    runtime.block_on(async {
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
        store
            .upsert_provider_token(&ProviderToken {
                provider: "linear".into(),
                access_token: "fixture-token".into(),
                refresh_token: None,
                oauth_client_id: None,
                expires_at: Some(now.unix_timestamp() + 3600),
                login: None,
                updated_at: now.unix_timestamp(),
                credential_type: CredentialType::OAuth,
            })
            .await
            .unwrap();
    });
    (directory, store, repo, task, wave)
}
