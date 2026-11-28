//! What OpenCode declares about itself.

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde_json::Value;

use crate::transcript::{self, Turn};
use crate::{command_program, Harness, HarnessKind, OpenCode};

fn is_supported_model_variant(variant: &str) -> bool {
    let variant = variant.trim().to_ascii_lowercase();
    let Some((provider, model_id)) = variant.split_once('/') else {
        return false;
    };
    if model_id.is_empty() {
        return false;
    }

    match provider {
        "moonshotai" => model_id.starts_with("kimi"),
        "opencode" => !model_id.contains("claude") && !model_id.contains("codex"),
        _ => false,
    }
}

impl Harness for OpenCode {
    fn kind(&self) -> HarnessKind {
        HarnessKind::OpenCode
    }
    fn recognizes_driver(&self, command: &str) -> bool {
        command_program(command, "opencode") && command.split_whitespace().any(|arg| arg == "run")
    }
    fn caller_session_env(&self) -> &'static [&'static str] {
        &[
            "OPENCODE",
            "OPENCODE_SESSION_ID",
            "OPENCODE_TERMINAL",
            "OPENCODE_CONFIG_CONTENT",
        ]
    }
    fn sign_in_advice(&self) -> &'static str {
        "run `opencode auth login`"
    }
    fn provider_env(&self) -> &'static [&'static str] {
        &["OPENCODE_API_KEY"]
    }
    fn api_key_env(&self) -> &'static [&'static str] {
        &["OPENCODE_API_KEY", "MOONSHOT_API_KEY"]
    }
    fn validate_model(&self, model: &str) -> Result<()> {
        anyhow::ensure!(is_supported_model_variant(model), "unsupported OpenCode model '{}': supported variants are 'opencode/*' (excluding claude/codex families) and 'moonshotai/kimi*'", model);
        Ok(())
    }
    /// OpenCode honors `XDG_DATA_HOME`.
    fn transcript_home(&self) -> PathBuf {
        crate::home_from_env("XDG_DATA_HOME", ".local/share").join("opencode")
    }
    fn transcript_reader(&self) -> fn(&[Value]) -> Vec<Turn> {
        transcript::opencode::read
    }
    fn transcript_root(&self, home: &Path) -> (PathBuf, usize) {
        (home.to_path_buf(), 0)
    }
    fn transcript_watch_store(&self, home: &Path) -> Option<PathBuf> {
        Some(transcript::opencode::store(home))
    }
}
