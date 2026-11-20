use crate::process::attachment;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use serde_json::json;
use tokio::sync::mpsc;

use super::pi::Driver as PiDriver;
use super::Driver;
use crate::agent::AgentConfig;
use loopflow_harness::fact::{Decoded, ProviderFact, ProviderTurn};
use loopflow_harness::pi_decode::Turn;
use loopflow_model::chat::types::{ConversationEvent, ConversationItem, Lifecycle};

async fn completed(rx: &mut mpsc::UnboundedReceiver<ConversationEvent>) -> Vec<ConversationEvent> {
    tokio::time::timeout(Duration::from_secs(10), async {
        let mut events = Vec::new();
        loop {
            let event = rx.recv().await.expect("Pi event stream");
            let done = matches!(event, ConversationEvent::TurnCompleted { .. });
            events.push(event);
            if done {
                return events;
            }
        }
    })
    .await
    .expect("Pi must settle the turn")
}

fn fixture(
    config: &mut AgentConfig,
) -> (
    crate::journal::TestLedgerGuard,
    crate::test_ambient::EnvGuard,
    crate::harness::Recording,
) {
    let ledger = crate::journal::TestLedgerGuard::new();
    let recording = super::admit_for_test(ledger.home());
    let env = crate::test_ambient::EnvGuard::clear(&["PATH", "LF_BIN"]);
    let script = ledger.home().join("pi");
    std::fs::write(
        &script,
        format!(
            "#!/usr/bin/env -S python3 -I\n{}",
            include_str!("testdata/pi_rpc.py")
        ),
    )
    .unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    std::env::set_var("PATH", format!("{}:/usr/bin:/bin", ledger.home().display()));
    std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
    config.cwd = Some(ledger.home().to_owned());
    config.agent = crate::config::Agent::parse("pi:fixture/model").unwrap();
    (ledger, env, recording)
}

#[tokio::test]
async fn pi_turns_resume_the_exact_conversation_and_capture_tools_usage() {
    let mut config = crate::agent::AgentConfig::default();
    let (_ledger, _env, recording) = fixture(&mut config);
    let (tx, mut rx) = mpsc::unbounded_channel();

    let mut pi = PiDriver::new(
        &crate::harness::launch_for_test(loopflow_harness::HarnessKind::Pi, &config, &recording),
        tx,
    );
    pi.start().await.unwrap();
    let token = pi.agent_session().unwrap();
    // Selecting another Machine must not redirect this admitted conversation's
    // input or shutdown to that Machine's store.
    let other_home = tempfile::tempdir().unwrap();
    let other_store =
        crate::store::opening::open_ephemeral(&other_home.path().join("loopflow.db")).unwrap();
    let selection = crate::test_ambient::EnvGuard::clear(&["LF_HOME"]);
    std::env::set_var("LF_HOME", other_home.path());
    pi.send_input("tool").await.unwrap();
    let events = completed(&mut rx).await;
    assert!(events.iter().any(|e| matches!(
        e,
        ConversationEvent::ItemCompleted {
            item: ConversationItem::Tool {
                status: Lifecycle::Completed,
                ..
            },
            ..
        }
    )));
    assert!(events.iter().any(|e| matches!(e, ConversationEvent::UsageCheckpoint { final_receipt: true, usage, .. } if usage.input_tokens == Some(20) && usage.total_input_tokens == Some(26))));
    pi.stop().await.unwrap();
    assert!(other_store.agent_processes().unwrap().is_empty());
    drop(selection);
    let crate::harness::Recording {
        store,
        session,
        attachment,
    } = recording.clone();
    let next = attachment::prepare_session_agent_process(&store, &session, &attachment).unwrap();
    let recording = crate::harness::Recording {
        store,
        session,
        attachment: next,
    };
    config.resume_token = Some(token.clone());
    let (tx, mut rx) = mpsc::unbounded_channel();

    let mut pi = PiDriver::new(
        &crate::harness::launch_for_test(loopflow_harness::HarnessKind::Pi, &config, &recording),
        tx,
    );
    pi.start().await.unwrap();
    assert_eq!(pi.agent_session(), Some(token));
    pi.send_input("second").await.unwrap();
    let events = completed(&mut rx).await;
    assert!(events.iter().any(|e| matches!(e, ConversationEvent::TextDelta { content, .. } if content == "answer 2: second")));
    pi.stop().await.unwrap();
}

