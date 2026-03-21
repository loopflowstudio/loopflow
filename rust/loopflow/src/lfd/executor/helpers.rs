use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use tracing::{debug, error, info, warn};

use time::OffsetDateTime;

use crate::engine::agent::{AgentCapabilities, AgentConfig, ProcessConfig};
use crate::engine::builtins::get_builtin_step;
use crate::engine::config::{load_config, load_config_or_default};
use crate::engine::flow::{
    expand_flow, load_flow, next_action, ConcreteItem, ConcreteStep, FlowAction,
};
use crate::engine::git::{
    create_branch, current_branch, fetch, get_default_branch, push_with_upstream, rev_parse,
};
use crate::engine::naming::{format_branch_name, generate_word_pair};
use crate::engine::prompt::write_prompt_log;
use crate::engine::prompt::Surface;
use crate::engine::worktrees::{
    branch_exists, create_with_schema_synced, worktree_path as wave_worktree_path,
};
use crate::engine::{current_step, ExecutionCursor};

use crate::engine::launch::{prepare_launch_prompt, LaunchPromptInput};
use crate::engine::structured_reply::ClientContext;
use crate::lfd::id::LfdId;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{
    AgentRun, AgentStatus, Wave, WaveRun, WaveRunSnapshot, WaveRunStackStatus, WaveRunStatus,
    WaveStatus,
};
use crate::ops::{
    commit_workflow, rebase_with_recovery, CommitOptions, NullProgress, Progress, RebaseOptions,
};

/// Create a wave run using a per-run worktree for parallel execution.
pub async fn create_parallel_wave_run(
    store: &SharedStore,
    wave: &Wave,
    run_id: &LfdId,
    target_branch: Option<&str>,
) -> anyhow::Result<WaveRun> {
    let stack_runs = store.list_stack_runs(wave.id()).await?;
    let last_run = stack_runs.last().cloned();
    let iteration = last_run.as_ref().map(|run| run.iteration + 1).unwrap_or(0);
    let stack_position = last_run
        .as_ref()
        .map(|run| run.stack_position + 1)
        .unwrap_or(0);
    let parent_run_id = last_run.as_ref().map(|run| run.id.clone());
    let parent_pr_number = last_run
        .as_ref()
        .and_then(|run| run.pr.as_ref())
        .and_then(|pr| pr.number);
    let stack_group_id = last_run
        .as_ref()
        .map(|run| run.stack_group_id.clone())
        .unwrap_or_else(|| wave.id().to_string());

    let main_repo = Path::new(wave.repo());
    let (wt_path, branch) =
        create_run_worktree(main_repo, wave.name(), run_id.as_str(), target_branch)?;

    let run = WaveRun {
        id: run_id.clone(),
        wave_id: wave.id().clone(),
        snapshot: WaveRunSnapshot {
            repo: wave.repo().clone(),
            flow: wave.primary_flow().clone(),
            direction: wave.direction().clone(),
            area: wave.area().clone(),
        },
        iteration,
        step_index: 0,
        status: WaveRunStatus::Running,
        worktree: wt_path,
        branch,
        started_at: Some(OffsetDateTime::now_utc()),
        ended_at: None,
        error: None,
        flow_parents: Vec::new(),
        execution_cursor: None,
        activation_log_id: None,
        parent_run_id,
        parent_pr_number,
        stack_position,
        stack_group_id,
        stack_status: WaveRunStackStatus::Active,
        lineage_inferred: false,
        target_branch: target_branch.unwrap_or("main").to_string(),
        repair_of: None,
        pr: None,
    };
    store.create_wave_run(&run).await?;
    if let Ok(Some(mut wave)) = store.get_wave(wave.id()).await {
        // New cycle: record the starting iteration for max_iterations safety valve.
        if wave.status == WaveStatus::Idle || wave.status == WaveStatus::Paused {
            wave.cycle_start_iteration = iteration;
        }
        wave.status = WaveStatus::Running;
        wave.iteration = iteration;
        if let Err(err) = store.update_wave(&wave).await {
            warn!(wave_id = %wave.id(), error = %err, "failed to set wave status to running");
        }
    }
    Ok(run)
}

