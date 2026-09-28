//! Command observation is durable even when no agent work starts.
#![cfg(unix)]

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use loopflow::durable::RunId;
use loopflow::id::ExecId;
use loopflow::session::{Run, Session, SessionKind, TitleSource};
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::{open_ephemeral_store, StorageConfig};
use loopflow_test_support::TestRepo;

#[tokio::test]
async fn inspection_records_one_completed_exec_without_starting_work() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("LF_")
            || key.to_string_lossy().starts_with("LOOPFLOW_")
        {
            command.env_remove(key);
        }
    }
    let output = command
        .args(["session", "list", "--all", "--json"])
        .current_dir(home.path())
        .env("LF_HOME", home.path())
        .env("LF_DB_PATH", &database)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let connection = rusqlite::Connection::open(&database).unwrap();
    let (count, completed): (i64, i64) = connection
        .query_row(
            "SELECT count(*), count(completed_at) FROM execs WHERE outcome='succeeded'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("every parsed lf command has an Exec row");
    assert_eq!((count, completed), (1, 1));
    let work: i64 = connection
        .query_row("SELECT count(*) FROM runs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(work, 0, "inspection must not reserve agent or Task work");
}

#[tokio::test]
async fn command_failure_records_the_process_result() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let result = command(
        home.path(),
        home.path(),
        &["session", "rename", "missing", "Name"],
    )
    .output()
    .unwrap();
    assert_eq!(result.status.code(), Some(1));
    let conn = rusqlite::Connection::open(database).unwrap();
    let row: (String, i32) = conn
        .query_row("SELECT outcome,exit_code FROM execs", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(row, ("failed".into(), 1));
}

fn command(home: &Path, cwd: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("LF_")
            || key.to_string_lossy().starts_with("LOOPFLOW_")
        {
            command.env_remove(key);
        }
    }
    command
        .args(args)
        .current_dir(cwd)
        .env("LF_HOME", home)
        .env("LF_DB_PATH", home.join("loopflow.db"));
    command
}

