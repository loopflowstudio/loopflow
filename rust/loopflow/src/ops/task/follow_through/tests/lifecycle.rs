//! One real Task/Flow population, with provider side effects isolated in fixtures.
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use axum::{routing::post, Router};
use serde_json::{json, Value};

use super::{fixture, respond, Linear};
use crate::lf::commands::work_watch::{WorkContent, WorkFrame};
use crate::ops::pm::{PmRefresh, PmTestContext, PM_TEST_CONTEXT};
use crate::ops::task::follow_through::{task_follow_up, FollowUpOptions};

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

// Unit-only provider injection cannot cross a subprocess boundary. Build the
// ordinary CLI, then use it for execution and every observation of this store.
fn build_cli() -> PathBuf {
    let output = Command::new("cargo")
        .args([
            "build",
            "-p",
            "loopflow",
            "--bin",
            "lf",
            "--message-format=json",
        ])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .unwrap()
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find_map(|entry| {
            (entry["target"]["name"] == "lf")
                .then(|| entry["executable"].as_str().map(PathBuf::from))
                .flatten()
        })
        .expect("Cargo reports the CLI artifact")
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
    roadmap["waves"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|wave| {
            wave["tasks"]["items"]
                .as_array()
                .unwrap_or_else(|| panic!("Task plan unavailable: {wave}"))
        })
        .find(|task| task["task"]["identifier"] == "FIX-1")
        .unwrap_or_else(|| panic!("source Task missing: {roadmap}"))
        .clone()
}

fn capture(binary: &Path, repo: &Path, home: &Path) -> Value {
    let status = read(binary, repo, home, &["task", "status", "FIX-1", "--json"]);
    let roadmap = read(binary, repo, home, &["roadmap", "--all", "--json"]);
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
                assert_eq!(projected["follow_through"], row["follow_through"]);
                planning = Some(frame);
            }
            WorkContent::Task(Some(part)) => {
                let work = serde_json::to_value(&part.work).unwrap();
                assert_eq!(work["workflow"], status["execution"]["work"]["workflow"]);
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
        let workflow = &execution["work"]["workflow"];
        let links: Vec<_> = execution["follow_through"]["links"]
            .as_array()
            .unwrap()
            .iter()
            .map(|link| json!([link["identifier"], link["due"]]))
            .collect();
        let history: Vec<_> = workflow["history"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| json!([entry["kind"], entry["from"], entry["to"], entry["edge"]]))
            .collect();
        let flows: Vec<_> = capture["task_frame"]["body"]["flow_processes"].as_array().unwrap().iter()
            .map(|flow| json!({"state":flow["entry"]["state"], "steps":flow["steps"].as_array().unwrap().iter()
                .map(|step| step["outcome"].clone()).collect::<Vec<_>>()})).collect();
        phases.insert(phase.into(), json!({
            "status": execution["status"], "pr": capture["row"]["pr"]["phase"], "links":links,
            "position": [workflow["position"]["kind"],workflow["position"]["node"],workflow["position"]["running"]],
            "history": history, "flows": flows,
        }));
    }
    Value::Object(phases)
}

