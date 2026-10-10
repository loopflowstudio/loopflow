use std::path::Path;
use std::process::{Command, Output};

use loopflow::store::{open_ephemeral_store, StorageConfig};
use loopflow_test_support::TestRepo;
use serde_json::Value;

fn invoke(repo: &Path, home: &Path, args: &[&str]) -> Output {
    invoke_lf(repo, home, &[&["repo", "planning"][..], args].concat())
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
fn personal_git_is_automatic_and_key_recovery_selects_the_same_ref() {
    let repo = TestRepo::new();
    let left = tempfile::tempdir().unwrap();
    let right = tempfile::tempdir().unwrap();
    initialize(left.path());
    initialize(right.path());
    let status: Value =
        serde_json::from_str(&run(repo.path(), left.path(), &["status", "--json"])).unwrap();
    assert_eq!(status["transport"], "git");
    let key = run(repo.path(), left.path(), &["key"]);
    assert!(uuid::Uuid::parse_str(&key).is_ok());
    run(repo.path(), right.path(), &["key", "--recover", &key]);
    let other: Value =
        serde_json::from_str(&run(repo.path(), right.path(), &["status", "--json"])).unwrap();
    assert_eq!(
        status["destinations"][0]["reference"],
        other["destinations"][0]["reference"]
    );
    assert_eq!(status["destinations"][0]["active"], true);
    assert!(status["destinations"][0]["publication_revision"].is_null());
    assert!(status["destinations"][0].get("endpoint").is_none());
}

#[test]
fn repository_selects_shared_git_or_linear_without_manual_enrollment() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    initialize(home.path());
    std::fs::create_dir_all(repo.path().join(".lf")).unwrap();
    std::fs::write(
        repo.path().join(".lf/config.yaml"),
        "planning:\n  provider: git\n  shared: employees\n",
    )
    .unwrap();
    let status: Value =
        serde_json::from_str(&run(repo.path(), home.path(), &["status", "--json"])).unwrap();
    assert_eq!(
        status["destinations"][0]["reference"],
        "refs/loopflow/planning/shared/employees"
    );
    assert!(!invoke(repo.path(), home.path(), &["key"]).status.success());
    std::fs::write(
        repo.path().join(".lf/config.yaml"),
        "planning:\n  provider: linear\n",
    )
    .unwrap();
    let status: Value =
        serde_json::from_str(&run(repo.path(), home.path(), &["status", "--json"])).unwrap();
    assert_eq!(status["transport"], "linear");
    assert_eq!(status["destinations"], serde_json::json!([]));
    let refs = Command::new("git")
        .current_dir(repo.path())
        .args(["ls-remote", "origin", "refs/loopflow/planning/*"])
        .output()
        .unwrap();
    assert!(refs.status.success());
    assert!(refs.stdout.is_empty());
    for retired in ["connect", "use", "select", "associate"] {
        assert!(!invoke(repo.path(), home.path(), &[retired])
            .status
            .success());
    }
}

#[test]
fn unavailable_linear_saves_locally_without_publishing_a_git_plan() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    initialize(home.path());
    std::fs::create_dir_all(repo.path().join(".lf")).unwrap();
    std::fs::write(
        repo.path().join(".lf/config.yaml"),
        "planning:\n  provider: linear\npm:\n  linear_team: unavailable-fixture\n",
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
    let task: Value = serde_json::from_slice(&output.stdout).unwrap();
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
        conn.query_row("SELECT count(*) FROM planning_destinations", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        conn.query_row("SELECT count(*) FROM task_creation_intents", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        1
    );
    let refs = Command::new("git")
        .current_dir(repo.path())
        .args(["ls-remote", "origin", "refs/loopflow/planning/*"])
        .output()
        .unwrap();
    assert!(refs.status.success());
    assert!(refs.stdout.is_empty());
}
