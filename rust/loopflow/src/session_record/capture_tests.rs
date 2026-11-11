use std::collections::BTreeMap;
use std::fs;

use crate::agent::AgentConfig;
use crate::chat::types::{ConversationEvent, TurnUsage};
use crate::process::LfProcessRuntime;
use crate::session_record::{
    read_provider_session, AgentProcessRequest, SessionCaptureManifest, SessionCaptureSpec,
    SubjectAttribution, TerminalReceipt,
};

#[test]
fn native_usage_reads_each_providers_counter_once_by_its_shape() {
    use crate::session::SessionEventKind::{Started, Usage};
    let event = |seq, thread: &str, kind, payload| crate::session::SessionEvent {
        seq,
        session_id: "session".into(),
        agent_session: Some(crate::id::AgentSessionId::from(thread)),
        provider_turn: Some("turn".into()),
        kind,
        agent_process_id: None,
        lf_process_id: None,
        task_id: None,
        wave_id: None,
        observed_at: 1,
        payload,
    };
    let tokens = |id: &str, input: u64, output: u64| {
        serde_json::json!({"provider":"opencode","message":{"id":id,
            "tokens":{"input":input,"output":output,"reasoning":0,"cache":{"read":0,"write":0}}}})
    };
    let codex = serde_json::json!({"inputTokens":30,"outputTokens":5,"cachedInputTokens":0,"reasoningOutputTokens":0});
    let history = vec![
        event(1, "pi", Started, serde_json::json!({})),
        event(
            2,
            "pi",
            Usage,
            serde_json::json!({"provider":"pi","usage":{"input_tokens":10},"final":false}),
        ),
        event(
            3,
            "pi",
            Usage,
            serde_json::json!({"provider":"pi","usage":{"input_tokens":20,"output_tokens":6},"final":true}),
        ),
        event(4, "opencode", Started, serde_json::json!({})),
        event(5, "opencode", Usage, tokens("m1", 3, 1)),
        event(6, "opencode", Usage, tokens("m2", 4, 2)),
        event(7, "codex", Started, serde_json::json!({})),
        event(
            8,
            "codex",
            Usage,
            serde_json::json!({"total":codex,"last":codex}),
        ),
    ];
    let (readings, unread) = crate::session_record::native_usage(&history);
    assert_eq!(unread, 0);
    let usage = crate::session_record::reduce_usage(&readings);
    assert_eq!((usage.streams, usage.final_streams, usage.gaps), (4, 3, 0));
    assert_eq!(usage.input_tokens, Some(20 + 3 + 4 + 30));
    assert_eq!(usage.output_tokens, Some(6 + 1 + 2 + 5));
}

#[test]
fn terminal_attachment_probe() {
    let Ok(expected) = std::env::var("LF_TEST_TERMINAL_ATTACHMENT") else {
        return;
    };
    assert_eq!(
        crate::session_record::current_terminal_id().as_deref(),
        (!expected.is_empty()).then_some(expected.as_str())
    );
}

#[test]
fn terminal_attachment_requires_the_original_pty() {
    use std::os::fd::FromRawFd;
    use std::process::{Command, Stdio};

    let mut master = -1;
    let mut slave = -1;
    let mut name = [0 as libc::c_char; 1024];
    // SAFETY: all output pointers are valid; null termios/winsize use defaults.
    assert_eq!(
        unsafe {
            libc::openpty(
                &mut master,
                &mut slave,
                name.as_mut_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        },
        0
    );
    // SAFETY: successful openpty returns two independently owned descriptors
    // and a NUL-terminated device name in the buffer.
    let (_master, slave, tty) = unsafe {
        (
            fs::File::from_raw_fd(master),
            fs::File::from_raw_fd(slave),
            std::ffi::CStr::from_ptr(name.as_ptr())
                .to_str()
                .unwrap()
                .to_owned(),
        )
    };
    for (expected_tty, attached) in [(tty.as_str(), true), ("/dev/another-terminal", false)] {
        for _ in 0..2 {
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "session_record::capture_tests::terminal_attachment_probe",
                    "--nocapture",
                ])
                .env("LF_TERMINAL_ID", "shell-one")
                .env("LF_TERMINAL_TTY", expected_tty)
                .env(
                    "LF_TEST_TERMINAL_ATTACHMENT",
                    if attached { "shell-one" } else { "" },
                )
                .stdin(Stdio::from(slave.try_clone().unwrap()))
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stdout)
            );
        }
    }
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "session_record::capture_tests::terminal_attachment_probe",
        ])
        .env("LF_TERMINAL_ID", "shell-one")
        .env("LF_TERMINAL_TTY", &tty)
        .env("LF_TEST_TERMINAL_ATTACHMENT", "")
        .stdin(Stdio::null())
        .output()
        .unwrap();
    assert!(output.status.success());
}

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
fn native_capture_replacement_retains_each_spawn_and_rejects_late_exit() {
    let ledger = crate::journal::TestLedgerGuard::new();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let home = ledger.home();
    crate::journal::with_runtime(home, &["lf".into(), "skill".into()], || {
        let capture = crate::session_record::SessionRecorder::begin_at(home, spec(home))
            .map(crate::process::LfProcessRuntime::new)?;
        capture.claim_session_attachment()?;
        let (session, first) = capture.session_attachment().unwrap();
        let invocation = crate::journal::current_lf_process_id().unwrap();
        assert_eq!(first.lf_process_id.as_ref(), Some(&invocation));
        assert_eq!(first.provider_lf_process_id, invocation);
        let store = crate::session_record::row_store(&capture.artifact_dir())?;
        let command = || {
            let mut command = std::process::Command::new("/bin/sh");
            command.env_clear().args(["-c", "exit 42"]);
            command
        };
        let (mut child, snapshot) = capture.spawn_native_agent(command())?;
        assert_eq!(snapshot, first);
        assert_eq!(child.wait()?.code(), Some(42));
        // Observed wait, not successful spawn or a finished capture, ends it.
        capture.record_native_agent_exit(&first)?;
        let ended = store.agent_process(&first.agent_process_id)?.unwrap();
        assert_eq!(ended.pid, Some(child.id()));
        assert!(ended.completed_at.is_some());

        let (mut child, snapshot) = capture.spawn_native_agent(command())?;
        let second = snapshot;
        assert_ne!(first.agent_process_id, second.agent_process_id);
        assert_eq!(second.lf_process_id.as_ref(), Some(&invocation));
        assert_eq!(second.provider_lf_process_id, invocation);
        for attachment in [&first, &second] {
            let process = store.agent_process(&attachment.agent_process_id)?.unwrap();
            assert_eq!(process.parent_lf_process_id.as_ref(), Some(&invocation));
        }
        assert_eq!(
            capture.session_attachment(),
            Some((session.clone(), second.clone()))
        );
        assert_eq!(child.wait()?.code(), Some(42));
        assert!(capture.record_native_agent_exit(&first).is_err());
        assert!(store
            .agent_process(&second.agent_process_id)?
            .unwrap()
            .completed_at
            .is_none());
        // The child exited but without its wait receipt another launch is refused.
        assert!(capture.spawn_native_agent(command()).is_err());
        capture.record_native_agent_exit(&second)?;
        assert_eq!(store.agent_process(&first.agent_process_id)?, Some(ended));
        capture.finish("completed")?;
        assert!(capture.spawn_native_agent(command()).is_err());
        assert!(store
            .session_attachment(&session)?
            .unwrap()
            .lf_process_id
            .is_none());
        Ok(())
    })
    .unwrap();
}

