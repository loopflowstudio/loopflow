use crate::engine::{
    check_cli_available, launch_agent, load_config_or_default, parse_agent, prepare_launch_prompt,
    write_prompt_log, AgentCapabilities, AgentConfig, Config, ContextSourceOverrides,
    LaunchPromptInput, LaunchTarget, ProcessConfig, PromptComponents, SkillSyncOptions,
    StreamFormat, Surface,
};
use crate::lf::commands::util::{find_repo_root, launch_session};
use crate::lf::output::{format_context_header, format_reproducible_command, Colors};
use crate::lf::Cli;
use anyhow::{anyhow, Result};
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::time::Instant;
use tracing::{debug, info, instrument, trace, warn};

/// Unified entry point for running skills, inline prompts, or interactive chat.
///
/// | skill    | message | behavior                              |
/// |---------|---------|---------------------------------------|
/// | Some    | None    | Run named skill                        |
/// | None    | Some    | Run inline prompt                     |
/// | Some    | Some    | Run skill with message as extra context |
/// | None    | None    | Interactive chat                      |
#[instrument(skip(cli), fields(skill = ?skill, has_message = message.is_some()))]
pub fn run(skill: Option<&str>, message: Option<&str>, cli: &Cli) -> Result<()> {
    let mut built = build_prompt(skill, message, cli)?;
    built.subject = cli.work_subject_selector();

    print_context_header(&built, cli);
    launch_prompt(&built, cli)
}

#[doc(hidden)]
pub fn run_bound(
    skill: &str,
    message: Option<&str>,
    cli: &Cli,
    binding: &crate::ops::WorkBinding,
) -> Result<()> {
    let message = match message.filter(|message| !message.trim().is_empty()) {
        Some(message) => format!(
            "<lf:work kind=\"{}\" id=\"{}\">\n{}\n</lf:work>\n\n{}",
            binding.work.kind(),
            binding.work.id(),
            binding.context,
            message,
        ),
        None => format!(
            "<lf:work kind=\"{}\" id=\"{}\">\n{}\n</lf:work>",
            binding.work.kind(),
            binding.work.id(),
            binding.context,
        ),
    };
    let mut built = build_bound_prompt_at(skill, &message, cli, &binding.cwd)?;
    built.agent_config.cwd = Some(binding.cwd.clone());
    built.agent_config.env.insert(
        crate::work::wave::context::WAVE_ID_ENV.to_string(),
        binding.wave_id.to_string(),
    );
    built.subject = Some(format!("{}:{}", binding.work.kind(), binding.work.id()));

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
    context: crate::trace::PreparedTurnContext,
    prompt: String,
    harness: String,
    model: Option<String>,
    skill_name: Option<String>,
    log_name: String,
    subject: Option<String>,
}

/// A skill turn ready for a runner-owned provider surface.
#[derive(Debug)]
pub(crate) struct PreparedHarnessTurn {
    pub config: AgentConfig,
    pub input: String,
    pub context: crate::trace::PreparedTurnContext,
    pub harness: String,
    pub model: Option<String>,
}

pub(crate) fn prepare_harness_turn(
    skill: &str,
    message: &str,
    wave: &str,
    max_turns: Option<u32>,
) -> Result<PreparedHarnessTurn> {
    let cli = Cli {
        batch: true,
        wave: Some(wave.to_string()),
        max_turns,
        ..Cli::default()
    };
    prepare_runner_turn(skill, message, &cli)
}

pub(crate) fn prepare_harness_turn_at(
    skill: &str,
    message: &str,
    wave: &str,
    max_turns: Option<u32>,
    repo_root: &Path,
) -> Result<PreparedHarnessTurn> {
    let cli = Cli {
        batch: true,
        wave: Some(wave.to_string()),
        max_turns,
        ..Cli::default()
    };
    prepare_runner_turn_at(skill, message, &cli, repo_root.to_path_buf(), true)
}

pub(crate) fn prepare_wave_harness_turn(
    skill: &str,
    message: &str,
    wave: &str,
    max_turns: Option<u32>,
    origin_repo: &std::path::Path,
    resident_repo: &std::path::Path,
) -> Result<PreparedHarnessTurn> {
    let cli = Cli {
        batch: true,
        wave: Some(wave.to_string()),
        max_turns,
        ..Cli::default()
    };
    let mut prepared =
        prepare_runner_turn_at(skill, message, &cli, origin_repo.to_path_buf(), true)?;
    prepared.config.cwd = Some(resident_repo.to_path_buf());
    Ok(prepared)
}

pub(crate) fn prepare_interactive_harness_turn_at(
    skill: &str,
    message: &str,
    wave: &str,
    repo_root: &Path,
) -> Result<PreparedHarnessTurn> {
    let cli = Cli {
        interactive: true,
        wave: Some(wave.to_string()),
        ..Cli::default()
    };
    // A durable human gate must receive the exact Task basis. Native vendor
    // skill launches replace the assembled prompt with a sigil and would lose
    // scratch documents that the human is here to review.
    prepare_runner_turn_at(skill, message, &cli, repo_root.to_path_buf(), false)
}

fn prepare_runner_turn(skill: &str, message: &str, cli: &Cli) -> Result<PreparedHarnessTurn> {
    let repo_root = find_repo_root()?;
    prepare_runner_turn_at(skill, message, cli, repo_root, true)
}

fn prepare_runner_turn_at(
    skill: &str,
    message: &str,
    cli: &Cli,
    repo_root: PathBuf,
    use_native_skill_launch: bool,
) -> Result<PreparedHarnessTurn> {
    let mut built = build_prompt_at(
        Some(skill),
        Some(message),
        cli,
        repo_root,
        use_native_skill_launch,
        Some((
            crate::trace::ContextAssetKind::Goal,
            crate::trace::ContextScope::Step,
        )),
    )?;
    let input = std::mem::take(&mut built.agent_config.task_prompt);
    Ok(PreparedHarnessTurn {
        config: built.agent_config,
        input,
        context: built.context,
        harness: built.harness,
        model: built.model,
    })
}

