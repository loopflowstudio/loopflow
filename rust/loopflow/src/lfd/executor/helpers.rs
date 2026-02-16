use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

use time::OffsetDateTime;

use crate::engine::agent::LaunchConfig;
use crate::engine::config::{load_config, load_config_or_default};
use crate::engine::flow::{ConcreteItem, ConcreteStep};
use crate::engine::git::{
    commit, create_branch, current_branch, is_clean, push_with_upstream, stage_all,
};
use crate::engine::naming::{format_branch_name, generate_word_pair};
use crate::engine::prompt::{
    drop_native_instruction_docs, format_context_prompt, format_prompt, format_task_prompt,
    gather_context, trim_context_with_breakdown, write_prompt_log, Document, GatherContextOpts,
    DEFAULT_CONTEXT_BUDGET,
};
use crate::engine::worktree::remove_worktree;
use crate::engine::worktrees::{
    branch_exists, create_with_schema, schedule_upstream_sync, worktree_path as wave_worktree_path,
};

use crate::lfd::id::LfdId;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{
    Agent, AgentStatus, Wave, WaveRun, WaveRunKind, WaveRunSnapshot, WaveRunStatus, WaveStatus,
};

use super::CiFailure;

/// Create a wave run with a worktree and branch for the wave.
pub fn create_wave_run_with_id(
    store: &SharedStore,
    wave: &Wave,
    run_id: &LfdId,
) -> anyhow::Result<WaveRun> {
    let last_run = store
        .list_wave_runs(Some(&wave.id), Some(1))?
        .into_iter()
        .next();
    let iteration = last_run.map(|run| run.iteration + 1).unwrap_or(0);

    let main_repo = Path::new(&wave.repo);
    let (wt_path, branch) = ensure_wave_worktree(main_repo, &wave.name)?;

    let run = WaveRun {
        id: run_id.clone(),
        wave_id: wave.id.clone(),
        snapshot: WaveRunSnapshot {
            repo: wave.repo.clone(),
            flow: wave.flow.clone(),
            direction: wave.direction.clone(),
            area: wave.area.clone(),
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
        run_kind: WaveRunKind::Main,
        sidecar_kind: None,
    };
    store.create_wave_run(&run)?;
    if let Ok(Some(mut wave)) = store.get_wave(&wave.id) {
        wave.status = WaveStatus::Running;
        wave.iteration = iteration;
        if let Err(err) = store.update_wave(&wave) {
            warn!(wave_id = %wave.id, error = %err, "failed to set wave status to running");
        }
    }
    Ok(run)
}

/// Create a worktree for this wave, or reuse the existing one.
pub fn ensure_wave_worktree(main_repo: &Path, wave_name: &str) -> anyhow::Result<(String, String)> {
    let wt = wave_worktree_path(main_repo, wave_name);
    if wt.exists() {
        let branch = current_branch(&wt)?.unwrap_or_default();
        if !branch.is_empty() {
            schedule_upstream_sync(wt.clone(), branch.clone());
        }
        return Ok((wt.to_string_lossy().to_string(), branch));
    }

    let config = load_config(Some(main_repo)).ok().flatten();
    let branch_config = config.as_ref().and_then(|c| c.branch_names.as_ref());
    let result = create_with_schema(main_repo, wave_name, None, branch_config)?;
    Ok((result.path.to_string_lossy().to_string(), result.branch))
}

pub(crate) fn fork_worktree_path(run: &WaveRun, branch_index: u32) -> String {
    format!("{}-fork-{branch_index}", run.worktree)
}

pub(crate) fn ci_fix_worktree_path(repo: &Path, wave_name: &str, run_id: &LfdId) -> PathBuf {
    let repo_root =
        crate::engine::worktrees::main_repo_root(repo).unwrap_or_else(|_| repo.to_path_buf());
    let repo_name = repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("repo");
    let short_id = short_hash(run_id.as_str(), 8);
    repo_root
        .parent()
        .unwrap_or(repo_root.as_path())
        .join(format!("{repo_name}.{wave_name}.ci-fix.{short_id}"))
}

pub(crate) fn create_ci_fix_worktree(
    repo: &Path,
    worktree: &Path,
    source_branch: &str,
    temp_branch: &str,
) -> Result<()> {
    if worktree.exists() {
        cleanup_ci_fix_worktree(worktree)?;
    }

    let fetch = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["fetch", "origin", source_branch])
        .output()?;
    if !fetch.status.success() {
        return Err(anyhow!(
            "git fetch origin {source_branch} failed: {}",
            String::from_utf8_lossy(&fetch.stderr).trim()
        ));
    }

    let start_point = format!("origin/{source_branch}");
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(repo)
        .args([
            "worktree",
            "add",
            "-b",
            temp_branch,
            worktree.to_string_lossy().as_ref(),
            &start_point,
        ])
        .output()?;
    if !output.status.success() && !worktree.join(".git").exists() {
        return Err(anyhow!(
            "git worktree add failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

pub(crate) fn cleanup_ci_fix_worktree(worktree: &Path) -> Result<()> {
    if !worktree.exists() {
        return Ok(());
    }

    if worktree.join(".git").exists() {
        remove_worktree(worktree, true)?;
        return Ok(());
    }

    std::fs::remove_dir_all(worktree)?;
    Ok(())
}

pub(crate) fn commit_and_push_ci_fix(
    worktree: &Path,
    target_branch: &str,
    check_name: &str,
) -> Result<()> {
    if !is_clean(worktree)? {
        stage_all(worktree)?;
        let message = format!("lf debug: fix CI check {check_name}");
        commit(worktree, &message)?;
    }

    let refspec = format!("HEAD:{target_branch}");
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(worktree)
        .args(["push", "origin", &refspec])
        .output()?;
    if !output.status.success() {
        return Err(anyhow!(
            "git push origin {refspec} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

pub(crate) fn is_active_wave_run_status(status: WaveRunStatus) -> bool {
    matches!(
        status,
        WaveRunStatus::Pending | WaveRunStatus::Running | WaveRunStatus::Waiting
    )
}

pub(crate) fn is_ephemeral_worktree_path(path: &str) -> bool {
    path.contains("-fork-") || path.contains(".ci-fix.")
}

pub(crate) fn format_ci_failure_message(failure: &CiFailure) -> String {
    format!(
        "Fix this failing CI check.\n\
         - Check: {}\n\
         - PR number: {}\n\
         - Branch: {}\n\
         - Failing commit SHA: {}\n\
         - Logs URL: {}\n\
         \n\
         Reproduce the failure from this branch, apply a fix, run relevant checks, and leave the branch ready to push.",
        failure.check_name, failure.pr_number, failure.branch, failure.commit_sha, failure.logs_url
    )
}

pub(crate) fn merge_directions(base: &[String], extra: &[String]) -> Vec<String> {
    if extra.is_empty() {
        return base.to_vec();
    }
    let mut combined = base.to_vec();
    for item in extra {
        if !combined.contains(item) {
            combined.push(item.clone());
        }
    }
    combined
}

pub(crate) fn flow_parents_for_index(items: &[ConcreteItem], step_index: u32) -> Vec<String> {
    match items.get(step_index as usize) {
        Some(ConcreteItem::Step(step)) => step.flow_parents.clone(),
        Some(ConcreteItem::Fork(fork)) => fork.flow_parents.clone(),
        None => Vec::new(),
    }
}

pub(crate) fn build_step_prompt(
    worktree: &str,
    step: &ConcreteStep,
    directions: &[String],
    wave: Option<&str>,
    summary_source: Option<(&SharedStore, &LfdId)>,
    message: Option<String>,
) -> Result<(String, String, LaunchConfig)> {
    let config = load_config_or_default(Some(Path::new(worktree)));
    let directions = merge_directions(directions, &step.step.directions);
    let opts = GatherContextOpts {
        repo_root: PathBuf::from(worktree),
        step: Some(step.step.name.clone()),
        message,
        run_mode: Some("auto".to_string()),
        directions,
        files: Vec::new(),
        lfdocs: config.lfdocs,
        diff_files: config.diff_files,
        diff: config.diff,
        clipboard: config.paste,
        area: config.area.clone(),
        wave: wave.map(str::to_string),
    };

    let mut components = gather_context(&opts)?;
    let repo_root = PathBuf::from(worktree);
    drop_native_instruction_docs(&mut components, &repo_root);

    // Inject wave summary if available
    if let Some((store, wave_id)) = summary_source {
        if let Ok(Some(summary)) = store.get_summary(wave_id) {
            components.summaries.push(Document {
                path: "wave-summary".to_string(),
                content: summary.content,
                category: "summaries".to_string(),
            });
        }
    }
    let (components, _breakdown) = trim_context_with_breakdown(components, DEFAULT_CONTEXT_BUDGET);

    // Log full prompt, then write context/task split for --append-system-prompt-file
    let _ = write_prompt_log(
        &repo_root,
        &format_prompt(&components),
        &step.step.name,
        None,
    );
    let task_prompt = format_task_prompt(&components);
    let context_file = write_prompt_log(
        &repo_root,
        &format_context_prompt(&components),
        &format!("{}.context", step.step.name),
        None,
    )
    .ok();

    let model = step
        .step
        .model
        .clone()
        .unwrap_or_else(|| config.agent_model.clone());
    let launch = LaunchConfig {
        auto: true,
        stream: true,
        skip_permissions: config.yolo,
        model_variant: None,
        chrome: config.chrome,
        cwd: Some(repo_root),
        context_file,
        ..Default::default()
    };

    Ok((task_prompt, model, launch))
}

pub(crate) fn build_agent_for_step(
    wave_run_id: &LfdId,
    repo: &str,
    worktree: &str,
    step: &ConcreteStep,
    status: AgentStatus,
    model: &str,
) -> Agent {
    Agent {
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
        model: model.to_string(),
        run_mode: "auto".to_string(),
    }
}

/// Commit any remaining changes, push, and create a draft PR.
/// When `mark_ready` is true (auto-stimulus waves), converts the draft to a real PR.
/// Returns the PR info if successful, None if skipped or failed.
pub(crate) fn auto_create_pr(
    worktree: &Path,
    mark_ready: bool,
) -> Option<crate::lfd::types::PullRequest> {
    use crate::ops::{
        commit_workflow, current_pr, generate_pr_message, update_pr, CommitOptions, NullProgress,
    };

    let commit_options = CommitOptions {
        add: true,
        lint: false,
        push: true,
        create_draft_pr: true,
        task: "commit".to_string(),
        flow_parents: Vec::new(),
        message: None,
    };
    if let Err(err) = commit_workflow(worktree, &commit_options, &NullProgress) {
        warn!(worktree = %worktree.display(), error = %err, "auto-create PR: commit/push failed");
        return None;
    }

    match current_pr(worktree) {
        Ok(Some(pr)) => {
            let mut title = None;

            // Update the draft PR with an LLM-generated title and description,
            // matching what `lf ops pr` produces.
            match generate_pr_message(worktree) {
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

            // Auto-stimulus waves get their draft promoted to a real PR.
            if mark_ready {
                if let Err(err) = crate::ops::mark_ready(worktree) {
                    warn!(worktree = %worktree.display(), error = %err, "auto-create PR: failed to mark PR ready");
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

pub(crate) fn short_hash(value: &str, chars: usize) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut hash = hex::encode(digest);
    hash.truncate(chars);
    hash
}
