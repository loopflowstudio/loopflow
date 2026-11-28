//! What each native client (Claude Code, Codex, OpenCode, Pi, Antigravity)
//! writes.
//!
//! Nothing here opens Loopflow's store, starts a process or reads the process
//! table; `loopflow-execution` does that with what these modules decode.

mod antigravity;
pub mod antigravity_decode;
mod claude;
pub mod claude_decode;
mod codex;
pub mod codex_decode;
pub mod fact;
mod lf_tag;
mod opencode;
pub mod opencode_decode;
mod pi;
pub mod pi_decode;
mod redact;
pub mod transcript;

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum HarnessKind {
    Codex,
    Claude,
    OpenCode,
    Pi,
    Antigravity,
}

impl HarnessKind {
    pub fn parse(name: &str) -> Option<Self> {
        match name.trim().to_ascii_lowercase().as_str() {
            "codex" => Some(Self::Codex),
            "claude" => Some(Self::Claude),
            "opencode" => Some(Self::OpenCode),
            "pi" => Some(Self::Pi),
            "agy" => Some(Self::Antigravity),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Codex => "codex",
            Self::Claude => "claude",
            Self::OpenCode => "opencode",
            Self::Pi => "pi",
            Self::Antigravity => "agy",
        }
    }

    pub const ALL: [Self; 5] = [
        Self::Claude,
        Self::Codex,
        Self::OpenCode,
        Self::Pi,
        Self::Antigravity,
    ];

    /// What this client writes and where; launching it is execution's
    /// `harness::launcher`.
    pub fn harness(self) -> &'static dyn Harness {
        match self {
            Self::Claude => &Claude,
            Self::Codex => &Codex,
            Self::OpenCode => &OpenCode,
            Self::Pi => &Pi,
            Self::Antigravity => &Antigravity,
        }
    }
}

#[derive(Debug)]
pub struct Claude;

#[derive(Debug)]
pub struct Codex;

#[derive(Debug)]
pub struct OpenCode;

#[derive(Debug)]
pub struct Pi;

#[derive(Debug)]
pub struct Antigravity;

/// Whether `command` runs the program named `program`, at any path.
fn command_program(command: &str, program: &str) -> bool {
    command.split_whitespace().any(|part| {
        Path::new(part)
            .file_name()
            .is_some_and(|name| name == program)
    })
}

/// What a native client declares about itself: the commands that are it,
/// where it keeps transcripts and for how long, its model names and sign-in.
pub trait Harness: Sync + std::fmt::Debug {
    fn kind(&self) -> HarnessKind;
    /// Whether `command` is this client serving a headless conversation.
    fn recognizes_driver(&self, command: &str) -> bool;
    /// Markers a new conversation must clear from a parent provider's shell.
    fn caller_session_env(&self) -> &'static [&'static str];
    fn title_args(&self, _name: &str) -> Vec<String> {
        Vec::new()
    }
    fn model_rungs(&self) -> &'static [&'static str] {
        &[]
    }
    fn sign_in_advice(&self) -> &'static str;
    fn provider_env(&self) -> &'static [&'static str] {
        &[]
    }
    fn api_key_env(&self) -> &'static [&'static str] {
        &[]
    }
    fn validate_model(&self, _model: &str) -> Result<()> {
        Ok(())
    }

    /// Where the client keeps its conversations on this Machine.
    fn transcript_home(&self) -> PathBuf;
    fn transcript_reader(&self) -> fn(&[Value]) -> Vec<transcript::Turn>;
    /// The conversation a transcript file name holds.
    fn transcript_id<'a>(&self, _name: &'a str) -> Option<&'a str> {
        None
    }
    /// The directory under a home holding transcripts, and how deep they are.
    fn transcript_root(&self, home: &Path) -> (PathBuf, usize) {
        (home.join("projects"), 1)
    }
    fn transcript_watch_store(&self, _home: &Path) -> Option<PathBuf> {
        None
    }
    fn direct_transcript(&self, _home: &Path, _cwd: &Path, _id: &str) -> Option<PathBuf> {
        None
    }
    fn transcript_siblings(&self) -> bool {
        false
    }
    fn expiry_policy(&self, _home: Option<&Path>) -> transcript::ExpiryPolicy {
        transcript::ExpiryPolicy::Kept
    }
    fn transcript_markers(&self) -> &'static [&'static [u8]] {
        &transcript::live::MARKERS
    }
    /// When a person last typed into each wanted conversation, in Unix
    /// milliseconds.
    fn human_input_times(&self, _home: &Path, _wanted: &BTreeSet<&str>) -> BTreeMap<String, i64> {
        BTreeMap::new()
    }
    fn transcript_completion_detail(&self, _turn: &transcript::Turn, detail: &mut Value) {
        detail["error"] = Value::Null;
    }
}

/// `variable` when set and not empty, else `default` under the person's home.
fn home_from_env(variable: &str, default: &str) -> PathBuf {
    std::env::var_os(variable)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_default().join(default))
}

#[cfg(test)]
mod tests {
    use super::HarnessKind;

    #[test]
    fn parsing_normalizes_case_and_whitespace() {
        assert_eq!(
            HarnessKind::parse(" claUDe ").map(HarnessKind::as_str),
            Some("claude")
        );
        assert_eq!(
            HarnessKind::parse(" CODEX").map(HarnessKind::as_str),
            Some("codex")
        );
        assert_eq!(
            HarnessKind::parse("OpenCode").map(HarnessKind::as_str),
            Some("opencode")
        );
        assert_eq!(
            HarnessKind::parse(" AGY").map(HarnessKind::as_str),
            Some("agy")
        );
        assert_eq!(HarnessKind::parse("antigravity"), None);
        assert_eq!(
            HarnessKind::parse("lfharness").map(HarnessKind::as_str),
            None
        );
    }
}
