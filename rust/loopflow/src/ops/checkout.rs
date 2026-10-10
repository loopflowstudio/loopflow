//! Checkout freshness shared by interactive syncs and unattended installs.

use std::fs::{File, OpenOptions};
use std::path::Path;
use std::process::Command;

use fs2::FileExt;

use crate::git::{
    absolute_git_dir, current_branch, fetch, find_worktree_for_branch, get_default_branch,
    intervention_state, is_ancestor, rev_parse,
};
use crate::ops::git_operation::begin_sync_operation;
use crate::ops::{OpsError, OpsResult, Progress};

fn git(repo: &Path, args: &[&str]) -> OpsResult<String> {
    let output = Command::new("git").args(args).current_dir(repo).output()?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("git {} (in {})", args.join(" "), repo.display()),
            stderr: format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn lock_shared(repo: &Path, name: &str) -> OpsResult<File> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(crate::git::worktrees::git_common_dir(repo)?.join(name))?;
    FileExt::lock_exclusive(&lock)?;
    Ok(lock)
}

// The resolver can write notes while the caller's untracked files are stashed.
// Move collisions aside before apply: a failed apply can already have restored
// tracked edits, so retrying the whole stash afterward is not safe.
fn preserve_untracked_collisions(repo: &Path, stash: &str) -> OpsResult<()> {
    if rev_parse(repo, &format!("{stash}^3")).is_err() {
        return Ok(());
    }
    let paths: Vec<_> = git(
        repo,
        &["ls-tree", "-rz", "--name-only", &format!("{stash}^3")],
    )?
    .split('\0')
    .filter(|path| !path.is_empty())
    .map(|path| repo.join(path))
    .collect();
    for path in &paths {
        let ancestors: Vec<_> = path.ancestors().take_while(|path| *path != repo).collect();
        let Some(collision) = ancestors.into_iter().rev().find(|ancestor| {
            std::fs::symlink_metadata(ancestor)
                .is_ok_and(|metadata| *ancestor == path || !metadata.is_dir())
        }) else {
            continue;
        };
        let relative_collision = collision
            .strip_prefix(repo)
            .expect("path belongs to checkout");
        if !git(
            repo,
            &[
                "ls-files",
                "-z",
                "--",
                &relative_collision.to_string_lossy(),
            ],
        )?
        .is_empty()
        {
            return Err(OpsError::Message(format!(
                "tracked path {} collides with stashed untracked files",
                collision.display()
            )));
        }
        let mut suffix = 1;
        let destination = loop {
            let mut name = collision.as_os_str().to_os_string();
            name.push(format!(".lf-sync-{suffix}"));
            let candidate = std::path::PathBuf::from(name);
            if !paths.iter().any(|path| path.starts_with(&candidate))
                && !candidate.try_exists()?
                && std::fs::symlink_metadata(&candidate).is_err()
            {
                break candidate;
            }
            suffix += 1;
        };
        std::fs::rename(collision, &destination)?;
        eprintln!(
            "Preserved checkout update file at {}",
            destination.display()
        );
    }
    Ok(())
}

/// Restore index, working edits and untracked files even when an update fails.
/// A conflicted operation retains the named stash instead of applying edits
/// into its sequencer. Failures name the durable recovery object.
pub fn with_preserved_edits<T>(repo: &Path, update: impl FnOnce() -> OpsResult<T>) -> OpsResult<T> {
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(absolute_git_dir(repo)?.join("lf-checkout-edits.lock"))?;
    FileExt::try_lock_exclusive(&lock).map_err(|error| {
        OpsError::Message(format!(
            "another checkout update owns {}: {error}",
            repo.display()
        ))
    })?;
    if let Some(state) = intervention_state(repo)? {
        return Err(OpsError::Message(format!(
            "{} has an active {state}; finish or abort it before updating",
            repo.display()
        )));
    }
    if git(repo, &["status", "--porcelain"])?.is_empty() {
        return update();
    }
    // refs/stash belongs to the repository, not to one worktree. Hold its
    // lock only while creating/identifying or removing our recovery entry.
    let stash_lock = lock_shared(repo, "lf-checkout-stash.lock")?;
    git(
        repo,
        &[
            "stash",
            "push",
            "--include-untracked",
            "-m",
            "lf checkout update: caller edits",
        ],
    )?;
    let stash = rev_parse(repo, "refs/stash")?;
    drop(stash_lock);
    let result = update();
    let restore = if intervention_state(repo)?.is_some() {
        Err(OpsError::Message(
            "Git operation needs resolution".to_string(),
        ))
    } else {
        preserve_untracked_collisions(repo, &stash)
            .and_then(|()| git(repo, &["stash", "apply", "--index", &stash]).map(|_| ()))
    };
    if let Err(error) = restore {
        return Err(OpsError::Message(format!(
            "{}; caller edits retained in stash {stash} in {}. After resolving the Git operation, restore with `git stash apply --index {stash}`: {error}",
            result.err().map(|error| error.to_string()).unwrap_or_else(|| "checkout updated".to_string()),
            repo.display()
        )));
    }
    let _stash_lock = lock_shared(repo, "lf-checkout-stash.lock")?;
    let stashes = git(repo, &["stash", "list", "--format=%H"])?;
    if let Some(index) = stashes.lines().position(|sha| sha == stash) {
        git(repo, &["stash", "drop", &format!("stash@{{{index}}}")])?;
    }
    result
}

