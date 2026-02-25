use crate::engine::{
    check_cli_available, launch_agent, load_config_or_default, parse_model, prepare_launch_prompt,
    seed_rlm_env, write_prompt_log, AgentCapabilities, AgentConfig, Config, ContextBreakdown,
    ContextSourceOverrides, LaunchPromptInput, ProcessConfig, PromptComponents, StreamFormat,
    DEFAULT_CONTEXT_BUDGET,
};
use crate::lf::commands::util::{copy_to_clipboard, find_repo_root, open_web_client};
use crate::lf::output::{format_context_header, format_reproducible_command, Colors};
use crate::lf::Cli;
use anyhow::{anyhow, Result};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Instant;
use tracing::{debug, info, instrument, trace};

/// Unified entry point for running steps, inline prompts, or interactive chat.
///
/// | step    | message | behavior                              |
/// |---------|---------|---------------------------------------|
/// | Some    | None    | Run named step                        |
/// | None    | Some    | Run inline prompt                     |
/// | Some    | Some    | Run step with message as extra context |
/// | None    | None    | Interactive chat                      |
#[instrument(skip(cli), fields(step = ?step, has_message = message.is_some()))]
pub fn run(step: Option<&str>, message: Option<&str>, cli: &Cli) -> Result<()> {
    let built = build_prompt(step, message, cli)?;
    print_context_header(&built, cli);
    launch_prompt(&built, cli)
}

struct PromptBuild {
    repo_root: PathBuf,
    config: Config,
    agent_config: AgentConfig,
    process: ProcessConfig,
    capabilities: AgentCapabilities,
    components: PromptComponents,
    breakdown: ContextBreakdown,
    prompt: String,
    backend: String,
    step_name: Option<String>,
    log_name: String,
}

fn build_prompt(step: Option<&str>, message: Option<&str>, cli: &Cli) -> Result<PromptBuild> {
    let start = Instant::now();
    let repo_root = find_repo_root()?;
    debug!(elapsed_ms = start.elapsed().as_millis(), "found repo root");

    let config_start = Instant::now();
    let config = load_config_or_default(Some(&repo_root));
    debug!(
        elapsed_ms = config_start.elapsed().as_millis(),
        "loaded config"
    );
    trace!(?config.agent_model, ?config.yolo, "loaded config");

    let discover_start = Instant::now();
    let discovered_step = if let Some(step_name) = step {
        Some(crate::lf::discovery::discover_step(&repo_root, step_name)?)
    } else {
        None
    };
    debug!(
        elapsed_ms = discover_start.elapsed().as_millis(),
        "discovered step"
    );

    if let Some(ref s) = discovered_step {
        debug!(s.name, s.interactive, "discovered step");
    }

    let is_interactive = cli.interactive
        || (!cli.batch
            && (discovered_step
                .as_ref()
                .and_then(|s| s.interactive)
                .unwrap_or(false)
                || step
                    .map(|s| config.interactive.contains(&s.to_string()))
                    .unwrap_or(false)
                || (step.is_none() && message.is_none())));

    let run_mode = if is_interactive {
        "interactive"
    } else {
        "auto"
    };

    info!("preparing launch prompt");
    let prepare_start = Instant::now();
    let prepared = prepare_launch_prompt(
        &config,
        LaunchPromptInput {
            repo_root: repo_root.clone(),
            step: step.map(|value| value.to_string()),
            run_mode: Some(run_mode.to_string()),
            directions: cli.direction.clone(),
            area: cli
                .area
                .first()
                .map(|path| path.to_string_lossy().to_string()),
            wave: cli.wave.clone(),
            message: message.map(|value| value.to_string()),
            model: cli.model.clone(),
            cwd: Some(repo_root.clone()),
            max_turns: None,
            yolo_mode: cli.yolo || config.yolo,
            include_config_directions: true,
            include_config_area: true,
            source_overrides: ContextSourceOverrides {
                lfdocs: cli.lfdocs_setting(),
                diff_files: cli.diff_files_setting(),
                diff: cli.diff_setting(),
                clipboard: if cli.clipboard { Some(true) } else { None },
            },
            summary: None,
        },
    )?;
    debug!(
        elapsed_ms = prepare_start.elapsed().as_millis(),
        "prepared launch prompt"
    );
    let model = prepared
        .config
        .model
        .clone()
        .expect("prepare_launch_prompt always sets launch model");
    let (backend, _variant) = parse_model(&model);

    let step_name = step.map(|value| value.to_string());
    let log_name = step_name
        .as_deref()
        .unwrap_or(if message.is_some() { "inline" } else { "chat" })
        .to_string();
    let process = ProcessConfig {
        auto: !is_interactive,
        stream: !is_interactive,
        ..Default::default()
    };
    let capabilities = AgentCapabilities {
        chrome: cli.chrome_setting().unwrap_or(config.chrome),
    };

    Ok(PromptBuild {
        repo_root,
        config,
        agent_config: prepared.config,
        process,
        capabilities,
        components: prepared.components,
        breakdown: prepared.breakdown,
        prompt: prepared.prompt,
        backend,
        step_name,
        log_name,
    })
}

