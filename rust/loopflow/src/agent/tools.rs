use crate::agent::anthropic::ToolDefinition;
use crate::agent::context::ContextStore;
use crate::agent::registry::{Tool, ToolRegistry, ToolResult};
use crate::chat::{AgentEvent, UserMessagePhase};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

// --- Tool implementations ---

pub struct GetCurrentTime;

impl Tool for GetCurrentTime {
    fn name(&self) -> &str {
        "get_current_time"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "get_current_time".to_string(),
            description: "Get the current date and time in UTC.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    fn call(&self, _input: &serde_json::Value) -> ToolResult {
        ToolResult {
            output: chrono::Utc::now().to_rfc3339(),
            event: None,
        }
    }
}

pub struct Calculate;

impl Tool for Calculate {
    fn name(&self) -> &str {
        "calculate"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "calculate".to_string(),
            description: "Evaluate a simple arithmetic expression. Supports +, -, *, / with integers and floats.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "expression": {
                        "type": "string",
                        "description": "The arithmetic expression to evaluate, e.g. '2 + 3 * 4'"
                    }
                },
                "required": ["expression"]
            }),
        }
    }

    fn call(&self, input: &serde_json::Value) -> ToolResult {
        let output = match input.get("expression").and_then(|v| v.as_str()) {
            Some(expr) => match eval_expr(expr) {
                Ok(result) => format!("{result}"),
                Err(e) => format!("error: {e}"),
            },
            None => "error: missing 'expression' field".to_string(),
        };
        ToolResult {
            output,
            event: None,
        }
    }
}

// --- Boundary tools ---

pub struct SendMessage;

impl Tool for SendMessage {
    fn name(&self) -> &str {
        "send_message"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "send_message".to_string(),
            description: "Send a message to the user. Use phase 'progress' for intermediate updates and 'final' for the concluding message.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "content": {
                        "type": "string",
                        "description": "The message content to send"
                    },
                    "phase": {
                        "type": "string",
                        "enum": ["progress", "final"],
                        "description": "Message phase: 'progress' for intermediate, 'final' for concluding"
                    }
                },
                "required": ["content", "phase"]
            }),
        }
    }

    fn call(&self, input: &serde_json::Value) -> ToolResult {
        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let phase = match input.get("phase").and_then(|v| v.as_str()) {
            Some("final") => UserMessagePhase::Final,
            _ => UserMessagePhase::Progress,
        };

        ToolResult {
            output: "message sent".to_string(),
            event: Some(AgentEvent::Message { content, phase }),
        }
    }
}

pub struct MemoryEdit;

impl Tool for MemoryEdit {
    fn name(&self) -> &str {
        "memory_edit"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "memory_edit".to_string(),
            description: "Edit the agent's persistent memory. Operations: 'upsert' to add or update a block, 'delete' to remove a block.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "op": {
                        "type": "string",
                        "enum": ["upsert", "delete"],
                        "description": "Operation: 'upsert' or 'delete'"
                    },
                    "block": {
                        "type": "string",
                        "description": "The memory block name"
                    },
                    "detail": {
                        "type": "string",
                        "description": "The content to store (for upsert) or reason for deletion"
                    }
                },
                "required": ["op", "block", "detail"]
            }),
        }
    }

    fn call(&self, input: &serde_json::Value) -> ToolResult {
        let op = input
            .get("op")
            .and_then(|v| v.as_str())
            .unwrap_or("upsert")
            .to_string();
        let block = input
            .get("block")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let detail = input
            .get("detail")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        ToolResult {
            output: "edit recorded".to_string(),
            event: Some(AgentEvent::MemoryEdit { op, block, detail }),
        }
    }
}

/// Build a registry with the default built-in tools.
pub fn default_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();
    registry.register(Box::new(GetCurrentTime));
    registry.register(Box::new(Calculate));
    registry.register(Box::new(SendMessage));
    registry.register(Box::new(MemoryEdit));
    registry
}

// --- Context tools ---

pub struct ContextRead {
    store: Arc<Mutex<ContextStore>>,
}

impl ContextRead {
    pub fn new(store: Arc<Mutex<ContextStore>>) -> Self {
        Self { store }
    }
}

