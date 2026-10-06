//! Command observation is durable even when no agent work starts.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use loopflow::harness::codex_connection::CodexConnection;
use loopflow::id::ExecId;
use loopflow::session::{AgentSession, TitleSource};
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::{open_ephemeral_store, StorageConfig};
use loopflow_test_support::TestRepo;

#[tokio::test]
async fn exec_discovery_pages_real_commands_and_preserves_unknown_history() {
    use loopflow::exec::{Exec, ExecPage};
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let connection = rusqlite::Connection::open(&database).unwrap();
    let parent = ExecId::new();
    let raw = serde_json::to_string(&["lf", "pr", "land", "--strict", "%_ literal"]).unwrap();
    connection
        .execute(
            "INSERT INTO execs(id,trace_id,started_at) VALUES(?1,?1,1)",
            [&parent],
        )
        .unwrap();
    let ids = [ExecId::new(), ExecId::new()];
    for (index, id) in ids.iter().enumerate() {
        connection.execute("INSERT INTO execs(id,trace_id,parent_exec_id,via_agent,caller_session_id,command,started_at)
            VALUES(?1,?1,?2,?3,?4,?5,?6)", rusqlite::params![id,parent,index != 0,
                if index == 0 { None } else { Some("caller-session") },raw,10-index as i64]).unwrap();
    }
    let invoke = |args: &[&str]| {
        let result = command(home.path(), home.path(), args).output().unwrap();
        assert!(result.status.success(), "{result:?}");
        result.stdout
    };
    let first: ExecPage = serde_json::from_slice(&invoke(&[
        "monitor",
        "list",
        "--all",
        "--parent",
        parent.as_str(),
        "--search",
        "PR LAND --strict %_",
        "--outcome",
        "unknown",
        "--limit",
        "1",
        "--json",
    ]))
    .unwrap();
    assert_eq!(first.entries[0].id, ids[0]);
    assert_eq!(first.entries[0].via_agent, Some(false));
    assert_eq!(first.entries[0].command.as_ref(), Some(&raw));
    assert_eq!(first.entries[0].outcome, None);
    let cursor = serde_json::to_string(first.next.as_ref().unwrap()).unwrap();
    let second: ExecPage = serde_json::from_slice(&invoke(&[
        "monitor",
        "list",
        "--all",
        "--parent",
        parent.as_str(),
        "--search",
        "PR LAND --strict %_",
        "--outcome",
        "unknown",
        "--limit",
        "1",
        "--after",
        &cursor,
        "--json",
    ]))
    .unwrap();
    assert_eq!(second.entries[0].id, ids[1]);
    assert_eq!(
        second.entries[0].caller_session_id.as_deref(),
        Some("caller-session")
    );
    assert_eq!(second.next, None);
    let detail: Exec = serde_json::from_slice(&invoke(&[
        "monitor",
        "show",
        &ids[1].as_str()[..20],
        "--json",
    ]))
    .unwrap();
    assert_eq!(detail, second.entries[0]);
    let caller: ExecPage = serde_json::from_slice(&invoke(&[
        "monitor",
        "list",
        "--all",
        "--caller",
        "caller-session",
        "--json",
    ]))
    .unwrap();
    assert_eq!(caller.entries, vec![detail]);
    let wave = loopflow::id::WaveId::new();
    let project = loopflow::durable::ProjectId::new();
    let task = loopflow::durable::TaskId::new();
    connection
        .execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'historical','/missing',1)",
            [&wave],
        )
        .unwrap();
    connection.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project-proof',1)",
        rusqlite::params![project.as_str(),wave]).unwrap();
    connection
        .execute(
            "INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,work_state,work_terminal_at,created_at)
        VALUES(?1,?2,'issue-proof','PROOF-1','done',2,1)",
            rusqlite::params![task.as_str(), project.as_str()],
        )
        .unwrap();
    connection.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id)
        VALUES('caller-session','Historical','human',1,1,'/missing',?1,?2)",
        rusqlite::params![task.as_str(),wave]).unwrap();
    connection.execute("INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,exec_id,task_id,wave_id,observed_at,payload)
        VALUES('caller-session','thread','turn','started','',?1,?2,?3,1,'unreadable history')",
        rusqlite::params![ids[1],task.as_str(),wave]).unwrap();
    for args in [
        vec!["monitor", "list", "--all", "--task", "PROOF-1", "--json"],
        vec![
            "monitor",
            "list",
            "--all",
            "--task",
            task.as_str(),
            "--json",
        ],
        vec!["monitor", "list", "--all", "--wave", "historical", "--json"],
    ] {
        let page: ExecPage = serde_json::from_slice(&invoke(&args)).unwrap();
        assert_eq!(
            page.entries.iter().map(|exec| &exec.id).collect::<Vec<_>>(),
            vec![&ids[1]]
        );
    }

    let repos = [TestRepo::new(), TestRepo::new()];
    let other_wave = loopflow::id::WaveId::new();
    let paths = repos
        .iter()
        .map(|repo| {
            loopflow::repository::CanonicalRepo::discover(repo.path())
                .unwrap()
                .to_string()
        })
        .collect::<Vec<_>>();
    connection
        .execute(
            "UPDATE waves SET repo=?2 WHERE id=?1",
            rusqlite::params![wave, paths[0]],
        )
        .unwrap();
    connection
        .execute(
            "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'historical',?2,1)",
            rusqlite::params![other_wave, paths[1]],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE execs SET repo=?2 WHERE id=?1",
            rusqlite::params![ids[1], paths[0]],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE execs SET repo=?2 WHERE id=?1",
            rusqlite::params![ids[0], paths[1]],
        )
        .unwrap();
    connection.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,wave_id)
        VALUES('other-session','Other','human',1,1,'/missing',?1)", [&other_wave]).unwrap();
    connection.execute("INSERT INTO session_events(session_id,provider_thread,provider_turn,kind,receipt_key,exec_id,wave_id,observed_at,payload)
        VALUES('other-session','thread','turn','started','',?1,?2,1,'unreadable history')", rusqlite::params![ids[0],other_wave]).unwrap();
    for (repo, expected) in repos.iter().zip([&ids[1], &ids[0]]) {
        let result = command(
            home.path(),
            repo.path(),
            &["monitor", "list", "--wave", "historical", "--json"],
        )
        .output()
        .unwrap();
        assert!(result.status.success(), "{result:?}");
        let page: ExecPage = serde_json::from_slice(&result.stdout).unwrap();
        assert_eq!(
            page.entries.iter().map(|exec| &exec.id).collect::<Vec<_>>(),
            vec![expected]
        );
    }
    let ambiguous = command(
        home.path(),
        repos[0].path(),
        &["monitor", "list", "--all", "--wave", "historical", "--json"],
    )
    .output()
    .unwrap();
    assert!(!ambiguous.status.success());
    assert!(String::from_utf8_lossy(&ambiguous.stderr).contains("Ambiguous Work selector"));
    let explicit: ExecPage = serde_json::from_slice(&invoke(&[
        "monitor",
        "list",
        "--all",
        "--wave",
        other_wave.as_str(),
        "--json",
    ]))
    .unwrap();
    assert_eq!(explicit.entries[0].id, ids[0]);
    let zero = command(
        home.path(),
        home.path(),
        &["monitor", "list", "--limit", "0"],
    )
    .output()
    .unwrap();
    assert_eq!(zero.status.code(), Some(2));
}

