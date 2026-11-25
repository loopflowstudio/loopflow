//! What Claude Code declares about itself, and what it reports in both its
//! live stream and its saved transcript.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use loopflow_model::chat::types::TurnUsage;
use serde_json::Value;

use crate::transcript::claude::is_user_input;
use crate::transcript::{self, json_lines, ExpiryPolicy, Turn};
use crate::{Claude, Harness, HarnessKind};

/// Claude's token fields, shared by live stream and saved transcript decoding.
pub(crate) fn usage(value: &serde_json::Value) -> TurnUsage {
    let tokens = |key: &str| value.get(key).and_then(serde_json::Value::as_u64);
    let input_tokens = tokens("input_tokens");
    let cache_read_tokens = tokens("cache_read_input_tokens");
    let cache_write_tokens = tokens("cache_creation_input_tokens");
    TurnUsage {
        input_tokens,
        total_input_tokens: input_tokens
            .map(|input| input + cache_read_tokens.unwrap_or(0) + cache_write_tokens.unwrap_or(0)),
        cache_read_tokens,
        cache_write_tokens,
        output_tokens: tokens("output_tokens"),
        reasoning_tokens: tokens("reasoning_tokens"),
        ..TurnUsage::default()
    }
}

fn input_time(path: &Path, id: &str) -> Option<i64> {
    json_lines(path)
        .filter_map(|row| {
            if row["sessionId"].as_str() != Some(id) || !is_user_input(&row) {
                return None;
            }
            let at = time::OffsetDateTime::parse(
                row["timestamp"].as_str()?,
                &time::format_description::well_known::Rfc3339,
            )
            .ok()?;
            i64::try_from(at.unix_timestamp_nanos() / 1_000_000).ok()
        })
        .max()
}

impl Harness for Claude {
    fn recognizes_driver(&self, command: &str) -> bool {
        crate::command_program(command, "claude")
    }
    fn sign_in_advice(&self) -> &'static str {
        "run `claude auth login`"
    }
    fn caller_session_env(&self) -> &'static [&'static str] {
        &[
            "CLAUDECODE",
            "CLAUDE_CODE_CHILD_SESSION",
            "CLAUDE_CODE_SESSION_ID",
            "CLAUDE_CODE_SESSION_ATTENDED",
            "CLAUDE_CODE_ENTRYPOINT",
            "CLAUDE_CODE_EXECPATH",
            "CLAUDE_CODE_MESSAGING_SOCKET",
            "CLAUDE_CODE_MESSAGING_TOKEN",
            "CLAUDE_CODE_CHILD_NOTES_DIR",
            "CLAUDE_PID",
            "CLAUDE_EFFORT",
        ]
    }
    fn kind(&self) -> HarnessKind {
        HarnessKind::Claude
    }
    fn title_args(&self, name: &str) -> Vec<String> {
        vec!["--name".into(), name.into()]
    }
    /// The models a bare `claude` step falls through to when the one it runs
    /// hits a scoped limit, best first.
    fn model_rungs(&self) -> &'static [&'static str] {
        &["opus", "sonnet", "haiku"]
    }
    fn provider_env(&self) -> &'static [&'static str] {
        &["CLAUDE_CODE_OAUTH_TOKEN", "ANTHROPIC_API_KEY"]
    }
    fn transcript_home(&self) -> PathBuf {
        crate::home_from_env("CLAUDE_CONFIG_DIR", ".claude")
    }
    fn transcript_reader(&self) -> fn(&[Value]) -> Vec<Turn> {
        transcript::claude::read
    }
    fn transcript_id<'a>(&self, name: &'a str) -> Option<&'a str> {
        name.strip_suffix(".jsonl")
    }
    fn direct_transcript(&self, home: &Path, cwd: &Path, id: &str) -> Option<PathBuf> {
        Some(transcript::live::claude_project(home, cwd).join(format!("{id}.jsonl")))
    }
    fn transcript_siblings(&self) -> bool {
        true
    }
    fn expiry_policy(&self, home: Option<&Path>) -> ExpiryPolicy {
        let settings = home.map(|home| home.join("settings.json"));
        let days = settings
            .as_deref()
            .and_then(|path| std::fs::read(path).ok())
            .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
            .and_then(|value| value["cleanupPeriodDays"].as_u64())
            .and_then(|days| u32::try_from(days).ok());
        match days {
            Some(days) => ExpiryPolicy::ClaudeCleanup {
                days,
                settings: settings.map(|path| path.display().to_string()),
            },
            None => ExpiryPolicy::ClaudeCleanup {
                days: 30,
                settings: None,
            },
        }
    }
    fn human_input_times(&self, home: &Path, wanted: &BTreeSet<&str>) -> BTreeMap<String, i64> {
        wanted
            .iter()
            .filter_map(|id| {
                let path = transcript::transcript(self.kind(), home, id)?;
                Some(((*id).to_owned(), input_time(&path, id)?))
            })
            .collect()
    }
    fn transcript_completion_detail(&self, turn: &Turn, detail: &mut Value) {
        detail["result_id"] = Value::Null;
        detail["subtype"] = Value::Null;
        detail["errors"] = if turn.receipt.errors.is_empty() {
            Value::Null
        } else {
            turn.receipt.errors.clone().into()
        };
    }
}
