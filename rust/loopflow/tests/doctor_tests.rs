use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use loopflow_test_support::TestRepo;

fn run_lf(home: &Path, args: &[&str]) -> Output {
    let binary_dir = Path::new(env!("CARGO_BIN_EXE_lf")).parent().unwrap();
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(args)
        .current_dir(home)
        .env("HOME", home)
        .env("LF_HOME", home)
        .env("LF_BIN", env!("CARGO_BIN_EXE_lf"))
        .env_remove("LF_AS")
        .env("NO_COLOR", "1")
        .env(
            "PATH",
            format!(
                "{}:{}",
                binary_dir.display(),
                std::env::var("PATH").unwrap()
            ),
        )
        // Doctor prefers the compiled source root; scope even git -C there to
        // this fixture so freshness checks cannot fetch into a shared checkout.
        .env("GIT_DIR", home.join(".git"))
        .env("GIT_WORK_TREE", home)
        .env("GIT_ALLOW_PROTOCOL", "file")
        .env_remove("LF_TRACE_ID")
        .env_remove("LF_PROCESS_ID")
        .env_remove("LF_WAVE_ID")
        .env_remove("LF_CAPTURE_KEY")
        .output()
        .unwrap()
}

#[test]
fn doctor_json_reports_the_build_revision_and_freshness_check() {
    let home = TestRepo::new();
    loopflow_execution::store::open_sqlite(&home.path().join("loopflow.db")).unwrap();
    let fetch_head = home.path().join(".git/FETCH_HEAD");
    let before = fs::read(&fetch_head).ok();
    let output = run_lf(home.path(), &["self", "doctor", "--json"]);
    assert!(
        output.status.success(),
        "lf self doctor failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    assert_eq!(fs::read(fetch_head).ok(), before);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        report["store"]["build_source_revision"],
        loopflow::build_info::source_revision()
    );
    assert!(report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| check["name"] == "binary-freshness"));
}

#[test]
fn doctor_does_not_initialize_a_missing_database() {
    let home = TestRepo::new();
    let output = run_lf(home.path(), &["self", "doctor", "--json"]);
    assert!(!output.status.success());
    assert!(!home.path().join("loopflow.db").exists());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["store"]["migration_error"]
        .as_str()
        .unwrap()
        .contains("does not exist"));
    assert!(report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| check["name"] == "install-selection"));
}

/// A store a newer lf migrated stays readable: doctor and status reads answer
/// from it instead of refusing the migration they don't know.
#[test]
fn reads_answer_from_a_store_a_newer_build_migrated() {
    let home = TestRepo::new();
    let path = home.path().join("loopflow.db");
    loopflow_execution::store::open_sqlite(&path).unwrap();
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute("INSERT INTO schema_migrations (version, applied_at) VALUES ('9.0.001_future', unixepoch() + 1)", []).unwrap();

    let output = run_lf(home.path(), &["self", "doctor", "--json"]);
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["store"]["migration_error"].is_null(), "{report}");

    let output = run_lf(home.path(), &["wave", "list"]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let latest: String = connection
        .query_row(
            "SELECT version FROM schema_migrations ORDER BY applied_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(latest, "9.0.001_future");
}

#[test]
fn doctor_keeps_reporting_when_the_database_is_corrupt() {
    let home = TestRepo::new();
    let path = home.path().join("loopflow.db");
    fs::write(&path, b"not a sqlite database").unwrap();
    let output = run_lf(home.path(), &["self", "doctor", "--json"]);
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["store"]["migration_error"].is_string());
    assert!(report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| check["name"] == "store" && check["status"] == "fail"));
    assert_eq!(fs::read(path).unwrap(), b"not a sqlite database");
}
