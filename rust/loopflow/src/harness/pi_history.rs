//! Pi evidence uses existing Session history; no provider-account inference.
use serde_json::{json, Value};

use super::activity_tracker::Signal;
use crate::chat::types::{ConversationEvent, ConversationItem, Lifecycle};
use crate::id::AgentSessionId;
use crate::session::SessionEventKind;
use crate::store::sqlite::SqliteStore;
use crate::store::StoreResult;

pub(super) fn signals(value: &Value, mapped: &[ConversationEvent]) -> Vec<Signal> {
    match value["type"].as_str() {
        Some("agent_start" | "message_update") => vec![Signal::Working],
        Some("tool_execution_start") => value["toolCallId"]
            .as_str()
            .map(|id| Signal::ToolStarted(id.into()))
            .into_iter()
            .collect(),
        Some("tool_execution_end") => value["toolCallId"]
            .as_str()
            .map(|id| Signal::ToolResolved(id.into()))
            .into_iter()
            .collect(),
        Some("agent_settled") => vec![Signal::Yielded(mapped.iter().any(|e| {
            matches!(
                e,
                ConversationEvent::TurnCompleted {
                    status: Lifecycle::Completed,
                    ..
                }
            )
        }))],
        _ => Vec::new(),
    }
}

pub(super) fn record(
    store: &SqliteStore,
    session: &str,
    thread: &AgentSessionId,
    events: &[ConversationEvent],
) -> StoreResult<()> {
    for event in events {
        let (turn, kind, payload) = match event {
            ConversationEvent::ItemCompleted {
                turn_id,
                item: ConversationItem::Message { text, phase, .. },
            } if phase.as_deref() == Some("final") => {
                (turn_id, SessionEventKind::Output, json!({"text":text}))
            }
            ConversationEvent::UsageCheckpoint {
                turn_id,
                usage,
                final_receipt,
            } => (
                turn_id,
                SessionEventKind::Usage,
                json!({"provider":"pi", "usage":usage, "final":final_receipt}),
            ),
            ConversationEvent::TurnCompleted { turn_id, status } => (
                turn_id,
                SessionEventKind::Completed,
                json!({"status":status.name()}),
            ),
            _ => continue,
        };
        store.record_session_event(session, thread, turn, kind, &payload)?;
    }
    Ok(())
}

/// The native extension is an owned provider callback, fenced like context delivery.
pub fn native_event() -> anyhow::Result<()> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(32 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    anyhow::ensure!(
        bytes.len() <= 32 * 1024 * 1024,
        "Pi native event exceeds 32 MiB"
    );
    let value: Value = serde_json::from_slice(&bytes)?;
    let adapter: crate::steer::SteerAdapter =
        serde_json::from_str(&std::env::var(crate::steer::PROVIDER_CONTEXT_ENV)?)?;
    let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
    observe_native(&store, &adapter, &value)
}

fn observe_native(
    store: &SqliteStore,
    adapter: &crate::steer::SteerAdapter,
    value: &Value,
) -> anyhow::Result<()> {
    let file = value["sessionFile"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Pi did not report a session file"))?;
    anyhow::ensure!(
        std::path::Path::new(file).is_absolute(),
        "Pi session file must be absolute"
    );
    let thread = AgentSessionId::from(file);
    let startup = value["type"] == "session_start";
    store.with_provider_context(adapter, file, startup, || {
        let attachment = store.session_attachment(&adapter.session)?.ok_or_else(|| {
            crate::store::StoreError::InvalidAuthority("Pi has no attachment".into())
        })?;
        store.record_session_agent_session(&adapter.session, &attachment, &thread)?;
        let session = store
            .session(&adapter.session)?
            .ok_or_else(|| crate::store::StoreError::InvalidData("Pi Session missing".into()))?;
        let input = session
            .input_key()
            .map_err(|e| crate::store::StoreError::InvalidData(e.to_string()))?
            .clone();
        let source = format!("pi-native:{}", uuid::Uuid::new_v4());
        store.retain_session_observation(
            &session,
            &crate::session::SessionObservation {
                artifact_key: input.clone(),
                source: source.clone(),
                observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                task_id: session.task_id.clone(),
                wave_id: session.wave_id.clone(),
                payload: json!({"input_id":input,"source":source,"evidence":value}),
            },
        )?;
        if startup {
            crate::session_record::write_provider_session(
                &crate::session_record::capture_dir(&input)
                    .map_err(|e| crate::store::StoreError::InvalidData(e.to_string()))?,
                &thread,
                None,
            )
            .map_err(|e| crate::store::StoreError::InvalidData(e.to_string()))?;
            return Ok(());
        }
        let turn = value["turn"].as_str();
        let require_turn = || {
            turn.ok_or_else(|| crate::store::StoreError::InvalidData("Pi event has no turn".into()))
        };
        if value["type"] == "agent_start" {
            store.record_session_turn_origin(
                &thread,
                require_turn()?,
                &store.session_turn_origin(&adapter.session, &attachment)?,
            )?;
        }
        let settled = value["type"] == "agent_settled";
        let mut yielded = turn.is_none() && value["type"] == "ui_prompt_end";
        if settled || (value["type"] == "message_end" && value["message"]["role"] == "assistant") {
            let mut state = super::pi_mapping::Turn::new(require_turn()?.into());
            let mut final_events = Vec::new();
            for event in value["events"].as_array().into_iter().flatten() {
                // The extension supplies cumulative evidence; retain only the latest
                // checkpoint. Existing receipt keys make repeated observations idempotent.
                final_events = state.event(event);
            }
            if settled {
                final_events = state.event(value);
            }
            yielded = final_events.iter().any(|event| {
                matches!(
                    event,
                    ConversationEvent::TurnCompleted {
                        status: Lifecycle::Completed,
                        ..
                    }
                )
            });
            record(store, &adapter.session, &thread, &final_events)?;
        }
        let activity = crate::session::SessionActivity {
            observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
            open_tools: value["openTools"].as_u64().ok_or_else(|| {
                crate::store::StoreError::InvalidData("Pi callback has no tool count".into())
            })? as usize,
            pending_input: value["pendingInput"].as_u64().ok_or_else(|| {
                crate::store::StoreError::InvalidData("Pi callback has no input count".into())
            })? as usize,
            yielded,
            preview: None,
        };
        store.record_session_activity(&adapter.session, &attachment, &activity)?;
        crate::transport::report(
            if activity.pending_input > 0 {
                crate::program_status::State::Blocked
            } else if yielded {
                crate::program_status::State::Idle
            } else if settled {
                crate::program_status::State::Error
            } else {
                crate::program_status::State::Working
            },
            false,
            None,
        );
        Ok(())
    })?;
    Ok(())
}