#[test]
fn parser_returns_exact_status_without_admitting_an_early_store() {
    for (args, code) in [
        (vec!["--help"], 0),
        (vec!["--version"], 0),
        (vec!["session", "list", "--definitely-not-a-flag"], 2),
    ] {
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("loopflow.db");
        // An incompatible existing target must not be opened or repaired for help.
        std::fs::write(&database, b"retained incompatible store").unwrap();
        let output = command(home.path(), home.path(), &args)
            .env("PATH", "")
            .env("RUST_LOG", "off")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(code), "{output:?}");
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains("Exec history unavailable: no compatible process ledger"));
        assert_eq!(
            std::fs::read(&database).unwrap(),
            b"retained incompatible store"
        );
        assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 1);
        if code == 0 {
            assert!(!output.stdout.is_empty(), "{output:?}");
        } else {
            assert!(String::from_utf8_lossy(&output.stderr).contains("unexpected argument"));
        }
    }
}

#[tokio::test]
async fn early_commands_record_exact_exits_without_initializing_or_migrating() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    drop(store);
    let conn = rusqlite::Connection::open(&database).unwrap();
    let schema: String = conn
        .query_row(
            "SELECT group_concat(sql) FROM sqlite_master ORDER BY name",
            [],
            |row| row.get(0),
        )
        .unwrap();
    for (args, code) in [
        (vec!["--help"], 0),
        (vec!["--version"], 0),
        (vec!["session", "list", "--definitely-not-a-flag"], 2),
    ] {
        let output = command(home.path(), home.path(), &args)
            .env("PATH", "")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(code), "{output:?}");
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("Exec history unavailable"),
            "{output:?}"
        );
    }
    let rows: Vec<(i32, String)> = conn
        .prepare("SELECT exit_code,outcome FROM execs ORDER BY started_at,rowid")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        rows,
        vec![
            (0, "succeeded".into()),
            (0, "succeeded".into()),
            (2, "failed".into())
        ]
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM agent_sessions", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row(
            "SELECT group_concat(sql) FROM sqlite_master ORDER BY name",
            [],
            |row| row.get::<_, String>(0)
        )
        .unwrap(),
        schema
    );
}