impl Tool for ContextRead {
    fn name(&self) -> &str {
        "context_read"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "context_read".to_string(),
            description: "Read a named context block from the session scratchpad.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The context block name to read"
                    }
                },
                "required": ["name"]
            }),
        }
    }

    fn call(&self, input: &serde_json::Value) -> ToolResult {
        let name = input.get("name").and_then(|v| v.as_str()).unwrap_or("");

        let store = self.store.lock().expect("context store lock poisoned");
        let output = match store.read(name) {
            Some(content) => content.to_string(),
            None => format!("no context block named '{name}'"),
        };

        ToolResult {
            output,
            event: None,
        }
    }
}

pub struct ContextWrite {
    store: Arc<Mutex<ContextStore>>,
}

impl ContextWrite {
    pub fn new(store: Arc<Mutex<ContextStore>>) -> Self {
        Self { store }
    }
}

impl Tool for ContextWrite {
    fn name(&self) -> &str {
        "context_write"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "context_write".to_string(),
            description: "Write or overwrite a named context block in the session scratchpad."
                .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The context block name"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to store"
                    }
                },
                "required": ["name", "content"]
            }),
        }
    }

    fn call(&self, input: &serde_json::Value) -> ToolResult {
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let mut store = self.store.lock().expect("context store lock poisoned");
        store.write(name, content);

        ToolResult {
            output: "context block written".to_string(),
            event: None,
        }
    }
}

pub struct ContextDelete {
    store: Arc<Mutex<ContextStore>>,
}

impl ContextDelete {
    pub fn new(store: Arc<Mutex<ContextStore>>) -> Self {
        Self { store }
    }
}

impl Tool for ContextDelete {
    fn name(&self) -> &str {
        "context_delete"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "context_delete".to_string(),
            description: "Delete a named context block from the session scratchpad.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "name": {
                        "type": "string",
                        "description": "The context block name to delete"
                    }
                },
                "required": ["name"]
            }),
        }
    }

    fn call(&self, input: &serde_json::Value) -> ToolResult {
        let name = input.get("name").and_then(|v| v.as_str()).unwrap_or("");

        let mut store = self.store.lock().expect("context store lock poisoned");
        let output = if store.delete(name) {
            format!("deleted context block '{name}'")
        } else {
            format!("no context block named '{name}'")
        };

        ToolResult {
            output,
            event: None,
        }
    }
}

pub struct ContextList {
    store: Arc<Mutex<ContextStore>>,
}

impl ContextList {
    pub fn new(store: Arc<Mutex<ContextStore>>) -> Self {
        Self { store }
    }
}

impl Tool for ContextList {
    fn name(&self) -> &str {
        "context_list"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "context_list".to_string(),
            description: "List all context blocks in the session scratchpad with their approximate token counts.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        }
    }

    fn call(&self, _input: &serde_json::Value) -> ToolResult {
        let store = self.store.lock().expect("context store lock poisoned");
        let entries = store.list();

        let output = if entries.is_empty() {
            "no context blocks".to_string()
        } else {
            entries
                .iter()
                .map(|(name, tokens)| format!("{name}: {tokens} tokens"))
                .collect::<Vec<_>>()
                .join("\n")
        };

        ToolResult {
            output,
            event: None,
        }
    }
}

/// Build a registry with base tools + context tools.
pub fn registry_with_context(store: Arc<Mutex<ContextStore>>) -> ToolRegistry {
    let mut registry = default_registry();
    registry.register(Box::new(ContextRead::new(Arc::clone(&store))));
    registry.register(Box::new(ContextWrite::new(Arc::clone(&store))));
    registry.register(Box::new(ContextDelete::new(Arc::clone(&store))));
    registry.register(Box::new(ContextList::new(store)));
    registry
}

// --- File + shell tools ---

const SHELL_OUTPUT_MAX_BYTES: usize = 32_768; // ~8K tokens

/// Validate that a relative path resolves within the workspace root.
fn validate_path(workspace: &Path, relative: &str) -> Result<PathBuf, String> {
    // Reject obviously empty paths
    if relative.is_empty() {
        return Err("empty path".to_string());
    }

    let joined = workspace.join(relative);

    // Canonicalize workspace (must exist). For the joined path, we need to
    // handle the case where the file doesn't exist yet (write_file).
    let canon_workspace =
        std::fs::canonicalize(workspace).map_err(|e| format!("workspace error: {e}"))?;

    // Try to canonicalize the joined path. If it doesn't exist, canonicalize
    // the longest existing prefix and check that.
    let canon_joined = if joined.exists() {
        std::fs::canonicalize(&joined).map_err(|e| format!("path error: {e}"))?
    } else {
        // Find the existing parent, canonicalize it, then append the rest
        let mut existing = joined.clone();
        let mut suffix_parts = Vec::new();
        while !existing.exists() {
            if let Some(file_name) = existing.file_name() {
                suffix_parts.push(file_name.to_owned());
            } else {
                return Err("invalid path".to_string());
            }
            existing = match existing.parent() {
                Some(p) => p.to_path_buf(),
                None => return Err("invalid path".to_string()),
            };
        }
        let mut canon = std::fs::canonicalize(&existing).map_err(|e| format!("path error: {e}"))?;
        for part in suffix_parts.into_iter().rev() {
            canon.push(part);
        }
        canon
    };

    if canon_joined.starts_with(&canon_workspace) {
        Ok(canon_joined)
    } else {
        Err(format!("path escapes workspace: {relative}"))
    }
}

