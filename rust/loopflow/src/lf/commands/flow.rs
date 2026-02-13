use crate::engine::git::current_branch;
use crate::engine::worktree::{create_worktree, remove_worktree};
use crate::engine::{
    expand_flow, next_action, ConcreteFork, ConcreteItem, Flow, FlowAction, ForkSelect,
};
use crate::lf::output::Colors;
use crate::lf::Cli;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

const FORK_MANIFEST_FILE: &str = ".lf/fork-manifest.json";

/// Run a flow: print pipeline header, then execute each step sequentially.
pub fn run(flow: &Flow, message: Option<&str>, cli: &Cli, repo: &Path) -> Result<()> {
    let items = expand_flow(flow, repo)?;
    print_pipeline_header(&flow.name, &items);
    run_steps(&items, message, cli, repo)
}

fn print_pipeline_header(flow_name: &str, items: &[ConcreteItem]) {
    let colors = Colors::new();
    let step_names: Vec<&str> = items
        .iter()
        .map(|item| match item {
            ConcreteItem::Step(s) => s.step.name.as_str(),
            ConcreteItem::Fork(_) => "[fork]",
        })
        .collect();

    let pipeline = step_names.join(&format!(
        " {dim}\u{2192}{reset} ",
        dim = colors.dim,
        reset = colors.reset,
    ));

    eprintln!(
        "\n{dim}\u{2500}\u{2500} flow {reset}{bold}{name}{reset} {dim}{pipeline}{reset}\n",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        name = flow_name,
        pipeline = pipeline,
    );
}

