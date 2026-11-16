use crate::session_record::{read_provider_session, SessionCaptureSpec, TerminalReceipt};
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
        .fail_and_begin_attempt("claude".to_string(), None, None)
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
    assert_eq!(attempts.len(), 2);
    assert_eq!(attempts[0].outcome.as_deref(), Some("failed"));
    assert_eq!(attempts[1].outcome.as_deref(), Some("completed"));
    assert!(attempts[0].started_at.is_some());
    assert_eq!(history.first_provider_attempt_at, attempts[0].started_at);
}

#[test]
fn each_attempt_publishes_its_conversation_with_the_account_it_ran_on() {
    let _lock = crate::journal::test_env_lock();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
    let home = tempfile::tempdir().unwrap();
    let capture = crate::session_record::SessionRecorder::begin_at(home.path(), spec(home.path()))
        .map(crate::process::LfProcessRuntime::new)
        .unwrap();
    let dir = capture.artifact_dir();
    let first = crate::store::ProviderAccountId::parse("primary").unwrap();
    let second = crate::store::ProviderAccountId::parse("fallback").unwrap();
    capture.mark_spawn_requested();
    capture.observe_provider(None, Some(first.clone()));
    assert!(read_provider_session(&dir).unwrap().is_none());

    capture.observe_provider(Some("first-session".into()), Some(first.clone()));
    capture.observe_provider(Some("first-session".into()), Some(first.clone()));
    assert_eq!(
        read_provider_session(&dir).unwrap().unwrap().account_id,
        Some(first)
    );
    capture
        .0
        .lock()
        .unwrap()
        .fail_and_begin_attempt("proof".into(), None, Some(second.clone()))
        .unwrap();
    capture.observe_provider(Some("second-session".into()), Some(second.clone()));
    let recovered = read_provider_session(&dir).unwrap().unwrap();
    assert_eq!(recovered.agent_session, "second-session".into());
    assert_eq!(recovered.account_id, Some(second));
    capture
        .0
        .lock()
        .unwrap()
        .fail_and_begin_attempt("proof".into(), None, None)
        .unwrap();
    capture.observe_provider(Some("ambient-session".into()), None);
    capture.finish("completed").unwrap();

    let ambient = read_provider_session(&dir).unwrap().unwrap();
    assert_eq!(ambient.agent_session, "ambient-session".into());
    assert_eq!(ambient.account_id, None);
    assert!(!dir.join("provider-session.json").exists());
    // The published reference is the only record of which conversation and
    // account each attempt used; history keeps no second copy.
    let types: Vec<_> = crate::session_record::row_store(&dir)
        .unwrap()
        .input_events(&capture.artifact_key())
        .unwrap()
        .into_iter()
        .map(|event| event["type"].clone())
        .collect();
    assert!(!types
        .iter()
        .any(|kind| kind == "provider_account_selected" || kind == "provider_session_observed"));
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
