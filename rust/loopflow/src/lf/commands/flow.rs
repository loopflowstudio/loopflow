use crate::engine::flow::expand_direction_names;
use crate::engine::flow::load_xor_path_items;
use crate::engine::fork::{
    fork_worktree_path, plan_fork_execution, ForkManifest, ForkManifestBranch, ForkManifestSkill,
    FORK_MANIFEST_RELATIVE_PATH, FORK_SYNTHESIZE_STEP,
};
use crate::engine::git::current_branch;
use crate::engine::worktree::create_worktree;
use crate::engine::{
    expand_flow, xor_verdict_path, ConcreteAnd, ConcreteLoop, ConcreteStep, ConcreteXor,
    ExecutionContext, ExecutionSkill, Flow, FlowEngine, FlowOutcome, FlowProgress, SkillExecutor,
    SkillOutcome, TEMP_XOR_ROUTE_STEP_NAME,
};
use crate::journal::{self, LfEventFields, LfEventType, LfNode};
use crate::lf::output::Colors;
use crate::lf::Cli;
use crate::lfd::executor::{
    cleanup_workspace_worktree, remove_workspace_file, write_workspace_file,
};
use crate::ops::{commit_workflow, CommitOptions, NullProgress};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Run a flow: print pipeline header, then execute each skill sequentially.
pub fn run(flow: &Flow, message: Option<&str>, cli: &Cli, repo: &Path) -> Result<()> {
    let items = expand_flow(flow, repo)?;
    print_pipeline_header(&flow.name, &items, repo)?;
    execute(&flow.name, &items, None, message, cli, repo)
}

/// Run exactly one expanded top-level step. The resident owns the cursor and
/// invokes this hidden primitive once per body, so a session boundary maps to
/// a product step instead of an entire flow.
pub fn run_step(flow: &str, index: usize, message: &str, cli: &Cli, repo: &Path) -> Result<()> {
    let definition = crate::engine::load_flow(flow, repo)?;
    let items = expand_flow(&definition, repo)?;
    let item = items
        .get(index)
        .cloned()
        .ok_or_else(|| anyhow!("flow '{flow}' has no step at index {index}"))?;
    execute(
        &definition.name,
        std::slice::from_ref(&item),
        Some(index as u32),
        Some(message),
        cli,
        repo,
    )
}

/// Execute expanded steps on a fresh runtime, bracketed by flow journal events.
/// `index` names the single step when the caller is running one body's worth.
fn execute(
    flow_name: &str,
    items: &[ConcreteStep],
    index: Option<u32>,
    message: Option<&str>,
    cli: &Cli,
    repo: &Path,
) -> Result<()> {
    let fields = |extra: LfEventFields| LfEventFields {
        flow: Some(flow_name.to_string()),
        index,
        ..extra
    };
    journal::emit(
        repo,
        LfNode::Flow,
        LfEventType::Started,
        fields(LfEventFields::default()),
    );
    let _flow_env = EnvVarGuard::set("LOOPFLOW_FLOW_NAME", flow_name);
    let executor = CliFlowExecutor {
        cli,
        message,
        repo: repo.to_path_buf(),
    };
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("failed to build flow runtime")?;
    let result = runtime
        .block_on(FlowEngine::new(executor).run(items, 0))
        .map(|outcome| match outcome {
            FlowOutcome::Completed | FlowOutcome::Waiting => (),
        });
    match &result {
        Ok(()) => journal::emit(
            repo,
            LfNode::Flow,
            LfEventType::Completed,
            fields(LfEventFields::default()),
        ),
        Err(err) => journal::emit(
            repo,
            LfNode::Flow,
            LfEventType::Errored,
            fields(LfEventFields {
                error: Some(err.to_string()),
                ..LfEventFields::default()
            }),
        ),
    }
    result
}