fn run_steps(items: &[ConcreteItem], message: Option<&str>, cli: &Cli, repo: &Path) -> Result<()> {
    let total = items.len();

    for index in 0..total {
        let action = next_action(items, index);
        match action {
            FlowAction::RunStep { step } | FlowAction::WaitInteractive { step } => {
                let colors = Colors::new();
                eprintln!(
                    "{dim}[{current}/{total}]{reset} {bold}{name}{reset}",
                    dim = colors.dim,
                    reset = colors.reset,
                    bold = colors.bold,
                    current = index + 1,
                    total = total,
                    name = step.step.name,
                );
                crate::lf::commands::run::run(Some(&step.step.name), message, cli)?;
            }
            FlowAction::Fork { fork } => {
                run_fork(&fork, message, cli, repo)?;
            }
            FlowAction::Complete => break,
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct ForkBranchTask {
    index: usize,
    step_name: String,
    step_directions: Vec<String>,
    cli_directions: Vec<String>,
    worktree: PathBuf,
    branch_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ForkManifest {
    branches: Vec<ForkManifestBranch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct ForkManifestBranch {
    index: usize,
    step: String,
    direction: String,
    worktree: String,
    branch: String,
    exit_code: i32,
}

fn run_fork(fork: &ConcreteFork, message: Option<&str>, cli: &Cli, repo: &Path) -> Result<()> {
    if !matches!(fork.select, ForkSelect::All) {
        return Err(anyhow!(
            "CLI fork execution only supports select: all (got {:?})",
            fork.select
        ));
    }

    if fork.branches.is_empty() {
        return Ok(());
    }

    let base_branch = current_branch(repo)?
        .ok_or_else(|| anyhow!("fork execution requires an active branch (detached HEAD)"))?;
    let mut tasks = Vec::new();

    for (index, branch) in fork.branches.iter().enumerate() {
        let worktree = fork_worktree_path(repo, index);
        let branch_name = format!("{base_branch}-fork-{index}");
        if let Err(err) = create_worktree(repo, &worktree, &branch_name).with_context(|| {
            format!(
                "failed to create fork worktree {} for branch {}",
                worktree.display(),
                branch_name
            )
        }) {
            cleanup_fork_artifacts(None, &tasks);
            return Err(err);
        }

        tasks.push(ForkBranchTask {
            index,
            step_name: branch.step.name.clone(),
            step_directions: branch.step.directions.clone(),
            cli_directions: cli.direction.clone(),
            worktree,
            branch_name,
        });
    }

    let mut handles = Vec::new();
    for task in tasks.iter().cloned() {
        let worktree = task.worktree.clone();
        let step_name = task.step_name.clone();
        let directions = merge_directions(&task.cli_directions, &task.step_directions);
        let msg = message.map(|value| value.to_string());
        let handle = std::thread::spawn(move || {
            run_fork_branch(&worktree, &step_name, &directions, msg.as_deref())
        });
        handles.push((task, handle));
    }

    let mut manifest_branches = Vec::new();
    let mut failed = 0usize;
    for (task, handle) in handles {
        let (exit_code, err) = match handle.join() {
            Ok(Ok(code)) => (code, None),
            Ok(Err(err)) => (1, Some(err.to_string())),
            Err(_) => (1, Some("fork thread panicked".to_string())),
        };

        if exit_code != 0 || err.is_some() {
            failed += 1;
            if let Some(err) = err {
                eprintln!(
                    "fork branch {} failed ({}): {}",
                    task.index, task.branch_name, err
                );
            }
        }

        manifest_branches.push(ForkManifestBranch {
            index: task.index,
            step: task.step_name.clone(),
            direction: task.step_directions.join(","),
            worktree: task.worktree.to_string_lossy().to_string(),
            branch: task.branch_name.clone(),
            exit_code,
        });
    }

    let manifest_path = match write_fork_manifest(repo, &manifest_branches) {
        Ok(path) => path,
        Err(err) => {
            cleanup_fork_artifacts(None, &tasks);
            return Err(err);
        }
    };
    let synthesize_result = run_synthesize(fork.synthesize.as_deref(), message, cli);
    cleanup_fork_artifacts(Some(&manifest_path), &tasks);

    synthesize_result?;

    if failed > 0 {
        return Err(anyhow!("{failed} fork branch(es) failed"));
    }

    Ok(())
}

fn run_fork_branch(
    worktree: &Path,
    step: &str,
    directions: &[String],
    message: Option<&str>,
) -> Result<i32> {
    let mut cmd = build_lf_command();
    cmd.arg(step).arg("-b");
    for direction in directions {
        cmd.arg("-d").arg(direction);
    }
    if let Some(message) = message {
        cmd.arg(message);
    }
    cmd.current_dir(worktree);
    let status = cmd.status().with_context(|| {
        format!(
            "failed to execute fork branch command in {}",
            worktree.display()
        )
    })?;
    Ok(status.code().unwrap_or(1))
}

fn build_lf_command() -> Command {
    if let Ok(path) = std::env::current_exe() {
        return Command::new(path);
    }
    Command::new("lf")
}

fn write_fork_manifest(repo: &Path, branches: &[ForkManifestBranch]) -> Result<PathBuf> {
    let manifest_path = repo.join(FORK_MANIFEST_FILE);
    if let Some(parent) = manifest_path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let manifest = ForkManifest {
        branches: branches.to_vec(),
    };
    let json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(&manifest_path, json)?;
    Ok(manifest_path)
}

fn run_synthesize(step_name: Option<&str>, message: Option<&str>, cli: &Cli) -> Result<()> {
    if let Some(step_name) = step_name {
        crate::lf::commands::run::run(Some(step_name), message, cli)?;
    }
    Ok(())
}

fn cleanup_fork_artifacts(manifest_path: Option<&Path>, tasks: &[ForkBranchTask]) {
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

    for task in tasks {
        if let Err(err) = remove_worktree(&task.worktree, true) {
            eprintln!(
                "failed to clean up fork worktree {}: {}",
                task.worktree.display(),
                err
            );
        }
    }
}

fn merge_directions(primary: &[String], secondary: &[String]) -> Vec<String> {
    let mut merged = primary.to_vec();
    merged.extend(secondary.iter().cloned());
    merged
}

fn fork_worktree_path(repo: &Path, index: usize) -> PathBuf {
    let name = repo
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .into_owned();
    repo.parent()
        .unwrap_or(repo)
        .join(format!("{name}.fork-{index}"))
}

#[cfg(test)]
mod tests {
    use super::{fork_worktree_path, write_fork_manifest, ForkManifest, ForkManifestBranch};
    use std::path::PathBuf;
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
}
