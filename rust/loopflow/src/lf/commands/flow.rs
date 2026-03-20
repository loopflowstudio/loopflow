use crate::engine::flow::expand_direction_names;
use crate::engine::flow::{
    build_xor_routing_suffix, load_step, load_xor_path_items, read_xor_verdict,
};
use crate::engine::fork::{
    fork_worktree_path, plan_fork_execution, ForkManifest, ForkManifestBranch, ForkManifestStep,
    FORK_MANIFEST_RELATIVE_PATH, FORK_SYNTHESIZE_STEP,
};
use crate::engine::git::current_branch;
use crate::engine::worktree::create_worktree;
use crate::engine::{
    expand_flow, next_action, ConcreteAnd, ConcreteItem, ConcreteLoop, ConcreteXor, Flow,
    FlowAction,
};
use crate::journal::{self, LfEventFields, LfEventType, LfNode};
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

const TEMP_XOR_ROUTE_STEP_NAME: &str = "xor-route";

/// Run a flow: print pipeline header, then execute each step sequentially.
pub fn run(flow: &Flow, message: Option<&str>, cli: &Cli, repo: &Path) -> Result<()> {
    let items = expand_flow(flow, repo)?;
    print_pipeline_header(&flow.name, &items, repo)?;
    journal::emit(
        repo,
        LfNode::Flow,
        LfEventType::Started,
        LfEventFields {
            flow: Some(flow.name.clone()),
            ..LfEventFields::default()
        },
    );
    let result = run_steps(&items, message, cli, repo);
    match &result {
        Ok(_) => journal::emit(
            repo,
            LfNode::Flow,
            LfEventType::Completed,
            LfEventFields::default(),
        ),
        Err(err) => journal::emit(
            repo,
            LfNode::Flow,
            LfEventType::Errored,
            LfEventFields {
                error: Some(err.to_string()),
                ..LfEventFields::default()
            },
        ),
    }
    result
}

