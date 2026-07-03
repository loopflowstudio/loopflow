use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use serde::Deserialize;

use crate::engine::config::{load_config_or_default, BranchNameConfig};
use crate::engine::git::{get_default_branch, worktree_add, WorktreeBranch};
use crate::engine::naming::format_branch_name;
use crate::engine::worktrees::branch_exists;
use crate::engine::worktrees::{main_repo_root, worktree_path};
use crate::engine::{
    available_flow_names, load_goal, parse_agent, prepare_goal_launch, render_goal,
    GoalRenderContext, InFlightDispatch,
};
use crate::lf::commands::util::{find_repo_root, launch_session};
use crate::lf::Cli;
use crate::lfd::client::{authorize, blocking_client, resolve_base_url};
use crate::lfd::http::dto::RunDto;
use crate::lfd::http::routes::wave_config::read_wave_config;
use crate::lfd::types::tmux_session_name;
use crate::ops::util::resolve_wave_name;

const IN_FLIGHT_FETCH_TIMEOUT: Duration = Duration::from_secs(5);

/// Launch a wave's goal as a looping top-level agent (`operate` on, interactive
/// surface). The agent dispatches real work via `lfq worker run`.
pub fn run(name: &str, once: bool, tmux: bool, cli: &Cli) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root).unwrap_or(repo_root);
    let wave_name = resolve_wave_name(&main_repo, Some(name))
        .ok_or_else(|| anyhow!("invalid wave name: '{name}'"))?;

    if tmux {
        return launch_in_tmux(&main_repo, &wave_name, once, cli);
    }

    let in_flight = fetch_in_flight_dispatches(&wave_name);
    let message = build_goal_message(&main_repo, &wave_name, once, in_flight)?;

    let config = load_config_or_default(Some(&main_repo));
    let prepared = prepare_goal_launch(
        &config,
        main_repo.clone(),
        message,
        cli.model.clone(),
        cli.yolo || config.yolo,
    )?;

    let agent = prepared
        .config
        .agent
        .clone()
        .expect("prepare_launch_prompt always sets agent");
    let (harness, model) = parse_agent(&agent);

    launch_session(
        config.session.launch,
        &harness,
        model.as_deref(),
        &main_repo,
        &prepared.prompt,
    )
}