/// Create a wave run with a worktree and branch for the wave.
///
/// For serialized waves targeting a specific branch (non-"main"),
/// uses a per-run worktree instead of the shared wave worktree.
pub async fn create_wave_run_with_id(
    store: &SharedStore,
    wave: &Wave,
    run_id: &LfdId,
    target_branch: Option<&str>,
) -> anyhow::Result<WaveRun> {
    let stack_runs = store.list_stack_runs(wave.id()).await?;
    let last_run = stack_runs.last().cloned();
    let iteration = last_run.as_ref().map(|run| run.iteration + 1).unwrap_or(0);
    let stack_position = last_run
        .as_ref()
        .map(|run| run.stack_position + 1)
        .unwrap_or(0);
    let parent_run_id = last_run.as_ref().map(|run| run.id.clone());
    let parent_pr_number = last_run
        .as_ref()
        .and_then(|run| run.pr.as_ref())
        .and_then(|pr| pr.number);
    let stack_group_id = last_run
        .as_ref()
        .map(|run| run.stack_group_id.clone())
        .unwrap_or_else(|| wave.id().to_string());

    let main_repo = Path::new(wave.repo());

    // Targeted activations (non-main branch) get their own worktree
    // even for serialized waves, since the wave's shared worktree is
    // on a different branch.
    let is_targeted = target_branch
        .map(|b| !b.is_empty() && b != "main")
        .unwrap_or(false);
    let (wt_path, branch) = if is_targeted {
        create_run_worktree(main_repo, wave.name(), run_id.as_str(), target_branch)?
    } else {
        ensure_wave_worktree(main_repo, wave.name())?
    };

    let run = WaveRun {
        id: run_id.clone(),
        wave_id: wave.id().clone(),
        snapshot: WaveRunSnapshot {
            repo: wave.repo().clone(),
            flow: wave.primary_flow().clone(),
            direction: wave.direction().clone(),
            area: wave.area().clone(),
        },
        iteration,
        step_index: 0,
        status: WaveRunStatus::Running,
        worktree: wt_path,
        branch,
        started_at: Some(OffsetDateTime::now_utc()),
        ended_at: None,
        error: None,
        flow_parents: Vec::new(),
        execution_cursor: None,
        activation_log_id: None,
        parent_run_id,
        parent_pr_number,
        stack_position,
        stack_group_id,
        stack_status: WaveRunStackStatus::Active,
        lineage_inferred: false,
        target_branch: target_branch.unwrap_or("main").to_string(),
        repair_of: None,
        pr: None,
    };
    store.create_wave_run(&run).await?;
    if let Ok(Some(mut wave)) = store.get_wave(wave.id()).await {
        if wave.status == WaveStatus::Idle || wave.status == WaveStatus::Paused {
            wave.cycle_start_iteration = iteration;
        }
        wave.status = WaveStatus::Running;
        wave.iteration = iteration;
        if let Err(err) = store.update_wave(&wave).await {
            warn!(wave_id = %wave.id(), error = %err, "failed to set wave status to running");
        }
    }
    Ok(run)
}

/// Create a worktree for this wave, or reuse the existing one.
pub fn ensure_wave_worktree(main_repo: &Path, wave_name: &str) -> anyhow::Result<(String, String)> {
    let wt = wave_worktree_path(main_repo, wave_name);
    if wt.exists() && wt.join(".git").exists() {
        let branch = current_branch(&wt)?.unwrap_or_default();
        return Ok((wt.to_string_lossy().to_string(), branch));
    }

    let config = load_config(Some(main_repo)).ok().flatten();
    let branch_config = config.as_ref().and_then(|c| c.branch_names.as_ref());
    let result = create_with_schema_synced(main_repo, wave_name, None, branch_config)?;
    Ok((result.path.to_string_lossy().to_string(), result.branch))
}