pub struct ReadFile {
    workspace: PathBuf,
}

impl ReadFile {
    pub fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }
}

impl Tool for ReadFile {
    fn name(&self) -> &str {
        "read_file"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "read_file".to_string(),
            description: "Read the contents of a file in the workspace.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path within the workspace"
                    }
                },
                "required": ["path"]
            }),
        }
    }

    fn call(&self, input: &serde_json::Value) -> ToolResult {
        let path_str = input.get("path").and_then(|v| v.as_str()).unwrap_or("");

        let output = match validate_path(&self.workspace, path_str) {
            Ok(path) => match std::fs::read_to_string(&path) {
                Ok(content) => content,
                Err(e) => format!("error reading file: {e}"),
            },
            Err(e) => format!("error: {e}"),
        };

        ToolResult {
            output,
            event: None,
        }
    }
}

pub struct WriteFile {
    workspace: PathBuf,
}

impl WriteFile {
    pub fn new(workspace: PathBuf) -> Self {
        Self { workspace }
    }
}

impl Tool for WriteFile {
    fn name(&self) -> &str {
        "write_file"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "write_file".to_string(),
            description:
                "Write content to a file in the workspace. Creates parent directories as needed."
                    .to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "Relative path within the workspace"
                    },
                    "content": {
                        "type": "string",
                        "description": "The content to write"
                    }
                },
                "required": ["path", "content"]
            }),
        }
    }

    fn call(&self, input: &serde_json::Value) -> ToolResult {
        let path_str = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
        let content = input.get("content").and_then(|v| v.as_str()).unwrap_or("");

        let output = match validate_path(&self.workspace, path_str) {
            Ok(path) => {
                if let Some(parent) = path.parent() {
                    if let Err(e) = std::fs::create_dir_all(parent) {
                        return ToolResult {
                            output: format!("error creating directories: {e}"),
                            event: None,
                        };
                    }
                }
                match std::fs::write(&path, content) {
                    Ok(()) => format!("wrote {} bytes to {path_str}", content.len()),
                    Err(e) => format!("error writing file: {e}"),
                }
            }
            Err(e) => format!("error: {e}"),
        };

        ToolResult {
            output,
            event: None,
        }
    }
}

pub struct Shell {
    workspace: PathBuf,
    timeout: Duration,
}

impl Shell {
    pub fn new(workspace: PathBuf, timeout: Duration) -> Self {
        Self { workspace, timeout }
    }
}

impl Tool for Shell {
    fn name(&self) -> &str {
        "shell"
    }

    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            name: "shell".to_string(),
            description: "Run a shell command in the workspace directory.".to_string(),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    }
                },
                "required": ["command"]
            }),
        }
    }

    fn call(&self, input: &serde_json::Value) -> ToolResult {
        let command = input.get("command").and_then(|v| v.as_str()).unwrap_or("");

        if command.is_empty() {
            return ToolResult {
                output: "error: empty command".to_string(),
                event: None,
            };
        }

        let output = match run_shell_command(command, &self.workspace, self.timeout) {
            Ok(out) => out,
            Err(e) => format!("error: {e}"),
        };

        ToolResult {
            output,
            event: None,
        }
    }
}