#[test]
fn merged_follow_up_completion_and_arrival_share_cli_monitor_and_desktop_evidence() {
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
    let binary = build_cli();
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
    for (path, contents) in [
        (
            ".lf/workflows/delivery.yaml",
            "edges:\n  - {from: start, to: end, flow: delivery}\n",
        ),
        (
            ".lf/flows/delivery.yaml",
            "- cmd: __telemetry-scorecard\n- cmd: task complete FIX-1\n",
        ),
        (
            "scripts/lifecycle_scorecard.py",
            r#"import json, os, time
from pathlib import Path
home = Path(os.environ['LF_HOME'])
(home / 'ready').touch()
while not (home / 'release').exists():
    time.sleep(.02)
print(json.dumps({'report': {'ok': True}, 'metric_observations': [], 'text': ''}))
"#,
        ),
    ] {
        repo.create_file(path, contents);
    }
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
    PM_TEST_CONTEXT.sync_scope(PmTestContext {
        path: home.path().join("loopflow.db"), store: store.clone(), graphql_url: url,
    }, || {
        let planning = crate::ops::task_pm::resolve_task(repo.path(), "FIX-1", PmRefresh::Force).unwrap();
        store.sqlite.put_pm_snapshot(&crate::store::PmSnapshotRow {
            wave_id: wave.id().clone(), provider: "linear".into(), initiative: "initiative-1".into(),
            synced_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            snapshot: serde_json::from_value(json!({"projects": [planning.project], "items": [planning.item]})).unwrap(),
        }).unwrap();
        let log_path = home.path().join("flow.log");
        let log = fs::File::create(&log_path).unwrap();
        let mut flow = Running { release: Some(home.path().join("release")), child: command(&binary, repo.path(), home.path(), &["-b", "task", "run", "FIX-1", "delivery"])
            .stdout(log.try_clone().unwrap()).stderr(log).spawn().unwrap() };
        let deadline = Instant::now() + Duration::from_secs(30);
        while !home.path().join("ready").exists() {
            assert!(flow.child.try_wait().unwrap().is_none() && Instant::now() < deadline, "{}", fs::read_to_string(&log_path).unwrap());
            std::thread::sleep(Duration::from_millis(20));
        }
        // Authoritative simulated GitHub observation precedes any filing.
        runtime.block_on(crate::ops::task::reconcile_delivered_task(&store, &mut source)).unwrap();
        let merged = capture(&binary, repo.path(), home.path());
        assert_eq!(merged["status"]["execution"]["work"]["workflow"]["position"]["running"], true);
        assert_ne!(merged["status"]["execution"]["status"], "done");
        assert!(crate::ops::task::task_complete(repo.path(), "FIX-1", None).is_err());
        task_follow_up(repo.path(), "FIX-1", &FollowUpOptions {
            key: Some("installed".into()), title: Some("Verify installed release".into()),
            notes: Some("Run the released command; retain failures.".into()), due: Some("2026-10-09".into()),
            ..Default::default()
        }).unwrap();
        task_follow_up(repo.path(), "FIX-1", &FollowUpOptions {
            finish: Some("Accepted installed check filed".into()), ..Default::default()
        }).unwrap();
        crate::ops::task::task_complete(repo.path(), "FIX-1", None).unwrap();
        let completed = capture(&binary, repo.path(), home.path());
        assert_eq!(completed["status"]["execution"]["status"], "done");
        assert_eq!(completed["status"]["execution"]["work"]["workflow"], merged["status"]["execution"]["work"]["workflow"]);
        assert!(flow.child.try_wait().unwrap().is_none());
        assert!(repo.path().exists());
        fs::write(home.path().join("release"), "").unwrap();
        assert!(flow.child.wait().unwrap().success(), "{}", fs::read_to_string(&log_path).unwrap());
        let arrived = capture(&binary, repo.path(), home.path());
        assert_eq!(arrived["status"]["execution"]["work"]["workflow"]["position"], json!({"kind":"node", "node":"end"}));
        let db = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        let completions: i64 = db.query_row("SELECT count(*) FROM task_events WHERE json_extract(kind_json,'$.kind')='completed'", [], |row| row.get(0)).unwrap();
        assert_eq!(completions, 1);
        let flows: i64 = db.query_row("SELECT count(*) FROM flow_processes f JOIN processes p ON p.lfid=f.process_lfid WHERE p.outcome='succeeded'", [], |row| row.get(0)).unwrap();
        assert_eq!(flows, 1);
        let linear = provider.lock().unwrap();
        assert!(linear.completed);
        assert_eq!(linear.issues.len(), 1);
        assert_eq!(linear.relations.len(), 1);
        assert_eq!(linear.issues.values().next().unwrap()["stateId"], "todo");
        let population = json!({"merged":merged,"completed":completed,"arrived":arrived});
        if std::env::var_os("LOOPFLOW_UPDATE_LIFECYCLE_FIXTURE").is_some() {
            let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/fixtures/dto/task_lifecycle.json");
            fs::write(path, serde_json::to_string_pretty(&population).unwrap() + "\n").unwrap();
        } else {
            let frozen: Value = serde_json::from_str(include_str!("../../../../../../../tests/fixtures/dto/task_lifecycle.json")).unwrap();
            assert_eq!(contract(&population), contract(&frozen));
        }
    });
    server.abort();
}
