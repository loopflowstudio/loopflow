use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::flow::ConcreteStep;

pub const FORK_MANIFEST_RELATIVE_PATH: &str = ".lf/fork-manifest.json";
pub const FORK_SYNTHESIZE_STEP: &str = "synthesize";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForkManifest {
    pub branches: Vec<ForkManifestBranch>,
}

/// A single fork branch result. Used both as the execution outcome and the
/// manifest entry persisted to [`FORK_MANIFEST_RELATIVE_PATH`].
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
pub struct ForkBranchExecutionPlan {
    pub index: usize,
    pub label: String,
    pub step: ConcreteStep,
    pub directions: Vec<String>,
}

/// Compute sibling worktree path for a fork branch.
///
/// Given `/tmp/myrepo` and index 2, returns `/tmp/myrepo-fork-2`.
pub fn fork_worktree_path(repo: &Path, index: usize) -> PathBuf {
    let mut path = repo.as_os_str().to_os_string();
    path.push(format!("-fork-{index}"));
    PathBuf::from(path)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::flow::{ConcreteStep, Step};

    #[test]
    fn fork_worktree_path_uses_dash_suffix() {
        let repo = PathBuf::from("/tmp/loopflow.remote.feature");
        let fork = fork_worktree_path(&repo, 2);
        assert_eq!(fork, PathBuf::from("/tmp/loopflow.remote.feature-fork-2"));
    }

    #[test]
    fn merge_directions_preserves_order_and_deduplicates() {
        let merged = merge_directions(
            &["security".to_string(), "ceo".to_string()],
            &["ceo".to_string(), "ux".to_string()],
        );
        assert_eq!(
            merged,
            vec!["security".to_string(), "ceo".to_string(), "ux".to_string()]
        );
    }

    #[test]
    fn merge_directions_returns_base_when_extra_empty() {
        let base = vec!["security".to_string()];
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
    fn fork_manifest_branch_counts_failures() {
        let branches = [
            ForkManifestBranch {
                index: 0,
                step: "reduce".to_string(),
                direction: "ux".to_string(),
                worktree: "/tmp/repo-fork-0".to_string(),
                branch: "main-fork-0".to_string(),
                exit_code: 0,
            },
            ForkManifestBranch {
                index: 1,
                step: "reduce".to_string(),
                direction: "infra".to_string(),
                worktree: "/tmp/repo-fork-1".to_string(),
                branch: "main-fork-1".to_string(),
                exit_code: 42,
            },
        ];

        let failed = branches.iter().filter(|b| b.exit_code != 0).count();
        assert_eq!(failed, 1);
        assert_eq!(branches.len(), 2);
        assert_eq!(branches[1].exit_code, 42);
    }
}