/// Spawn `lf goal <wave>` in a detached tmux session and print its handle.
///
/// This is the launch primitive Concerto uses without lfd: it runs the goal
/// loop in a background tmux session and prints the session name, which the
/// client attaches to with `tmux attach`. Idempotent — re-running against a
/// live session just reprints the handle so a re-click re-attaches.
fn launch_in_tmux(main_repo: &Path, wave_name: &str, once: bool, cli: &Cli) -> Result<()> {
    let worktree = worktree_path(main_repo, wave_name);
    if !worktree.exists() {
        let branch = stable_wave_branch_name(main_repo, wave_name)?;
        create_goal_worktree(main_repo, &worktree, &branch).with_context(|| {
            format!(
                "failed to create worktree '{}' on branch '{}'",
                worktree.display(),
                branch
            )
        })?;
    }

    let handle = wave_worktree_session_handle(&worktree);

    if tmux_has_session(&handle) {
        println!("{handle}");
        return Ok(());
    }

    let lf_bin = std::env::current_exe()
        .map_err(|err| anyhow!("cannot resolve the lf binary path: {err}"))?;
    let mut inner = vec![
        lf_bin.to_string_lossy().into_owned(),
        "goal".to_string(),
        wave_name.to_string(),
    ];
    if once {
        inner.push("--once".to_string());
    }
    if let Some(model) = &cli.model {
        inner.push("-m".to_string());
        inner.push(model.clone());
    }
    if cli.yolo {
        inner.push("--yolo".to_string());
    }
    // Run under a login shell so the agent inherits the user's PATH/env, matching
    // how lfd launches its tmux sessions.
    let inner_cmd = inner
        .iter()
        .map(|arg| sh_quote(arg))
        .collect::<Vec<_>>()
        .join(" ");

    // Detach tmux's stdio from ours. A cold `new-session` forks the tmux server,
    // which would otherwise inherit our stdout pipe and hold it open for the life
    // of the detached session — a parent reading our stdout to EOF (e.g. Concerto's
    // `readDataToEndOfFile`) would then block forever waiting for the handle. With
    // the server's stdio pointed at /dev/null, our `println!` below is the only
    // writer, so EOF arrives the moment we exit.
    let status = Command::new("tmux")
        .args([
            "new-session",
            "-d",
            "-s",
            &handle,
            "-c",
            &worktree.to_string_lossy(),
            "/bin/zsh",
            "-lc",
            &inner_cmd,
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| anyhow!("failed to run tmux: {err}"))?;
    if !status.success() {
        return Err(anyhow!("tmux failed to launch goal session '{wave_name}'"));
    }

    // Match lfd: let scroll reach tmux rather than the inner shell.
    let _ = Command::new("tmux")
        .args(["set-option", "-t", &handle, "mouse", "on"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    println!("{handle}");
    Ok(())
}

fn create_goal_worktree(main_repo: &Path, worktree: &Path, branch: &str) -> Result<()> {
    if branch_exists(main_repo, branch)
        .map_err(|err| anyhow!("failed to check branch '{branch}': {err}"))?
    {
        return worktree_add(main_repo, worktree, branch, WorktreeBranch::Existing)
            .map_err(|err| anyhow!("failed to add existing branch '{branch}' as worktree: {err}"));
    }

    let default_branch = get_default_branch(main_repo)
        .map_err(|err| anyhow!("failed to resolve default branch: {err}"))?;
    worktree_add(
        main_repo,
        worktree,
        branch,
        WorktreeBranch::New {
            start_point: default_branch.as_str(),
        },
    )
    .map_err(|err| anyhow!("failed to add new branch '{branch}' as worktree: {err}"))
}

fn stable_wave_branch_name(main_repo: &Path, wave_name: &str) -> Result<String> {
    let stable_schema = BranchNameConfig {
        schema_: "{user}.{name}".to_string(),
    };
    format_branch_name(wave_name, Some(&stable_schema), main_repo)
        .map_err(|err| anyhow!("failed to format branch name for wave '{wave_name}': {err}"))
}

fn wave_worktree_session_handle(worktree: &Path) -> String {
    let worktree_name = worktree
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("repo");
    tmux_session_name(worktree_name)
}

fn tmux_has_session(name: &str) -> bool {
    Command::new("tmux")
        .args(["has-session", "-t", name])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

/// Single-quote a string for safe interpolation into a shell command.
fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

/// Render the goal prompt: the wave's `GOAL.md` body plus flows, roadmap
/// handle, metrics, memory, and in-flight dispatches.
fn build_goal_message(
    repo: &Path,
    wave_name: &str,
    once: bool,
    in_flight: Vec<InFlightDispatch>,
) -> Result<String> {
    let goal = load_goal(wave_name, repo)
        .map_err(|err| anyhow!("failed to load goal for wave '{wave_name}': {err}"))?;
    let wave_config = read_wave_config(repo, wave_name).unwrap_or_default();
    let memory = std::fs::read_to_string(repo.join("wave").join(wave_name).join("MEMORY.md"))
        .unwrap_or_default();

    let ctx = GoalRenderContext {
        flows: available_flow_names(repo),
        roadmap: wave_config.roadmap.unwrap_or_default(),
        memory,
        metrics: wave_config.metrics.unwrap_or_default(),
        in_flight,
    };

    let mut message = render_goal(&goal, &ctx);
    if once {
        message.push_str(
            "\n\n<lf:goal-once>\nRun a single loop iteration, then stop and summarize.\n</lf:goal-once>",
        );
    }
    Ok(message)
}

/// Best-effort fetch of the wave's open dispatches from a running `lfd`.
/// Returns an empty list if `lfd` isn't reachable — the goal loop starts
/// with no known in-flight work rather than failing to launch.
fn fetch_in_flight_dispatches(wave: &str) -> Vec<InFlightDispatch> {
    let Ok(client) = blocking_client(IN_FLIGHT_FETCH_TIMEOUT) else {
        return Vec::new();
    };
    let url = format!("{}/v0/waves/{wave}/runs", resolve_base_url());
    let Ok(response) = authorize(client.get(&url)).send() else {
        return Vec::new();
    };
    if !response.status().is_success() {
        return Vec::new();
    }
    let Ok(parsed) = response.json::<RunsListResponse>() else {
        return Vec::new();
    };

    parsed
        .data
        .into_iter()
        .filter(|run| {
            is_active_run_status(&run.status)
                || is_open_pr_state(run.pr.as_ref().and_then(|pr| pr.state.as_deref()))
        })
        .map(|run| InFlightDispatch {
            task: run.task,
            flow: run.flow,
            status: run.status,
            pr_url: run.pr.as_ref().map(|pr| pr.url.clone()),
            pr_state: run.pr.as_ref().and_then(|pr| pr.state.clone()),
        })
        .collect()
}

#[derive(Debug, Deserialize)]
struct RunsListResponse {
    data: Vec<RunDto>,
}

fn is_active_run_status(status: &str) -> bool {
    matches!(status, "pending" | "running" | "waiting")
}

fn is_open_pr_state(state: Option<&str>) -> bool {
    matches!(state, Some(value) if value.eq_ignore_ascii_case("open") || value.eq_ignore_ascii_case("draft"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::config::Config;
    use std::path::PathBuf;
    use tempfile::TempDir;

    fn wave_fixture() -> TempDir {
        let tmp = tempfile::tempdir().expect("tempdir");
        let wave_dir = tmp.path().join("wave/ship");
        std::fs::create_dir_all(&wave_dir).expect("wave dir");
        std::fs::write(
            wave_dir.join("GOAL.md"),
            "---\nroadmap: wave/ship\nmetrics:\n  - tests pass\n---\nDrive the ship wave.",
        )
        .expect("write goal");
        std::fs::write(wave_dir.join("MEMORY.md"), "Last loop shipped auth.")
            .expect("write memory");
        tmp
    }

    fn git_repo_fixture() -> (TempDir, PathBuf) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let repo = tmp.path().join("repo");
        std::fs::create_dir_all(&repo).expect("repo dir");
        run_git(&repo, &["init"]);
        run_git(&repo, &["config", "user.email", "test@example.com"]);
        run_git(&repo, &["config", "user.name", "Test User"]);
        std::fs::write(repo.join("README.md"), "# test\n").expect("seed file");
        run_git(&repo, &["add", "."]);
        run_git(&repo, &["commit", "-m", "initial"]);
        (tmp, repo)
    }

    fn run_git(repo: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .expect("git command");
        assert!(
            output.status.success(),
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    #[test]
    fn build_goal_message_renders_goal_context_and_memory() {
        let tmp = wave_fixture();
        let message =
            build_goal_message(tmp.path(), "ship", false, Vec::new()).expect("build message");

        assert!(message.contains("Drive the ship wave."));
        assert!(message.contains("<lf:goal-context>"));
        assert!(message.contains("wave/ship"));
        assert!(message.contains("- tests pass"));
        assert!(message.contains("Last loop shipped auth."));
        assert!(!message.contains("<lf:goal-once>"));
    }

    #[test]
    fn build_goal_message_appends_once_marker() {
        let tmp = wave_fixture();
        let message =
            build_goal_message(tmp.path(), "ship", true, Vec::new()).expect("build message");

        assert!(message.contains("<lf:goal-once>"));
        assert!(message.contains("Run a single loop iteration"));
    }

    #[test]
    fn build_goal_message_fails_for_missing_goal() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let err = build_goal_message(tmp.path(), "missing", false, Vec::new())
            .expect_err("missing goal should fail");
        assert!(err.to_string().contains("missing"));
    }

    #[test]
    fn build_goal_message_can_render_builtin_vsm_goal() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let message =
            build_goal_message(tmp.path(), "s3", true, Vec::new()).expect("build message");

        assert!(message.contains("True north: the whole is worth more than the sum of its parts."));
        assert!(message.contains("<lf:goal-context>"));
        assert!(message.contains("<lf:goal-once>"));
    }

    #[test]
    fn goal_launch_prompt_includes_operate_and_goal_context() {
        let tmp = wave_fixture();
        let message =
            build_goal_message(tmp.path(), "ship", false, Vec::new()).expect("build message");

        let config = Config {
            agent: Some("claude:opus".to_string()),
            ..Config::default()
        };
        let prepared = prepare_goal_launch(&config, tmp.path().to_path_buf(), message, None, false)
            .expect("prepare goal launch");

        assert!(prepared.prompt.contains("<lf:loopflow>"));
        assert!(prepared.prompt.contains("<lf:goal-context>"));
        assert!(prepared.prompt.contains("Drive the ship wave."));
    }

    #[test]
    fn is_active_run_status_matches_expected_states() {
        assert!(is_active_run_status("pending"));
        assert!(is_active_run_status("running"));
        assert!(is_active_run_status("waiting"));
        assert!(!is_active_run_status("completed"));
        assert!(!is_active_run_status("failed"));
    }

    #[test]
    fn is_open_pr_state_accepts_open_and_draft() {
        assert!(is_open_pr_state(Some("open")));
        assert!(is_open_pr_state(Some("DRAFT")));
        assert!(!is_open_pr_state(Some("merged")));
        assert!(!is_open_pr_state(None));
    }

    #[test]
    fn wave_worktree_session_handle_uses_worktree_basename() {
        let handle = wave_worktree_session_handle(Path::new("/tmp/loopflow.concerto"));

        assert_eq!(handle, "lf-loopflow-concerto");
    }

    #[test]
    fn create_goal_worktree_reuses_existing_branch() {
        let (_tmp, repo) = git_repo_fixture();
        let branch = "jack.concerto";
        run_git(&repo, &["branch", branch]);

        let worktree = repo
            .parent()
            .expect("repo has parent")
            .join("repo.concerto");
        create_goal_worktree(&repo, &worktree, branch).expect("create worktree");

        let output = Command::new("git")
            .arg("-C")
            .arg(&worktree)
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .expect("git rev-parse");
        assert!(output.status.success());
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), branch);
    }
}
