//! A native conversation as plain text another provider can read: what people
//! asked, what the agent said, one line per tool call, and the tail of each
//! failed call's output. No summarization: each harness's transcript is read
//! through `transcript`.

use std::fs;

use anyhow::{anyhow, bail, Context, Result};
use serde_json::Value;

use crate::chat::types::{ConversationItem, Lifecycle};
use crate::harness::HarnessKind;
use crate::session::LfSession;
use crate::store::sqlite::SqliteStore;
use crate::transcript::{self, RequestSource, Turn};

const TOOL_INPUT_CHARS: usize = 200;
const ERROR_TAIL_CHARS: usize = 400;
/// The first turn quotes the log's opening request and its latest exchanges,
/// staying well under the terminal argument cap.
const EXCERPT_HEAD_CHARS: usize = 6_000;
const EXCERPT_TAIL_CHARS: usize = 14_000;

/// Read the history before preparing a provider change or releasing its terminal.
pub(super) fn read(store: &SqliteStore, session: &LfSession) -> Result<String> {
    let id = &session.id;
    let from = session.provider.as_deref().unwrap_or_default();
    let harness = HarnessKind::parse(from)
        .ok_or_else(|| anyhow!("Session {id} has no {from} history another provider can read"))?;
    let reference = store
        .input_provider_session(session.input_key()?)?
        .ok_or_else(|| anyhow!("Session {id} has no native conversation to continue"))?;
    let thread = &reference.agent_session;
    let located = transcript::locate(
        store,
        harness,
        thread,
        reference.account_id.as_ref(),
        &session.cwd,
    )?;
    let transcript::Located::Found { source, .. } = located else {
        bail!("The {from} transcript of Session {id} ({thread}) was not found. Keep this conversation open and retry after {from} saves its first turn.");
    };
    Ok(render(&transcript::read(harness, &source)?))
}

/// Save the current conversation and return the first turn for another provider.
pub(crate) fn continuation(store: &SqliteStore, session: &LfSession) -> Result<String> {
    let id = &session.id;
    let from = session.provider.as_deref().unwrap_or_default();
    let log = read(store, session)?;
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

/// The conversation as markdown: requests people made or launched, what the
/// agent said, one line per tool call, and the tail of each failure.
fn render(turns: &[Turn]) -> String {
    let mut markdown = String::new();
    for turn in turns {
        let request = turn
            .request
            .as_ref()
            .and_then(|request| match request.from {
                RequestSource::Person => Some(request.text.clone()),
                RequestSource::Injected => {
                    transcript::Launch::parse(&request.text).map(|launch| launch.text())
                }
            });
        if let Some(text) = request {
            markdown.push_str(&format!("## user\n\n{}\n\n", text.trim()));
        }
        for item in &turn.items {
            match item {
                ConversationItem::Message { text, .. } => {
                    markdown.push_str(&format!("## assistant\n\n{}\n\n", text.trim()))
                }
                ConversationItem::Tool {
                    name,
                    status,
                    input,
                    output,
                    ..
                } => {
                    let input = match input {
                        Some(Value::String(input)) => input.clone(),
                        Some(input) => input.to_string(),
                        None => String::new(),
                    };
                    let input = input.split_whitespace().collect::<Vec<_>>().join(" ");
                    markdown.push_str(&format!(
                        "- tool: {name} {}\n",
                        head(&input, TOOL_INPUT_CHARS)
                    ));
                    if *status == Lifecycle::Failed {
                        markdown.push_str(&format!(
                            "  failed: {}\n",
                            failure(output.as_deref().unwrap_or_default())
                        ));
                    }
                    markdown.push('\n');
                }
                _ => {}
            }
        }
    }
    markdown
}

fn failure(output: &str) -> String {
    tail(output.trim(), ERROR_TAIL_CHARS).replace('\n', "\n    ")
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
    use crate::transcript::Source;

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
        let text = &render(
            &transcript::read(HarnessKind::Claude, &Source::File(transcript.path().into()))
                .unwrap(),
        );
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
        let text = &render(
            &transcript::read(HarnessKind::Codex, &Source::File(transcript.path().into())).unwrap(),
        );
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
        let export = serde_json::json!({"messages":[
            {"type":"user","text":"Fix the build\n\n<lf:loopflow>rules</lf:loopflow>"},
            {"type":"synthetic","text":"injected file"},
            {"type":"assistant","content":[
                {"type":"reasoning","text":"hidden reasoning"},
                {"type":"text","text":"Checking status."},
                {"type":"tool","id":"c1","name":"bash","state":{"status":"completed","input":{"command":"ls"},"content":[{"type":"text","text":"listing output"}]}},
                {"type":"tool","id":"c2","name":"bash","state":{"status":"error","input":{"command":"git push"},"error":"remote rejected"}}]}
        ]});
        let text = &render(&transcript::opencode::turns(&export));
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
