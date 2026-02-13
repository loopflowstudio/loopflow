mod docker;
mod helpers;
mod local;
mod wave;

use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::engine::stream::{render_event, ParseResult, StreamParser};
use crate::lfd::output::{OutputEvent, OutputHub};
use crate::lfd::types::Wave;

pub use helpers::{create_wave_run_with_id, ensure_wave_worktree};
pub use wave::WaveExecutor;

#[async_trait]
pub trait AgentExecutor: Send + Sync {
    async fn run(
        &self,
        cmd: Vec<String>,
        cwd: &Path,
        wave_id: &str,
        agent_id: &str,
        wave_run_id: &str,
        output: &OutputHub,
    ) -> Result<i32>;
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
}

// Stream processing helpers — used by both LocalProcessExecutor and DockerExecutor.

pub(crate) async fn read_stream<R: tokio::io::AsyncRead + Unpin>(
    reader: R,
    output: OutputHub,
    wave_id: String,
    wave_run_id: String,
    agent_id: String,
) {
    let mut parser = StreamParser::new();
    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        handle_output_line(
            &line,
            &mut parser,
            &output,
            &wave_id,
            &wave_run_id,
            &agent_id,
        );
    }
}

pub(crate) fn handle_output_line(
    line: &str,
    parser: &mut StreamParser,
    output: &OutputHub,
    wave_id: &str,
    wave_run_id: &str,
    agent_id: &str,
) {
    match parser.feed_line(line) {
        ParseResult::Events(events) => {
            for event in &events {
                let (stdout, stderr) = render_event(event, false);
                let text = if !stdout.is_empty() { stdout } else { stderr };
                let text = text.trim_end_matches('\n').to_string();
                if !text.is_empty() {
                    send_output(output, wave_id, wave_run_id, agent_id, text);
                }
            }
        }
        ParseResult::Skipped => {}
        ParseResult::Passthrough => {
            send_output(output, wave_id, wave_run_id, agent_id, line.to_string());
        }
    }
}

pub(crate) fn send_output(
    output: &OutputHub,
    wave_id: &str,
    wave_run_id: &str,
    agent_id: &str,
    text: String,
) {
    output.send(OutputEvent {
        wave_id: wave_id.to_string(),
        wave_run_id: wave_run_id.to_string(),
        agent_id: agent_id.to_string(),
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
            output.clone(),
            "wave-1".to_string(),
            "run-1".to_string(),
            "agent-1".to_string(),
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
            output.clone(),
            "wave-1".to_string(),
            "run-2".to_string(),
            "agent-1".to_string(),
        )
        .await;

        write_task.await.expect("writer task should complete");

        let lines = output.read_log("run-2").expect("output log should exist").0;

        assert_eq!(
            lines,
            vec![r#"{"type":"mystery","payload":42}"#, "plain text line"]
        );
    }
}
