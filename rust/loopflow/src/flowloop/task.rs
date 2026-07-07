use std::path::Path;
use std::thread;
use std::time::{Duration, Instant};

use crate::flowloop::oracle::{poll_pr_oracle, worktree_clean, PrOracle, PrState};
use crate::flowloop::pass::{check_wall_clock, escalate_parent, run_pass, PassOptions};
use crate::flowloop::run::FlowloopRun;
use crate::flowloop::Tier;
use crate::lfd::types::{PullRequest, Run};
use crate::lfdb::SharedStore;
use crate::ops::pm::{pm_complete, pm_show, PmCompleteOptions, PmShowOptions};
use crate::ops::{NullProgress, OpsError, OpsResult};

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

pub fn run_task_loop(repo: &Path, options: &TaskLoopOptions) -> OpsResult<()> {
    let wave_name = crate::ops::util::resolve_wave_name(repo, options.wave.as_deref())
        .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?;
    let task = resolve_task_statement(repo, &wave_name, &options.item_id)?;
    let mut flowloop = FlowloopRun::start(&wave_name, Tier::Task, task.prompt())?;
    eprintln!("task {} running in {}", task.id, flowloop.run.worktree);

    let worktree = flowloop.worktree();
    let runtime = &flowloop.runtime;
    let store = &flowloop.store;
    let run = &mut flowloop.run;
    let result = run_task_passes(&worktree, &wave_name, &task, options, |pr| {
        update_run_pr(runtime, store, run, pr)
    });

    flowloop.finish(result)
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
    let mut pass = 0;
    loop {
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

        if pass >= options.max_passes {
            break;
        }
        pass += 1;
        eprintln!("task pass {pass}/{}", options.max_passes);
        run_pass(
            worktree,
            Tier::Task.pass_flow(),
            &task.prompt(),
            &PassOptions {
                timeout: options.pass_timeout,
                max_turns: options.max_turns,
            },
        )?;

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

fn closed_without_merging(pr: &PrOracle) -> OpsError {
    OpsError::Message(format!("{} was closed without merging", pr.url))
}

fn close_task(worktree: &Path, wave: &str, task: &TaskStatement, pr: &PrOracle) -> OpsResult<()> {
    pm_complete(
        worktree,
        &PmCompleteOptions {
            wave: Some(wave.to_string()),
            id: task.id.clone(),
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
