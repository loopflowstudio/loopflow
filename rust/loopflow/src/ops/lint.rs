use std::path::Path;
use std::process::Command;

use crate::engine::config::load_config_or_default;
use crate::engine::prompt::{
    default_gather_sources, format_prompt, gather_context, trim_context_with_breakdown,
    GatherContextOpts, PromptFormatMode, DEFAULT_CONTEXT_BUDGET,
};
use crate::engine::{launch_agent, AgentCapabilities, AgentConfig, ProcessConfig};

use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;

pub fn ensure_lint_passes(repo: &Path, progress: &impl Progress) -> OpsResult<bool> {
    let config = load_config_or_default(Some(repo));
    let lint_cmd = match config.lint.as_deref() {
        Some(cmd) => cmd,
        None => {
            progress.status("Lint skipped (no `lint` command configured)");
            return Ok(true);
        }
    };

    if run_check(repo, lint_cmd)? {
        progress.status("Lint passed");
        return Ok(true);
    }

    progress.status("Lint issues found, running fixer...");
    run_lint_agent(repo, progress)?;

    if run_check(repo, lint_cmd)? {
        Ok(true)
    } else {
        Err(OpsError::LintFailed)
    }
}

fn run_check(repo: &Path, cmd: &str) -> OpsResult<bool> {
    let status = Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .current_dir(repo)
        .status()?;
    Ok(status.success())
}

fn run_lint_agent(repo: &Path, progress: &impl Progress) -> OpsResult<()> {
    let config = load_config_or_default(Some(repo));
    let opts = GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("lint".to_string()),
        message: None,
        run_mode: Some("auto".to_string()),
        directions: config.direction.unwrap_or_default(),
        files: Vec::new(),
        sources: default_gather_sources(
            config.lfdocs,
            config.diff_files || config.diff,
            config.paste,
        ),
        area: config.area,
        wave: None,
    };

    let gathered = gather_context(&opts)?;
    let budgeted = trim_context_with_breakdown(gathered, DEFAULT_CONTEXT_BUDGET);
    let prompt = format_prompt(PromptFormatMode::Full, &budgeted).into_string();

    let launch = AgentConfig {
        task_prompt: prompt,
        model: Some(config.agent_model.clone()),
        cwd: Some(repo.to_path_buf()),
        skip_permissions: true,
        ..Default::default()
    };
    let process = ProcessConfig {
        auto: true,
        stream: false,
        ..Default::default()
    };
    let capabilities = AgentCapabilities {
        chrome: config.chrome,
    };

    progress.status("Launching lint fixer...");
    let result = launch_agent(&launch, &process, &capabilities)
        .map_err(|err| OpsError::AgentFailed(err.to_string()))?;
    if result.exit_code != 0 {
        return Err(OpsError::AgentFailed(result.stderr));
    }
    Ok(())
}
