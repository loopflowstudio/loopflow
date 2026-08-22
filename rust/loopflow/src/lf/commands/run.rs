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
use std::path::PathBuf;
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
    let built = build_prompt(skill, message, cli)?;

    print_context_header(&built, cli);
    launch_prompt(&built, cli, None)
}

#[doc(hidden)]
pub fn run_bound(
    skill: &str,
    message: Option<&str>,
    cli: &Cli,
    store: crate::store::SharedStore,
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
    let built = build_prompt(Some(skill), Some(&message), cli)?;
    let surface = bound_surface(&built, cli)?;
    let route = crate::durable::InvocationRoute {
        provider: built.harness.clone(),
        model: built.model.clone(),
        account_id: None,
    };
    let runtime = tokio::runtime::Runtime::new()?;
    let mut direct =
        runtime.block_on(crate::ops::DirectRun::start(store, binding, route, surface))?;
    let _environment = BoundEnvironment::enter(&direct, binding);
    let control = CaptureControl {
        basis: direct.context().basis.clone(),
        supervision: crate::trace::SupervisedInvocation {
            invocation_id: direct.invocation().id.clone(),
            supervising_run_id: direct.context().run_id.clone(),
            account_id: direct.invocation().route.account_id.clone(),
            resume_token: direct.invocation().resume_token.clone(),
        },
    };

    print_context_header(&built, cli);
    let result = launch_prompt(&built, cli, Some(control));
    let outcome = if result.is_ok() {
        crate::durable::BoundaryState::Succeeded
    } else {
        crate::durable::BoundaryState::Failed
    };
    let cleanup = runtime.block_on(direct.finish(outcome));
    match (result, cleanup) {
        (Err(error), Err(cleanup)) => Err(error.context(format!(
            "bound Run {} also failed to settle: {cleanup}",
            direct.context().run_id
        ))),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(anyhow!(
            "bound skill completed but Run {} did not settle: {error}",
            direct.context().run_id
        )),
        (Ok(()), Ok(())) => Ok(()),
    }
}

struct PromptBuild {
    repo_root: PathBuf,
    config: Config,
    agent_config: AgentConfig,
    process: ProcessConfig,
    capabilities: AgentCapabilities,
    components: PromptComponents,
    deduplicated_docs: Vec<crate::engine::Document>,
    context: crate::trace::PreparedTurnContext,
    prompt: String,
    harness: String,
    model: Option<String>,
    skill_name: Option<String>,
    log_name: String,
    context_gather_ms: u64,
    context_render_ms: u64,
}

#[derive(Debug, Clone)]
struct CaptureControl {
    basis: crate::durable::Basis,
    supervision: crate::trace::SupervisedInvocation,
}

