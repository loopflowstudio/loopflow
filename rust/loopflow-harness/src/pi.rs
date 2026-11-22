//! What Pi declares about itself.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::transcript::{self, Turn};
use crate::{command_program, Harness, HarnessKind, Pi};

impl Harness for Pi {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Pi
    }
    fn recognizes_driver(&self, command: &str) -> bool {
        (command_program(command, "pi")
            || command
                .split_whitespace()
                .any(|arg| arg.contains("/pi-coding-agent/") && arg.ends_with("/cli.js")))
            && command.split_whitespace().any(|arg| arg == "--mode")
            && command.split_whitespace().any(|arg| arg == "rpc")
    }
    fn api_key_env(&self) -> &'static [&'static str] {
        &[
            "ANTHROPIC_API_KEY",
            "OPENAI_API_KEY",
            "GEMINI_API_KEY",
            "MOONSHOT_API_KEY",
        ]
    }
    /// Pi honors `PI_CODING_AGENT_DIR`.
    fn transcript_home(&self) -> PathBuf {
        crate::home_from_env("PI_CODING_AGENT_DIR", ".pi/agent")
    }
    fn transcript_reader(&self) -> fn(&[Value]) -> Vec<Turn> {
        transcript::pi::read
    }
    fn transcript_root(&self, home: &Path) -> (PathBuf, usize) {
        (home.join("sessions"), 1)
    }
    /// Every Pi row is parsed: its entries chain through parents.
    fn transcript_markers(&self) -> &'static [&'static [u8]] {
        &[]
    }
}
