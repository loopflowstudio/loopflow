use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::json;
use tokio::sync::{broadcast, Mutex};
use tokio::task::JoinHandle;

use crate::agent::{tools, turn};
use crate::chat::{validate_turn_completion, AgentEvent, CompletionError};
use crate::lfd::sessions::harness::common::TurnInProgressGuard;
use crate::lfd::sessions::harness::{SessionHarness, SessionHarnessError};
use crate::lfd::sessions::types::{
    ItemStatus, SessionConfig, SessionEvent, SessionItem, TurnStatus,
};

pub struct LfHarness {
    events: broadcast::Sender<SessionEvent>,
    config: Option<SessionConfig>,
    turn_in_progress: Arc<AtomicBool>,
    turn_task: Option<JoinHandle<()>>,
    active_turn_id: Arc<Mutex<Option<String>>>,
    shutdown_requested: Arc<AtomicBool>,
}

impl std::fmt::Debug for LfHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("LfHarness").finish()
    }
}

impl LfHarness {
    pub fn new(events: broadcast::Sender<SessionEvent>) -> Self {
        Self {
            events,
            config: None,
            turn_in_progress: Arc::new(AtomicBool::new(false)),
            turn_task: None,
            active_turn_id: Arc::new(Mutex::new(None)),
            shutdown_requested: Arc::new(AtomicBool::new(false)),
        }
    }
}

fn map_agent_event(
    turn_id: &str,
    event: &AgentEvent,
    memory_edit_seq: &mut u64,
) -> Option<SessionEvent> {
    match event {
        AgentEvent::Message { content, .. } => Some(SessionEvent::TextDelta {
            turn_id: turn_id.to_string(),
            content: content.clone(),
        }),
        AgentEvent::MemoryEdit { op, block, detail } => {
            *memory_edit_seq = memory_edit_seq.saturating_add(1);
            Some(SessionEvent::ItemCompleted {
                turn_id: turn_id.to_string(),
                item: SessionItem::Tool {
                    id: format!("memory_edit_{}", memory_edit_seq),
                    name: "memory_edit".to_string(),
                    status: ItemStatus::Completed,
                    input: Some(json!({
                        "op": op,
                        "block": block,
                        "detail": detail,
                    })),
                    output: Some("edit recorded".to_string()),
                },
            })
        }
        _ => None,
    }
}

fn completion_error_code(error: &CompletionError) -> &'static str {
    match error {
        CompletionError::MissingFinalMessage => "missing_final_message",
        CompletionError::MultipleFinalMessages => "multiple_final_messages",
        CompletionError::FinalMessageOnFailedTurn => "final_message_on_failed_turn",
    }
}

#[async_trait]
impl SessionHarness for LfHarness {
    async fn start(&mut self, config: &SessionConfig) -> Result<()> {
        self.config = Some(config.clone());
        Ok(())
    }

    async fn send_input(&mut self, content: &str) -> Result<()> {
        let text = content.trim();
        if text.is_empty() {
            return Ok(());
        }

        if self
            .turn_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(SessionHarnessError::TurnAlreadyInProgress.into());
        }
        let mut turn_guard = TurnInProgressGuard::new(self.turn_in_progress.clone());

        let config = self
            .config
            .as_ref()
            .ok_or_else(|| anyhow!("lfharness not started"))?
            .clone();

        let turn_id = format!("turn_{}", uuid::Uuid::new_v4());
        {
            let mut active_turn_id = self.active_turn_id.lock().await;
            *active_turn_id = Some(turn_id.clone());
        }
        let _ = self.events.send(SessionEvent::TurnStarted {
            turn_id: turn_id.clone(),
        });

        let mut turn_config = turn::TurnConfig {
            system: config.system_prompt.clone(),
            ..Default::default()
        };
        if let Some(max_turns) = config.max_turns {
            turn_config.max_iterations = max_turns;
        }

        self.shutdown_requested.store(false, Ordering::SeqCst);

