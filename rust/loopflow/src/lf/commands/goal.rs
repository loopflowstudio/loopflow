use std::path::Path;
use std::time::Duration;

use anyhow::{anyhow, Result};
use serde::Deserialize;

use crate::engine::config::load_config_or_default;
use crate::engine::worktrees::main_repo_root;
use crate::engine::{
    available_flow_names, load_goal, parse_agent, prepare_goal_launch, render_goal,
    GoalRenderContext, InFlightDispatch,
};
use crate::lf::commands::util::{find_repo_root, launch_session};
use crate::lf::Cli;
use crate::lfd::client::{authorize, blocking_client, resolve_base_url};
use crate::lfd::http::dto::WaveRunDto;
use crate::lfd::http::routes::wave_config::read_wave_config;
use crate::ops::util::resolve_wave_name;

const IN_FLIGHT_FETCH_TIMEOUT: Duration = Duration::from_secs(5);

/// Launch a wave's goal as a looping top-level agent (`operate` on, interactive
/// surface). The agent dispatches real work via `lf op dispatch`.
pub fn run(name: &str, once: bool, cli: &Cli) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root).unwrap_or(repo_root);
    let wave_name = resolve_wave_name(&main_repo, Some(name))
        .ok_or_else(|| anyhow!("invalid wave name: '{name}'"))?;

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
    data: Vec<WaveRunDto>,
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

        assert!(prepared.prompt.contains("<lf:operate>"));
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
}
