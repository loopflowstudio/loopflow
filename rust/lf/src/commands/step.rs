use crate::commands::util::{copy_to_clipboard, find_repo_root, open_web_client};
use crate::Cli;
use anyhow::{anyhow, Result};
use loopflow_engine::{
    check_cli_available, format_context_prompt, format_prompt, format_task_prompt, gather_context,
    launch_agent, load_config_or_default, parse_model, write_prompt_log, GatherContextOpts,
    LaunchConfig,
};
use tracing::{debug, info, instrument, trace};

/// Run a step by name.
#[instrument(skip(cli), fields(name = %step_name))]
pub fn run(step_name: &str, step_args: &[String], cli: &Cli) -> Result<()> {
    let repo_root = find_repo_root()?;
    let config = load_config_or_default(Some(&repo_root));
    trace!(?config.agent_model, ?config.yolo, "loaded config");

    let step = crate::discovery::discover_step(&repo_root, step_name)?;
    debug!(step.name, step.interactive, "discovered step");

    let mut directions = config.direction.clone().unwrap_or_default();
    directions.extend(cli.direction.clone());

    let is_interactive = cli.interactive
        || (!cli.batch
            && (step.interactive.unwrap_or(false) || config.interactive.contains(&step.name)));

    let area = if !cli.area.is_empty() {
        cli.area.first().map(|p| p.to_string_lossy().to_string())
    } else {
        config.area.clone()
    };

    let include_clipboard = cli.clipboard || config.paste;

    let components = gather_context(&GatherContextOpts {
        repo_root: repo_root.clone(),
        step: Some(step_name.to_string()),
        inline: None,
        step_args: step_args.to_vec(),
        message: None,
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
        lfdocs: config.lfdocs,
        diff_files: config.diff_files,
        diff: config.diff,
        clipboard: include_clipboard,
        area,
        wave: cli.wave.clone(),
    })?;

    let prompt = format_prompt(&components);

    let model = cli.model.as_deref().unwrap_or(&config.agent_model);
    let (backend, variant) = parse_model(model);

    if cli.web {
        info!("copying to clipboard and opening web client");
        copy_to_clipboard(&prompt)?;
        open_web_client(&backend)?;
        println!("Copied to clipboard.");
        return Ok(());
    }

    if !check_cli_available(&backend) {
        return Err(anyhow!("'{}' CLI not found", backend));
    }

    // Write full prompt to .lf/log/ for debugging
    write_prompt_log(&repo_root, &prompt, step_name, None)?;

    // Write context separately for system prompt loading via native mechanisms:
    // - Claude: --append-system-prompt-file
    // - Codex: -c model_instructions_file="..."
    // - Gemini: GEMINI_SYSTEM_MD env var
    let context_prompt = format_context_prompt(&components);
    let task_prompt = format_task_prompt(&components);
    let context_file = write_prompt_log(
        &repo_root,
        &context_prompt,
        &format!("{}.context", step_name),
        None,
    )?;

    let launch_config = LaunchConfig {
        auto: !is_interactive,
        stream: !is_interactive,
        skip_permissions: cli.yolo || config.yolo,
        model_variant: variant,
        chrome: cli.chrome_setting().unwrap_or(config.chrome),
        cwd: Some(repo_root),
        context_file: Some(context_file),
    };
    debug!(?launch_config, "launching agent");

    let result = launch_agent(model, &task_prompt, &launch_config)?;
    debug!(exit_code = result.exit_code, "agent completed");
    std::process::exit(result.exit_code);
}

#[instrument(skip(cli))]
pub fn run_interactive(cli: &Cli) -> Result<()> {
    let repo_root = find_repo_root()?;
    debug!(?repo_root, "found repository root");

    let config = load_config_or_default(Some(&repo_root));

    let mut directions = config.direction.clone().unwrap_or_default();
    directions.extend(cli.direction.clone());

    let include_clipboard = cli.clipboard || config.paste;

    debug!("gathering context for interactive session");
    let components = gather_context(&GatherContextOpts {
        repo_root: repo_root.clone(),
        step: None,
        inline: None,
        step_args: Vec::new(),
        message: None,
        run_mode: Some("interactive".to_string()),
        directions,
        files: Vec::new(),
        lfdocs: config.lfdocs,
        diff_files: config.diff_files,
        diff: config.diff,
        clipboard: include_clipboard,
        area: config.area.clone(),
        wave: cli.wave.clone(),
    })?;

    let prompt = format_prompt(&components);
    let model = cli.model.as_deref().unwrap_or(&config.agent_model);
    let (backend, variant) = parse_model(model);

    if !check_cli_available(&backend) {
        return Err(anyhow!("'{}' CLI not found", backend));
    }

    // Write full prompt to .lf/log/ for debugging
    write_prompt_log(&repo_root, &prompt, "chat", None)?;

    // Write context separately for system prompt loading via native mechanisms:
    // - Claude: --append-system-prompt-file
    // - Codex: -c model_instructions_file="..."
    // - Gemini: GEMINI_SYSTEM_MD env var
    let context_prompt = format_context_prompt(&components);
    let task_prompt = format_task_prompt(&components);
    let context_file = write_prompt_log(&repo_root, &context_prompt, "chat.context", None)?;

    let launch_config = LaunchConfig {
        auto: false,
        stream: false,
        skip_permissions: cli.yolo || config.yolo,
        model_variant: variant,
        chrome: cli.chrome_setting().unwrap_or(config.chrome),
        cwd: Some(repo_root),
        context_file: Some(context_file),
    };
    debug!(?launch_config, "launching interactive agent");

    let result = launch_agent(model, &task_prompt, &launch_config)?;
    std::process::exit(result.exit_code);
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
