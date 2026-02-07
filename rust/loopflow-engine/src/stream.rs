//! Stateful parser and formatter for agent stream-json events.
//!
//! Parses streaming JSON lines from Claude, Codex, and Gemini into structured
//! events and renders them as human-readable output (tool use summaries,
//! cost/duration, text fragments). Gracefully degrades: unrecognized lines
//! pass through to the caller.

use std::io::Write;

/// A parsed stream event.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    /// Streaming text fragment (partial assistant response).
    Text(String),
    /// A tool invocation, with a short summary of what it did.
    ToolUse { name: String, summary: String },
    /// Final result event with cost and timing.
    Result {
        subtype: ResultSubtype,
        cost_usd: Option<f64>,
        duration_secs: Option<f64>,
    },
}

/// Whether the agent run succeeded or errored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResultSubtype {
    Success,
    Error,
}

/// Result of feeding a line to the parser.
#[derive(Debug, Clone, PartialEq)]
pub enum ParseResult {
    /// Parsed into one or more displayable events.
    Events(Vec<StreamEvent>),
    /// Recognized JSON we intentionally skip (system init, user messages, etc.).
    Skipped,
    /// Not recognized — caller should pass through raw.
    Passthrough,
}

/// How to display streaming output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StreamFormat {
    /// Pass through raw JSON lines (current behavior).
    #[default]
    Raw,
    /// Human-readable summary. Bool controls ANSI color.
    Human(bool),
}

/// Stateful parser for stream-json lines.
///
/// Handles Claude (`--output-format stream-json`), Codex (`--json`), and
/// Gemini (`--output-format stream-json`) formats, normalizing all three
/// into the same `StreamEvent` types.
#[derive(Debug, Default)]
pub struct StreamParser;

impl StreamParser {
    pub fn new() -> Self {
        Self
    }

    /// Feed a single line of streaming output.
    ///
    /// Returns `Events` for displayable events, `Skipped` for known-uninteresting
    /// JSON events, and `Passthrough` for anything unrecognized (non-JSON, unknown
    /// format, etc.) so the caller can print it raw.
    pub fn feed_line(&mut self, line: &str) -> ParseResult {
        let v = match serde_json::from_str::<serde_json::Value>(line) {
            Ok(v) => v,
            Err(_) => return ParseResult::Passthrough,
        };

        let event_type = match v.get("type").and_then(|t| t.as_str()) {
            Some(t) => t,
            None => return ParseResult::Passthrough,
        };

        match event_type {
            // ── Claude ──────────────────────────────────────────────
            // Claude CLI emits {"type":"assistant","message":{"content":[...]}}
            // with text and tool_use blocks nested in content array.
            "assistant" => {
                let events = parse_claude_assistant(&v);
                if events.is_empty() {
                    ParseResult::Skipped
                } else {
                    ParseResult::Events(events)
                }
            }
            "system" | "user" => ParseResult::Skipped,

            // ── Codex ───────────────────────────────────────────────
            // Codex emits item.started/completed with nested item types.
            "item.started" | "item.completed" => {
                let events = parse_codex_item(&v);
                if events.is_empty() {
                    ParseResult::Skipped
                } else {
                    ParseResult::Events(events)
                }
            }
            "turn.completed" => {
                let events = parse_codex_turn_completed(&v);
                if events.is_empty() {
                    ParseResult::Skipped
                } else {
                    ParseResult::Events(events)
                }
            }
            "turn.failed" => ParseResult::Events(vec![StreamEvent::Result {
                subtype: ResultSubtype::Error,
                cost_usd: None,
                duration_secs: None,
            }]),
            "thread.started" | "turn.started" | "item.updated" => ParseResult::Skipped,

            // ── Gemini ──────────────────────────────────────────────
            // Gemini emits message/tool_use/tool_result/result events.
            "message" => match parse_gemini_message(&v) {
                Some(event) => ParseResult::Events(vec![event]),
                None => ParseResult::Skipped,
            },
            "tool_use" => match parse_gemini_tool_use(&v) {
                Some(event) => ParseResult::Events(vec![event]),
                None => ParseResult::Skipped,
            },
            "init" | "tool_result" => ParseResult::Skipped,

            // ── Shared ──────────────────────────────────────────────
            // Both Claude and Gemini emit "result" events (different schemas).
            "result" => ParseResult::Events(vec![parse_result(&v)]),

            // Both Codex and Gemini emit "error" events.
            "error" => ParseResult::Skipped,

            _ => ParseResult::Passthrough,
        }
    }
}

