//! `lf status` is an audit surface, so its contract is user-facing: the JSON it
//! promises must be the JSON it emits, and the wave you are standing in must be
//! the wave it reports. Drives the real binary against a seeded `LF_HOME`.

use std::path::Path;
use std::process::Command;

use loopflow::id::WaveId;
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::RunEventRow;
use loopflow::wave::Wave;

/// A machine home holding one wave with one finished run in the ledger. The
/// registry and the ledger are the same database.
fn seed(home: &Path, wave_name: &str) -> Wave {
    std::fs::create_dir_all(home).expect("home");
    let db = home.join("loopflow.db");
    let store = SqliteStore::new(&db).expect("open store");
    let wave = Wave::new(
        WaveId::new(),
        wave_name.to_string(),
        home.join("repo").display().to_string(),
    );
    store.create_wave(&wave).expect("register wave");

    let now = chrono::Utc::now().timestamp();
    let event = |seq: i64, ts: i64, event: &str| RunEventRow {
        run_id: "run-1".to_string(),
        process_id: "proc-1".to_string(),
        parent_process_id: None,
        seq,
        ts,
        repo: Some(home.join("repo").display().to_string()),
        worktree: None,
        wave: Some(wave_name.to_string()),
        node: "run".to_string(),
        event: event.to_string(),
        command: Some(r#"["lf","pm","sync"]"#.to_string()),
        flow: None,
        skill: None,
        step_index: None,
        error: None,
        input_tokens: None,
        output_tokens: None,
        cache_read_tokens: None,
        cost_usd: None,
        duration_secs: None,
        provider: None,
        model: None,
    };
    store
        .insert_run_event(&event(0, now - 120, "started"))
        .expect("seed run start");
    store
        .insert_run_event(&event(1, now - 60, "completed"))
        .expect("seed run end");
    wave
}

/// `lf status --json` in a clean environment, optionally standing inside a wave.
fn status_json(home: &Path, args: &[&str], ambient_wave_id: Option<&str>) -> serde_json::Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_lf"));
    command
        .arg("status")
        .args(args)
        .arg("--json")
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .env_remove("LF_RUN_ID")
        .env_remove("LF_CHANNEL")
        .env_remove("LF_WAVE_ID");
    if let Some(id) = ambient_wave_id {
        command.env("LF_WAVE_ID", id);
    }
    let output = command.output().expect("lf status runs");
    assert!(
        output.status.success(),
        "lf status failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).expect("utf8");
    serde_json::from_str(stdout.trim()).unwrap_or_else(|err| panic!("not JSON: {err}\n{stdout}"))
}

#[test]
fn status_carries_the_runs_and_attention_it_promises() {
    let home = tempfile::tempdir().expect("tempdir");
    seed(home.path(), "audit-a");

    let status = status_json(home.path(), &["audit-a"], None);

    assert_eq!(status["wave"]["name"], "audit-a");
    assert_eq!(status["runs"]["state"], "ok");
    let runs = status["runs"]["items"].as_array().expect("runs array");
    let seeded = runs
        .iter()
        .find(|run| run["run_id"] == "run-1")
        .expect("the wave's run is in its status");
    assert_eq!(seeded["status"], "ok");
    assert_eq!(seeded["label"], "pm sync");

    // Nothing is waiting, and the snapshot says so — it does not omit the field.
    assert_eq!(status["attention"]["state"], "ok");
    assert_eq!(status["attention"]["items"], serde_json::json!([]));
}

/// The reproduced break: inside a resident wave, `LF_WAVE_ID` is a wave id, and
/// bare `lf status` read it as a name.
#[test]
fn ambient_wave_id_resolves_the_wave_it_names() {
    let home = tempfile::tempdir().expect("tempdir");
    let wave = seed(home.path(), "audit-b");

    let status = status_json(home.path(), &[], Some(wave.id().as_str()));

    assert_eq!(status["wave"]["id"], wave.id().as_str());
    assert_eq!(status["wave"]["name"], "audit-b");
    assert_eq!(status["runs"]["state"], "ok");
}

/// A wave that has done nothing reports an empty reading, not a missing one:
/// "we looked and found nothing" is a claim a client can trust.
#[test]
fn a_wave_with_no_runs_reports_an_empty_reading_not_a_missing_one() {
    let home = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(home.path()).expect("home");
    let store = SqliteStore::new(&home.path().join("loopflow.db")).expect("open store");
    let wave = Wave::new(
        WaveId::new(),
        "audit-c".to_string(),
        home.path().join("repo").display().to_string(),
    );
    store.create_wave(&wave).expect("register wave");

    let status = status_json(home.path(), &["audit-c"], None);

    assert_eq!(status["runs"]["state"], "ok");
    assert_eq!(status["runs"]["items"], serde_json::json!([]));
    assert_eq!(status["runs"]["truncated"], false);
}
