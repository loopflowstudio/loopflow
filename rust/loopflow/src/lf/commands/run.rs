use crate::engine::fast_path::{try_fast_path, FailureContext, FastPathResult};
use crate::engine::{
    check_cli_available, durable_log_dir, launch_agent, load_config_or_default, parse_agent,
    prepare_launch_prompt, seed_rlm_env, write_prompt_log, AgentCapabilities, AgentConfig, Config,
    ContextBreakdown, ContextSourceOverrides, LaunchPromptInput, ProcessConfig, PromptComponents,
    StreamFormat, Surface, DEFAULT_CONTEXT_BUDGET,
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
    let mut built = build_prompt(step, message, cli)?;

    // Try fast-path: run the command before spinning up an agent.
    if let Some(ref cmd) = built.fast_path {
        info!(cmd = cmd, "trying fast-path");
        match try_fast_path(cmd, &built.repo_root) {
            Ok(FastPathResult::Success) => {
                info!("fast-path succeeded, skipping agent");
                return Ok(());
            }
            Ok(FastPathResult::Failed {
                exit_code,
                stdout,
                stderr,
            }) => {
                info!(exit_code, "fast-path failed, falling back to agent");
                let ctx = FailureContext {
                    cmd,
                    exit_code,
                    stdout: &stdout,
                    stderr: &stderr,
                };
                built.agent_config.task_prompt = format!("{ctx}{}", built.agent_config.task_prompt);
            }
            Err(err) => {
                info!(error = %err, "fast-path execution error, falling back to agent");
            }
        }
    }

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
    harness: String,
    step_name: Option<String>,
    log_name: String,
    fast_path: Option<String>,
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
    trace!(
        agent = config.agent.as_deref().unwrap_or("claude:opus"),
        ?config.yolo,
        "loaded config"
    );

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

    info!("preparing launch prompt");
    let prepare_start = Instant::now();
    let prepared = prepare_launch_prompt(
        &config,
        LaunchPromptInput {
            repo_root: repo_root.clone(),
            step: step.map(|value| value.to_string()),
            resolved_step: discovered_step.clone(),
            surface: if is_interactive {
                Surface::Cli
            } else {
                Surface::Headless
            },
            directions: cli.direction.clone(),
            area: cli
                .area
                .first()
                .map(|path| path.to_string_lossy().to_string()),
            wave: cli.wave.clone(),
            message: message.map(|value| value.to_string()),
            agent: cli.model.clone(),
            cwd: Some(repo_root.clone()),
            max_turns: None,
            yolo_mode: cli.yolo || config.yolo,
            include_config_directions: !cli.no_direction,
            include_config_area: true,
            source_overrides: ContextSourceOverrides {
                lfdocs: cli.lfdocs_setting(),
                diff_files: cli.diff_files_setting(),
                diff: cli.diff_setting(),
                clipboard: if cli.clipboard { Some(true) } else { None },
            },
            summary: None,
            client_context: Default::default(),
            related_repos: Vec::new(),
        },
    )?;
    debug!(
        elapsed_ms = prepare_start.elapsed().as_millis(),
        "prepared launch prompt"
    );
    let agent = prepared
        .config
        .agent
        .clone()
        .expect("prepare_launch_prompt always sets agent");
    let (harness, _model) = parse_agent(&agent);

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

    let fast_path = discovered_step.as_ref().and_then(|s| s.fast_path.clone());

    Ok(PromptBuild {
        repo_root,
        config,
        agent_config: prepared.config,
        process,
        capabilities,
        components: prepared.components,
        breakdown: prepared.breakdown,
        prompt: prepared.prompt,
        harness,
        step_name,
        log_name,
        fast_path,
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
        built.agent_config.agent.as_deref()
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
        open_web_client(&built.harness)?;
        println!("Copied to clipboard.");
        return Ok(());
    }

    let cli_check_start = Instant::now();
    if !check_cli_available(&built.harness) {
        return Err(anyhow!(
            "'{}' CLI not found. Run `lf op doctor` to check dependencies.",
            built.harness
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

    // Set up directive relay so agent steps can issue shell directives
    // (e.g. `cd` after `lf op land` rotates worktrees).
    let directive_file = std::env::var("LOOPFLOW_DIRECTIVE_FILE").ok();
    let mut agent_config = built.agent_config.clone();
    let relay_path = directive_file.as_ref().and_then(|_| {
        tempfile::NamedTempFile::new()
            .ok()
            .map(|f| f.into_temp_path().to_path_buf())
    });
    if let Some(ref path) = relay_path {
        agent_config.directive_relay = Some(path.clone());
    }

    debug!(launch = ?agent_config, ?process, ?built.capabilities, "launching agent");

    seed_rlm_env(&built.config);

    info!(harness = built.harness, "launching agent");
    let launch_start = Instant::now();
    let result = launch_agent(&agent_config, &process, &built.capabilities);

    // Relay safe directives from the agent back to the invoking shell.
    if let (Some(relay), Some(ref target)) = (relay_path, directive_file) {
        relay_directives(&relay, target);
    }

    let result = result?;
    debug!(
        elapsed_ms = launch_start.elapsed().as_millis(),
        "agent finished"
    );
    debug!(exit_code = result.exit_code, "agent completed");
    if result.exit_code == 0 {
        Ok(())
    } else {
        let log_hint = durable_log_dir(&built.repo_root)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "~/.lf/logs/".to_string());
        Err(anyhow!(
            "agent exited with code {}. Check {} for details.",
            result.exit_code,
            log_hint
        ))
    }
}

/// Forward safe shell directives from the agent's relay file to the real
/// directive file. Only `cd` commands are relayed — arbitrary shell commands
/// from agent subprocesses are not forwarded.
fn relay_directives(relay: &std::path::Path, target: &str) {
    let content = match std::fs::read_to_string(relay) {
        Ok(c) => c,
        Err(_) => return,
    };
    let _ = std::fs::remove_file(relay);

    let safe_lines: Vec<&str> = content
        .lines()
        .filter(|line| line.starts_with("cd "))
        .collect();
    if safe_lines.is_empty() {
        return;
    }

    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(target)
    {
        use std::io::Write;
        for line in safe_lines {
            let _ = writeln!(file, "{}", line);
        }
    }
}

pub fn split_step_args(args: &[String]) -> Result<(String, Vec<String>)> {
    let first = args.first().ok_or_else(|| anyhow!("no step specified"))?;

    let mut step = first.clone();
    let step_args = args.iter().skip(1).cloned().collect::<Vec<_>>();

    // Trailing colon is a separator: `implement: add auth` → step="implement"
    if let Some(stripped) = step.strip_suffix(':') {
        step = stripped.to_string();
    }
    // Inline colons are skill source prefixes: `npx:explain-code` stays intact.

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
    fn split_step_args_preserves_skill_prefix() {
        let args = vec!["npx:explain-code".to_string()];
        let (step, rest) = split_step_args(&args).expect("split args");
        assert_eq!(step, "npx:explain-code");
        assert!(rest.is_empty());
    }

    #[test]
    fn split_step_args_preserves_skill_prefix_with_args() {
        let args = vec!["sp:brainstorm".to_string(), "auth flow".to_string()];
        let (step, rest) = split_step_args(&args).expect("split args");
        assert_eq!(step, "sp:brainstorm");
        assert_eq!(rest, vec!["auth flow".to_string()]);
    }
}