// An interactive Pi records the id lf gave it; a headless turn resumes
// that conversation by the file Pi keeps under the id.
#[tokio::test]
async fn pi_headless_resumes_a_conversation_recorded_by_its_id() {
    let mut config = crate::agent::AgentConfig::default();
    let (ledger, _env, recording) = fixture(&mut config);
    let _dir = crate::test_ambient::EnvGuard::clear(&["PI_CODING_AGENT_DIR"]);
    std::env::set_var("PI_CODING_AGENT_DIR", ledger.home());
    let id: loopflow_model::id::AgentSessionId = "cac985ee-d62a-4ce9-abba-f3e66d053410".into();
    let project = ledger.home().join("sessions").join("--repo--");
    std::fs::create_dir_all(&project).unwrap();
    let file = project.join(format!("2026-11-17T07-26-34-224Z_{id}.jsonl"));
    std::fs::write(&file, format!("{{\"type\":\"session\",\"id\":\"{id}\"}}\n")).unwrap();
    config.resume_token = Some(id);
    let (tx, mut rx) = mpsc::unbounded_channel();

    let mut pi = PiDriver::new(
        &crate::harness::launch_for_test(loopflow_harness::HarnessKind::Pi, &config, &recording),
        tx,
    );
    pi.start().await.unwrap();
    assert_eq!(pi.agent_session(), Some(file.display().to_string().into()));
    pi.send_input("again").await.unwrap();
    let events = completed(&mut rx).await;
    assert!(events.iter().any(|e| matches!(e, ConversationEvent::TextDelta { content, .. } if content == "answer 1: again")));
    pi.stop().await.unwrap();
}

#[tokio::test]
async fn pi_interrupt_keeps_the_conversation_alive_and_auth_failure_is_not_success() {
    let mut config = crate::agent::AgentConfig::default();
    let (_ledger, _env, recording) = fixture(&mut config);
    let (tx, mut rx) = mpsc::unbounded_channel();

    let mut pi = PiDriver::new(
        &crate::harness::launch_for_test(loopflow_harness::HarnessKind::Pi, &config, &recording),
        tx,
    );
    pi.start().await.unwrap();
    let id = pi.agent_session();
    pi.send_input("wait").await.unwrap();
    assert!(pi.send_input("overlap").await.is_err());
    pi.interrupt().await.unwrap();
    assert!(matches!(
        completed(&mut rx).await.last(),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Interrupted,
            ..
        })
    ));
    pi.send_input("auth").await.unwrap();
    let events = completed(&mut rx).await;
    assert!(events.iter().any(
        |e| matches!(e, ConversationEvent::Error { message, .. } if message.contains("API key"))
    ));
    assert!(matches!(
        events.last(),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Failed,
            ..
        })
    ));
    pi.send_input("recovered").await.unwrap();
    assert!(matches!(
        completed(&mut rx).await.last(),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Completed,
            ..
        })
    ));
    assert_eq!(pi.agent_session(), id);
    pi.stop().await.unwrap();
}

#[tokio::test]
async fn pi_crash_closes_open_tools_and_fails_the_turn() {
    let mut config = crate::agent::AgentConfig::default();
    let (_ledger, _env, recording) = fixture(&mut config);
    let (tx, mut rx) = mpsc::unbounded_channel();

    let mut pi = PiDriver::new(
        &crate::harness::launch_for_test(loopflow_harness::HarnessKind::Pi, &config, &recording),
        tx,
    );
    pi.start().await.unwrap();
    pi.send_input("crash").await.unwrap();
    let events = completed(&mut rx).await;
    assert!(events.iter().any(|e| matches!(
        e,
        ConversationEvent::ItemCompleted {
            item: ConversationItem::Tool {
                status: Lifecycle::Failed,
                ..
            },
            ..
        }
    )));
    assert!(matches!(
        events.last(),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Failed,
            ..
        })
    ));
    pi.stop().await.unwrap();
}

/// A test turn whose decoded events are reduced to display.
struct DisplayTurn(Turn);

