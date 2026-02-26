use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::flow::{ConcreteForkBranch, ConcreteStep};

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
    pub steps: Vec<ForkManifestStep>,
    pub direction: String,
    pub worktree: String,
    pub branch: String,
    pub exit_code: i32,
}

/// Per-step outcome within a fork branch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ForkManifestStep {
    pub name: String,
    pub exit_code: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForkBranchExecutionPlan {
    pub index: usize,
    pub label: String,
    pub steps: Vec<ConcreteStep>,
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
    branches: &[ConcreteForkBranch],
    base_directions: &[String],
) -> Result<Vec<ForkBranchExecutionPlan>, String> {
    if branches.is_empty() {
        return Err("fork has no branches".to_string());
    }

    let build_branch = |index: usize| -> Result<ForkBranchExecutionPlan, String> {
        let branch = branches
            .get(index)
            .ok_or_else(|| format!("fork selected branch {index}, but it does not exist"))?;
        for step in &branch.steps {
            if step.step.interactive.unwrap_or(false) {
                return Err("interactive fork branches are not supported".to_string());
            }
        }
        Ok(ForkBranchExecutionPlan {
            index,
            label: format!("fork-{index}"),
            directions: merge_directions(base_directions, &branch.directions),
            steps: branch.steps.clone(),
        })
    };

    (0..branches.len()).map(build_branch).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::flow::{ConcreteForkBranch, ConcreteStep, Step};

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

    fn single_step_branch(name: &str) -> ConcreteForkBranch {
        ConcreteForkBranch {
            steps: vec![ConcreteStep {
                step: Step::named(name),
                flow_parents: Vec::new(),
            }],
            flow_parents: Vec::new(),
            label: name.to_string(),
            directions: Vec::new(),
        }
    }

    fn multi_step_branch(names: &[&str], directions: Vec<String>) -> ConcreteForkBranch {
        ConcreteForkBranch {
            steps: names
                .iter()
                .map(|name| ConcreteStep {
                    step: Step::named(name),
                    flow_parents: Vec::new(),
                })
                .collect(),
            flow_parents: Vec::new(),
            label: names.first().unwrap_or(&"branch").to_string(),
            directions,
        }
    }

    #[test]
    fn plan_fork_execution_returns_labeled_branches() {
        let branches = vec![single_step_branch("a"), single_step_branch("b")];
        let base = vec!["base".to_string()];
        let planned = plan_fork_execution(&branches, &base).expect("planned");
        assert_eq!(planned.len(), 2);
        assert_eq!(planned[0].label, "fork-0");
        assert_eq!(planned[0].directions, vec!["base".to_string()]);
        assert_eq!(planned[0].steps.len(), 1);
        assert_eq!(planned[0].steps[0].step.name, "a");
    }

    #[test]
    fn plan_fork_execution_multi_step_branches() {
        let branches = vec![
            multi_step_branch(&["impl", "compress", "gate"], vec!["infra".to_string()]),
            multi_step_branch(&["impl", "compress", "gate"], vec!["ux".to_string()]),
        ];
        let base = vec!["base".to_string()];
        let planned = plan_fork_execution(&branches, &base).expect("planned");
        assert_eq!(planned.len(), 2);
        assert_eq!(planned[0].steps.len(), 3);
        assert_eq!(planned[0].directions, vec!["base", "infra"]);
        assert_eq!(planned[1].directions, vec!["base", "ux"]);
    }

    #[test]
    fn plan_fork_execution_rejects_interactive_branch() {
        let mut step = Step::named("a");
        step.interactive = Some(true);
        let branches = vec![ConcreteForkBranch {
            steps: vec![ConcreteStep {
                step,
                flow_parents: Vec::new(),
            }],
            flow_parents: Vec::new(),
            label: "a".to_string(),
            directions: Vec::new(),
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
                steps: vec![ForkManifestStep {
                    name: "reduce".to_string(),
                    exit_code: 0,
                }],
                direction: "ux".to_string(),
                worktree: "/tmp/repo-fork-0".to_string(),
                branch: "main-fork-0".to_string(),
                exit_code: 0,
            },
            ForkManifestBranch {
                index: 1,
                steps: vec![ForkManifestStep {
                    name: "reduce".to_string(),
                    exit_code: 42,
                }],
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

    #[test]
    fn fork_manifest_serde_roundtrip() {
        let manifest = ForkManifest {
            branches: vec![ForkManifestBranch {
                index: 0,
                steps: vec![
                    ForkManifestStep {
                        name: "implement".to_string(),
                        exit_code: 0,
                    },
                    ForkManifestStep {
                        name: "compress".to_string(),
                        exit_code: 1,
                    },
                ],
                direction: "infra".to_string(),
                worktree: "/tmp/repo-fork-0".to_string(),
                branch: "run-1-fork-0".to_string(),
                exit_code: 1,
            }],
        };
        let json = serde_json::to_string_pretty(&manifest).unwrap();
        let parsed: ForkManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest, parsed);
        assert_eq!(parsed.branches[0].steps.len(), 2);
        assert_eq!(parsed.branches[0].steps[1].name, "compress");
        assert_eq!(parsed.branches[0].steps[1].exit_code, 1);
    }
}
