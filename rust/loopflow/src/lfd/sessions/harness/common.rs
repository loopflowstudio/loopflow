use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::task::JoinHandle;

use crate::lfd::sessions::types::{ItemStatus, SessionItem};

#[derive(Debug)]
pub(super) struct TurnInProgressGuard {
    flag: Arc<AtomicBool>,
    armed: bool,
}

impl TurnInProgressGuard {
    pub(super) fn new(flag: Arc<AtomicBool>) -> Self {
        Self { flag, armed: true }
    }

    pub(super) fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TurnInProgressGuard {
    fn drop(&mut self) {
        if self.armed {
            self.flag.store(false, Ordering::SeqCst);
        }
    }
}

pub(super) fn spawn_stderr_logger<R>(stderr: R, target: &'static str) -> JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if !line.trim().is_empty() {
                tracing::debug!(stderr_target = target, "{line}");
            }
        }
    })
}

#[derive(Debug, Default)]
pub(super) struct InFlightItems {
    items: HashMap<String, InFlightItem>,
}

#[derive(Debug, Clone)]
struct InFlightItem {
    turn_id: String,
    item: SessionItem,
}

impl InFlightItems {
    pub(super) fn insert(&mut self, turn_id: &str, item: &SessionItem) {
        let Some(item_id) = item_id(item) else {
            return;
        };
        self.items.insert(
            item_id.to_string(),
            InFlightItem {
                turn_id: turn_id.to_string(),
                item: item.clone(),
            },
        );
    }

    pub(super) fn remove(&mut self, item: &SessionItem) {
        let Some(item_id) = item_id(item) else {
            return;
        };
        self.items.remove(item_id);
    }

    pub(super) fn drain_failed(&mut self, metadata: &str) -> Vec<(String, SessionItem)> {
        let drained = std::mem::take(&mut self.items);
        drained
            .into_values()
            .map(|entry| (entry.turn_id, mark_item_failed(entry.item, metadata)))
            .collect()
    }

    pub(super) fn clear(&mut self) {
        self.items.clear();
    }
}

fn item_id(item: &SessionItem) -> Option<&str> {
    match item {
        SessionItem::Command { id, .. }
        | SessionItem::File { id, .. }
        | SessionItem::Message { id, .. }
        | SessionItem::Thought { id, .. }
        | SessionItem::Tool { id, .. } => Some(id.as_str()),
    }
}

fn mark_item_failed(item: SessionItem, metadata: &str) -> SessionItem {
    let crash_note = format!("harness crash: {metadata}");
    match item {
        SessionItem::Command {
            id,
            command,
            cwd,
            output,
            ..
        } => SessionItem::Command {
            id,
            command,
            cwd,
            status: ItemStatus::Failed,
            output: Some(append_output(output, &crash_note)),
            exit_code: None,
            duration_ms: None,
        },
        SessionItem::File { id, changes, .. } => SessionItem::File {
            id,
            changes,
            status: ItemStatus::Failed,
        },
        SessionItem::Tool {
            id,
            name,
            input,
            output,
            ..
        } => SessionItem::Tool {
            id,
            name,
            status: ItemStatus::Failed,
            input,
            output: Some(append_output(output, &crash_note)),
        },
        SessionItem::Message { id, text, phase } => SessionItem::Message {
            id,
            text: append_output(Some(text), &crash_note),
            phase,
        },
        SessionItem::Thought { id, text } => SessionItem::Thought {
            id,
            text: append_output(Some(text), &crash_note),
        },
    }
}

fn append_output(existing: Option<String>, metadata: &str) -> String {
    match existing {
        Some(existing) if existing.is_empty() => metadata.to_string(),
        Some(existing) => format!("{existing}\n{metadata}"),
        None => metadata.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn in_flight_items_remove_unknown_is_noop() {
        let mut tracker = InFlightItems::default();
        tracker.remove(&SessionItem::Tool {
            id: "unknown".to_string(),
            name: "tool".to_string(),
            status: ItemStatus::InProgress,
            input: None,
            output: None,
        });
        assert!(tracker.items.is_empty());
    }

    #[test]
    fn in_flight_items_drain_marks_items_failed() {
        let mut tracker = InFlightItems::default();
        let command = SessionItem::Command {
            id: "cmd_1".to_string(),
            command: vec!["git".to_string(), "status".to_string()],
            cwd: "/tmp".to_string(),
            status: ItemStatus::InProgress,
            output: Some("partial".to_string()),
            exit_code: None,
            duration_ms: None,
        };
        tracker.insert("turn_1", &command);

        let drained = tracker.drain_failed("codex exited");
        assert_eq!(drained.len(), 1);
        assert!(tracker.items.is_empty());

        let (turn_id, item) = &drained[0];
        assert_eq!(turn_id, "turn_1");
        match item {
            SessionItem::Command { status, output, .. } => {
                assert_eq!(*status, ItemStatus::Failed);
                assert_eq!(
                    output.as_deref(),
                    Some("partial\nharness crash: codex exited")
                );
            }
            other => panic!("expected failed command item, got {other:?}"),
        }
    }
}