#[test]
#[cfg(unix)]
fn a_failed_terminal_receipt_still_closes_only_the_owned_provider() {
    use std::os::unix::process::CommandExt;

    let ledger = crate::journal::TestLedgerGuard::new();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let home = ledger.home();
    crate::journal::with_runtime(home, &["lf".into(), "skill".into()], || {
        for takeover in [false, true] {
            let capture = crate::session_record::SessionRecorder::begin_at(home, spec(home))
                .map(crate::process::LfProcessRuntime::new)?;
            capture.claim_session_attachment()?;
            capture.mark_spawn_requested();
            let (session, attachment) = capture.session_attachment().unwrap();
            let store = crate::session_record::row_store(&capture.artifact_dir())?;
            let mut command = std::process::Command::new("/bin/sleep");
            command.env_clear().arg("60").process_group(0);
            store.record_agent_process_launch(&session, &attachment, &command, false)?;
            let mut child = command.spawn()?;
            let pid = child.id();
            let birth = crate::journal::process_started_at(pid)?.unwrap();
            store.record_agent_process_identity(&session, &attachment, pid, birth)?;
            let replacement = takeover.then(|| {
                store
                    .claim_session_attachment(
                        &session,
                        Some(&attachment),
                        &store.test_lf_process(),
                        false,
                    )
                    .unwrap()
            });
            // A directory at the receipt path makes the write fail independently
            // of the store that owns the still-running provider.
            let receipt = capture.artifact_dir().join("terminal.json");
            fs::create_dir(&receipt)?;
            let result = capture.finish("interrupted");
            let life = crate::journal::process_identity_evidence(pid, birth);
            let current = store.session_attachment(&session);
            let _ = child.kill();
            let _ = child.wait();

            assert!(
                result.is_err(),
                "the missing receipt remains a caller error"
            );
            let current = current?.unwrap();
            if let Some(replacement) = replacement {
                assert_eq!(life, crate::journal::ProcessIdentityEvidence::Live);
                assert_eq!(current, replacement);
            } else {
                assert_eq!(life, crate::journal::ProcessIdentityEvidence::Dead);
                assert!(current.lf_process_id.is_none());
            }
            assert!(capture.finish("failed").is_err());
            assert!(capture
                .spawn_native_agent(std::process::Command::new("/bin/true"))
                .is_err());
            fs::remove_dir(&receipt)?;
            capture.finish("interrupted")?;
            let recorded: TerminalReceipt = serde_json::from_slice(&fs::read(&receipt)?)?;
            assert_eq!(recorded.outcome, "interrupted");
            let history = store.input_events(&capture.artifact_key())?;
            capture.finish("interrupted")?;
            assert_eq!(store.input_events(&capture.artifact_key())?, history);
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn inherited_capture_requires_a_recorded_owner_and_matching_payload() {
    let _lock = crate::journal::test_env_lock();
    let home = tempfile::tempdir().unwrap();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let _home = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
    std::env::set_var("LF_HOME", home.path());
    let capture = crate::session_record::SessionRecorder::begin_at(home.path(), spec(home.path()))
        .map(crate::process::LfProcessRuntime::new)
        .unwrap();
    let key = capture.artifact_key();
    std::env::set_var(crate::session_record::CAPTURE_KEY_ENV, &key);
    assert_eq!(
        crate::session_record::inherited_capture_key().unwrap(),
        Some(key.clone())
    );
    std::env::set_var(
        crate::session_record::CAPTURE_KEY_ENV,
        crate::session_record::new_artifact_key(),
    );
    assert!(crate::session_record::inherited_capture_key().is_err());
    std::env::set_var(crate::session_record::CAPTURE_KEY_ENV, "../another-home");
    assert!(crate::session_record::inherited_capture_key().is_err());
    std::env::set_var(crate::session_record::CAPTURE_KEY_ENV, &key);
    let path = capture.artifact_dir().join("manifest.json");
    let mut manifest = crate::session_record::read_manifest(&capture.artifact_dir()).unwrap();
    manifest.artifact_key = crate::session_record::new_artifact_key();
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
    assert!(crate::session_record::inherited_capture_key().is_err());
    fs::write(&path, b"invalid manifest").unwrap();
    assert!(crate::session_record::inherited_capture_key().is_err());
    fs::remove_file(path).unwrap();
    assert_eq!(
        crate::session_record::inherited_capture_key().unwrap(),
        Some(key)
    );
}

#[test]
fn prepared_run_keeps_its_recorded_pr_instead_of_the_launching_pr() {
    let _lock = crate::journal::test_env_lock();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
    let home = tempfile::tempdir().unwrap();
    let original = crate::work::task::TaskPrId::new();
    let mut step = crate::session_record::SessionGraphStep {
        task_id: Some(crate::work::task::TaskId::new()),
        task_pr_id: Some(original.clone()),
        invocation_id: "invocation".into(),
        graph: "feature".into(),
        step: "review".into(),
        node: Some("1".into()),
        key: Some(1),
        iterations: Some(Vec::new()),
    };
    let mut prepared = spec(home.path());
    prepared.graph = crate::session_record::SessionGraphMembership::Step(step.clone());
    let id =
        crate::session_record::SessionRecorder::prepare_at(home.path(), prepared, None).unwrap();
    let (dir, _) = crate::session_record::resolve_manifest(home.path(), id.as_str()).unwrap();
    step.task_pr_id = Some(crate::work::task::TaskPrId::new());
    let mut process = spec(home.path());
    process.graph = crate::session_record::SessionGraphMembership::Step(step);
    let context = crate::trace::PreparedTurnContext::from_prompts("system", "review");
    let capture = crate::session_record::SessionRecorder::start_prepared(
        home.path(),
        &id,
        process,
        &context,
        None,
    )
    .map(crate::process::LfProcessRuntime::new)
    .unwrap();
    capture.finish("completed").unwrap();

    let manifest = crate::session_record::read_manifest(&dir).unwrap();
    let Some(crate::session_record::SessionGraphMembership::Step(step)) = manifest.graph else {
        panic!("prepared membership retained");
    };
    assert_eq!(step.task_pr_id, Some(original));
}

#[test]
fn helper_capture_admits_a_headless_conversation_with_its_input_and_outcome() {
    let _guard = crate::journal::TestLedgerGuard::new();
    let home = tempfile::tempdir().unwrap();
    let process = AgentProcessRequest::from_prepared(&AgentConfig {
        task_prompt: "repair the failed operation".into(),
        ..Default::default()
    })
    .unwrap();
    let capture = crate::session_record::SessionRecorder::begin_at_with_request(
        home.path(),
        spec(home.path()),
        process,
    )
    .map(crate::process::LfProcessRuntime::new)
    .unwrap();
    let store = crate::session_record::row_store(&capture.artifact_dir()).unwrap();
    let session = store
        .session_for_artifact(&capture.artifact_key())
        .unwrap()
        .unwrap();
    let run = session.clone();
    assert!(!session.interactive);
    assert_eq!(session.title, "repair failed operation");
    assert_eq!(run.provider.as_deref(), Some("proof"));
    assert_eq!(
        crate::session_record::read_manifest(&capture.artifact_dir())
            .unwrap()
            .process
            .unwrap()
            .task_prompt,
        "repair the failed operation"
    );
    capture.finish("failed").unwrap();
    let after = store
        .session_for_artifact(&capture.artifact_key())
        .unwrap()
        .unwrap();

    assert_eq!(after.id, session.id);
    assert_eq!(
        crate::session_record::row_store(&capture.artifact_dir())
            .unwrap()
            .input_history(capture.artifact_key().as_str())
            .unwrap()
            .recorded_outcome
            .as_deref(),
        Some("failed")
    );
    assert!(after.completed_at.is_none());
}

#[test]
fn reserved_publication_recovers_artifacts_without_repeating_process_authority() {
    for boundary in [
        "before_artifacts",
        "context_staged",
        "manifest_staged",
        "artifacts_published",
    ] {
        let home = tempfile::tempdir().unwrap();
        let id = crate::session_record::new_artifact_key();
        let context = crate::trace::PreparedTurnContext::from_prompts("system", "review");
        let (manifest, bytes) = crate::session_record::prepare_manifest(
            spec(home.path()),
            id.clone(),
            None,
            None,
            Some(&context),
        )
        .unwrap();
        let dir = crate::session_record::record_dir(home.path(), &id).unwrap();
        let staging = dir.parent().unwrap().join(format!(".{id}.staging"));
        if boundary == "context_staged" || boundary == "manifest_staged" {
            std::fs::create_dir_all(&staging).unwrap();
            std::fs::write(staging.join("context.json"), bytes.as_ref().unwrap()).unwrap();
            if boundary == "manifest_staged" {
                std::fs::write(
                    staging.join("manifest.json"),
                    serde_json::to_vec_pretty(&manifest).unwrap(),
                )
                .unwrap();
            }
        }
        if boundary == "artifacts_published" {
            crate::session_record::publish_manifest(home.path(), &manifest, bytes.as_deref())
                .unwrap();
        }
        // Interrupt after artifacts, before SQL publication. There must be
        // no capture Drop receipt falsely settling the prepared capture.
        let denied = || {
            Err(crate::store::StoreError::InvalidAuthority(
                "interrupted publication".into(),
            ))
        };
        assert!(crate::session_record::SessionRecorder::begin_reserved_at(
            home.path(),
            spec(home.path()),
            id.clone(),
            None,
            None,
            &context,
            |_| denied()
        )
        .map(crate::process::LfProcessRuntime::new)
        .is_err());
        let original = std::fs::read(dir.join("manifest.json")).unwrap();
        assert!(!dir.join("terminal.json").exists());
        assert!(!dir.join("events.jsonl").exists());
        let capture = crate::session_record::SessionRecorder::begin_reserved_at(
            home.path(),
            spec(home.path()),
            id.clone(),
            None,
            None,
            &context,
            |_| Ok(()),
        )
        .map(crate::process::LfProcessRuntime::new)
        .unwrap();
        assert_eq!(capture.artifact_key(), id);
        assert_eq!(std::fs::read(dir.join("manifest.json")).unwrap(), original);
        assert_eq!(
            std::fs::read(dir.join("context.json")).unwrap(),
            bytes.unwrap()
        );
        // SQL has granted authority once, but no provider has started.
        // An absent provider receipt does not grant a second process.
        assert!(crate::session_record::SessionRecorder::begin_reserved_at(
            home.path(),
            spec(home.path()),
            id.clone(),
            None,
            None,
            &context,
            |_| denied()
        )
        .map(crate::process::LfProcessRuntime::new)
        .is_err());
        assert!(!dir.join("terminal.json").exists());
        capture.finish("interrupted").unwrap();
        let terminal = std::fs::read(dir.join("terminal.json")).unwrap();
        assert!(crate::session_record::SessionRecorder::begin_reserved_at(
            home.path(),
            spec(home.path()),
            id,
            None,
            None,
            &context,
            |_| Ok(())
        )
        .map(crate::process::LfProcessRuntime::new)
        .is_err());
        assert_eq!(std::fs::read(dir.join("terminal.json")).unwrap(), terminal);
    }
}

#[test]
fn reserved_publication_retains_conflicting_inputs() {
    let home = tempfile::tempdir().unwrap();
    let id = crate::session_record::new_artifact_key();
    let context = crate::trace::PreparedTurnContext::from_prompts("system", "review");
    let (manifest, bytes) = crate::session_record::prepare_manifest(
        spec(home.path()),
        id.clone(),
        None,
        None,
        Some(&context),
    )
    .unwrap();
    let dir =
        crate::session_record::publish_manifest(home.path(), &manifest, bytes.as_deref()).unwrap();
    let original = std::fs::read(dir.join("manifest.json")).unwrap();
    let changed = crate::trace::PreparedTurnContext::from_prompts("system", "different review");
    let error = crate::session_record::SessionRecorder::begin_reserved_at(
        home.path(),
        spec(home.path()),
        id.clone(),
        None,
        None,
        &changed,
        |_| Ok(()),
    )
    .map(crate::process::LfProcessRuntime::new)
    .unwrap_err();
    assert!(error
        .to_string()
        .contains("different immutable launch inputs"));
    assert_eq!(std::fs::read(dir.join("manifest.json")).unwrap(), original);
    assert_eq!(
        std::fs::read(dir.join("context.json")).unwrap(),
        bytes.unwrap()
    );
    assert!(!dir.join("terminal.json").exists());

    std::fs::write(dir.join("context.json"), b"interrupted write").unwrap();
    assert!(crate::session_record::SessionRecorder::begin_reserved_at(
        home.path(),
        spec(home.path()),
        id,
        None,
        None,
        &context,
        |_| Ok(())
    )
    .map(crate::process::LfProcessRuntime::new)
    .is_err());
    assert_eq!(
        std::fs::read(dir.join("context.json")).unwrap(),
        b"interrupted write"
    );
}

#[test]
fn prepared_session_run_is_resolvable_and_consumed_once() {
    let _lock = crate::journal::test_env_lock();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
    let home = tempfile::tempdir().unwrap();
    let caller = crate::session_record::new_artifact_key();
    let id = crate::session_record::SessionRecorder::prepare_at(
        home.path(),
        spec(home.path()),
        Some(caller.clone()),
    )
    .unwrap();
    let (dir, prepared) =
        crate::session_record::resolve_manifest(home.path(), id.as_str()).unwrap();
    assert_eq!(prepared.artifact_key, id);
    assert_eq!(prepared.caller_artifact_key.as_ref(), Some(&caller));
    // Recover a crash after the manifest was published but before the prepared marker.
    fs::remove_file(dir.join("prepared")).unwrap();
    assert_eq!(
        crate::session_record::SessionRecorder::prepare_at_with_key(
            home.path(),
            spec(home.path()),
            Some(caller.clone()),
            id.clone()
        )
        .unwrap(),
        id
    );
    assert_eq!(
        crate::session_record::read_manifest(&dir)
            .unwrap()
            .created_at,
        prepared.created_at
    );
    assert_eq!(
        crate::session_record::SessionRecorder::prepare_at_with_key(
            home.path(),
            spec(home.path()),
            Some(caller.clone()),
            id.clone()
        )
        .unwrap(),
        id
    );
    assert!(!dir.join("terminal.json").exists());
    assert!(!dir.join("provider-clients").exists());
    let context = crate::trace::PreparedTurnContext::from_prompts("system", "human prompt");
    let capture = crate::session_record::SessionRecorder::start_prepared(
        home.path(),
        &id,
        spec(home.path()),
        &context,
        None,
    )
    .map(crate::process::LfProcessRuntime::new)
    .unwrap();
    let launched = crate::session_record::read_manifest(&dir).unwrap();
    assert_eq!(capture.artifact_key(), id);
    assert!(crate::session_record::SessionRecorder::prepare_at_with_key(
        home.path(),
        spec(home.path()),
        Some(caller.clone()),
        id.clone()
    )
    .is_err());
    assert!(
        !dir.join("prepared").exists(),
        "recovery cannot rearm an already claimed launch"
    );
    assert_eq!(launched.created_at, prepared.created_at);
    assert_eq!(launched.caller_artifact_key, Some(caller));
    let context_ref = launched.context.as_ref().unwrap();
    let context_bytes = fs::read(dir.join(&context_ref.path)).unwrap();
    assert_eq!(context_ref.bytes, context_bytes.len() as u64);
    assert_eq!(
        context_ref.content_sha256,
        hex::encode(<sha2::Sha256 as sha2::Digest>::digest(&context_bytes))
    );
    assert!(crate::session_record::SessionRecorder::start_prepared(
        home.path(),
        &id,
        spec(home.path()),
        &context,
        None
    )
    .map(crate::process::LfProcessRuntime::new)
    .is_err());
    capture.mark_spawn_requested();
    capture.finish("completed").unwrap();
    assert_eq!(
        serde_json::from_slice::<crate::session_record::TerminalReceipt>(
            &fs::read(dir.join("terminal.json")).unwrap()
        )
        .unwrap()
        .outcome,
        "completed"
    );
    assert!(crate::session_record::record_dir(home.path(), &id)
        .unwrap()
        .is_dir());
}

#[test]
fn manifest_round_trips_the_exact_prepared_process_without_ambient_authority() {
    let home = tempfile::tempdir().unwrap();
    let config = AgentConfig {
        chrome: Some(true),
        codex_service_tier: Some(crate::config::CodexServiceTier::Default),
        system_prompt: "system context\r\nwith unicode λ\n \t".to_string(),
        task_prompt: "authored task\n\nwith trailing space ".to_string(),
        agent: Some("codex:gpt-5.6".to_string()),
        provider_account_id: Some(crate::store::ProviderAccountId::parse("engineering").unwrap()),
        max_turns: Some(7),
        resume_token: Some("resume-secret".into()),
        skip_permissions: true,
        directive_relay: Some("/tmp/directive-secret".into()),
        env: BTreeMap::from([("TOKEN".to_string(), "ambient-secret".to_string())]),
        ..AgentConfig::default()
    };
    let expected = AgentProcessRequest::from_prepared(&config).unwrap();
    let capture = crate::session_record::SessionRecorder::begin_at_with_request(
        home.path(),
        spec(home.path()),
        expected.clone(),
    )
    .map(crate::process::LfProcessRuntime::new)
    .unwrap();

    let bytes = fs::read(capture.artifact_dir().join("manifest.json")).unwrap();
    let saved: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(saved["artifact_key"], capture.artifact_key());
    assert!(saved.get("caller_artifact_key").is_some());
    assert!(saved.get("exec").is_some());
    assert!(saved.get("run_id").is_none());
    assert!(saved.get("launch").is_none());
    let manifest: SessionCaptureManifest = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(manifest.process, Some(expected));
    let context_ref = manifest.context.expect("manifest references exact context");
    let context = fs::read(capture.artifact_dir().join(&context_ref.path)).unwrap();
    assert_eq!(context_ref.bytes, context.len() as u64);
    assert_eq!(
        context_ref.content_sha256,
        hex::encode(<sha2::Sha256 as sha2::Digest>::digest(&context))
    );
    let context: serde_json::Value = serde_json::from_slice(&context).unwrap();
    assert_eq!(
        context.pointer("/context/system/text").unwrap(),
        "system context\r\nwith unicode λ\n \t"
    );
    assert_eq!(
        context.pointer("/context/task/text").unwrap(),
        "authored task\n\nwith trailing space "
    );
    // Interrupted/tampered context reconciliation is covered by
    // reserved_publication_retains_conflicting_inputs.
    assert!(bytes
        .windows(b"engineering".len())
        .any(|window| window == b"engineering"));
    assert!(!bytes
        .windows(b"resume-secret".len())
        .any(|window| window == b"resume-secret"));
    assert!(!bytes
        .windows(b"ambient-secret".len())
        .any(|window| window == b"ambient-secret"));
    assert!(!bytes
        .windows(b"directive-secret".len())
        .any(|window| window == b"directive-secret"));
}

#[test]
fn record_keeps_direct_usage_and_one_immutable_terminal_without_an_owner_claim() {
    let _lock = crate::journal::test_env_lock();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let _storage = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
    let home = tempfile::tempdir().unwrap();
    let capture = crate::session_record::SessionRecorder::begin_at(home.path(), spec(home.path()))
        .map(crate::process::LfProcessRuntime::new)
        .expect("publish Session capture manifest");
    let dir = capture.artifact_dir();
    let artifact_key = capture.artifact_key().to_string();

    assert!(dir.join("manifest.json").is_file());
    assert_eq!(
        dir.parent().and_then(|path| path.file_name()),
        Some(std::ffi::OsStr::new(
            &artifact_key.strip_prefix("run_").unwrap_or(&artifact_key)[..2]
        ))
    );
    assert!(!dir.join("inbox").exists());
    assert!(!dir.join("observations").exists());
    assert!(!dir.join("owner.json").exists());
    assert!(!dir.join("terminal.json").exists());

    capture.mark_spawn_requested();
    capture.record_conversation(ConversationEvent::TextDelta {
        turn_id: "provider-turn".to_string(),
        content: "provider response".to_string(),
    });
    capture.record_conversation(ConversationEvent::UsageCheckpoint {
        turn_id: "provider-turn".to_string(),
        usage: TurnUsage {
            input_tokens: Some(10),
            output_tokens: Some(4),
            ..TurnUsage::default()
        },
        final_receipt: false,
    });
    capture.finish("completed").expect("settle capture");

    let terminal = fs::read(dir.join("terminal.json")).unwrap();
    let terminal: TerminalReceipt = serde_json::from_slice(&terminal).unwrap();
    assert_eq!(terminal.outcome, "completed");
    let mut files = fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect::<Vec<_>>();
    files.sort();
    assert_eq!(
        files,
        ["manifest.json", "terminal.json"].map(std::ffi::OsString::from)
    );

    let error = capture.finish("failed").unwrap_err();
    assert!(error.to_string().contains("already settled as completed"));
    let unchanged: TerminalReceipt =
        serde_json::from_slice(&fs::read(dir.join("terminal.json")).unwrap()).unwrap();
    assert_eq!(unchanged.outcome, "completed");
}

#[test]
fn history_keeps_only_how_a_turn_ended() {
    let home = tempfile::tempdir().unwrap();
    let capture = crate::session_record::SessionRecorder::begin_at(home.path(), spec(home.path()))
        .map(crate::process::LfProcessRuntime::new)
        .unwrap();
    let turn = || "turn-1".to_string();
    capture.record_conversation(ConversationEvent::TurnStarted { turn_id: turn() });
    for word in ["streamed ", "one ", "token ", "at a time"] {
        capture.record_conversation(ConversationEvent::TextDelta {
            turn_id: turn(),
            content: word.to_string(),
        });
    }
    for diff in ["first", "first\nsecond"] {
        capture.record_conversation(ConversationEvent::DiffUpdated {
            turn_id: turn(),
            diff: diff.to_string(),
        });
    }
    capture.record_conversation(ConversationEvent::UsageCheckpoint {
        turn_id: turn(),
        usage: TurnUsage {
            input_tokens: Some(10),
            ..TurnUsage::default()
        },
        final_receipt: true,
    });
    capture.record_conversation(ConversationEvent::TurnCompleted {
        turn_id: turn(),
        status: crate::chat::types::Lifecycle::Completed,
    });
    capture.finish("completed").unwrap();

    // Capture prose remains readable, but cannot substitute for native turn output.
    assert_eq!(capture.final_answer().unwrap(), None);
    assert!(!capture.artifact_dir().join("events.jsonl").exists());

    let history = crate::session_record::row_store(&capture.artifact_dir())
        .unwrap()
        .input_events(&capture.artifact_key())
        .unwrap();
    let kept: Vec<_> = history
        .iter()
        .map(|event| match event["type"].as_str().unwrap() {
            "conversation" => event["event"]["type"].as_str().unwrap().to_string(),
            other => other.to_string(),
        })
        .collect();
    assert_eq!(kept, ["turn_completed"]);
}

#[test]
fn continuation_resumes_the_saved_thread_on_a_fresh_agent_process() {
    let ledger = crate::journal::TestLedgerGuard::new();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let original =
        crate::session_record::SessionRecorder::begin_at(ledger.home(), spec(ledger.home()))
            .map(crate::process::LfProcessRuntime::new)
            .unwrap();
    let store = crate::session_record::row_store(&original.artifact_dir()).unwrap();
    let session = store
        .session_for_artifact(&original.artifact_key())
        .unwrap()
        .unwrap();
    let command = vec!["lf".into(), "skill".into()];
    crate::journal::with_runtime(ledger.home(), &command, || {
        original.claim_session_attachment()?;
        let (_, attachment) = original.session_attachment().unwrap();
        store.record_session_connection(
            &session.id,
            &attachment,
            "/retained.sock",
            &"native-thread".into(),
        )?;
        // The previous provider is a throwaway child whose exit is observed
        // before continuation; a live one would refuse replacement.
        let mut provider = std::process::Command::new("/bin/sleep").arg("60").spawn()?;
        store.record_agent_process_identity(
            &session.id,
            &attachment,
            provider.id(),
            crate::journal::process_started_at(provider.id())?.unwrap(),
        )?;
        provider.kill()?;
        provider.wait()?;
        original.finish("completed")?;
        Ok(())
    })
    .unwrap();
    assert!(store
        .session(&session.id)
        .unwrap()
        .unwrap()
        .completed_at
        .is_some());
    let history = store.session_history(&session.id, 0, 0).unwrap();
    let attachment = store.session_attachment(&session.id).unwrap().unwrap();
    let manifest = fs::read(original.artifact_dir().join("manifest.json")).unwrap();
    let request = AgentProcessRequest::from_prepared(&AgentConfig {
        task_prompt: "the next instruction".into(),
        ..AgentConfig::default()
    })
    .unwrap();
    let context = crate::trace::PreparedTurnContext::from_prompts("", "the next instruction");
    crate::journal::with_runtime(ledger.home(), &command, || {
        let next = LfProcessRuntime::continue_with_context(
            &session.id,
            spec(ledger.home()),
            &context,
            request.clone(),
        )?;
        let (id, next_attachment) = next.session_attachment().unwrap();
        assert_eq!(id, session.id.as_str());
        assert_eq!(
            next.conversation_resume_token()?,
            Some("native-thread".into())
        );
        // The finished attachment's AgentProcess is never adopted.
        assert_ne!(
            next_attachment.agent_process_id,
            attachment.agent_process_id
        );
        assert_eq!(
            Some(&next_attachment.provider_lf_process_id),
            next_attachment.lf_process_id.as_ref()
        );
        assert!(store.session_connection(&session.id)?.is_none());
        assert!(store.agent_process_identity(&session.id)?.is_none());
        assert_ne!(next_attachment.token, attachment.token);
        let saved = crate::session_record::read_manifest(&next.artifact_dir()).unwrap();
        assert_eq!(
            serde_json::to_value(saved.process).unwrap(),
            serde_json::to_value(Some(&request)).unwrap()
        );
        assert!(saved.context.is_some());
        let current = store.session(&session.id)?.unwrap();
        assert!(current.completed_at.is_some());
        assert!(store
            .session_history(&session.id, 0, 0)?
            .starts_with(&history));
        assert_ne!(current.captured, session.captured);
        assert_eq!(current.cwd, session.cwd);
        assert_eq!(current.title, session.title);
        assert!(current.input_published);
        assert_eq!(current.task_id, session.task_id);
        assert_eq!(current.wave_id, session.wave_id);
        assert_eq!(
            store
                .session_for_artifact(&original.artifact_key())?
                .unwrap()
                .id,
            session.id
        );
        assert_eq!(
            fs::read(original.artifact_dir().join("manifest.json"))?,
            manifest
        );
        next.finish("failed")?;
        Ok(())
    })
    .unwrap();
}

#[test]
fn continuation_publication_failure_keeps_reservation_without_settling_a_turn() {
    let ledger = crate::journal::TestLedgerGuard::new();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let original =
        crate::session_record::SessionRecorder::begin_at(ledger.home(), spec(ledger.home()))
            .map(crate::process::LfProcessRuntime::new)
            .unwrap();
    let store = crate::session_record::row_store(&original.artifact_dir()).unwrap();
    let session = store
        .session_for_artifact(&original.artifact_key())
        .unwrap()
        .unwrap();
    fs::rename(
        ledger.home().join("runs"),
        ledger.home().join("retained-runs"),
    )
    .unwrap();
    fs::write(ledger.home().join("runs"), "unavailable capture directory").unwrap();
    let command = vec!["lf".into(), "skill".into()];
    crate::journal::with_runtime(ledger.home(), &command, || {
        assert!(LfProcessRuntime::continue_with_context(
            &session.id,
            spec(ledger.home()),
            &crate::trace::PreparedTurnContext::from_prompts("", "next"),
            AgentProcessRequest::from_prepared(&AgentConfig::default()).unwrap(),
        )
        .is_err());
        let next = store.session(&session.id)?.unwrap();
        assert_ne!(next.captured, session.captured);
        assert!(!next.input_published);
        assert!(next.completed_at.is_none());
        assert!(store
            .session_attachment(&session.id)?
            .unwrap()
            .lf_process_id
            .is_none());
        assert!(!store
            .session_history(&session.id, 0, 0)?
            .iter()
            .any(|event| {
                matches!(
                    event.kind,
                    crate::session::SessionEventKind::Started
                        | crate::session::SessionEventKind::Completed
                )
            }));
        Ok(())
    })
    .unwrap();
    fs::remove_file(ledger.home().join("runs")).unwrap();
    fs::rename(
        ledger.home().join("retained-runs"),
        ledger.home().join("runs"),
    )
    .unwrap();
}

/// A Session whose attached LfProcess is a throwaway child, so a test can kill it.
#[cfg(unix)]
fn attached_session(
    ledger: &crate::journal::TestLedgerGuard,
) -> (
    crate::store::sqlite::SqliteStore,
    crate::process::SessionAttachment,
    std::process::Child,
    crate::id::LfProcessId,
) {
    let path = ledger.home().join("loopflow.db");
    let store = crate::store::sqlite::SqliteStore::open_ephemeral(&path).unwrap();
    store.test_session("conversation", &crate::session_record::new_artifact_key());
    let first = crate::id::LfProcessId::new();
    let second = crate::id::LfProcessId::new();
    let sql = rusqlite::Connection::open(&path).unwrap();
    let trace = crate::id::TraceId::new();
    for process in [&first, &second] {
        sql.execute(
            "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,?2,1)",
            rusqlite::params![process, trace],
        )
        .unwrap();
    }
    sql.execute(
        "UPDATE agent_sessions SET provider='codex',interactive=0",
        [],
    )
    .unwrap();
    let process = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let receipts = ledger.home().join(crate::journal::PROCESS_RECEIPT_ROOT);
    fs::create_dir_all(&receipts).unwrap();
    fs::write(
        receipts.join(format!("{first}.json")),
        serde_json::to_vec(&crate::journal::ProcessReceipt {
            schema_version: 1,
            trace_id: trace.to_string(),
            lf_process_id: first.to_string(),
            pid: process.id(),
            started_at: crate::journal::process_started_at(process.id())
                .unwrap()
                .unwrap(),
        })
        .unwrap(),
    )
    .unwrap();
    let attachment = store
        .claim_session_attachment("conversation", None, &first, true)
        .unwrap();
    (store, attachment, process, second)
}

#[test]
#[cfg(unix)]
fn resume_dead_attachment_ends_its_agent_before_starting_another() {
    use std::os::unix::process::CommandExt;

    let ledger = crate::journal::TestLedgerGuard::new();
    let (store, attachment, mut process, next) = attached_session(&ledger);
    let mut agent_process = std::process::Command::new("sleep")
        .arg("30")
        .process_group(0)
        .spawn()
        .unwrap();
    let started = crate::journal::process_started_at(agent_process.id())
        .unwrap()
        .unwrap();
    store
        .record_agent_process_identity("conversation", &attachment, agent_process.id(), started)
        .unwrap();
    let socket = ledger.home().join("agent.sock");
    store
        .record_session_connection(
            "conversation",
            &attachment,
            socket.to_str().unwrap(),
            &"saved-thread".into(),
        )
        .unwrap();

    // A live attachment keeps both the conversation and its AgentProcess.
    let error =
        crate::process::resume_session_agent_process(&store, "conversation", &next).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Conversation already has an attached LfProcess; connect to it"),
        "{error}"
    );
    assert!(agent_process.try_wait().unwrap().is_none());

    process.kill().unwrap();
    process.wait().unwrap();
    let replacement =
        crate::process::resume_session_agent_process(&store, "conversation", &next).unwrap();
    assert_eq!(
        crate::journal::process_identity_evidence(agent_process.id(), started),
        crate::journal::ProcessIdentityEvidence::Dead,
    );
    // Ending the AgentProcess may already have reaped this exact child.
    let _ = agent_process.wait();
    assert_ne!(replacement.agent_process_id, attachment.agent_process_id);
    assert_eq!(replacement.provider_lf_process_id, next);
    assert!(store.session_connection("conversation").unwrap().is_none());
    assert!(store
        .agent_process_identity("conversation")
        .unwrap()
        .is_none());
    assert_eq!(
        store.session_thread("conversation").unwrap(),
        Some("saved-thread".into())
    );
}

#[test]
#[cfg(unix)]
fn resume_of_a_bare_conversation_starts_beside_its_running_agent() {
    use std::os::unix::process::CommandExt;

    let ledger = crate::journal::TestLedgerGuard::new();
    let (store, attachment, mut process, next) = attached_session(&ledger);
    // `lf <skill>` in another terminal: an interactive agent, no Work named,
    // no holder or owner, its lf still waiting.
    let sql = rusqlite::Connection::open(ledger.home().join("loopflow.db")).unwrap();
    sql.execute(
        "UPDATE agent_sessions SET provider='claude',interactive=1,work_source=NULL",
        [],
    )
    .unwrap();
    sql.execute(
        "UPDATE processes SET agent_interactive=1 WHERE id=?1",
        [&attachment.agent_process_id],
    )
    .unwrap();
    let mut agent = std::process::Command::new("sleep")
        .arg("30")
        .process_group(0)
        .spawn()
        .unwrap();
    let started = crate::journal::process_started_at(agent.id())
        .unwrap()
        .unwrap();
    store
        .record_agent_process_identity("conversation", &attachment, agent.id(), started)
        .unwrap();

    let beside =
        crate::process::resume_session_agent_process(&store, "conversation", &next).unwrap();
    let first_running = agent.try_wait().unwrap().is_none();
    let first_open: Option<i64> = sql
        .query_row(
            "SELECT completed_at FROM processes WHERE id=?1",
            [&attachment.agent_process_id],
            |row| row.get(0),
        )
        .unwrap();
    agent.kill().unwrap();
    agent.wait().unwrap();
    process.kill().unwrap();
    process.wait().unwrap();

    assert!(first_running, "the first agent keeps running");
    assert_eq!(first_open, None, "a running agent is not recorded as ended");
    assert_ne!(beside.agent_process_id, attachment.agent_process_id);
    assert_eq!(beside.lf_process_id, Some(next));
}

#[test]
#[cfg(unix)]
fn resume_refuses_live_agent_that_cannot_be_ended() {
    let ledger = crate::journal::TestLedgerGuard::new();
    let (store, attachment, mut process, next) = attached_session(&ledger);
    store
        .record_session_connection(
            "conversation",
            &attachment,
            ledger.home().join("absent.sock").to_str().unwrap(),
            &"saved-thread".into(),
        )
        .unwrap();
    // A provider outside a group of its own is never signalled.
    let mut provider = std::process::Command::new("sleep")
        .arg("30")
        .spawn()
        .unwrap();
    let started = crate::journal::process_started_at(provider.id())
        .unwrap()
        .unwrap();
    store
        .record_agent_process_identity("conversation", &attachment, provider.id(), started)
        .unwrap();
    process.kill().unwrap();
    process.wait().unwrap();

    let error =
        crate::process::resume_session_agent_process(&store, "conversation", &next).unwrap_err();
    let remained_alive = provider.try_wait().unwrap().is_none();
    let retained_attachment = store.session_attachment("conversation").unwrap();
    provider.kill().unwrap();
    provider.wait().unwrap();
    assert!(
        error
            .to_string()
            .contains("does not lead its own process group"),
        "{error}"
    );
    assert!(remained_alive);
    assert_eq!(retained_attachment, Some(attachment.clone()));

    let replacement =
        crate::process::resume_session_agent_process(&store, "conversation", &next).unwrap();
    assert_ne!(replacement.agent_process_id, attachment.agent_process_id);
}

#[test]
fn agent_session_identity_is_durable_before_the_provider_starts() {
    let home = tempfile::tempdir().unwrap();
    let capture = crate::session_record::SessionRecorder::begin_at(home.path(), spec(home.path()))
        .map(crate::process::LfProcessRuntime::new)
        .unwrap();
    capture.observe_provider(Some("provider-session".into()), None);

    assert_eq!(
        read_provider_session(&capture.artifact_dir())
            .unwrap()
            .map(|session| session.agent_session),
        Some("provider-session".into())
    );
}

#[test]
fn provider_session_preserves_the_selected_account() {
    let home = tempfile::tempdir().unwrap();
    let capture = crate::session_record::SessionRecorder::begin_at(home.path(), spec(home.path()))
        .map(crate::process::LfProcessRuntime::new)
        .unwrap();
    let account_id = crate::store::ProviderAccountId::parse("primary").unwrap();
    crate::session_record::write_provider_session(
        &capture.artifact_dir(),
        &"provider-session".into(),
        Some(account_id.clone()),
    )
    .unwrap();

    fs::remove_dir_all(capture.artifact_dir()).unwrap();
    let session = read_provider_session(&capture.artifact_dir())
        .unwrap()
        .expect("provider session reference");
    assert_eq!(session.agent_session, "provider-session".into());
    assert_eq!(session.account_id, Some(account_id));
}

#[test]
fn recording_failure_cannot_forget_the_thread_or_account_used_by_retry() {
    let ledger = crate::journal::TestLedgerGuard::new();
    let _ambient = crate::test_ambient::EnvGuard::new();
    let home = ledger.home();
    crate::journal::with_runtime(home, &["lf".into(), "skill".into()], || {
        // Exercise failure both before and after the account fact is saved,
        // including a stale persisted thread after a provider-side switch.
        for (source, failover) in ["events.jsonl:%", "provider-session:%"]
            .into_iter()
            .flat_map(|source| [false, true].map(|failover| (source, failover)))
        {
            let capture = crate::session_record::SessionRecorder::begin_at(home, spec(home))
                .map(crate::process::LfProcessRuntime::new)?;
            capture.claim_session_attachment()?;
            let primary = crate::store::ProviderAccountId::parse("primary").unwrap();
            let fallback = crate::store::ProviderAccountId::parse("fallback").unwrap();
            let thread = crate::id::AgentSessionId::from("observed-thread");
            let sql = rusqlite::Connection::open(crate::session_record::row_database(
                &capture.artifact_dir(),
            )?)?;
            let (session, original) = capture.session_attachment().unwrap();
            sql.execute(
                "UPDATE agent_sessions SET provider_thread='previous-thread' WHERE id=?1",
                [&session],
            )?;
            sql.execute_batch(&format!(
                "CREATE TRIGGER reject_observation BEFORE INSERT ON session_events
                 WHEN NEW.kind='observed' AND json_extract(NEW.payload,'$.source') LIKE '{source}'
                 BEGIN SELECT RAISE(FAIL, 'observation unavailable'); END;"
            ))?;
            capture.observe_provider(Some(thread.clone()), Some(primary.clone()));
            sql.execute_batch("DROP TRIGGER reject_observation")?;
            let store = crate::session_record::row_store(&capture.artifact_dir())?;
            store.record_agent_process_exit(&session, &original, false)?;

            let forgotten =
                capture.retry_agent_process("proof".into(), None, Some(primary.clone()), None);
            assert_eq!(
                forgotten.unwrap_err().to_string(),
                "AgentProcess retry must retain its thread",
                "{source}"
            );
            assert_eq!(
                capture.session_attachment(),
                Some((session.clone(), original))
            );
            if failover {
                capture.retry_agent_process("proof".into(), None, Some(fallback), None)?;
            } else {
                capture.retry_agent_process("proof".into(), None, Some(primary), Some(&thread))?;
            }
            capture.finish("completed")?;
        }
        Ok(())
    })
    .unwrap();
}

#[test]
fn provider_observation_recovers_then_stops_repeating() {
    let home = tempfile::tempdir().unwrap();
    let capture = crate::session_record::SessionRecorder::begin_at(home.path(), spec(home.path()))
        .map(crate::process::LfProcessRuntime::new)
        .unwrap();
    let dir = capture.artifact_dir();
    let sql =
        rusqlite::Connection::open(crate::session_record::row_database(&dir).unwrap()).unwrap();
    sql.execute_batch(
        "CREATE TRIGGER reject_observation BEFORE INSERT ON session_events
         WHEN NEW.kind='observed'
         BEGIN SELECT RAISE(FAIL, 'observation unavailable'); END;",
    )
    .unwrap();
    let account = crate::store::ProviderAccountId::parse("primary").unwrap();
    capture.observe_provider(Some("thread".into()), Some(account.clone()));
    assert!(read_provider_session(&dir).unwrap().is_none());
    sql.execute_batch("DROP TRIGGER reject_observation")
        .unwrap();

    // A later account-only observation must also recover the lost thread.
    capture.observe_provider(None, Some(account.clone()));
    let recorded = read_provider_session(&dir).unwrap().unwrap();
    assert_eq!(recorded.agent_session, "thread".into());
    assert_eq!(recorded.account_id, Some(account.clone()));
    let store = crate::session_record::row_store(&dir).unwrap();
    let events = store.input_events(&capture.artifact_key()).unwrap();
    capture.observe_provider(Some("thread".into()), Some(account));
    assert_eq!(store.input_events(&capture.artifact_key()).unwrap(), events);
}

#[test]
fn two_runs_about_one_task_keep_distinct_identity_and_shared_provenance() {
    let home = tempfile::tempdir().unwrap();
    let selector = "task:LOO-267".to_string();
    let mut first = spec(home.path());
    first.skill = Some("research".to_string());
    first.subjects = vec![SubjectAttribution::declared(selector.clone())];
    let second = first.clone();

    let first = crate::session_record::SessionRecorder::begin_at(home.path(), first)
        .map(crate::process::LfProcessRuntime::new)
        .unwrap();
    let second = crate::session_record::SessionRecorder::begin_at(home.path(), second)
        .map(crate::process::LfProcessRuntime::new)
        .unwrap();
    let first_id = first.artifact_key();
    let second_id = second.artifact_key();

    assert_ne!(first_id, second_id);
    for capture in [&first, &second] {
        let manifest = crate::session_record::read_manifest(&capture.artifact_dir()).unwrap();
        assert_eq!(manifest.subjects[0].selector, selector);
    }
    assert!(!first.artifact_dir().join("terminal.json").exists());
    assert!(!second.artifact_dir().join("terminal.json").exists());
}

#[test]
fn dropping_the_last_capture_settles_unexpected_control_flow_as_failed() {
    let home = tempfile::tempdir().unwrap();
    let capture = crate::session_record::SessionRecorder::begin_at(home.path(), spec(home.path()))
        .map(crate::process::LfProcessRuntime::new)
        .expect("publish Session capture manifest");
    let dir = capture.artifact_dir();

    drop(capture);

    let terminal: TerminalReceipt =
        serde_json::from_slice(&fs::read(dir.join("terminal.json")).unwrap()).unwrap();
    assert_eq!(terminal.outcome, "failed");
}

#[test]
fn reader_rejects_an_unsupported_manifest_schema() {
    let home = tempfile::tempdir().unwrap();
    let capture = crate::session_record::SessionRecorder::begin_at(home.path(), spec(home.path()))
        .map(crate::process::LfProcessRuntime::new)
        .unwrap();
    let dir = capture.artifact_dir();
    capture.finish("completed").unwrap();
    let path = dir.join("manifest.json");
    let mut manifest: serde_json::Value =
        serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["schema_version"] = serde_json::json!(999);
    fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();

    let error = crate::session_record::read_manifest(&dir).unwrap_err();
    assert!(error
        .to_string()
        .contains("unsupported Session capture manifest schema 999"));
}