fn print_pipeline_header(flow_name: &str, items: &[ConcreteItem], repo: &Path) -> Result<()> {
    let colors = Colors::new();
    let lines = render_pipeline_lines(items, repo)?;
    let pipeline = lines
        .into_iter()
        .map(|line| {
            format!(
                "  {dim}{line}{reset}",
                dim = colors.dim,
                reset = colors.reset
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    eprintln!(
        "\n{dim}\u{2500}\u{2500} flow {reset}{bold}{name}{reset}\n{pipeline}\n",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        name = flow_name,
        pipeline = pipeline,
    );
    Ok(())
}

fn render_pipeline_lines(items: &[ConcreteItem], repo: &Path) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    for item in items {
        lines.extend(render_pipeline_item(item, repo)?);
    }
    Ok(lines)
}

fn render_pipeline_item(item: &ConcreteItem, repo: &Path) -> Result<Vec<String>> {
    match item {
        ConcreteItem::Step(step) => Ok(vec![step.step.name.clone()]),
        ConcreteItem::Op(ops) => Ok(vec![format!("op: {}", ops.item.display_name())]),
        ConcreteItem::And(and) => {
            let mut lines = vec!["[and]".to_string()];
            for (index, branch) in and.branches.iter().enumerate() {
                let branch_chain = branch
                    .steps
                    .iter()
                    .map(|step| step.step.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" → ");
                let branch_prefix = tree_prefix(index, and.branches.len());
                lines.push(format!("{branch_prefix} {} → {branch_chain}", branch.label));
            }
            Ok(lines)
        }
        ConcreteItem::Xor(branch) => {
            render_branch_item("xor", branch, TEMP_XOR_ROUTE_STEP_NAME, repo)
        }
        ConcreteItem::Or(branch) => render_branch_item("or", branch, "or-route", repo),
        ConcreteItem::Loop(loop_def) => render_loop_pipeline(loop_def, repo),
    }
}

fn render_branch_item(
    kind: &str,
    branch: &ConcreteXor,
    default_router: &str,
    repo: &Path,
) -> Result<Vec<String>> {
    render_branch_pipeline(
        kind,
        branch.router.as_deref().unwrap_or(default_router),
        &branch.paths,
        repo,
    )
}

fn render_branch_pipeline(
    kind: &str,
    router: &str,
    paths: &std::collections::HashMap<String, crate::engine::XorPath>,
    repo: &Path,
) -> Result<Vec<String>> {
    let mut lines = vec![format!("[{kind} via {router}]")];
    let mut keys: Vec<&String> = paths.keys().collect();
    keys.sort();

    for (index, key) in keys.into_iter().enumerate() {
        let path = paths
            .get(key)
            .expect("branch path key collected from map should exist");
        let nested_items = load_xor_path_items(path, repo)?;
        let nested = render_pipeline_lines(&nested_items, repo)?;
        let branch_prefix = tree_prefix(index, paths.len());
        if nested.is_empty() {
            lines.push(format!("{branch_prefix} {key}"));
            continue;
        }

        let nested_chain = nested.join(" → ");
        lines.push(format!("{branch_prefix} {key} → {nested_chain}"));
    }

    Ok(lines)
}

fn render_loop_pipeline(loop_def: &ConcreteLoop, repo: &Path) -> Result<Vec<String>> {
    let mut lines = vec!["loop".to_string()];

    let mut body_lines = Vec::new();
    for item in &loop_def.steps {
        body_lines.extend(render_pipeline_item(item, repo)?);
    }
    lines.extend(prefix_nested_lines(&body_lines));

    let router = loop_def
        .exit
        .router
        .as_deref()
        .unwrap_or(TEMP_XOR_ROUTE_STEP_NAME);
    let mut exit_lines = vec![format!("[exit via {router}]")];
    let mut keys: Vec<&String> = loop_def.exit.paths.keys().collect();
    keys.sort();
    for (index, key) in keys.into_iter().enumerate() {
        let path = loop_def
            .exit
            .paths
            .get(key)
            .expect("loop exit path key collected from map should exist");
        let nested_items = load_xor_path_items(path, repo)?;
        let nested = render_pipeline_lines(&nested_items, repo)?;
        let branch_prefix = tree_prefix(index, loop_def.exit.paths.len());
        if nested.is_empty() {
            let outcome = if key == "done" { "continue" } else { "restart" };
            exit_lines.push(format!("{branch_prefix} {key} → {outcome}"));
            continue;
        }

        let nested_chain = nested.join(" → ");
        exit_lines.push(format!("{branch_prefix} {key} → {nested_chain}"));
    }

    lines.extend(prefix_nested_lines(&exit_lines));
    Ok(lines)
}

fn prefix_nested_lines(lines: &[String]) -> Vec<String> {
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| format!("{} {line}", tree_prefix(index, lines.len())))
        .collect()
}

fn tree_prefix(index: usize, total: usize) -> &'static str {
    if index + 1 == total {
        "└─"
    } else {
        "├─"
    }
}

