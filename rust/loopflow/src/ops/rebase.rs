use std::path::Path;
use std::time::Duration;

use crate::engine::git::{fetch, rebase};

use crate::ops::agent::{run_builtin_agent, BuiltinAgentOptions};
use crate::ops::error::OpsResult;
use crate::ops::progress::Progress;

#[derive(Debug, Clone)]
pub struct RebaseOptions {
    pub onto: String,
    pub push: bool,
}

pub fn rebase_with_recovery(
    repo: &Path,
    options: &RebaseOptions,
    progress: &impl Progress,
) -> OpsResult<()> {
    if options.onto.starts_with("origin/") {
        if let Some(branch) = options.onto.strip_prefix("origin/") {
            let _ = fetch(repo, "origin", branch);
        }
    }

    // Try auto-rebase first. rebase() aborts on conflict and returns the conflict list.
    progress.status(&format!("Rebasing onto {}...", options.onto));
    let result = rebase(repo, &options.onto, None)?;
    if result.success {
        if options.push {
            crate::ops::commit::push_with_upstream_or_error(repo)?;
        }
        return Ok(());
    }

    // Auto-rebase failed (and was aborted). Launch the rebase agent to handle
    // the full workflow: re-attempt the rebase, resolve conflicts, continue.
    progress.status("Auto-rebase failed, launching rebase agent...");
    run_rebase_agent(repo, &options.onto, progress)?;

    if options.push {
        crate::ops::commit::push_with_upstream_or_error(repo)?;
    }
    Ok(())
}

fn run_rebase_agent(repo: &Path, onto: &str, progress: &impl Progress) -> OpsResult<()> {
    let options = BuiltinAgentOptions {
        step_name: "rebase".to_string(),
        suffix: format!("Rebase onto: {onto}"),
        timeout: Some(Duration::from_secs(30 * 60)),
    };
    run_builtin_agent(repo, &options, progress)
}