/// Create a run-scoped worktree on the wave's remote branch.
///
/// Used for parallel (non-serialized) execution: each run gets its own
/// worktree so concurrent runs don't stomp on each other's files.
/// The worktree is placed at `{wave-worktree}-run-{hash}` and tracks the
/// same remote branch as the wave worktree.
///
/// When `target_branch` is `Some` and not `"main"`, the worktree tracks
/// that branch directly (e.g. a PR branch for CI-fix activations) instead
/// of the wave's own branch.
pub fn create_run_worktree(
    main_repo: &Path,
    wave_name: &str,
    run_id: &str,
    target_branch: Option<&str>,
) -> anyhow::Result<(String, String)> {
    use crate::engine::git::{worktree_add, WorktreeBranch};

    let base_wt = wave_worktree_path(main_repo, wave_name);
    let suffix = short_hash(run_id, 8);
    let run_wt = base_wt.with_file_name(format!(
        "{}-run-{suffix}",
        base_wt
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("wave")
    ));

    // Determine which remote branch to track.
    let is_targeted = target_branch
        .map(|b| !b.is_empty() && b != "main")
        .unwrap_or(false);

    let branch = if is_targeted {
        // Targeted activation: track the specified branch directly.
        let tb = target_branch.expect("checked above");
        fetch(main_repo, "origin", tb)?;
        tb.to_string()
    } else if base_wt.exists() {
        // Normal: reuse the wave worktree's branch.
        let branch = current_branch(&base_wt)?.unwrap_or_default();
        sync_existing_worktree(main_repo, &base_wt, &branch)?;
        branch
    } else {
        let config = load_config(Some(main_repo)).ok().flatten();
        let branch_config = config.as_ref().and_then(|c| c.branch_names.as_ref());
        let result = create_with_schema_synced(main_repo, wave_name, None, branch_config)?;
        result.branch
    };

    // Each run gets a temporary local branch that tracks origin/{branch}.
    // This lets git push work normally from the worktree.
    let run_branch = format!("{branch}-run-{suffix}");
    let remote_ref = format!("origin/{branch}");
    worktree_add(
        main_repo,
        &run_wt,
        &run_branch,
        WorktreeBranch::Track {
            remote: &remote_ref,
        },
    )?;

    // Sync to pick up latest from origin.
    sync_existing_worktree(main_repo, &run_wt, &run_branch)?;

    // Return the wave branch name (not the run-local branch) so the run
    // record tracks which remote branch it pushes to.
    Ok((run_wt.to_string_lossy().to_string(), branch))
}

pub(crate) fn fork_worktree_path(run: &WaveRun, branch_index: u32) -> String {
    crate::engine::fork::fork_worktree_path(Path::new(&run.worktree), branch_index as usize)
        .to_string_lossy()
        .to_string()
}

pub(crate) fn is_active_wave_run_status(status: WaveRunStatus) -> bool {
    matches!(
        status,
        WaveRunStatus::Pending | WaveRunStatus::Running | WaveRunStatus::Waiting
    )
}

pub(crate) fn is_ephemeral_worktree_path(path: &str) -> bool {
    let worktree_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);
    has_fork_suffix(worktree_name) || has_run_suffix(worktree_name)
}

fn has_fork_suffix(path_component: &str) -> bool {
    let Some((_, suffix)) = path_component.rsplit_once("-fork-") else {
        return false;
    };
    !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_digit())
}

fn has_run_suffix(path_component: &str) -> bool {
    let Some((_, suffix)) = path_component.rsplit_once("-run-") else {
        return false;
    };
    !suffix.is_empty() && suffix.chars().all(|ch| ch.is_ascii_hexdigit())
}

pub(crate) fn flow_parents_for_index(items: &[ConcreteItem], step_index: u32) -> Vec<String> {
    match items.get(step_index as usize) {
        Some(ConcreteItem::Step(step)) => step.flow_parents.clone(),
        Some(ConcreteItem::Op(ops)) => ops.flow_parents.clone(),
        Some(ConcreteItem::And(and)) => and.flow_parents.clone(),
        Some(ConcreteItem::Xor(or_item)) => or_item.flow_parents.clone(),
        Some(ConcreteItem::Or(or_item)) => or_item.flow_parents.clone(),
        Some(ConcreteItem::Loop(loop_item)) => loop_item.flow_parents.clone(),
        None => Vec::new(),
    }
}

pub(crate) fn resolve_current_step_name(run: &WaveRun, step_index: u32) -> String {
    let repo = Path::new(&run.worktree);
    let cursor = run
        .execution_cursor
        .as_deref()
        .filter(|raw| !raw.trim().is_empty())
        .and_then(|raw| serde_json::from_str::<ExecutionCursor>(raw).ok())
        .unwrap_or(ExecutionCursor {
            index: step_index as usize,
            child: None,
        });
    let name = load_flow(&run.snapshot.flow, repo)
        .ok()
        .and_then(|flow| expand_flow(&flow, repo).ok())
        .and_then(|plan| {
            current_step(&plan, &cursor, repo)
                .ok()
                .flatten()
                .map(|step| step.step.name)
                .or_else(|| match next_action(&plan, step_index as usize) {
                    FlowAction::RunOps { ops } => Some(format!("op {}", ops.item.command)),
                    FlowAction::Loop { .. } => Some("loop".to_string()),
                    _ => None,
                })
        });
    name.unwrap_or_else(|| format!("step-{step_index}"))
}

pub(crate) fn auto_commit_if_dirty(worktree: &Path, step_name: &str) -> Result<()> {
    let options = CommitOptions {
        add: true,
        message: None,
        ..CommitOptions::for_task(step_name)
    };
    commit_workflow(worktree, &options, &NullProgress)
        .map_err(|err| anyhow!("auto-commit failed: {err}"))?;
    Ok(())
}

