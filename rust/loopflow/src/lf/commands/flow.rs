use crate::engine::flow::expand_direction_names;
use crate::engine::flow::{
    build_or_routing_suffix, load_or_path_items, load_step, read_or_verdict,
};
use crate::engine::fork::{
    fork_worktree_path, plan_fork_execution, ForkManifest, ForkManifestBranch, ForkManifestStep,
    FORK_MANIFEST_RELATIVE_PATH, FORK_SYNTHESIZE_STEP,
};
use crate::engine::git::current_branch;
use crate::engine::worktree::create_worktree;
use crate::engine::{
    expand_flow, next_action, ConcreteAnd, ConcreteItem, ConcreteOr, Flow, FlowAction,
};
use crate::lf::output::Colors;
use crate::lf::Cli;
use crate::lfd::executor::{
    cleanup_workspace_worktree, remove_workspace_file, write_workspace_file,
};
use crate::ops::{commit_workflow, CommitOptions, NullProgress};
use anyhow::{anyhow, Context, Result};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const TEMP_OR_ROUTE_STEP_NAME: &str = "or-route";

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
            ConcreteItem::Op(_) => "[op]",
            ConcreteItem::And(_) => "[and]",
            ConcreteItem::Or(_) => "[or]",
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
                let step_name = step.step.name.clone();
                let colors = Colors::new();
                eprintln!(
                    "{dim}[{current}/{total}]{reset} {bold}{name}{reset}",
                    dim = colors.dim,
                    reset = colors.reset,
                    bold = colors.bold,
                    current = index + 1,
                    total = total,
                    name = step_name,
                );
                crate::lf::commands::run::run(Some(&step_name), message, cli)?;
                commit_step_work(repo, &step_name)?;
            }
            FlowAction::RunOps { ops } => {
                let colors = Colors::new();
                eprintln!(
                    "{dim}[{current}/{total}]{reset} {bold}ops:{reset} {cmd}",
                    dim = colors.dim,
                    reset = colors.reset,
                    bold = colors.bold,
                    current = index + 1,
                    total = total,
                    cmd = ops.item.display_name(),
                );
                crate::ops::execute_flow_ops(repo, &ops.item, &NullProgress)?;
            }
            FlowAction::And { fork } => {
                run_and(&fork, message, cli, repo)?;
                commit_step_work(repo, "and")?;
            }
            FlowAction::Or { branch } => {
                run_or(&branch, message, cli, repo)?;
                commit_step_work(repo, "or")?;
            }
            FlowAction::Complete => break,
        }
    }

    Ok(())
}

/// Commit any uncommitted changes left by the previous step.
fn commit_step_work(repo: &Path, step_name: &str) -> Result<()> {
    let options = CommitOptions {
        add: true,
        message: Some(format!("lf commit: {step_name}")),
        ..CommitOptions::for_task(step_name)
    };
    commit_workflow(repo, &options, &NullProgress)?;
    Ok(())
}

/// Run or-routing: execute a routing step, read the verdict, then run the
/// selected sub-flow inline.
fn run_or(or_def: &ConcreteOr, message: Option<&str>, cli: &Cli, repo: &Path) -> Result<()> {
    let colors = Colors::new();
    let verdict_path = repo.join("scratch/route-or.md");

    let router_name = or_def.router.as_deref().unwrap_or(TEMP_OR_ROUTE_STEP_NAME);

    eprintln!(
        "{dim}[or]{reset} {bold}{step}{reset} choosing between {n} paths",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        step = router_name,
        n = or_def.paths.len(),
    );

    // Build routing instructions that get appended to the step's prompt.
    let routing_suffix = build_or_routing_suffix(or_def);

    // Write the routing step — either a wrapper around the router step's
    // content + routing instructions, or a standalone generic routing prompt.
    let prompt = if let Some(ref router_name) = or_def.router {
        let router = load_step(router_name, repo)?;
        let base = router.content.as_deref().unwrap_or("");
        format!("{base}\n\n{routing_suffix}")
    } else {
        format!(
            "---\nagent: claude:sonnet\n---\n\
             Previous steps have analyzed the current state and written their findings to scratch/.\n\
             Read scratch/ to understand what's been decided, then choose the right path forward.\n\n\
             {routing_suffix}"
        )
    };

    let temp_step = write_or_route_step(repo, &prompt)?;
    let result = crate::lf::commands::run::run(Some(TEMP_OR_ROUTE_STEP_NAME), message, cli);
    drop(temp_step);

    result?;
    commit_step_work(repo, router_name)?;

    let selected = read_or_verdict(&verdict_path, or_def).map_err(anyhow::Error::msg)?;

    eprintln!(
        "{dim}[or]{reset} {bold}{selected}{reset} selected",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
    );

    // Load and execute the selected sub-flow.
    let or_path = or_def
        .paths
        .get(&selected)
        .expect("selected path validated by read_or_verdict");

    let sub_items = load_or_path_items(or_path, repo)?;

    for sub_item in &sub_items {
        let sub_step = match sub_item {
            ConcreteItem::Step(step) => step,
            ConcreteItem::Op(ops) => {
                crate::ops::execute_flow_ops(repo, &ops.item, &NullProgress)?;
                continue;
            }
            _ => continue,
        };

        let step_name = sub_step.step.name.clone();
        eprintln!(
            "{dim}[or/{selected}]{reset} {bold}{step_name}{reset}",
            dim = colors.dim,
            reset = colors.reset,
            bold = colors.bold,
        );
        crate::lf::commands::run::run(Some(&step_name), message, cli)?;
        commit_step_work(repo, &step_name)?;
    }

    Ok(())
}