struct BoundEnvironment(Vec<(&'static str, Option<std::ffi::OsString>)>);

impl BoundEnvironment {
    fn enter(direct: &crate::ops::DirectRun, binding: &crate::ops::WorkBinding) -> Self {
        let values = [
            (crate::durable::RUN_CONTEXT_ENV, "agent"),
            (crate::durable::RUN_ID_ENV, direct.context().run_id.as_str()),
            (
                crate::durable::AGENT_INVOCATION_ENV,
                direct.invocation().id.as_str(),
            ),
            (
                crate::engine::wave_context::WAVE_ID_ENV,
                binding.wave_id.as_str(),
            ),
        ];
        let previous = values
            .iter()
            .map(|(key, value)| {
                let previous = std::env::var_os(key);
                std::env::set_var(key, value);
                (*key, previous)
            })
            .collect();
        Self(previous)
    }
}

impl Drop for BoundEnvironment {
    fn drop(&mut self) {
        for (key, value) in self.0.drain(..).rev() {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
    }
}

/// A skill turn ready for a runner-owned provider surface.
#[derive(Debug)]
pub(crate) struct PreparedHarnessTurn {
    pub config: AgentConfig,
    pub input: String,
    pub context: crate::trace::PreparedTurnContext,
    pub harness: String,
    pub model: Option<String>,
    pub context_gather_ms: u64,
    pub context_render_ms: u64,
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
    let mut prepared = prepare_runner_turn_at(skill, message, &cli, origin_repo.to_path_buf())?;
    prepared.config.cwd = Some(resident_repo.to_path_buf());
    Ok(prepared)
}

pub(crate) fn prepare_interactive_harness_turn(
    skill: &str,
    message: &str,
    wave: &str,
) -> Result<PreparedHarnessTurn> {
    let cli = Cli {
        interactive: true,
        wave: Some(wave.to_string()),
        ..Cli::default()
    };
    prepare_runner_turn(skill, message, &cli)
}

fn prepare_runner_turn(skill: &str, message: &str, cli: &Cli) -> Result<PreparedHarnessTurn> {
    let repo_root = find_repo_root()?;
    prepare_runner_turn_at(skill, message, cli, repo_root)
}

fn prepare_runner_turn_at(
    skill: &str,
    message: &str,
    cli: &Cli,
    repo_root: PathBuf,
) -> Result<PreparedHarnessTurn> {
    let mut built = build_prompt_at(Some(skill), Some(message), cli, repo_root)?;
    built.components.message_context = Some((
        crate::trace::ContextAssetKind::Goal,
        crate::trace::ContextScope::Step,
    ));
    let input = std::mem::take(&mut built.agent_config.task_prompt);
    let system_prompt =
        crate::engine::agent::system_prompt_with_structured_replies(&built.agent_config);
    let context = attributed_context(
        &built.components,
        &system_prompt,
        &input,
        &built.deduplicated_docs,
    );
    Ok(PreparedHarnessTurn {
        config: built.agent_config,
        input,
        context,
        harness: built.harness,
        model: built.model,
        context_gather_ms: built.context_gather_ms,
        context_render_ms: built.context_render_ms,
    })
}

fn build_prompt(skill: Option<&str>, message: Option<&str>, cli: &Cli) -> Result<PromptBuild> {
    let start = Instant::now();
    let repo_root = find_repo_root()?;
    debug!(elapsed_ms = start.elapsed().as_millis(), "found repo root");
    build_prompt_at(skill, message, cli, repo_root)
}

fn build_prompt_at(
    skill: Option<&str>,
    message: Option<&str>,
    cli: &Cli,
    repo_root: PathBuf,
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

    let prepared = prepare_launch_prompt(
        &config,
        LaunchPromptInput {
            repo_root: repo_root.clone(),
            skill: skill.map(|value| value.to_string()),
            resolved_skill: discovered_skill.clone(),
            surface,
            directions: cli.direction.clone(),
            docs: cli.docs.clone(),
            wave: cli.wave.clone(),
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
    let context_gather_ms = prepare_start.elapsed().as_millis() as u64;
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
        if is_interactive && should_launch_via_skill(skill_name) {
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
        } else if is_interactive {
            warn!(
                skill = skill_name,
                "external skill skill uses assembled prompt fallback"
            );
        }
    }

    let components = prepared.components;
    let deduplicated_docs = prepared.deduplicated_docs;
    let effective_system =
        crate::engine::agent::system_prompt_with_structured_replies(&agent_config);
    let render_start = Instant::now();
    let context = attributed_context(
        &components,
        &effective_system,
        &agent_config.task_prompt,
        &deduplicated_docs,
    );
    let context_render_ms = render_start.elapsed().as_millis() as u64;
    Ok(PromptBuild {
        repo_root,
        config,
        agent_config,
        process,
        capabilities,
        components,
        deduplicated_docs,
        context,
        prompt,
        harness,
        model,
        skill_name,
        log_name,
        context_gather_ms,
        context_render_ms,
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

fn bound_surface(built: &PromptBuild, cli: &Cli) -> Result<&'static str> {
    if built.process.auto {
        return Ok("headless");
    }
    let target = if cli.ide {
        LaunchTarget::Ide
    } else if cli.tui || built.skill_name.as_deref() == Some("loopflow") {
        LaunchTarget::Tui
    } else {
        built.config.session.launch
    };
    if target == LaunchTarget::Ide {
        return Err(anyhow!(
            "`lf --as` cannot supervise a vendor-app handoff; use `--tui` or `--batch`"
        ));
    }
    Ok("tui")
}

fn launch_prompt(built: &PromptBuild, cli: &Cli, control: Option<CaptureControl>) -> Result<()> {
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
        let capture = begin_capture(built, if cli.ide { "ide" } else { "tui" }, control.clone())?;
        let result = launch_session(
            forced_target.unwrap_or(built.config.session.launch),
            &built.harness,
            built.model.as_deref(),
            &built.repo_root,
            &built.prompt,
        );
        capture.finish(
            if result.is_ok() {
                "completed"
            } else {
                "failed"
            },
            true,
        )?;
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

    let effective_system =
        crate::engine::agent::system_prompt_with_structured_replies(&built.agent_config);
    let capture = begin_capture(built, "headless", control)?;

    // Skill-launched skills clear the system prompt (the seed carries everything
    // in the task prompt). Don't write or pass a context file in that case: codex
    // treats an empty `model_instructions_file` as an error.
    let context_file_start = Instant::now();
    let context_file = if effective_system.trim().is_empty() {
        None
    } else {
        Some(write_prompt_log(
            &built.repo_root,
            &effective_system,
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
    process.capture = Some(capture.clone());

    // Set up directive relay so agent skills can issue shell directives
    // (e.g. `cd` after `lf pr land` rotates worktrees).
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

    info!(harness = built.harness, "launching agent");
    let launch_start = Instant::now();
    let result = launch_agent(&agent_config, &process, &built.capabilities);

    // Relay safe directives from the agent back to the invoking shell.
    if let (Some(relay), Some(ref target)) = (relay_path, directive_file) {
        relay_directives(&relay, target);
    }

    let outcome = match &result {
        Ok(result) if result.exit_code == 0 => "completed",
        Ok(_) | Err(_) => "failed",
    };
    capture.finish(outcome, false)?;
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

fn begin_capture(
    built: &PromptBuild,
    surface: &str,
    control: Option<CaptureControl>,
) -> Result<crate::trace::CaptureHandle> {
    let context =
        crate::journal::trace_capture_context(&built.repo_root, None, built.skill_name.clone())
            .map_err(|_| anyhow!("trace capture identity is unavailable before agent launch"))?;
    crate::trace::CaptureHandle::begin(
        context,
        built.context.clone(),
        crate::trace::CaptureStart {
            provider: built.harness.clone(),
            model: built.model.clone(),
            surface: surface.to_string(),
            input_op: "initial".to_string(),
            gather_ms: built.context_gather_ms,
            render_ms: built.context_render_ms,
            raw_provider: surface == "headless",
            basis: control.as_ref().map(|control| control.basis.clone()),
            supervision: control.map(|control| control.supervision),
        },
    )
    .map_err(|error| anyhow!("failed to establish trace capture before agent launch: {error}"))
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
        attributed_context, is_interactive_run, is_interactive_run_with_tty,
        prepare_wave_harness_turn, should_launch_via_skill, skill_launch_seed, split_skill_args,
        BoundEnvironment,
    };
    use crate::durable::{BoundaryState, InvocationRoute, WorkRef};
    use crate::engine::prompt::{Document, DocumentSource, PromptComponents};
    use crate::engine::Surface;
    use crate::id::WaveId;
    use crate::lf::Cli;
    use crate::ops::{DirectRun, WorkBinding};
    use crate::store::{open_store, StorageConfig};
    use crate::trace::{ContextAssetKind, ContextScope};
    use crate::wave::Wave;
    use clap::Parser;
    use std::sync::Arc;

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

    #[tokio::test]
    async fn bound_environment_exports_exact_run_identity_and_restores_ambient_values() {
        let directory = tempfile::tempdir().unwrap();
        let store = Arc::new(
            open_store(&StorageConfig::sqlite(directory.path().join("registry.db")))
                .await
                .unwrap(),
        );
        let wave = Wave::new(
            WaveId::new(),
            "runtime".to_string(),
            directory.path().display().to_string(),
        );
        store.create_wave(&wave).await.unwrap();
        let binding = WorkBinding {
            work: WorkRef::Wave(wave.id().clone()),
            wave_id: wave.id().clone(),
            wave_name: wave.name().to_string(),
            cwd: directory.path().to_path_buf(),
            context: "Wave runtime".to_string(),
        };
        let mut direct = DirectRun::start(
            store,
            &binding,
            InvocationRoute {
                provider: "codex".to_string(),
                model: None,
                account_id: None,
            },
            "headless",
        )
        .await
        .unwrap();
        let lock = crate::journal::test_env_lock();
        let keys = [
            crate::durable::RUN_CONTEXT_ENV,
            crate::durable::RUN_ID_ENV,
            crate::durable::AGENT_INVOCATION_ENV,
            crate::engine::wave_context::WAVE_ID_ENV,
        ];
        let previous = keys
            .iter()
            .map(|key| (*key, std::env::var_os(key)))
            .collect::<Vec<_>>();
        for key in keys {
            std::env::set_var(key, "ambient");
        }

        {
            let _environment = BoundEnvironment::enter(&direct, &binding);
            assert_eq!(
                std::env::var(crate::durable::RUN_CONTEXT_ENV).unwrap(),
                "agent"
            );
            assert_eq!(
                std::env::var(crate::durable::RUN_ID_ENV).unwrap(),
                direct.context().run_id.as_str()
            );
            assert_eq!(
                std::env::var(crate::durable::AGENT_INVOCATION_ENV).unwrap(),
                direct.invocation().id.as_str()
            );
            assert_eq!(
                std::env::var(crate::engine::wave_context::WAVE_ID_ENV).unwrap(),
                binding.wave_id.as_str()
            );
        }
        for key in keys {
            assert_eq!(std::env::var(key).unwrap(), "ambient");
        }
        for (key, value) in previous {
            match value {
                Some(value) => std::env::set_var(key, value),
                None => std::env::remove_var(key),
            }
        }
        drop(lock);
        direct.finish(BoundaryState::Succeeded).await.unwrap();
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
