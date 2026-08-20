use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// The Home-local identity of a repository checkout.
///
/// Linked worktrees collapse to their main checkout and symlink spellings
/// collapse to one absolute path. Provider repository identity remains
/// [`RepoId`]; this type owns local Wave addressing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(transparent)]
pub struct CanonicalRepo(PathBuf);

#[derive(Debug, thiserror::Error)]
#[error("cannot canonicalize repository {path}: {message}")]
pub struct CanonicalRepoError {
    path: PathBuf,
    message: String,
}

impl CanonicalRepo {
    pub fn discover(path: &Path) -> Result<Self, CanonicalRepoError> {
        let main =
            crate::engine::worktrees::main_repo_root(path).unwrap_or_else(|_| path.to_path_buf());
        let canonical = main.canonicalize().map_err(|error| CanonicalRepoError {
            path: main,
            message: error.to_string(),
        })?;
        if !canonical.is_dir() {
            return Err(CanonicalRepoError {
                path: canonical,
                message: "repository root is not a directory".to_string(),
            });
        }
        Ok(Self(canonical))
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }

    /// The repository scope of the current working directory, or `None` when the
    /// cwd is outside any git checkout (nothing to scope to).
    pub fn current() -> Option<Self> {
        Self::discover(&std::env::current_dir().ok()?).ok()
    }

    /// Whether `path` resolves to this same repository, collapsing worktrees to
    /// their main checkout. A path that no longer exists on disk is not "this
    /// repo" and returns `false`.
    pub fn contains(&self, path: &Path) -> bool {
        Self::discover(path)
            .map(|other| &other == self)
            .unwrap_or(false)
    }
}

impl fmt::Display for CanonicalRepo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.display().fmt(formatter)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RepoId(String);

#[derive(Debug, thiserror::Error)]
#[error("invalid repo id: {0}")]
pub struct RepoIdError(String);

impl RepoId {
    pub fn parse(value: &str) -> Result<Self, RepoIdError> {
        let trimmed = value.trim();
        let Some((owner, repo)) = trimmed.split_once('/') else {
            return Err(RepoIdError("expected owner/repo".to_string()));
        };

        if owner.is_empty() || repo.is_empty() || repo.contains('/') {
            return Err(RepoIdError("expected owner/repo".to_string()));
        }

        Ok(Self(trimmed.to_string()))
    }

    pub fn from_owner_repo(owner: &str, repo: &str) -> Result<Self, RepoIdError> {
        Self::parse(&format!("{owner}/{repo}"))
    }

    pub fn from_remote_url(value: &str) -> Result<Self, RepoIdError> {
        let value = value.trim().trim_end_matches('/').trim_end_matches(".git");
        let path = match value.rsplit_once(':') {
            Some((scheme, path)) if !scheme.contains('/') => path,
            _ => value,
        };
        let parts = path
            .split('/')
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>();
        let [.., owner, repo] = parts.as_slice() else {
            return Err(RepoIdError("expected a remote owner/repo URL".to_string()));
        };
        Self::from_owner_repo(owner, repo)
    }

    pub fn discover(path: &Path) -> Result<Self, RepoIdError> {
        let output = Command::new("git")
            .args(["-C", &path.to_string_lossy(), "remote", "get-url", "origin"])
            .output()
            .map_err(|error| RepoIdError(format!("read origin remote: {error}")))?;
        if !output.status.success() {
            return Err(RepoIdError("git origin remote is unavailable".to_string()));
        }
        let remote = String::from_utf8(output.stdout)
            .map_err(|error| RepoIdError(format!("origin remote is not UTF-8: {error}")))?;
        Self::from_remote_url(&remote)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the repo name portion (after the `/`).
    pub fn name(&self) -> &str {
        self.0
            .split_once('/')
            .map(|(_, name)| name)
            .unwrap_or(&self.0)
    }
}

impl fmt::Display for RepoId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for RepoId {
    type Err = RepoIdError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::parse(value)
    }
}

impl From<RepoId> for String {
    fn from(repo_id: RepoId) -> Self {
        repo_id.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    fn git(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(args)
            .output()
            .expect("run git");
        assert!(output.status.success(), "git {args:?} failed");
    }

    fn init_repo() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("temp repo");
        git(dir.path(), &["init", "-b", "main"]);
        git(dir.path(), &["config", "user.name", "tester"]);
        git(dir.path(), &["config", "user.email", "t@example.com"]);
        std::fs::write(dir.path().join("README.md"), "base").expect("seed file");
        git(dir.path(), &["add", "README.md"]);
        git(dir.path(), &["commit", "-m", "base"]);
        dir
    }

    #[test]
    fn canonical_repo_collapses_worktree_and_separates_distinct_repo() {
        let repo_a = init_repo();
        let repo_b = init_repo();

        // A linked worktree of A collapses to A's main checkout.
        let wt_parent = tempfile::tempdir().expect("worktree parent");
        let worktree = wt_parent.path().join("a.child");
        git(
            repo_a.path(),
            &["worktree", "add", "-b", "child", worktree.to_str().unwrap()],
        );

        let scope_a = CanonicalRepo::discover(repo_a.path()).expect("discover A");
        assert_eq!(
            CanonicalRepo::discover(&worktree).expect("discover worktree"),
            scope_a,
            "worktree should resolve to its main checkout"
        );
        assert!(scope_a.contains(&worktree));

        // A distinct repository is a different scope.
        assert!(!scope_a.contains(repo_b.path()));

        // A path outside any checkout is not this repo.
        assert!(!scope_a.contains(Path::new("/nonexistent/path")));
    }

    #[test]
    fn repo_id_name_returns_repo_portion() {
        let id = RepoId::parse("loopflowstudio/loopflow").unwrap();
        assert_eq!(id.name(), "loopflow");
    }

    #[test]
    fn repo_id_name_with_different_owner() {
        let id = RepoId::parse("acme/widgets").unwrap();
        assert_eq!(id.name(), "widgets");
    }

    #[test]
    fn repo_id_parses_common_remote_urls() {
        for remote in [
            "git@github.com:loopflowstudio/loopflow.git",
            "ssh://git@github.com/loopflowstudio/loopflow.git",
            "https://github.com/loopflowstudio/loopflow.git",
        ] {
            assert_eq!(
                RepoId::from_remote_url(remote).unwrap().as_str(),
                "loopflowstudio/loopflow"
            );
        }
    }
}