fn write_or_route_step(repo: &Path, prompt: &str) -> Result<OrRouteStepGuard> {
    let tmp_step_dir = repo.join(".lf/steps");
    std::fs::create_dir_all(&tmp_step_dir)?;
    let path = tmp_step_dir.join(format!("{TEMP_OR_ROUTE_STEP_NAME}.md"));
    let original_content = std::fs::read_to_string(&path).ok();
    std::fs::write(&path, prompt)?;
    Ok(OrRouteStepGuard {
        path,
        original_content,
    })
}

struct OrRouteStepGuard {
    path: PathBuf,
    original_content: Option<String>,
}

impl Drop for OrRouteStepGuard {
    fn drop(&mut self) {
        let result = match &self.original_content {
            Some(content) => std::fs::write(&self.path, content),
            None => match std::fs::remove_file(&self.path) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(err) => Err(err),
            },
        };

        if let Err(err) = result {
            eprintln!(
                "failed to restore temporary or-route step {}: {}",
                self.path.display(),
                err
            );
        }
    }
}

#[derive(Debug, Clone)]
struct ForkBranchTask {
    index: usize,
    step_names: Vec<String>,
    directions: Vec<String>,
    worktree: PathBuf,
    branch_name: String,
}

fn run_and(fork: &ConcreteAnd, message: Option<&str>, cli: &Cli, repo: &Path) -> Result<()> {
    let expanded_cli_directions = expand_direction_names(&cli.direction, repo);
    let planned = plan_fork_execution(&fork.branches, &expanded_cli_directions)
        .map_err(|err| anyhow!(err))?;

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
            step_names: branch.steps.iter().map(|s| s.step.name.clone()).collect(),
            directions: branch.directions.clone(),
            worktree,
            branch_name,
        });
    }

    let mut handles = Vec::new();
    for task in tasks.iter().cloned() {
        let worktree = task.worktree.clone();
        let step_names = task.step_names.clone();
        let directions = task.directions.clone();
        let branch_label = format!("fork-{}", task.index);
        let msg = message.map(|value| value.to_string());
        let handle = std::thread::spawn(move || {
            run_fork_branch_steps(
                &worktree,
                &step_names,
                &directions,
                msg.as_deref(),
                &branch_label,
            )
        });
        handles.push((task, handle));
    }

    let mut outcomes = Vec::new();
    for (task, handle) in handles {
        let (exit_code, step_results, err) = match handle.join() {
            Ok(Ok((code, results))) => (code, results, None),
            Ok(Err(err)) => (1, Vec::new(), Some(err.to_string())),
            Err(_) => (1, Vec::new(), Some("fork thread panicked".to_string())),
        };

        if exit_code != 0 || err.is_some() {
            let failed_step = step_results
                .iter()
                .rev()
                .find(|s| s.exit_code != 0)
                .map(|s| s.name.as_str());
            if let Some(err) = err {
                eprintln!(
                    "fork branch {} failed ({}): {}",
                    task.index, task.branch_name, err
                );
            } else if let Some(step_name) = failed_step {
                eprintln!(
                    "fork branch {} failed ({}) at step '{}': exited with {}",
                    task.index, task.branch_name, step_name, exit_code
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
            steps: step_results,
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

/// Run steps sequentially within a fork branch. Returns the first non-zero
/// exit code (fail-fast) or 0, along with per-step outcomes.
fn run_fork_branch_steps(
    worktree: &Path,
    step_names: &[String],
    directions: &[String],
    message: Option<&str>,
    branch_label: &str,
) -> Result<(i32, Vec<ForkManifestStep>)> {
    let mut step_results = Vec::new();
    for step_name in step_names {
        let exit_code =
            run_fork_branch_step(worktree, step_name, directions, message, branch_label)?;
        step_results.push(ForkManifestStep {
            name: step_name.clone(),
            exit_code,
        });
        if exit_code != 0 {
            return Ok((exit_code, step_results));
        }
    }
    Ok((0, step_results))
}

fn run_fork_branch_step(
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

#[cfg(test)]
mod tests {
    use super::write_or_route_step;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn write_or_route_step_removes_temp_file_when_none_existed() {
        let temp = TempDir::new().unwrap();
        let step_path = temp.path().join(".lf/steps/or-route.md");

        {
            let _guard = write_or_route_step(temp.path(), "temporary route prompt").unwrap();
            assert_eq!(
                fs::read_to_string(&step_path).unwrap(),
                "temporary route prompt"
            );
        }

        assert!(
            !step_path.exists(),
            "temporary or-route step should be removed after use"
        );
    }

    #[test]
    fn write_or_route_step_restores_existing_file() {
        let temp = TempDir::new().unwrap();
        let steps_dir = temp.path().join(".lf/steps");
        fs::create_dir_all(&steps_dir).unwrap();
        let step_path = steps_dir.join("or-route.md");
        fs::write(&step_path, "existing route prompt").unwrap();

        {
            let _guard = write_or_route_step(temp.path(), "temporary route prompt").unwrap();
            assert_eq!(
                fs::read_to_string(&step_path).unwrap(),
                "temporary route prompt"
            );
        }

        assert_eq!(
            fs::read_to_string(&step_path).unwrap(),
            "existing route prompt"
        );
    }
}
