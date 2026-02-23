use std::path::Path;
use std::process::Command;

use crate::engine::config::load_config_or_default;
use crate::engine::prompt::{
    default_gather_sources, format_prompt, gather_context, GatherContextOpts, PromptFormatMode,
};
use crate::engine::{launch_agent, LaunchConfig};

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

    let components = gather_context(&opts)?;
    let prompt = format_prompt(PromptFormatMode::Full, &components);

    let launch_config = LaunchConfig {
        auto: true,
        stream: false,
        skip_permissions: true,
        model_variant: None,
        chrome: config.chrome,
        cwd: Some(repo.to_path_buf()),
        context_file: None,
        ..Default::default()
    };

    progress.status("Launching lint fixer...");
    let result = launch_agent(&config.agent_model, &prompt, &launch_config)
        .map_err(|err| OpsError::AgentFailed(err.to_string()))?;
    if result.exit_code != 0 {
        return Err(OpsError::AgentFailed(result.stderr));
    }
    Ok(())
}
