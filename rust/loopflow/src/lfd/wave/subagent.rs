//! A single subagent run: a child process that streams stream-json, folded
//! into [`ChatTurn`]s in-process.
//!
//! This is the unit of work the wave server spawns. A run owns one child
//! process (`codex exec --json`, or any command that emits the same
//! stream-json on stdout), parses each line with [`StreamParser`] into
//! [`StreamEvent`]s, and folds them into [`ChatTurn`]s via [`TurnBuilder`].
//! Every finalized turn is handed to a callback so the caller can narrate it.
//!
//! Nothing here touches files. The turns flow back through an in-process
//! callback; the caller (the progress arm, or a reactive spawn) decides what to
//! do with them.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::Command;

use crate::engine::stream::{ParseResult, StreamParser};
use crate::lfd::conversations::turns::{ChatTurn, TurnBuilder};

/// Drain a child's stderr to the debug log so a failing subagent leaves a
/// trace, without blocking the stdout parse loop.
fn drain_stderr<R>(stderr: R)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if !line.trim().is_empty() {
                tracing::debug!(target: "wave_subagent", "{line}");
            }
        }
    });
}

/// How to launch one subagent process.
///
/// `program` + `args` are spawned directly (no shell). `cwd` is the working
/// directory. The process must emit stream-json on stdout in one of the
/// formats [`StreamParser`] understands (codex `--json`, claude
/// `stream-json`, opencode `--format json`).
#[derive(Debug, Clone)]
pub struct SubagentSpec {
    pub label: String,
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
}

impl SubagentSpec {
    /// A real codex progress pass: `codex exec --json --full-auto <prompt>`,
    /// run in the wave's worktree. `--full-auto` keeps it sandboxed to
    /// workspace-write with safe auto-approval — the server drives it headless.
    pub fn codex_progress(cwd: &Path, prompt: &str) -> Self {
        Self {
            label: "progress".to_string(),
            program: "codex".to_string(),
            args: vec![
                "exec".to_string(),
                "--json".to_string(),
                "--full-auto".to_string(),
                prompt.to_string(),
            ],
            cwd: cwd.to_path_buf(),
        }
    }
}

/// Run one subagent to completion, invoking `on_turn` for every finalized
/// [`ChatTurn`]. Returns when the child exits (or its stdout closes).
///
/// The child's stdout is read line by line; each line is fed to the parser and
/// any resulting events folded into turns. If the process ends mid-turn, the
/// open turn is finalized (marked failed) so a partial pass still narrates.
///
/// # Errors
/// Returns an error only if the process can't be spawned or its stdout can't be
/// captured — a normal non-zero exit is not an error here (the failed turn
/// already carries that signal).
pub async fn run_subagent<F>(spec: &SubagentSpec, mut on_turn: F) -> Result<()>
where
    F: FnMut(ChatTurn),
{
    let mut child = Command::new(&spec.program)
        .args(&spec.args)
        .current_dir(&spec.cwd)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn subagent `{}`", spec.program))?;

    let stdout = child
        .stdout
        .take()
        .context("subagent stdout was not captured")?;
    if let Some(stderr) = child.stderr.take() {
        drain_stderr(stderr);
    }

    let mut parser = StreamParser::new();
    let mut builder = TurnBuilder::new();
    let mut lines = BufReader::new(stdout).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        if line.trim().is_empty() {
            continue;
        }
        if let ParseResult::Events(events) = parser.feed_line(&line) {
            for event in &events {
                if let Some(turn) = builder.feed(event) {
                    on_turn(turn);
                }
            }
        }
    }

    // The stream ended. Close any turn left open (no terminating Result).
    if let Some(turn) = builder.finish_open() {
        on_turn(turn);
    }

    let _ = child.wait().await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::conversations::turns::{ChatRole, ChatTurnStatus};

    /// A spec that replays canned codex `exec --json` output via `printf`, so
    /// the run path is exercised without a real codex binary.
    fn replay_spec(lines: &[&str]) -> SubagentSpec {
        let payload = lines.join("\n");
        SubagentSpec {
            label: "test".to_string(),
            program: "printf".to_string(),
            args: vec!["%s\\n".to_string(), payload],
            cwd: std::env::temp_dir(),
        }
    }

    #[tokio::test]
    async fn folds_codex_stream_into_a_completed_turn() {
        let spec = replay_spec(&[
            r#"{"type":"item.started","item":{"type":"command_execution","command":"cargo test"}}"#,
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"Implemented the feature."}}"#,
            r#"{"type":"turn.completed","usage":{"input_tokens":10,"output_tokens":5}}"#,
        ]);

        let mut turns = Vec::new();
        run_subagent(&spec, |turn| turns.push(turn))
            .await
            .expect("run subagent");

        assert_eq!(turns.len(), 1);
        let turn = &turns[0];
        assert_eq!(turn.role, ChatRole::Assistant);
        assert_eq!(turn.status, ChatTurnStatus::Completed);
        assert!(turn.text.contains("Implemented the feature."));
        assert_eq!(turn.items.len(), 1);
    }

    #[tokio::test]
    async fn stream_without_result_finalizes_open_turn() {
        let spec = replay_spec(&[
            r#"{"type":"item.completed","item":{"type":"agent_message","text":"half a thought"}}"#,
        ]);

        let mut turns = Vec::new();
        run_subagent(&spec, |turn| turns.push(turn))
            .await
            .expect("run subagent");

        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].status, ChatTurnStatus::Failed);
        assert!(turns[0].text.contains("half a thought"));
    }
}
