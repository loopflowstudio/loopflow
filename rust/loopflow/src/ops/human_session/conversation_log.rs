//! A native conversation as plain text another provider can read: what people
//! asked, what the agent said, one line per tool call, and the tail of each
//! failed call's output. No summarization: Claude and Codex are read from
//! their transcript files, OpenCode from `opencode export`.

use std::fs;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;

use super::provider_conversation::{conversation_home, is_user_input, transcript};
use crate::provider_auth::Provider;
use crate::session::LfSession;
use crate::store::sqlite::SqliteStore;

const TOOL_INPUT_CHARS: usize = 200;
const ERROR_TAIL_CHARS: usize = 400;
/// The first turn quotes the log's opening request and its latest exchanges,
/// staying well under the terminal argument cap.
const EXCERPT_HEAD_CHARS: usize = 6_000;
const EXCERPT_TAIL_CHARS: usize = 14_000;

#[derive(Debug)]
enum Entry {
    User(String),
    Assistant(String),
    Tool {
        call: String,
        line: String,
        error: Option<String>,
    },
}

/// Log the Session's current native conversation beside its capture and
/// return the first turn that hands it to another provider.
pub(crate) fn continuation(store: &SqliteStore, session: &LfSession) -> Result<String> {
    let id = &session.id;
    let from = session.provider.as_deref().unwrap_or_default();
    let reference = store
        .input_provider_session(session.input_key()?)?
        .ok_or_else(|| anyhow!("Session {id} has no native conversation to continue"))?;
    let thread = reference.agent_session.as_str();
    let entries = match from {
        "opencode" => opencode_entries(&opencode_export(&session.cwd, thread)?),
        "claude" | "codex" => {
            let provider = if from == "codex" {
                Provider::Codex
            } else {
                Provider::Claude
            };
            let source = conversation_home(store, provider, &reference)?
                .and_then(|home| transcript(provider, &home, thread))
                .ok_or_else(|| {
                    anyhow!("The {from} transcript of Session {id} ({thread}) was not found")
                })?;
            transcript_entries(provider, &source)?
        }
        _ => bail!("Session {id} has no {from} history another provider can read"),
    };
    let log = render(&entries);
    let path = super::local_capture_dir(session.input_key()?)
        .ok_or_else(|| anyhow!("Session {id} has an invalid capture reference"))?
        .join("conversation.md");
    fs::create_dir_all(path.parent().expect("a capture file has a directory"))?;
    fs::write(&path, &log).context("write conversation log")?;
    Ok(format!(
        "This Session continues a conversation that began in {from}. Its full log is at {}; \
         the excerpt below is its beginning and end. Pick up where it left off.\n\n{}",
        path.display(),
        excerpt(&log)
    ))
}

/// The log's beginning and end, within what one terminal turn can carry.
fn excerpt(log: &str) -> String {
    let count = log.chars().count();
    if count <= EXCERPT_HEAD_CHARS + EXCERPT_TAIL_CHARS {
        return log.trim().to_owned();
    }
    let omitted = count - EXCERPT_HEAD_CHARS - EXCERPT_TAIL_CHARS;
    format!(
        "{}\n\n[… {omitted} characters omitted; read the full log …]\n\n{}",
        head(log, EXCERPT_HEAD_CHARS).trim_end_matches('…').trim(),
        tail(log, EXCERPT_TAIL_CHARS).trim_start_matches('…').trim()
    )
}

/// Claude or Codex JSONL transcript entries.
fn transcript_entries(provider: Provider, transcript: &Path) -> Result<Vec<Entry>> {
    let file = fs::File::open(transcript)
        .with_context(|| format!("open transcript {}", transcript.display()))?;
    let mut entries = Vec::new();
    for line in BufReader::new(file).lines() {
        let Ok(row) = serde_json::from_str::<Value>(&line?) else {
            continue;
        };
        match provider {
            Provider::Codex => codex_row(&row, &mut entries),
            _ => claude_row(&row, &mut entries),
        }
    }
    Ok(entries)
}

