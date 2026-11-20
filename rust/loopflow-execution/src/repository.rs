use std::fmt;
use std::path::Path;
use std::process::Command;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

pub mod location;

pub use loopflow_storage::address::{CanonicalRepo, CanonicalRepoError};

pub fn discover(path: &Path) -> Result<CanonicalRepo, CanonicalRepoError> {
    let main = crate::git::worktrees::main_repo_root(path).unwrap_or_else(|_| path.to_path_buf());
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
    Ok(CanonicalRepo::from_canonical_path(canonical))
}

pub fn current() -> anyhow::Result<Option<CanonicalRepo>> {
    containing(&std::env::current_dir()?)
}
pub fn containing(directory: &Path) -> anyhow::Result<Option<CanonicalRepo>> {
    crate::repo::discover_repo_root(directory)?
        .map(|root| discover(&root).map_err(Into::into))
        .transpose()
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

        let scope_a = crate::repository::discover(repo_a.path()).expect("discover A");
        assert_eq!(
            crate::repository::discover(&worktree).expect("discover worktree"),
            scope_a,
            "worktree should resolve to its main checkout"
        );
        assert_eq!(crate::repository::discover(&worktree).unwrap(), scope_a);

        // A distinct repository is a different scope.
        assert_ne!(crate::repository::discover(repo_b.path()).unwrap(), scope_a);

        // A path outside any checkout is not this repo.
        assert!(crate::repository::discover(Path::new("/nonexistent/path")).is_err());
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