fn run_steps(items: &[ConcreteItem], message: Option<&str>, cli: &Cli, repo: &Path) -> Result<()> {
    let total = items.len();

    for index in 0..total {
        let action = next_action(items, index);
        match action {
            FlowAction::RunStep { step } => {
                run_flow_step(&step.step.name, (index, total), message, cli, repo, false)?
            }
            FlowAction::WaitInteractive { step } => {
                run_flow_step(&step.step.name, (index, total), message, cli, repo, true)?;
            }
            FlowAction::RunOps { ops } => {
                let colors = Colors::new();
                eprintln!(
                    "{dim}[{current}/{total}]{reset} {bold}op:{reset} {cmd}",
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
            FlowAction::Xor { branch } => {
                run_xor(&branch, message, cli, repo, index)?;
                commit_step_work(repo, "xor")?;
            }
            FlowAction::Or { .. } => {
                anyhow::bail!("or (multi-select) execution is not yet implemented");
            }
            FlowAction::Loop { body } => {
                run_loop(&body, message, cli, repo)?;
                commit_step_work(repo, "loop")?;
            }
            FlowAction::Complete => break,
        }
    }

    Ok(())
}

fn run_flow_step(
    step_name: &str,
    progress: (usize, usize),
    message: Option<&str>,
    cli: &Cli,
    repo: &Path,
    _waiting: bool,
) -> Result<()> {
    let (index, total) = progress;
    print_step_progress(index, total, step_name);
    run_step_with_journal(repo, step_name, index, || {
        crate::lf::commands::run::run(Some(step_name), message, cli)?;
        commit_step_work(repo, step_name)?;
        Ok(())
    })
}

fn print_step_progress(index: usize, total: usize, step_name: &str) {
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
}

fn run_step_with_journal(
    repo: &Path,
    step_name: &str,
    index: usize,
    run: impl FnOnce() -> Result<()>,
) -> Result<()> {
    journal::emit(
        repo,
        LfNode::Step,
        LfEventType::Started,
        LfEventFields {
            step: Some(step_name.to_string()),
            index: Some(index as u32),
            ..LfEventFields::default()
        },
    );
    let result = run();
    match &result {
        Ok(_) => journal::emit(
            repo,
            LfNode::Step,
            LfEventType::Completed,
            LfEventFields {
                step: Some(step_name.to_string()),
                index: Some(index as u32),
                ..LfEventFields::default()
            },
        ),
        Err(err) => journal::emit(
            repo,
            LfNode::Step,
            LfEventType::Errored,
            LfEventFields {
                step: Some(step_name.to_string()),
                index: Some(index as u32),
                error: Some(err.to_string()),
                ..LfEventFields::default()
            },
        ),
    }
    result
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

/// Run xor-routing: execute a routing step, read the verdict, then run the
/// selected sub-flow inline.
fn run_xor(
    xor_def: &ConcreteXor,
    message: Option<&str>,
    cli: &Cli,
    repo: &Path,
    index: usize,
) -> Result<String> {
    let colors = Colors::new();
    let verdict_path = repo.join("scratch/route-xor.md");

    let router_name = xor_def
        .router
        .as_deref()
        .unwrap_or(TEMP_XOR_ROUTE_STEP_NAME);

    eprintln!(
        "{dim}[xor]{reset} {bold}{step}{reset} choosing between {n} paths",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        step = router_name,
        n = xor_def.paths.len(),
    );

    // Build routing instructions that get appended to the step's prompt.
    let routing_suffix = build_xor_routing_suffix(xor_def);

    // Write the routing step — either a wrapper around the router step's
    // content + routing instructions, or a standalone generic routing prompt.
    let prompt = if let Some(ref router_name) = xor_def.router {
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

    let temp_step = write_xor_route_step(repo, &prompt)?;
    let result = run_step_with_journal(repo, router_name, index, || {
        crate::lf::commands::run::run(Some(TEMP_XOR_ROUTE_STEP_NAME), message, cli)?;
        commit_step_work(repo, router_name)?;
        Ok(())
    });
    drop(temp_step);

    result?;

    let selected = read_xor_verdict(&verdict_path, xor_def).map_err(anyhow::Error::msg)?;

    eprintln!(
        "{dim}[xor]{reset} {bold}{selected}{reset} selected",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
    );

    // Load and execute the selected sub-flow.
    let xor_path = xor_def
        .paths
        .get(&selected)
        .expect("selected path validated by read_xor_verdict");

    let sub_items = load_xor_path_items(xor_path, repo)?;
    run_steps(&sub_items, message, cli, repo)?;
    Ok(selected)
}

fn run_loop(loop_def: &ConcreteLoop, message: Option<&str>, cli: &Cli, repo: &Path) -> Result<()> {
    loop {
        run_steps(&loop_def.steps, message, cli, repo)?;
        let selected = run_xor(&loop_def.exit, message, cli, repo, loop_def.steps.len())?;
        if selected == "done" {
            return Ok(());
        }
    }
}

fn write_xor_route_step(repo: &Path, prompt: &str) -> Result<XorRouteStepGuard> {
    let tmp_step_dir = repo.join(".lf/steps");
    std::fs::create_dir_all(&tmp_step_dir)?;
    let path = tmp_step_dir.join(format!("{TEMP_XOR_ROUTE_STEP_NAME}.md"));
    let original_content = std::fs::read_to_string(&path).ok();
    std::fs::write(&path, prompt)?;
    Ok(XorRouteStepGuard {
        path,
        original_content,
    })
}

struct XorRouteStepGuard {
    path: PathBuf,
    original_content: Option<String>,
}

impl Drop for XorRouteStepGuard {
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
                "failed to restore temporary xor-route step {}: {}",
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
    use super::{render_pipeline_lines, write_xor_route_step};
    use crate::engine::{ConcreteItem, Flow};
    use std::fs;
    use tempfile::tempdir;
    use tempfile::TempDir;

    #[test]
    fn write_xor_route_step_removes_temp_file_when_none_existed() {
        let temp = TempDir::new().unwrap();
        let step_path = temp.path().join(".lf/steps/xor-route.md");

        {
            let _guard = write_xor_route_step(temp.path(), "temporary route prompt").unwrap();
            assert_eq!(
                fs::read_to_string(&step_path).unwrap(),
                "temporary route prompt"
            );
        }

        assert!(
            !step_path.exists(),
            "temporary xor-route step should be removed after use"
        );
    }

    #[test]
    fn write_xor_route_step_restores_existing_file() {
        let temp = TempDir::new().unwrap();
        let steps_dir = temp.path().join(".lf/steps");
        fs::create_dir_all(&steps_dir).unwrap();
        let step_path = steps_dir.join("xor-route.md");
        fs::write(&step_path, "existing route prompt").unwrap();

        {
            let _guard = write_xor_route_step(temp.path(), "temporary route prompt").unwrap();
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

    #[test]
    fn render_pipeline_lines_expands_xor_paths_on_separate_lines() {
        let temp = tempdir().unwrap();
        let flows_dir = temp.path().join(".lf/flows/tend");
        fs::create_dir_all(&flows_dir).unwrap();
        fs::write(
            flows_dir.join("tune.yaml"),
            "- tend/play-chord\n- tend/review-chord\n",
        )
        .unwrap();

        let flow = Flow {
            name: "tend".to_string(),
            items: vec![
                crate::engine::flow::FlowItem::Step(crate::engine::flow::Step::named(
                    "tend/scan-waves",
                )),
                crate::engine::flow::FlowItem::Xor(crate::engine::flow::XorDef {
                    router: Some("tend/assess".to_string()),
                    paths: [
                        (
                            "tune".to_string(),
                            crate::engine::flow::XorPath {
                                flow: Some("tend/tune".to_string()),
                                step: None,
                                steps: Vec::new(),
                                description: "Adjust the chord".to_string(),
                                direction: Vec::new(),
                            },
                        ),
                        (
                            "silence".to_string(),
                            crate::engine::flow::XorPath {
                                flow: None,
                                step: None,
                                steps: Vec::new(),
                                description: "No-op".to_string(),
                                direction: Vec::new(),
                            },
                        ),
                    ]
                    .into_iter()
                    .collect(),
                }),
            ],
        };

        let items = crate::engine::expand_flow(&flow, temp.path()).unwrap();
        let lines = render_pipeline_lines(&items, temp.path()).unwrap();

        assert_eq!(
            lines,
            vec![
                "tend/scan-waves".to_string(),
                "[xor via tend/assess]".to_string(),
                "├─ silence".to_string(),
                "└─ tune → tend/play-chord → tend/review-chord".to_string(),
            ]
        );

        assert!(matches!(items[1], ConcreteItem::Xor(_)));
    }
}
