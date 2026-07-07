use std::path::Path;
use std::time::{Duration, Instant};

use crate::flowloop::oracle::{poll_kr_set, KrItem, KrSetStatus};
use crate::flowloop::pass::{check_wall_clock, escalate_parent, run_pass, PassOptions};
use crate::flowloop::run::FlowloopRun;
use crate::flowloop::Tier;
use crate::ops::{OpsError, OpsResult};

const DEFAULT_MAX_PASSES: u32 = 12;
const DEFAULT_PASS_TIMEOUT_SECS: u64 = 60 * 30;
const DEFAULT_WALL_CLOCK_SECS: u64 = 60 * 60 * 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectLoopOptions {
    pub wave: Option<String>,
    pub max_passes: u32,
    pub pass_timeout: Duration,
    pub wall_clock: Duration,
    pub max_turns: Option<u32>,
}

impl ProjectLoopOptions {
    pub fn new(wave: Option<String>) -> Self {
        Self {
            wave,
            max_passes: DEFAULT_MAX_PASSES,
            pass_timeout: Duration::from_secs(DEFAULT_PASS_TIMEOUT_SECS),
            wall_clock: Duration::from_secs(DEFAULT_WALL_CLOCK_SECS),
            max_turns: None,
        }
    }
}

pub fn run_project_loop(repo: &Path, options: &ProjectLoopOptions) -> OpsResult<()> {
    let wave_name = crate::ops::util::resolve_wave_name(repo, options.wave.as_deref())
        .ok_or_else(|| OpsError::Message("cannot determine wave name".to_string()))?;

    match poll_kr_set(repo, &wave_name)? {
        KrSetStatus::Empty => return Err(empty_kr_set(&wave_name)),
        KrSetStatus::Done(_) => {
            eprintln!("project flowloop for wave/{wave_name} already complete");
            return Ok(());
        }
        KrSetStatus::Open(_) => {}
    }

    let flowloop = FlowloopRun::start(
        &wave_name,
        Tier::Project,
        format!("Project flowloop for wave/{wave_name}"),
    )?;
    eprintln!(
        "project flowloop for wave/{wave_name} running in {}",
        flowloop.run.worktree
    );
    let result = run_project_passes(&flowloop.worktree(), &wave_name, options);
    flowloop.finish(result)
}

fn run_project_passes(worktree: &Path, wave: &str, options: &ProjectLoopOptions) -> OpsResult<()> {
    let started = Instant::now();
    let mut pass = 0;
    loop {
        if let Err(err) = check_wall_clock(started, options.wall_clock) {
            escalate_parent(&err.to_string());
            return Err(err);
        }

        let status = poll_kr_set(worktree, wave)?;
        match status {
            KrSetStatus::Empty => return Err(empty_kr_set(wave)),
            KrSetStatus::Done(_) => return Ok(()),
            KrSetStatus::Open(items) => {
                if pass >= options.max_passes {
                    break;
                }
                pass += 1;
                eprintln!("project pass {pass}/{}", options.max_passes);
                run_pass(
                    worktree,
                    Tier::Project.pass_flow(),
                    &project_prompt(wave, &items),
                    &PassOptions {
                        timeout: options.pass_timeout,
                        max_turns: options.max_turns,
                    },
                )?;
            }
        }
    }

    let message = format!(
        "project flowloop for wave/{wave} exhausted {} pass(es) with open KRs",
        options.max_passes
    );
    escalate_parent(&message);
    Err(OpsError::Message(message))
}

fn project_prompt(wave: &str, items: &[KrItem]) -> String {
    let mut prompt = format!(
        "Linear KR set for wave/{wave}\n\n\
         Work the project until every kr-labeled roadmap item is completed. \
         If a KR is stale, clarify or renew it in Linear; the runtime only \
         stops when the KR oracle reports all done.\n"
    );
    for item in items {
        let marker = if item.completed { "x" } else { " " };
        prompt.push_str(&format!("\n- [{marker}] {} {}", item.id, item.name));
    }
    prompt
}

fn empty_kr_set(wave: &str) -> OpsError {
    OpsError::Message(format!(
        "project flowloop for wave/{wave} needs at least one kr-labeled roadmap item"
    ))
}

#[cfg(test)]
mod tests {
    use super::project_prompt;
    use crate::flowloop::oracle::KrItem;

    #[test]
    fn project_prompt_lists_kr_set() {
        let prompt = project_prompt(
            "goals",
            &[
                KrItem {
                    id: "KR-1".to_string(),
                    name: "Done".to_string(),
                    completed: true,
                },
                KrItem {
                    id: "KR-2".to_string(),
                    name: "Open".to_string(),
                    completed: false,
                },
            ],
        );

        assert!(prompt.contains("Linear KR set for wave/goals"));
        assert!(prompt.contains("- [x] KR-1 Done"));
        assert!(prompt.contains("- [ ] KR-2 Open"));
    }
}
