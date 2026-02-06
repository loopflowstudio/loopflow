//! Stateful parser and formatter for agent stream-json events.
//!
//! Parses streaming JSON lines into structured events and renders them as
//! human-readable output (tool use summaries, cost/duration, text fragments).
//! Gracefully degrades: unrecognized lines pass through to the caller.

use std::collections::HashMap;
use std::io::Write;

/// A parsed stream event.
#[derive(Debug, Clone, PartialEq)]
pub enum StreamEvent {
    /// Streaming text fragment (partial assistant response).
    Text(String),
    /// A tool invocation completed, with a short summary of what it did.
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
    /// Parsed into a displayable event.
    Event(StreamEvent),
    /// Recognized JSON we intentionally skip (message_start, ping, etc.).
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

/// Tracks in-flight content blocks to accumulate tool input JSON.
#[derive(Debug, Default)]
struct BlockState {
    /// tool name for tool_use blocks
    tool_name: Option<String>,
    /// accumulated input_json fragments
    input_json: String,
}

/// Known event types we deliberately skip (no display needed).
const SKIP_TYPES: &[&str] = &["message_start", "message_delta", "message_stop", "ping"];

/// Stateful parser for stream-json lines.
#[derive(Debug, Default)]
pub struct StreamParser {
    blocks: HashMap<u64, BlockState>,
}

impl StreamParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Feed a single line of streaming output.
    ///
    /// Returns `Event` for displayable events, `Skipped` for known-uninteresting
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
            "content_block_start" => {
                if let Some(event) = self.handle_block_start(&v) {
                    return ParseResult::Event(event);
                }
                ParseResult::Skipped
            }
            "content_block_delta" => match self.handle_block_delta(&v) {
                Some(event) => ParseResult::Event(event),
                None => ParseResult::Skipped,
            },
            "content_block_stop" => match self.handle_block_stop(&v) {
                Some(event) => ParseResult::Event(event),
                None => ParseResult::Skipped,
            },
            "result" => ParseResult::Event(self.handle_result(&v)),
            t if SKIP_TYPES.contains(&t) => ParseResult::Skipped,
            _ => ParseResult::Passthrough,
        }
    }

    fn handle_block_start(&mut self, v: &serde_json::Value) -> Option<StreamEvent> {
        let index = v.get("index")?.as_u64()?;
        let block = v.get("content_block")?;
        let block_type = block.get("type")?.as_str()?;
        if block_type == "tool_use" {
            let name = block.get("name")?.as_str()?.to_string();
            self.blocks.insert(
                index,
                BlockState {
                    tool_name: Some(name),
                    input_json: String::new(),
                },
            );
        }
        None
    }

    fn handle_block_delta(&mut self, v: &serde_json::Value) -> Option<StreamEvent> {
        let index = v.get("index")?.as_u64()?;
        let delta = v.get("delta")?;
        let delta_type = delta.get("type")?.as_str()?;

        match delta_type {
            "text_delta" => {
                let text = delta.get("text")?.as_str()?;
                Some(StreamEvent::Text(text.to_string()))
            }
            "input_json_delta" => {
                let partial = delta.get("partial_json")?.as_str()?;
                if let Some(block) = self.blocks.get_mut(&index) {
                    block.input_json.push_str(partial);
                }
                None
            }
            _ => None,
        }
    }

    fn handle_block_stop(&mut self, v: &serde_json::Value) -> Option<StreamEvent> {
        let index = v.get("index")?.as_u64()?;
        if let Some(block) = self.blocks.remove(&index) {
            if let Some(tool_name) = block.tool_name {
                let input: serde_json::Value =
                    serde_json::from_str(&block.input_json).unwrap_or_default();
                let summary = summarize_tool(&tool_name, &input);
                return Some(StreamEvent::ToolUse {
                    name: tool_name,
                    summary,
                });
            }
        }
        None
    }

    fn handle_result(&self, v: &serde_json::Value) -> StreamEvent {
        let subtype = match v.get("subtype").and_then(|s| s.as_str()) {
            Some("error") => ResultSubtype::Error,
            _ => ResultSubtype::Success,
        };
        let cost_usd = v.get("cost_usd").and_then(|c| c.as_f64()).or_else(|| {
            v.get("result")
                .and_then(|r| r.get("cost_usd"))
                .and_then(|c| c.as_f64())
        });
        let duration_secs = v.get("duration_secs").and_then(|d| d.as_f64()).or_else(|| {
            v.get("result")
                .and_then(|r| r.get("duration_secs"))
                .and_then(|d| d.as_f64())
        });
        StreamEvent::Result {
            subtype,
            cost_usd,
            duration_secs,
        }
    }
}

