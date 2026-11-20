use std::path::Path;
use std::process::{Command, Output};

use loopflow_execution::store::{open_ephemeral_store, StorageConfig};
use loopflow_test_support::TestRepo;
use serde_json::Value;

fn invoke(repo: &Path, home: &Path, args: &[&str]) -> Output {
    invoke_lf(repo, home, &[&["config", "planning"][..], args].concat())
}

fn invoke_lf(repo: &Path, home: &Path, args: &[&str]) -> Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    for (name, _) in std::env::vars_os() {
        let name = name.to_string_lossy();
        if name.starts_with("LF_") || name.starts_with("LOOPFLOW_") {
            command.env_remove(name.as_ref());
        }
    }
    command
        .current_dir(repo)
        .args(args)
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .output()
        .unwrap()
}

fn run(repo: &Path, home: &Path, args: &[&str]) -> String {
    let output = invoke(repo, home, args);
    assert!(
        output.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

fn initialize(home: &Path) {
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(open_ephemeral_store(&StorageConfig::sqlite(
            home.join("loopflow.db"),
        )))
        .unwrap();
}

#[test]
fn without_a_tracker_the_plan_is_on_this_machine() {
    let repo = TestRepo::new();
    loopflow_execution::git::git_stdout(repo.path(), &["remote", "remove", "origin"]).unwrap();
    let home = tempfile::tempdir().unwrap();
    initialize(home.path());
    // `provider: git` is the released shape; it reads as no tracker.
    for config in [
        None,
        Some("planning:\n  provider: git\n  shared: employees\n"),
    ] {
        if let Some(config) = config {
            std::fs::create_dir_all(repo.path().join(".lf")).unwrap();
            std::fs::write(repo.path().join(".lf/config.yaml"), config).unwrap();
        }
        let status: Value =
            serde_json::from_str(&run(repo.path(), home.path(), &["status", "--json"])).unwrap();
        assert_eq!(status["tracker"], Value::Null, "{config:?}");
        assert_eq!(
            status["plan"],
            serde_json::json!({ "git": null, "hub": null }),
            "{config:?}"
        );
        assert_eq!(
            run(repo.path(), home.path(), &["status"]),
            "Plan: this Machine."
        );
    }
}

#[test]
fn a_linear_repository_reports_its_tracker() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    initialize(home.path());
    std::fs::create_dir_all(repo.path().join(".lf")).unwrap();
    std::fs::write(
        repo.path().join(".lf/config.yaml"),
        "planning:\n  provider: linear\n",
    )
    .unwrap();
    let status: Value =
        serde_json::from_str(&run(repo.path(), home.path(), &["status", "--json"])).unwrap();
    assert_eq!(status["tracker"], "linear");
    assert_eq!(status["plan"], Value::Null);
    for retired in ["connect", "use", "select", "associate", "key", "journal"] {
        assert!(!invoke(repo.path(), home.path(), &[retired])
            .status
            .success());
    }
}

#[test]
fn unavailable_linear_saves_locally() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    initialize(home.path());
    std::fs::create_dir_all(repo.path().join(".lf")).unwrap();
    std::fs::write(
        repo.path().join(".lf/config.yaml"),
        "planning:\n  provider: linear\n  team: unavailable-fixture\n",
    )
    .unwrap();
    let output = invoke_lf(
        repo.path(),
        home.path(),
        &["task", "create", "--title", "Buffered follow-up", "--json"],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let task = serde_json::from_slice::<Value>(&output.stdout).unwrap()["task"].clone();
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    let title: String = conn
        .query_row(
            "SELECT issue_title FROM tasks WHERE id=?1",
            [task["id"].as_str().unwrap()],
            |r| r.get(0),
        )
        .unwrap();
    assert_eq!(title, "Buffered follow-up");
    assert_eq!(
        conn.query_row("SELECT count(*) FROM task_creation_intents", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn a_checkout_miss_without_a_tracker_ignores_retained_linear_bindings() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    initialize(home.path());
    repo.create_file(
        ".lf/config.yaml",
        "planning:\n  provider: git\n  shared: fixture\n",
    );
    repo.create_file(
        ".lf/wave/product/GOAL.md",
        "---\nlinear:\n  initiative: retained-initiative\n---\nRetain work.\n",
    );
    let output = invoke_lf(repo.path(), home.path(), &["checkout", "MISSING-1"]);
    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("not found"), "{error}");
    let conn = rusqlite::Connection::open(home.path().join("loopflow.db")).unwrap();
    assert_eq!(
        conn.query_row("SELECT count(*) FROM tasks", [], |row| row.get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM tracker_items", [], |row| row
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
}
