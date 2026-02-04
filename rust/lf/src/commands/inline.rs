use crate::commands::util::{copy_to_clipboard, find_repo_root, open_web_client};
use crate::Cli;
use anyhow::{anyhow, Result};
use loopflow_engine::{
    check_cli_available, format_context_prompt, format_prompt, gather_context, launch_agent,
    load_config_or_default, parse_model, write_prompt_log, GatherContextOpts, LaunchConfig,
};

pub fn run(prompt_parts: &[String], cli: &Cli) -> Result<()> {
    let prompt_text = prompt_parts.join(" ").trim().to_string();
    if prompt_text.is_empty() {
        return Err(anyhow!("inline prompt is empty"));
    }

    let repo_root = find_repo_root()?;
    let config = load_config_or_default(Some(&repo_root));

    let mut directions = config.direction.clone().unwrap_or_default();
    directions.extend(cli.direction.clone());

    let is_interactive = cli.interactive || !cli.batch;

    let area = if !cli.area.is_empty() {
        cli.area.first().map(|p| p.to_string_lossy().to_string())
    } else {
        config.area.clone()
    };

    let include_clipboard = cli.clipboard || config.paste;

    let components = gather_context(&GatherContextOpts {
        repo_root: repo_root.clone(),
        step: None,
        inline: Some(prompt_text.clone()),
        step_args: Vec::new(),
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

    let model = cli.model.as_deref().unwrap_or(&config.agent_model);
    let (backend, variant) = parse_model(model);

    if cli.web {
        let mut prompt = format_prompt(&components);
        prompt.push_str("\n\n");
        prompt.push_str(&prompt_text);
        copy_to_clipboard(&prompt)?;
        open_web_client(&backend)?;
        println!("Copied to clipboard.");
        return Ok(());
    }

    if !check_cli_available(&backend) {
        return Err(anyhow!("'{}' CLI not found", backend));
    }

    // Context goes to file, inline prompt is the task
    let context_prompt = format_context_prompt(&components);
    let context_file = write_prompt_log(&repo_root, &context_prompt, "inline.context", None)?;

    let launch_config = LaunchConfig {
        auto: !is_interactive,
        stream: !is_interactive,
        skip_permissions: cli.yolo || config.yolo,
        model_variant: variant,
        chrome: cli.chrome_setting().unwrap_or(config.chrome),
        cwd: Some(repo_root),
        context_file: Some(context_file),
    };

    let result = launch_agent(model, &prompt_text, &launch_config)?;
    std::process::exit(result.exit_code);
}