fn wait_file(path: &Path, child: &mut Child) {
    let deadline = Instant::now() + Duration::from_secs(60);
    while !path.exists() {
        assert!(
            child.try_wait().unwrap().is_none(),
            "child exited before {}",
            path.display()
        );
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {}",
            path.display()
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

struct Driver {
    child: Child,
    id: ExecId,
    stop: std::path::PathBuf,
}

impl Driver {
    fn start(home: &Path, repo: &Path, name: &str) -> Self {
        let mut child = command(home, repo, &["__telemetry-scorecard"])
            .env("LF_TEST_DRIVER", name)
            .env("LF_TEST_BINARY", env!("CARGO_BIN_EXE_lf"))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let ready = home.join(format!("{name}.ready"));
        wait_file(&ready, &mut child);
        Self {
            child,
            id: ExecId::parse(std::fs::read_to_string(ready).unwrap().trim()).unwrap(),
            stop: home.join(format!("{name}.stop")),
        }
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        std::fs::write(&self.stop, "").unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.child.try_wait().unwrap().is_none() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        if self.child.try_wait().unwrap().is_none() {
            let _ = self.child.kill();
        }
        let _ = self.child.wait();
    }
}

fn write_scorecard(repo: &Path) {
    std::fs::create_dir(repo.join("scripts")).unwrap();
    std::fs::write(
        repo.join("scripts/lifecycle_scorecard.py"),
        r#"
import json, os, pathlib, subprocess, time
home = pathlib.Path(os.environ['LF_HOME'])
name = os.environ['LF_TEST_DRIVER']
subprocess.run([os.environ['LF_TEST_BINARY'], 'session', 'list', '--all', '--json'],
               check=True, stdout=subprocess.DEVNULL)
(home / (name + '.ready')).write_text(os.environ['LF_PROCESS_ID'])
while not (home / (name + '.stop')).exists():
    time.sleep(.02)
print(json.dumps({'report': {'ok': True}, 'metric_observations': [], 'text': ''}))
"#,
    )
    .unwrap();
}

#[tokio::test]
async fn interruption_records_the_exec_without_a_fabricated_signal_name() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    write_scorecard(repo.path());
    let mut driver = Driver::start(home.path(), repo.path(), "interrupt");
    // SAFETY: this PID is our still-owned child, retained until wait completes.
    assert_eq!(
        unsafe { libc::kill(driver.child.id() as i32, libc::SIGINT) },
        0
    );
    assert_eq!(driver.child.wait().unwrap().code(), Some(130));
    let conn = rusqlite::Connection::open(database).unwrap();
    let row: (String, i32, Option<String>) = conn
        .query_row(
            "SELECT outcome,exit_code,signal FROM execs WHERE id=?1",
            [driver.id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(row, ("interrupted".into(), 130, None));
}

#[tokio::test]
#[ignore = "requires actual Codex 0.157.1 and uv; only local synthetic Responses, no credentials"]
async fn actual_engine_children_follow_driver_handoff_but_not_provider_replacement() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let store = SqliteStore::new(&database).unwrap();
    write_scorecard(repo.path());
    let original = Driver::start(home.path(), repo.path(), "original");
    let original_id = original.id.clone();
    let replacement = Driver::start(home.path(), repo.path(), "replacement");
    let restart = Driver::start(home.path(), repo.path(), "restart");
    let run_id = RunId::new();
    let session_id = "engine-ownership-fixture";
    store
        .create_session(
            Session {
                id: session_id.into(),
                current_run_id: run_id.clone(),
                kind: SessionKind::Interactive,
                title: "Engine ownership".into(),
                title_source: TitleSource::Generated,
                request: None,
                ready_summary: None,
                completed_at: None,
                created_at: 1,
            },
            Run {
                id: run_id,
                session_id: Some(session_id.into()),
                invocation_id: None,
                node: None,
                iterations: None,
                attempt: None,
                task_id: None,
                wave_id: None,
                work_source: None,
                created_at: 1,
                published: false,
                cwd: repo.path().into(),
                skill: None,
                provider: Some("codex".into()),
                model: None,
                caller_run_id: None,
                ended: None,
            },
            None,
        )
        .unwrap();
    let first = store
        .claim_session_driver(session_id, None, &original.id, false)
        .unwrap();
    let control = home.path().join("control");
    std::fs::create_dir(&control).unwrap();
    std::fs::write(
        control.join("caller.json"),
        serde_json::to_vec(&first.caller(session_id.into())).unwrap(),
    )
    .unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let codex =
        std::env::var("CODEX_TEST_BIN").expect("set CODEX_TEST_BIN to the real Codex executable");
    let mut engine = Command::new("uv")
        .current_dir(&root)
        .arg("run")
        .arg("tests/e2e/codex_connect.py")
        .arg("--codex")
        .arg(codex)
        .arg("--output")
        .arg(control.join("evidence"))
        .arg("--lf")
        .arg(env!("CARGO_BIN_EXE_lf"))
        .arg("--lf-home")
        .arg(home.path())
        .arg("--control")
        .arg(&control)
        .spawn()
        .unwrap();
    wait_file(&control.join("before.done"), &mut engine);
    let vacant = store.release_session_driver(session_id, &first).unwrap();
    assert_eq!(vacant.exec_id, None);
    assert_eq!(vacant.provider_generation, first.provider_generation);
    assert_eq!(vacant.provider_exec_id, first.provider_exec_id);
    assert_eq!(
        store.session_driver(session_id).unwrap(),
        Some(vacant.clone())
    );
    let handed_off = store
        .claim_session_driver(session_id, Some(&vacant), &replacement.id, false)
        .unwrap();
    assert_eq!(handed_off.provider_generation, first.provider_generation);
    assert_ne!(handed_off.generation, first.generation);
    assert!(store
        .claim_session_driver(session_id, Some(&first), &original.id, false)
        .is_err());
    drop(original);
    std::fs::write(control.join("handoff.go"), "").unwrap();
    wait_file(&control.join("after.done"), &mut engine);
    let restarted = store
        .claim_session_driver(session_id, Some(&handed_off), &restart.id, true)
        .unwrap();
    assert_ne!(
        restarted.provider_generation,
        handed_off.provider_generation
    );
    std::fs::write(control.join("replace.go"), "").unwrap();
    assert!(engine.wait().unwrap().success());
    let conn = rusqlite::Connection::open(&database).unwrap();
    let parents: Vec<String> = conn
        .prepare("SELECT parent_exec_id FROM execs WHERE caller_session_id=?1 ORDER BY rowid")
        .unwrap()
        .query_map([session_id], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect();
    assert_eq!(
        parents,
        [
            original_id.to_string(),
            replacement.id.to_string(),
            original_id.to_string()
        ]
    );
    let direct_children: i64 = conn
        .query_row(
            "SELECT count(*) FROM execs WHERE parent_exec_id=?1 AND via_agent=0",
            [original_id.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(direct_children, 1);
}