#[tokio::test]
#[ignore = "requires disposable OS installation: scripts/test_task_installation.py"]
async fn early_observation_records_preflight_and_screenshot_child_ancestry() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    drop(store);
    let output = command(
        home.path(),
        home.path(),
        &["home", "install", "preflight", "--json"],
    )
    .output()
    .unwrap();
    assert!(!output.status.success(), "{output:?}");
    // No browser executable is available; both actual lf processes still exist.
    let output = command(
        home.path(),
        home.path(),
        &["home", "screenshot", "missing.html", "-o", "missing.png"],
    )
    .env("PATH", "")
    .output()
    .unwrap();
    assert!(!output.status.success(), "{output:?}");
    let conn = rusqlite::Connection::open(&database).unwrap();
    let (count, completed): (i64, i64) = conn
        .query_row(
            "SELECT count(*),count(completed_at) FROM execs",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!((count, completed), (3, 3));
    let (child, parent): (String,String) = conn.query_row(
        "SELECT c.id,p.id FROM execs c JOIN execs p ON c.parent_exec_id=p.id WHERE c.command LIKE '%__screenshot-supervisor%'",
        [], |row| Ok((row.get(0)?,row.get(1)?))).unwrap();
    assert_ne!(child, parent);
    assert!(!home.path().join("missing.png").exists());
}

#[test]
fn remote_command_status_is_the_local_exec_status() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let bin = home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    // No real credential CLI, Keychain reader or remote transport participates.
    for (name, script) in [
        ("ssh", "#!/bin/sh\ncat >/dev/null\nexit 42\n"),
        ("gh", "#!/bin/sh\nexit 1\n"),
        ("security", "#!/bin/sh\nexit 1\n"),
    ] {
        let path = bin.join(name);
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let output = command(
        home.path(),
        repo.path(),
        &["home", "ssh", "proof@example.invalid", "catalog"],
    )
    .env_clear()
    .env("HOME", home.path())
    .env("LF_HOME", home.path())
    .env(
        "PATH",
        format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", bin.display()),
    )
    .output()
    .unwrap();
    assert_eq!(output.status.code(), Some(42), "{output:?}");
    assert!(
        !String::from_utf8_lossy(&output.stderr).contains("Error:"),
        "{output:?}"
    );
    assert_recorded_exit(home.path(), 42);
}

#[test]
fn empty_release_check_returns_through_exec_completion() {
    let repo = TestRepo::new();
    let tagged = Command::new("git")
        .args(["tag", "v0.9.0"])
        .current_dir(repo.path())
        .output()
        .unwrap();
    assert!(tagged.status.success(), "{tagged:?}");
    let home = tempfile::tempdir().unwrap();
    let bin = home.path().join("bin");
    std::fs::create_dir(&bin).unwrap();
    let gh = bin.join("gh");
    std::fs::write(&gh, "#!/bin/sh\ncase \"$1 $2\" in '--version ') exit 0;; 'pr list') echo '[]'; exit 0;; esac\nexit 1\n").unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let output = command(home.path(), repo.path(), &["release", "check"])
        .env(
            "PATH",
            format!("{}:/usr/bin:/bin:/usr/sbin:/sbin", bin.display()),
        )
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("No commits in the target area since the last tag."),
        "{output:?}"
    );
    assert!(!stderr.contains("Error:"), "{output:?}");
    assert_recorded_exit(home.path(), 1);
}

