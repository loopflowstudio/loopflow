use std::path::Path;

use crate::engine::git::{fetch, rebase};

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
    Err(OpsError::Message(format!(
        "rebase onto {} failed ({detail})",
        options.onto
    )))
}
