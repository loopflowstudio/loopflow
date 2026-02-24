use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::broadcast;
use tokio::task::JoinHandle;

use crate::lfd::sessions::adapter::SessionAdapter;
use crate::lfd::sessions::types::{
    FileEdit, ItemStatus, SessionConfig, SessionEvent, SessionItem, TurnStatus,
};

pub struct ClaudeAdapter {
    events: broadcast::Sender<SessionEvent>,
    config: Option<SessionConfig>,
    provider_session_id: Option<String>,
    turn_in_progress: Arc<AtomicBool>,
    child: Option<Child>,
    reader_task: Option<JoinHandle<()>>,
    stderr_task: Option<JoinHandle<()>>,
    shutdown_requested: Arc<AtomicBool>,
}

impl std::fmt::Debug for ClaudeAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaudeAdapter").finish()
    }
}

/// Map Claude tool name to a SessionItem category.
fn infer_item(tool_name: &str, tool_use_id: &str, input: Option<Value>) -> SessionItem {
    build_item(tool_name, tool_use_id, input, ItemStatus::InProgress, None)
}

fn build_item(
    tool_name: &str,
    tool_use_id: &str,
    input: Option<Value>,
    status: ItemStatus,
    output: Option<String>,
) -> SessionItem {
    match tool_name {
        "Bash" => SessionItem::Command {
            id: tool_use_id.to_string(),
            command: command_from_input(input.as_ref()),
            cwd: String::new(),
            status,
            output,
            exit_code: None,
            duration_ms: None,
        },
        "Edit" | "Write" | "NotebookEdit" => SessionItem::File {
            id: tool_use_id.to_string(),
            changes: file_changes_from_input(tool_name, input.as_ref()),
            status,
        },
        _ => SessionItem::Tool {
            id: tool_use_id.to_string(),
            name: tool_name.to_string(),
            status,
            input,
            output,
        },
    }
}

