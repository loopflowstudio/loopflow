//! `lf status` is an audit surface, so its contract is user-facing: the JSON it
//! promises must be the JSON it emits, and the wave you are standing in must be
//! the wave it reports. Drives the real binary against a seeded `LF_HOME`.

use std::path::Path;
use std::process::Command;

use loopflow::id::WaveId;
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::RunEventRow;
use loopflow::trace::{AgentLaunchRow, AgentTurnRow};
use loopflow::wave::Wave;

/// A machine home holding one wave with lookup noise, a flow, and a skill. The
/// registry and the ledgers are the same database.
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
        process_id: "proc-lookup".to_string(),
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

    let mut flow_start = event(1, now - 50, "started");
    flow_start.run_id = "run-flow".to_string();
    flow_start.process_id = "proc-flow".to_string();
    flow_start.node = "flow".to_string();
    flow_start.command = Some(r#"["lf","build"]"#.to_string());
    flow_start.flow = Some("build".to_string());
    store
        .insert_run_event(&flow_start)
        .expect("seed flow start");
    let mut flow_end = flow_start.clone();
    flow_end.seq = 2;
    flow_end.ts = now - 40;
    flow_end.event = "completed".to_string();
    store.insert_run_event(&flow_end).expect("seed flow end");

    let launch = AgentLaunchRow {
        id: "launch-wave-mutate".to_string(),
        run_id: "run-resident".to_string(),
        process_id: "proc-resident".to_string(),
        started_at: now - 30,
        ended_at: Some(now - 20),
        repo: home.join("repo").display().to_string(),
        worktree: home.join("repo").display().to_string(),
        wave: Some(wave_name.to_string()),
        flow: Some("wave".to_string()),
        skill: Some("wave_mutate".to_string()),
        provider: "codex".to_string(),
        model: Some("gpt-5".to_string()),
        surface: "headless".to_string(),
        capture_status: "complete".to_string(),
        incomplete_reason: None,
        outcome: "completed".to_string(),
        artifact_dir: "traces/launch-wave-mutate".to_string(),
        conversation_path: "traces/launch-wave-mutate/conversation.jsonl".to_string(),
        provider_events_path: None,
        provider_session_id: None,
        provider_session_path: None,
        conversation_event_count: 2,
        conversation_bytes: 10,
    };
    let turn = AgentTurnRow {
        id: "turn-wave-mutate".to_string(),
        launch_id: launch.id.clone(),
        ordinal: 1,
        provider_turn_id: None,
        started_at: now - 30,
        ended_at: Some(now - 20),
        status: "completed".to_string(),
        input_op: "initial".to_string(),
        context_coverage: "assembled".to_string(),
        tokenizer: "o200k_base".to_string(),
        system_prompt_path: None,
        task_prompt_path: "traces/launch-wave-mutate/task.md".to_string(),
        system_tokens: 0,
        task_tokens: 10,
        supplied_context_tokens: 10,
        provider_input_tokens: Some(10),
        provider_output_tokens: Some(5),
        reasoning_tokens: None,
        cache_read_tokens: Some(0),
        cache_write_tokens: None,
        cost_usd: Some(0.01),
        context_gather_ms: 1,
        context_render_ms: 1,
        context_persist_ms: 1,
        first_event_seq: None,
        last_event_seq: None,
    };
    store
        .insert_trace_capture(&launch, &turn, &[], &[])
        .expect("seed skill launch");
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
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
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

fn status_human(home: &Path, wave: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["status", wave])
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .output()
        .expect("lf status runs");
    assert!(
        output.status.success(),
        "lf status failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("status is utf8")
}

fn execs_json(home: &Path) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["execs", "--json"])
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .output()
        .expect("lf execs runs");
    assert!(
        output.status.success(),
        "lf execs failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("lf execs emits JSON")
}

fn runs_json(home: &Path) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["runs", "--json"])
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .output()
        .expect("lf runs runs");
    assert!(
        output.status.success(),
        "lf runs failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("lf runs emits JSON")
}

fn trace_json(home: &Path, exec_id: &str) -> serde_json::Value {
    let output = Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(["trace", exec_id, "--json"])
        .env("LF_HOME", home)
        .env_remove("LF_DB_PATH")
        .output()
        .expect("lf trace runs");
    assert!(
        output.status.success(),
        "lf trace failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("lf trace emits JSON")
}

#[test]
fn status_reports_skill_runs_without_lookup_or_flow_processes() {
    let home = tempfile::tempdir().expect("tempdir");
    seed(home.path(), "audit-a");

    let status = status_json(home.path(), &["audit-a"], None);

    assert_eq!(status["wave"]["name"], "audit-a");
    assert_eq!(status["runs"]["state"], "ok");
    let runs = status["runs"]["items"].as_array().expect("runs array");
    assert_eq!(runs.len(), 1);
    let skill = runs
        .iter()
        .find(|run| run["skill"] == "wave_mutate")
        .expect("the wave's skill is in its status");
    assert_eq!(skill["flow"], "wave");
    assert_eq!(skill["provider"], "codex");
    assert_eq!(skill["supplied_context_tokens"], 10);
    assert_eq!(skill["input_tokens"], 10);
    assert_eq!(skill["output_tokens"], 5);

    let human = status_human(home.path(), "audit-a");
    assert!(human.contains("wave/wave_mutate"));
    assert!(human.contains("ctx      10"));
    assert!(human.contains("tok      15"));
    assert!(!human.contains("pm sync"));
    assert!(!human.contains("build"));

    // Nothing is waiting, and the snapshot says so — it does not omit the field.
    assert_eq!(status["attention"]["state"], "ok");
    assert_eq!(status["attention"]["items"], serde_json::json!([]));
}

#[test]
fn execs_keep_the_lookup_process_ledger() {
    let home = tempfile::tempdir().expect("tempdir");
    seed(home.path(), "audit-execs");

    let execs = execs_json(home.path());
    let execs = execs.as_array().expect("exec array");
    let lookup = execs
        .iter()
        .find(|exec| exec["label"] == "pm sync")
        .expect("lookup stays available as an exec");
    assert_eq!(lookup["status"], "ok");
    assert_eq!(lookup["trace_id"], "run-1");

    let trace = trace_json(home.path(), "proc-lookup");
    assert_eq!(trace["spans"][0]["process_id"], "proc-lookup");
}

#[test]
fn runs_are_skill_launches_with_context_and_token_evidence() {
    let home = tempfile::tempdir().expect("tempdir");
    seed(home.path(), "audit-runs");

    let runs = runs_json(home.path());
    let runs = runs.as_array().expect("run array");
    assert_eq!(runs.len(), 1);
    let run = &runs[0];
    assert_eq!(run["id"], "launch-wave-mutate");
    assert_eq!(run["trace_id"], "run-resident");
    assert_eq!(run["exec_id"], "proc-resident");
    assert_eq!(run["skill"], "wave_mutate");
    assert_eq!(run["supplied_context_tokens"], 10);
    assert_eq!(run["input_tokens"], 10);
    assert_eq!(run["output_tokens"], 5);
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