/// OpenCode keeps history in its own store; its CLI exports one session as JSON.
fn opencode_export(cwd: &Path, thread: &str) -> Result<Value> {
    let output = Command::new("opencode")
        .args(["export", thread])
        .current_dir(cwd)
        .output()
        .context("run opencode export")?;
    if !output.status.success() {
        bail!(
            "opencode export {thread} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice(&output.stdout).context("parse opencode export")
}

/// Entries of an `opencode export`: messages, each with text and tool parts.
fn opencode_entries(export: &Value) -> Vec<Entry> {
    let mut entries = Vec::new();
    for message in export["messages"].as_array().into_iter().flatten() {
        let role = message["info"]["role"].as_str();
        for part in message["parts"].as_array().into_iter().flatten() {
            if part["synthetic"].as_bool() == Some(true) {
                continue;
            }
            let text = part["text"].as_str().unwrap_or_default();
            match (role, part["type"].as_str()) {
                (Some("user"), Some("text")) => {
                    // Loopflow's context rides in the user turn after the person's words.
                    let request = lf_message(text).unwrap_or_else(|| {
                        text.split("<lf:").next().unwrap_or_default().to_owned()
                    });
                    if !request.trim().is_empty() {
                        entries.push(Entry::User(request));
                    }
                }
                (Some("assistant"), Some("text")) if !text.trim().is_empty() => {
                    entries.push(Entry::Assistant(text.to_owned()));
                }
                (Some("assistant"), Some("tool")) => {
                    let state = &part["state"];
                    let mut entry = tool(
                        part["callID"].as_str(),
                        part["tool"].as_str(),
                        &state["input"].to_string(),
                    );
                    if state["status"] == "error" {
                        if let Entry::Tool { error, .. } = &mut entry {
                            *error = Some(failure(&text_of(&state["error"])));
                        }
                    }
                    entries.push(entry);
                }
                _ => {}
            }
        }
    }
    entries
}

/// Plain-text log: user and assistant text, one line per tool call.
fn render(entries: &[Entry]) -> String {
    let mut markdown = String::new();
    for entry in entries {
        match entry {
            Entry::User(text) => markdown.push_str(&format!("## user\n\n{}\n\n", text.trim())),
            Entry::Assistant(text) => {
                markdown.push_str(&format!("## assistant\n\n{}\n\n", text.trim()))
            }
            Entry::Tool { line, error, .. } => {
                markdown.push_str(&format!("- tool: {line}\n"));
                if let Some(error) = error {
                    markdown.push_str(&format!("  failed: {error}\n"));
                }
                markdown.push('\n');
            }
        }
    }
    markdown
}

fn claude_row(row: &Value, entries: &mut Vec<Entry>) {
    if row["isSidechain"].as_bool() == Some(true) {
        return;
    }
    let content = &row["message"]["content"];
    match row["type"].as_str() {
        Some("user") => {
            if is_user_input(row) {
                entries.push(Entry::User(text_of(content)));
                return;
            }
            if row["isMeta"].as_bool() != Some(true) {
                if let Some(message) = lf_message(&text_of(content)) {
                    entries.push(Entry::User(message));
                }
            }
            for block in content.as_array().into_iter().flatten() {
                if block["type"] == "tool_result" && block["is_error"].as_bool() == Some(true) {
                    fail(
                        entries,
                        block["tool_use_id"].as_str().unwrap_or_default(),
                        &text_of(&block["content"]),
                    );
                }
            }
        }
        Some("assistant") => {
            for block in content.as_array().into_iter().flatten() {
                match block["type"].as_str() {
                    Some("text") => {
                        let text = block["text"].as_str().unwrap_or_default();
                        if !text.trim().is_empty() {
                            entries.push(Entry::Assistant(text.to_owned()));
                        }
                    }
                    Some("tool_use") => entries.push(tool(
                        block["id"].as_str(),
                        block["name"].as_str(),
                        &block["input"].to_string(),
                    )),
                    _ => {}
                }
            }
        }
        _ => {}
    }
}

fn codex_row(row: &Value, entries: &mut Vec<Entry>) {
    if row["type"] != "response_item" {
        return;
    }
    let item = &row["payload"];
    match item["type"].as_str() {
        Some("message") => {
            let text = text_of(&item["content"]);
            match item["role"].as_str() {
                Some("user") => {
                    if let Some(request) = codex_request(&text) {
                        entries.push(Entry::User(request));
                    }
                }
                Some("assistant") if !text.trim().is_empty() => {
                    entries.push(Entry::Assistant(text));
                }
                _ => {}
            }
        }
        Some("function_call") => entries.push(tool(
            item["call_id"].as_str(),
            item["name"].as_str(),
            item["arguments"].as_str().unwrap_or_default(),
        )),
        Some("custom_tool_call") => entries.push(tool(
            item["call_id"].as_str(),
            item["name"].as_str(),
            item["input"].as_str().unwrap_or_default(),
        )),
        Some("function_call_output" | "custom_tool_call_output") => {
            let output = text_of(&item["output"]);
            if exited_nonzero(&output) {
                fail(
                    entries,
                    item["call_id"].as_str().unwrap_or_default(),
                    &output,
                );
            }
        }
        _ => {}
    }
}

/// A Codex user message, minus the instructions and context Codex and Loopflow inject.
fn codex_request(text: &str) -> Option<String> {
    if let Some(message) = lf_message(text) {
        return Some(message);
    }
    let trimmed = text.trim_start();
    let injected = trimmed.is_empty()
        || trimmed.starts_with("# AGENTS.md instructions")
        || trimmed.starts_with("<environment_context>")
        || trimmed.starts_with("<user_instructions>")
        || trimmed.contains("<lf:");
    (!injected).then(|| text.to_owned())
}

/// A Loopflow launch turn: the skill it invoked and the person's words.
fn lf_message(text: &str) -> Option<String> {
    let skill = text.find("<lf:skill:").and_then(|open| {
        let name_start = open + "<lf:skill:".len();
        let name_end = text[name_start..].find('>')? + name_start;
        let name = &text[name_start..name_end];
        let close = format!("</lf:skill:{name}>");
        let body_end = text[name_end..].find(&close)? + name_end;
        Some(format!(
            "Skill {name}:\n\n{}",
            text[name_end + 1..body_end].trim()
        ))
    });
    let message = text.find("<lf:message>").and_then(|open| {
        let start = open + "<lf:message>".len();
        let end = text[start..].find("</lf:message>")? + start;
        Some(text[start..end].trim().to_owned())
    });
    let turn = [skill, message].into_iter().flatten().collect::<Vec<_>>();
    (!turn.is_empty()).then(|| turn.join("\n\n"))
}

fn tool(call: Option<&str>, name: Option<&str>, input: &str) -> Entry {
    let input = input.split_whitespace().collect::<Vec<_>>().join(" ");
    Entry::Tool {
        call: call.unwrap_or_default().to_owned(),
        line: format!(
            "{} {}",
            name.unwrap_or("unknown"),
            head(&input, TOOL_INPUT_CHARS)
        ),
        error: None,
    }
}

fn failure(output: &str) -> String {
    tail(output.trim(), ERROR_TAIL_CHARS).replace('\n', "\n    ")
}

fn fail(entries: &mut [Entry], call_id: &str, output: &str) {
    let tail = failure(output);
    if let Some(Entry::Tool { error, .. }) = entries
        .iter_mut()
        .rev()
        .find(|entry| matches!(entry, Entry::Tool { call, .. } if call == call_id))
    {
        *error = Some(tail);
    }
}

/// Whether a Codex tool output reports a nonzero exit, in any of the shapes
/// Codex has written: `"exit_code":N`, `Exit code: N`, `exited with code N`.
fn exited_nonzero(output: &str) -> bool {
    ["\"exit_code\":", "Exit code: ", "exited with code "]
        .iter()
        .any(|marker| {
            output.match_indices(marker).any(|(at, _)| {
                let rest = output[at + marker.len()..].trim_start();
                let digits = rest
                    .char_indices()
                    .take_while(|(index, c)| c.is_ascii_digit() || (*index == 0 && *c == '-'))
                    .map(|(_, c)| c)
                    .collect::<String>();
                digits.parse::<i64>().is_ok_and(|code| code != 0)
            })
        })
}

/// Text of a string, or of the text blocks in a content array.
fn text_of(content: &Value) -> String {
    if let Some(text) = content.as_str() {
        return text.to_owned();
    }
    content
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|block| block["text"].as_str())
        .collect::<Vec<_>>()
        .join("\n")
}

fn head(text: &str, chars: usize) -> String {
    match text.char_indices().nth(chars) {
        Some((at, _)) => format!("{}…", &text[..at]),
        None => text.to_owned(),
    }
}

fn tail(text: &str, chars: usize) -> String {
    let count = text.chars().count();
    if count <= chars {
        return text.to_owned();
    }
    let (at, _) = text
        .char_indices()
        .nth(count - chars)
        .expect("tail start lies within the text");
    format!("…{}", &text[at..])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(lines: &[Value]) -> tempfile::NamedTempFile {
        let file = tempfile::NamedTempFile::new().unwrap();
        let body = lines
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(file.path(), body).unwrap();
        file
    }

    #[test]
    fn claude_log_keeps_conversation_and_failures_only() {
        let long_error = format!("{}permission denied", "x".repeat(1000));
        let transcript = write(&[
            serde_json::json!({"type":"user","userType":"external","message":{"content":"<lf:skill:fix>\nFix things.\n</lf:skill:fix>\n\n<lf:message>\nFix the parser\n</lf:message>"}}),
            serde_json::json!({"type":"assistant","message":{"content":[
                {"type":"thinking","thinking":"secret plan"},
                {"type":"text","text":"Reading the parser."},
                {"type":"tool_use","id":"t1","name":"Read","input":{"file_path":"src/parser.rs"}},
                {"type":"tool_use","id":"t2","name":"Bash","input":{"command":"cargo test"}}]}}),
            serde_json::json!({"type":"user","userType":"external","toolUseResult":"ok","message":{"content":[
                {"type":"tool_result","tool_use_id":"t1","content":"fn parse() {}"},
                {"type":"tool_result","tool_use_id":"t2","is_error":true,"content":long_error}]}}),
            serde_json::json!({"type":"user","userType":"external","isSidechain":true,"message":{"content":"sidechain chatter"}}),
            serde_json::json!({"type":"user","userType":"external","isMeta":true,"message":{"content":"meta line"}}),
            serde_json::json!({"type":"user","userType":"external","message":{"content":[{"type":"text","text":"Try again with sudo"}]}}),
        ]);
        let text = &render(&transcript_entries(Provider::Claude, transcript.path()).unwrap());
        assert!(text.contains("## user\n\nSkill fix:\n\nFix things.\n\nFix the parser"));
        assert!(text.contains("## assistant\n\nReading the parser."));
        assert!(text.contains(r#"- tool: Read {"file_path":"src/parser.rs"}"#));
        assert!(text.contains("- tool: Bash"));
        assert!(text.contains("permission denied"));
        assert!(!text.contains(&"x".repeat(500)));
        for absent in ["fn parse", "secret plan", "sidechain", "meta line"] {
            assert!(!text.contains(absent), "{absent} leaked into {text}");
        }
        assert!(text.trim_end().ends_with("Try again with sudo"));
    }

    #[test]
    fn codex_log_keeps_conversation_and_failures_only() {
        let transcript = write(&[
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"developer","content":[{"type":"input_text","text":"<lf:loopflow>rules</lf:loopflow>"}]}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"# AGENTS.md instructions for /repo"}]}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"<lf:message>\nShip the fix\n</lf:message>"}]}}),
            serde_json::json!({"type":"response_item","payload":{"type":"reasoning","summary":[{"text":"hidden reasoning"}]}}),
            serde_json::json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Running tests."}]}}),
            serde_json::json!({"type":"response_item","payload":{"type":"function_call","name":"shell","arguments":"{\"command\":[\"cargo\",\"test\"]}","call_id":"c1"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"c1","output":"{\"output\":\"test result: FAILED\",\"metadata\":{\"exit_code\":101}}"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"custom_tool_call","name":"apply_patch","input":"*** Begin Patch","call_id":"c2"}}),
            serde_json::json!({"type":"response_item","payload":{"type":"custom_tool_call_output","call_id":"c2","output":[{"type":"input_text","text":"Exit code: 0\nOutput:\npatched cleanly"}]}}),
            serde_json::json!({"type":"event_msg","payload":{"type":"user_message","message":"duplicate"}}),
        ]);
        let text = &render(&transcript_entries(Provider::Codex, transcript.path()).unwrap());
        assert!(text.contains("## user\n\nShip the fix"));
        assert!(text.contains("## assistant\n\nRunning tests."));
        assert!(text.contains("- tool: shell"));
        assert!(text.contains("test result: FAILED"));
        assert!(text.contains("- tool: apply_patch *** Begin Patch"));
        for absent in [
            "rules",
            "AGENTS.md",
            "hidden reasoning",
            "patched cleanly",
            "duplicate",
        ] {
            assert!(!text.contains(absent), "{absent} leaked into {text}");
        }
    }

    #[test]
    fn opencode_log_keeps_conversation_and_failures_only() {
        let export = serde_json::json!({"info":{"id":"ses_1"},"messages":[
            {"info":{"role":"user"},"parts":[
                {"type":"text","text":"Fix the build\n\n<lf:loopflow>\nrules\n</lf:loopflow>"},
                {"type":"text","synthetic":true,"text":"injected file"}]},
            {"info":{"role":"assistant"},"parts":[
                {"type":"step-start"},
                {"type":"reasoning","text":"hidden reasoning"},
                {"type":"text","text":"Checking status."},
                {"type":"tool","callID":"c1","tool":"bash","state":{"status":"completed","input":{"command":"ls"},"output":"listing output"}},
                {"type":"tool","callID":"c2","tool":"bash","state":{"status":"error","input":{"command":"git push"},"error":"remote rejected"}}]}
        ]});
        let text = &render(&opencode_entries(&export));
        assert!(text.contains("## user\n\nFix the build"));
        assert!(text.contains("## assistant\n\nChecking status."));
        assert!(text.contains(r#"- tool: bash {"command":"ls"}"#));
        assert!(text.contains("remote rejected"));
        for absent in [
            "rules",
            "injected file",
            "hidden reasoning",
            "listing output",
        ] {
            assert!(!text.contains(absent), "{absent} leaked into {text}");
        }
    }

    #[test]
    fn excerpt_keeps_the_beginning_and_end_of_a_long_log() {
        let log = format!(
            "## user\n\nFirst request\n\n{}\n\n## user\n\nLatest request\n",
            "middle ".repeat(10_000)
        );
        let quoted = excerpt(&log);
        assert!(quoted.starts_with("## user\n\nFirst request"));
        assert!(quoted.ends_with("Latest request"));
        assert!(quoted.contains("characters omitted"));
        assert!(quoted.chars().count() < EXCERPT_HEAD_CHARS + EXCERPT_TAIL_CHARS + 200);
        assert_eq!(excerpt("## user\n\nShort\n"), "## user\n\nShort");
    }
}
