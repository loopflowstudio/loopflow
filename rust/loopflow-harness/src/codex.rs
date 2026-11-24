//! What Codex declares about itself.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::transcript::{self, json_lines, Turn};
use crate::{command_program, Codex, Harness, HarnessKind};

/// Codex keeps what people typed, across conversations, in `history.jsonl`.
fn input_times(path: &Path, wanted: &BTreeSet<&str>) -> BTreeMap<String, i64> {
    let mut times = BTreeMap::<String, i64>::new();
    for row in json_lines(path) {
        let Some(id) = row["session_id"].as_str().filter(|id| wanted.contains(id)) else {
            continue;
        };
        let Some(at) = row["ts"]
            .as_i64()
            .filter(|at| *at >= 0)
            .and_then(|at| at.checked_mul(1000))
        else {
            continue;
        };
        times
            .entry(id.to_string())
            .and_modify(|saved| *saved = (*saved).max(at))
            .or_insert(at);
    }
    times
}

impl Harness for Codex {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Codex
    }
    fn recognizes_driver(&self, command: &str) -> bool {
        command_program(command, "codex")
            && command.split_whitespace().any(|arg| arg == "app-server")
            && command.split_whitespace().any(|arg| arg == "--listen")
    }
    fn title_args(&self, _name: &str) -> Vec<String> {
        vec!["-c".into(), "tui.terminal_title=[]".into()]
    }
    fn provider_env(&self) -> &'static [&'static str] {
        &["CODEX_ACCESS_TOKEN", "OPENAI_API_KEY"]
    }
    fn transcript_home(&self) -> PathBuf {
        crate::home_from_env("CODEX_HOME", ".codex")
    }
    fn transcript_reader(&self) -> fn(&[Value]) -> Vec<Turn> {
        transcript::codex::read
    }
    /// `sessions/<year>/<month>/<day>/rollout-<time>-<id>.jsonl`
    fn transcript_id<'a>(&self, name: &'a str) -> Option<&'a str> {
        let rest = name.strip_suffix(".jsonl")?.strip_prefix("rollout-")?;
        let at = rest.len().checked_sub(36)?;
        rest.is_char_boundary(at)
            .then(|| &rest[at..])
            .filter(|id| uuid::Uuid::parse_str(id).is_ok())
    }
    fn transcript_root(&self, home: &Path) -> (PathBuf, usize) {
        (home.join("sessions"), 3)
    }
    fn human_input_times(&self, home: &Path, wanted: &BTreeSet<&str>) -> BTreeMap<String, i64> {
        input_times(&home.join("history.jsonl"), wanted)
    }
}
