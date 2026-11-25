//! Explicit delivery cleanup; ordinary launches and Git operations keep scratch.
use std::path::Path;

use crate::ops::error::OpsResult;

/// Default comparison base for `lf scratch clear`: the Task's recorded base
/// (keeping a live stack parent's notes out of the child's diff), else the
/// integration merge-base.
pub fn default_base(repo: &Path) -> OpsResult<String> {
    match crate::ops::task::task_diff_base(repo)? {
        Some(base) => Ok(base),
        None => Ok(crate::git::scratch::base(repo)?),
    }
}

/// Clear committed additions relative to `base`. Checkpoint intended notes
/// before calling this.
pub fn clear(repo: &Path, base: &str) -> OpsResult<usize> {
    let paths = crate::git::scratch::added(repo, base, "HEAD")?;
    if paths.is_empty() {
        return Ok(0);
    }
    let mut args = vec!["--literal-pathspecs", "rm", "-f", "--"];
    args.extend(paths.iter().map(String::as_str));
    crate::git::git_stdout(repo, &args)?;
    crate::ops::commit_selected(repo, &paths, Some("Clear added scratch files"))?;
    Ok(paths.len())
}
