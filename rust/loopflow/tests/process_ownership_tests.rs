//! Command observation is durable even when no agent work starts.
#![cfg(unix)]

use loopflow_execution::process::attachment;
#[allow(dead_code)] // Linked for its inherited provider-home scrub.
mod support;

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use loopflow_execution::session::{LfSession, TitleSource};
use loopflow_execution::store::sqlite::SqliteStore;
use loopflow_execution::store::{open_ephemeral_store, StorageConfig};
use loopflow_model::id::LfProcessId;
use loopflow_test_support::{RunningParent, TestRepo};

#[test]
fn task_checkout_blockers_name_the_held_session() {
    let home = tempfile::tempdir().unwrap();
    let repo = TestRepo::new();
    let lf = |cwd: &Path, args: &[&str]| {
        let mut command = command(home.path(), cwd, args);
        command
            .env("HOME", home.path())
            .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
            .env("LF_USER_NAME", "Fixture Person")
            .stdin(std::process::Stdio::null());
        command.output().unwrap()
    };
    let json = |cwd: &Path, args: &[&str]| {
        let output = lf(cwd, args);
        assert!(output.status.success(), "{args:?}: {output:?}");
        serde_json::from_slice::<serde_json::Value>(&output.stdout).unwrap()
    };
    let created = json(
        repo.path(),
        &["task", "create", "--title", "Hold a Task", "--json"],
    )["task"]
        .clone();
    let id = created["id"].as_str().unwrap();
    let placed = json(repo.path(), &["checkout", id, "--json"]);
    let worktree = Path::new(placed["worktree"].as_str().unwrap());

    let database = home.path().join("loopflow.db");
    let store = loopflow_execution::store::open_sqlite(&database).unwrap();
    reserve_session(&store, "held", worktree);
    attachment::claim_session_attachment(&store, "held", None, &recorded_lf_process(&database))
        .unwrap();

    // Cancellation is the Task's decision; only checkout cleanup is held.
    let abandoned = lf(repo.path(), &["task", "abandon", id]);
    assert!(abandoned.status.success(), "{abandoned:?}");
    let reason = String::from_utf8_lossy(&abandoned.stderr);
    assert!(
        reason.contains("Session held has live or unresolved execution"),
        "{reason}"
    );
    assert!(worktree.exists());
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
        assert_eq!(
            std::fs::read(&database).unwrap(),
            b"retained incompatible store"
        );
        assert_eq!(std::fs::read_dir(home.path()).unwrap().count(), 1);
        if code == 0 {
            assert!(!output.stdout.is_empty(), "{output:?}");
            assert!(output.stderr.is_empty(), "{output:?}");
        } else {
            assert!(String::from_utf8_lossy(&output.stderr).contains("unexpected argument"));
        }
    }
}

#[tokio::test]
async fn early_commands_exit_exactly_without_recording_initializing_or_migrating() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    drop(store);
    let parent = RunningParent::insert(&database);
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
        let output = child(home.path(), home.path(), &args, &parent)
            .env("PATH", "")
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(code), "{output:?}");
    }
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

#[test]
fn empty_release_check_returns_through_process_completion() {
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
    let database = home.path().join("loopflow.db");
    drop(loopflow_execution::store::open_sqlite(&database).unwrap());
    let parent = RunningParent::insert(&database);
    let output = child(home.path(), repo.path(), &["release", "check"], &parent)
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
}

#[tokio::test]
async fn command_failures_exit_1_and_keep_retained_work() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let repo = TestRepo::new();
    let parent = RunningParent::insert(&database);
    for args in [
        vec!["session", "rename", "missing", "Name"],
        vec![
            "--wave",
            "fixture-wave-does-not-exist",
            "session",
            "resume",
            "--prepare",
            "--json",
        ],
    ] {
        let result = child(home.path(), repo.path(), &args, &parent)
            .output()
            .unwrap();
        assert_eq!(result.status.code(), Some(1), "{result:?}");
    }
    std::fs::write(repo.path().join("change.txt"), "retained work").unwrap();
    let lock = repo.path().join(".git/index.lock");
    std::fs::write(&lock, "retained lock").unwrap();
    let result = child(
        home.path(),
        repo.path(),
        &["commit", "-m", "fixture"],
        &parent,
    )
    .output()
    .unwrap();
    assert_eq!(result.status.code(), Some(1), "{result:?}");
    assert!(String::from_utf8_lossy(&result.stderr).contains("index.lock"));
    assert_eq!(std::fs::read_to_string(lock).unwrap(), "retained lock");
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

fn child(home: &Path, cwd: &Path, args: &[&str], parent: &RunningParent) -> Command {
    let mut command = command(home, cwd, args);
    parent.adopt(&mut command);
    command
}

/// An attaching lf records its own row; fixtures attaching from outside one record it here.
fn recorded_lf_process(database: &Path) -> LfProcessId {
    let id = LfProcessId::new();
    rusqlite::Connection::open(database)
        .unwrap()
        .execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?1,unixepoch())",
            [&id],
        )
        .unwrap();
    id
}

fn reserve_session(store: &SqliteStore, session_id: &str, repo: &Path) {
    let run_id = uuid::Uuid::new_v4().simple().to_string();
    loopflow_execution::store::sessions::create_session(
        store,
        LfSession {
            captured: None,
            task_id: None,
            wave_id: None,
            graph_lf_process_id: None,
            work_source: None,
            bound_at: None,
            id: session_id.into(),
            artifact_key: Some(run_id),
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
            title: "AgentProcess ownership".into(),
            title_source: TitleSource::Generated,
            request: None,
            completed_at: None,
            created_at: 1,
        },
        None,
    )
    .unwrap();
}

#[tokio::test]
async fn historical_sessions_are_not_live_availability() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let store = loopflow_execution::store::open_sqlite(&database).unwrap();
    for id in ["waiting", "finished", "legacy_finished", "unobserved"] {
        reserve_session(&store, id, home.path());
    }
    let finished = attachment::claim_session_attachment(
        &store,
        "finished",
        None,
        &recorded_lf_process(&database),
    )
    .unwrap();
    let connection = rusqlite::Connection::open(&database).unwrap();
    connection
        .execute(
            "UPDATE agent_sessions SET completed_at=2 WHERE id='legacy_finished'",
            [],
        )
        .unwrap();
    connection.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
    connection
        .execute(
            "UPDATE processes SET completed_at=2 WHERE id=?1",
            [&finished.agent_process_id],
        )
        .unwrap();
    for id in ["waiting", "finished", "legacy_finished", "unobserved"] {
        let output = command(home.path(), home.path(), &["session", "card", id, "--json"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let card: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        let session = &card["session"];
        assert!(session["availability"].is_null(), "{session}");
        assert_ne!(session["state"], "active", "{session}");
    }
}

#[tokio::test]
async fn inspection_starts_no_work() {
    let home = tempfile::tempdir().unwrap();
    let database = home.path().join("loopflow.db");
    let _store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
        .await
        .unwrap();
    let parent = RunningParent::insert(&database);
    let output = child(
        home.path(),
        home.path(),
        &["session", "list", "--all", "--json"],
        &parent,
    )
    .output()
    .unwrap();
    assert!(output.status.success(), "{output:?}");
    let connection = rusqlite::Connection::open(&database).unwrap();
    let work: i64 = connection
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='runs'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(work, 0, "inspection must not reserve agent or Task work");
}
