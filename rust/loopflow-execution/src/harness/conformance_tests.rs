use crate::process::attachment;

use serde_json::Value;

use super::fact_writer::FactWriter;
use loopflow_harness::claude_decode::ClaudeDecoder;
use loopflow_harness::codex_decode::{rpc_error, CodexDecoder};
use loopflow_harness::fact::{self, Decoded, ProviderFact, ProviderTurn};
use loopflow_harness::opencode_decode::Turn;
use loopflow_harness::HarnessKind;
use loopflow_model::chat::types::{ConversationEvent, ConversationItem, Lifecycle};
use loopflow_model::id::AgentSessionId;

fn read_trace_lines(file_name: &str) -> Vec<String> {
    fact::trace(file_name)
        .iter()
        .map(Value::to_string)
        .collect()
}

fn replay_claude_trace(file_name: &str) -> (Vec<ConversationEvent>, Option<AgentSessionId>) {
    let mut events = Vec::new();
    let mut decoder = ClaudeDecoder::default();
    let mut session_id = None;
    let mut saw_turn_completed = false;

    'lines: for line in read_trace_lines(file_name) {
        if line.trim().is_empty() {
            continue;
        }
        // The decoder reports the provider turn's end; the reader (mirrored
        // here) owns the terminal TurnCompleted.
        for decoded in decoder.decode(&line, "turn_trace").unwrap() {
            match decoded {
                Decoded::Display(event) => events.push(event),
                Decoded::Fact(ProviderFact::SessionObserved(id)) => session_id = Some(id),
                Decoded::Fact(ProviderFact::TurnCompleted { status, .. }) => {
                    saw_turn_completed = true;
                    events.push(ConversationEvent::TurnCompleted {
                        turn_id: "turn_trace".to_string(),
                        status,
                    });
                    break 'lines;
                }
                Decoded::Fact(_) => {}
            }
        }
    }

    if !saw_turn_completed {
        for item in decoder.drain_open_items(Lifecycle::Failed) {
            events.push(ConversationEvent::ItemCompleted {
                turn_id: "turn_trace".to_string(),
                item,
            });
        }
        events.push(ConversationEvent::TurnCompleted {
            turn_id: "turn_trace".to_string(),
            status: Lifecycle::Failed,
        });
    }

    (events, session_id)
}

fn replay_codex_trace(file_name: &str) -> Vec<ConversationEvent> {
    replay_codex_lines(read_trace_lines(file_name))
}

/// Replay codex app-server lines through the production decoder, so traces
/// pin real behavior. Unawaited error responses display as the live reader's.
fn replay_codex_lines(lines: Vec<String>) -> Vec<ConversationEvent> {
    let mut decoder = CodexDecoder::default();
    let mut events = Vec::new();

    for line in lines {
        if line.trim().is_empty() {
            continue;
        }

        let value: Value = serde_json::from_str(&line).expect("trace line should be valid json");
        events.extend(decoder.decode(&value, None).into_iter().filter_map(
            |decoded| match decoded {
                Decoded::Display(event) => Some(event),
                Decoded::Fact(_) => None,
            },
        ));
        if let (None, Some(error)) = (value.get("method"), value.get("error")) {
            events.push(rpc_error(error));
        }
    }

    events
}

#[test]
fn claude_trace_normal_turn() {
    let (events, session_id) = replay_claude_trace("claude_normal_turn.ndjson");
    assert_eq!(
        session_id,
        Some("sess_claude_normal".into()),
        "system event's session id should be captured for --resume"
    );
    let event_types: Vec<_> = events.iter().map(ConversationEvent::event_type).collect();
    assert_eq!(
        event_types,
        vec!["text_delta", "usage_checkpoint", "turn_completed"]
    );
    assert!(matches!(
        events.get(1),
        Some(ConversationEvent::UsageCheckpoint { usage, .. })
            if usage.input_tokens == Some(321)
                && usage.output_tokens == Some(123)
                && usage.total_input_tokens == Some(337)
                && usage.cache_read_tokens == Some(11)
                && usage.cache_write_tokens == Some(5)
                && usage.reasoning_tokens == Some(9)
                && usage.model.as_deref() == Some("claude-sonnet-4")
                && usage.cost_usd == Some(0.2)
    ));
    assert!(matches!(
        events
            .iter()
            .find(|event| matches!(event, ConversationEvent::TurnCompleted { .. })),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Completed,
            ..
        })
    ));
}

#[test]
fn claude_trace_crash_mid_tool_marks_failed_items() {
    let (events, _session_id) = replay_claude_trace("claude_crash_mid_tool.ndjson");

    assert!(matches!(
        events.first(),
        Some(ConversationEvent::ItemStarted {
            item: ConversationItem::Command { .. },
            ..
        })
    ));

    assert!(events.iter().any(|event| {
        matches!(
            event,
            ConversationEvent::ItemCompleted {
                item: ConversationItem::Command { status, .. },
                ..
            } if *status == loopflow_model::chat::types::Lifecycle::Failed
        )
    }));

    assert!(matches!(
        events.last(),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Failed,
            ..
        })
    ));
}

