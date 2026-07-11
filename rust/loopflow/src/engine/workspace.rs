use std::path::Path;

use anyhow::Result;

pub fn ensure_wave_worktree(main_repo: &Path, wave_name: &str) -> Result<(String, String)> {
    let lease = crate::engine::worktrees::ensure_wave_worktree(main_repo, wave_name)?;
    Ok((lease.path.to_string_lossy().to_string(), lease.branch))
}

pub(crate) fn write_workspace_file(cwd: &Path, relative_path: &str, content: &[u8]) -> Result<()> {
    let path = cwd.join(relative_path);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, content)?;
    Ok(())
}

pub(crate) fn remove_workspace_file(cwd: &Path, relative_path: &str) -> Result<()> {
    let path = cwd.join(relative_path);
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err.into()),
    }
}

pub(crate) fn cleanup_workspace_worktree(worktree: &Path) -> Result<()> {
    if !worktree.exists() {
        return Ok(());
    }

    if worktree.join(".git").exists() {
        crate::engine::worktree::remove_worktree(worktree, true)?;
    } else {
        std::fs::remove_dir_all(worktree)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use loopflow_test_support::TestRepo;

    use super::ensure_wave_worktree;

    fn git(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(repo)
            .output()
            .expect("git command should run");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn ensure_wave_worktree_reuses_diverged_tree_without_rebasing() {
        let repo = TestRepo::new();
        let (worktree, branch) =
            ensure_wave_worktree(repo.path(), "ship").expect("create wave worktree");
        let worktree = Path::new(&worktree);

        std::fs::write(worktree.join("shared.txt"), "wave change\n").expect("write wave file");
        git(worktree, &["add", "."]);
        git(worktree, &["commit", "-m", "wave change"]);

        repo.create_file("shared.txt", "main change\n");
        repo.stage_all();
        repo.commit("main change");

        let (reused, reused_branch) =
            ensure_wave_worktree(repo.path(), "ship").expect("reuse wave worktree");
        assert_eq!(Path::new(&reused), worktree);
        assert_eq!(reused_branch, branch);
        assert_eq!(
            std::fs::read_to_string(worktree.join("shared.txt")).expect("read wave file"),
            "wave change\n"
        );
    }
}