impl DisplayTurn {
    fn new(id: &str) -> Self {
        Self(Turn::new(ProviderTurn {
            thread: "/tmp/pi-session.jsonl".into(),
            turn: id.into(),
        }))
    }

    fn event(&mut self, event: &serde_json::Value) -> Vec<ConversationEvent> {
        self.0
            .event(event)
            .into_iter()
            .filter_map(|decoded| match decoded {
                Decoded::Display(event) => Some(event),
                Decoded::Fact(_) => None,
            })
            .collect()
    }
}

#[test]
fn pi_waits_for_settlement_and_does_not_invent_missing_usage() {
    let mut turn = DisplayTurn::new("turn");
    assert!(turn
        .event(&json!({"type":"agent_end","willRetry":false}))
        .is_empty());
    turn.event(&json!({"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"hello"}],"stopReason":"stop"}}));
    let events = turn.event(&json!({"type":"agent_settled","aborted":false}));
    assert!(!events
        .iter()
        .any(|e| matches!(e, ConversationEvent::UsageCheckpoint { .. })));
    assert!(matches!(
        events.last(),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Completed,
            ..
        })
    ));
}

#[test]
fn pi_missing_conversation_is_not_recreated() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("missing.jsonl");
    assert!(
        super::pi::validate_resume(&path.display().to_string().into())
            .unwrap_err()
            .to_string()
            .contains("missing")
    );
    assert!(!path.exists());
}

#[tokio::test]
async fn pi_rejected_prompt_and_malformed_stream_remain_failures() {
    let mut config = crate::agent::AgentConfig::default();
    let (_ledger, _env, recording) = fixture(&mut config);
    let (tx, mut rx) = mpsc::unbounded_channel();

    let mut pi = PiDriver::new(
        &crate::harness::launch_for_test(loopflow_harness::HarnessKind::Pi, &config, &recording),
        tx,
    );
    pi.start().await.unwrap();
    assert!(pi
        .send_input("reject")
        .await
        .unwrap_err()
        .to_string()
        .contains("Model not found"));
    assert!(matches!(
        completed(&mut rx).await.last(),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Failed,
            ..
        })
    ));
    pi.send_input("invalid").await.unwrap();
    assert!(matches!(
        completed(&mut rx).await.last(),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Failed,
            ..
        })
    ));
    assert!(pi.send_input("after EOF").await.is_err());
    pi.stop().await.unwrap();
}

#[tokio::test]
async fn pi_headless_dialogs_and_extension_failures_do_not_hang_or_succeed() {
    for (input, failed, reason) in [
        (
            "dialog",
            "unanswered_question",
            "Pi asked a question with nobody there to answer it: Use the fixture?",
        ),
        (
            "extension error",
            "pi_disconnected",
            "context delivery failed",
        ),
    ] {
        let mut config = crate::agent::AgentConfig::default();
        let (_ledger, _env, recording) = fixture(&mut config);
        let (tx, mut rx) = mpsc::unbounded_channel();

        let mut pi = PiDriver::new(
            &crate::harness::launch_for_test(
                loopflow_harness::HarnessKind::Pi,
                &config,
                &recording,
            ),
            tx,
        );
        pi.start().await.unwrap();
        pi.send_input(input).await.unwrap();
        assert!(matches!(
            completed(&mut rx).await.last(),
            Some(ConversationEvent::TurnCompleted {
                status: Lifecycle::Failed,
                ..
            })
        ));
        assert!(
            matches!(rx.recv().await.unwrap(), ConversationEvent::Error { code, message, .. } if code == failed && message.contains(reason))
        );
        pi.stop().await.unwrap();
    }
}

#[tokio::test]
async fn pi_missing_binary_records_a_failed_launch_without_a_live_process() {
    let mut config = crate::agent::AgentConfig::default();
    let (ledger, _env, recording) = fixture(&mut config);
    std::fs::remove_file(ledger.home().join("pi")).unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();

    let mut pi = PiDriver::new(
        &crate::harness::launch_for_test(loopflow_harness::HarnessKind::Pi, &config, &recording),
        tx,
    );
    let error = pi.start().await.unwrap_err().to_string();
    assert!(error.contains("Install Pi"), "{error}");
    assert_eq!(pi.pid(), None);
    let crate::harness::Recording {
        store, attachment, ..
    } = recording.clone();
    assert!(store
        .agent_process(&attachment.agent_process_id)
        .unwrap()
        .unwrap()
        .completed_at
        .is_some());
}