/// Extract the most useful field from a tool's input for a one-line summary.
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

    #[test]
    fn text_delta_emits_text_event() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hello"}}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Event(StreamEvent::Text("hello".to_string()))
        );
    }

    #[test]
    fn tool_use_emits_on_stop() {
        let mut parser = StreamParser::new();

        // Start tool_use block
        let start = r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"t1","name":"Read","input":{}}}"#;
        assert_eq!(parser.feed_line(start), ParseResult::Skipped);

        // Accumulate input JSON
        let delta1 = r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"file_path\""}}"#;
        assert_eq!(parser.feed_line(delta1), ParseResult::Skipped);

        let delta2 = r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":":\"/src/main.rs\"}"}}"#;
        assert_eq!(parser.feed_line(delta2), ParseResult::Skipped);

        // Stop emits the event
        let stop = r#"{"type":"content_block_stop","index":1}"#;
        assert_eq!(
            parser.feed_line(stop),
            ParseResult::Event(StreamEvent::ToolUse {
                name: "Read".to_string(),
                summary: "/src/main.rs".to_string(),
            })
        );
    }

    #[test]
    fn result_success_event() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"result","subtype":"success","cost_usd":0.05,"duration_secs":45.2}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Event(StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: Some(0.05),
                duration_secs: Some(45.2),
            })
        );
    }

    #[test]
    fn result_error_event() {
        let mut parser = StreamParser::new();
        let line = r#"{"type":"result","subtype":"error"}"#;
        assert_eq!(
            parser.feed_line(line),
            ParseResult::Event(StreamEvent::Result {
                subtype: ResultSubtype::Error,
                cost_usd: None,
                duration_secs: None,
            })
        );
    }

    #[test]
    fn known_skip_types() {
        let mut parser = StreamParser::new();
        assert_eq!(
            parser.feed_line(r#"{"type":"message_start","message":{"id":"msg_1"}}"#),
            ParseResult::Skipped
        );
        assert_eq!(parser.feed_line(r#"{"type":"ping"}"#), ParseResult::Skipped);
    }

    #[test]
    fn unknown_json_type_passes_through() {
        let mut parser = StreamParser::new();
        assert_eq!(
            parser.feed_line(r#"{"type":"some_new_thing","data":123}"#),
            ParseResult::Passthrough
        );
    }

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
    fn summarize_tool_bash_uses_description() {
        let input: serde_json::Value = serde_json::json!({
            "command": "cargo test --all",
            "description": "Run all tests"
        });
        assert_eq!(summarize_tool("Bash", &input), "Run all tests");
    }

    #[test]
    fn summarize_tool_bash_falls_back_to_command() {
        let input: serde_json::Value = serde_json::json!({
            "command": "cargo test --all"
        });
        assert_eq!(summarize_tool("Bash", &input), "cargo test --all");
    }

    #[test]
    fn summarize_tool_bash_truncates_long_command() {
        let long_cmd = "x".repeat(100);
        let input: serde_json::Value = serde_json::json!({
            "command": long_cmd
        });
        let summary = summarize_tool("Bash", &input);
        assert!(summary.len() <= 63); // 60 + "..."
        assert!(summary.ends_with("..."));
    }

    #[test]
    fn summarize_tool_grep_with_path() {
        let input: serde_json::Value = serde_json::json!({
            "pattern": "fn main",
            "path": "src/"
        });
        assert_eq!(summarize_tool("Grep", &input), "fn main  src/");
    }

    #[test]
    fn summarize_tool_grep_without_path() {
        let input: serde_json::Value = serde_json::json!({
            "pattern": "TODO"
        });
        assert_eq!(summarize_tool("Grep", &input), "TODO");
    }

    #[test]
    fn summarize_tool_unknown_returns_empty() {
        let input: serde_json::Value = serde_json::json!({"foo": "bar"});
        assert_eq!(summarize_tool("SomeNewTool", &input), "");
    }

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
        assert!(stderr.contains("\x1b[0m"));
    }

    #[test]
    fn render_tool_use_empty_summary() {
        let (_, stderr) = render_event(
            &StreamEvent::ToolUse {
                name: "SomeNewTool".to_string(),
                summary: String::new(),
            },
            false,
        );
        assert_eq!(stderr, "-> SomeNewTool");
    }

    #[test]
    fn render_result_success_with_cost() {
        let (stdout, stderr) = render_event(
            &StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: Some(0.05),
                duration_secs: Some(45.0),
            },
            false,
        );
        assert!(stdout.is_empty());
        assert_eq!(stderr, "ok (0.05 USD, 45s)");
    }

    #[test]
    fn render_result_success_long_duration() {
        let (_, stderr) = render_event(
            &StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: None,
                duration_secs: Some(125.0),
            },
            false,
        );
        assert_eq!(stderr, "ok (2m5s)");
    }

    #[test]
    fn render_result_success_no_details() {
        let (_, stderr) = render_event(
            &StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: None,
                duration_secs: None,
            },
            false,
        );
        assert_eq!(stderr, "ok");
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

    #[test]
    fn render_result_success_with_color() {
        let (_, stderr) = render_event(
            &StreamEvent::Result {
                subtype: ResultSubtype::Success,
                cost_usd: Some(0.12),
                duration_secs: Some(30.0),
            },
            true,
        );
        assert!(stderr.contains("\x1b[32m"));
        assert!(stderr.contains("ok"));
    }
}
