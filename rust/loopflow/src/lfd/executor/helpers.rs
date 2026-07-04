use std::path::Path;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use sha2::{Digest, Sha256};
use tracing::{debug, error, info, warn};

use time::OffsetDateTime;

use crate::engine::config::{load_config, load_config_or_default};
use crate::engine::flow::ConcreteStep;
use crate::engine::git::{
    create_branch, current_branch, fetch, get_default_branch, push_with_upstream, rev_parse,
    sync_main, worktree_add, WorktreeBranch,
};
use crate::engine::naming::{format_branch_name, generate_word_pair};
use crate::engine::worktrees::{
    branch_exists, create_with_schema_synced, schedule_upstream_sync,
    worktree_path as wave_worktree_path,
};

use crate::lfd::id::LfdId;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{
    ExecutionProcess, ExecutionProcessStatus, Run, RunStackStatus, RunStatus, Wave, WaveStatus,
};
use crate::ops::{rebase_with_recovery, Progress, RebaseOptions};

/// Where a dispatched run's work happens on disk.
///
/// Every dispatch names its placement explicitly — there is no implicit
/// shared-vs-per-run heuristic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// New `<repo>.<wave>.<short-run-id>` worktree off the default branch;
    /// independent branch, independent PR, independent land.
    Fresh,
    /// Reuse the wave's shared `<repo>.<wave>` worktree. Pooled runs share
    /// one branch; concurrent pooled dispatches can collide — pooling is a
    /// conscious opt-in.
    Pool,
    /// New worktree whose branch forks from the parent run's branch, for
    /// dependent series. Branch lineage carries the stack; the filesystem
    /// stays flat.
    Stack { parent_run_id: LfdId },
}

