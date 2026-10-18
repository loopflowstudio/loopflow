use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

use serde_json::json;
use tokio::sync::mpsc;

use super::pi::PiHarness;
use super::pi_mapping::Turn;
use super::{Harness, SendCurrentOutcome};
use crate::agent::AgentConfig;
use crate::chat::types::{ConversationEvent, ConversationItem, Lifecycle};

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
) {
    let ledger = super::admit_for_test(config);
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
    config.agent = Some("pi:fixture/model".into());
    (ledger, env)
}

#[tokio::test]
async fn pi_turns_resume_the_exact_conversation_and_capture_tools_usage() {
    let mut config = AgentConfig::default();
    let (_ledger, _env) = fixture(&mut config);
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut pi = PiHarness::new(tx);
    pi.start(&config).await.unwrap();
    let token = pi.agent_session().unwrap();
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
    let (store, session, attachment) =
        super::agent_process::open_owner(config.session_attachment.as_ref()).unwrap();
    let next = store
        .prepare_session_agent_process(&session, &attachment)
        .unwrap();
    config.session_attachment = Some((session, next));
    config.resume_token = Some(token.clone());
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut pi = PiHarness::new(tx);
    pi.start(&config).await.unwrap();
    assert_eq!(pi.agent_session(), Some(token));
    pi.send_input("second").await.unwrap();
    let events = completed(&mut rx).await;
    assert!(events.iter().any(|e| matches!(e, ConversationEvent::TextDelta { content, .. } if content == "answer 2: second")));
    pi.stop().await.unwrap();
}

#[tokio::test]
async fn pi_interrupt_keeps_the_conversation_alive_and_auth_failure_is_not_success() {
    let mut config = AgentConfig::default();
    let (_ledger, _env) = fixture(&mut config);
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut pi = PiHarness::new(tx);
    pi.start(&config).await.unwrap();
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
    let mut config = AgentConfig::default();
    let (_ledger, _env) = fixture(&mut config);
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut pi = PiHarness::new(tx);
    pi.start(&config).await.unwrap();
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

#[test]
fn pi_waits_for_settlement_and_does_not_invent_missing_usage() {
    let mut turn = Turn::new("turn".into());
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
    let mut config = AgentConfig::default();
    let (_ledger, _env) = fixture(&mut config);
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut pi = PiHarness::new(tx);
    pi.start(&config).await.unwrap();
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
    for (input, reason) in [
        ("dialog", "interactive input"),
        ("extension error", "context delivery failed"),
    ] {
        let mut config = AgentConfig::default();
        let (_ledger, _env) = fixture(&mut config);
        let (tx, mut rx) = mpsc::unbounded_channel();
        let mut pi = PiHarness::new(tx);
        pi.start(&config).await.unwrap();
        pi.send_input(input).await.unwrap();
        assert!(matches!(
            completed(&mut rx).await.last(),
            Some(ConversationEvent::TurnCompleted {
                status: Lifecycle::Failed,
                ..
            })
        ));
        assert!(
            matches!(rx.recv().await.unwrap(), ConversationEvent::Error { message, .. } if message.contains(reason))
        );
        pi.stop().await.unwrap();
    }
}

#[tokio::test]
async fn pi_missing_binary_records_a_failed_launch_without_a_live_process() {
    let mut config = AgentConfig::default();
    let (ledger, _env) = fixture(&mut config);
    std::fs::remove_file(ledger.home().join("pi")).unwrap();
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut pi = PiHarness::new(tx);
    let error = pi.start(&config).await.unwrap_err().to_string();
    assert!(error.contains("Install Pi"), "{error}");
    assert_eq!(pi.pid(), None);
    let (store, _, attachment) =
        super::agent_process::open_owner(config.session_attachment.as_ref()).unwrap();
    assert!(store
        .process(&attachment.agent_process_id)
        .unwrap()
        .unwrap()
        .completed_at
        .is_some());
}

#[test]
fn pi_model_selection_native_resume_and_credentials_keep_their_owners() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("pi.jsonl");
    std::fs::write(&path, "{\"type\":\"session\",\"id\":\"native\"}\n").unwrap();
    let token = path.display().to_string().into();
    let config = AgentConfig {
        agent: Some("pi:openai/gpt-5:high".into()),
        cwd: Some(directory.path().into()),
        ..Default::default()
    };
    let cmd = crate::agent::build_session_command(
        &config,
        crate::agent::ProviderInvocation::Resume { session_id: &token },
        Some(&directory.path().join("context.md")),
    )
    .unwrap();
    assert_eq!(cmd.program, "pi");
    assert!(cmd
        .args
        .windows(2)
        .any(|a| a == ["--model", "openai/gpt-5:high"]));
    assert!(cmd
        .args
        .windows(2)
        .any(|a| a == ["--session", path.to_str().unwrap()]));
    for name in [
        "ANTHROPIC_API_KEY",
        "OPENAI_API_KEY",
        "GEMINI_API_KEY",
        "MOONSHOT_API_KEY",
    ] {
        assert!(crate::provider_auth::api_key_env_allowed_for_program(
            "pi", name
        ));
        assert!(!crate::provider_auth::provider_env_allowed_for_program(
            "pi", name
        ));
    }
}

