use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::error::CoreError;
use crate::engine::worktree::remove_worktree;

const FORK_MANIFEST_FILE: &str = ".lf/fork-manifest.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForkManifest {
    pub branches: Vec<ForkManifestBranch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForkManifestBranch {
    pub index: usize,
    pub step: String,
    pub direction: String,
    pub worktree: String,
    pub branch: String,
    pub exit_code: i32,
}

/// Compute sibling worktree path for a fork branch.
///
/// Given `/tmp/myrepo` and index 2, returns `/tmp/myrepo.fork-2`.
pub fn fork_worktree_path(repo: &Path, index: usize) -> PathBuf {
    let name = repo
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    repo.parent()
        .unwrap_or(repo)
        .join(format!("{name}.fork-{index}"))
}

/// Merge base directions with extra directions, preserving order and deduplicating.
pub fn merge_directions(base: &[String], extra: &[String]) -> Vec<String> {
    if extra.is_empty() {
        return base.to_vec();
    }
    let mut combined = base.to_vec();
    for direction in extra {
        if !combined.contains(direction) {
            combined.push(direction.clone());
        }
    }
    combined
}

/// Write fork manifest to `.lf/fork-manifest.json` in the repo root.
pub fn write_fork_manifest(
    repo: &Path,
    branches: &[ForkManifestBranch],
) -> Result<PathBuf, CoreError> {
    let manifest_path = repo.join(FORK_MANIFEST_FILE);
    if let Some(parent) = manifest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let manifest = ForkManifest {
        branches: branches.to_vec(),
    };
    let json =
        serde_json::to_string_pretty(&manifest).map_err(|e| CoreError::IoError(e.to_string()))?;
    std::fs::write(&manifest_path, json)?;
    Ok(manifest_path)
}

/// Remove fork worktrees and optionally the manifest file.
pub fn cleanup_fork_worktrees(manifest_path: Option<&Path>, worktrees: &[PathBuf]) {
    if let Some(manifest_path) = manifest_path {
        if let Err(err) = std::fs::remove_file(manifest_path) {
            if err.kind() != ErrorKind::NotFound {
                eprintln!(
                    "failed to remove fork manifest {}: {}",
                    manifest_path.display(),
                    err
                );
            }
        }
    }

    std::thread::scope(|s| {
        for worktree in worktrees {
            s.spawn(|| {
                if let Err(err) = remove_worktree(worktree, true) {
                    eprintln!(
                        "failed to clean up fork worktree {}: {}",
                        worktree.display(),
                        err
                    );
                }
            });
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn fork_worktree_path_uses_dotted_sibling_convention() {
        let repo = PathBuf::from("/tmp/loopflow.remote.feature");
        let fork = fork_worktree_path(&repo, 2);
        assert_eq!(fork, PathBuf::from("/tmp/loopflow.remote.feature.fork-2"));
    }

    #[test]
    fn write_fork_manifest_persists_branch_results() {
        let tmp = tempdir().expect("tempdir");
        let branches = vec![ForkManifestBranch {
            index: 1,
            step: "reduce".to_string(),
            direction: "designer".to_string(),
            worktree: "/tmp/repo.fork-1".to_string(),
            branch: "feature-fork-1".to_string(),
            exit_code: 0,
        }];

        let path = write_fork_manifest(tmp.path(), &branches).expect("write manifest");
        let raw = std::fs::read_to_string(path).expect("read manifest");
        let manifest: ForkManifest = serde_json::from_str(&raw).expect("parse manifest");

        assert_eq!(manifest.branches, branches);
    }

    #[test]
    fn merge_directions_preserves_order_and_deduplicates() {
        let merged = merge_directions(
            &["designer".to_string(), "ceo".to_string()],
            &["ceo".to_string(), "product-engineer".to_string()],
        );
        assert_eq!(
            merged,
            vec![
                "designer".to_string(),
                "ceo".to_string(),
                "product-engineer".to_string()
            ]
        );
    }

    #[test]
    fn merge_directions_returns_base_when_extra_empty() {
        let base = vec!["designer".to_string()];
        let merged = merge_directions(&base, &[]);
        assert_eq!(merged, base);
    }
}