fn assert_recorded_exit(home: &Path, code: i32) {
    let conn = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
    let rows: Vec<(String, Option<i32>, bool, Option<String>)> = conn
        .prepare("SELECT outcome,exit_code,completed_at IS NOT NULL,signal FROM execs")
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows, vec![("failed".into(), Some(code), true, None)]);
    let sessions: i64 = conn
        .query_row("SELECT count(*) FROM agent_sessions", [], |row| row.get(0))
        .unwrap();
    assert_eq!(sessions, 0);
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
    command.args(args).current_dir(cwd).env("LF_HOME", home);
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
    let run_id = uuid::Uuid::new_v4().simple().to_string();
    store
        .create_session(
            AgentSession {
                captured: None,
                task_id: None,
                wave_id: None,
                flow_id: None,
                work_source: None,
                bound_at: None,
                id: session_id.into(),
                artifact_key: run_id,
                caller_artifact_key: None,
                input_published: false,
                cwd: repo.into(),
                skill: None,
                provider: Some("codex".into()),
                model: None,
                node: None,
                iterations: None,
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

#[tokio::test]
async fn monitor_prune_preserves_unknown_outcomes_and_removes_only_settled_dead_receipts() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let conn = rusqlite::Connection::open(database).unwrap();
    let mut child = Command::new("sleep").arg("30").spawn().unwrap();
    let pid = child.id();
    child.kill().unwrap();
    child.wait().unwrap();
    let directory = home.path().join("runtime/exec-processes");
    std::fs::create_dir_all(&directory).unwrap();

    for terminal in [false, true] {
        let id = ExecId::new();
        conn.execute(
            "INSERT INTO execs(id,trace_id,started_at,completed_at,outcome) VALUES(?1,?1,1,?2,?3)",
            rusqlite::params![id, terminal.then_some(2), terminal.then_some("failed")],
        )
        .unwrap();
        // Existing PID-named receipts and new Exec-named receipts share retention rules.
        for name in [pid.to_string(), id.to_string()] {
            let receipt = directory.join(format!("{name}.json"));
            let bytes = serde_json::to_vec(&serde_json::json!({
                "schema_version": 1, "trace_id": id, "exec_id": id,
                "pid": pid, "started_at": 1,
            }))
            .unwrap();
            for json in [false, true] {
                std::fs::write(&receipt, &bytes).unwrap();
                for dry_run in [true, false] {
                    let mut args = vec!["monitor", "prune"];
                    if dry_run {
                        args.push("--dry-run");
                    }
                    if json {
                        args.push("--json");
                    }
                    let output = command(home.path(), home.path(), &args).output().unwrap();
                    assert!(output.status.success(), "{output:?}");
                    assert_eq!(receipt.exists(), dry_run || !terminal, "{args:?}");
                    if receipt.exists() {
                        assert_eq!(std::fs::read(&receipt).unwrap(), bytes);
                    }
                    if json {
                        let report: serde_json::Value =
                            serde_json::from_slice(&output.stdout).unwrap();
                        assert_eq!(report["dry_run"], dry_run);
                        assert_eq!(
                            report["removed_exec_receipts"],
                            u32::from(!dry_run && terminal)
                        );
                        assert_eq!(report["errors"], 0);
                    }
                }
            }
            if receipt.exists() {
                std::fs::remove_file(receipt).unwrap();
            }
        }
        let outcome: Option<String> = conn
            .query_row("SELECT outcome FROM execs WHERE id=?1", [&id], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(outcome.as_deref(), terminal.then_some("failed"));
    }
}

#[tokio::test]
async fn monitor_does_not_treat_historical_feedback_as_live_readiness() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let store = SqliteStore::new(&database).unwrap();
    for id in ["waiting", "finished", "unobserved"] {
        reserve_session(&store, id, home.path());
    }
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute(
            r#"INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES('waiting','observed','legacy_review_feedback',1,'{"type":"legacy_review_feedback","summary":"Review the API"}')"#,
            [],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE agent_sessions SET completed_at=2 WHERE id='finished'",
            [],
        )
        .unwrap();
    let output = command(home.path(), home.path(), &["monitor", "--all", "--json"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let overview: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let items = overview["items"].as_array().unwrap();
    for (id, state, reason, action) in [
        (
            "waiting",
            "unknown",
            "No current provider observation",
            "lf session history waiting",
        ),
        (
            "finished",
            "finished",
            "completion is recorded",
            "lf session history finished",
        ),
        (
            "unobserved",
            "unknown",
            "No current provider observation",
            "lf session history unobserved",
        ),
    ] {
        let item = items.iter().find(|item| item["id"] == id).unwrap();
        assert_eq!(item["state"], state);
        assert!(item["reason"].as_str().unwrap().contains(reason));
        assert_eq!(item["next_action"], action);
    }
    assert!(overview["recent_commands"]["entries"].is_array());
    assert!(overview["active"]["gaps"].is_array());
    assert!(overview["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|item| item["state"] != "active"));
    for args in [
        vec!["monitor", "ps", "--help"],
        vec!["monitor", "top", "--help"],
        vec!["monitor", "--help"],
    ] {
        let help = command(home.path(), home.path(), &args).output().unwrap();
        assert!(help.status.success(), "{help:?}");
        assert!(String::from_utf8_lossy(&help.stdout).contains("monitor"));
    }
}

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
