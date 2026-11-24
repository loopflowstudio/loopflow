use crate::session_record::{SessionCaptureSpec, TerminalReceipt};
use std::fs;
fn spec(cwd: &std::path::Path) -> SessionCaptureSpec {
    SessionCaptureSpec {
        harness: "proof".to_string(),
        model: Some("model".to_string()),
        surface: "headless".to_string(),
        cwd: cwd.to_path_buf(),
        repo: Some(cwd.to_path_buf()),
        worktree: Some(cwd.to_path_buf()),
        skill: Some("implement".to_string()),
        subjects: Vec::new(),
        graph: crate::session_record::SessionGraphMembership::Independent,
        work: None,
    }
}

#[test]
fn history_times_provider_work_from_its_first_recorded_attempt() {
    let home = tempfile::tempdir().unwrap();
    let capture = crate::session_record::SessionRecorder::begin_at(home.path(), spec(home.path()))
        .map(crate::process::LfProcessRuntime::new)
        .unwrap();
    capture.mark_spawn_requested();
    capture
        .0
        .lock()
        .unwrap()
        .fail_and_begin_attempt("claude".to_string(), None)
        .unwrap();
    capture.finish("completed").unwrap();

    let history = crate::store::sessions::input_history(
        &crate::session_record::row_store(&capture.artifact_dir()).unwrap(),
        capture.artifact_key().as_str(),
    )
    .unwrap();
    let attempts = history
        .providers
        .iter()
        .filter(|provider| {
            matches!(
                provider.reference,
                crate::session_record::ProviderHistoryReference::RecordedAttempt { .. }
            )
        })
        .collect::<Vec<_>>();
    // The completed retry says nothing its turn rows would not, so only the
    // failed attempt is kept.
    assert_eq!(attempts.len(), 1);
    assert_eq!(attempts[0].outcome.as_deref(), Some("failed"));
    assert!(attempts[0].started_at.is_some());
    assert_eq!(history.first_provider_attempt_at, attempts[0].started_at);
}

#[test]
fn an_interrupt_observer_keeps_execution_alive_until_it_drops() {
    let home = tempfile::tempdir().unwrap();
    let capture = crate::session_record::SessionRecorder::begin_at(home.path(), spec(home.path()))
        .map(crate::process::LfProcessRuntime::new)
        .unwrap();
    let dir = capture.artifact_dir();
    // Interrupt cleanup upgrades a weak reference while handling the exit.
    let observer = std::sync::Arc::downgrade(&capture.0).upgrade().unwrap();
    drop(capture);
    assert!(!dir.join("terminal.json").exists());
    drop(observer);
    let terminal: TerminalReceipt =
        serde_json::from_slice(&fs::read(dir.join("terminal.json")).unwrap()).unwrap();
    assert_eq!(terminal.outcome, "failed");
}

#[test]
fn a_panic_during_recording_still_settles_the_execution() {
    let home = tempfile::tempdir().unwrap();
    let capture = crate::session_record::SessionRecorder::begin_at(home.path(), spec(home.path()))
        .map(crate::process::LfProcessRuntime::new)
        .unwrap();
    let dir = capture.artifact_dir();
    let recording = capture.clone();
    assert!(std::panic::catch_unwind(move || {
        let _locked = recording.0.lock().unwrap();
        panic!("recording interrupted");
    })
    .is_err());
    drop(capture);
    let terminal: TerminalReceipt =
        serde_json::from_slice(&fs::read(dir.join("terminal.json")).unwrap()).unwrap();
    assert_eq!(terminal.outcome, "failed");
}

/// November 7: Fable ran out of usage credits, and the step went on with
/// Opus on the same account. That retry resumes the thread it saved.
#[test]
fn a_retry_on_another_model_keeps_its_thread() {
    let thread: loopflow_model::id::AgentSessionId = "thread".into();
    let other: loopflow_model::id::AgentSessionId = "other".into();
    assert!(super::retry_keeps_its_thread(
        Some(&thread),
        Some(&thread),
        false
    ));
    assert!(!super::retry_keeps_its_thread(Some(&thread), None, false));
    assert!(super::retry_keeps_its_thread(Some(&thread), None, true));
    assert!(!super::retry_keeps_its_thread(
        Some(&thread),
        Some(&other),
        true
    ));
}