// ── Claude parsing ──────────────────────────────────────────────────────────

fn parse_claude_assistant(v: &serde_json::Value) -> Vec<StreamEvent> {
    let mut events = Vec::new();
    let content = v
        .get("message")
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_array());

    let Some(blocks) = content else {
        return events;
    };

    for block in blocks {
        let block_type = block.get("type").and_then(|t| t.as_str()).unwrap_or("");
        match block_type {
            "text" => {
                if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                    if !text.is_empty() {
                        events.push(StreamEvent::Text(text.to_string()));
                    }
                }
            }
            "tool_use" => {
                let name = block
                    .get("name")
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                let input = block.get("input").cloned().unwrap_or_default();
                let summary = summarize_tool(&name, &input);
                events.push(StreamEvent::ToolUse { name, summary });
            }
            _ => {}
        }
    }
    events
}

// ── Codex parsing ───────────────────────────────────────────────────────────

fn parse_codex_item(v: &serde_json::Value) -> Vec<StreamEvent> {
    let item = match v.get("item") {
        Some(item) => item,
        None => return vec![],
    };
    let item_type = item.get("type").and_then(|t| t.as_str()).unwrap_or("");
    let event_type = v.get("type").and_then(|t| t.as_str()).unwrap_or("");

    match item_type {
        "agent_message" | "assistant_message" | "message" => {
            // Only emit text on item.completed (full message)
            if event_type == "item.completed" {
                if let Some(text) = extract_codex_text(item) {
                    return vec![StreamEvent::Text(text)];
                }
            }
            vec![]
        }
        "command_execution" => {
            // Show command when it starts
            if event_type == "item.started" {
                let cmd = item.get("command").and_then(|c| c.as_str()).unwrap_or("");
                return vec![StreamEvent::ToolUse {
                    name: "Bash".to_string(),
                    summary: truncate(cmd, 60),
                }];
            }
            vec![]
        }
        "file_change" => {
            if event_type == "item.completed" {
                if let Some(changes) = item.get("changes").and_then(|c| c.as_array()) {
                    let paths: Vec<&str> = changes
                        .iter()
                        .filter_map(|c| c.get("path").and_then(|p| p.as_str()))
                        .take(3)
                        .collect();
                    let summary = paths.join(", ");
                    return vec![StreamEvent::ToolUse {
                        name: "Edit".to_string(),
                        summary,
                    }];
                }
            }
            vec![]
        }
        "mcp_tool_call" => {
            if event_type == "item.started" {
                let tool = item
                    .get("tool")
                    .and_then(|t| t.as_str())
                    .unwrap_or("unknown");
                let server = item.get("server").and_then(|s| s.as_str()).unwrap_or("");
                let summary = if server.is_empty() {
                    String::new()
                } else {
                    server.to_string()
                };
                return vec![StreamEvent::ToolUse {
                    name: tool.to_string(),
                    summary,
                }];
            }
            vec![]
        }
        _ => vec![],
    }
}

fn parse_codex_turn_completed(v: &serde_json::Value) -> Vec<StreamEvent> {
    // Codex doesn't report cost or duration in the JSON output
    let usage = v.get("usage");
    let _ = usage; // available for future use
    let mut events = Vec::new();
    if let Some(text) = extract_codex_turn_text(v) {
        events.push(StreamEvent::Text(text));
    }
    events.push(StreamEvent::Result {
        subtype: ResultSubtype::Success,
        cost_usd: None,
        duration_secs: None,
    });
    events
}

// ── Gemini parsing ──────────────────────────────────────────────────────────

