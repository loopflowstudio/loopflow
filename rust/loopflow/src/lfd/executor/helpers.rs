use std::path::Path;

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

use time::OffsetDateTime;

use crate::engine::agent::{AgentCapabilities, AgentConfig, ProcessConfig};
use crate::engine::config::{load_config, load_config_or_default};
use crate::engine::flow::{
    expand_flow, load_flow, next_action, ConcreteItem, ConcreteStep, FlowAction,
};
use crate::engine::git::{
    commit, create_branch, current_branch, fetch, get_default_branch, is_clean, push_with_upstream,
    rebase, rev_parse, stage_all,
};
use crate::engine::naming::{format_branch_name, generate_word_pair};
use crate::engine::prompt::write_prompt_log;
use crate::engine::prompt::Surface;
use crate::engine::worktrees::{
    branch_exists, create_with_schema, worktree_path as wave_worktree_path,
};

use crate::engine::launch::{prepare_launch_prompt, LaunchPromptInput};
use crate::engine::structured_reply::ClientContext;
use crate::lfd::id::LfdId;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{
    AgentRun, AgentStatus, Wave, WaveRun, WaveRunSnapshot, WaveRunStackStatus, WaveRunStatus,
    WaveStatus,
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
        .and_then(|run| run.snapshot.pr.as_ref())
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
            flow: wave.flow().clone(),
            direction: wave.direction().clone(),
            area: wave.area().clone(),
            pr: None,
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
        activation_log_id: None,
        parent_run_id,
        parent_pr_number,
        stack_position,
        stack_group_id,
        stack_status: WaveRunStackStatus::Active,
        lineage_inferred: false,
        target_branch: target_branch.unwrap_or("main").to_string(),
    };
    store.create_wave_run(&run).await?;
    if let Ok(Some(mut wave)) = store.get_wave(wave.id()).await {
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
        .and_then(|run| run.snapshot.pr.as_ref())
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
            flow: wave.flow().clone(),
            direction: wave.direction().clone(),
            area: wave.area().clone(),
            pr: None,
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
        activation_log_id: None,
        parent_run_id,
        parent_pr_number,
        stack_position,
        stack_group_id,
        stack_status: WaveRunStackStatus::Active,
        lineage_inferred: false,
        target_branch: target_branch.unwrap_or("main").to_string(),
    };
    store.create_wave_run(&run).await?;
    if let Ok(Some(mut wave)) = store.get_wave(wave.id()).await {
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
    if wt.exists() {
        let branch = current_branch(&wt)?.unwrap_or_default();
        sync_existing_worktree(main_repo, &wt, &branch)?;
        return Ok((wt.to_string_lossy().to_string(), branch));
    }

    let config = load_config(Some(main_repo)).ok().flatten();
    let branch_config = config.as_ref().and_then(|c| c.branch_names.as_ref());
    let result = create_with_schema(main_repo, wave_name, None, branch_config)?;
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
        let result = create_with_schema(main_repo, wave_name, None, branch_config)?;
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
        Some(ConcreteItem::Fork(fork)) => fork.flow_parents.clone(),
        None => Vec::new(),
    }
}

pub(crate) fn resolve_current_step_name(run: &WaveRun, step_index: u32) -> String {
    let repo = Path::new(&run.snapshot.repo);
    let name = load_flow(&run.snapshot.flow, repo)
        .ok()
        .and_then(|flow| expand_flow(&flow, repo).ok())
        .and_then(|plan| match next_action(&plan, step_index as usize) {
            FlowAction::WaitInteractive { step } => Some(step.step.name),
            FlowAction::RunStep { step } => Some(step.step.name),
            _ => None,
        });
    name.unwrap_or_else(|| format!("step-{step_index}"))
}