fn print_pipeline_header(flow_name: &str, items: &[ConcreteStep], repo: &Path) -> Result<()> {
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

fn render_pipeline_lines(items: &[ConcreteStep], repo: &Path) -> Result<Vec<String>> {
    let mut lines = Vec::new();
    for item in items {
        lines.extend(render_pipeline_item(item, repo)?);
    }
    Ok(lines)
}

fn render_pipeline_item(item: &ConcreteStep, repo: &Path) -> Result<Vec<String>> {
    match item {
        ConcreteStep::Skill(skill) => Ok(vec![skill.skill.name.clone()]),
        ConcreteStep::Op(ops) => Ok(vec![format!("op: {}", ops.item.display_name())]),
        ConcreteStep::And(and) => {
            let mut lines = vec!["[and]".to_string()];
            let total_lines = and.branches.len() + 1;
            for (index, branch) in and.branches.iter().enumerate() {
                let branch_chain = branch
                    .steps
                    .iter()
                    .map(|skill| skill.skill.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" → ");
                let branch_prefix = tree_prefix(index, total_lines);
                lines.push(format!("{branch_prefix} {} → {branch_chain}", branch.label));
            }
            let synth_skill = and.synthesize.as_deref().unwrap_or(FORK_SYNTHESIZE_STEP);
            let synth_prefix = tree_prefix(and.branches.len(), total_lines);
            lines.push(format!("{synth_prefix} synthesize → {synth_skill}"));
            Ok(lines)
        }
        ConcreteStep::Xor(branch) => {
            render_branch_item("xor", branch, TEMP_XOR_ROUTE_STEP_NAME, repo)
        }
        ConcreteStep::Or(branch) => render_branch_item("or", branch, "or-route", repo),
        ConcreteStep::Loop(loop_def) => render_loop_pipeline(loop_def, repo),
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

struct EnvVarGuard {
    key: &'static str,
    previous: Option<std::ffi::OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let previous = std::env::var_os(key);
        std::env::set_var(key, value);
        Self { key, previous }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        if let Some(previous) = &self.previous {
            std::env::set_var(self.key, previous);
        } else {
            std::env::remove_var(self.key);
        }
    }
}

struct CliFlowExecutor<'a> {
    cli: &'a Cli,
    message: Option<&'a str>,
    repo: PathBuf,
}

#[async_trait]
impl SkillExecutor for CliFlowExecutor<'_> {
    fn repo_root(&self) -> &Path {
        &self.repo
    }

    async fn run_skill(
        &self,
        skill: &ExecutionSkill,
        ctx: ExecutionContext,
    ) -> Result<SkillOutcome> {
        if let Some(progress) = ctx.progress {
            print_skill_progress(progress, &skill.display_name);
        } else {
            print_nested_skill_progress(&skill.display_name);
        }

        run_skill_with_journal(
            &self.repo,
            &skill.display_name,
            ctx.progress.map(|progress| progress.index),
            || {
                if let Some(prompt) = skill.temporary_content.as_deref() {
                    let _guard = write_temp_skill(&self.repo, &skill.invoke_as, prompt)?;
                    crate::lf::commands::run::run(
                        Some(skill.invoke_as.as_str()),
                        self.message,
                        self.cli,
                    )?;
                } else {
                    crate::lf::commands::run::run(
                        Some(skill.invoke_as.as_str()),
                        self.message,
                        self.cli,
                    )?;
                }
                commit_skill_work(&self.repo, &skill.display_name)?;
                Ok(())
            },
        )?;
        Ok(SkillOutcome::Completed)
    }

    async fn run_op(&self, ops: &crate::engine::ConcreteOp, ctx: ExecutionContext) -> Result<()> {
        if let Some(progress) = ctx.progress {
            let colors = Colors::new();
            eprintln!(
                "{dim}[{current}/{total}]{reset} {bold}op:{reset} {cmd}",
                dim = colors.dim,
                reset = colors.reset,
                bold = colors.bold,
                current = progress.index + 1,
                total = progress.total,
                cmd = ops.item.display_name(),
            );
        } else {
            eprintln!("op: {}", ops.item.display_name());
        }
        crate::ops::execute_flow_ops(&self.repo, &ops.item, &NullProgress)?;
        Ok(())
    }

    async fn run_and(&self, fork: &ConcreteAnd, _ctx: ExecutionContext) -> Result<()> {
        run_and(fork, self.message, self.cli, &self.repo)?;
        commit_skill_work(&self.repo, "and")?;
        Ok(())
    }

    async fn read_xor_verdict(&self, branch: &crate::engine::ConcreteXor) -> Result<String> {
        crate::engine::flow::read_xor_verdict(&xor_verdict_path(&self.repo), branch)
            .map_err(anyhow::Error::msg)
    }
}

fn print_skill_progress(progress: FlowProgress, skill_name: &str) {
    let colors = Colors::new();
    eprintln!(
        "{dim}[{current}/{total}]{reset} {bold}{name}{reset}",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        current = progress.index + 1,
        total = progress.total,
        name = skill_name,
    );
}

