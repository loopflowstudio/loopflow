use std::path::Path;

use crate::engine::git::{fetch, rebase, squash_merge_fork_point, sync_main};

use crate::ops::error::{OpsError, OpsResult};
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
    if let Some(branch) = options.onto.strip_prefix("origin/") {
        let _ = fetch(repo, "origin", branch);
        // Keep local main in sync with origin so downstream reads see current state.
        let _ = sync_main(repo, branch);
    }

    // When a stacked branch's parent was squash-merged into the target,
    // a plain rebase replays the parent's commits (already in target) and
    // hits conflicts. Detect this and use --onto to skip them.
    let fork_point = squash_merge_fork_point(repo, &options.onto).unwrap_or(None);

    progress.status(&format!("Rebasing onto {}...", options.onto));
    let result = rebase(repo, &options.onto, fork_point.as_deref())?;
    if result.success {
        if fork_point.is_some() {
            progress.status("Skipped squash-merged parent commits");
        }
        if options.push {
            crate::ops::commit::push_with_upstream_if_needed(repo)?;
        }
        return Ok(());
    }
    let detail = result
        .conflicts
        .filter(|conflicts| !conflicts.is_empty())
        .map(|conflicts| {
            let conflict_paths = conflicts
                .iter()
                .map(|path| path.display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            format!("conflicts: {conflict_paths}")
        })
        .unwrap_or_else(|| "manual resolution required".to_string());
    Err(OpsError::RebaseConflict {
        onto: options.onto.clone(),
        detail,
    })
}
