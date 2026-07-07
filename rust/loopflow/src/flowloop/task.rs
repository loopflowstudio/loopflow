use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use serde::Deserialize;
use time::OffsetDateTime;

use crate::lfd::executor::{create_run_for_placement, Placement};
use crate::lfd::id::LfdId;
use crate::lfd::types::{PullRequest, Run, RunStatus};
use crate::lfdb::{open_existing_store, SharedStore};
use crate::ops::pm::{pm_show, pm_update, PmShowOptions, PmUpdateOptions};
use crate::ops::{current_pr, NullProgress, OpsError, OpsResult};

const TASK_PASS_FLOW: &str = "task-pass";
const DEFAULT_MAX_PASSES: u32 = 8;
const DEFAULT_PASS_TIMEOUT_SECS: u64 = 60 * 30;
const DEFAULT_WALL_CLOCK_SECS: u64 = 60 * 60 * 2;
const DEFAULT_POLL_SECS: u64 = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskLoopOptions {
    pub item_id: String,
    pub wave: Option<String>,
    pub max_passes: u32,
    pub pass_timeout: Duration,
    pub wall_clock: Duration,
    pub poll: Duration,
    pub max_turns: Option<u32>,
}

impl TaskLoopOptions {
    pub fn new(item_id: String, wave: Option<String>) -> Self {
        Self {
            item_id,
            wave,
            max_passes: DEFAULT_MAX_PASSES,
            pass_timeout: Duration::from_secs(DEFAULT_PASS_TIMEOUT_SECS),
            wall_clock: Duration::from_secs(DEFAULT_WALL_CLOCK_SECS),
            poll: Duration::from_secs(DEFAULT_POLL_SECS),
            max_turns: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskStatement {
    id: String,
    title: String,
    description: String,
}

impl TaskStatement {
    fn prompt(&self) -> String {
        format!(
            "Linear task: {}\n\nTitle: {}\n\nDescription:\n{}",
            self.id, self.title, self.description
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PrState {
    Open,
    Merged,
    Closed,
}

impl PrState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Merged => "merged",
            Self::Closed => "closed",
        }
    }
}

/// A PR the task loop is tracking. Absence (no PR yet) is `Option::None`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PrOracle {
    number: u64,
    url: String,
    state: PrState,
}

pub fn run_task_loop(repo: &Path, options: &TaskLoopOptions) -> OpsResult<()> {
    let wave_name = crate::ops::util::resolve_wave_name(repo, options.wave.as_deref())
        .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?;
    let task = resolve_task_statement(repo, &wave_name, &options.item_id)?;
    let runtime = tokio::runtime::Runtime::new()
        .map_err(|err| OpsError::Message(format!("failed to build task runtime: {err}")))?;
    let store: SharedStore = Arc::new(runtime.block_on(async {
        open_existing_store().await.ok_or_else(|| {
            OpsError::Message(
                "no run registry on this machine - start or register the wave first".to_string(),
            )
        })
    })?);

    let mut run = runtime.block_on(async {
        let wave = store
            .get_wave_by_name(&wave_name)
            .await
            .map_err(|err| OpsError::Message(format!("failed to read wave registry: {err}")))?
            .ok_or_else(|| OpsError::Message(format!("wave '{wave_name}' not found")))?;
        let run_id = LfdId::new();
        let mut run = create_run_for_placement(&store, &wave, &run_id, &Placement::Fresh, None)
            .await
            .map_err(|err| OpsError::Message(format!("failed to create task worktree: {err}")))?;
        run.flow = TASK_PASS_FLOW.to_string();
        run.task = Some(task.prompt());
        store
            .update_run(&run)
            .await
            .map_err(|err| OpsError::Message(format!("failed to update task run: {err}")))?;
        Ok::<Run, OpsError>(run)
    })?;

    eprintln!("task {} running in {}", task.id, run.worktree);
    let result = run_task_passes(
        &PathBuf::from(&run.worktree),
        &wave_name,
        &task,
        options,
        |pr| update_run_pr(&runtime, &store, &mut run, pr),
    );

    runtime.block_on(async {
        run.status = if result.is_ok() {
            RunStatus::Completed
        } else {
            RunStatus::Failed
        };
        run.ended_at = Some(OffsetDateTime::now_utc());
        run.error = result.as_ref().err().map(ToString::to_string);
        store
            .update_run(&run)
            .await
            .map_err(|err| OpsError::Message(format!("failed to finish task run: {err}")))
    })?;

    result
}

fn resolve_task_statement(repo: &Path, wave: &str, item_id: &str) -> OpsResult<TaskStatement> {
    let result = pm_show(
        repo,
        &PmShowOptions {
            wave: Some(wave.to_string()),
        },
        &NullProgress,
    )?;
    let item = result
        .items
        .into_iter()
        .find(|item| item.id == item_id)
        .ok_or_else(|| OpsError::Message(format!("roadmap item not found: {item_id}")))?;
    Ok(TaskStatement {
        id: item.id,
        title: item.name,
        description: item.description,
    })
}

fn run_task_passes(
    worktree: &Path,
    wave: &str,
    task: &TaskStatement,
    options: &TaskLoopOptions,
    mut on_pr: impl FnMut(Option<&PrOracle>) -> OpsResult<()>,
) -> OpsResult<()> {
    let started = Instant::now();
    let mut remembered_pr: Option<u64> = None;
    for pass in 1..=options.max_passes {
        if let Err(err) = check_wall_clock(started, options.wall_clock) {
            escalate_parent(&err.to_string());
            return Err(err);
        }

        let before = poll_pr_oracle(worktree, remembered_pr)?;
        on_pr(before.as_ref())?;
        if let Some(pr) = &before {
            match pr.state {
                PrState::Merged => return close_task(worktree, wave, task, pr),
                PrState::Closed => return Err(closed_without_merging(pr)),
                PrState::Open if worktree_clean(worktree)? => {
                    eprintln!("task PR open; waiting for merge");
                    thread::sleep(options.poll);
                    remembered_pr = Some(pr.number);
                    continue;
                }
                PrState::Open => {}
            }
        }

        eprintln!("task pass {pass}/{}", options.max_passes);
        run_task_pass(worktree, task, options)?;

        let after = poll_pr_oracle(worktree, remembered_pr)?;
        remembered_pr = after.as_ref().map(|pr| pr.number).or(remembered_pr);
        on_pr(after.as_ref())?;
        if let Some(pr) = &after {
            match pr.state {
                PrState::Merged => return close_task(worktree, wave, task, pr),
                PrState::Closed => return Err(closed_without_merging(pr)),
                PrState::Open => {}
            }
        }
    }

    let message = format!(
        "task {} exhausted {} pass(es) without a merged PR",
        task.id, options.max_passes
    );
    escalate_parent(&message);
    Err(OpsError::Message(message))
}

fn check_wall_clock(started: Instant, wall_clock: Duration) -> OpsResult<()> {
    if started.elapsed() >= wall_clock {
        let message = format!(
            "task flowloop exceeded wall-clock cap of {}s",
            wall_clock.as_secs()
        );
        return Err(OpsError::Message(message));
    }
    Ok(())
}

fn run_task_pass(
    worktree: &Path,
    task: &TaskStatement,
    options: &TaskLoopOptions,
) -> OpsResult<()> {
    let mut cmd = lf_command();
    cmd.arg("-b");
    if let Some(max_turns) = options.max_turns {
        cmd.arg("--max-turns").arg(max_turns.to_string());
    }
    cmd.arg(TASK_PASS_FLOW);
    cmd.arg(task.prompt());
    cmd.current_dir(worktree);
    let output = run_with_timeout(cmd, options.pass_timeout)?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("lf -b {TASK_PASS_FLOW}"),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(())
}

fn closed_without_merging(pr: &PrOracle) -> OpsError {
    OpsError::Message(format!("{} was closed without merging", pr.url))
}

fn poll_pr_oracle(worktree: &Path, remembered_pr: Option<u64>) -> OpsResult<Option<PrOracle>> {
    if let Some(number) = remembered_pr {
        return poll_pr_by_number(worktree, number);
    }
    match current_pr(worktree)? {
        Some(pr) => poll_pr_by_number(worktree, pr.number),
        None => Ok(None),
    }
}

fn poll_pr_by_number(worktree: &Path, number: u64) -> OpsResult<Option<PrOracle>> {
    let output = Command::new("gh")
        .arg("pr")
        .arg("view")
        .arg(number.to_string())
        .arg("--json")
        .arg("state,url,number")
        .current_dir(worktree)
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    parse_pr_view_json(&output.stdout).map(Some)
}

#[derive(Debug, Deserialize)]
struct GhPrView {
    state: String,
    url: String,
    number: u64,
}

fn parse_pr_view_json(raw: &[u8]) -> OpsResult<PrOracle> {
    let view: GhPrView = serde_json::from_slice(raw)
        .map_err(|err| OpsError::Parse(format!("failed to parse gh pr view: {err}")))?;
    let state = match view.state.as_str() {
        "MERGED" => PrState::Merged,
        "CLOSED" => PrState::Closed,
        _ => PrState::Open,
    };
    Ok(PrOracle {
        number: view.number,
        url: view.url,
        state,
    })
}

fn close_task(worktree: &Path, wave: &str, task: &TaskStatement, pr: &PrOracle) -> OpsResult<()> {
    pm_update(
        worktree,
        &PmUpdateOptions {
            wave: Some(wave.to_string()),
            id: Some(task.id.clone()),
            title: task.title.clone(),
            notes: Some(task.description.clone()),
            status: Some("done".to_string()),
            pr: Some(pr.url.clone()),
        },
        &NullProgress,
    )?;
    Ok(())
}

fn update_run_pr(
    runtime: &tokio::runtime::Runtime,
    store: &SharedStore,
    run: &mut Run,
    pr: Option<&PrOracle>,
) -> OpsResult<()> {
    let Some(pr) = pr else {
        return Ok(());
    };
    run.pr = Some(PullRequest {
        url: pr.url.clone(),
        number: Some(pr.number as u32),
        state: Some(pr.state.as_str().to_string()),
        title: None,
        branch: Some(run.branch.clone()),
    });
    runtime.block_on(async {
        store
            .update_run(run)
            .await
            .map_err(|err| OpsError::Message(format!("failed to record task PR: {err}")))
    })
}

fn worktree_clean(worktree: &Path) -> OpsResult<bool> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(worktree)
        .output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "git status --porcelain".to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }
    Ok(output.stdout.is_empty())
}

