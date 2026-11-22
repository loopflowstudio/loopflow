use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};

/// The Machine-local identity of a repository checkout.
///
/// Linked worktrees collapse to their main checkout and symlink spellings
/// collapse to one absolute path. Provider repository identity remains
/// the provider repository ID; this type owns local Wave addressing.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(transparent)]
pub struct CanonicalRepo(PathBuf);

#[derive(Debug, thiserror::Error)]
#[error("cannot canonicalize repository {path}: {message}")]
pub struct CanonicalRepoError {
    pub path: PathBuf,
    pub message: String,
}

impl CanonicalRepo {
    /// Construct an address the caller has already canonicalized.
    pub fn from_canonical_path(path: PathBuf) -> Self {
        Self(path)
    }

    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl fmt::Display for CanonicalRepo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.display().fmt(formatter)
    }
}
