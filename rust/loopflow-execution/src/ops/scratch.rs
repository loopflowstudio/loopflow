//! Explicit delivery cleanup; ordinary launches and Git operations keep scratch.
use std::path::Path;

use crate::ops::error::{OpsError, OpsResult};

/// Clear committed additions relative to the Task's recorded base (keeping a
/// live stack parent's notes out of the child's diff), else the integration
/// merge-base. Checkpoint intended notes before calling this.
pub fn clear(repo: &Path) -> OpsResult<usize> {
    let base = match crate::ops::task::task_diff_base(repo)? {
        Some(base) => base,
        None => crate::git::scratch::base(repo)?,
    };
    clear_against(repo, &base)
}

/// Clear committed additions against delivery's integration base and refuse
/// when clearing leaves no authored changes.
pub(crate) fn clear_for_delivery(repo: &Path, base: &str) -> OpsResult<usize> {
    let cleared = clear_against(repo, base)?;
    if crate::git::rev_parse(repo, "HEAD^{tree}")?
        == crate::git::rev_parse(repo, &format!("{base}^{{tree}}"))?
    {
        return Err(OpsError::Message(
            "no authored changes remain after integration; refusing to publish an empty PR"
                .to_string(),
        ));
    }
    Ok(cleared)
}

fn clear_against(repo: &Path, base: &str) -> OpsResult<usize> {
    let paths = crate::git::scratch::added(repo, base, "HEAD")?;
    if paths.is_empty() {
        return Ok(0);
    }
    crate::git::scratch::remove(repo, &paths)?;
    crate::ops::commit_selected(repo, &paths, Some("Clear added scratch files"))?;
    Ok(paths.len())
}
