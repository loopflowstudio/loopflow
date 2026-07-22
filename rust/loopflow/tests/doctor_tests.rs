use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use loopflow::store::sqlite::SqliteStore;
use loopflow::trace::{
    AgentInvocationRow, AgentTurnRow, RecordedConversationEvent, RecordedConversationPayload,
};
use time::OffsetDateTime;

fn run_lf(home: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_lf"))
        .args(args)
        .current_dir(home)
        .env("LF_HOME", home)
        .env("LF_DB_PATH", home.join("loopflow.db"))
        .env("NO_COLOR", "1")
        .env_remove("LF_CONTROL_HOME")
        .env_remove("LF_CONTROL_DB_PATH")
        .env_remove("LF_TRACE_ID")
        .env_remove("LF_PROCESS_ID")
        .env_remove("LF_WAVE_ID")
        .env_remove("LF_RUN_CONTEXT")
        .env_remove("LF_RUN_LEASE")
        .env_remove("LF_AGENT_INVOCATION_ID")
        .output()
        .unwrap()
}

fn capture_check(output: &Output) -> serde_json::Value {
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    report["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|check| check["name"] == "capture")
        .unwrap()
        .clone()
}

fn text_check_detail<'a>(output: &'a Output, mark: &str, name: &str) -> &'a str {
    let prefix = format!("{mark}  {name:<13} ");
    std::str::from_utf8(&output.stdout)
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .unwrap()
}

fn normalize_storage_snapshot(detail: &str) -> String {
    let Some((prefix, storage_and_metrics)) = detail.split_once("; storage ") else {
        return detail.to_string();
    };
    let (_, metrics) = storage_and_metrics.split_once("; ").unwrap();
    format!("{prefix}; storage <snapshot>; {metrics}")
}

fn insert_capture(
    store: &SqliteStore,
    home: &Path,
    id: &str,
    ended_at: i64,
    capture_status: &str,
    incomplete_reason: Option<&str>,
) -> (AgentInvocationRow, std::path::PathBuf) {
    let artifact_dir = format!("run-{id}/process-{id}/{id}");
    let conversation_path = format!("{artifact_dir}/conversation.jsonl");
    let task_prompt_path = format!("{artifact_dir}/0001-task.md");
    let absolute_artifact_dir = home.join("traces").join(&artifact_dir);
    let absolute_conversation_path = home.join("traces").join(&conversation_path);
    fs::create_dir_all(&absolute_artifact_dir).unwrap();
    fs::write(home.join("traces").join(&task_prompt_path), "task\n").unwrap();
    let event = RecordedConversationEvent {
        schema_version: 1,
        seq: 1,
        ts: OffsetDateTime::from_unix_timestamp(ended_at).unwrap(),
        turn_id: None,
        payload: RecordedConversationPayload::LegacyText {
            stream: "assistant".to_string(),
            text: "captured".to_string(),
        },
    };
    let mut conversation = serde_json::to_vec(&event).unwrap();
    conversation.push(b'\n');
    fs::write(&absolute_conversation_path, &conversation).unwrap();

    let invocation = AgentInvocationRow {
        id: id.to_string(),
        run_id: format!("run-{id}"),
        answer_ask_id: None,
        process_id: format!("process-{id}"),
        started_at: ended_at - 1,
        ended_at: Some(ended_at),
        repo: "/src/loopflow".to_string(),
        worktree: "/src/loopflow.keep-daily-telemetry-actionable-after".to_string(),
        wave: Some("infrastructure".to_string()),
        flow: None,
        skill: Some("implement".to_string()),
        project: Some("stability-security".to_string()),
        task: Some("LOO-219".to_string()),
        provider: "codex".to_string(),
        model: Some("gpt-5".to_string()),
        surface: "headless".to_string(),
        capture_status: capture_status.to_string(),
        incomplete_reason: incomplete_reason.map(str::to_string),
        outcome: "completed".to_string(),
        artifact_dir,
        conversation_path,
        provider_events_path: None,
        provider_session_id: None,
        provider_session_path: None,
        conversation_event_count: 1,
        conversation_bytes: conversation.len() as i64,
        supervision: None,
    };
    let turn = AgentTurnRow {
        id: format!("turn-{id}"),
        invocation_id: id.to_string(),
        ordinal: 1,
        provider_turn_id: None,
        started_at: ended_at - 1,
        ended_at: Some(ended_at),
        status: "completed".to_string(),
        input_op: "initial".to_string(),
        context_coverage: "unknown".to_string(),
        tokenizer: "cl100k_base".to_string(),
        system_prompt_path: None,
        task_prompt_path,
        system_tokens: 0,
        task_tokens: 0,
        supplied_context_tokens: 0,
        provider_input_tokens: None,
        provider_total_input_tokens: None,
        peak_input_tokens: None,
        context_window_tokens: None,
        provider_output_tokens: None,
        reasoning_tokens: None,
        cache_read_tokens: None,
        cache_write_tokens: None,
        cost_usd: None,
        context_gather_ms: 0,
        context_render_ms: 0,
        context_persist_ms: 0,
        first_event_seq: Some(1),
        last_event_seq: Some(1),
        root_output: Some("captured".to_string()),
        basis: None,
    };
    store
        .insert_trace_capture(&invocation, &turn, &[], &[])
        .unwrap();
    (invocation, absolute_conversation_path)
}