/// Create a run with a worktree and branch chosen by `placement`.
pub async fn create_run_for_placement(
    store: &SharedStore,
    wave: &Wave,
    run_id: &LfdId,
    placement: &Placement,
    target_branch: Option<&str>,
) -> anyhow::Result<Run> {
    let stack_runs = store.list_stack_runs(wave.id()).await?;
    let last_run = stack_runs.last().cloned();
    let iteration = last_run.as_ref().map(|run| run.iteration + 1).unwrap_or(0);

    // Lineage: stacked runs fork from their named parent; fresh and pooled
    // runs continue the wave's linear chain (the existing land queue order).
    let lineage_parent = match placement {
        Placement::Stack { parent_run_id } => Some(
            store
                .get_run(parent_run_id)
                .await?
                .ok_or_else(|| anyhow!("stack parent run not found: {parent_run_id}"))?,
        ),
        Placement::Fresh | Placement::Pool => last_run,
    };
    let stack_position = lineage_parent
        .as_ref()
        .map(|run| run.stack_position + 1)
        .unwrap_or(0);
    let parent_run_id = lineage_parent.as_ref().map(|run| run.id.clone());
    let parent_pr_number = lineage_parent
        .as_ref()
        .and_then(|run| run.pr.as_ref())
        .and_then(|pr| pr.number);
    let stack_group_id = lineage_parent
        .as_ref()
        .map(|run| run.stack_group_id.clone())
        .unwrap_or_else(|| wave.id().to_string());

    let repo_work = wave
        .repos
        .first()
        .expect("wave always has at least one RepoWork");
    let main_repo = Path::new(&repo_work.repo);

    // Targeted activations (non-main branch, e.g. CI fixes on a PR branch)
    // always get their own worktree tracking that branch, since the wave's
    // shared worktree is on a different branch.
    let is_targeted = target_branch
        .map(|b| !b.is_empty() && b != "main")
        .unwrap_or(false);
    let ((wt_path, branch), run_target_branch) = match placement {
        Placement::Pool if !is_targeted => (
            ensure_wave_worktree(main_repo, wave.name())?,
            target_branch.unwrap_or("main").to_string(),
        ),
        Placement::Fresh | Placement::Pool => (
            create_run_worktree(main_repo, wave.name(), run_id.as_str(), target_branch)?,
            target_branch.unwrap_or("main").to_string(),
        ),
        Placement::Stack { .. } => {
            let parent = lineage_parent
                .as_ref()
                .expect("stack placement resolved its parent above");
            (
                create_stacked_run_worktree(
                    main_repo,
                    wave.name(),
                    run_id.as_str(),
                    &parent.branch,
                )?,
                parent.branch.clone(),
            )
        }
    };

    let run = Run {
        id: run_id.clone(),
        wave_id: wave.id().clone(),
        repo: repo_work.repo.clone(),
        flow: wave.primary_flow().clone(),
        task: None,
        direction: wave.direction().clone(),
        area: wave.area().clone(),
        iteration,
        step_index: 0,
        status: RunStatus::Running,
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
        stack_status: RunStackStatus::Active,
        lineage_inferred: false,
        target_branch: run_target_branch,
        repair_of: None,
        pr: None,
    };
    store.create_run(&run).await?;
    if let Ok(Some(mut wave)) = store.get_wave(wave.id()).await {
        if let Some(rw) = wave.repo_work_mut(&run.repo) {
            // New cycle: record the starting iteration for max_iterations safety valve.
            if rw.status == WaveStatus::Idle || rw.status == WaveStatus::Paused {
                rw.cycle_start_iteration = iteration;
            }
            rw.status = WaveStatus::Running;
            rw.iteration = iteration;
        }
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

/// Short run id: the leading 8 hex chars of the run's UUID, tying the
/// worktree directory to its Run row.
pub(crate) fn short_run_id(run_id: &str) -> String {
    let hex: String = run_id
        .chars()
        .filter(|ch| ch.is_ascii_hexdigit())
        .take(8)
        .collect();
    if hex.len() == 8 {
        hex
    } else {
        short_hash(run_id, 8)
    }
}

/// Sibling worktree for a wave-dispatched worker: `<repo>.<wave>.<short-run-id>`.
///
/// Segment count carries ownership: two segments (`<repo>.<name>`) is a human
/// or wave worktree, three is a dispatched worker tied to a Run row.
pub(crate) fn run_worktree_path(main_repo: &Path, wave_name: &str, run_id: &str) -> PathBuf {
    let base_wt = wave_worktree_path(main_repo, wave_name);
    let base_name = base_wt
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("wave");
    base_wt.with_file_name(format!("{base_name}.{}", short_run_id(run_id)))
}

/// Generate a schema-formatted branch for the wave, de-colliding with word pairs.
fn unique_wave_branch(main_repo: &Path, wave_name: &str) -> anyhow::Result<String> {
    let config = load_config_or_default(Some(main_repo));
    let branch_config = config.branch_names.as_ref();
    let mut branch = format_branch_name(wave_name, branch_config, main_repo)
        .map_err(|e| anyhow!("failed to generate branch name: {e}"))?;
    while branch_exists(main_repo, &branch)? {
        branch = format!("{branch}.{}", generate_word_pair());
    }
    Ok(branch)
}

/// Create a run-scoped worktree at `<repo>.<wave>.<short-run-id>`.
///
/// Fresh placement: the worktree gets its own branch forked from the default
/// branch — independent PR, independent land.
///
/// When `target_branch` is `Some` and not `"main"`, the worktree instead
/// tracks that branch directly (e.g. a PR branch for CI-fix activations).
pub fn create_run_worktree(
    main_repo: &Path,
    wave_name: &str,
    run_id: &str,
    target_branch: Option<&str>,
) -> anyhow::Result<(String, String)> {
    let run_wt = run_worktree_path(main_repo, wave_name, run_id);

    let is_targeted = target_branch
        .map(|b| !b.is_empty() && b != "main")
        .unwrap_or(false);
    if is_targeted {
        // Targeted activation: track the specified branch directly through a
        // run-local branch, so git push from the worktree lands on it.
        let tb = target_branch.expect("checked above");
        fetch(main_repo, "origin", tb)?;
        let run_branch = format!("{tb}-run-{}", short_run_id(run_id));
        let remote_ref = format!("origin/{tb}");
        worktree_add(
            main_repo,
            &run_wt,
            &run_branch,
            WorktreeBranch::Track {
                remote: &remote_ref,
            },
        )?;
        sync_existing_worktree(main_repo, &run_wt, &run_branch)?;
        // Return the target branch (not the run-local branch) so the run
        // record tracks which remote branch it pushes to.
        return Ok((run_wt.to_string_lossy().to_string(), tb.to_string()));
    }

    // Fresh: own branch off the default branch.
    let default_branch = get_default_branch(main_repo)?;
    let _ = sync_main(main_repo, &default_branch);
    let branch = unique_wave_branch(main_repo, wave_name)?;
    worktree_add(
        main_repo,
        &run_wt,
        &branch,
        WorktreeBranch::New {
            start_point: &default_branch,
        },
    )?;
    schedule_upstream_sync(run_wt.clone(), branch.clone());
    Ok((run_wt.to_string_lossy().to_string(), branch))
}

/// Create a run-scoped worktree whose branch forks from `parent_branch`.
///
/// Stack placement: the new branch starts at the parent run's branch tip
/// (remote tip when available), so dependent work builds on unlanded work.
pub fn create_stacked_run_worktree(
    main_repo: &Path,
    wave_name: &str,
    run_id: &str,
    parent_branch: &str,
) -> anyhow::Result<(String, String)> {
    let run_wt = run_worktree_path(main_repo, wave_name, run_id);

    let _ = fetch(main_repo, "origin", parent_branch);
    let remote_ref = format!("origin/{parent_branch}");
    let start_point = if rev_parse(main_repo, &remote_ref).is_ok() {
        remote_ref
    } else if branch_exists(main_repo, parent_branch)? {
        parent_branch.to_string()
    } else {
        return Err(anyhow!("stack parent branch not found: {parent_branch}"));
    };

    let branch = unique_wave_branch(main_repo, wave_name)?;
    worktree_add(
        main_repo,
        &run_wt,
        &branch,
        WorktreeBranch::New {
            start_point: &start_point,
        },
    )?;
    schedule_upstream_sync(run_wt.clone(), branch.clone());
    Ok((run_wt.to_string_lossy().to_string(), branch))
}

pub(crate) fn is_active_run_status(status: RunStatus) -> bool {
    matches!(
        status,
        RunStatus::Pending | RunStatus::Running | RunStatus::Waiting
    )
}

pub(crate) fn is_ephemeral_worktree_path(path: &str) -> bool {
    let worktree_name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);
    has_fork_suffix(worktree_name)
        || has_run_suffix(worktree_name)
        || has_run_id_segment(worktree_name)
}

/// `<repo>.<wave>.<short-run-id>` — a wave-dispatched worker worktree.
///
/// Three or more dot segments ending in exactly 8 hex chars. Two-segment
/// `<repo>.<name>` human/wave worktrees and preserved `<name>.<timestamp>`
/// trees (13-char `YYYYMMDD_HHMM` suffix) never match.
fn has_run_id_segment(path_component: &str) -> bool {
    let Some((prefix, suffix)) = path_component.rsplit_once('.') else {
        return false;
    };
    prefix.contains('.') && suffix.len() == 8 && suffix.chars().all(|ch| ch.is_ascii_hexdigit())
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

pub(crate) fn build_agent_for_step(
    run_id: &LfdId,
    repo: &str,
    worktree: &str,
    step: &ConcreteStep,
    status: ExecutionProcessStatus,
    agent: &str,
) -> ExecutionProcess {
    ExecutionProcess {
        id: LfdId::new(),
        step: step.step.name.clone(),
        repo: repo.to_string(),
        worktree: worktree.to_string(),
        run_id: Some(run_id.clone()),
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
    docs: &[String],
    wave_name: &str,
) -> Vec<String> {
    let mut cmd = vec![
        resolve_lf_binary().to_string_lossy().to_string(),
        step_name.to_string(),
    ];
    append_lf_run_options(&mut cmd, batch, directions, docs, wave_name);
    cmd
}

/// The dispatch form of an `lf` invocation: `lf <flow>: <task>` plus the
/// standard run options. One builder for every dispatcher — the lfd executor
/// and `lf q worker run` must launch byte-identical workers.
pub(crate) fn build_lf_dispatch_command(
    flow: &str,
    task: &str,
    directions: &[String],
    docs: &[String],
    wave_name: &str,
) -> Vec<String> {
    let mut cmd = build_lf_step_command(flow, true, directions, docs, wave_name);
    cmd[1].push(':');
    cmd.push(task.to_string());
    cmd
}

/// Shell-quote one argv element for the tmux launch line.
pub(crate) fn shell_escape(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

/// Where a tmux-wrapped session records its exit code (read by whoever
/// reconciles the session — the lfd executor's watcher today).
pub(crate) fn tmux_exit_file(cwd: &Path, session_id: &LfdId) -> PathBuf {
    cwd.join(".lf/tmp/sessions")
        .join(format!("{session_id}.exit"))
}

pub(crate) fn build_lf_inline_command(
    prompt: &str,
    batch: bool,
    directions: &[String],
    docs: &[String],
    wave_name: &str,
) -> Vec<String> {
    let mut cmd = vec![resolve_lf_binary().to_string_lossy().to_string()];
    append_lf_run_options(&mut cmd, batch, directions, docs, wave_name);
    cmd.push(":".to_string());
    cmd.push(prompt.to_string());
    cmd
}

fn append_lf_run_options(
    cmd: &mut Vec<String>,
    batch: bool,
    directions: &[String],
    docs: &[String],
    wave_name: &str,
) {
    if batch {
        cmd.push("-b".to_string());
    }
    cmd.push("--no-direction".to_string());
    for direction in directions {
        cmd.push("-d".to_string());
        cmd.push(direction.clone());
    }
    for target in docs {
        cmd.push("--docs".to_string());
        cmd.push(target.clone());
    }
    cmd.push("-w".to_string());
    cmd.push(wave_name.to_string());
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

/// Clean up a run-scoped worktree after the run completes.
///
/// Only removes per-run worktrees (`<repo>.<wave>.<short-run-id>`, or the
/// legacy `-run-` suffix) — never shared wave or human worktrees.
pub(crate) fn cleanup_run_worktree(worktree: &Path) -> Result<()> {
    let name = worktree.file_name().and_then(|n| n.to_str()).unwrap_or("");
    if !has_run_suffix(name) && !has_run_id_segment(name) {
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

    fn warning(&self, msg: &str) {
        warn!("{msg}");
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

    use super::{ensure_wave_worktree, is_ephemeral_worktree_path};
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
    fn is_ephemeral_worktree_path_detects_run_id_segment() {
        // <repo>.<wave>.<short-run-id> — a wave-dispatched worker worktree.
        assert!(is_ephemeral_worktree_path("/tmp/repo.wave.a1b2c3d4"));
        // Two segments = human or wave worktree.
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave"));
        assert!(!is_ephemeral_worktree_path("/tmp/repo.feature"));
        // Preserved worktrees carry a timestamp suffix, not a run id.
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave.20260703_1511"));
        // Non-hex or wrong-length final segments.
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave.release"));
        assert!(!is_ephemeral_worktree_path("/tmp/repo.wave.a1b2"));
    }

    #[test]
    fn cleanup_run_worktree_removes_worker_tree_and_spares_named_trees() {
        let (_temp, main_repo, _origin) = setup_repo_with_remote();
        let parent = main_repo.parent().expect("main repo parent");

        let worker = parent.join("main.wave.a1b2c3d4");
        run_git(
            &main_repo,
            &[
                "worktree",
                "add",
                "-b",
                "worker-branch",
                worker.to_str().unwrap(),
                "main",
            ],
        );
        let human = parent.join("main.feature");
        run_git(
            &main_repo,
            &[
                "worktree",
                "add",
                "-b",
                "feature-branch",
                human.to_str().unwrap(),
                "main",
            ],
        );

        super::cleanup_run_worktree(&worker).expect("remove worker tree");
        super::cleanup_run_worktree(&human).expect("no-op on human tree");

        assert!(!worker.exists(), "worker worktree should be removed");
        assert!(human.exists(), "human worktree must be left alone");
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
