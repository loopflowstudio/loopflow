use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use loopflow::process::LfProcess;
use loopflow::store::sqlite::SqliteStore;
use time::OffsetDateTime;

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

fn insert_process(store: &SqliteStore, _id: &str, ts: i64) {
    store
        .record_process(&LfProcess {
            kind: loopflow::process::ProcessKind::Lf,
            agent_session_id: None,
            os_started_at: None,
            id: loopflow::id::LfProcessId::new(),
            pid: None,
            trace_id: loopflow::id::TraceId::new(),
            parent_lf_process_id: None,
            // A finished root nothing references is forgotten at exit; agent-issued
            // history is what a copied store retains.
            via_agent: Some(true),
            caller_session_id: None,
            caller_agent_process_id: None,
            command: Some(r#"["lf","graph","telemetry-daily"]"#.into()),
            repo: Some("/src/loopflow".into()),
            cwd: None,
            started_at: ts,
            completed_at: Some(ts),
            outcome: Some("succeeded".into()),
            exit_code: Some(0),
            signal: None,
            error: None,
        })
        .unwrap();
}

#[test]
fn doctor_json_reports_the_build_revision_and_freshness_check() {
    let home = TestRepo::new();
    SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
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
fn copied_production_history_does_not_block_the_telemetry_cron() {
    let home = TestRepo::new();
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    insert_process(
        &store,
        "august-03",
        OffsetDateTime::parse(
            "2026-08-03T12:00:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap()
        .unix_timestamp(),
    );
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let mut timestamp = OffsetDateTime::parse(
        "2026-08-12T12:00:00Z",
        &time::format_description::well_known::Rfc3339,
    )
    .unwrap()
    .unix_timestamp();
    let mut ordinal = 12;
    while timestamp <= now {
        insert_process(&store, &format!("after-gap-{ordinal}"), timestamp);
        timestamp += 86_400;
        ordinal += 1;
    }
    let original_events = store.processes_since(0).unwrap();
    fs::create_dir_all(home.path().join(".lf/graphs")).unwrap();
    fs::write(
        home.path().join(".lf/graphs/telemetry-daily.yaml"),
        "- cmd: doctor\n",
    )
    .unwrap();

    let doctor = run_lf(home.path(), &["doctor", "--json"]);
    assert!(
        doctor.status.success(),
        "lf self doctor failed: {}{}",
        String::from_utf8_lossy(&doctor.stdout),
        String::from_utf8_lossy(&doctor.stderr)
    );

    let telemetry = run_lf(home.path(), &["--batch", "graph", "telemetry-daily"]);
    assert!(
        telemetry.status.success(),
        "telemetry-daily failed: {}{}",
        String::from_utf8_lossy(&telemetry.stdout),
        String::from_utf8_lossy(&telemetry.stderr)
    );
    let events_after_telemetry = store.processes_since(0).unwrap();
    for original in original_events {
        assert!(events_after_telemetry.contains(&original));
    }
}

#[test]
fn doctor_accepts_machine_commands_without_a_repository() {
    let home = TestRepo::new();
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    insert_process(
        &store,
        "machine-command",
        OffsetDateTime::now_utc().unix_timestamp(),
    );
    let mut event = store.processes_since(0).unwrap().pop().unwrap();
    event.id = loopflow::id::LfProcessId::new();
    event.repo = None;
    event.command = Some("lf help".to_string());
    store.record_process(&event).unwrap();

    let output = run_lf(home.path(), &["self", "doctor", "--json"]);
    assert!(
        output.status.success(),
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let identity = report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == "identity")
        .unwrap();
    assert_eq!(identity["status"], "ok");
    assert!(identity["detail"]
        .as_str()
        .unwrap()
        .contains("1 Process(s) without repository scope"));
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

#[test]
fn doctor_reports_processes_despite_an_unknown_migration() {
    let home = TestRepo::new();
    let path = home.path().join("loopflow.db");
    let store = SqliteStore::new(&path).unwrap();
    insert_process(&store, "recent", OffsetDateTime::now_utc().unix_timestamp());
    let connection = rusqlite::Connection::open(&path).unwrap();
    connection.execute("INSERT INTO schema_migrations (version, applied_at) VALUES ('9.0.001_future', unixepoch() + 1)", []).unwrap();
    let output = run_lf(home.path(), &["self", "doctor", "--json"]);
    assert!(!output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(report["store"]["migration_error"]
        .as_str()
        .unwrap()
        .contains("9.0.001_future"));
    assert!(report["rows"].as_u64().unwrap() >= 1);
    assert!(report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .any(|check| check["name"] == "attribution" && check["status"] == "ok"));
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
        .any(|check| check["name"] == "processes" && check["status"] == "fail"));
    assert_eq!(fs::read(path).unwrap(), b"not a sqlite database");
}