pub(crate) async fn build_step_prompt(
    worktree: &str,
    step: &ConcreteStep,
    directions: &[String],
    wave: Option<&str>,
    summary_source: Option<(&SharedStore, &LfdId)>,
    agent: Option<String>,
    message: Option<String>,
) -> Result<(AgentConfig, ProcessConfig)> {
    let repo_root = Path::new(worktree);
    let config = load_config_or_default(Some(repo_root));

    let summary = if let Some((store, wave_id)) = summary_source {
        store
            .get_summary(wave_id)
            .await
            .ok()
            .flatten()
            .map(|record| record.content)
    } else {
        None
    };

    let prepared = prepare_launch_prompt(
        &config,
        LaunchPromptInput {
            repo_root: repo_root.to_path_buf(),
            step: Some(step.step.name.clone()),
            resolved_step: None,
            surface: Surface::Headless,
            directions: directions.to_vec(),
            area: None,
            wave: wave.map(str::to_string),
            message,
            agent,
            cwd: None,
            max_turns: None,
            yolo_mode: false,
            include_config_directions: false,
            include_config_area: true,
            source_overrides: Default::default(),
            summary,
            client_context: ClientContext::default(),
            related_repos: Vec::new(),
        },
    )?;

    let _ = write_prompt_log(repo_root, &prepared.prompt, &step.step.name, None);
    let mut agent_config = prepared.config;

    let cwd = agent_config
        .cwd
        .clone()
        .unwrap_or_else(|| repo_root.to_path_buf());
    let context_file = write_prompt_log(
        &cwd,
        &agent_config.system_prompt,
        &format!("{}.context", step.step.name),
        None,
    )
    .ok();
    agent_config.cwd = Some(cwd);
    agent_config.skip_permissions = config.yolo;

    let process = ProcessConfig {
        auto: true,
        stream: true,
        context_file,
        ..Default::default()
    };

    Ok((agent_config, process))
}

pub(crate) fn build_agent_capabilities(worktree: &str) -> AgentCapabilities {
    let config = load_config_or_default(Some(Path::new(worktree)));
    AgentCapabilities {
        chrome: config.chrome,
    }
}

pub(crate) fn build_agent_for_step(
    wave_run_id: &LfdId,
    repo: &str,
    worktree: &str,
    step: &ConcreteStep,
    status: AgentStatus,
    agent: &str,
) -> AgentRun {
    AgentRun {
        id: LfdId::new(),
        step: step.step.name.clone(),
        repo: repo.to_string(),
        worktree: worktree.to_string(),
        wave_run_id: Some(wave_run_id.clone()),
        status,
        started_at: Some(OffsetDateTime::now_utc()),
        ended_at: None,
        pid: None,
        container_id: None,
        agent: agent.to_string(),
        run_mode: "auto".to_string(),
    }
}

pub(crate) fn resolve_lf_binary() -> PathBuf {
    if let Ok(path) = std::env::var("LF_BIN") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Ok(path) = std::env::var("CARGO_BIN_EXE_lf") {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            return PathBuf::from(trimmed);
        }
    }

    if let Ok(current) = std::env::current_exe() {
        if current
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "lf")
        {
            return current;
        }
        if let Some(parent) = current.parent() {
            let sibling = parent.join("lf");
            if sibling.exists() {
                return sibling;
            }
        }
    }

    PathBuf::from("lf")
}

pub(crate) fn build_lf_step_command(
    step_name: &str,
    batch: bool,
    directions: &[String],
    area: &[String],
    wave_name: &str,
) -> Vec<String> {
    let mut cmd = vec![
        resolve_lf_binary().to_string_lossy().to_string(),
        step_name.to_string(),
    ];
    if batch {
        cmd.push("-b".to_string());
    }
    cmd.push("--no-direction".to_string());
    for direction in directions {
        cmd.push("-d".to_string());
        cmd.push(direction.clone());
    }
    for scope in area {
        cmd.push("-a".to_string());
        cmd.push(scope.clone());
    }
    cmd.push("-w".to_string());
    cmd.push(wave_name.to_string());
    cmd
}