pub(crate) fn auto_commit_if_dirty(worktree: &Path, step_name: &str) -> Result<()> {
    if is_clean(worktree)? {
        return Ok(());
    }
    stage_all(worktree)?;
    let message = format!("lfd: auto-commit after interactive step '{step_name}'");
    commit(worktree, &message)?;
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

/// Commit any remaining changes, push, and create a draft PR.
/// Returns the PR info if successful, None if skipped or failed.
pub(crate) fn auto_create_pr(
    worktree: &Path,
    wave_name: Option<String>,
) -> Option<crate::lfd::types::PullRequest> {
    use crate::ops::{
        commit_workflow, current_pr, generate_pr_message, update_pr, CommitOptions, NullProgress,
    };

    let commit_options = CommitOptions {
        add: true,
        push: true,
        create_draft_pr: true,
        ..CommitOptions::for_task("commit")
    };
    if let Err(err) = commit_workflow(worktree, &commit_options, &NullProgress) {
        warn!(worktree = %worktree.display(), error = %err, "auto-create PR: commit/push failed");
        return None;
    }

    match current_pr(worktree) {
        Ok(Some(pr)) => {
            let mut title = None;

            // Update the draft PR with an LLM-generated title and description,
            // matching what `lf ops pr` produces. Wave name becomes the PR title prefix.
            match generate_pr_message(worktree, wave_name.as_deref()) {
                Ok(message) => {
                    title = Some(message.title.clone());
                    if let Err(err) = update_pr(worktree, pr.number, &message.title, &message.body)
                    {
                        warn!(worktree = %worktree.display(), error = %err, "auto-create PR: failed to update title/body");
                    }
                }
                Err(err) => {
                    warn!(worktree = %worktree.display(), error = %err, "auto-create PR: failed to generate PR message");
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

/// Pre-step sync: fetch and rebase the worktree onto its remote branch.
///
/// Used at step boundaries so concurrent runs pick up each other's work.
/// Silently skips if the worktree has no git directory or no remote.
pub(crate) fn pre_step_sync(worktree: &Path, branch: &str) -> Result<()> {
    if branch.is_empty() || !worktree.join(".git").exists() {
        return Ok(());
    }
    let remote_ref = format!("origin/{branch}");
    if fetch(worktree, "origin", branch).is_ok() && rev_parse(worktree, &remote_ref).is_ok() {
        let result = rebase(worktree, &remote_ref, None)?;
        if !result.success {
            return Err(anyhow!(
                "pre-step rebase onto {remote_ref} failed — aborting step"
            ));
        }
    }
    Ok(())
}

/// Post-step sync: stage, commit, and push changes after a successful step.
///
/// On a non-fast-forward push, fetches and rebases then retries once.
/// Silently skips if the worktree has no git directory or no remote.
pub(crate) fn post_step_sync(worktree: &Path, branch: &str, step_name: &str) -> Result<()> {
    if branch.is_empty() || !worktree.join(".git").exists() {
        return Ok(());
    }

    if is_clean(worktree)? {
        return Ok(());
    }
    stage_all(worktree)?;
    let message = format!("lf commit: {step_name}");
    commit(worktree, &message)?;

    match push_with_upstream(worktree, "origin", branch) {
        Ok(()) => Ok(()),
        Err(_) => {
            debug!(branch = %branch, "push failed, retrying after fetch+rebase");
            let remote_ref = format!("origin/{branch}");
            fetch(worktree, "origin", branch)?;
            let result = rebase(worktree, &remote_ref, None)?;
            if !result.success {
                return Err(anyhow!(
                    "post-step rebase onto {remote_ref} failed after push rejection"
                ));
            }
            push_with_upstream(worktree, "origin", branch)
                .map_err(|err| anyhow!("post-step push failed after rebase: {err}"))
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

    if fetch(main_repo, "origin", branch).is_ok()
        && rev_parse(main_repo, &format!("origin/{branch}")).is_ok()
    {
        let branch_rebase = rebase(worktree, &format!("origin/{branch}"), None)?;
        if !branch_rebase.success {
            return Err(anyhow!("failed to rebase worktree onto origin/{branch}"));
        }
    }

    let default_branch = get_default_branch(main_repo)?;
    if fetch(main_repo, "origin", &default_branch).is_ok()
        && rev_parse(main_repo, &format!("origin/{default_branch}")).is_ok()
    {
        let upstream_rebase = rebase(worktree, &format!("origin/{default_branch}"), None)?;
        if !upstream_rebase.success {
            return Err(anyhow!(
                "failed to rebase worktree onto origin/{default_branch}"
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_ephemeral_worktree_path;

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
}