fn run_shell_command(
    command: &str,
    working_dir: &Path,
    timeout: Duration,
) -> Result<String, String> {
    let mut child = std::process::Command::new("sh")
        .arg("-c")
        .arg(command)
        .current_dir(working_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn error: {e}"))?;

    // Wait with timeout using a polling approach
    let start = std::time::Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let stdout = child.stdout.take().map_or_else(Vec::new, |mut s| {
                    let mut buf = Vec::new();
                    std::io::Read::read_to_end(&mut s, &mut buf).unwrap_or(0);
                    buf
                });
                let stderr = child.stderr.take().map_or_else(Vec::new, |mut s| {
                    let mut buf = Vec::new();
                    std::io::Read::read_to_end(&mut s, &mut buf).unwrap_or(0);
                    buf
                });

                let mut combined = String::new();
                if !stdout.is_empty() {
                    combined.push_str(&String::from_utf8_lossy(&stdout));
                }
                if !stderr.is_empty() {
                    if !combined.is_empty() {
                        combined.push('\n');
                    }
                    combined.push_str(&String::from_utf8_lossy(&stderr));
                }

                // Truncate if needed
                let truncated = if combined.len() > SHELL_OUTPUT_MAX_BYTES {
                    let mut s = combined[..SHELL_OUTPUT_MAX_BYTES].to_string();
                    s.push_str("\n[truncated]");
                    s
                } else {
                    combined
                };

                let exit_info = if status.success() {
                    String::new()
                } else {
                    format!("\n[exit code: {}]", status.code().unwrap_or(-1))
                };

                return Ok(format!("{truncated}{exit_info}"));
            }
            Ok(None) => {
                if start.elapsed() > timeout {
                    let _ = child.kill();
                    return Err("command timed out".to_string());
                }
                std::thread::sleep(Duration::from_millis(50));
            }
            Err(e) => return Err(format!("wait error: {e}")),
        }
    }
}

const DEFAULT_SHELL_TIMEOUT: Duration = Duration::from_secs(30);

/// Build a registry with all 9 tools.
pub fn full_registry(store: Arc<Mutex<ContextStore>>, workspace: PathBuf) -> ToolRegistry {
    let mut registry = registry_with_context(store);
    registry.register(Box::new(ReadFile::new(workspace.clone())));
    registry.register(Box::new(WriteFile::new(workspace.clone())));
    registry.register(Box::new(Shell::new(workspace, DEFAULT_SHELL_TIMEOUT)));
    registry
}

// --- Arithmetic evaluation ---

fn eval_expr(expr: &str) -> Result<f64, String> {
    let tokens = tokenize(expr)?;
    parse_expression(&tokens, &mut 0)
}

#[derive(Debug, Clone)]
enum Token {
    Num(f64),
    Op(char),
}

fn tokenize(expr: &str) -> Result<Vec<Token>, String> {
    let mut tokens = Vec::new();
    let mut chars = expr.chars().peekable();
    while let Some(&c) = chars.peek() {
        if c.is_whitespace() {
            chars.next();
        } else if c.is_ascii_digit() || c == '.' {
            let mut num = String::new();
            while let Some(&d) = chars.peek() {
                if d.is_ascii_digit() || d == '.' {
                    num.push(d);
                    chars.next();
                } else {
                    break;
                }
            }
            tokens.push(Token::Num(num.parse::<f64>().map_err(|e| e.to_string())?));
        } else if "+-*/".contains(c) {
            tokens.push(Token::Op(c));
            chars.next();
        } else {
            return Err(format!("unexpected character: {c}"));
        }
    }
    Ok(tokens)
}

// Recursive descent: expression = term ((+|-) term)*
fn parse_expression(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_term(tokens, pos)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Op('+') => {
                *pos += 1;
                left += parse_term(tokens, pos)?;
            }
            Token::Op('-') => {
                *pos += 1;
                left -= parse_term(tokens, pos)?;
            }
            _ => break,
        }
    }
    Ok(left)
}

// term = factor ((*|/) factor)*
fn parse_term(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    let mut left = parse_factor(tokens, pos)?;
    while *pos < tokens.len() {
        match &tokens[*pos] {
            Token::Op('*') => {
                *pos += 1;
                left *= parse_factor(tokens, pos)?;
            }
            Token::Op('/') => {
                *pos += 1;
                let right = parse_factor(tokens, pos)?;
                if right == 0.0 {
                    return Err("division by zero".to_string());
                }
                left /= right;
            }
            _ => break,
        }
    }
    Ok(left)
}