#[test]
fn pi_model_selection_native_resume_and_credentials_keep_their_owners() {
    let _lock = crate::journal::test_env_lock();
    let _env = crate::test_ambient::EnvGuard::clear(&["LF_BIN"]);
    // Pi's context extension calls back into this checkout's lf.
    std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("pi.jsonl");
    std::fs::write(&path, "{\"type\":\"session\",\"id\":\"native\"}\n").unwrap();
    let token = path.display().to_string().into();
    let config = AgentConfig {
        agent: crate::config::Agent::parse("pi:openai/gpt-5:high").unwrap(),
        cwd: Some(directory.path().into()),
        ..crate::agent::AgentConfig::default()
    };
    let cmd = crate::harness::launcher(loopflow_harness::HarnessKind::Pi)
        .open(&crate::harness::Launch {
            harness: crate::harness::launcher(loopflow_harness::HarnessKind::Pi),
            config: &config,
            recording: &crate::harness::admit_for_test(tempfile::tempdir().unwrap().path()),
            kind: crate::harness::LaunchKind::Batch,
            start: crate::harness::Start::Resume { thread: &token },
            context_file: Some(&directory.path().join("context.md")),
        })
        .unwrap();
    assert_eq!(cmd.process.get_program(), "pi");
    assert!(cmd
        .process
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .windows(2)
        .any(|a| a == ["--model", "openai/gpt-5:high"]));
    assert!(cmd
        .process
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .windows(2)
        .any(|a| a == ["--session", path.to_str().unwrap()]));
    for name in [
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GEMINI_API_KEY",
        "MOONSHOT_API_KEY",
    ] {
        assert!(crate::provider_auth::api_key_env_allowed(
            loopflow_harness::HarnessKind::Pi,
            name
        ));
        assert!(!crate::provider_auth::provider_env_allowed(
            loopflow_harness::HarnessKind::Pi,
            name
        ));
    }
}

#[test]
fn pi_empty_final_answers_fail_but_tool_steps_and_internal_retries_can_recover() {
    for (text, stop) in [
        ("", "stop"),
        ("  ", "stop"),
        ("\n", "stop"),
        ("Checking", "toolUse"),
    ] {
        let mut turn = DisplayTurn::new("empty");
        turn.event(&json!({"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"intermediate"}],"stopReason":"toolUse"}}));
        turn.event(&json!({"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":text}],"stopReason":stop}}));
        let events = turn.event(&json!({"type":"agent_settled","aborted":false}));
        assert!(events
            .iter()
            .any(|e| matches!(e, ConversationEvent::Error {code, ..} if code == "pi_empty_turn")));
        assert!(matches!(
            events.last(),
            Some(ConversationEvent::TurnCompleted {
                status: Lifecycle::Failed,
                ..
            })
        ));
    }
    let mut turn = DisplayTurn::new("recovered");
    for message in [
        json!({"role":"assistant","content":[],"stopReason":"error","errorMessage":"connection reset"}),
        json!({"role":"assistant","content":[{"type":"toolCall","id":"tool"}],"stopReason":"toolUse"}),
        json!({"role":"assistant","content":[{"type":"text","text":"done"}],"stopReason":"stop"}),
    ] {
        assert!(!turn
            .event(&json!({"type":"message_end","message":message}))
            .iter()
            .any(|e| matches!(e, ConversationEvent::TurnCompleted { .. })));
    }
    assert!(matches!(
        turn.event(&json!({"type":"agent_settled","aborted":false}))
            .last(),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Completed,
            ..
        })
    ));
}

/// What a failed turn's completion records as its error.
fn completed_error(decoded: Vec<Decoded>) -> serde_json::Value {
    decoded
        .into_iter()
        .find_map(|decoded| match decoded {
            Decoded::Fact(ProviderFact::TurnCompleted { detail, .. }) => {
                Some(detail["error"].clone())
            }
            _ => None,
        })
        .expect("turn completion")
}

