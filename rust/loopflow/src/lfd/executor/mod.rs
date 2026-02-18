pub(crate) mod docker;
pub(crate) mod helpers;
pub(crate) mod local;
mod wave;

use std::path::Path;

use anyhow::Result;
use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::engine::stream::{render_event, ParseResult, StreamParser};
use crate::lfd::id::LfdId;
use crate::lfd::output::{OutputEvent, OutputHub};
use crate::lfd::types::Wave;

pub use helpers::{create_wave_run_with_id, ensure_wave_worktree};
pub use wave::WaveExecutor;

#[derive(Debug, Clone, Copy)]
pub struct AgentRunContext<'a> {
    pub wave_id: &'a str,
    pub agent_id: &'a str,
    pub wave_run_id: &'a str,
    pub output: &'a OutputHub,
    pub output_prefix: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub(crate) struct OutputContext {
    pub(crate) wave_id: String,
    pub(crate) wave_run_id: String,
    pub(crate) agent_id: String,
    pub(crate) output: OutputHub,
    pub(crate) output_prefix: Option<String>,
}

impl<'a> From<AgentRunContext<'a>> for OutputContext {
    fn from(context: AgentRunContext<'a>) -> Self {
        Self {
            wave_id: context.wave_id.to_string(),
            wave_run_id: context.wave_run_id.to_string(),
            agent_id: context.agent_id.to_string(),
            output: context.output.clone(),
            output_prefix: context.output_prefix.map(str::to_string),
        }
    }
}

#[async_trait]
pub trait AgentExecutor: Send + Sync {
    async fn run(&self, cmd: Vec<String>, cwd: &Path, context: AgentRunContext<'_>) -> Result<i32>;
    async fn terminate(&self, agent_id: &str) -> Result<()>;
    async fn recover_startup(&self, _output: &OutputHub) -> Result<StartupRecovery> {
        Ok(StartupRecovery::default())
    }
    async fn cleanup_wave(&self, _wave: &Wave) -> Result<()> {
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StartupRecovery {
    pub orphaned_runs_failed: u32,
    pub rehydrated_agents: u32,
    pub lost_agents_failed: u32,
    pub orphaned_containers_removed: u32,
    pub orphaned_fork_runs_cleaned: u32,
    pub orphaned_fork_worktrees_removed: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EphemeralOwnerKind {
    Fork,
    Sidecar,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EphemeralWorktree {
    pub path: String,
    pub owner_kind: EphemeralOwnerKind,
    pub owner_id: String,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JanitorReport {
    pub removed: u32,
    pub active: u32,
    pub errors: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CiFailure {
    pub wave_id: LfdId,
    pub wave_run_id: LfdId,
    pub pr_number: u32,
    pub branch: String,
    pub commit_sha: String,
    pub check_name: String,
    pub logs_url: String,
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

        assert_eq!(
            lines,
            vec![r#"{"type":"mystery","payload":42}"#, "plain text line"]
        );
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
