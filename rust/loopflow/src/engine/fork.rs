use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::engine::error::CoreError;
use crate::engine::flow::{ConcreteStep, ForkSelect};
use crate::engine::worktree::remove_worktree;

const FORK_MANIFEST_FILE: &str = ".lf/fork-manifest.json";
type ForkPromptChooser<'a> =
    &'a mut dyn FnMut(&str, &[ConcreteStep]) -> std::result::Result<usize, String>;

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

fn resolve_fork_selection(
    select: &ForkSelect,
    branches: &[ConcreteStep],
    mut prompt_choice: Option<ForkPromptChooser<'_>>,
) -> Result<usize, String> {
    if branches.is_empty() {
        return Err("fork has no branches".to_string());
    }

    match select {
        ForkSelect::One => Ok(0),
        ForkSelect::Prompt { prompt } => {
            let chooser = prompt_choice
                .as_mut()
                .ok_or_else(|| "fork(select=prompt) requires an interactive TTY".to_string())?;
            let selected = chooser(prompt, branches)?;
            if selected >= branches.len() {
                return Err(format!(
                    "fork prompt selected branch {} but only {} branch(es) exist",
                    selected,
                    branches.len()
                ));
            }
            Ok(selected)
        }
        ForkSelect::All => Err("fork(select=all) does not select a single branch".to_string()),
    }
}

pub fn plan_fork_execution(
    select: &ForkSelect,
    branches: &[ConcreteStep],
    base_directions: &[String],
    prompt_choice: Option<ForkPromptChooser<'_>>,
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

    match select {
        ForkSelect::All => (0..branches.len()).map(build_branch).collect(),
        ForkSelect::One | ForkSelect::Prompt { .. } => {
            let index = resolve_fork_selection(select, branches, prompt_choice)?;
            Ok(vec![build_branch(index)?])
        }
    }
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
    use crate::engine::flow::{ConcreteStep, ForkSelect, Step};
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
    fn resolve_fork_selection_one_is_deterministic_first_branch() {
        let branches = vec![branch("a"), branch("b")];
        let selected = resolve_fork_selection(&ForkSelect::One, &branches, None).expect("select");
        assert_eq!(selected, 0);
    }

    #[test]
    fn resolve_fork_selection_prompt_requires_chooser() {
        let branches = vec![branch("a"), branch("b")];
        let err = resolve_fork_selection(
            &ForkSelect::Prompt {
                prompt: "choose".to_string(),
            },
            &branches,
            None,
        )
        .expect_err("missing chooser should fail");
        assert_eq!(err, "fork(select=prompt) requires an interactive TTY");
    }

    #[test]
    fn resolve_fork_selection_prompt_uses_chooser_result() {
        let branches = vec![branch("a"), branch("b"), branch("c")];
        let mut chooser = |_prompt: &str, _branches: &[ConcreteStep]| Ok(2usize);
        let selected = resolve_fork_selection(
            &ForkSelect::Prompt {
                prompt: "choose".to_string(),
            },
            &branches,
            Some(&mut chooser),
        )
        .expect("prompt select");
        assert_eq!(selected, 2);
    }

    #[test]
    fn plan_fork_execution_all_returns_labeled_branches() {
        let branches = vec![branch("a"), branch("b")];
        let base = vec!["base".to_string()];
        let planned =
            plan_fork_execution(&ForkSelect::All, &branches, &base, None).expect("planned");
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
        let err = plan_fork_execution(&ForkSelect::All, &branches, &[], None)
            .expect_err("interactive branch should fail");
        assert_eq!(err, "interactive fork branches are not supported");
    }
}
