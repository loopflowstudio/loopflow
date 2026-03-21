pub(crate) mod docker;
pub(crate) mod helpers;
pub(crate) mod local;
pub(crate) mod wave;

use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::engine::stream::{render_event, ParseResult, StreamParser};
use crate::lfd::output::{OutputEvent, OutputHub};
use crate::lfd::types::Wave;

pub use helpers::{create_parallel_wave_run, create_wave_run_with_id, ensure_wave_worktree};
pub use wave::WaveExecutor;

pub(crate) fn write_workspace_file(cwd: &Path, relative_path: &str, content: &[u8]) -> Result<()> {
    let path = cwd.join(relative_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

pub(crate) fn remove_workspace_file(cwd: &Path, relative_path: &str) -> Result<()> {
    let path = cwd.join(relative_path);
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

pub(crate) fn cleanup_workspace_worktree(worktree: &Path) -> Result<()> {
    if !worktree.exists() {
        return Ok(());
    }

    if worktree.join(".git").exists() {
        crate::engine::worktree::remove_worktree(worktree, true)?;
    } else {
        std::fs::remove_dir_all(worktree)?;
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct AgentRunContext {
    pub wave_id: String,
    pub agent_id: String,
    pub wave_run_id: String,
    pub branch: Option<String>,
    pub output: OutputHub,
    pub output_prefix: Option<String>,
    pub extra_env: Vec<(String, String)>,
}

#[derive(Debug, Clone)]
pub(crate) struct OutputContext {
    pub(crate) wave_id: String,
    pub(crate) wave_run_id: String,
    pub(crate) agent_id: String,
    pub(crate) output: OutputHub,
    pub(crate) output_prefix: Option<String>,
}

impl From<AgentRunContext> for OutputContext {
    fn from(context: AgentRunContext) -> Self {
        Self {
            wave_id: context.wave_id,
            wave_run_id: context.wave_run_id,
            agent_id: context.agent_id,
            output: context.output,
            output_prefix: context.output_prefix,
        }
    }
}

#[async_trait]
pub trait AgentExecutor: Send + Sync {
    async fn run(&self, cmd: Vec<String>, cwd: &Path, context: AgentRunContext) -> Result<i32>;
    async fn terminate(&self, agent_id: &str) -> Result<()>;
    async fn write_to_workspace(
        &self,
        cwd: &Path,
        relative_path: &str,
        content: &[u8],
    ) -> Result<()> {
        write_workspace_file(cwd, relative_path, content)
    }
    async fn remove_from_workspace(&self, cwd: &Path, relative_path: &str) -> Result<()> {
        remove_workspace_file(cwd, relative_path)
    }
    async fn cleanup_ephemeral_worktree(&self, _repo: &Path, worktree: &Path) -> Result<()> {
        cleanup_workspace_worktree(worktree)
    }
    async fn recover_startup(&self, _output: &OutputHub) -> Result<StartupRecovery> {
        Ok(StartupRecovery::default())
    }
    async fn ensure_wave_workspace(&self, _wave: &Wave) -> Result<()> {
        Ok(())
    }
    async fn cleanup_wave_workspace(&self, _wave: &Wave) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StartupRecovery {
    pub orphaned_runs_failed: u32,
    pub rehydrated_agents: u32,
    pub lost_agents_failed: u32,
    pub orphaned_containers_removed: u32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JanitorReport {
    pub removed: u32,
    pub active: u32,
    pub errors: u32,
}

// -- Stream helpers ----------------------------------------------------------

pub(crate) async fn read_stream<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    context: OutputContext,
) {
    let mut parser = StreamParser::new();
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        handle_output_line(&line, &mut parser, &context);
    }
}

pub(crate) fn handle_output_line(line: &str, parser: &mut StreamParser, context: &OutputContext) {
    match parser.feed_line(line) {
        ParseResult::Events(events) => {
            for event in &events {
                let (stdout, stderr) = render_event(event, false);
                let text = if !stdout.is_empty() { stdout } else { stderr };
                let text = text.trim_end_matches('\n').to_string();
                if !text.is_empty() {
                    send_output(context, text);
                }
            }
        }
        ParseResult::Skipped => {}
        ParseResult::Passthrough => {
            send_output(context, line.to_string());
        }
    }
}

fn send_output(context: &OutputContext, text: String) {
    let text = if let Some(prefix) = context.output_prefix.as_deref() {
        format!("{prefix}{text}")
    } else {
        text
    };
    context.output.send(OutputEvent {
        wave_id: context.wave_id.clone(),
        wave_run_id: context.wave_run_id.clone(),
        agent_id: context.agent_id.clone(),
        text,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use tokio::io::{AsyncWriteExt, DuplexStream};

    async fn write_lines(mut writer: DuplexStream, lines: &[&str]) {
        for line in lines {
            writer
                .write_all(line.as_bytes())
                .await
                .expect("writer should accept line");
            writer
                .write_all(b"\n")
                .await
                .expect("writer should accept newline");
        }
        writer.shutdown().await.expect("writer should shut down");
    }

    #[tokio::test]
    async fn read_stream_renders_stream_json_events() {
        let output_dir = tempdir().expect("tempdir should be created");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let (writer, reader) = tokio::io::duplex(4096);

        let write_task = tokio::spawn(write_lines(
            writer,
            &[
                r#"{"type":"assistant","message":{"content":[{"type":"text","text":"hello"}]}}"#,
                r#"{"type":"assistant","message":{"content":[{"type":"tool_use","name":"Read","input":{"file_path":"src/lib.rs"}}]}}"#,
                r#"{"type":"result","subtype":"success"}"#,
            ],
        ));

        read_stream(
            reader,
            OutputContext {
                wave_id: "wave-1".to_string(),
                wave_run_id: "run-1".to_string(),
                agent_id: "agent-1".to_string(),
                output: output.clone(),
                output_prefix: None,
            },
        )
        .await;

        write_task.await.expect("writer task should complete");

        let lines = output.read_log("run-1").expect("output log should exist").0;

        assert_eq!(lines, vec!["hello", "-> Read  src/lib.rs", "ok"]);
    }

    #[tokio::test]
    async fn read_stream_skips_known_events_and_passes_through_unknown_lines() {
        let output_dir = tempdir().expect("tempdir should be created");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let (writer, reader) = tokio::io::duplex(4096);

        let write_task = tokio::spawn(write_lines(
            writer,
            &[
                r#"{"type":"system","message":"skip me"}"#,
                r#"{"type":"mystery","payload":42}"#,
                "plain text line",
            ],
        ));

        read_stream(
            reader,
            OutputContext {
                wave_id: "wave-1".to_string(),
                wave_run_id: "run-2".to_string(),
                agent_id: "agent-1".to_string(),
                output: output.clone(),
                output_prefix: None,
            },
        )
        .await;

        write_task.await.expect("writer task should complete");

        let lines = output.read_log("run-2").expect("output log should exist").0;

        assert_eq!(lines, vec!["plain text line"]);
    }

    #[test]
    fn handle_output_line_applies_output_prefix() {
        let output_dir = tempdir().expect("tempdir should be created");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let mut parser = StreamParser::new();

        handle_output_line(
            "plain text line",
            &mut parser,
            &OutputContext {
                wave_id: "wave-1".to_string(),
                wave_run_id: "run-prefix".to_string(),
                agent_id: "agent-1".to_string(),
                output: output.clone(),
                output_prefix: Some("[fork-0] ".to_string()),
            },
        );

        let lines = output
            .read_log("run-prefix")
            .expect("output log should exist")
            .0;

        assert_eq!(lines, vec!["[fork-0] plain text line"]);
    }
}
