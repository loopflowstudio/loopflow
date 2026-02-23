use crate::engine::{
    check_cli_available, default_gather_sources, drop_native_instruction_docs,
    format_context_prompt, format_prompt, format_task_prompt, gather_context, launch_agent,
    load_config_or_default, parse_model, seed_rlm_env, trim_context_with_breakdown,
    write_prompt_log, Config, ContextBreakdown, GatherContextOpts, LaunchConfig, PromptComponents,
    PromptFormatMode, StreamFormat, DEFAULT_CONTEXT_BUDGET,
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
    components: PromptComponents,
    breakdown: ContextBreakdown,
    prompt: String,
    model: String,
    backend: String,
    variant: Option<String>,
    area: Option<String>,
    is_interactive: bool,
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

    let mut directions = config.direction.clone().unwrap_or_default();
    directions.extend(cli.direction.clone());

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

    let area = if !cli.area.is_empty() {
        cli.area.first().map(|p| p.to_string_lossy().to_string())
    } else {
        config.area.clone()
    };

    let include_clipboard = cli.clipboard || config.paste;
    let lfdocs = cli.lfdocs_setting().unwrap_or(config.lfdocs);
    let diff_files = cli.diff_files_setting().unwrap_or(config.diff_files);
    let diff = cli.diff_setting().unwrap_or(config.diff);

    info!("gathering context");
    let gather_start = Instant::now();
    let components = gather_context(&GatherContextOpts {
        repo_root: repo_root.clone(),
        step: step.map(|s| s.to_string()),
        message: message.map(|s| s.to_string()),
        run_mode: Some(
            if is_interactive {
                "interactive"
            } else {
                "auto"
            }
            .to_string(),
        ),
        directions,
        files: Vec::new(),
        sources: default_gather_sources(
            lfdocs,
            diff_files || diff,
            include_clipboard,
            area.as_deref(),
            cli.wave.as_deref(),
        ),
        area: area.clone(),
        wave: cli.wave.clone(),
    })?;
    debug!(
        elapsed_ms = gather_start.elapsed().as_millis(),
        "gathered context"
    );
    let model = cli
        .model
        .as_deref()
        .unwrap_or(&config.agent_model)
        .to_string();
    let (backend, variant) = parse_model(&model);

    let mut components = components;
    drop_native_instruction_docs(&mut components, &repo_root);
    let trim_start = Instant::now();
    let (components, breakdown) = trim_context_with_breakdown(components, DEFAULT_CONTEXT_BUDGET);
    debug!(
        elapsed_ms = trim_start.elapsed().as_millis(),
        "trimmed context"
    );

    let prompt_start = Instant::now();
    let prompt = format_prompt(PromptFormatMode::Full, &components);
    debug!(
        elapsed_ms = prompt_start.elapsed().as_millis(),
        "formatted prompt"
    );

    let step_name = step.map(|value| value.to_string());
    let log_name = step_name
        .as_deref()
        .unwrap_or(if message.is_some() { "inline" } else { "chat" })
        .to_string();

    Ok(PromptBuild {
        repo_root,
        config,
        components,
        breakdown,
        prompt,
        model,
        backend,
        variant,
        area,
        is_interactive,
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
        Some(built.model.as_str())
    } else {
        None
    };
    let command = format_reproducible_command(
        built.step_name.as_deref(),
        &direction_names,
        built.components.wave.as_deref(),
        built.area.as_deref(),
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

    let context_prompt_start = Instant::now();
    let context_prompt = format_context_prompt(&built.components);
    let task_prompt = format_task_prompt(&built.components);
    debug!(
        elapsed_ms = context_prompt_start.elapsed().as_millis(),
        "formatted context/task prompt"
    );
    let context_file_start = Instant::now();
    let context_file = Some(write_prompt_log(
        &built.repo_root,
        &context_prompt,
        &format!("{}.context", built.log_name),
        None,
    )?);
    debug!(
        elapsed_ms = context_file_start.elapsed().as_millis(),
        "wrote context log"
    );

    let use_color = std::env::var("NO_COLOR").is_err() && std::io::stderr().is_terminal();
    let launch_config = LaunchConfig {
        auto: !built.is_interactive,
        stream: !built.is_interactive,
        skip_permissions: cli.yolo || built.config.yolo,
        model_variant: built.variant.clone(),
        chrome: cli.chrome_setting().unwrap_or(built.config.chrome),
        cwd: Some(built.repo_root.clone()),
        context_file,
        stream_format: StreamFormat::Human(use_color),
    };
    debug!(?launch_config, "launching agent");

    seed_rlm_env(&built.config);
    info!(backend = built.backend, "launching agent");
    let launch_start = Instant::now();
    let result = launch_agent(&built.model, &task_prompt, &launch_config)?;
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
    } else if let Some((name, rest)) = step.split_once(':') {
        let name = name.to_string();
        let rest = rest.to_string();
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
