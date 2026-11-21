//! What Antigravity declares about itself. Its CLI is `agy`, which execs a
//! process named `antigravity`.

use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::transcript::{self, Turn};
use crate::{command_program, Antigravity, Harness, HarnessKind};

impl Harness for Antigravity {
    fn kind(&self) -> HarnessKind {
        HarnessKind::Antigravity
    }
    fn sign_in_advice(&self) -> &'static str {
        "run `agy` and sign in"
    }
    fn recognizes_driver(&self, command: &str) -> bool {
        (command_program(command, "agy") || command_program(command, "antigravity"))
            && command
                .split_whitespace()
                .any(|arg| arg == "--output-format")
    }
    fn transcript_home(&self) -> PathBuf {
        dirs::home_dir()
            .unwrap_or_default()
            .join(".gemini/antigravity-cli")
    }
    fn transcript_reader(&self) -> fn(&[Value]) -> Vec<Turn> {
        transcript::antigravity::read
    }
    fn direct_transcript(&self, home: &Path, _cwd: &Path, id: &str) -> Option<PathBuf> {
        Some(transcript::antigravity::file(home, id))
    }
    fn transcript_markers(&self) -> &'static [&'static [u8]] {
        &[]
    }
}