#[tokio::test]
async fn pi_stale_attachment_cannot_send_or_stop_the_provider() {
    let mut config = AgentConfig::default();
    let (_ledger, _env) = fixture(&mut config);
    let (tx, _rx) = mpsc::unbounded_channel();
    let mut pi = PiHarness::new(tx);
    pi.start(&config).await.unwrap();
    let (store, session, previous) =
        super::agent_process::open_owner(config.session_attachment.as_ref()).unwrap();
    let current = store
        .claim_session_attachment(
            &session,
            Some(&previous),
            previous.lf_process_id.as_ref().unwrap(),
            false,
        )
        .unwrap();
    assert!(pi.send_input("stale direction").await.is_err());
    assert!(pi.stop().await.is_err());
    let pid = pi.pid().unwrap();
    let process = store.process(&current.agent_process_id).unwrap().unwrap();
    assert_eq!(
        crate::journal::process_identity_evidence(pid, process.os_started_at.unwrap()),
        crate::journal::ProcessIdentityEvidence::Live
    );
    super::agent_process::stop(&(store.clone(), session.clone(), current)).unwrap();
    assert!(store
        .session(&session)
        .unwrap()
        .unwrap()
        .completed_at
        .is_none());
}

#[test]
fn pi_empty_final_answers_fail_but_tool_steps_and_internal_retries_can_recover() {
    for (text, stop) in [
        ("", "stop"),
        ("  ", "stop"),
        ("\n", "stop"),
        ("Checking", "toolUse"),
    ] {
        let mut turn = Turn::new("empty".into());
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
    let mut turn = Turn::new("recovered".into());
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

#[tokio::test]
async fn pi_steering_receipts_preserve_one_turn_and_distinguish_uncertainty() {
    let mut config = AgentConfig::default();
    let (_ledger, _env) = fixture(&mut config);
    let (tx, mut rx) = mpsc::unbounded_channel();
    let mut pi = PiHarness::new(tx);
    pi.start(&config).await.unwrap();
    assert_eq!(
        pi.send_current("idle").await,
        SendCurrentOutcome::NotSteerable
    );
    pi.send_input("wait").await.unwrap();
    let ConversationEvent::TurnStarted { turn_id } = rx.recv().await.unwrap() else {
        panic!("turn starts");
    };
    let (store, session, _) =
        super::agent_process::open_owner(config.session_attachment.as_ref()).unwrap();
    let thread = pi.agent_session().unwrap();
    let origin = store.session_history(&session, 0, 0).unwrap();
    assert!(matches!(
        pi.send_current("reject").await,
        SendCurrentOutcome::Failed { .. }
    ));
    assert!(matches!(
        pi.send_current("handled").await,
        SendCurrentOutcome::Unknown { .. }
    ));
    assert!(matches!(
        pi.send_current("malformed").await,
        SendCurrentOutcome::Unknown { .. }
    ));
    assert_eq!(
        pi.send_current("direction 雪").await,
        SendCurrentOutcome::Sent {
            provider_turn_id: turn_id.clone()
        }
    );
    assert_eq!(
        pi.send_current("finish").await,
        SendCurrentOutcome::Sent {
            provider_turn_id: turn_id.clone()
        }
    );
    let events = completed(&mut rx).await;
    assert!(!events
        .iter()
        .any(|e| matches!(e, ConversationEvent::TurnStarted { .. })));
    assert!(
        matches!(events.last(), Some(ConversationEvent::TurnCompleted {turn_id: id, status: Lifecycle::Completed}) if id == &turn_id)
    );
    assert_eq!(pi.agent_session().unwrap(), thread);
    let start = origin
        .iter()
        .find(|e| e.kind == crate::session::SessionEventKind::Started)
        .unwrap();
    let done = store
        .session_history(&session, 0, 0)
        .unwrap()
        .into_iter()
        .find(|e| e.kind == crate::session::SessionEventKind::Completed)
        .unwrap();
    assert_eq!(done.lf_process_id, start.lf_process_id);
    assert_eq!(done.agent_process_id, start.agent_process_id);
    assert_eq!(done.task_id, start.task_id);
    assert_eq!(done.provider_turn, start.provider_turn);
    let history = store.session_history(&session, 0, 0).unwrap();
    assert_eq!(
        history
            .iter()
            .filter(|e| e.kind == crate::session::SessionEventKind::Started)
            .count(),
        origin
            .iter()
            .filter(|e| e.kind == crate::session::SessionEventKind::Started)
            .count()
    );
    pi.send_input("wait").await.unwrap();
    assert!(matches!(
        pi.send_current("late").await,
        SendCurrentOutcome::Unknown { .. }
    ));
    completed(&mut rx).await;
    pi.send_input("wait").await.unwrap();
    assert!(matches!(
        pi.send_current("disconnect").await,
        SendCurrentOutcome::Unknown { .. }
    ));
    completed(&mut rx).await;
    pi.stop().await.unwrap();
}
