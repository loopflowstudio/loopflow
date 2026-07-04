//! The progress arm: autonomous, always-running work.
//!
//! Progress is independent of chat. This driver keeps a progress subagent alive
//! at all times: it builds a prompt from the wave's `GOAL.md` and current
//! memory, runs one supervised subagent pass (`codex exec --json`), and folds
//! every turn increment through a [`TurnSink`] — journaled as it happens,
//! committed to the thread at finalization — then immediately starts the next
//! pass. Each pass is a tracked run in the [`Supervisor`], so passes — and any
//! children a pass spawns — form a supervised set with independent lifetimes,
//! not a single flat bounded loop.

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::oneshot;

use crate::engine::flow::{available_flow_names, load_goal, render_goal, GoalRenderContext};
use crate::lfd::wave::memory::Memory;
use crate::lfd::wave::runtime::{TurnSink, WaveRuntime};
use crate::lfd::wave::subagent::{run_subagent, SubagentSpec};

/// Breather after a pass finishes so a fast-failing codex (missing binary,
/// auth error) can't hot-spin the arm. A healthy pass takes far longer than
/// this, so it adds no real latency to steady-state progress.
const PASS_BACKOFF: Duration = Duration::from_secs(2);

/// Run the progress arm until the runtime is shut down (the server spawns the
/// arm as a plain tokio task and aborts it directly; only the passes it spawns
/// live in the [`Supervisor`]). Passes run back-to-back so there is always one
/// progress subagent grinding.
pub async fn run_progress_arm(runtime: Arc<WaveRuntime>) {
    loop {
        let prompt = build_progress_prompt(runtime.repo_root(), runtime.name(), runtime.memory());
        let spec = SubagentSpec::codex_progress(runtime.repo_root(), &prompt);

        let rt = runtime.clone();
        let (done_tx, done_rx) = oneshot::channel();
        // The pass is its own tracked run. Its sink journals every increment
        // and commits each turn to the thread as it finalizes.
        runtime.supervisor().spawn(async move {
            let mut sink = TurnSink::new(rt);
            if let Err(err) = run_subagent(&spec, |delta| sink.on_delta(delta)).await {
                tracing::warn!(error = %err, "progress pass failed to run");
            }
            let _ = done_tx.send(());
        });

        // Keep one pass alive at a time: wait for it before starting the next.
        let _ = done_rx.await;
        runtime.supervisor().reap();
        tokio::time::sleep(PASS_BACKOFF).await;
    }
}

/// Build the progress prompt: the wave's rendered `GOAL.md` plus current
/// memory, or a minimal-but-real fallback when there's no `GOAL.md` so the
/// server still makes progress.
fn build_progress_prompt(repo: &Path, wave: &str, memory: &Memory) -> String {
    match load_goal(wave, repo) {
        Ok(goal) => {
            let ctx = GoalRenderContext {
                flows: available_flow_names(repo),
                roadmap: String::new(),
                memory: memory.read(),
                metrics: Vec::new(),
                in_flight: Vec::new(),
            };
            render_goal(&goal, &ctx)
        }
        Err(_) => fallback_prompt(wave, memory),
    }
}

fn fallback_prompt(wave: &str, memory: &Memory) -> String {
    let mem = memory.read();
    let mem_block = if mem.trim().is_empty() {
        "(memory is empty)".to_string()
    } else {
        mem
    };
    format!(
        "You are the progress worker for the '{wave}' wave. Make one concrete \
         unit of progress toward the wave's goal, then stop and summarize what \
         you did in a sentence.\n\nCurrent memory:\n{mem_block}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_prompt_includes_wave_and_memory() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("MEMORY.md"), "shipped the parser").expect("seed");
        let memory = Memory::for_wave(tmp.path(), "ship");

        let prompt = build_progress_prompt(tmp.path(), "ship", &memory);
        assert!(prompt.contains("ship"));
        assert!(prompt.contains("shipped the parser"));
    }

    #[test]
    fn goal_backed_prompt_renders_goal_body() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&dir).expect("dir");
        std::fs::write(dir.join("GOAL.md"), "---\n---\nDrive the ship wave.").expect("goal");
        std::fs::write(dir.join("MEMORY.md"), "last: wired chat").expect("mem");
        let memory = Memory::for_wave(tmp.path(), "ship");

        let prompt = build_progress_prompt(tmp.path(), "ship", &memory);
        assert!(prompt.contains("Drive the ship wave."));
        assert!(prompt.contains("wired chat"));
    }
}
