//! Command observation is durable even when no agent work starts.
#![cfg(unix)]

use std::path::Path;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use loopflow::durable::RunId;
use loopflow::harness::codex_connection::CodexConnection;
use loopflow::id::ExecId;
use loopflow::session::{AgentSession, SessionKind, TitleSource};
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
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='runs'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(work, 0, "inspection must not reserve agent or Task work");
}

#[test]
fn obstructed_file_journal_preserves_command_start_and_completion_in_sql() {
    for append in [false, true] {
        let repo = TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let cwd = repo.path().join("nested");
        std::fs::create_dir(&cwd).unwrap();
        let trace = uuid::Uuid::new_v4().to_string();
        let root = repo.path().join(".lf/journal/runs");
        let obstruction = if append {
            let path = root.join(&trace).join("events.jsonl");
            std::fs::create_dir_all(&path).unwrap();
            path
        } else {
            std::fs::create_dir_all(root.parent().unwrap()).unwrap();
            std::fs::write(&root, "retained obstruction").unwrap();
            root
        };
        let output = command(home.path(), &cwd, &["session", "list", "--all", "--json"])
            .env("LF_TRACE_ID", &trace)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap(),
            serde_json::json!([])
        );
        assert_eq!(obstruction.is_dir(), append);
        if !append {
            assert_eq!(
                std::fs::read_to_string(&obstruction).unwrap(),
                "retained obstruction"
            );
        }
        let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
        let events: Vec<String> = conn
            .prepare("SELECT event FROM run_events WHERE node='run' ORDER BY seq")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(events, ["started", "completed"]);
        let facts: (i64, i64) = conn.query_row("SELECT (SELECT count(*) FROM execs WHERE outcome='succeeded'),(SELECT count(*) FROM sqlite_master WHERE type='table' AND name='runs')", [], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!(facts, (1, 0));
        let recorded_cwd: String = conn
            .query_row("SELECT cwd FROM execs", [], |row| row.get(0))
            .unwrap();
        assert_eq!(recorded_cwd, cwd.canonicalize().unwrap().to_string_lossy());
        assert!(
            !cwd.join(".lf").exists(),
            "file journal belongs at the checkout root"
        );
    }
}

#[tokio::test]
async fn command_failure_records_the_process_result() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let repo = TestRepo::new();
    for args in [
        vec!["session", "rename", "missing", "Name"],
        vec![
            "--wave",
            "fixture-wave-does-not-exist",
            "session",
            "list",
            "--all",
            "--json",
        ],
    ] {
        let result = command(home.path(), repo.path(), &args).output().unwrap();
        assert_eq!(result.status.code(), Some(1), "{result:?}");
    }
    std::fs::write(repo.path().join("change.txt"), "retained work").unwrap();
    let lock = repo.path().join(".git/index.lock");
    std::fs::write(&lock, "retained lock").unwrap();
    let result = command(home.path(), repo.path(), &["commit", "-m", "fixture"])
        .output()
        .unwrap();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    assert!(String::from_utf8_lossy(&result.stderr).contains("index.lock"));
    assert_eq!(std::fs::read_to_string(lock).unwrap(), "retained lock");
    let conn = rusqlite::Connection::open(database).unwrap();
    let row: (String, i32) = conn
        .query_row("SELECT outcome,exit_code FROM execs", [], |row| {
            Ok((row.get(0)?, row.get(1)?))
        })
        .unwrap();
    assert_eq!(row, ("failed".into(), 1));
    let failed: i64 = conn
        .query_row(
            "SELECT count(*) FROM execs WHERE outcome='failed' AND exit_code=1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(
        failed, 3,
        "resolution, dispatch and delivery failures retain their command outcomes"
    );
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
    output: std::path::PathBuf,
}