/// Commit any remaining changes, push, and create a draft PR.
/// Returns the PR info if successful, None if skipped or failed.
pub(crate) fn auto_create_pr(
    worktree: &Path,
    wave_name: Option<String>,
) -> Option<crate::lfd::types::PullRequest> {
    use crate::ops::{commit_workflow, current_pr, CommitOptions, NullProgress};

    let commit_options = CommitOptions {
        add: true,
        push: true,
        create_draft_pr: true,
        message: Some("lfd: auto-create draft PR".to_string()),
        ..CommitOptions::for_task("commit")
    };
    if let Err(err) = commit_workflow(worktree, &commit_options, &NullProgress) {
        warn!(worktree = %worktree.display(), error = %err, "auto-create PR: commit/push failed");
        return None;
    }

    match current_pr(worktree) {
        Ok(Some(pr)) => {
            let title = draft_pr_title(worktree, wave_name.as_deref());
            if let Some(draft_title) = title.as_deref() {
                if let Err(err) = set_pr_title(worktree, pr.number, draft_title) {
                    warn!(
                        worktree = %worktree.display(),
                        error = %err,
                        "auto-create PR: failed to set draft title"
                    );
                }
            }

            Some(crate::lfd::types::PullRequest {
                url: pr.url,
                number: Some(pr.number as u32),
                state: Some(pr.state),
                branch: Some(pr.branch),
                title,
            })
        }
        Ok(None) => {
            debug!(worktree = %worktree.display(), "auto-create PR: no PR found after push");
            None
        }
        Err(err) => {
            warn!(worktree = %worktree.display(), error = %err, "auto-create PR: failed to fetch PR info");
            None
        }
    }
}

fn draft_pr_title(worktree: &Path, wave_name: Option<&str>) -> Option<String> {
    if let Some(name) = wave_name.map(str::trim).filter(|name| !name.is_empty()) {
        return Some(format!("{name}: draft"));
    }

    first_branch_commit_subject(worktree)
}

fn first_branch_commit_subject(worktree: &Path) -> Option<String> {
    let default_branch = get_default_branch(worktree).ok()?;
    let merge_base_output = std::process::Command::new("git")
        .args(["merge-base", "HEAD", &format!("origin/{default_branch}")])
        .current_dir(worktree)
        .output()
        .ok()?;
    if !merge_base_output.status.success() {
        return None;
    }

    let merge_base = String::from_utf8_lossy(&merge_base_output.stdout)
        .trim()
        .to_string();
    if merge_base.is_empty() {
        return None;
    }

    let range = format!("{merge_base}..HEAD");
    let log_output = std::process::Command::new("git")
        .args(["log", "--reverse", "--format=%s", &range])
        .current_dir(worktree)
        .output()
        .ok()?;
    if !log_output.status.success() {
        return None;
    }

    String::from_utf8_lossy(&log_output.stdout)
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(ToString::to_string)
}

