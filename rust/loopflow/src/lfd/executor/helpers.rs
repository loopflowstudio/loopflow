use std::path::Path;
use std::path::PathBuf;

use anyhow::{anyhow, Result};
use tracing::{error, info, warn};

use time::OffsetDateTime;

use crate::engine::config::load_config_or_default;
use crate::engine::git::{
    fetch, get_default_branch, is_ancestor, rev_parse, sync_main, worktree_add, WorktreeBranch,
};
use crate::engine::naming::{format_branch_name, generate_word_pair};
use crate::engine::worktrees::{
    branch_exists, ensure_wave_worktree as ensure_wave_worktree_lease, run_worktree_path,
    schedule_upstream_sync, short_run_id,
};

use crate::lfd::id::LfdId;
use crate::lfd::types::{Run, RunStackStatus, RunStatus, Session, Wave, WaveStatus};
use crate::lfdb::SharedStore;
use crate::ops::{rebase_with_recovery, Progress, RebaseOptions};

// TODO(M1): extract dispatch into crate::dispatch. lfd/executor should not own
// placement, worktree creation, tmux launch wiring, or the run env contract.
/// Where a dispatched run's work happens on disk.
///
/// Every dispatch names its placement explicitly — there is no implicit
/// shared-vs-per-run heuristic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// New `<repo>.<wave>.<short-run-id>` worktree off the default branch;
    /// independent branch, independent PR, independent land.
    Fresh,
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

    // Lineage: stacked runs fork from their named parent; fresh
    // runs continue the wave's linear chain (the existing land queue order).
    let lineage_parent = match placement {
        Placement::Stack { parent_run_id } => Some(
            store
                .get_run(parent_run_id)
                .await?
                .ok_or_else(|| anyhow!("stack parent run not found: {parent_run_id}"))?,
        ),
        Placement::Fresh => last_run,
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

    let main_repo = Path::new(&wave.repo);

    let ((wt_path, branch), run_target_branch) = match placement {
        Placement::Fresh => (
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
        repo: wave.repo.clone(),
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

/// Create a worktree for this wave, or reuse the existing one.
pub fn ensure_wave_worktree(main_repo: &Path, wave_name: &str) -> anyhow::Result<(String, String)> {
    let lease = ensure_wave_worktree_lease(main_repo, wave_name)?;
    Ok((lease.path.to_string_lossy().to_string(), lease.branch))
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
/// tracks that branch directly (e.g. a fix dispatched onto a PR branch).
pub(crate) fn create_run_worktree(
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
        // Targeted dispatch: track the specified branch directly through a
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
pub(crate) fn create_stacked_run_worktree(
    main_repo: &Path,
    wave_name: &str,
    run_id: &str,
    parent_branch: &str,
) -> anyhow::Result<(String, String)> {
    let run_wt = run_worktree_path(main_repo, wave_name, run_id);

    let _ = fetch(main_repo, "origin", parent_branch);
    let remote_ref = format!("origin/{parent_branch}");
    let local_exists = branch_exists(main_repo, parent_branch)?;
    let remote_exists = rev_parse(main_repo, &remote_ref).is_ok();
    // Fork from the freshest tip: the parent run's unpushed local commits
    // must reach the stack, so the local branch wins unless it is strictly
    // behind the remote (absent, or all its commits already on origin).
    let start_point = match (local_exists, remote_exists) {
        (false, false) => {
            return Err(anyhow!("stack parent branch not found: {parent_branch}"));
        }
        (true, false) => parent_branch.to_string(),
        (false, true) => remote_ref,
        (true, true) => {
            if local_strictly_behind(main_repo, parent_branch, &remote_ref)? {
                remote_ref
            } else {
                parent_branch.to_string()
            }
        }
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

/// Whether `local` is strictly behind `remote_ref`: the remote has commits
/// the local branch lacks and the local branch has none of its own. Equal,
/// ahead, or diverged all read as "not behind" — the local tip carries
/// everything the remote does (or more).
fn local_strictly_behind(repo: &Path, local: &str, remote_ref: &str) -> anyhow::Result<bool> {
    let local_sha = rev_parse(repo, local)?;
    let remote_sha = rev_parse(repo, remote_ref)?;
    if local_sha == remote_sha {
        return Ok(false);
    }
    Ok(is_ancestor(repo, &local_sha, &remote_sha)?)
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

/// Shell-quote one argv element for the tmux launch line.
pub(crate) fn shell_escape(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

/// Whether a tmux session with this name exists (`tmux has-session` probe).
/// Shared by the executor's session reconciliation and the wave registry's
/// worker-liveness observer.
pub(crate) async fn tmux_session_exists(session_name: &str) -> Result<bool> {
    let status = tokio::process::Command::new("tmux")
        .args(["has-session", "-t", session_name])
        .status()
        .await
        .map_err(|err| anyhow!("tmux session probe failed: {err}"))?;
    Ok(status.success())
}

/// Where a tmux-wrapped session records its exit code (read by whoever
/// reconciles the session — the lfd executor's watcher today).
pub(crate) fn tmux_exit_file(cwd: &Path, session_id: &LfdId) -> PathBuf {
    cwd.join(".lf/tmp/sessions")
        .join(format!("{session_id}.exit"))
}

/// Wrapper tail for run-to-completion sessions: propagate the exit code.
pub(crate) const TMUX_EXIT_TAIL: &str = r#"exit "$EXIT_CODE""#;

/// The exit-file shell wrapper every tmux-backed session runs — the single
/// authoring site of the exit-file wire contract.
///
/// Two invariants live here:
/// - `unset LFD_SESSION_INHERITED` first: a fresh tmux server inherits the
///   dispatcher's environment (verified empirically), so a dispatcher that is
///   itself a registered session would leak `LFD_SESSION_INHERITED=1` into
///   the worker's login shell. That flips `classify_run_context` from
///   OwnSession to NeedsRegistration and the worker registers a duplicate row
///   instead of adopting the one the dispatcher created. The session's own
///   env contract is re-exported explicitly by the inline prefix.
/// - The exit code lands in the session's exit file so whoever reconciles
///   the row (a running lfd, or the next boot) can close it.
pub(crate) fn tmux_shell_command(session: &Session, tail: &str) -> String {
    let exit_file = tmux_exit_file(Path::new(&session.cwd), &session.id);
    let exit_dir = exit_file
        .parent()
        .expect("tmux exit file always has a parent");
    let env_prefix = session
        .env
        .iter()
        .map(|(key, value)| format!("{key}={} ", shell_escape(value)))
        .collect::<String>();
    let command = session
        .argv
        .iter()
        .map(|arg| shell_escape(arg))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "unset LFD_SESSION_INHERITED; mkdir -p {exit_dir}; rm -f {exit_file}; {env_prefix}{command}; EXIT_CODE=$?; printf '%s' \"$EXIT_CODE\" > {exit_file}; {tail}",
        exit_dir = shell_escape(&exit_dir.display().to_string()),
        exit_file = shell_escape(&exit_file.display().to_string()),
    )
}

/// Launch `session` detached in tmux running the exit-file wrapper, then
/// enable mouse mode so scroll events reach tmux rather than the inner shell.
/// Callers own the session row's lifecycle (start on `Ok`, fail on `Err`).
pub(crate) async fn launch_session_in_tmux(session: &Session, tail: &str) -> Result<()> {
    let shell_command = tmux_shell_command(session, tail);
    let status = tokio::process::Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            &session.tmux_name,
            "-c",
            &session.cwd,
            "/bin/zsh",
            "-lc",
            &shell_command,
        ])
        .status()
        .await
        .map_err(|err| anyhow!("tmux failed to spawn: {err}"))?;
    if !status.success() {
        return Err(anyhow!("tmux failed to launch terminal session"));
    }
    let _ = tokio::process::Command::new("tmux")
        .args(["set-option", "-t", &session.tmux_name, "mouse", "on"])
        .status()
        .await;
    Ok(())
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
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::process::Command;

    use tempfile::TempDir;
    use time::OffsetDateTime;

    use super::{
        create_stacked_run_worktree, ensure_wave_worktree, is_ephemeral_worktree_path,
        tmux_shell_command, TMUX_EXIT_TAIL,
    };
    use crate::engine::worktrees::worktree_path as wave_worktree_path;
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{Session, SessionStatus, SessionUse, TMUX_TERMINAL_SOURCE};

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

    /// The exit-file wire contract, pinned. Tmux sessions run through this
    /// one wrapper: it clears the inherited-session marker the tmux server
    /// leaks from the dispatcher's environment, exports the session env
    /// inline, and records the exit code in the session's exit file.
    #[test]
    fn tmux_shell_command_unsets_inherited_marker_and_wires_exit_file() {
        let session_id = LfdId::new();
        let session = Session {
            id: session_id.clone(),
            wave_id: LfdId::new(),
            run_id: None,
            parent_session_id: None,
            session_use: SessionUse::Worker,
            step: "dispatch:implement".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo.wave.a1b2c3d4".to_string(),
            argv: vec![
                "lf".to_string(),
                "implement:".to_string(),
                "Do it".to_string(),
            ],
            env: BTreeMap::from([
                ("LFD_SESSION_ID".to_string(), session_id.to_string()),
                ("LFD_AGENT_ROLE".to_string(), "worker".to_string()),
            ]),
            source: TMUX_TERMINAL_SOURCE.to_string(),
            tmux_name: "lf-test".to_string(),
            status: SessionStatus::Pending,
            attached_at: None,
            started_at: None,
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        };

        let wrapper = tmux_shell_command(&session, TMUX_EXIT_TAIL);

        assert!(
            wrapper.starts_with("unset LFD_SESSION_INHERITED; "),
            "the wrapper must clear the marker a fresh tmux server inherits \
             from the dispatcher, or workers register duplicate rows: {wrapper}"
        );
        assert!(
            wrapper.contains(&format!("LFD_SESSION_ID='{session_id}' ")),
            "the session env is re-exported inline: {wrapper}"
        );
        assert!(
            wrapper.contains(&format!(
                "'/tmp/repo.wave.a1b2c3d4/.lf/tmp/sessions/{session_id}.exit'"
            )),
            "exit code lands in the session's exit file: {wrapper}"
        );
        assert!(wrapper.ends_with(TMUX_EXIT_TAIL));
    }

    #[test]
    fn stacked_worktree_forks_from_local_parent_when_ahead_of_origin() {
        let (_temp, main_repo, _origin) = setup_repo_with_remote();

        // Parent branch pushed, then one MORE local commit never pushed.
        run_git(&main_repo, &["checkout", "-b", "parent-branch"]);
        write_file(&main_repo.join("pushed.txt"), "pushed\n");
        run_git(&main_repo, &["add", "."]);
        run_git(&main_repo, &["commit", "-m", "pushed work"]);
        run_git(&main_repo, &["push", "-u", "origin", "parent-branch"]);
        write_file(&main_repo.join("local-only.txt"), "local only\n");
        run_git(&main_repo, &["add", "."]);
        run_git(&main_repo, &["commit", "-m", "local-only work"]);
        run_git(&main_repo, &["checkout", "main"]);

        let (worktree, _branch) =
            create_stacked_run_worktree(&main_repo, "wave", "a1b2c3d4e5f6", "parent-branch")
                .expect("stacked worktree");

        assert!(
            Path::new(&worktree).join("local-only.txt").exists(),
            "the stack must fork from the local tip when it is ahead of origin"
        );
    }

    #[test]
    fn stacked_worktree_forks_from_origin_when_local_is_behind() {
        let (_temp, main_repo, origin) = setup_repo_with_remote();

        run_git(&main_repo, &["checkout", "-b", "parent-branch"]);
        write_file(&main_repo.join("pushed.txt"), "pushed\n");
        run_git(&main_repo, &["add", "."]);
        run_git(&main_repo, &["commit", "-m", "pushed work"]);
        run_git(&main_repo, &["push", "-u", "origin", "parent-branch"]);
        run_git(&main_repo, &["checkout", "main"]);

        // A collaborator advances the branch on origin; local is now behind.
        let collaborator = main_repo
            .parent()
            .expect("main repo parent")
            .join("collaborator-stack");
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
        run_git(&collaborator, &["checkout", "parent-branch"]);
        write_file(&collaborator.join("remote-only.txt"), "remote only\n");
        run_git(&collaborator, &["add", "."]);
        run_git(&collaborator, &["commit", "-m", "remote-only work"]);
        run_git(&collaborator, &["push"]);

        let (worktree, _branch) =
            create_stacked_run_worktree(&main_repo, "wave", "b2c3d4e5f6a1", "parent-branch")
                .expect("stacked worktree");

        assert!(
            Path::new(&worktree).join("remote-only.txt").exists(),
            "a strictly-behind local branch must yield to origin's tip"
        );
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
