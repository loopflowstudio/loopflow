//! Output format shared by harness runs and replay.

/// How to display streaming output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamFormat {
    /// Pass through raw JSON lines.
    Raw,
    /// Human-readable summary. Bool controls ANSI color.
    Human(bool),
}

impl Default for StreamFormat {
    fn default() -> Self {
        Self::Human(false)
    }
}
