mod support;

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use loopflow::flow::Command as FlowCommand;
use loopflow_test_support::TestRepo;
use serde_json::Value;
use support::EnvGuard;

fn run(repo: &Path, home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .env_clear()
        .env("HOME", home)
        .env("LF_HOME", home.join(".lf"))
        .env("PATH", "/usr/bin:/bin")
        .env("NO_COLOR", "1")
        .current_dir(repo)
        .args(args)
        .output()
        .unwrap()
}

fn success(output: Output) -> Vec<u8> {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

#[test]
fn tokens_measures_tracked_source_through_the_repo_owner() {
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file("source.txt", "alpha beta\ngamma delta\n");
    repo.stage_all();
    repo.commit("Add measured source");
    repo.create_file("untracked.txt", "This is not source evidence.\n");

    let tree: Value = serde_json::from_slice(&success(run(
        repo.path(),
        home.path(),
        &["repo", "tokens", "--json"],
    )))
    .unwrap();
    let source = tree["children"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["path"] == "source.txt")
        .unwrap();
    assert_eq!(source["lines"], 2);
    assert!(source["tokens"].as_u64().unwrap() > 0);
    assert!(!tree["children"]
        .as_array()
        .unwrap()
        .iter()
        .any(|node| node["path"] == "untracked.txt"));
    let history: Value = serde_json::from_slice(&success(run(
        repo.path(),
        home.path(),
        &["repo", "tokens", "--days", "1", "--json"],
    )))
    .unwrap();
    assert_eq!(history.as_array().unwrap().len(), 1);
    assert_eq!(history[0]["lines"], tree["lines"]);
    assert_eq!(history[0]["tokens"], tree["tokens"]);
}

#[test]
fn ci_remains_a_read_only_home_report_outside_a_checkout() {
    let dir = tempfile::tempdir().unwrap();
    let home = tempfile::tempdir().unwrap();
    let report: Value = serde_json::from_slice(&success(run(
        dir.path(),
        home.path(),
        &["repo", "ci", "--repo", "example/repo", "--json"],
    )))
    .unwrap();
    assert_eq!(report["repo"], "example/repo");
    assert_eq!(report["summary"]["incidents"], 0);
    assert!(report["summary"]["median_detection_seconds"].is_null());
    assert_eq!(report["incidents"], serde_json::json!([]));
    let db = rusqlite::Connection::open(home.path().join(".lf/loopflow.db")).unwrap();
    let sessions: i64 = db
        .query_row("SELECT count(*) FROM agent_sessions", [], |row| row.get(0))
        .unwrap();
    let completed: i64 = db
        .query_row(
            "SELECT count(*) FROM processes WHERE outcome='succeeded'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(sessions, 0);
    assert_eq!(completed, 1);
}

#[test]
fn release_bump_uses_the_same_operation_from_cli_and_captured_flow() {
    let _env = EnvGuard::new(&[]);
    let repo = TestRepo::new();
    let home = tempfile::tempdir().unwrap();
    repo.create_file(
        "Cargo.toml",
        "[package]\nname = \"fixture\"\nversion = \"0.9.0\"\n",
    );
    success(run(
        repo.path(),
        home.path(),
        &["repo", "release", "bump", "0.9.1"],
    ));
    assert!(fs::read_to_string(repo.path().join("Cargo.toml"))
        .unwrap()
        .contains("version = \"0.9.1\""));

    for (command, args, version) in [
        ("repo", vec!["release", "bump", "0.9.2"], "0.9.2"),
        ("release", vec!["bump", "0.9.3"], "0.9.3"),
    ] {
        let captured = FlowCommand {
            command: command.into(),
            args: args.into_iter().map(str::to_string).collect(),
        };
        // A Flow runs the operation as its own command.
        let argv = captured.argv();
        let argv: Vec<&str> = argv[1..].iter().map(String::as_str).collect();
        success(run(repo.path(), home.path(), &argv));
        assert!(fs::read_to_string(repo.path().join("Cargo.toml"))
            .unwrap()
            .contains(&format!("version = \"{version}\"")));
    }
}