fn set_pr_title(worktree: &Path, number: u64, title: &str) -> Result<()> {
    let output = std::process::Command::new("gh")
        .arg("pr")
        .arg("edit")
        .arg(number.to_string())
        .arg("--title")
        .arg(title)
        .current_dir(worktree)
        .output()
        .map_err(|err| anyhow!("failed to run gh pr edit: {err}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "gh pr edit failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

/// Create a new branch in the worktree for the next loop iteration.
pub(crate) fn advance_branch(worktree: &Path, wave_name: &str) -> anyhow::Result<String> {
    let config = load_config_or_default(Some(worktree));
    let branch_config = config.branch_names.as_ref();
    let mut new_branch = format_branch_name(wave_name, branch_config, worktree)
        .map_err(|e| anyhow!("failed to generate branch name: {e}"))?;

    while branch_exists(worktree, &new_branch)? {
        new_branch = format!("{new_branch}.{}", generate_word_pair());
    }

    create_branch(worktree, &new_branch)?;
    push_with_upstream(worktree, "origin", &new_branch)?;
    Ok(new_branch)
}

#[derive(Debug, Clone)]
struct BuiltinAgentOptions {
    step_name: String,
    suffix: String,
    timeout: Option<Duration>,
}

fn run_builtin_agent(
    repo: &Path,
    options: &BuiltinAgentOptions,
    progress: &impl Progress,
) -> Result<()> {
    let config = load_config_or_default(Some(repo));
    let step_content = get_builtin_step(&options.step_name)
        .ok_or_else(|| anyhow!("built-in step '{}' not found", options.step_name))?;
    let prompt = format!(
        "<lf:step>\n{}\n</lf:step>\n\n{}\n",
        step_content, options.suffix
    );

    let launch = AgentConfig {
        task_prompt: prompt,
        agent: config.agent.clone(),
        cwd: Some(repo.to_path_buf()),
        skip_permissions: true,
        ..Default::default()
    };
    let process = ProcessConfig {
        auto: true,
        stream: true,
        timeout: options.timeout,
        ..Default::default()
    };
    let capabilities = AgentCapabilities {
        chrome: config.chrome,
    };

    progress.status(&format!("Launching {} agent...", options.step_name));
    let result = crate::engine::launch_agent(&launch, &process, &capabilities)
        .map_err(|err| anyhow!("failed to launch builtin agent: {err}"))?;
    if result.exit_code != 0 {
        return Err(anyhow!("builtin agent failed: {}", result.stderr));
    }
    Ok(())
}

/// Pre-step sync: fetch and rebase the worktree onto its remote branch.
///
/// Used at step boundaries so concurrent runs pick up each other's work.
/// Silently skips if the worktree has no git directory or no remote.
pub(crate) fn pre_step_sync(main_repo: &Path, worktree: &Path, branch: &str) -> Result<()> {
    if branch.is_empty() || !worktree.join(".git").exists() {
        return Ok(());
    }

    dual_rebase(main_repo, worktree, branch)
}

/// Post-step sync: stage, commit, and push changes after a successful step.
///
/// On a non-fast-forward push, fetches and rebases then retries. If the retry
/// also fails, escalates to a debug agent session before giving up.
/// Silently skips if the worktree has no git directory or no remote.
pub(crate) fn post_step_sync(worktree: &Path, branch: &str, step_name: &str) -> Result<()> {
    if branch.is_empty() || !worktree.join(".git").exists() {
        return Ok(());
    }

    let options = CommitOptions {
        add: true,
        message: None,
        ..CommitOptions::for_task(step_name)
    };
    let committed = commit_workflow(worktree, &options, &NullProgress)
        .map_err(|err| anyhow!("commit_workflow failed for step {step_name}: {err}"))?;
    if !committed {
        return Ok(());
    }

    match push_with_upstream(worktree, "origin", branch) {
        Ok(()) => Ok(()),
        Err(push_err) => {
            debug!(branch = %branch, "push failed, retrying after fetch+rebase");
            let remote_ref = format!("origin/{branch}");
            fetch(worktree, "origin", branch)?;
            rebase_with_recovery(
                worktree,
                &RebaseOptions {
                    onto: remote_ref,
                    push: false,
                },
                &TracingProgress,
            )?;
            match push_with_upstream(worktree, "origin", branch) {
                Ok(()) => Ok(()),
                Err(retry_err) => {
                    warn!(branch = %branch, "push retry exhausted, escalating to debug agent");
                    let error_context = format!(
                        "git push to origin/{branch} failed after fetch+rebase retry.\n\
                         Error: {retry_err}\n\
                         Working directory: {}\n\
                         Branch: {branch}",
                        worktree.display()
                    );
                    let options = BuiltinAgentOptions {
                        step_name: "debug".to_string(),
                        suffix: error_context,
                        timeout: Some(Duration::from_secs(5 * 60)),
                    };

                    match run_builtin_agent(worktree, &options, &TracingProgress) {
                        Ok(()) => push_with_upstream(worktree, "origin", branch).map_err(|err| {
                            anyhow!(
                                "push failed after debug agent intervention.\n\
                                 Original error: {push_err}\n\
                                 Retry error: {retry_err}\n\
                                 Post-agent error: {err}\n\
                                 Worktree: {}\n\
                                 Branch: {branch}\n\
                                 Manual resolution may be needed.",
                                worktree.display()
                            )
                        }),
                        Err(agent_err) => Err(anyhow!(
                            "push failed and debug agent could not resolve it.\n\
                             Original error: {push_err}\n\
                             Retry error: {retry_err}\n\
                             Agent error: {agent_err}\n\
                             Worktree: {}\n\
                             Branch: {branch}\n\
                             Manual resolution needed.",
                            worktree.display()
                        )),
                    }
                }
            }
        }
    }
}

/// Clean up a run-scoped worktree after the run completes.
///
/// Only removes worktrees with the `-run-` suffix to avoid touching wave worktrees.
pub(crate) fn cleanup_run_worktree(worktree: &Path) -> Result<()> {
    let name = worktree.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !has_run_suffix(name) {
        return Ok(());
    }
    super::cleanup_workspace_worktree(worktree)
}

pub(crate) fn short_hash(value: &str, chars: usize) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut hash = hex::encode(digest);
    hash.truncate(chars);
    hash
}

fn sync_existing_worktree(main_repo: &Path, worktree: &Path, branch: &str) -> anyhow::Result<()> {
    if branch.is_empty() {
        return Ok(());
    }

    dual_rebase(main_repo, worktree, branch)
}

#[derive(Debug, Clone, Copy, Default)]
struct TracingProgress;

impl Progress for TracingProgress {
    fn status(&self, msg: &str) {
        info!("{msg}");
    }

    fn error(&self, msg: &str) {
        error!("{msg}");
    }

    fn confirm(&self, _msg: &str) -> bool {
        true
    }
}

fn dual_rebase(main_repo: &Path, worktree: &Path, branch: &str) -> Result<()> {
    let progress = TracingProgress;
    rebase_onto_if_available(main_repo, worktree, branch, &progress)?;

    let default_branch = get_default_branch(main_repo)?;
    rebase_onto_if_available(main_repo, worktree, &default_branch, &progress)?;
    Ok(())
}

fn rebase_onto_if_available(
    main_repo: &Path,
    worktree: &Path,
    branch: &str,
    progress: &impl Progress,
) -> Result<()> {
    if fetch(main_repo, "origin", branch).is_err() {
        return Ok(());
    }

    let remote_ref = format!("origin/{branch}");
    if rev_parse(main_repo, &remote_ref).is_err() {
        return Ok(());
    }

    rebase_with_recovery(
        worktree,
        &RebaseOptions {
            onto: remote_ref,
            push: false,
        },
        progress,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use tempfile::TempDir;

    use super::{ensure_wave_worktree, is_ephemeral_worktree_path, pre_step_sync};
    use crate::engine::worktrees::worktree_path as wave_worktree_path;

    fn run_git(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git command should run");
        if !output.status.success() {
            panic!(
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    fn run_git_status(dir: &Path, args: &[&str]) -> bool {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .status()
            .expect("git command should run")
            .success()
    }

    fn write_file(path: &Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent directories");
        }
        std::fs::write(path, content).expect("write file");
    }

    fn setup_repo_with_remote() -> (TempDir, std::path::PathBuf, std::path::PathBuf) {
        let temp = TempDir::new().expect("temp dir");
        let origin = temp.path().join("origin.git");
        let main_repo = temp.path().join("main");
        std::fs::create_dir_all(&main_repo).expect("create repo dir");

        run_git(temp.path(), &["init", "--bare", "-b", "main", "origin.git"]);
        run_git(&main_repo, &["init", "-b", "main"]);
        run_git(&main_repo, &["config", "user.email", "test@example.com"]);
        run_git(&main_repo, &["config", "user.name", "Test User"]);
        write_file(&main_repo.join("README.md"), "initial\n");
        run_git(&main_repo, &["add", "."]);
        run_git(&main_repo, &["commit", "-m", "initial"]);
        run_git(
            &main_repo,
            &["remote", "add", "origin", origin.to_str().unwrap_or("")],
        );
        run_git(&main_repo, &["push", "-u", "origin", "main"]);
        run_git(
            &main_repo,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        );
        (temp, main_repo, origin)
    }

    #[test]
    fn is_ephemeral_worktree_path_detects_numeric_fork_suffix() {
        assert!(is_ephemeral_worktree_path("/tmp/repo.wave-fork-0"));
        assert!(is_ephemeral_worktree_path("/tmp/repo-fork-123"));
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave-fork-x"));
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave.fork-1"));
    }

    #[test]
    fn is_ephemeral_worktree_path_detects_run_suffix() {
        assert!(is_ephemeral_worktree_path("/tmp/repo.wave-run-a1b2c3d4"));
        assert!(is_ephemeral_worktree_path("/tmp/repo-run-deadbeef"));
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave-run-"));
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave-run-xyz!"));
    }

    #[test]
    fn is_ephemeral_worktree_path_ignores_non_fork_paths() {
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave.main"));
    }

    #[test]
    fn pre_step_sync_rebases_onto_wave_and_default_branch() {
        let (_temp, main_repo, origin) = setup_repo_with_remote();

        run_git(&main_repo, &["checkout", "-b", "wave"]);
        write_file(&main_repo.join("wave-local.txt"), "local wave work\n");
        run_git(&main_repo, &["add", "."]);
        run_git(&main_repo, &["commit", "-m", "wave local"]);
        run_git(&main_repo, &["push", "-u", "origin", "wave"]);

        let collaborator = main_repo
            .parent()
            .expect("main repo parent")
            .join("collaborator");
        run_git(
            main_repo.parent().expect("main repo parent"),
            &[
                "clone",
                origin.to_str().unwrap_or(""),
                collaborator.to_str().unwrap_or(""),
            ],
        );
        run_git(&collaborator, &["config", "user.email", "test@example.com"]);
        run_git(&collaborator, &["config", "user.name", "Test User"]);
        run_git(&collaborator, &["checkout", "wave"]);
        write_file(&collaborator.join("wave-remote.txt"), "remote wave work\n");
        run_git(&collaborator, &["add", "."]);
        run_git(&collaborator, &["commit", "-m", "wave remote"]);
        run_git(&collaborator, &["push"]);
        run_git(&collaborator, &["checkout", "main"]);
        write_file(&collaborator.join("main-remote.txt"), "remote main work\n");
        run_git(&collaborator, &["add", "."]);
        run_git(&collaborator, &["commit", "-m", "main remote"]);
        run_git(&collaborator, &["push"]);

        run_git(&main_repo, &["checkout", "main"]);
        let worktree = main_repo
            .parent()
            .expect("main repo parent")
            .join("wave-wt");
        run_git(
            &main_repo,
            &["worktree", "add", worktree.to_str().unwrap_or(""), "wave"],
        );

        pre_step_sync(&main_repo, &worktree, "wave").expect("pre-step sync");

        assert!(worktree.join("wave-remote.txt").exists());
        assert!(worktree.join("main-remote.txt").exists());
        assert!(run_git_status(
            &worktree,
            &["merge-base", "--is-ancestor", "origin/main", "HEAD"]
        ));
    }

    #[test]
    fn pre_step_sync_skips_missing_remote_branch_and_still_rebases_default() {
        let (_temp, main_repo, origin) = setup_repo_with_remote();

        run_git(&main_repo, &["checkout", "-b", "local-only"]);
        write_file(&main_repo.join("local-only.txt"), "local branch\n");
        run_git(&main_repo, &["add", "."]);
        run_git(&main_repo, &["commit", "-m", "local only"]);

        let collaborator = main_repo
            .parent()
            .expect("main repo parent")
            .join("collaborator-default");
        run_git(
            main_repo.parent().expect("main repo parent"),
            &[
                "clone",
                origin.to_str().unwrap_or(""),
                collaborator.to_str().unwrap_or(""),
            ],
        );
        run_git(&collaborator, &["config", "user.email", "test@example.com"]);
        run_git(&collaborator, &["config", "user.name", "Test User"]);
        write_file(&collaborator.join("main-update.txt"), "updated main\n");
        run_git(&collaborator, &["add", "."]);
        run_git(&collaborator, &["commit", "-m", "main update"]);
        run_git(&collaborator, &["push"]);

        run_git(&main_repo, &["checkout", "main"]);
        let worktree = main_repo
            .parent()
            .expect("main repo parent")
            .join("local-only-wt");
        run_git(
            &main_repo,
            &[
                "worktree",
                "add",
                worktree.to_str().unwrap_or(""),
                "local-only",
            ],
        );

        pre_step_sync(&main_repo, &worktree, "local-only").expect("pre-step sync");

        assert!(worktree.join("main-update.txt").exists());
        assert!(run_git_status(
            &worktree,
            &["merge-base", "--is-ancestor", "origin/main", "HEAD"]
        ));
    }

    #[test]
    fn ensure_wave_worktree_reuses_existing_sibling_without_rebasing() {
        let (_temp, main_repo, origin) = setup_repo_with_remote();
        let wave_name = "agent-embedding";
        let worktree = wave_worktree_path(&main_repo, wave_name);

        run_git(&main_repo, &["checkout", "-b", wave_name]);
        write_file(&main_repo.join("shared.txt"), "wave branch change\n");
        run_git(&main_repo, &["add", "."]);
        run_git(&main_repo, &["commit", "-m", "wave branch change"]);
        run_git(&main_repo, &["checkout", "main"]);

        run_git(
            &main_repo,
            &[
                "worktree",
                "add",
                worktree.to_str().unwrap_or(""),
                wave_name,
            ],
        );

        let collaborator = main_repo
            .parent()
            .expect("main repo parent")
            .join("collaborator-conflict");
        run_git(
            main_repo.parent().expect("main repo parent"),
            &[
                "clone",
                origin.to_str().unwrap_or(""),
                collaborator.to_str().unwrap_or(""),
            ],
        );
        run_git(&collaborator, &["config", "user.email", "test@example.com"]);
        run_git(&collaborator, &["config", "user.name", "Test User"]);
        write_file(&collaborator.join("shared.txt"), "main branch change\n");
        run_git(&collaborator, &["add", "."]);
        run_git(&collaborator, &["commit", "-m", "main branch change"]);
        run_git(&collaborator, &["push"]);

        let (resolved_worktree, resolved_branch) =
            ensure_wave_worktree(&main_repo, wave_name).expect("reuse existing worktree");

        assert_eq!(resolved_worktree, worktree.to_string_lossy());
        assert_eq!(resolved_branch, wave_name);
        let shared =
            std::fs::read_to_string(worktree.join("shared.txt")).expect("read shared file");
        assert_eq!(shared, "wave branch change\n");
    }
}