fn run_with_timeout(mut cmd: Command, timeout: Duration) -> OpsResult<std::process::Output> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output().map_err(OpsError::from);
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(OpsError::Message(format!(
                "task pass timed out after {}s",
                timeout.as_secs()
            )));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

fn escalate_parent(message: &str) {
    let _ = lf_command()
        .arg("chat")
        .arg("--parent")
        .arg(message)
        .status();
}

fn lf_command() -> Command {
    if let Ok(path) = std::env::current_exe() {
        return Command::new(path);
    }
    Command::new("lf")
}

#[cfg(test)]
mod tests {
    use std::process::Command;
    use std::time::{Duration, Instant};

    use super::{check_wall_clock, parse_pr_view_json, run_with_timeout, PrOracle, PrState};

    #[test]
    fn pr_oracle_parses_merged() {
        let oracle = parse_pr_view_json(
            br#"{"number":12,"state":"MERGED","url":"https://github.test/pr/12"}"#,
        )
        .expect("oracle");

        assert_eq!(
            oracle,
            PrOracle {
                number: 12,
                url: "https://github.test/pr/12".to_string(),
                state: PrState::Merged,
            }
        );
    }

    #[test]
    fn pr_oracle_parses_open() {
        let oracle =
            parse_pr_view_json(br#"{"number":7,"state":"OPEN","url":"https://github.test/pr/7"}"#)
                .expect("oracle");

        assert_eq!(
            oracle,
            PrOracle {
                number: 7,
                url: "https://github.test/pr/7".to_string(),
                state: PrState::Open,
            }
        );
    }

    #[test]
    fn pr_oracle_parses_closed() {
        let oracle = parse_pr_view_json(
            br#"{"number":8,"state":"CLOSED","url":"https://github.test/pr/8"}"#,
        )
        .expect("oracle");

        assert_eq!(
            oracle,
            PrOracle {
                number: 8,
                url: "https://github.test/pr/8".to_string(),
                state: PrState::Closed,
            }
        );
    }

    #[test]
    fn wall_clock_cap_fires() {
        let started = Instant::now() - Duration::from_secs(5);
        let err = check_wall_clock(started, Duration::from_secs(1)).expect_err("cap");

        assert!(err.to_string().contains("wall-clock cap"));
    }

    #[test]
    fn pass_runner_kills_on_timeout() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 2"]);

        let err = run_with_timeout(cmd, Duration::from_millis(50)).expect_err("timeout");

        assert!(err.to_string().contains("timed out"));
    }
}
