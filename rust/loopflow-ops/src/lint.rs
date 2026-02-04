use std::path::Path;
use std::process::Command;

use loopflow_engine::config::load_config_or_default;
use loopflow_engine::prompt::{format_prompt, gather_context, GatherContextOpts};
use loopflow_engine::{launch_agent, LaunchConfig};

use crate::error::{OpsError, OpsResult};
use crate::progress::Progress;
use crate::util::command_exists;

pub fn ensure_lint_passes(repo: &Path, progress: &impl Progress) -> OpsResult<bool> {
    let config = load_config_or_default(Some(repo));
    match check_lint(repo, config.lint_check.as_deref())? {
        Some(true) => {
            progress.status("Lint passed");
            return Ok(true);
        }
        Some(false) => progress.status("Lint issues found, running fixer..."),
        None => progress.status("Running lint..."),
    }

    run_lint_agent(repo, progress)?;

    match check_lint(repo, config.lint_check.as_deref())? {
        Some(true) => Ok(true),
        Some(false) => Err(OpsError::LintFailed),
        None => Ok(true),
    }
}

fn check_lint(repo: &Path, lint_check: Option<&str>) -> OpsResult<Option<bool>> {
    if let Some(cmd) = lint_check {
        let status = Command::new("sh")
            .arg("-c")
            .arg(cmd)
            .current_dir(repo)
            .status()?;
        return Ok(Some(status.success()));
    }

    if !command_exists("ruff") {
        return Ok(None);
    }

    let mut targets = Vec::new();
    if repo.join("src").is_dir() {
        targets.push("src/");
    }
    if repo.join("tests").is_dir() {
        targets.push("tests/");
    }
    if targets.is_empty() {
        return Ok(None);
    }

    let check_status = Command::new("ruff")
        .arg("check")
        .args(&targets)
        .current_dir(repo)
        .status()?;
    if !check_status.success() {
        return Ok(Some(false));
    }

    let fmt_status = Command::new("ruff")
        .arg("format")
        .arg("--check")
        .args(&targets)
        .current_dir(repo)
        .status()?;

    Ok(Some(fmt_status.success()))
}

fn run_lint_agent(repo: &Path, progress: &impl Progress) -> OpsResult<()> {
    let config = load_config_or_default(Some(repo));
    let opts = GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: Some("lint".to_string()),
        inline: None,
        step_args: Vec::new(),
        run_mode: Some("auto".to_string()),
        directions: config.direction.unwrap_or_default(),
        files: Vec::new(),
        lfdocs: config.lfdocs,
        diff_files: config.diff_files,
        diff: config.diff,
        clipboard: config.paste,
        area: config.area,
        wave: None,
    };

    let components = gather_context(&opts)?;
    let prompt = format_prompt(&components);

    let launch_config = LaunchConfig {
        auto: true,
        stream: false,
        skip_permissions: true,
        model_variant: None,
        chrome: config.chrome,
        cwd: Some(repo.to_path_buf()),
    };

    progress.status("Launching lint fixer...");
    let result = launch_agent(&config.agent_model, &prompt, &launch_config)
        .map_err(|err| OpsError::AgentFailed(err.to_string()))?;
    if result.exit_code != 0 {
        return Err(OpsError::AgentFailed(result.stderr));
    }
    Ok(())
}