/// Fetch on every call and preserve unpublished default-branch commits.
/// Divergence is merged, so local commit identities remain available as bases
/// for new worktrees. This operation never publishes.
pub fn refresh_main(repo: &Path, progress: &impl Progress) -> OpsResult<String> {
    // Sibling callers share only this update; their feature sequencers remain
    // independent. Wait for the other refresh, then fetch again for this call.
    let _lock = lock_shared(repo, "lf-main-refresh.lock")?;
    let branch = get_default_branch(repo)?;
    let target = format!("origin/{branch}");
    let checkout = if current_branch(repo)?.as_deref() == Some(&branch) {
        Some(repo.to_path_buf())
    } else {
        find_worktree_for_branch(repo, &branch)?
    };
    let Some(checkout) = checkout else {
        fetch(repo, "origin", &branch)?;
        let upstream = rev_parse(repo, &target)?;
        let local = rev_parse(repo, &branch)?;
        if is_ancestor(repo, &upstream, &local)? {
            return Ok(branch);
        }
        let updated = if is_ancestor(repo, &local, &upstream)? {
            upstream
        } else {
            // Git's in-memory merge leaves the caller's checkout untouched.
            // Conflicts fail before moving any ref; both parent commits remain.
            let tree = git(repo, &["merge-tree", "--write-tree", &local, &upstream])?;
            git(
                repo,
                &[
                    "commit-tree",
                    tree.lines().next().expect("merge-tree returned a tree"),
                    "-p",
                    &local,
                    "-p",
                    &upstream,
                    "-m",
                    &format!("Merge {target} into {branch}"),
                ],
            )?
        };
        git(
            repo,
            &[
                "update-ref",
                &format!("refs/heads/{branch}"),
                &updated,
                &local,
            ],
        )?;
        return Ok(branch);
    };
    let mut operation = begin_sync_operation(&checkout, &target)?;
    fetch(&checkout, "origin", &branch)?;
    let upstream = rev_parse(&checkout, &target)?;
    operation.pin_target(upstream.clone())?;
    if is_ancestor(&checkout, &upstream, "HEAD")? {
        operation.complete()?;
        return Ok(branch);
    }
    progress.status(&format!("Updating {branch} in {}...", checkout.display()));
    let result = with_preserved_edits(&checkout, || {
        let merge = git(
            &checkout,
            &[
                "-c",
                "commit.gpgsign=false",
                "merge",
                "--ff",
                "--no-edit",
                &upstream,
            ],
        );
        if merge.is_err() && intervention_state(&checkout)? == Some("merge") {
            git(&checkout, &["merge", "--abort"])?;
        }
        merge.map_err(|error| OpsError::Message(format!(
            "could not update {branch} in {}; original commits retained. Resolve the upstream/local conflict before retrying `lf sync`: {error}", checkout.display()
        )))?;
        if !is_ancestor(&checkout, &upstream, "HEAD")? {
            return Err(OpsError::Message(format!(
                "{branch} did not reach fetched target {upstream}"
            )));
        }
        Ok(())
    });
    operation.complete()?;
    result?;
    Ok(branch)
}