#[test]
fn doctor_json_reports_the_build_revision_and_freshness_check() {
    let home = tempfile::tempdir().unwrap();
    let output = run_lf(home.path(), &["doctor", "--json"]);
    assert!(
        output.status.success(),
        "lf doctor failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

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
fn doctor_formats_accept_recovered_history_and_reject_a_recurrence() {
    let home = tempfile::tempdir().unwrap();
    let store = SqliteStore::new(&home.path().join("loopflow.db")).unwrap();
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let (historical, historical_file) = insert_capture(
        &store,
        home.path(),
        "historical-loss",
        now - 100 * 3600,
        "partial",
        Some("No space left on device"),
    );
    insert_capture(
        &store,
        home.path(),
        "recovery",
        now - 49 * 3600,
        "complete",
        None,
    );
    let historical_bytes = fs::read(&historical_file).unwrap();

    let text = run_lf(home.path(), &["doctor"]);
    assert!(
        text.status.success(),
        "recovered text doctor failed: {}{}",
        String::from_utf8_lossy(&text.stdout),
        String::from_utf8_lossy(&text.stderr)
    );
    let json = run_lf(home.path(), &["doctor", "--json"]);
    assert!(json.status.success());
    let recovered = capture_check(&json);
    assert_eq!(recovered["status"], "ok");
    let recovered_detail = recovered["detail"].as_str().unwrap();
    assert!(recovered_detail.contains("capture recovered"));
    assert!(recovered_detail.contains("1 partial capture(s)"));
    assert_eq!(
        text_check_detail(&text, "ok  ", "capture"),
        recovered_detail
    );

    let historical_after = store
        .agent_invocations_since(0)
        .unwrap()
        .into_iter()
        .find(|invocation| invocation.id == historical.id)
        .unwrap();
    assert_eq!(historical_after, historical);
    assert_eq!(fs::read(&historical_file).unwrap(), historical_bytes);

    let (recurrence, _) = insert_capture(
        &store,
        home.path(),
        "recurring-loss",
        OffsetDateTime::now_utc().unix_timestamp(),
        "partial",
        Some("No space left on device"),
    );
    let text = run_lf(home.path(), &["doctor"]);
    assert!(!text.status.success());
    let json = run_lf(home.path(), &["doctor", "--json"]);
    assert!(!json.status.success());
    let recurring = capture_check(&json);
    assert_eq!(recurring["status"], "fail");
    let recurring_detail = recurring["detail"].as_str().unwrap();
    for expected in [
        "capture active loss",
        "task LOO-219",
        recurrence.id.as_str(),
        "via codex",
        "No space left on device",
        "storage",
        "available",
        ".lf",
        "traces",
    ] {
        assert!(
            recurring_detail.contains(expected),
            "missing {expected:?}: {recurring_detail}"
        );
    }
    let text_detail = text_check_detail(&text, "FAIL", "capture");
    for expected in ["storage", "available", ".lf", "traces"] {
        assert!(
            text_detail.contains(expected),
            "missing {expected:?}: {text_detail}"
        );
    }
    assert_eq!(
        normalize_storage_snapshot(text_detail),
        normalize_storage_snapshot(recurring_detail)
    );
}