fn build_prompt(skill: Option<&str>, message: Option<&str>, cli: &Cli) -> Result<PromptBuild> {
    let start = Instant::now();
    let repo_root = find_repo_root()?;
    debug!(elapsed_ms = start.elapsed().as_millis(), "found repo root");
    build_prompt_at(skill, message, cli, repo_root, true, None)
}

fn build_bound_prompt_at(
    skill: &str,
    message: &str,
    cli: &Cli,
    repo_root: &Path,
) -> Result<PromptBuild> {
    // A Work-bound launch carries context that cannot be reconstructed by a
    // vendor skill sigil: the selected Work seed and the Task worktree's exact
    // scratch snapshot. Keep the assembled prompt on interactive surfaces too.
    build_prompt_at(
        Some(skill),
        Some(message),
        cli,
        repo_root.to_path_buf(),
        false,
        Some((
            crate::trace::ContextAssetKind::Goal,
            crate::trace::ContextScope::Task,
        )),
    )
}

fn build_prompt_at(
    skill: Option<&str>,
    message: Option<&str>,
    cli: &Cli,
    repo_root: PathBuf,
    use_native_skill_launch: bool,
    message_context: Option<(crate::trace::ContextAssetKind, crate::trace::ContextScope)>,
) -> Result<PromptBuild> {
    let config_start = Instant::now();
    let config = load_config_or_default(Some(&repo_root));
    debug!(
        elapsed_ms = config_start.elapsed().as_millis(),
        "loaded config"
    );
    trace!(
        agent = config.agent(),
        ?config.yolo,
        "loaded config"
    );

    let discover_start = Instant::now();
    let discovered_skill = if let Some(skill_name) = skill {
        Some(crate::lf::discovery::discover_skill(
            &repo_root, skill_name,
        )?)
    } else {
        None
    };
    debug!(
        elapsed_ms = discover_start.elapsed().as_millis(),
        "discovered skill"
    );

    let is_interactive = is_interactive_run(cli, skill, message);

    info!("preparing launch prompt");
    let prepare_start = Instant::now();
    let surface = if cli.ide {
        Surface::Ide
    } else if is_interactive {
        Surface::Cli
    } else {
        Surface::Headless
    };

    let wave = cli
        .wave
        .clone()
        .or_else(crate::work::wave::context::resolve_ambient_wave_name);
    let wave_memory = wave
        .as_deref()
        .and_then(|wave| crate::work::wave::context::gather_wave_memory(&repo_root, wave));
    let prepared = prepare_launch_prompt(
        &config,
        LaunchPromptInput {
            repo_root: repo_root.clone(),
            skill: skill.map(|value| value.to_string()),
            resolved_skill: discovered_skill.clone(),
            surface,
            directions: cli.direction.clone(),
            docs: cli.docs.clone(),
            wave,
            wave_memory,
            message: message.map(|value| value.to_string()),
            no_loopflow: cli.no_loopflow,
            agent: cli.model.clone(),
            cwd: Some(repo_root.clone()),
            max_turns: cli.max_turns,
            yolo_mode: cli.yolo || config.yolo,
            include_config_directions: !cli.no_direction,
            source_overrides: ContextSourceOverrides {
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

    let skill_name = discovered_skill
        .as_ref()
        .map(|skill| skill.name.clone())
        .or_else(|| skill.map(|value| value.to_string()));
    let log_name = skill_name
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

    let mut agent_config = prepared.config;
    let mut prompt = prepared.prompt;
    // Interactive handoffs use the vendor skill sigil because the vendor owns
    // the session from that point. Headless launches keep the fully assembled
    // prompt so every context source remains explicit and attributable.
    if let Some(skill_name) = skill_name.as_deref() {
        if is_interactive && use_native_skill_launch && should_launch_via_skill(skill_name) {
            let sync_start = Instant::now();
            crate::engine::sync_skills(&SkillSyncOptions::default())?;
            debug!(
                elapsed_ms = sync_start.elapsed().as_millis(),
                "synced vendor skills"
            );
            let wave_memory =
                crate::engine::prompt::format_wave_memory_section(&prepared.components);
            prompt = skill_launch_seed(
                &harness,
                surface,
                skill_name,
                message,
                prepared.components.operate,
                wave_memory.as_deref(),
            );
            agent_config.system_prompt.clear();
            agent_config.task_prompt = prompt.clone();
        } else if is_interactive && use_native_skill_launch {
            warn!(
                skill = skill_name,
                "external skill uses assembled prompt fallback"
            );
        }
    }

    let mut components = prepared.components;
    components.message_context = message_context;
    let deduplicated_docs = prepared.deduplicated_docs;
    let effective_system =
        crate::engine::agent::system_prompt_with_structured_replies(&agent_config);
    let context = attributed_context(
        &components,
        &effective_system,
        &agent_config.task_prompt,
        &deduplicated_docs,
    );
    Ok(PromptBuild {
        repo_root,
        config,
        agent_config,
        process,
        capabilities,
        components,
        context,
        prompt,
        harness,
        model,
        skill_name,
        log_name,
        subject: None,
    })
}

pub(crate) fn is_interactive_run(cli: &Cli, skill: Option<&str>, message: Option<&str>) -> bool {
    is_interactive_run_with_tty(
        cli,
        skill,
        message,
        std::io::stdin().is_terminal() || std::io::stdout().is_terminal(),
    )
}

fn is_interactive_run_with_tty(
    cli: &Cli,
    skill: Option<&str>,
    message: Option<&str>,
    attached_tty: bool,
) -> bool {
    cli.tui
        || cli.ide
        || cli.interactive
        || (!cli.batch && (attached_tty || (skill.is_none() && message.is_none())))
}

fn should_launch_via_skill(skill_name: &str) -> bool {
    !skill_name.starts_with("npx/") && !skill_name.starts_with("rams/")
}

/// Build the launch seed for a vendor skill handoff: the skill invocation,
/// system-safe instruction sections, Wave memory (when non-empty), and an
/// optional user message. Orientation now
/// lives in the skill bodies themselves, and the skill body loads from the
/// synced skill on invoke, so this stays small enough for the GUI deep-link
/// cap.
///
/// The invocation sigil is harness-specific: Codex's interactive composer
/// reserves `/` for built-in commands, so skills fire with `$name` there (and
/// `$` works in `codex exec` too). Claude uses `/name` everywhere.
fn skill_launch_seed(
    harness: &str,
    surface: Surface,
    skill_name: &str,
    message: Option<&str>,
    loopflow: bool,
    wave_memory: Option<&str>,
) -> String {
    let sigil = if harness == "codex" { '$' } else { '/' };
    let system_components = PromptComponents {
        surface,
        operate: loopflow,
        ..Default::default()
    };
    let system_sections = crate::engine::prompt::format_system_sections(&system_components);
    let mut seed = format!("{sigil}{skill_name}\n\n{}", system_sections.join("\n\n"));
    if let Some(memory) = wave_memory {
        seed.push_str("\n\n");
        seed.push_str(memory);
    }
    if let Some(message) = message.filter(|value| !value.trim().is_empty()) {
        seed.push_str("\n\n<lf:message>\n");
        seed.push_str(message);
        seed.push_str("\n</lf:message>");
    }
    seed
}

fn print_context_header(built: &PromptBuild, cli: &Cli) {
    let colors = Colors::new();
    let header = format_context_header(&built.context, &built.components);
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
        built.skill_name.as_deref(),
        &direction_names,
        built.components.wave.as_deref(),
        &cli.docs,
        cli.clipboard,
        cli.no_loopflow,
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
    // Bare terminal control always stays in the TUI. Other human-present skills
    // use explicit flags first, then the configured launch target.
    let forced_target = if built.skill_name.as_deref() == Some("loopflow") {
        Some(LaunchTarget::Tui)
    } else if cli.ide {
        Some(LaunchTarget::Ide)
    } else if cli.tui {
        Some(LaunchTarget::Tui)
    } else {
        None
    };

    if forced_target.is_some() || !built.process.auto {
        info!("launching interactive vendor session");
        let target = forced_target.unwrap_or(built.config.session.launch);
        let surface = if target == LaunchTarget::Ide {
            "ide"
        } else {
            "tui"
        };
        let capture = begin_run_capture(built, surface, &built.agent_config)?;
        let result = launch_session(
            target,
            &built.harness,
            built.model.as_deref(),
            &built.repo_root,
            &built.prompt,
        );
        if target == LaunchTarget::Ide && result.is_ok() {
            capture.mark_handoff(surface);
        } else {
            capture.finish(if result.is_ok() {
                "completed"
            } else {
                "failed"
            })?;
        }
        return result;
    }

    let cli_check_start = Instant::now();
    if !check_cli_available(&built.harness) {
        return Err(anyhow!(
            "'{}' CLI not found. Install it and rerun `lf init`.",
            built.harness
        ));
    }
    debug!(
        elapsed_ms = cli_check_start.elapsed().as_millis(),
        "checked cli availability"
    );

    let mut agent_config = built.agent_config.clone();
    crate::engine::agent::pin_provider_account_id_blocking(&mut agent_config)
        .map_err(anyhow::Error::from)?;
    let effective_system =
        crate::engine::agent::system_prompt_with_structured_replies(&agent_config);
    let capture = begin_run_capture(built, "headless", &agent_config)?;

    let result = launch_headless_prompt(built, &capture, &effective_system, &agent_config);
    let outcome = if result.is_ok() {
        "completed"
    } else {
        "failed"
    };
    let settlement = capture.finish(outcome);
    match (result, settlement) {
        (Err(error), Err(settlement)) => {
            Err(error.context(format!("Run execution also failed to settle: {settlement}")))
        }
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(anyhow!("Run completed but did not settle: {error}")),
        (Ok(()), Ok(())) => Ok(()),
    }
}

fn launch_headless_prompt(
    built: &PromptBuild,
    capture: &crate::run_record::CaptureHandle,
    effective_system: &str,
    prepared_config: &AgentConfig,
) -> Result<()> {
    // Skill-launched skills clear the system prompt (the seed carries everything
    // in the task prompt). Don't write or pass a context file in that case: codex
    // treats an empty `model_instructions_file` as an error.
    let context_file_start = Instant::now();
    let context_file = if effective_system.trim().is_empty() {
        None
    } else {
        Some(write_prompt_log(
            &built.repo_root,
            effective_system,
            &format!("{}.context", built.log_name),
            None,
        )?)
    };
    debug!(
        elapsed_ms = context_file_start.elapsed().as_millis(),
        "wrote context log"
    );

    let use_color = std::env::var("NO_COLOR").is_err() && std::io::stderr().is_terminal();
    let mut process = built.process.clone();
    process.context_file = context_file;
    process.stream_format = StreamFormat::Human(use_color);
    process.capture = Some(capture.clone().into());

    // Set up directive relay so agent skills can issue shell directives
    // (e.g. `cd` after `lf pr land` rotates worktrees).
    let directive_file = std::env::var("LOOPFLOW_DIRECTIVE_FILE").ok();
    let mut agent_config = prepared_config.clone();
    let relay_path = directive_file.as_ref().and_then(|_| {
        tempfile::NamedTempFile::new()
            .ok()
            .map(|f| f.into_temp_path().to_path_buf())
    });
    if let Some(ref path) = relay_path {
        agent_config.directive_relay = Some(path.clone());
    }

    debug!(launch = ?agent_config, ?process, ?built.capabilities, "launching agent");

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
    } else if let Some(failure) = &result.failure {
        Err(anyhow!(
            "agent stopped after {failure}. Check {} for details.",
            capture.artifact_dir().display()
        ))
    } else {
        Err(anyhow!(
            "agent exited with code {}. Check {} for details.",
            result.exit_code,
            capture.artifact_dir().display()
        ))
    }
}

fn begin_run_capture(
    built: &PromptBuild,
    surface: &str,
    prepared_config: &AgentConfig,
) -> Result<crate::run_record::CaptureHandle> {
    let cwd = built
        .agent_config
        .cwd
        .clone()
        .unwrap_or_else(|| built.repo_root.clone());
    let spec = crate::run_record::RunSpec {
        harness: built.harness.clone(),
        model: built.model.clone(),
        surface: surface.to_string(),
        cwd,
        repo: Some(built.repo_root.clone()),
        worktree: Some(built.repo_root.clone()),
        skill: built.skill_name.clone(),
        subjects: built
            .subject
            .clone()
            .map(crate::run_record::SubjectAttribution::declared)
            .into_iter()
            .collect(),
    };
    let capture = if surface == "headless" {
        let launch = crate::run_record::RunLaunchRequest::from_prepared(
            prepared_config,
            &built.capabilities,
        );
        crate::run_record::CaptureHandle::begin_with_launch_and_context(
            spec,
            launch,
            &built.context,
        )
    } else {
        crate::run_record::CaptureHandle::begin_with_context(spec, &built.context)
    }
    .map_err(|error| anyhow!("failed to publish Run manifest before agent launch: {error}"))?;
    capture.record_input("initial", &built.context.task.text);
    Ok(capture)
}

pub(crate) fn attributed_context(
    components: &PromptComponents,
    system_prompt: &str,
    task_prompt: &str,
    deduplicated_docs: &[crate::engine::Document],
) -> crate::trace::PreparedTurnContext {
    use crate::engine::prompt::{DiffTier, DocumentSource};
    use crate::trace::{
        ContextAssetKind as Kind, ContextAssetSpec, ContextChannel, ContextDecision,
        ContextDecisionKind, ContextScope as Scope,
    };

    let mut specs = Vec::new();
    let mut push = |content: &str,
                    kind: Kind,
                    scope: Scope,
                    label: String,
                    source_path: Option<String>,
                    included_by: &str| {
        if content.is_empty() {
            return;
        }
        for channel in [ContextChannel::System, ContextChannel::Task]
            .into_iter()
            .filter(|channel| match channel {
                ContextChannel::System => system_prompt.contains(content),
                ContextChannel::Task => task_prompt.contains(content),
            })
        {
            specs.push(ContextAssetSpec {
                channel,
                kind,
                scope,
                label: label.clone(),
                source_path: source_path.clone(),
                included_by: included_by.to_string(),
                content: content.to_string(),
                match_all_occurrences: !matches!(included_by, "message" | "vendor_skill"),
            });
        }
    };

    if components.operate {
        let source_path = std::path::Path::new(&components.repo_root)
            .join("rust/loopflow/src/engine/builtins/LOOPFLOW.md");
        push(
            &crate::engine::prompt::loopflow_section(),
            Kind::OperatingInstructions,
            Scope::Global,
            "LOOPFLOW.md".to_string(),
            source_path
                .is_file()
                .then(|| source_path.to_string_lossy().to_string()),
            "operate",
        );
    }
    if let Some(guidance) = tagged_block(
        system_prompt,
        "<lf:structured_replies>",
        "</lf:structured_replies>",
    ) {
        push(
            guidance,
            Kind::ProviderInstructions,
            Scope::Provider,
            "structured reply contract".to_string(),
            None,
            "provider_invocation",
        );
    }
    push(
        components.surface.instructions(),
        Kind::SurfaceInstructions,
        Scope::Global,
        format!("{:?} surface", components.surface),
        None,
        "surface",
    );
    for direction in &components.directions {
        push(
            &direction.content,
            Kind::Direction,
            Scope::Task,
            direction.name.clone(),
            direction
                .source
                .is_file()
                .then(|| direction.source.to_string_lossy().to_string()),
            "direction",
        );
    }
    if let Some(wave) = &components.wave {
        let open = format!("<lf:wave name=\"{wave}\">");
        let goal = tagged_block(task_prompt, &open, "</lf:wave>").unwrap_or(open.as_str());
        push(
            goal,
            Kind::Goal,
            Scope::Wave,
            wave.clone(),
            Some(format!("wave/{wave}/GOAL.md")),
            "wave",
        );
    }
    if let Some(memory) = &components.wave_memory {
        push(
            &memory.content,
            Kind::Memory,
            Scope::Wave,
            "wave memory".to_string(),
            Some(memory.path.clone()),
            "wave",
        );
    }
    for document in &components.docs {
        let kind = if document.source == DocumentSource::Scratch {
            Kind::Scratch
        } else if document.path.ends_with("AGENTS.md") || document.path.ends_with("CLAUDE.md") {
            Kind::RepoInstructions
        } else {
            Kind::Document
        };
        push(
            &document.content,
            kind,
            Scope::Repo,
            document.path.clone(),
            Some(document.path.clone()),
            "docs",
        );
    }
    for document in &components.diff_files {
        push(
            &document.content,
            Kind::Diff,
            Scope::Repo,
            document.path.clone(),
            Some(document.path.clone()),
            "diff_files",
        );
    }
    if let Some(diff) = &components.diff {
        push(
            diff,
            Kind::Diff,
            Scope::Repo,
            "branch diff".to_string(),
            None,
            "diff",
        );
    }
    for summary in &components.summaries {
        push(
            &summary.content,
            Kind::Summary,
            Scope::Task,
            summary.path.clone(),
            Some(summary.path.clone()),
            "summary",
        );
    }
    if let Some(clipboard) = &components.clipboard {
        push(
            clipboard,
            Kind::Clipboard,
            Scope::User,
            "clipboard".to_string(),
            None,
            "clipboard",
        );
    }
    if let Some(skill) = &components.skill {
        if let Some(content) = &skill.content {
            let source_path = crate::engine::find_skill_source_path(
                &skill.name,
                std::path::Path::new(&components.repo_root),
            );
            push(
                content,
                Kind::SkillInstructions,
                Scope::Step,
                skill.name.clone(),
                source_path.map(|path| path.to_string_lossy().to_string()),
                "skill",
            );
        } else {
            push(
                &skill.name,
                Kind::SkillInstructions,
                Scope::Step,
                skill.name.clone(),
                None,
                "vendor_skill",
            );
        }
    }
    if let Some(message) = &components.message {
        let (kind, scope) = components
            .message_context
            .unwrap_or((Kind::UserMessage, Scope::User));
        push(
            message,
            kind,
            scope,
            if kind == Kind::UserMessage {
                "user message".to_string()
            } else {
                "inherited launch goal".to_string()
            },
            None,
            "message",
        );
    }

    let mut decisions = Vec::new();
    for (position, document) in deduplicated_docs.iter().enumerate() {
        decisions.push(ContextDecision {
            position: position as u32,
            kind: Kind::RepoInstructions,
            scope: Scope::Repo,
            label: document.path.clone(),
            source_path: Some(document.path.clone()),
            decision: ContextDecisionKind::Deduplicated,
            reason: "provider-native instruction discovery owns this file or its symlink target"
                .to_string(),
            original_bytes: Some(document.content.len() as u64),
            original_tokens: Some(crate::engine::prompt::count_tokens(&document.content) as u64),
            asset_position: None,
        });
    }
    if components.diff_tier == DiffTier::StatOnly {
        decisions.push(ContextDecision {
            position: decisions.len() as u32,
            kind: Kind::Diff,
            scope: Scope::Repo,
            label: "branch diff".to_string(),
            source_path: None,
            decision: ContextDecisionKind::StatOnly,
            reason: "unified diff exceeded the context tier limit".to_string(),
            original_bytes: None,
            original_tokens: None,
            asset_position: None,
        });
    }

    crate::trace::PreparedTurnContext::from_attributed_prompts(
        system_prompt,
        task_prompt,
        specs,
        decisions,
    )
}

fn tagged_block<'a>(text: &'a str, open: &str, close: &str) -> Option<&'a str> {
    let start = text.find(open)?;
    let end = text[start..].find(close)? + start + close.len();
    Some(&text[start..end])
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

pub fn split_skill_args(args: &[String]) -> Result<(String, Vec<String>)> {
    let first = args.first().ok_or_else(|| anyhow!("no skill specified"))?;

    let mut skill = first.clone();
    let skill_args = args.iter().skip(1).cloned().collect::<Vec<_>>();

    // Trailing colon is a separator: `implement: add auth` → skill="implement"
    if let Some(stripped) = skill.strip_suffix(':') {
        skill = stripped.to_string();
    }

    if skill.is_empty() {
        return Err(anyhow!("no skill specified"));
    }

    Ok((skill, skill_args))
}

#[cfg(test)]
mod tests {
    use super::{
        attributed_context, begin_run_capture, build_bound_prompt_at, is_interactive_run,
        is_interactive_run_with_tty, launch_headless_prompt, launch_prompt,
        prepare_wave_harness_turn, should_launch_via_skill, skill_launch_seed, split_skill_args,
        PromptBuild,
    };
    use crate::durable::RunId;
    use crate::engine::agent::{launch_agent, AgentCapabilities, AgentConfig, ProcessConfig};
    use crate::engine::prompt::{Document, DocumentSource, PromptComponents};
    use crate::engine::{Config, Surface};
    use crate::lf::Cli;
    use crate::trace::{ContextAssetKind, ContextScope};
    use clap::Parser;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    struct EnvironmentRestore(Vec<(&'static str, Option<std::ffi::OsString>)>);

    impl EnvironmentRestore {
        fn capture(keys: &[&'static str]) -> Self {
            Self(
                keys.iter()
                    .map(|key| (*key, std::env::var_os(key)))
                    .collect(),
            )
        }
    }

    impl Drop for EnvironmentRestore {
        fn drop(&mut self) {
            for (key, value) in self.0.drain(..).rev() {
                match value {
                    Some(value) => std::env::set_var(key, value),
                    None => std::env::remove_var(key),
                }
            }
        }
    }

    #[cfg(unix)]
    #[test]
    fn ad_hoc_batch_launch_uses_generic_run_record_without_planning_registry() {
        let _lock = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let evidence = home.path().join("provider-run");
        let implicit_evidence = home.path().join("implicit-provider-run");
        let provider = bin.join("gemini");
        std::fs::write(
            &provider,
            r#"#!/bin/sh
printf '%s\n' "$LF_RUN_ID|$LF_RUN_DIR|${LF_TRACE_ID-unset}|${LF_PROCESS_ID-unset}|${LF_PARENT_RUN_ID-unset}" >> "$LF_TEST_RUN_EVIDENCE"
if [ -n "${LF_TEST_ATTEMPT_FILE:-}" ] && [ ! -e "$LF_TEST_ATTEMPT_FILE" ]; then
  touch "$LF_TEST_ATTEMPT_FILE"
  printf '%s\n' '{"type":"result","is_error":true,"result":"service unavailable"}'
  exit 1
fi
printf '%s\n' '{"type":"result","subtype":"success","usage":{"input_tokens":7,"output_tokens":3}}'
"#,
        )
        .unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();

        let keys = [
            "PATH",
            "LF_BIN",
            "LF_HOME",
            "LF_DB_PATH",
            crate::journal::LF_TRACE_ID_ENV,
            crate::journal::LF_PROCESS_ID_ENV,
            crate::durable::RUN_ID_ENV,
            crate::run_record::RUN_DIR_ENV,
            crate::run_record::PARENT_RUN_ID_ENV,
            crate::store::CONTROL_HOME_ENV,
            crate::store::CONTROL_DB_PATH_ENV,
        ];
        let _environment = EnvironmentRestore::capture(&keys);
        let path = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        std::env::set_var("PATH", path);
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        std::env::set_var("LF_HOME", home.path());
        let registry_blocker = home.path().join("unreadable-registry");
        std::fs::create_dir(&registry_blocker).unwrap();
        std::env::set_var("LF_DB_PATH", &registry_blocker);
        std::env::set_var(crate::journal::LF_TRACE_ID_ENV, "trace_stale");
        std::env::set_var(crate::journal::LF_PROCESS_ID_ENV, "process_stale");
        std::env::set_var(crate::durable::RUN_ID_ENV, RunId::new().as_str());
        std::env::set_var(
            crate::run_record::RUN_DIR_ENV,
            home.path().join("stale-run"),
        );
        std::env::set_var(crate::run_record::PARENT_RUN_ID_ENV, RunId::new().as_str());
        std::env::remove_var(crate::store::CONTROL_HOME_ENV);
        std::env::remove_var(crate::store::CONTROL_DB_PATH_ENV);

        let task = "prove the generic Run launch";
        let context = crate::trace::PreparedTurnContext::from_prompts("", task);
        let mut env = std::collections::BTreeMap::new();
        env.insert(
            "LF_TEST_RUN_EVIDENCE".to_string(),
            evidence.display().to_string(),
        );
        let built = PromptBuild {
            repo_root: home.path().to_path_buf(),
            config: Config::default(),
            agent_config: AgentConfig {
                task_prompt: task.to_string(),
                agent: Some("gemini".to_string()),
                cwd: Some(home.path().to_path_buf()),
                skip_permissions: true,
                env,
                ..AgentConfig::default()
            },
            process: ProcessConfig {
                auto: true,
                ..ProcessConfig::default()
            },
            capabilities: AgentCapabilities::default(),
            components: PromptComponents::default(),
            context,
            prompt: task.to_string(),
            harness: "gemini".to_string(),
            model: None,
            skill_name: Some("implement".to_string()),
            log_name: "generic-run-proof".to_string(),
            subject: Some("task:LOO-265".to_string()),
        };
        let capture = begin_run_capture(&built, "headless", &built.agent_config).unwrap();
        let run_id = capture.run_id();
        let run_dir = capture.artifact_dir();

        assert!(run_dir.join("manifest.json").is_file());
        let manifest = std::fs::read_to_string(run_dir.join("manifest.json")).unwrap();
        assert!(manifest.contains("task:LOO-265"));
        assert!(!run_dir.join("terminal.json").exists());
        let effective_system =
            crate::engine::agent::system_prompt_with_structured_replies(&built.agent_config);
        let result =
            launch_headless_prompt(&built, &capture, &effective_system, &built.agent_config);
        capture
            .finish(if result.is_ok() {
                "completed"
            } else {
                "failed"
            })
            .unwrap();
        result.unwrap();

        let provider_identity = std::fs::read_to_string(evidence).unwrap();
        assert_eq!(
            provider_identity.trim(),
            format!("{}|{}|unset|unset|unset", run_id, run_dir.display())
        );
        assert!(run_dir.join("terminal.json").is_file());
        assert!(!run_dir.join("owner.json").exists());
        let events = std::fs::read_to_string(run_dir.join("events.jsonl")).unwrap();
        assert!(events.contains("\"type\":\"usage\""));

        let mut implicit_launch = built.agent_config.clone();
        implicit_launch.env.insert(
            "LF_TEST_RUN_EVIDENCE".to_string(),
            implicit_evidence.display().to_string(),
        );
        implicit_launch.env.insert(
            "LF_TEST_ATTEMPT_FILE".to_string(),
            home.path().join("implicit-attempt").display().to_string(),
        );
        let result = launch_agent(&implicit_launch, &built.process, &built.capabilities).unwrap();
        assert_eq!(result.exit_code, 0);
        let implicit_identities = std::fs::read_to_string(implicit_evidence).unwrap();
        let identities = implicit_identities.lines().collect::<Vec<_>>();
        assert_eq!(identities.len(), 2, "transient failure should retry once");
        assert_eq!(identities[0], identities[1], "retry must stay in one Run");
        let fields = identities[0].split('|').collect::<Vec<_>>();
        assert_eq!(&fields[2..], ["unset", "unset", "unset"]);
        let implicit_run_id = RunId::parse(fields[0]).unwrap();
        let implicit_run_dir = std::path::Path::new(fields[1]);
        assert_eq!(
            implicit_run_dir.file_name().and_then(|name| name.to_str()),
            Some(implicit_run_id.as_str())
        );
        assert!(implicit_run_dir.join("manifest.json").is_file());
        assert!(implicit_run_dir.join("terminal.json").is_file());
        assert!(!implicit_run_dir.join("owner.json").exists());
        let implicit_events =
            std::fs::read_to_string(implicit_run_dir.join("events.jsonl")).unwrap();
        assert_eq!(
            implicit_events.matches("provider_attempt_started").count(),
            2
        );
        assert!(registry_blocker.is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn two_task_research_runs_leave_distinct_uncommitted_artifacts_for_the_next_prompt() {
        fn research_build(
            repo: &std::path::Path,
            output: &std::path::Path,
            content: &str,
            delay: &str,
            log_name: &str,
        ) -> PromptBuild {
            let task = "research one bounded part of LOO-267";
            let mut env = std::collections::BTreeMap::new();
            env.insert(
                "LF_TEST_RESEARCH_OUTPUT".to_string(),
                output.display().to_string(),
            );
            env.insert("LF_TEST_RESEARCH_CONTENT".to_string(), content.to_string());
            env.insert("LF_TEST_RESEARCH_DELAY".to_string(), delay.to_string());
            PromptBuild {
                repo_root: repo.to_path_buf(),
                config: Config::default(),
                agent_config: AgentConfig {
                    task_prompt: task.to_string(),
                    agent: Some("gemini".to_string()),
                    cwd: Some(repo.to_path_buf()),
                    skip_permissions: true,
                    env,
                    ..AgentConfig::default()
                },
                process: ProcessConfig {
                    auto: true,
                    ..ProcessConfig::default()
                },
                capabilities: AgentCapabilities::default(),
                components: PromptComponents::default(),
                context: crate::trace::PreparedTurnContext::from_prompts("", task),
                prompt: task.to_string(),
                harness: "gemini".to_string(),
                model: None,
                skill_name: Some("research".to_string()),
                log_name: log_name.to_string(),
                subject: Some("task:LOO-267".to_string()),
            }
        }

        let _lock = crate::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let provider = bin.join("gemini");
        std::fs::write(
            &provider,
            r#"#!/bin/sh
if [ "${1:-}" = "--version" ]; then
  printf '%s\n' 'gemini test'
  exit 0
fi
sleep "$LF_TEST_RESEARCH_DELAY"
mkdir -p "$(dirname "$LF_TEST_RESEARCH_OUTPUT")"
temporary="$LF_TEST_RESEARCH_OUTPUT.$LF_RUN_ID.tmp"
printf '%s\n' "$LF_TEST_RESEARCH_CONTENT" > "$temporary"
mv "$temporary" "$LF_TEST_RESEARCH_OUTPUT"
printf '%s\n' '{"type":"result","subtype":"success","usage":{"input_tokens":7,"output_tokens":3}}'
"#,
        )
        .unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();

        // A suite launched from a live agent session must not inherit that
        // session's execution identity.
        let ambient_identity = [
            crate::durable::RUN_ID_ENV,
            crate::run_record::RUN_DIR_ENV,
            crate::run_record::PARENT_RUN_ID_ENV,
            "LF_WAVE_ID",
            "LF_ACCOUNT_LEASE",
        ];
        let keys: Vec<&'static str> = ["PATH", "LF_BIN", "LF_HOME"]
            .into_iter()
            .chain(ambient_identity)
            .collect();
        let _environment = EnvironmentRestore::capture(&keys);
        for name in ambient_identity {
            std::env::remove_var(name);
        }
        std::env::set_var(
            "PATH",
            format!(
                "{}:{}",
                bin.display(),
                std::env::var("PATH").unwrap_or_default()
            ),
        );
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        std::env::set_var("LF_HOME", home.path());

        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(".lf/skills/proof.md", "inspect every research artifact");
        repo.stage_all();
        repo.commit("test basis");
        let head = crate::engine::git::rev_parse(repo.path(), "HEAD").unwrap();
        let runtime = repo.path().join("scratch/research-runtime-model.md");
        let handoff = repo.path().join("scratch/research-design-handoff.md");
        let first = research_build(
            repo.path(),
            &runtime,
            "runtime evidence bytes",
            "0.2",
            "runtime-research",
        );
        let second = research_build(
            repo.path(),
            &handoff,
            "handoff evidence bytes",
            "0",
            "handoff-research",
        );
        let cli = Cli::default();

        std::thread::scope(|scope| {
            let first = scope.spawn(|| launch_prompt(&first, &cli));
            let second = scope.spawn(|| launch_prompt(&second, &cli));
            first.join().unwrap().unwrap();
            second.join().unwrap().unwrap();
        });

        assert_eq!(
            std::fs::read_to_string(&runtime).unwrap(),
            "runtime evidence bytes\n"
        );
        assert_eq!(
            std::fs::read_to_string(&handoff).unwrap(),
            "handoff evidence bytes\n"
        );
        assert_eq!(
            crate::engine::git::rev_parse(repo.path(), "HEAD").unwrap(),
            head
        );
        assert!(std::process::Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(repo.path())
            .status()
            .unwrap()
            .success());
        assert_eq!(
            crate::run_record::observed_run_ids(&["task:LOO-267".to_string()])
                .unwrap()
                .len(),
            2
        );

        let built = build_bound_prompt_at("proof", "reconcile", &cli, repo.path()).unwrap();
        assert!(built
            .agent_config
            .task_prompt
            .contains("runtime evidence bytes"));
        assert!(built
            .agent_config
            .task_prompt
            .contains("handoff evidence bytes"));
    }

    #[test]
    fn wave_harness_loads_canonical_skill_and_executes_in_resident_worktree() {
        let origin = loopflow_test_support::TestRepo::new();
        origin.create_file(".lf/skills/proof.md", "canonical skill instructions");
        origin.stage_all();
        origin.commit("canonical skill");
        let resident = loopflow_test_support::TestRepo::new();
        resident.create_file(".lf/skills/proof.md", "stale resident skill instructions");
        resident.stage_all();
        resident.commit("stale resident skill");

        let prepared = prepare_wave_harness_turn(
            "proof",
            "continue",
            "ship",
            Some(4),
            origin.path(),
            resident.path(),
        )
        .unwrap();

        assert_eq!(prepared.config.cwd.as_deref(), Some(resident.path()));
        assert!(prepared.input.contains("canonical skill instructions"));
        assert!(!prepared.input.contains("stale resident skill instructions"));
    }

    #[test]
    fn worktree_harness_preloads_committed_and_untracked_scratch_with_provenance() {
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(".lf/skills/proof.md", "inspect the complete basis");
        repo.create_file("scratch/a-committed.md", "committed evidence bytes");
        repo.stage_all();
        repo.commit("committed basis");
        repo.create_file("scratch/z-untracked.md", "untracked evidence bytes");

        let cli = Cli {
            batch: true,
            wave: Some("ship".to_string()),
            ..Cli::default()
        };
        let built = build_bound_prompt_at("proof", "continue", &cli, repo.path()).unwrap();

        let committed = built
            .agent_config
            .task_prompt
            .find("committed evidence bytes")
            .unwrap();
        let untracked = built
            .agent_config
            .task_prompt
            .find("untracked evidence bytes")
            .unwrap();
        assert!(committed < untracked);
        assert!(built.context.task.assets.iter().any(|asset| {
            asset.kind == ContextAssetKind::Scratch
                && asset.source_path.as_deref() == Some("scratch/a-committed.md")
        }));
        assert!(built.context.task.assets.iter().any(|asset| {
            asset.kind == ContextAssetKind::Scratch
                && asset.source_path.as_deref() == Some("scratch/z-untracked.md")
        }));
    }

    #[test]
    fn interactive_bound_skill_keeps_the_assembled_scratch_snapshot() {
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(".lf/skills/proof.md", "inspect the complete basis");
        repo.create_file("scratch/research-runtime.md", "runtime evidence bytes");
        repo.stage_all();
        repo.commit("bound basis");
        let cli = Cli {
            interactive: true,
            ..Cli::default()
        };

        let built = build_bound_prompt_at(
            "proof",
            "<lf:work kind=\"task\" id=\"task_test\">Task seed</lf:work>",
            &cli,
            repo.path(),
        )
        .unwrap();
        repo.create_file(
            "scratch/research-runtime.md",
            "evidence published after launch",
        );

        assert!(built
            .agent_config
            .task_prompt
            .contains("runtime evidence bytes"));
        assert!(!built
            .agent_config
            .task_prompt
            .contains("evidence published after launch"));
        assert!(built
            .agent_config
            .task_prompt
            .contains("inspect the complete basis"));
        assert!(built.agent_config.task_prompt.contains("Task seed"));
        assert!(built.context.task.assets.iter().any(|asset| {
            asset.kind == ContextAssetKind::Scratch
                && asset.source_path.as_deref() == Some("scratch/research-runtime.md")
        }));
    }

    #[test]
    fn forced_session_handoff_counts_as_interactive() {
        let cli = Cli::parse_from(["lf", "--ide", "gate"]);

        assert!(is_interactive_run(&cli, Some("gate"), None));
    }

    #[test]
    fn batch_named_skill_is_headless() {
        let cli = Cli::parse_from(["lf", "--batch", "design"]);
        assert!(!is_interactive_run(&cli, Some("design"), None));
    }

    #[test]
    fn direct_tty_is_human_present_but_detached_named_launch_is_headless() {
        let cli = Cli::parse_from(["lf", "design"]);
        assert!(is_interactive_run_with_tty(
            &cli,
            Some("design"),
            None,
            true
        ));
        assert!(!is_interactive_run_with_tty(
            &cli,
            Some("design"),
            None,
            false
        ));
    }

    #[test]
    fn skill_launch_seed_starts_with_slash_skill_and_message() {
        let seed = skill_launch_seed(
            "claude",
            Surface::Cli,
            "implement",
            Some("build auth"),
            false,
            None,
        );
        assert!(seed.starts_with("/implement\n\n"));
        // Orientation now lives in the skill body, not the seed.
        assert!(!seed.contains("<lf:orientation>"));
        assert!(seed.contains("<lf:message>\nbuild auth\n</lf:message>"));
    }

    #[test]
    fn skill_launch_seed_uses_dollar_sigil_for_codex() {
        // Codex's interactive composer reserves `/` for built-in commands, so
        // skills fire with `$name`.
        let seed = skill_launch_seed("codex", Surface::Cli, "gate", None, false, None);
        assert!(seed.starts_with("$gate\n\n"));
    }

    #[test]
    fn skill_launch_seed_interactive_surfaces_have_no_preamble() {
        for surface in [Surface::Cli, Surface::Ide, Surface::Mac] {
            let seed = skill_launch_seed("claude", surface, "gate", None, false, None);
            assert!(seed.starts_with("/gate\n\n"));
            assert!(!seed.contains("Run mode"), "surface {surface:?}");
        }
    }

    #[test]
    fn skill_launch_seed_omits_message_when_absent() {
        let seed = skill_launch_seed("claude", Surface::Cli, "gate", None, false, None);
        assert!(!seed.contains("<lf:message>"));
        assert!(!seed.contains("<lf:orientation>"));
    }

    #[test]
    fn skill_launch_seed_headless_includes_preamble() {
        let seed = skill_launch_seed("claude", Surface::Headless, "implement", None, false, None);
        assert!(seed.contains("Run mode is headless"));
    }

    #[test]
    fn skill_launch_seed_omits_loopflow_when_disabled() {
        let seed = skill_launch_seed("claude", Surface::Headless, "implement", None, false, None);
        assert!(!seed.contains("<lf:loopflow>"));
        assert!(!seed.contains("lf commit"));
    }

    #[test]
    fn skill_launch_seed_includes_loopflow_when_enabled() {
        let seed = skill_launch_seed("claude", Surface::Headless, "implement", None, true, None);
        assert!(seed.contains("<lf:loopflow>"));
        assert!(seed.contains("lf commit"));
        assert!(seed.contains("</lf:loopflow>"));
        assert_eq!(
            seed.matches("<lf:loopflow>").count(),
            1,
            "the skill seed carries the loopflow operating document once"
        );
        assert!(seed.contains("Execute Here First"));
        assert!(seed.contains("edit\n`wave/<name>/MEMORY.md`"));
        assert!(!seed.contains("lf pm show"));
        assert!(!seed.contains("--detach"));
    }

    #[test]
    fn skill_launch_seed_carries_wave_memory_before_the_message() {
        let memory = "<lf:wave-memory>\n- prefer small PRs\n</lf:wave-memory>";
        let seed = skill_launch_seed(
            "claude",
            Surface::Headless,
            "implement",
            Some("build auth"),
            false,
            Some(memory),
        );
        let memory_pos = seed.find("<lf:wave-memory>").unwrap();
        let message_pos = seed.find("<lf:message>").unwrap();
        assert!(memory_pos < message_pos);
    }

    #[test]
    fn external_skill_skills_keep_assembled_prompt_fallback() {
        assert!(!should_launch_via_skill("npx/vercel-labs/deep-research"));
        assert!(!should_launch_via_skill("rams/rams"));
        assert!(should_launch_via_skill("implement"));
    }

    #[test]
    fn attributed_context_keeps_nested_message_and_repeated_channel_sources() {
        let components = PromptComponents {
            wave_memory: Some(Document {
                path: "wave/product/MEMORY.md".to_string(),
                content: "MEMORY".to_string(),
                source: DocumentSource::WaveMemory,
            }),
            message: Some("outer MEMORY remainder".to_string()),
            message_context: Some((ContextAssetKind::Goal, ContextScope::Step)),
            ..PromptComponents::default()
        };

        let prepared = attributed_context(&components, "MEMORY", "outer MEMORY remainder", &[]);

        assert_eq!(
            prepared.system.unwrap().assets[0].kind,
            ContextAssetKind::Memory
        );
        assert_eq!(
            prepared
                .task
                .assets
                .iter()
                .filter(|asset| asset.kind == ContextAssetKind::Memory)
                .count(),
            1
        );
        assert_eq!(
            prepared
                .task
                .assets
                .iter()
                .filter(|asset| asset.kind == ContextAssetKind::Goal)
                .count(),
            2
        );
        assert!(prepared
            .task
            .assets
            .iter()
            .all(|asset| asset.kind != ContextAssetKind::Assembly));
    }

    #[test]
    fn split_skill_args_handles_trailing_colon() {
        let args = vec![
            "implement:".to_string(),
            "add".to_string(),
            "logs".to_string(),
        ];
        let (skill, rest) = split_skill_args(&args).expect("split args");
        assert_eq!(skill, "implement");
        assert_eq!(rest, vec!["add".to_string(), "logs".to_string()]);
    }

    #[test]
    fn split_skill_args_preserves_namespaced_skill() {
        let args = vec!["npx/explain-code".to_string()];
        let (skill, rest) = split_skill_args(&args).expect("split args");
        assert_eq!(skill, "npx/explain-code");
        assert!(rest.is_empty());
    }

    #[test]
    fn split_skill_args_preserves_namespaced_skill_with_args() {
        let args = vec!["gstack/office-hours".to_string(), "auth flow".to_string()];
        let (skill, rest) = split_skill_args(&args).expect("split args");
        assert_eq!(skill, "gstack/office-hours");
        assert_eq!(rest, vec!["auth flow".to_string()]);
    }
}