fn print_context_header(built: &PromptBuild, cli: &Cli) {
    let colors = Colors::new();
    let header = format_context_header(&built.breakdown, DEFAULT_CONTEXT_BUDGET);
    let direction_names: Vec<String> = built
        .components
        .directions
        .iter()
        .map(|d| d.name.clone())
        .collect();
    let cli_model = if cli.model.is_some() {
        built.agent_config.model.as_deref()
    } else {
        None
    };
    let command = format_reproducible_command(
        built.step_name.as_deref(),
        &direction_names,
        built.components.wave.as_deref(),
        built.components.area.as_deref(),
        cli.clipboard,
        cli_model,
    );
    eprintln!(
        "{dim}{header}\n\n  {command}{reset}",
        dim = colors.dim,
        header = header,
        command = command,
        reset = colors.reset,
    );
}

fn launch_prompt(built: &PromptBuild, cli: &Cli) -> Result<()> {
    if cli.web {
        info!("copying to clipboard and opening web client");
        copy_to_clipboard(&built.prompt)?;
        open_web_client(&built.backend)?;
        println!("Copied to clipboard.");
        return Ok(());
    }

    let cli_check_start = Instant::now();
    if !check_cli_available(&built.backend) {
        return Err(anyhow!(
            "'{}' CLI not found. Run `lf ops doctor` to check dependencies.",
            built.backend
        ));
    }
    debug!(
        elapsed_ms = cli_check_start.elapsed().as_millis(),
        "checked cli availability"
    );

    let write_prompt_start = Instant::now();
    write_prompt_log(&built.repo_root, &built.prompt, &built.log_name, None)?;
    debug!(
        elapsed_ms = write_prompt_start.elapsed().as_millis(),
        "wrote prompt log"
    );

    let context_file_start = Instant::now();
    let context_file = Some(write_prompt_log(
        &built.repo_root,
        &built.agent_config.system_prompt,
        &format!("{}.context", built.log_name),
        None,
    )?);
    debug!(
        elapsed_ms = context_file_start.elapsed().as_millis(),
        "wrote context log"
    );

    let use_color = std::env::var("NO_COLOR").is_err() && std::io::stderr().is_terminal();
    let mut process = built.process.clone();
    process.context_file = context_file;
    process.stream_format = StreamFormat::Human(use_color);
    debug!(launch = ?built.agent_config, ?process, ?built.capabilities, "launching agent");

    seed_rlm_env(&built.config);

    // Inject steps and directions as .claude/commands/ for Claude agents.
    // Default off for CLI; always on for lfd. Enable with `inject_skills: true` in .lf/config.yaml.
    let injected_skills = if built.config.inject_skills && built.backend == "claude" {
        crate::engine::skills::inject_skills(&built.repo_root, &built.repo_root)
    } else {
        Vec::new()
    };

    info!(backend = built.backend, "launching agent");
    let launch_start = Instant::now();
    let result = launch_agent(&built.agent_config, &process, &built.capabilities);

    crate::engine::skills::cleanup_injected_skills(&injected_skills);

    let result = result?;
    debug!(
        elapsed_ms = launch_start.elapsed().as_millis(),
        "agent finished"
    );
    debug!(exit_code = result.exit_code, "agent completed");
    if result.exit_code == 0 {
        Ok(())
    } else {
        Err(anyhow!(
            "agent exited with code {}. Check .lf/logs/ for details.",
            result.exit_code
        ))
    }
}

pub fn split_step_args(args: &[String]) -> Result<(String, Vec<String>)> {
    let first = args.first().ok_or_else(|| anyhow!("no step specified"))?;

    let mut step = first.clone();
    let mut step_args = args.iter().skip(1).cloned().collect::<Vec<_>>();

    if let Some(stripped) = step.strip_suffix(':') {
        step = stripped.to_string();
    } else if let Some((name, rest)) = step
        .split_once(':')
        .map(|(name, rest)| (name.to_string(), rest.to_string()))
    {
        step = name;
        if !rest.is_empty() {
            step_args.insert(0, rest);
        }
    }

    if step.is_empty() {
        return Err(anyhow!("no step specified"));
    }

    Ok((step, step_args))
}

#[cfg(test)]
mod tests {
    use super::split_step_args;

    #[test]
    fn split_step_args_handles_trailing_colon() {
        let args = vec![
            "implement:".to_string(),
            "add".to_string(),
            "logs".to_string(),
        ];
        let (step, rest) = split_step_args(&args).expect("split args");
        assert_eq!(step, "implement");
        assert_eq!(rest, vec!["add".to_string(), "logs".to_string()]);
    }

    #[test]
    fn split_step_args_handles_inline_suffix() {
        let args = vec!["fix:bug".to_string(), "now".to_string()];
        let (step, rest) = split_step_args(&args).expect("split args");
        assert_eq!(step, "fix");
        assert_eq!(rest, vec!["bug".to_string(), "now".to_string()]);
    }
}
