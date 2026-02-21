use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::error::CoreError;
use crate::engine::flow::ConcreteStep;
use crate::engine::worktree::remove_worktree;

const FORK_MANIFEST_FILE: &str = ".lf/fork-manifest.json";
pub const FORK_SYNTHESIZE_STEP: &str = "synthesize";

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkBranchOutcome {
    pub index: usize,
    pub step: String,
    pub direction: String,
    pub worktree: String,
    pub branch: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkBranchExecutionPlan {
    pub index: usize,
    pub label: String,
    pub step: ConcreteStep,
    pub directions: Vec<String>,
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

pub fn plan_fork_execution(
    branches: &[ConcreteStep],
    base_directions: &[String],
) -> Result<Vec<ForkBranchExecutionPlan>, String> {
    if branches.is_empty() {
        return Err("fork has no branches".to_string());
    }

    let build_branch = |index: usize| -> Result<ForkBranchExecutionPlan, String> {
        let step = branches
            .get(index)
            .cloned()
            .ok_or_else(|| format!("fork selected branch {index}, but it does not exist"))?;
        if step.step.interactive.unwrap_or(false) {
            return Err("interactive fork branches are not supported".to_string());
        }
        Ok(ForkBranchExecutionPlan {
            index,
            label: format!("fork-{index}"),
            directions: merge_directions(base_directions, &step.step.directions),
            step,
        })
    };

    (0..branches.len()).map(build_branch).collect()
}

pub fn summarize_fork_outcomes(outcomes: &[ForkBranchOutcome]) -> (Vec<ForkManifestBranch>, usize) {
    let failed = outcomes
        .iter()
        .filter(|outcome| outcome.exit_code != 0)
        .count();
    let branches = outcomes
        .iter()
        .map(|outcome| ForkManifestBranch {
            index: outcome.index,
            step: outcome.step.clone(),
            direction: outcome.direction.clone(),
            worktree: outcome.worktree.clone(),
            branch: outcome.branch.clone(),
            exit_code: outcome.exit_code,
        })
        .collect();
    (branches, failed)
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
    use crate::engine::flow::{ConcreteStep, Step};
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

    fn branch(name: &str) -> ConcreteStep {
        ConcreteStep {
            step: Step::named(name),
            flow_parents: Vec::new(),
        }
    }

    #[test]
    fn plan_fork_execution_returns_labeled_branches() {
        let branches = vec![branch("a"), branch("b")];
        let base = vec!["base".to_string()];
        let planned = plan_fork_execution(&branches, &base).expect("planned");
        assert_eq!(planned.len(), 2);
        assert_eq!(planned[0].label, "fork-0");
        assert_eq!(planned[0].directions, vec!["base".to_string()]);
    }

    #[test]
    fn plan_fork_execution_rejects_interactive_branch() {
        let mut step = Step::named("a");
        step.interactive = Some(true);
        let branches = vec![ConcreteStep {
            step,
            flow_parents: Vec::new(),
        }];
        let err = plan_fork_execution(&branches, &[]).expect_err("interactive branch should fail");
        assert_eq!(err, "interactive fork branches are not supported");
    }

    #[test]
    fn plan_fork_execution_rejects_empty_branches() {
        let err = plan_fork_execution(&[], &[]).expect_err("empty branches should fail");
        assert_eq!(err, "fork has no branches");
    }

    #[test]
    fn summarize_fork_outcomes_builds_manifest_and_counts_failures() {
        let outcomes = vec![
            ForkBranchOutcome {
                index: 0,
                step: "reduce".to_string(),
                direction: "designer".to_string(),
                worktree: "/tmp/repo.fork-0".to_string(),
                branch: "main-fork-0".to_string(),
                exit_code: 0,
            },
            ForkBranchOutcome {
                index: 1,
                step: "reduce".to_string(),
                direction: "infra".to_string(),
                worktree: "/tmp/repo.fork-1".to_string(),
                branch: "main-fork-1".to_string(),
                exit_code: 42,
            },
        ];

        let (manifest, failed) = summarize_fork_outcomes(&outcomes);
        assert_eq!(failed, 1);
        assert_eq!(manifest.len(), 2);
        assert_eq!(manifest[1].exit_code, 42);
    }
}