#[test]
fn a_failed_pi_turn_keeps_why_on_its_completion() {
    let provider_turn = || ProviderTurn {
        thread: "/tmp/pi-session.jsonl".into(),
        turn: "turn".into(),
    };
    let mut turn = Turn::new(provider_turn());
    turn.event(&json!({"type":"message_end","message":{"role":"assistant","content":[],"stopReason":"error","errorMessage":"429 quota exceeded"}}));
    let settled = turn.event(&json!({"type":"agent_settled","aborted":false}));
    assert_eq!(
        completed_error(settled),
        json!({"code":"pi_model_error","message":"429 quota exceeded"})
    );

    let mut turn = Turn::new(provider_turn());
    let settled = turn.event(&json!({"type":"agent_settled","aborted":false}));
    assert_eq!(completed_error(settled)["code"], "pi_empty_turn");

    // A model error Pi reported stays the cause when its stream then closes.
    let mut turn = Turn::new(provider_turn());
    turn.event(&json!({"type":"message_end","message":{"role":"assistant","content":[],"stopReason":"error","errorMessage":"overloaded"}}));
    let died = turn.finish(
        Lifecycle::Failed,
        Some(json!({"code":"pi_disconnected","message":"Pi RPC stream closed"})),
    );
    assert_eq!(completed_error(died)["code"], "pi_model_error");
}

#[test]
fn bare_pi_names_its_thread_and_resumes_it_by_that_id() {
    let _lock = crate::journal::test_env_lock();
    let _env = crate::test_ambient::EnvGuard::clear(&["PI_CODING_AGENT_DIR", "LF_BIN"]);
    // Pi's context extension calls back into this checkout's lf.
    std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
    let agent_dir = tempfile::tempdir().unwrap();
    std::env::set_var("PI_CODING_AGENT_DIR", agent_dir.path());
    let config = AgentConfig {
        agent: crate::config::Agent::parse("pi").unwrap(),
        cwd: Some(agent_dir.path().into()),
        ..crate::agent::AgentConfig::default()
    };
    // A bare Pi's thread is the one its admission assigned.
    let recording = crate::harness::admit_for_test(tempfile::tempdir().unwrap().path());
    let id = recording.assigned_thread().unwrap();
    let start = crate::harness::launcher(loopflow_harness::HarnessKind::Pi)
        .open(&crate::harness::Launch {
            harness: crate::harness::launcher(loopflow_harness::HarnessKind::Pi),
            config: &config,
            recording: &recording,
            kind: crate::harness::LaunchKind::Batch,
            start: crate::harness::Start::New,
            context_file: None,
        })
        .unwrap();
    assert!(start
        .process
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .windows(2)
        .any(|a| a == ["--session-id", id.as_str()]));

    // Before Pi wrote the conversation (no reply yet), a resume starts it
    // fresh under the same id.
    let fresh = crate::harness::launcher(loopflow_harness::HarnessKind::Pi)
        .open(&crate::harness::Launch {
            harness: crate::harness::launcher(loopflow_harness::HarnessKind::Pi),
            config: &config,
            recording: &crate::harness::admit_for_test(tempfile::tempdir().unwrap().path()),
            kind: crate::harness::LaunchKind::Batch,
            start: crate::harness::Start::Resume { thread: &id },
            context_file: None,
        })
        .unwrap();
    assert!(fresh
        .process
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .windows(2)
        .any(|a| a == ["--session-id", id.as_str()]));
    let project = agent_dir.path().join("sessions").join("--repo--");
    std::fs::create_dir_all(&project).unwrap();
    let file = project.join(format!("2026-11-11T11-22-55-913Z_{id}.jsonl"));
    std::fs::write(&file, format!("{{\"type\":\"session\",\"id\":\"{id}\"}}\n")).unwrap();
    let resume = crate::harness::launcher(loopflow_harness::HarnessKind::Pi)
        .open(&crate::harness::Launch {
            harness: crate::harness::launcher(loopflow_harness::HarnessKind::Pi),
            config: &config,
            recording: &crate::harness::admit_for_test(tempfile::tempdir().unwrap().path()),
            kind: crate::harness::LaunchKind::Batch,
            start: crate::harness::Start::Resume { thread: &id },
            context_file: None,
        })
        .unwrap();
    assert!(resume
        .process
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .windows(2)
        .any(|a| a == ["--session", file.to_str().unwrap()]));
}