impl Driver {
    fn start(home: &Path, repo: &Path, name: &str, preload: Option<&Path>) -> Self {
        let output = home.join(format!("{name}.output"));
        let log = std::fs::File::create(&output).unwrap();
        let mut command = command(home, repo, &["__telemetry-scorecard"]);
        if let Some(preload) = preload {
            command.env("LD_PRELOAD", preload);
        }
        let mut child = command
            .env("LF_TEST_DRIVER", name)
            .env("LF_TEST_BINARY", env!("CARGO_BIN_EXE_lf"))
            .stdout(log.try_clone().unwrap())
            .stderr(log)
            .spawn()
            .unwrap();
        let ready = home.join(format!("{name}.ready"));
        wait_file(&ready, &mut child);
        Self {
            child,
            id: ExecId::parse(std::fs::read_to_string(ready).unwrap().trim()).unwrap(),
            stop: home.join(format!("{name}.stop")),
            output,
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
(home / (name + '.pid')).write_text(str(os.getpid()))
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
    // Hold the existing cleanup hook after killing its owned group. Without
    // exit coordination the awakened command returns 1 while Exec already says
    // interrupted/130. A passing ordinary timing alone does not cover this race.
    #[cfg(target_os = "linux")]
    let preload = {
        let source = home.path().join("hold_group_kill.c");
        let library = home.path().join("hold_group_kill.so");
        std::fs::write(&source, include_str!("support/hold_group_kill.c")).unwrap();
        let output = Command::new("cc")
            .args(["-shared", "-fPIC", "-o"])
            .arg(&library)
            .arg(&source)
            .arg("-ldl")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        Some(library)
    };
    #[cfg(not(target_os = "linux"))]
    let preload: Option<std::path::PathBuf> = None;
    let mut driver = Driver::start(home.path(), repo.path(), "interrupt", preload.as_deref());
    // SAFETY: this PID is our still-owned child, retained until wait completes.
    assert_eq!(
        unsafe { libc::kill(driver.child.id() as i32, libc::SIGINT) },
        0
    );
    let exit = driver.child.wait().unwrap();
    let conn = rusqlite::Connection::open(database).unwrap();
    let row: (String, i32, Option<String>) = conn
        .query_row(
            "SELECT outcome,exit_code,signal FROM execs WHERE id=?1",
            [driver.id.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        (exit.code(), row),
        (Some(130), ("interrupted".into(), 130, None)),
        "{}",
        std::fs::read_to_string(&driver.output).unwrap()
    );
    if preload.is_some() {
        assert!(std::fs::read_to_string(&driver.output)
            .unwrap()
            .contains("test ordering: owned group killed before interrupt hook returns"));
    }
    let scorecard_pid = std::fs::read_to_string(home.path().join("interrupt.pid")).unwrap();
    let script = repo
        .path()
        .canonicalize()
        .unwrap()
        .join("scripts/lifecycle_scorecard.py");
    let alive = || {
        let output = Command::new("ps")
            .args(["-p", scorecard_pid.trim(), "-o", "command="])
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout).contains(script.to_str().unwrap())
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    while alive() && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
    }
    let survived = alive();
    if survived {
        eprintln!(
            "owned scorecard survived interrupted lf: pid={} script={}",
            scorecard_pid.trim(),
            script.display()
        );
        // Keep fixture cleanup distinct from the interruption result. Release
        // only this script and observe exit before deleting its stop directory.
        std::fs::write(&driver.stop, "").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while alive() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(20));
        }
        assert!(!alive(), "owned scorecard failed fixture cleanup");
    }
    assert!(
        !survived,
        "interrupted command left its owned scorecard alive"
    );
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
    let original = Driver::start(home.path(), repo.path(), "original", None);
    let original_id = original.id.clone();
    let replacement = Driver::start(home.path(), repo.path(), "replacement", None);
    let restart = Driver::start(home.path(), repo.path(), "restart", None);
    let session_id = "engine-ownership-fixture";
    reserve_session(&store, session_id, repo.path());
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

fn reserve_session(store: &SqliteStore, session_id: &str, repo: &Path) {
    let run_id = RunId::new();
    store
        .create_session(
            AgentSession {
                task_id: None,
                wave_id: None,
                flow_session_id: None,
                work_source: None,
                bound_at: None,
                id: session_id.into(),
                input_id: run_id,
                caller_input_id: None,
                input_published: false,
                cwd: repo.into(),
                skill: None,
                provider: Some("codex".into()),
                model: None,
                node: None,
                iterations: None,
                kind: SessionKind::Conversation,
                interactive: true,
                repo: None,
                title: "Engine ownership".into(),
                title_source: TitleSource::Generated,
                request: None,
                ready_summary: None,
                completed_at: None,
                created_at: 1,
            },
            None,
            None,
        )
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "requires actual Codex and uv; private Homes and synthetic Responses only"]
async fn retained_native_client_loses_writes_but_keeps_display_after_transfer() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let store = SqliteStore::new(&database).unwrap();
    write_scorecard(repo.path());
    let original = Driver::start(home.path(), repo.path(), "original", None);
    let replacement = Driver::start(home.path(), repo.path(), "replacement", None);
    let session = "native-client-transfer";
    reserve_session(&store, session, repo.path());
    let first = store
        .claim_session_driver(session, None, &original.id, false)
        .unwrap();
    let control = home.path().join("gate");
    std::fs::create_dir(&control).unwrap();
    std::fs::write(
        control.join("caller.json"),
        serde_json::to_vec(&first.caller(session.into())).unwrap(),
    )
    .unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut probe = Command::new("uv")
        .current_dir(root)
        .args(["run", "tests/e2e/codex_connect.py", "--gated", "--codex"])
        .arg(std::env::var("CODEX_TEST_BIN").expect("set CODEX_TEST_BIN"))
        .arg("--output")
        .arg(control.join("evidence"))
        .arg("--control")
        .arg(&control)
        .arg("--lf")
        .arg(env!("CARGO_BIN_EXE_lf"))
        .arg("--lf-home")
        .arg(home.path())
        .spawn()
        .unwrap();
    wait_file(&control.join("native.done"), &mut probe);
    let engine: serde_json::Value =
        serde_json::from_slice(&std::fs::read(control.join("engine.json")).unwrap()).unwrap();
    let connection = CodexConnection {
        store: store.clone(),
        session_id: session.into(),
        thread_id: engine["thread"].as_str().unwrap().into(),
        driver: Some(first.clone()),
    };
    let endpoint = Path::new(engine["endpoint"].as_str().unwrap());
    let old = serve_gate(&control.join("old.sock"), endpoint, connection.clone());
    let mut passive = connection.clone();
    passive.driver = None;
    let observer = serve_gate(&control.join("observer.sock"), endpoint, passive);
    std::fs::write(control.join("sockets.go"), "").unwrap();
    wait_file(&control.join("attached.done"), &mut probe);
    assert_eq!(
        store.session_driver(session).unwrap(),
        Some(first.clone()),
        "passive display must not claim the conversation"
    );
    let second = store
        .claim_session_driver(session, Some(&first), &replacement.id, false)
        .unwrap();
    let mut current = connection;
    current.driver = Some(second.clone());
    let current = serve_gate(&control.join("current.sock"), endpoint, current);
    std::fs::write(control.join("transfer.go"), "").unwrap();
    assert!(probe.wait().unwrap().success());
    let result: serde_json::Value =
        serde_json::from_slice(&std::fs::read(control.join("evidence/results.json")).unwrap())
            .unwrap();
    assert_eq!(result["old_client_still_receives_completion"], true);
    assert_eq!(result["current_driver_continued"], true);
    assert_eq!(result["engine_alive"], true);
    assert_eq!(
        result["rejected_old_client_writes"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
    assert_eq!(store.session_driver(session).unwrap(), Some(second));
    let conn = rusqlite::Connection::open(database).unwrap();
    let parents: Vec<String> = conn.prepare(
        "SELECT parent_exec_id FROM execs WHERE caller_session_id=?1 AND via_agent=1 ORDER BY rowid"
    ).unwrap().query_map([session], |row| row.get(0)).unwrap().map(Result::unwrap).collect();
    assert_eq!(parents.first(), Some(&original.id.to_string()));
    assert!(
        parents.len() >= 3,
        "initial, continuing and subsequent turns must invoke lf"
    );
    assert!(
        parents[1..]
            .iter()
            .all(|parent| *parent == replacement.id.to_string()),
        "{parents:?}"
    );
    for task in [old, observer, current] {
        task.abort();
    }
}

fn serve_gate(
    socket: &Path,
    engine: &Path,
    connection: CodexConnection,
) -> tokio::task::JoinHandle<()> {
    let listener = tokio::net::UnixListener::bind(socket).unwrap();
    let engine = engine.to_path_buf();
    tokio::spawn(async move {
        let mut clients = tokio::task::JoinSet::new();
        loop {
            let (socket, _) = listener.accept().await.unwrap();
            let connection = connection.clone();
            let engine = engine.clone();
            clients.spawn(async move {
                if let Err(error) = connection.serve(socket, &engine).await {
                    eprintln!("native fixture client closed: {error}");
                }
            });
        }
    })
}