        let events = self.events.clone();
        let turn_in_progress = self.turn_in_progress.clone();
        let active_turn_id = self.active_turn_id.clone();
        let shutdown_requested = self.shutdown_requested.clone();
        let prompt = text.to_string();
        let task_turn_id = turn_id.clone();
        self.turn_task = Some(tokio::spawn(async move {
            let registry = tools::default_registry();
            let mut memory_edit_seq = 0_u64;

            let run_result =
                turn::run_with_event_handler(&prompt, &turn_config, &registry, |agent_event| {
                    if let Some(event) =
                        map_agent_event(&task_turn_id, agent_event, &mut memory_edit_seq)
                    {
                        let _ = events.send(event);
                    }
                })
                .await;

            if !shutdown_requested.load(Ordering::SeqCst) {
                match run_result {
                    Ok(result) => match validate_turn_completion(&result.events) {
                        Ok(()) => {
                            let _ = events.send(SessionEvent::TurnCompleted {
                                turn_id: task_turn_id.clone(),
                                status: TurnStatus::Completed,
                            });
                        }
                        Err(error) => {
                            let _ = events.send(SessionEvent::Error {
                                code: completion_error_code(&error).to_string(),
                                message: error.to_string(),
                            });
                            let _ = events.send(SessionEvent::TurnCompleted {
                                turn_id: task_turn_id.clone(),
                                status: TurnStatus::Failed,
                            });
                        }
                    },
                    Err(error) => {
                        let _ = events.send(SessionEvent::Error {
                            code: "chat_turn_failed".to_string(),
                            message: error.to_string(),
                        });
                        let _ = events.send(SessionEvent::TurnCompleted {
                            turn_id: task_turn_id.clone(),
                            status: TurnStatus::Failed,
                        });
                    }
                }
            }

            turn_in_progress.store(false, Ordering::SeqCst);
            let mut current_turn_id = active_turn_id.lock().await;
            *current_turn_id = None;
        }));

        turn_guard.disarm();
        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.shutdown_requested.store(true, Ordering::SeqCst);

        if let Some(task) = self.turn_task.take() {
            task.abort();
            let _ = task.await;
        }

        let turn_id = {
            let mut active_turn_id = self.active_turn_id.lock().await;
            active_turn_id.take()
        };

        if self.turn_in_progress.swap(false, Ordering::SeqCst) {
            if let Some(turn_id) = turn_id {
                let _ = self.events.send(SessionEvent::TurnCompleted {
                    turn_id,
                    status: TurnStatus::Interrupted,
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::UserMessagePhase;

    #[test]
    fn map_agent_event_message_emits_text_delta() {
        let mut memory_edit_seq = 0_u64;
        let event = AgentEvent::Message {
            content: "working".to_string(),
            phase: UserMessagePhase::Progress,
        };
        let mapped = map_agent_event("turn_1", &event, &mut memory_edit_seq)
            .expect("message event should map");
        match mapped {
            SessionEvent::TextDelta { turn_id, content } => {
                assert_eq!(turn_id, "turn_1");
                assert_eq!(content, "working");
            }
            other => panic!("expected text delta, got {other:?}"),
        }
    }

    #[test]
    fn map_agent_event_memory_edit_emits_tool_item() {
        let mut memory_edit_seq = 0_u64;
        let event = AgentEvent::MemoryEdit {
            op: "upsert".to_string(),
            block: "prefs".to_string(),
            detail: "Use short answers".to_string(),
        };
        let mapped = map_agent_event("turn_1", &event, &mut memory_edit_seq)
            .expect("memory edit event should map");
        match mapped {
            SessionEvent::ItemCompleted { turn_id, item } => {
                assert_eq!(turn_id, "turn_1");
                match item {
                    SessionItem::Tool {
                        id,
                        name,
                        status,
                        input,
                        output,
                    } => {
                        assert_eq!(id, "memory_edit_1");
                        assert_eq!(name, "memory_edit");
                        assert_eq!(status, ItemStatus::Completed);
                        assert_eq!(
                            input,
                            Some(json!({
                                "op": "upsert",
                                "block": "prefs",
                                "detail": "Use short answers",
                            }))
                        );
                        assert_eq!(output.as_deref(), Some("edit recorded"));
                    }
                    other => panic!("expected tool item, got {other:?}"),
                }
            }
            other => panic!("expected item completed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn stop_running_turn_emits_interrupted_completion() {
        let (tx, mut rx) = broadcast::channel(16);
        let mut harness = LfHarness::new(tx);
        harness.turn_in_progress.store(true, Ordering::SeqCst);
        *harness.active_turn_id.lock().await = Some("turn_123".to_string());

        harness.stop().await.expect("stop should succeed");

        let event = rx.try_recv().expect("stop should emit turn completion");
        match event {
            SessionEvent::TurnCompleted { turn_id, status } => {
                assert_eq!(turn_id, "turn_123");
                assert_eq!(status, TurnStatus::Interrupted);
            }
            other => panic!("expected turn completed, got {other:?}"),
        }
    }
}
