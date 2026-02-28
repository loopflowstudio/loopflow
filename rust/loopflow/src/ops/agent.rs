use std::path::Path;
use std::time::Duration;

use crate::engine::builtins::get_builtin_step;
use crate::engine::config::load_config_or_default;
use crate::engine::prompt::{
    default_gather_sources, format_prompt, gather_context, trim_context_with_breakdown,
    GatherContextOpts, PromptFormatMode, Surface, DEFAULT_CONTEXT_BUDGET,
};
use crate::engine::{launch_agent, AgentCapabilities, AgentConfig, ProcessConfig};

use crate::ops::error::{OpsError, OpsResult};
use crate::ops::progress::Progress;

#[derive(Debug, Clone)]
pub struct BuiltinAgentOptions {
    pub step_name: String,
    pub suffix: String,
    pub timeout: Option<Duration>,
}

pub fn run_builtin_agent(
    repo: &Path,
    options: &BuiltinAgentOptions,
    progress: &impl Progress,
) -> OpsResult<()> {
    let config = load_config_or_default(Some(repo));
    let step_content = get_builtin_step(&options.step_name).ok_or_else(|| {
        OpsError::AgentFailed(format!("built-in step '{}' not found", options.step_name))
    })?;

    let opts = GatherContextOpts {
        repo_root: repo.to_path_buf(),
        step: None,
        message: None,
        surface: Surface::Headless,
        directions: config.direction.unwrap_or_default(),
        files: Vec::new(),
        sources: default_gather_sources(
            config.lfdocs,
            config.diff_files || config.diff,
            config.paste,
        ),
        area: config.area,
        wave: None,
        related_repos: Vec::new(),
    };
    let gathered = gather_context(&opts)?;
    let budgeted = trim_context_with_breakdown(gathered, DEFAULT_CONTEXT_BUDGET);
    let base_prompt = format_prompt(PromptFormatMode::Full, &budgeted).into_string();
    let prompt = format!(
        "{}\n\n<lf:step>\n{}\n</lf:step>\n\n{}\n",
        base_prompt, step_content, options.suffix
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
    let result = launch_agent(&launch, &process, &capabilities)
        .map_err(|err| OpsError::AgentFailed(err.to_string()))?;
    if result.exit_code != 0 {
        return Err(OpsError::AgentFailed(result.stderr));
    }
    Ok(())
}
