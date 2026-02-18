use crate::engine::fork::{
    cleanup_fork_worktrees, fork_worktree_path, plan_fork_execution, write_fork_manifest,
    ForkManifestBranch,
};
use crate::engine::git::current_branch;
use crate::engine::worktree::create_worktree;
use crate::engine::{
    expand_flow, next_action, ConcreteFork, ConcreteItem, Flow, FlowAction, ForkSelect,
};
use crate::lf::output::Colors;
use crate::lf::Cli;
use anyhow::{anyhow, Context, Result};
use std::io::{BufRead, BufReader, Write};
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
    let mut chooser = |prompt: &str, branches: &[crate::engine::ConcreteStep]| {
        choose_fork_branch(prompt, branches)
    };
    let planned = plan_fork_execution(
        &fork.select,
        &fork.branches,
        &cli.direction,
        if atty::is(atty::Stream::Stdin) {
            Some(&mut chooser)
        } else {
            None
        },
    )
    .map_err(|err| anyhow!(err))?;

    if !matches!(fork.select, ForkSelect::All) {
        let selected = planned
            .first()
            .ok_or_else(|| anyhow!("selected fork branch missing"))?;
        let exit_code = run_fork_branch(
            repo,
            &selected.step.step.name,
            &selected.directions,
            message,
            selected.label.as_str(),
        )?;
        if exit_code != 0 {
            return Err(anyhow!(
                "fork branch {} exited with {}",
                selected.step.step.name,
                exit_code
            ));
        }
        return Ok(());
    }

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
            cleanup_fork_worktrees(None, &worktrees);
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
            } else {
                eprintln!(
                    "fork branch {} failed ({}): exited with {}",
                    task.index, task.branch_name, exit_code
                );
            }
        }

        manifest_branches.push(ForkManifestBranch {
            index: task.index,
            step: task.step_name.clone(),
            direction: task.directions.join(","),
            worktree: task.worktree.to_string_lossy().to_string(),
            branch: task.branch_name.clone(),
            exit_code,
        });
    }

    let manifest_path = match write_fork_manifest(repo, &manifest_branches) {
        Ok(path) => path,
        Err(err) => {
            cleanup_fork_worktrees(None, &worktrees);
            return Err(err.into());
        }
    };
    let synthesize_result = crate::lf::commands::run::run(Some("synthesize"), message, cli);
    cleanup_fork_worktrees(Some(&manifest_path), &worktrees);

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

fn choose_fork_branch(
    prompt: &str,
    branches: &[crate::engine::ConcreteStep],
) -> std::result::Result<usize, String> {
    if !atty::is(atty::Stream::Stdin) {
        return Err("fork(select=prompt) requires an interactive TTY".to_string());
    }

    eprintln!("{prompt}");
    for (index, branch) in branches.iter().enumerate() {
        let dirs = if branch.step.directions.is_empty() {
            String::new()
        } else {
            format!(" [{}]", branch.step.directions.join(","))
        };
        eprintln!("  {}. {}{}", index + 1, branch.step.name, dirs);
    }
    eprint!("Select branch [1-{}]: ", branches.len());
    let _ = std::io::stderr().flush();

    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|err| format!("failed reading fork selection: {err}"))?;
    let raw = line.trim();
    let parsed = raw
        .parse::<usize>()
        .map_err(|_| format!("invalid fork selection '{raw}'"))?;
    if parsed == 0 || parsed > branches.len() {
        return Err(format!(
            "fork selection out of range: {} (expected 1-{})",
            parsed,
            branches.len()
        ));
    }
    Ok(parsed - 1)
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