#[test]
fn claude_trace_multi_tool_lifecycle() {
    let (events, _session_id) = replay_claude_trace("claude_multi_tool.ndjson");
    let started = events
        .iter()
        .filter(|event| matches!(event, ConversationEvent::ItemStarted { .. }))
        .count();
    let completed = events
        .iter()
        .filter(|event| matches!(event, ConversationEvent::ItemCompleted { .. }))
        .count();

    assert_eq!(started, 2);
    assert_eq!(completed, 2);
    assert!(matches!(
        events
            .iter()
            .find(|event| matches!(event, ConversationEvent::TurnCompleted { .. })),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Completed,
            ..
        })
    ));
}

#[test]
fn codex_trace_normal_turn() {
    let events = replay_codex_trace("codex_normal_turn.jsonl");
    let event_types: Vec<_> = events.iter().map(ConversationEvent::event_type).collect();
    // User echoes and status notifications produce no events. The final-answer
    // completion preserves its phase receipt; tokenUsage folds into usage_checkpoint.
    assert_eq!(
        event_types,
        vec![
            "turn_started",
            "text_delta",
            "item_completed",
            "usage_checkpoint",
            "usage_checkpoint",
            "turn_completed",
        ]
    );
    assert!(matches!(
        events
            .iter()
            .find(|event| matches!(event, ConversationEvent::TurnCompleted { .. })),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Completed,
            ..
        })
    ));
    assert!(matches!(
        events.get(1),
        Some(ConversationEvent::TextDelta { content, .. }) if content == "OK"
    ));
    assert!(matches!(
        events.get(2),
        Some(ConversationEvent::ItemCompleted {
            item: ConversationItem::Message { text, phase, .. },
            ..
        }) if text == "OK" && phase.as_deref() == Some("final_answer")
    ));
    // Codex reports cumulative gross input (cache included); the harness
    // reports the turn's own spend in Claude's shape: input net of cache,
    // gross in total_input_tokens.
    assert!(matches!(
        events.get(3),
        Some(ConversationEvent::UsageCheckpoint {
            usage,
            final_receipt: false,
            ..
        })
            if usage.input_tokens == Some(6465)
                && usage.output_tokens == Some(5)
                && usage.cache_read_tokens == Some(9600)
                && usage.total_input_tokens == Some(16065)
    ));
    assert!(matches!(
        events.get(4),
        Some(ConversationEvent::UsageCheckpoint {
            final_receipt: true,
            ..
        })
    ));
}

#[test]
fn codex_trace_error_turn() {
    let events = replay_codex_trace("codex_error.jsonl");
    let event_types: Vec<_> = events.iter().map(ConversationEvent::event_type).collect();
    assert_eq!(event_types, vec!["turn_started", "error", "turn_completed"]);
    assert!(matches!(
        events[1],
        ConversationEvent::Error { ref code, ref message, .. }
            if code == "codex_error" && message == "stream disconnected before completion"
    ));
    assert!(matches!(
        events
            .iter()
            .find(|event| matches!(event, ConversationEvent::TurnCompleted { .. })),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Failed,
            ..
        })
    ));
}

#[test]
fn codex_inline_error_survives_without_a_separate_notification() {
    let lines: Vec<_> = read_trace_lines("codex_error.jsonl")
        .into_iter()
        .filter(|line| serde_json::from_str::<Value>(line).unwrap()["method"] != "error")
        .collect();
    let events = replay_codex_lines(lines.clone().into_iter().chain(lines).collect());
    let kinds: Vec<_> = events.iter().map(ConversationEvent::event_type).collect();
    assert_eq!(
        kinds,
        vec![
            "turn_started",
            "error",
            "turn_completed",
            "turn_started",
            "error",
            "turn_completed"
        ]
    );
}

/// A `willRetry: true` error mid-turn must NOT produce a terminal Error
/// event: the vendor keeps the turn alive and retries, so the turn survives
/// to its real completion. The error surfaces non-terminally as a Thought
/// item (journaled, visible, harmless to the scheduler).
#[test]
fn codex_trace_will_retry_error_does_not_end_the_turn() {
    let events = replay_codex_trace("codex_error_will_retry.jsonl");
    let event_types: Vec<_> = events.iter().map(ConversationEvent::event_type).collect();
    assert_eq!(
        event_types,
        vec![
            "turn_started",
            "item_completed", // the retryable error, as a Thought
            "item_completed", // the real answer after the retry
            "usage_checkpoint",
            "usage_checkpoint",
            "turn_completed",
        ],
        "no terminal error event for a retryable error"
    );
    assert!(matches!(
        &events[1],
        ConversationEvent::ItemCompleted {
            item: ConversationItem::Thought { text, .. },
            ..
        } if text.contains("will retry") && text.contains("stream disconnected")
    ));
    assert!(matches!(
        events
            .iter()
            .find(|event| matches!(event, ConversationEvent::TurnCompleted { .. })),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Completed,
            ..
        })
    ));
}