fn parse_gemini_message(v: &serde_json::Value) -> Option<StreamEvent> {
    let role = v.get("role").and_then(|r| r.as_str())?;
    if role != "assistant" {
        return None;
    }
    let content = v.get("content").and_then(|c| c.as_str())?;
    if content.is_empty() {
        return None;
    }
    Some(StreamEvent::Text(content.to_string()))
}

fn extract_codex_text(item: &serde_json::Value) -> Option<String> {
    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
        if !text.is_empty() {
            return Some(text.to_string());
        }
    }

    if let Some(message) = item.get("message").and_then(|m| m.as_str()) {
        if !message.is_empty() {
            return Some(message.to_string());
        }
    }

    if let Some(content) = item.get("content").and_then(|c| c.as_array()) {
        let mut parts = Vec::new();
        for block in content {
            if let Some(text) = block.get("text").and_then(|t| t.as_str()) {
                if !text.is_empty() {
                    parts.push(text);
                }
            }
        }
        if !parts.is_empty() {
            return Some(parts.join(""));
        }
    }

    None
}

fn extract_codex_turn_text(v: &serde_json::Value) -> Option<String> {
    let candidates = [
        v.get("output_text"),
        v.get("text"),
        v.get("final_text"),
        v.get("response").and_then(|r| r.get("text")),
        v.get("response").and_then(|r| r.get("output_text")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if let Some(text) = candidate.as_str() {
            if !text.is_empty() {
                return Some(text.to_string());
            }
        }
    }

    None
}

fn parse_gemini_tool_use(v: &serde_json::Value) -> Option<StreamEvent> {
    let tool_name = v.get("tool_name").and_then(|n| n.as_str())?.to_string();
    let params = v.get("parameters").cloned().unwrap_or_default();
    let summary = summarize_gemini_tool(&tool_name, &params);
    Some(StreamEvent::ToolUse {
        name: tool_name,
        summary,
    })
}

fn summarize_gemini_tool(name: &str, params: &serde_json::Value) -> String {
    match name {
        "run_shell_command" => params
            .get("description")
            .or_else(|| params.get("command"))
            .and_then(|v| v.as_str())
            .map(|s| truncate(s, 60))
            .unwrap_or_default(),
        "read_file" | "write_file" | "edit" => params
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "read_many_files" => "multiple files".to_string(),
        "google_web_search" => params
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "web_fetch" => params
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

// ── Shared parsing ──────────────────────────────────────────────────────────

/// Parse a "result" event — works for both Claude and Gemini.
/// Claude uses "subtype", Gemini uses "status".
fn parse_result(v: &serde_json::Value) -> StreamEvent {
    let subtype_str = v
        .get("subtype")
        .or_else(|| v.get("status"))
        .and_then(|s| s.as_str());
    let subtype = match subtype_str {
        Some("error") => ResultSubtype::Error,
        _ => ResultSubtype::Success,
    };
    // Claude: total_cost_usd (or cost_usd for older versions)
    let cost_usd = v
        .get("total_cost_usd")
        .or_else(|| v.get("cost_usd"))
        .and_then(|c| c.as_f64());
    // Claude: duration_ms; Gemini: stats.duration_ms; fallback: duration_secs
    let duration_secs = v
        .get("duration_secs")
        .and_then(|d| d.as_f64())
        .or_else(|| {
            v.get("duration_ms")
                .and_then(|d| d.as_f64())
                .map(|ms| ms / 1000.0)
        })
        .or_else(|| {
            v.get("stats")
                .and_then(|s| s.get("duration_ms"))
                .and_then(|d| d.as_f64())
                .map(|ms| ms / 1000.0)
        });
    StreamEvent::Result {
        subtype,
        cost_usd,
        duration_secs,
    }
}

// ── Tool summarizer (Claude) ────────────────────────────────────────────────

/// Extract the most useful field from a Claude tool's input for a one-line summary.
fn summarize_tool(name: &str, input: &serde_json::Value) -> String {
    match name {
        "Read" | "Edit" | "Write" => input
            .get("file_path")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "Bash" => input
            .get("description")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| {
                let cmd = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
                truncate(cmd, 60)
            }),
        "Grep" => {
            let pattern = input.get("pattern").and_then(|v| v.as_str()).unwrap_or("");
            match input.get("path").and_then(|v| v.as_str()) {
                Some(path) => format!("{pattern}  {path}"),
                None => pattern.to_string(),
            }
        }
        "Glob" => input
            .get("pattern")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "Task" => input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "WebFetch" => input
            .get("url")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        "WebSearch" => input
            .get("query")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
    }
}

// ── Rendering ───────────────────────────────────────────────────────────────

/// Render a stream event to display strings (pure, no I/O).
///
/// Returns `(stdout, stderr)` — at most one will be non-empty.
/// `use_color` controls whether ANSI escape codes are included.
pub fn render_event(event: &StreamEvent, use_color: bool) -> (String, String) {
    match event {
        StreamEvent::Text(text) => (text.clone(), String::new()),
        StreamEvent::ToolUse { name, summary } => {
            let line = if summary.is_empty() {
                format!("-> {name}")
            } else {
                format!("-> {name}  {summary}")
            };
            if use_color {
                (String::new(), format!("\x1b[90m{line}\x1b[0m"))
            } else {
                (String::new(), line)
            }
        }
        StreamEvent::Result {
            subtype,
            cost_usd,
            duration_secs,
        } => {
            let line = match subtype {
                ResultSubtype::Success => {
                    let mut parts = Vec::new();
                    if let Some(cost) = cost_usd {
                        parts.push(format!("{cost:.2} USD"));
                    }
                    if let Some(dur) = duration_secs {
                        let secs = dur.round() as u64;
                        if secs >= 60 {
                            parts.push(format!("{}m{}s", secs / 60, secs % 60));
                        } else {
                            parts.push(format!("{secs}s"));
                        }
                    }
                    if parts.is_empty() {
                        "ok".to_string()
                    } else {
                        format!("ok ({})", parts.join(", "))
                    }
                }
                ResultSubtype::Error => "failed".to_string(),
            };
            if use_color {
                let color = match subtype {
                    ResultSubtype::Success => "\x1b[32m",
                    ResultSubtype::Error => "\x1b[31m",
                };
                (String::new(), format!("{color}{line}\x1b[0m"))
            } else {
                (String::new(), line)
            }
        }
    }
}

/// Write a stream event to stdout/stderr. Thin wrapper over `render_event`.
pub fn format_event(event: &StreamEvent, use_color: bool) {
    let (stdout_str, stderr_str) = render_event(event, use_color);
    if !stdout_str.is_empty() {
        print!("{stdout_str}");
        let _ = std::io::stdout().flush();
    }
    if !stderr_str.is_empty() {
        eprintln!("{stderr_str}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Claude tests ────────────────────────────────────────────

    #[test]
    fn claude_assistant_text() {
        let mut parser = StreamParser::new();
        let line =
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello world"}]}}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Events(vec![StreamEvent::Text("hello world".to_string())])
        );
    }

    #[test]
    fn claude_assistant_tool_use() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"/src/main.rs"}}]}}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Events(vec![StreamEvent::ToolUse {
                name: "Read".to_string(),
                summary: "/src/main.rs".to_string(),
            }])
        );
    }

    #[test]
    fn claude_assistant_mixed_content() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"reading file"},{"type":"tool_use","name":"Bash","input":{"command":"cargo test","description":"Run tests"}}]}}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Events(vec![
                StreamEvent::Text("reading file".to_string()),
                StreamEvent::ToolUse {
                    name: "Bash".to_string(),
                    summary: "Run tests".to_string(),
                },
            ])
        );
    }

    #[test]
    fn claude_system_skipped() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"system","subtype":"init","session_id":"abc"}"#;
        assert_eq!(parser.feed_line(line), ParseResult::Skipped);
    }

    #[test]
    fn claude_user_skipped() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"user","message":{"role":"user","content":[]}}"#;
        assert_eq!(parser.feed_line(line), ParseResult::Skipped);
    }

    #[test]
    fn claude_result_success() {
        let mut parser = StreamParser::new();
        let line =
            r#"{"type":"result","subtype":"success","total_cost_usd":0.05,"duration_ms":45200}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Events(vec![StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: Some(0.05),
                duration_secs: Some(45.2),
            }])
        );
    }

    #[test]
    fn claude_result_legacy_field_names() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"result","subtype":"success","cost_usd":0.03,"duration_secs":30.0}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Events(vec![StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: Some(0.03),
                duration_secs: Some(30.0),
            }])
        );
    }

    #[test]
    fn claude_result_error() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"result","subtype":"error"}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Events(vec![StreamEvent::Result {
                subtype: ResultSubtype::Error,
                cost_usd: None,
                duration_secs: None,
            }])
        );
    }

    // ── Codex tests ─────────────────────────────────────────────

    #[test]
    fn codex_command_started() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"item.started","item":{"id":"i1","type":"command_execution","command":"bash -lc 'cargo test'","status":"in_progress"}}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Events(vec![StreamEvent::ToolUse {
                name: "Bash".to_string(),
                summary: "bash -lc 'cargo test'".to_string(),
            }])
        );
    }

    #[test]
    fn codex_agent_message() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"item.completed","item":{"id":"i1","type":"agent_message","text":"Done fixing the bug."}}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Events(vec![StreamEvent::Text("Done fixing the bug.".to_string())])
        );
    }

    #[test]
    fn codex_file_change() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"item.completed","item":{"id":"i1","type":"file_change","changes":[{"path":"src/main.rs","kind":"update"}],"status":"completed"}}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Events(vec![StreamEvent::ToolUse {
                name: "Edit".to_string(),
                summary: "src/main.rs".to_string(),
            }])
        );
    }

    #[test]
    fn codex_turn_completed() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"turn.completed","usage":{"input_tokens":100,"output_tokens":50}}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Events(vec![StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: None,
                duration_secs: None,
            }])
        );
    }

    #[test]
    fn codex_turn_failed() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"turn.failed","error":{"message":"boom"}}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Events(vec![StreamEvent::Result {
                subtype: ResultSubtype::Error,
                cost_usd: None,
                duration_secs: None,
            }])
        );
    }

    #[test]
    fn codex_thread_started_skipped() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"thread.started","thread_id":"abc"}"#;
        assert_eq!(parser.feed_line(line), ParseResult::Skipped);
    }

    // ── Gemini tests ────────────────────────────────────────────

    #[test]
    fn gemini_assistant_message() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"message","timestamp":"2025-01-01T00:00:00Z","role":"assistant","content":"hello","delta":true}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Events(vec![StreamEvent::Text("hello".to_string())])
        );
    }

    #[test]
    fn gemini_user_message_skipped() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"message","timestamp":"2025-01-01T00:00:00Z","role":"user","content":"do something"}"#;
        assert_eq!(parser.feed_line(line), ParseResult::Skipped);
    }

    #[test]
    fn gemini_tool_use() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"tool_use","timestamp":"2025-01-01T00:00:00Z","tool_name":"run_shell_command","tool_id":"c1","parameters":{"command":"ls","description":"List files"}}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Events(vec![StreamEvent::ToolUse {
                name: "run_shell_command".to_string(),
                summary: "List files".to_string(),
            }])
        );
    }

    #[test]
    fn gemini_tool_use_read_file() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"tool_use","timestamp":"2025-01-01T00:00:00Z","tool_name":"read_file","tool_id":"c1","parameters":{"file_path":"/src/main.rs"}}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Events(vec![StreamEvent::ToolUse {
                name: "read_file".to_string(),
                summary: "/src/main.rs".to_string(),
            }])
        );
    }

    #[test]
    fn gemini_result_with_stats() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"result","timestamp":"2025-01-01T00:00:00Z","status":"success","stats":{"duration_ms":5000,"total_tokens":250}}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Events(vec![StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: None,
                duration_secs: Some(5.0),
            }])
        );
    }

    #[test]
    fn gemini_init_skipped() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"init","timestamp":"2025-01-01T00:00:00Z","session_id":"s1","model":"gemini-2.0-flash"}"#;
        assert_eq!(parser.feed_line(line), ParseResult::Skipped);
    }

    // ── General tests ───────────────────────────────────────────

    #[test]
    fn non_json_passes_through() {
        let mut parser = StreamParser::new();
        assert_eq!(
            parser.feed_line("not json at all"),
            ParseResult::Passthrough
        );
        assert_eq!(parser.feed_line("{"), ParseResult::Passthrough);
        assert_eq!(parser.feed_line(""), ParseResult::Passthrough);
    }

    #[test]
    fn json_without_type_passes_through() {
        let mut parser = StreamParser::new();
        assert_eq!(
            parser.feed_line(r#"{"data":"something"}"#),
            ParseResult::Passthrough
        );
    }

    #[test]
    fn unknown_json_type_passes_through() {
        let mut parser = StreamParser::new();
        assert_eq!(
            parser.feed_line(r#"{"type":"some_new_thing","data":123}"#),
            ParseResult::Passthrough
        );
    }

    // ── Tool summarizer tests ───────────────────────────────────

    #[test]
    fn summarize_tool_bash_uses_description() {
        let input =
            serde_json::json!({"command": "cargo test --all", "description": "Run all tests"});
        assert_eq!(summarize_tool("Bash", &input), "Run all tests");
    }

    #[test]
    fn summarize_tool_bash_falls_back_to_command() {
        let input = serde_json::json!({"command": "cargo test --all"});
        assert_eq!(summarize_tool("Bash", &input), "cargo test --all");
    }

    #[test]
    fn summarize_tool_bash_truncates_long_command() {
        let long_cmd = "x".repeat(100);
        let input = serde_json::json!({"command": long_cmd});
        let summary = summarize_tool("Bash", &input);
        assert!(summary.len() <= 63); // 60 + "..."
        assert!(summary.ends_with("..."));
    }

    #[test]
    fn summarize_tool_grep_with_path() {
        let input = serde_json::json!({"pattern": "fn main", "path": "src/"});
        assert_eq!(summarize_tool("Grep", &input), "fn main  src/");
    }

    // ── Render tests ────────────────────────────────────────────

    #[test]
    fn render_text_event() {
        let (stdout, stderr) = render_event(&StreamEvent::Text("hello".to_string()), false);
        assert_eq!(stdout, "hello");
        assert!(stderr.is_empty());
    }

    #[test]
    fn render_tool_use_no_color() {
        let (stdout, stderr) = render_event(
            &StreamEvent::ToolUse {
                name: "Read".to_string(),
                summary: "/src/main.rs".to_string(),
            },
            false,
        );
        assert!(stdout.is_empty());
        assert_eq!(stderr, "-> Read  /src/main.rs");
    }

    #[test]
    fn render_tool_use_with_color() {
        let (_, stderr) = render_event(
            &StreamEvent::ToolUse {
                name: "Read".to_string(),
                summary: "/src/main.rs".to_string(),
            },
            true,
        );
        assert!(stderr.contains("\x1b[90m"));
        assert!(stderr.contains("-> Read  /src/main.rs"));
    }

    #[test]
    fn render_result_success_with_cost() {
        let (_, stderr) = render_event(
            &StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: Some(0.05),
                duration_secs: Some(45.0),
            },
            false,
        );
        assert_eq!(stderr, "ok (0.05 USD, 45s)");
    }

    #[test]
    fn render_result_error() {
        let (_, stderr) = render_event(
            &StreamEvent::Result {
                subtype: ResultSubtype::Error,
                cost_usd: None,
                duration_secs: None,
            },
            false,
        );
        assert_eq!(stderr, "failed");
    }

    #[test]
    fn render_result_error_with_color() {
        let (_, stderr) = render_event(
            &StreamEvent::Result {
                subtype: ResultSubtype::Error,
                cost_usd: None,
                duration_secs: None,
            },
            true,
        );
        assert!(stderr.contains("\x1b[31m"));
        assert!(stderr.contains("failed"));
    }
}