fn command_from_input(input: Option<&Value>) -> Vec<String> {
    let command = input
        .and_then(|value| value.get("command"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if command.is_empty() {
        Vec::new()
    } else {
        vec![command.to_string()]
    }
}

fn file_changes_from_input(tool_name: &str, input: Option<&Value>) -> Vec<FileEdit> {
    let path = input
        .and_then(|value| {
            value
                .get("file_path")
                .or_else(|| value.get("notebook_path"))
        })
        .and_then(Value::as_str)
        .unwrap_or_default();
    if path.is_empty() {
        Vec::new()
    } else {
        vec![FileEdit {
            path: path.to_string(),
            kind: Some(tool_name.to_lowercase()),
            diff: None,
        }]
    }
}

/// Build CLI args for a Claude invocation.
fn build_args(content: &str, config: &SessionConfig, resume_id: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "-p".to_string(),
        content.to_string(),
        "--output-format".to_string(),
        "stream-json".to_string(),
        "--verbose".to_string(),
    ];

    if let Some(id) = resume_id {
        args.push("--resume".to_string());
        args.push(id.to_string());
    }

    if let Some(model) = &config.model {
        args.push("--model".to_string());
        args.push(model.clone());
    }

    if config.yolo_mode {
        args.push("--dangerously-skip-permissions".to_string());
    }

    if let Some(max_turns) = config.max_turns {
        args.push("--max-turns".to_string());
        args.push(max_turns.to_string());
    }

    if let Some(system_prompt) = &config.system_prompt {
        args.push("--append-system-prompt".to_string());
        args.push(system_prompt.clone());
    }

    args
}

/// Reader-local state for tracking in-flight content blocks.
#[derive(Debug, Default)]
struct ReaderState {
    /// Accumulated input_json_delta fragments per tool_use block index.
    tool_inputs: HashMap<usize, String>,
    /// Map block index → (tool_use_id, tool_name).
    tool_blocks: HashMap<usize, (String, String)>,
}

impl ClaudeAdapter {
    pub fn new(events: broadcast::Sender<SessionEvent>) -> Self {
        Self {
            events,
            config: None,
            provider_session_id: None,
            turn_in_progress: Arc::new(AtomicBool::new(false)),
            child: None,
            reader_task: None,
            stderr_task: None,
            shutdown_requested: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Parse a single NDJSON line and emit SessionEvents.
    fn process_line(
        line: &str,
        turn_id: &str,
        events: &broadcast::Sender<SessionEvent>,
        state: &mut ReaderState,
    ) -> bool {
        let Ok(value) = serde_json::from_str::<Value>(line) else {
            return false;
        };
        let Some(event_type) = value.get("type").and_then(Value::as_str) else {
            return false;
        };

        match event_type {
            "system" => {
                if let Some(session_id) = value.get("session_id").and_then(Value::as_str) {
                    let _ = events.send(SessionEvent::ProviderSessionId {
                        provider_session_id: session_id.to_string(),
                    });
                }
            }

            "content_block_start" => {
                let Some(index) = value.get("index").and_then(Value::as_u64) else {
                    return false;
                };
                let Some(block) = value.get("content_block") else {
                    return false;
                };
                let Some(block_type) = block.get("type").and_then(Value::as_str) else {
                    return false;
                };
                let index = index as usize;

                if block_type == "tool_use" {
                    let Some(tool_id) = block.get("id").and_then(Value::as_str) else {
                        return false;
                    };
                    let Some(tool_name) = block.get("name").and_then(Value::as_str) else {
                        return false;
                    };
                    let tool_id = tool_id.to_string();
                    let tool_name = tool_name.to_string();
                    state
                        .tool_blocks
                        .insert(index, (tool_id.clone(), tool_name.clone()));
                    state.tool_inputs.insert(index, String::new());

                    let item = infer_item(&tool_name, &tool_id, None);
                    let _ = events.send(SessionEvent::ItemStarted {
                        turn_id: turn_id.to_string(),
                        item,
                    });
                }
            }

            "content_block_delta" => {
                let index = value
                    .get("index")
                    .and_then(Value::as_u64)
                    .map(|v| v as usize);
                let Some(delta) = value.get("delta") else {
                    return false;
                };
                let Some(delta_type) = delta.get("type").and_then(Value::as_str) else {
                    return false;
                };

                match delta_type {
                    "text_delta" => {
                        if let Some(text) = delta.get("text").and_then(Value::as_str) {
                            let _ = events.send(SessionEvent::TextDelta {
                                turn_id: turn_id.to_string(),
                                content: text.to_string(),
                            });
                        }
                    }
                    "thinking_delta" | "summary_delta" => {
                        if let Some(text) = delta
                            .get("thinking")
                            .or_else(|| delta.get("summary"))
                            .and_then(Value::as_str)
                        {
                            let _ = events.send(SessionEvent::ReasoningDelta {
                                turn_id: turn_id.to_string(),
                                content: text.to_string(),
                            });
                        }
                    }
                    "input_json_delta" => {
                        if let (Some(idx), Some(json_chunk)) =
                            (index, delta.get("partial_json").and_then(Value::as_str))
                        {
                            if let Some(buf) = state.tool_inputs.get_mut(&idx) {
                                buf.push_str(json_chunk);
                            }
                        }
                    }
                    _ => {}
                }
            }

            "content_block_stop" => {
                // Tool blocks will get completed when we see the tool result.
                // Text blocks don't need explicit completion.
            }

            "result" => {
                let _ = events.send(SessionEvent::TurnCompleted {
                    turn_id: turn_id.to_string(),
                    status: TurnStatus::Completed,
                });
                return true;
            }

            // Claude emits "assistant" events with full message content.
            // These contain tool_use blocks and text blocks.
            "assistant" => {
                Self::process_assistant_message(&value, turn_id, events, state);
            }

            // Tool result events: type "user" with tool_result content.
            "user" => {
                Self::process_user_message(&value, turn_id, events, state);
            }

            _ => {}
        }

        false
    }

    fn process_assistant_message(
        value: &Value,
        turn_id: &str,
        events: &broadcast::Sender<SessionEvent>,
        state: &mut ReaderState,
    ) {
        let content = value
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(Value::as_array);
        let Some(blocks) = content else { return };

        for (idx, block) in blocks.iter().enumerate() {
            let Some(block_type) = block.get("type").and_then(Value::as_str) else {
                continue;
            };
            match block_type {
                "tool_use" => {
                    let tool_id = block
                        .get("id")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let tool_name = block
                        .get("name")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let input = block.get("input").cloned();

                    // If we haven't already emitted ItemStarted via streaming,
                    // emit both started and track it.
                    if let std::collections::hash_map::Entry::Vacant(e) =
                        state.tool_blocks.entry(idx)
                    {
                        e.insert((tool_id.clone(), tool_name.clone()));
                        let item = infer_item(&tool_name, &tool_id, input);
                        let _ = events.send(SessionEvent::ItemStarted {
                            turn_id: turn_id.to_string(),
                            item,
                        });
                    }
                }
                "text" => {
                    if let Some(text) = block.get("text").and_then(Value::as_str) {
                        if !text.is_empty() {
                            let _ = events.send(SessionEvent::TextDelta {
                                turn_id: turn_id.to_string(),
                                content: text.to_string(),
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    }

    fn process_user_message(
        value: &Value,
        turn_id: &str,
        events: &broadcast::Sender<SessionEvent>,
        state: &mut ReaderState,
    ) {
        let content = value
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(Value::as_array);
        let Some(blocks) = content else { return };

        for block in blocks {
            let Some(block_type) = block.get("type").and_then(Value::as_str) else {
                continue;
            };
            if block_type != "tool_result" {
                continue;
            }

            let tool_use_id = block
                .get("tool_use_id")
                .and_then(Value::as_str)
                .unwrap_or_default();

            let Some((tool_index, tool_name)) =
                state.tool_blocks.iter().find_map(|(index, (id, name))| {
                    if id == tool_use_id {
                        Some((*index, name.clone()))
                    } else {
                        None
                    }
                })
            else {
                continue;
            };

            // Extract output from content array or text.
            let output = Self::extract_tool_result_text(block);

            // Get accumulated input for this tool.
            let input: Option<Value> = state.tool_inputs.get(&tool_index).and_then(|s| {
                if s.is_empty() {
                    None
                } else {
                    serde_json::from_str(s).ok()
                }
            });

            let completed_item = complete_item(&tool_name, tool_use_id, input, output);
            let _ = events.send(SessionEvent::ItemCompleted {
                turn_id: turn_id.to_string(),
                item: completed_item,
            });
        }
    }

    fn extract_tool_result_text(block: &Value) -> Option<String> {
        // Try nested content array first.
        if let Some(content) = block.get("content").and_then(Value::as_array) {
            let texts: Vec<&str> = content
                .iter()
                .filter_map(|c| {
                    if c.get("type").and_then(Value::as_str) == Some("text") {
                        c.get("text").and_then(Value::as_str)
                    } else {
                        None
                    }
                })
                .collect();
            if !texts.is_empty() {
                return Some(texts.join("\n"));
            }
        }
        // Try direct text field.
        block
            .get("content")
            .and_then(Value::as_str)
            .map(String::from)
    }
}

fn complete_item(
    tool_name: &str,
    tool_use_id: &str,
    input: Option<Value>,
    output: Option<String>,
) -> SessionItem {
    build_item(tool_name, tool_use_id, input, ItemStatus::Completed, output)
}

#[async_trait]
impl SessionAdapter for ClaudeAdapter {
    async fn start(&mut self, config: &SessionConfig) -> Result<()> {
        // Validate claude binary on PATH.
        let output = Command::new("claude").arg("--version").output().await;
        match output {
            Ok(out) if out.status.success() => {
                let version = String::from_utf8_lossy(&out.stdout);
                tracing::info!(version = %version.trim(), "claude binary found");
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr);
                return Err(anyhow!(
                    "claude --version failed (exit {}): {stderr}",
                    out.status
                ));
            }
            Err(err) => {
                return Err(anyhow!(
                    "claude binary not found on PATH: {err}. Install Claude Code first."
                ));
            }
        }

        self.config = Some(config.clone());
        Ok(())
    }

    async fn send_input(&mut self, content: &str) -> Result<()> {
        if self
            .turn_in_progress
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return Err(anyhow!("turn already in progress"));
        }

        let config = self
            .config
            .as_ref()
            .ok_or_else(|| anyhow!("claude adapter not started"))?;

        let turn_id = format!("turn_{}", uuid::Uuid::new_v4());

        let _ = self.events.send(SessionEvent::TurnStarted {
            turn_id: turn_id.clone(),
        });

        let args = build_args(content, config, self.provider_session_id.as_deref());
        let mut cmd = Command::new("claude");
        cmd.args(&args);
        cmd.stdout(std::process::Stdio::piped());
        cmd.stderr(std::process::Stdio::piped());
        cmd.stdin(std::process::Stdio::null());

        if let Some(cwd) = &config.cwd {
            cmd.current_dir(cwd);
        }

        let mut child = cmd
            .spawn()
            .map_err(|err| anyhow!("failed to spawn claude: {err}"))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("failed to capture claude stdout"))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("failed to capture claude stderr"))?;

        self.child = Some(child);

        // Spawn reader task for NDJSON stdout.
        let events = self.events.clone();
        let turn_in_progress = self.turn_in_progress.clone();
        let shutdown = self.shutdown_requested.clone();
        let reader_turn_id = turn_id.clone();
        self.reader_task = Some(tokio::spawn(async move {
            let reader = BufReader::new(stdout);
            let mut lines = reader.lines();
            let mut state = ReaderState::default();
            let mut saw_turn_completed = false;

            while let Ok(Some(line)) = lines.next_line().await {
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                if line.trim().is_empty() {
                    continue;
                }

                if ClaudeAdapter::process_line(&line, &reader_turn_id, &events, &mut state) {
                    saw_turn_completed = true;
                    break;
                }
            }

            if !saw_turn_completed && !shutdown.load(Ordering::Relaxed) {
                tracing::warn!(
                    turn_id = %reader_turn_id,
                    "claude turn ended without result event"
                );
                let _ = events.send(SessionEvent::TurnCompleted {
                    turn_id: reader_turn_id,
                    status: TurnStatus::Failed,
                });
            }

            turn_in_progress.store(false, Ordering::SeqCst);
        }));

        // Spawn stderr task for logging.
        self.stderr_task = Some(tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                if !line.trim().is_empty() {
                    tracing::debug!(target: "claude_adapter", "{line}");
                }
            }
        }));

        Ok(())
    }

    async fn stop(&mut self) -> Result<()> {
        self.shutdown_requested.store(true, Ordering::SeqCst);

        if let Some(mut child) = self.child.take() {
            let _ = child.kill().await;
            let _ = child.wait().await;
        }

        if let Some(task) = self.reader_task.take() {
            task.abort();
        }
        if let Some(task) = self.stderr_task.take() {
            task.abort();
        }

        self.turn_in_progress.store(false, Ordering::SeqCst);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn infer_item_bash_to_command() {
        let item = infer_item("Bash", "tu_1", Some(json!({"command": "cargo test"})));
        match item {
            SessionItem::Command { id, command, .. } => {
                assert_eq!(id, "tu_1");
                assert_eq!(command, vec!["cargo test"]);
            }
            other => panic!("expected Command, got {other:?}"),
        }
    }

    #[test]
    fn infer_item_edit_to_file() {
        let item = infer_item("Edit", "tu_2", Some(json!({"file_path": "src/main.rs"})));
        match item {
            SessionItem::File { id, changes, .. } => {
                assert_eq!(id, "tu_2");
                assert_eq!(changes[0].path, "src/main.rs");
                assert_eq!(changes[0].kind.as_deref(), Some("edit"));
            }
            other => panic!("expected File, got {other:?}"),
        }
    }

    #[test]
    fn infer_item_write_to_file() {
        let item = infer_item("Write", "tu_3", Some(json!({"file_path": "new.txt"})));
        match item {
            SessionItem::File { id, changes, .. } => {
                assert_eq!(id, "tu_3");
                assert_eq!(changes[0].path, "new.txt");
                assert_eq!(changes[0].kind.as_deref(), Some("write"));
            }
            other => panic!("expected File, got {other:?}"),
        }
    }

    #[test]
    fn infer_item_notebook_edit_to_file() {
        let item = infer_item(
            "NotebookEdit",
            "tu_4",
            Some(json!({"notebook_path": "analysis.ipynb"})),
        );
        match item {
            SessionItem::File { id, changes, .. } => {
                assert_eq!(id, "tu_4");
                assert_eq!(changes[0].path, "analysis.ipynb");
            }
            other => panic!("expected File, got {other:?}"),
        }
    }

    #[test]
    fn infer_item_unknown_to_tool() {
        let item = infer_item("WebSearch", "tu_5", Some(json!({"query": "rust async"})));
        match item {
            SessionItem::Tool {
                id, name, input, ..
            } => {
                assert_eq!(id, "tu_5");
                assert_eq!(name, "WebSearch");
                assert_eq!(input, Some(json!({"query": "rust async"})));
            }
            other => panic!("expected Tool, got {other:?}"),
        }
    }

    #[test]
    fn build_args_minimal() {
        let config = SessionConfig {
            model: None,
            cwd: None,
            system_prompt: None,
            max_turns: None,
            yolo_mode: false,
        };
        let args = build_args("hello", &config, None);
        assert_eq!(
            args,
            vec!["-p", "hello", "--output-format", "stream-json", "--verbose"]
        );
    }

    #[test]
    fn build_args_full() {
        let config = SessionConfig {
            model: Some("claude-sonnet-4-5-20250514".to_string()),
            cwd: Some("/tmp".to_string()),
            system_prompt: Some("Be concise".to_string()),
            max_turns: Some(5),
            yolo_mode: true,
        };
        let args = build_args("fix tests", &config, Some("sess_abc"));
        assert!(args.contains(&"--resume".to_string()));
        assert!(args.contains(&"sess_abc".to_string()));
        assert!(args.contains(&"--model".to_string()));
        assert!(args.contains(&"claude-sonnet-4-5-20250514".to_string()));
        assert!(args.contains(&"--dangerously-skip-permissions".to_string()));
        assert!(args.contains(&"--max-turns".to_string()));
        assert!(args.contains(&"5".to_string()));
        assert!(args.contains(&"--append-system-prompt".to_string()));
        assert!(args.contains(&"Be concise".to_string()));
    }

    #[test]
    fn build_args_resume_without_extras() {
        let config = SessionConfig::default();
        let args = build_args("next", &config, Some("sess_123"));
        assert!(args.contains(&"--resume".to_string()));
        assert!(args.contains(&"sess_123".to_string()));
        assert!(!args.contains(&"--model".to_string()));
        assert!(!args.contains(&"--dangerously-skip-permissions".to_string()));
    }

    #[test]
    fn process_line_system_event() {
        let (tx, mut rx) = broadcast::channel(16);
        let mut state = ReaderState::default();
        let line = r#"{"type":"system","session_id":"sess_abc123","tools":[]}"#;

        let result = ClaudeAdapter::process_line(line, "turn_1", &tx, &mut state);
        assert!(!result);

        let event = rx.try_recv().expect("should have event");
        match event {
            SessionEvent::ProviderSessionId {
                provider_session_id,
            } => {
                assert_eq!(provider_session_id, "sess_abc123");
            }
            other => panic!("expected ProviderSessionId event, got {other:?}"),
        }
    }

    #[test]
    fn process_line_text_delta() {
        let (tx, mut rx) = broadcast::channel(16);
        let mut state = ReaderState::default();
        let line = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello world"}}"#;

        ClaudeAdapter::process_line(line, "turn_1", &tx, &mut state);

        let event = rx.try_recv().expect("should have event");
        match event {
            SessionEvent::TextDelta { turn_id, content } => {
                assert_eq!(turn_id, "turn_1");
                assert_eq!(content, "Hello world");
            }
            other => panic!("expected TextDelta, got {other:?}"),
        }
    }

    #[test]
    fn process_line_thinking_delta() {
        let (tx, mut rx) = broadcast::channel(16);
        let mut state = ReaderState::default();
        let line = r#"{"type":"content_block_delta","index":0,"delta":{"type":"thinking_delta","thinking":"Let me think..."}}"#;

        ClaudeAdapter::process_line(line, "turn_1", &tx, &mut state);

        let event = rx.try_recv().expect("should have event");
        match event {
            SessionEvent::ReasoningDelta { turn_id, content } => {
                assert_eq!(turn_id, "turn_1");
                assert_eq!(content, "Let me think...");
            }
            other => panic!("expected ReasoningDelta, got {other:?}"),
        }
    }

    #[test]
    fn process_line_result_completes_turn() {
        let (tx, mut rx) = broadcast::channel(16);
        let mut state = ReaderState::default();
        let line = r#"{"type":"result","duration_ms":1234,"cost_usd":0.01,"is_error":false,"result":"done","session_id":"sess_abc"}"#;

        let result = ClaudeAdapter::process_line(line, "turn_1", &tx, &mut state);
        assert!(result);

        let event = rx.try_recv().expect("should have event");
        match event {
            SessionEvent::TurnCompleted { turn_id, status } => {
                assert_eq!(turn_id, "turn_1");
                assert_eq!(status, TurnStatus::Completed);
            }
            other => panic!("expected TurnCompleted, got {other:?}"),
        }
    }

    #[test]
    fn process_line_tool_use_start() {
        let (tx, mut rx) = broadcast::channel(16);
        let mut state = ReaderState::default();
        let line = r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"tu_bash_1","name":"Bash"}}"#;

        ClaudeAdapter::process_line(line, "turn_1", &tx, &mut state);

        assert!(state.tool_blocks.contains_key(&1));
        let (id, name) = &state.tool_blocks[&1];
        assert_eq!(id, "tu_bash_1");
        assert_eq!(name, "Bash");

        let event = rx.try_recv().expect("should have event");
        match event {
            SessionEvent::ItemStarted { turn_id, item } => {
                assert_eq!(turn_id, "turn_1");
                assert!(matches!(item, SessionItem::Command { .. }));
            }
            other => panic!("expected ItemStarted, got {other:?}"),
        }
    }

    #[test]
    fn process_line_input_json_delta_accumulates() {
        let (tx, _rx) = broadcast::channel(16);
        let mut state = ReaderState::default();

        // Start a tool block first.
        let start_line = r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"tu_1","name":"Bash"}}"#;
        ClaudeAdapter::process_line(start_line, "turn_1", &tx, &mut state);

        // Send partial input.
        let delta1 = r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"com"}}"#;
        let delta2 = r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"mand\":\"ls\"}"}}"#;
        ClaudeAdapter::process_line(delta1, "turn_1", &tx, &mut state);
        ClaudeAdapter::process_line(delta2, "turn_1", &tx, &mut state);

        assert_eq!(
            state.tool_inputs.get(&0).map(String::as_str),
            Some("{\"command\":\"ls\"}")
        );
    }
}