#[test]
fn codex_rpc_error_response_maps_to_error_event() {
    // Live 0.142.5 shape: malformed requests (e.g. `content` instead of
    // `input` on turn/steer) come back as JSON-RPC error responses.
    let events = replay_codex_lines(vec![
        r#"{"error":{"code":-32600,"message":"Invalid request: missing field `input`"},"id":5}"#
            .to_string(),
    ]);

    assert_eq!(events.len(), 1);
    assert!(matches!(
        events[0],
        ConversationEvent::Error { ref code, ref message, .. }
            if code == "-32600" && message == "Invalid request: missing field `input`"
    ));
}

// A recorded `opencode run` (2.2.1). Usage rows come from the transcript
// reader (`transcript::opencode`), so the stream writes none.
#[test]
fn opencode_run_preserves_output_and_tools_and_leaves_usage_to_the_transcript() {
    use loopflow_model::id::LfProcessId;

    let home = tempfile::tempdir().unwrap();
    let path = home.path().join("store.db");
    let store = crate::store::opening::open_ephemeral(&path).unwrap();
    let sql = rusqlite::Connection::open(&path).unwrap();
    let process = LfProcessId::new();
    sql.execute(
        "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
        [process.as_str()],
    )
    .unwrap();
    let input = crate::session_record::new_artifact_key();
    let session = "session".to_owned();
    store.test_session(&session, &input);
    let attachment =
        attachment::claim_session_attachment(&store, &session, None, &process).unwrap();
    let mut writer = FactWriter::new(
        store.clone(),
        session.clone(),
        Some(attachment),
        Default::default(),
    );
    let turn = ProviderTurn {
        thread: "ses_e0fc53b6cffe5Hj2VueBVr1q78".into(),
        turn: "request".into(),
    };
    let mut decoded: Vec<Decoded> = vec![
        ProviderFact::SessionObserved(turn.thread.clone()).into(),
        ProviderFact::TurnAnswered {
            turn: turn.clone(),
            request: "request".into(),
        }
        .into(),
    ];
    let mut run = Turn::new(turn);
    for line in fact::trace("opencode_run.jsonl") {
        decoded.extend(run.event(&line));
    }
    decoded.extend(run.finish(true, false));
    writer.write(&decoded).unwrap();

    let display: Vec<_> = decoded
        .iter()
        .filter_map(|decoded| match decoded {
            Decoded::Display(event) => Some(event),
            Decoded::Fact(_) => None,
        })
        .collect();
    assert!(display.iter().any(|event| matches!(event, ConversationEvent::ItemCompleted {item:ConversationItem::Command {output:Some(text),exit_code:Some(0),..},..} if text == "probe\n")));
    assert!(display.iter().any(
        |event| matches!(event, ConversationEvent::TextDelta {content,..} if content == "done")
    ));
    assert!(matches!(
        display.last(),
        Some(ConversationEvent::TurnCompleted {
            status: Lifecycle::Completed,
            ..
        })
    ));
    let turns = store.session_turns(&session).unwrap();
    assert!(turns.iter().any(|turn| turn.ended_at.is_some()));
    assert!(turns.iter().all(|turn| turn.usage.is_none()));
    let usage = crate::store::sessions::input_history(&store, input.as_str())
        .unwrap()
        .usage;
    assert_eq!(usage.input_tokens, None);
}

