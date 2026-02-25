use crate::engine::fork::{
    fork_worktree_path, plan_fork_execution, ForkManifest, ForkManifestBranch,
    FORK_MANIFEST_RELATIVE_PATH, FORK_SYNTHESIZE_STEP,
};
use crate::engine::git::current_branch;
use crate::engine::worktree::create_worktree;
use crate::engine::{expand_flow, next_action, ConcreteFork, ConcreteItem, Flow, FlowAction};
use crate::lf::output::Colors;
use crate::lf::Cli;
use crate::lfd::executor::{
    cleanup_workspace_worktree, remove_workspace_file, write_workspace_file,
};
use anyhow::{anyhow, Context, Result};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

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
    directions: Vec<String>,
    worktree: PathBuf,
    branch_name: String,
}

fn run_fork(fork: &ConcreteFork, message: Option<&str>, cli: &Cli, repo: &Path) -> Result<()> {
    let planned =
        plan_fork_execution(&fork.branches, &cli.direction).map_err(|err| anyhow!(err))?;

    let base_branch = current_branch(repo)?
        .ok_or_else(|| anyhow!("fork execution requires an active branch (detached HEAD)"))?;
    let mut tasks = Vec::new();
    let mut worktrees = Vec::new();

    for branch in &planned {
        let index = branch.index;
        let worktree = fork_worktree_path(repo, index);
        let branch_name = format!("{base_branch}-fork-{index}");
        if let Err(err) = create_worktree(repo, &worktree, &branch_name).with_context(|| {
            format!(
                "failed to create fork worktree {} for branch {}",
                worktree.display(),
                branch_name
            )
        }) {
            cleanup_fork_worktrees(&worktrees);
            return Err(err);
        }

        worktrees.push(worktree.clone());
        tasks.push(ForkBranchTask {
            index,
            step_name: branch.step.step.name.clone(),
            directions: branch.directions.clone(),
            worktree,
            branch_name,
        });
    }

    let mut handles = Vec::new();
    for task in tasks.iter().cloned() {
        let worktree = task.worktree.clone();
        let step_name = task.step_name.clone();
        let directions = task.directions.clone();
        let branch_label = format!("fork-{}", task.index);
        let msg = message.map(|value| value.to_string());
        let handle = std::thread::spawn(move || {
            run_fork_branch(
                &worktree,
                &step_name,
                &directions,
                msg.as_deref(),
                branch_label.as_str(),
            )
        });
        handles.push((task, handle));
    }

    let mut outcomes = Vec::new();
    for (task, handle) in handles {
        let (exit_code, err) = match handle.join() {
            Ok(Ok(code)) => (code, None),
            Ok(Err(err)) => (1, Some(err.to_string())),
            Err(_) => (1, Some("fork thread panicked".to_string())),
        };

        if exit_code != 0 || err.is_some() {
            if let Some(err) = err {
                eprintln!(
                    "fork branch {} failed ({}): {}",
                    task.index, task.branch_name, err
                );
            } else {
                eprintln!(
                    "fork branch {} failed ({}): exited with {}",
                    task.index, task.branch_name, exit_code
                );
            }
        }

        outcomes.push(ForkManifestBranch {
            index: task.index,
            step: task.step_name.clone(),
            direction: task.directions.join(","),
            worktree: task.worktree.to_string_lossy().to_string(),
            branch: task.branch_name.clone(),
            exit_code,
        });
    }
    let failed = outcomes.iter().filter(|o| o.exit_code != 0).count();

    let manifest = ForkManifest { branches: outcomes };
    let manifest_json =
        serde_json::to_vec_pretty(&manifest).context("failed to encode fork manifest as JSON")?;
    if let Err(err) = write_workspace_file(repo, FORK_MANIFEST_RELATIVE_PATH, &manifest_json) {
        cleanup_fork_worktrees(&worktrees);
        return Err(err);
    }

    let synthesize_result = crate::lf::commands::run::run(Some(FORK_SYNTHESIZE_STEP), message, cli);
    cleanup_fork_artifacts(repo, &worktrees);

    synthesize_result?;

    if failed > 0 {
        return Err(anyhow!("{failed} fork branch(es) failed"));
    }

    Ok(())
}

fn cleanup_fork_artifacts(repo: &Path, worktrees: &[PathBuf]) {
    if let Err(err) = remove_workspace_file(repo, FORK_MANIFEST_RELATIVE_PATH) {
        eprintln!(
            "failed to remove fork manifest {} in {}: {}",
            FORK_MANIFEST_RELATIVE_PATH,
            repo.display(),
            err
        );
    }
    cleanup_fork_worktrees(worktrees);
}

fn cleanup_fork_worktrees(worktrees: &[PathBuf]) {
    for worktree in worktrees {
        if let Err(err) = cleanup_workspace_worktree(worktree) {
            eprintln!(
                "failed to clean up fork worktree {}: {}",
                worktree.display(),
                err
            );
        }
    }
}

fn run_fork_branch(
    worktree: &Path,
    step: &str,
    directions: &[String],
    message: Option<&str>,
    branch_label: &str,
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
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());
    let mut child = cmd.spawn().with_context(|| {
        format!(
            "failed to execute fork branch command in {}",
            worktree.display()
        )
    })?;

    let mut log_threads = Vec::new();
    if let Some(stdout) = child.stdout.take() {
        let label = branch_label.to_string();
        log_threads.push(std::thread::spawn(move || {
            relay_fork_logs(stdout, &label, false);
        }));
    }
    if let Some(stderr) = child.stderr.take() {
        let label = branch_label.to_string();
        log_threads.push(std::thread::spawn(move || {
            relay_fork_logs(stderr, &label, true);
        }));
    }

    let status = child.wait()?;
    for thread in log_threads {
        let _ = thread.join();
    }
    Ok(status.code().unwrap_or(1))
}

fn build_lf_command() -> Command {
    if let Ok(path) = std::env::current_exe() {
        return Command::new(path);
    }
    Command::new("lf")
}

fn relay_fork_logs<R: std::io::Read>(reader: R, branch_label: &str, stderr: bool) {
    let buffered = BufReader::new(reader);
    for line in buffered.lines().map_while(|line| line.ok()) {
        if stderr {
            eprintln!("[{branch_label}] {line}");
        } else {
            println!("[{branch_label}] {line}");
        }
    }
}