fn parse_factor(tokens: &[Token], pos: &mut usize) -> Result<f64, String> {
    if *pos >= tokens.len() {
        return Err("unexpected end of expression".to_string());
    }
    match &tokens[*pos] {
        Token::Num(n) => {
            let val = *n;
            *pos += 1;
            Ok(val)
        }
        _ => Err(format!("expected number, got {:?}", tokens[*pos])),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculate_simple() {
        let tool = Calculate;
        let result = tool.call(&serde_json::json!({"expression": "2 + 3"}));
        assert_eq!(result.output, "5");
        assert!(result.event.is_none());
    }

    #[test]
    fn calculate_precedence() {
        let tool = Calculate;
        let result = tool.call(&serde_json::json!({"expression": "2 + 3 * 4"}));
        assert_eq!(result.output, "14");
    }

    #[test]
    fn calculate_division() {
        let tool = Calculate;
        let result = tool.call(&serde_json::json!({"expression": "10 / 3"}));
        let val: f64 = result.output.parse().unwrap();
        assert!((val - 3.3333333333333335).abs() < 1e-10);
    }

    #[test]
    fn calculate_division_by_zero() {
        let tool = Calculate;
        let result = tool.call(&serde_json::json!({"expression": "5 / 0"}));
        assert!(result.output.contains("division by zero"));
    }

    #[test]
    fn calculate_missing_expression() {
        let tool = Calculate;
        let result = tool.call(&serde_json::json!({}));
        assert!(result.output.contains("missing 'expression' field"));
    }

    #[test]
    fn get_current_time_is_rfc3339() {
        let tool = GetCurrentTime;
        let result = tool.call(&serde_json::json!({}));
        assert!(result.output.contains('T'));
        assert!(result.output.ends_with("+00:00") || result.output.ends_with('Z'));
    }

    #[test]
    fn default_registry_has_all_base_tools() {
        let registry = default_registry();
        let defs = registry.definitions();
        assert_eq!(defs.len(), 4);

        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"get_current_time"));
        assert!(names.contains(&"calculate"));
        assert!(names.contains(&"send_message"));
        assert!(names.contains(&"memory_edit"));
    }

    #[test]
    fn default_registry_dispatches_calculate() {
        let registry = default_registry();
        let result = registry
            .dispatch("calculate", &serde_json::json!({"expression": "7 * 6"}))
            .expect("calculate should be registered");
        assert_eq!(result.output, "42");
    }

    #[test]
    fn send_message_returns_event() {
        let tool = SendMessage;
        let result = tool.call(&serde_json::json!({
            "content": "hello",
            "phase": "progress"
        }));
        assert_eq!(result.output, "message sent");
        assert_eq!(
            result.event,
            Some(AgentEvent::Message {
                content: "hello".to_string(),
                phase: UserMessagePhase::Progress,
            })
        );
    }

    #[test]
    fn send_message_final_phase() {
        let tool = SendMessage;
        let result = tool.call(&serde_json::json!({
            "content": "done",
            "phase": "final"
        }));
        assert_eq!(
            result.event,
            Some(AgentEvent::Message {
                content: "done".to_string(),
                phase: UserMessagePhase::Final,
            })
        );
    }

    #[test]
    fn context_write_then_read() {
        let store = Arc::new(Mutex::new(ContextStore::new()));
        let registry = registry_with_context(store);

        registry
            .dispatch(
                "context_write",
                &serde_json::json!({"name": "notes", "content": "hello world"}),
            )
            .expect("context_write registered");

        let result = registry
            .dispatch("context_read", &serde_json::json!({"name": "notes"}))
            .expect("context_read registered");
        assert_eq!(result.output, "hello world");
    }

    #[test]
    fn context_read_missing_block() {
        let store = Arc::new(Mutex::new(ContextStore::new()));
        let read = ContextRead::new(store);
        let result = read.call(&serde_json::json!({"name": "missing"}));
        assert!(result.output.contains("no context block"));
    }

    #[test]
    fn context_delete_existing_block() {
        let store = Arc::new(Mutex::new(ContextStore::new()));
        let registry = registry_with_context(store);

        registry.dispatch(
            "context_write",
            &serde_json::json!({"name": "tmp", "content": "data"}),
        );
        let result = registry
            .dispatch("context_delete", &serde_json::json!({"name": "tmp"}))
            .expect("context_delete registered");
        assert!(result.output.contains("deleted"));

        let result = registry
            .dispatch("context_read", &serde_json::json!({"name": "tmp"}))
            .unwrap();
        assert!(result.output.contains("no context block"));
    }

    #[test]
    fn context_list_shows_blocks() {
        let store = Arc::new(Mutex::new(ContextStore::new()));
        let registry = registry_with_context(store);

        registry.dispatch(
            "context_write",
            &serde_json::json!({"name": "a", "content": "first"}),
        );
        registry.dispatch(
            "context_write",
            &serde_json::json!({"name": "b", "content": "second"}),
        );

        let result = registry
            .dispatch("context_list", &serde_json::json!({}))
            .expect("context_list registered");
        assert!(result.output.contains("a:"));
        assert!(result.output.contains("b:"));
        assert!(result.output.contains("tokens"));
    }

    #[test]
    fn context_list_empty() {
        let store = Arc::new(Mutex::new(ContextStore::new()));
        let list = ContextList::new(store);
        let result = list.call(&serde_json::json!({}));
        assert_eq!(result.output, "no context blocks");
    }

    #[test]
    fn registry_with_context_has_eight_tools() {
        let store = Arc::new(Mutex::new(ContextStore::new()));
        let registry = registry_with_context(store);
        assert_eq!(registry.definitions().len(), 8);
    }

    #[test]
    fn memory_edit_returns_event() {
        let tool = MemoryEdit;
        let result = tool.call(&serde_json::json!({
            "op": "upsert",
            "block": "preferences",
            "detail": "likes dark mode"
        }));
        assert_eq!(result.output, "edit recorded");
        assert_eq!(
            result.event,
            Some(AgentEvent::MemoryEdit {
                op: "upsert".to_string(),
                block: "preferences".to_string(),
                detail: "likes dark mode".to_string(),
            })
        );
    }

    // --- File + shell tool tests ---

    fn temp_workspace() -> tempfile::TempDir {
        tempfile::tempdir().expect("create tempdir")
    }

    #[test]
    fn write_and_read_file_round_trip() {
        let ws = temp_workspace();
        let write = WriteFile::new(ws.path().to_path_buf());
        let read = ReadFile::new(ws.path().to_path_buf());

        let wr = write.call(&serde_json::json!({
            "path": "test.txt",
            "content": "hello file"
        }));
        assert!(wr.output.contains("wrote"));

        let rr = read.call(&serde_json::json!({"path": "test.txt"}));
        assert_eq!(rr.output, "hello file");
    }

    #[test]
    fn write_file_creates_parent_dirs() {
        let ws = temp_workspace();
        let write = WriteFile::new(ws.path().to_path_buf());

        let result = write.call(&serde_json::json!({
            "path": "sub/dir/file.txt",
            "content": "nested"
        }));
        assert!(result.output.contains("wrote"));
        assert!(ws.path().join("sub/dir/file.txt").exists());
    }

    #[test]
    fn path_traversal_rejected() {
        let ws = temp_workspace();
        let read = ReadFile::new(ws.path().to_path_buf());

        let result = read.call(&serde_json::json!({"path": "../../../etc/passwd"}));
        assert!(result.output.contains("error"));
    }

    #[test]
    fn shell_runs_command() {
        let ws = temp_workspace();
        let shell = Shell::new(ws.path().to_path_buf(), Duration::from_secs(5));

        let result = shell.call(&serde_json::json!({"command": "echo hello"}));
        assert!(result.output.contains("hello"));
    }

    #[test]
    fn shell_captures_stderr() {
        let ws = temp_workspace();
        let shell = Shell::new(ws.path().to_path_buf(), Duration::from_secs(5));

        let result = shell.call(&serde_json::json!({"command": "echo err >&2"}));
        assert!(result.output.contains("err"));
    }

    #[test]
    fn shell_reports_exit_code() {
        let ws = temp_workspace();
        let shell = Shell::new(ws.path().to_path_buf(), Duration::from_secs(5));

        let result = shell.call(&serde_json::json!({"command": "exit 42"}));
        assert!(result.output.contains("exit code: 42"));
    }

    #[test]
    fn shell_timeout() {
        let ws = temp_workspace();
        let shell = Shell::new(ws.path().to_path_buf(), Duration::from_millis(100));

        let result = shell.call(&serde_json::json!({"command": "sleep 10"}));
        assert!(result.output.contains("timed out"));
    }

    #[test]
    fn shell_empty_command() {
        let ws = temp_workspace();
        let shell = Shell::new(ws.path().to_path_buf(), Duration::from_secs(5));

        let result = shell.call(&serde_json::json!({"command": ""}));
        assert!(result.output.contains("empty command"));
    }

    #[test]
    fn full_registry_has_eleven_tools() {
        let store = Arc::new(Mutex::new(ContextStore::new()));
        let ws = temp_workspace();
        let registry = full_registry(store, ws.path().to_path_buf());
        assert_eq!(registry.definitions().len(), 11);

        let defs = registry.definitions();
        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        assert!(names.contains(&"read_file"));
        assert!(names.contains(&"write_file"));
        assert!(names.contains(&"shell"));
    }
}
