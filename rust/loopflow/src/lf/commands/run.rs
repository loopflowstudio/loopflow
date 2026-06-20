use crate::engine::fast_path::{try_fast_path, FailureContext, FastPathResult};
use crate::engine::{
    check_cli_available, durable_log_dir, launch_agent, load_config_or_default, parse_agent,
    prepare_launch_prompt, seed_rlm_env, write_prompt_log, AgentCapabilities, AgentConfig, Config,
    ContextBreakdown, ContextSourceOverrides, LaunchPromptInput, LaunchTarget, ProcessConfig,
    PromptComponents, SkillSyncOptions, StreamFormat, Surface, DEFAULT_CONTEXT_BUDGET,
};
use crate::lf::commands::util::{find_repo_root, launch_session};
use crate::lf::output::{format_context_header, format_reproducible_command, Colors};
use crate::lf::Cli;
use anyhow::{anyhow, Result};
use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Instant;
use tracing::{debug, info, instrument, trace, warn};

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
    model: Option<String>,
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
    let surface = if is_interactive {
        Surface::Cli
    } else {
        Surface::Headless
    };

    let prepared = prepare_launch_prompt(
        &config,
        LaunchPromptInput {
            repo_root: repo_root.clone(),
            step: step.map(|value| value.to_string()),
            resolved_step: discovered_step.clone(),
            surface,
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
    let (harness, model) = parse_agent(&agent);

    let step_name = discovered_step
        .as_ref()
        .map(|step| step.name.clone())
        .or_else(|| step.map(|value| value.to_string()));
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
    let mut agent_config = prepared.config;
    let mut prompt = prepared.prompt;
    if let Some(step_name) = step_name.as_deref() {
        if should_launch_via_skill(step_name) {
            let sync_start = Instant::now();
            crate::engine::sync_skills(&repo_root, &SkillSyncOptions::default())?;
            debug!(
                elapsed_ms = sync_start.elapsed().as_millis(),
                "synced vendor skills"
            );
            prompt = skill_launch_seed(
                surface,
                step_name,
                message,
                prepared.components.voice_doc.as_deref(),
            );
            agent_config.system_prompt.clear();
            agent_config.task_prompt = prompt.clone();
        } else {
            warn!(
                step = step_name,
                "external skill step uses assembled prompt fallback"
            );
        }
    }

    Ok(PromptBuild {
        repo_root,
        config,
        agent_config,
        process,
        capabilities,
        components: prepared.components,
        breakdown: prepared.breakdown,
        prompt,
        harness,
        model,
        step_name,
        log_name,
        fast_path,
    })
}

fn should_launch_via_skill(step_name: &str) -> bool {
    !step_name.starts_with("npx/") && !step_name.starts_with("rams/")
}

/// Build the launch seed for a `/step` handoff: the slash command, the surface
/// preamble, and the ambient context the assembled prompt used to carry as a
/// system prompt (voice + orientation). The step body itself loads from the
/// synced skill on invoke, so this stays small enough for the GUI deep-link cap.
fn skill_launch_seed(
    surface: Surface,
    step_name: &str,
    message: Option<&str>,
    voice: Option<&str>,
) -> String {
    let mut seed = format!("/{step_name}\n\n{}", surface.instructions());
    if let Some(voice) = voice.map(str::trim).filter(|value| !value.is_empty()) {
        seed.push_str("\n\n<lf:voice>\n");
        seed.push_str(voice);
        seed.push_str("\n</lf:voice>");
    }
    seed.push_str("\n\n<lf:orientation>\n");
    seed.push_str(crate::engine::builtins::ORIENTATION_DOC.trim());
    seed.push_str("\n</lf:orientation>");
    if let Some(message) = message.filter(|value| !value.trim().is_empty()) {
        seed.push_str("\n\n<lf:message>\n");
        seed.push_str(message);
        seed.push_str("\n</lf:message>");
    }
    seed
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
    // `--tui` / `--ide` force a handoff and override the repo default; an
    // interactive step with neither flag uses `session.launch`.
    let forced_target = if cli.ide {
        Some(LaunchTarget::Ide)
    } else if cli.tui {
        Some(LaunchTarget::Tui)
    } else {
        None
    };

    if forced_target.is_some() || !built.process.auto {
        info!("launching interactive vendor session");
        launch_session(
            forced_target.unwrap_or(built.config.session.launch),
            &built.harness,
            built.model.as_deref(),
            &built.repo_root,
            &built.prompt,
        )?;
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

    if step.is_empty() {
        return Err(anyhow!("no step specified"));
    }

    Ok((step, step_args))
}

#[cfg(test)]
mod tests {
    use super::{should_launch_via_skill, skill_launch_seed, split_step_args};
    use crate::engine::Surface;

    #[test]
    fn skill_launch_seed_starts_with_slash_step_and_surface() {
        let seed = skill_launch_seed(
            Surface::Headless,
            "implement",
            Some("build auth"),
            Some("Be terse."),
        );
        assert!(seed.starts_with("/implement\n\n"));
        assert!(seed.contains("Run mode is headless"));
        assert!(seed.contains("<lf:voice>\nBe terse.\n</lf:voice>"));
        assert!(seed.contains("<lf:orientation>"));
        assert!(seed.contains("scratch/"));
        assert!(seed.contains("<lf:message>\nbuild auth\n</lf:message>"));
    }

    #[test]
    fn skill_launch_seed_omits_voice_and_message_when_absent() {
        let seed = skill_launch_seed(Surface::Cli, "gate", None, None);
        assert!(seed.starts_with("/gate\n\n"));
        assert!(!seed.contains("<lf:voice>"));
        assert!(!seed.contains("<lf:message>"));
        // Orientation is always present — every handoff should read scratch/.
        assert!(seed.contains("<lf:orientation>"));
    }

    #[test]
    fn external_skill_steps_keep_assembled_prompt_fallback() {
        assert!(!should_launch_via_skill("npx/vercel-labs/deep-research"));
        assert!(!should_launch_via_skill("rams/rams"));
        assert!(should_launch_via_skill("implement"));
    }

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
    fn split_step_args_preserves_namespaced_step() {
        let args = vec!["npx/explain-code".to_string()];
        let (step, rest) = split_step_args(&args).expect("split args");
        assert_eq!(step, "npx/explain-code");
        assert!(rest.is_empty());
    }

    #[test]
    fn split_step_args_preserves_namespaced_step_with_args() {
        let args = vec!["gstack/office-hours".to_string(), "auth flow".to_string()];
        let (step, rest) = split_step_args(&args).expect("split args");
        assert_eq!(step, "gstack/office-hours");
        assert_eq!(rest, vec!["auth flow".to_string()]);
    }
}