/// Each harness reads its own provider's refusal, as the provider worded it.
#[test]
fn each_harness_classifies_its_own_refusals() {
    use super::HarnessKind::{Antigravity, Claude, Codex, OpenCode, Pi};
    use crate::agent::AgentFailure::{CredentialInvalidated, ModelUnavailable, UsageLimit};
    for harness in HarnessKind::ALL {
        let cases: &[(&str, &str, crate::agent::AgentFailure)] = match harness {
            Claude | Codex => &[
                ("provider_rate_limited", "rejected: seven_day", UsageLimit),
                ("provider_credential_invalidated", "", CredentialInvalidated),
                ("provider_model_unavailable", "", ModelUnavailable),
            ],
            OpenCode => &[
                ("opencode_error", "Insufficient balance. This request is pay-as-you-go, billed to OpenCode Models credits. Top up at https://opencode.ai/console/billing?credits=1.", UsageLimit),
                ("opencode_error", "Model unavailable: opencode/no-such-model-xyz", ModelUnavailable),
                ("opencode_error", "Your refresh token was revoked", CredentialInvalidated),
            ],
            Pi => &[
                ("pi_error", "Your refresh token was revoked", CredentialInvalidated),
                ("pi_error", "Usage limit reached", UsageLimit),
                ("pi_error", "Model is not available", ModelUnavailable),
            ],
            // agy 1.5.13 signed out and with an unknown model. The quota
            // wording is from its binary's strings.
            Antigravity => &[
                ("agy_error", "authentication failed or timed out", CredentialInvalidated),
                ("agy_error", "Error: authentication required. Run 'agy' to log in, then retry.", CredentialInvalidated),
                ("agy_error", "invalid model selection (--model \"no-such-model-xyz\" --effort \"\"): model no-such-model-xyz is not recognized as a known model or custom model in settings", ModelUnavailable),
                ("agy_error", "You have exhausted your quota on this model.", UsageLimit),
                ("agy_error", "RESOURCE_EXHAUSTED", UsageLimit),
            ],
        };
        for expected in [UsageLimit, CredentialInvalidated, ModelUnavailable] {
            assert!(
                cases.iter().any(|(_, _, failure)| *failure == expected),
                "{harness:?}: missing {expected:?} refusal case"
            );
        }
        for &(code, message, failure) in cases {
            let classified = super::launcher(harness).classify_failure(code, message);
            assert_eq!(classified, Some(failure), "{harness:?}: {code}: {message}");
        }
    }
}

/// Claude and Codex leave a transient failure to lf; the others retry their own.
#[test]
fn a_transient_failure_is_lfs_to_retry_only_where_the_provider_does_not() {
    for harness in HarnessKind::ALL {
        let retried = super::launcher(harness)
            .classify_failure("fixture_error", "connection reset")
            .is_some();
        assert_eq!(
            retried,
            matches!(harness, HarnessKind::Claude | HarnessKind::Codex),
            "{harness:?}"
        );
    }
}

/// Each harness lists a synced skill and Graph: the export is in every
/// directory the harness declares, in a shape no harness drops. A wrong path
/// passes here; only the real binary knows where it reads.
#[test]
fn every_harness_reads_a_synced_skill() {
    use crate::skills::{sync_skills, SkillSyncOptions};
    use loopflow_definitions::text::split_frontmatter;
    use std::fs;

    let home = tempfile::TempDir::new().unwrap();
    let repo = tempfile::TempDir::new().unwrap();
    fs::create_dir_all(repo.path().join(".lf/skills")).unwrap();
    fs::create_dir_all(repo.path().join(".lf/graphs")).unwrap();
    fs::write(repo.path().join(".lf/skills/audit.md"), "Audit the change.").unwrap();
    fs::write(
        repo.path().join(".lf/graphs/proof.yaml"),
        "- step: {name: audit, id: work}\n",
    )
    .unwrap();
    let global = SkillSyncOptions {
        prune: true,
        global_home: Some(home.path().to_owned()),
        repo: None,
    };
    sync_skills(&global).unwrap();
    sync_skills(&SkillSyncOptions {
        repo: Some(repo.path().to_owned()),
        ..global
    })
    .unwrap();

    for harness in HarnessKind::ALL {
        let installed = super::launcher(harness).capabilities().installed_skills;
        if let Some(registry) = installed.global_registry {
            let config: serde_json::Value =
                serde_json::from_str(&fs::read_to_string(home.path().join(registry)).unwrap())
                    .unwrap();
            let global = std::path::absolute(home.path().join(installed.global)).unwrap();
            assert!(
                config["entries"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|entry| entry["path"].as_str() == global.to_str()),
                "{harness:?}: {registry} doesn't list {}",
                global.display()
            );
        }
        for (base, directory, skill, graph) in [
            (home.path(), installed.global, "implement", "code"),
            (repo.path(), installed.repository, "audit", "proof"),
        ] {
            for name in [skill, graph] {
                let path = base.join(directory).join(name).join("SKILL.md");
                let content = fs::read_to_string(&path)
                    .unwrap_or_else(|error| panic!("{harness:?}: {}: {error}", path.display()));
                let (frontmatter, _) = split_frontmatter(&content).expect("export has frontmatter");
                let frontmatter: serde_yaml_ng::Value =
                    serde_yaml_ng::from_str(frontmatter).unwrap();
                assert_eq!(
                    frontmatter["name"].as_str(),
                    Some(name),
                    "{}",
                    path.display()
                );
                assert!(
                    frontmatter["description"]
                        .as_str()
                        .is_some_and(|description| !description.trim().is_empty()),
                    "{}",
                    path.display()
                );
                assert_eq!(
                    frontmatter["disable-model-invocation"].as_bool(),
                    (name == graph).then_some(true),
                    "{}",
                    path.display()
                );
            }
        }
    }
}