fn print_nested_skill_progress(skill_name: &str) {
    let colors = Colors::new();
    eprintln!(
        "{dim}[*]{reset} {bold}{name}{reset}",
        dim = colors.dim,
        reset = colors.reset,
        bold = colors.bold,
        name = skill_name,
    );
}

fn run_skill_with_journal(
    repo: &Path,
    skill_name: &str,
    index: Option<usize>,
    run: impl FnOnce() -> Result<()>,
) -> Result<()> {
    journal::emit(
        repo,
        LfNode::Skill,
        LfEventType::Started,
        LfEventFields {
            skill: Some(skill_name.to_string()),
            index: index.map(|value| value as u32),
            ..LfEventFields::default()
        },
    );
    let result = run();
    match &result {
        Ok(_) => journal::emit(
            repo,
            LfNode::Skill,
            LfEventType::Completed,
            LfEventFields {
                skill: Some(skill_name.to_string()),
                index: index.map(|value| value as u32),
                ..LfEventFields::default()
            },
        ),
        Err(err) => journal::emit(
            repo,
            LfNode::Skill,
            LfEventType::Errored,
            LfEventFields {
                skill: Some(skill_name.to_string()),
                index: index.map(|value| value as u32),
                error: Some(err.to_string()),
                ..LfEventFields::default()
            },
        ),
    }
    result
}

/// Commit any uncommitted changes left by the previous skill.
pub(crate) fn commit_skill_work(repo: &Path, skill_name: &str) -> Result<()> {
    let options = CommitOptions {
        add: true,
        message: Some(format!("lf commit: {skill_name}")),
        ..CommitOptions::for_task(skill_name)
    };
    commit_workflow(repo, &options, &NullProgress)?;
    Ok(())
}
fn write_temp_skill(repo: &Path, name: &str, prompt: &str) -> Result<TempSkillGuard> {
    let tmp_skill_dir = repo.join(".lf/skills");
    std::fs::create_dir_all(&tmp_skill_dir)?;
    let path = tmp_skill_dir.join(format!("{name}.md"));
    let original_content = std::fs::read_to_string(&path).ok();
    std::fs::write(&path, prompt)?;
    Ok(TempSkillGuard {
        path,
        original_content,
    })
}

struct TempSkillGuard {
    path: PathBuf,
    original_content: Option<String>,
}

impl Drop for TempSkillGuard {
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
                "failed to restore temporary skill {}: {}",
                self.path.display(),
                err
            );
        }
    }
}

