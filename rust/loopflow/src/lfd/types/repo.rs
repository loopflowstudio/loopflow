use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

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

    pub(crate) fn from_raw(value: impl Into<String>) -> Self {
        Self(value.into())
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Repo {
    pub path: String,
    pub repo_id: RepoId,
    pub name: String,
    pub added_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoEdge {
    pub parent_repo_id: RepoId,
    pub child_repo_id: RepoId,
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn repo_id_name_raw_without_slash() {
        let id = RepoId::from_raw("bare");
        assert_eq!(id.name(), "bare");
    }
}
