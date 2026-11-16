//! Explicit delivery cleanup; ordinary launches and Git operations keep scratch.
use std::path::Path;

use crate::ops::error::OpsResult;

/// Clear committed additions only. Checkpoint intended notes before calling this.
pub fn clear(repo: &Path) -> OpsResult<usize> {
    let base = match crate::ops::task::task_diff_base(repo)? {
        Some(base) => base,
        None => crate::git::scratch::base(repo)?,
    };
    let paths = crate::git::scratch::added(repo, &base, "HEAD")?;
    if paths.is_empty() {
        return Ok(0);
    }
    let mut args = vec!["--literal-pathspecs", "rm", "-f", "--"];
    args.extend(paths.iter().map(String::as_str));
    crate::git::git_stdout(repo, &args)?;
    crate::ops::commit_selected(repo, &paths, Some("Clear added scratch files"))?;
    Ok(paths.len())
}