#[derive(Debug, Clone)]
struct ForkBranchTask {
    index: usize,
    skill_names: Vec<String>,
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
            skill_names: branch.steps.iter().map(|s| s.skill.name.clone()).collect(),
            directions: branch.directions.clone(),
            worktree,
            branch_name,
        });
    }

    let mut handles = Vec::new();
    for task in tasks.iter().cloned() {
        let worktree = task.worktree.clone();
        let skill_names = task.skill_names.clone();
        let directions = task.directions.clone();
        let branch_label = format!("fork-{}", task.index);
        let msg = message.map(|value| value.to_string());
        let handle = std::thread::spawn(move || {
            run_fork_branch_skills(
                &worktree,
                &skill_names,
                &directions,
                msg.as_deref(),
                &branch_label,
            )
        });
        handles.push((task, handle));
    }

    let mut outcomes = Vec::new();
    for (task, handle) in handles {
        let (exit_code, skill_results, err) = match handle.join() {
            Ok(Ok((code, results))) => (code, results, None),
            Ok(Err(err)) => (1, Vec::new(), Some(err.to_string())),
            Err(_) => (1, Vec::new(), Some("fork thread panicked".to_string())),
        };

        if exit_code != 0 || err.is_some() {
            let failed_skill = skill_results
                .iter()
                .rev()
                .find(|s| s.exit_code != 0)
                .map(|s| s.name.as_str());
            if let Some(err) = err {
                eprintln!(
                    "fork branch {} failed ({}): {}",
                    task.index, task.branch_name, err
                );
            } else if let Some(skill_name) = failed_skill {
                eprintln!(
                    "fork branch {} failed ({}) at skill '{}': exited with {}",
                    task.index, task.branch_name, skill_name, exit_code
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
            skills: skill_results,
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

    let synth_skill = fork.synthesize.as_deref().unwrap_or(FORK_SYNTHESIZE_STEP);
    let synthesize_result = crate::lf::commands::run::run(Some(synth_skill), message, cli);
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

/// Run skills sequentially within a fork branch. Returns the first non-zero
/// exit code (fail-fast) or 0, along with per-skill outcomes.
fn run_fork_branch_skills(
    worktree: &Path,
    skill_names: &[String],
    directions: &[String],
    message: Option<&str>,
    branch_label: &str,
) -> Result<(i32, Vec<ForkManifestSkill>)> {
    let mut skill_results = Vec::new();
    for skill_name in skill_names {
        let exit_code =
            run_fork_branch_skill(worktree, skill_name, directions, message, branch_label)?;
        skill_results.push(ForkManifestSkill {
            name: skill_name.clone(),
            exit_code,
        });
        if exit_code != 0 {
            return Ok((exit_code, skill_results));
        }
    }
    Ok((0, skill_results))
}

fn run_fork_branch_skill(
    worktree: &Path,
    skill: &str,
    directions: &[String],
    message: Option<&str>,
    branch_label: &str,
) -> Result<i32> {
    let mut cmd = build_lf_command();
    cmd.arg(skill).arg("-b");
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
    use super::{render_pipeline_lines, write_temp_skill};
    use crate::engine::{ConcreteStep, Flow};
    use std::fs;
    use tempfile::tempdir;
    use tempfile::TempDir;

    #[test]
    fn write_xor_route_skill_removes_temp_file_when_none_existed() {
        let temp = TempDir::new().unwrap();
        let skill_path = temp.path().join(".lf/skills/xor-route.md");

        {
            let _guard =
                write_temp_skill(temp.path(), "xor-route", "temporary route prompt").unwrap();
            assert_eq!(
                fs::read_to_string(&skill_path).unwrap(),
                "temporary route prompt"
            );
        }

        assert!(
            !skill_path.exists(),
            "temporary xor-route skill should be removed after use"
        );
    }

    #[test]
    fn write_xor_route_skill_restores_existing_file() {
        let temp = TempDir::new().unwrap();
        let skills_dir = temp.path().join(".lf/skills");
        fs::create_dir_all(&skills_dir).unwrap();
        let skill_path = skills_dir.join("xor-route.md");
        fs::write(&skill_path, "existing route prompt").unwrap();

        {
            let _guard =
                write_temp_skill(temp.path(), "xor-route", "temporary route prompt").unwrap();
            assert_eq!(
                fs::read_to_string(&skill_path).unwrap(),
                "temporary route prompt"
            );
        }

        assert_eq!(
            fs::read_to_string(&skill_path).unwrap(),
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
                crate::engine::flow::Step::Skill(crate::engine::flow::Skill::named(
                    "tend/scan-waves",
                )),
                crate::engine::flow::Step::Xor(crate::engine::flow::XorDef {
                    router: Some("tend/assess".to_string()),
                    paths: [
                        (
                            "tune".to_string(),
                            crate::engine::flow::XorPath {
                                flow: Some("tend/tune".to_string()),
                                skill: None,
                                steps: Vec::new(),
                                description: "Adjust the chord".to_string(),
                                direction: Vec::new(),
                            },
                        ),
                        (
                            "silence".to_string(),
                            crate::engine::flow::XorPath {
                                flow: None,
                                skill: None,
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

        assert!(matches!(items[1], ConcreteStep::Xor(_)));
    }

    #[test]
    fn render_pipeline_lines_shows_and_synthesize_skill() {
        let temp = tempdir().unwrap();
        // Use synthetic skill names that don't collide with builtin flows or
        // skills so expansion stays under test control.
        let flow = Flow {
            name: "demo-and".to_string(),
            items: vec![crate::engine::flow::Step::And {
                branches: vec![
                    crate::engine::flow::Step::Skill(crate::engine::flow::Skill::named(
                        "demo-branch-a",
                    )),
                    crate::engine::flow::Step::Skill(crate::engine::flow::Skill::named(
                        "demo-branch-b",
                    )),
                    crate::engine::flow::Step::Skill(crate::engine::flow::Skill::named(
                        "demo-branch-c",
                    )),
                ],
                synthesize: Some("demo-synthesize".to_string()),
            }],
        };

        let items = crate::engine::expand_flow(&flow, temp.path()).unwrap();
        let lines = render_pipeline_lines(&items, temp.path()).unwrap();

        assert_eq!(
            lines,
            vec![
                "[and]".to_string(),
                "├─ demo-branch-a → demo-branch-a".to_string(),
                "├─ demo-branch-b → demo-branch-b".to_string(),
                "├─ demo-branch-c → demo-branch-c".to_string(),
                "└─ synthesize → demo-synthesize".to_string(),
            ]
        );
    }
}
