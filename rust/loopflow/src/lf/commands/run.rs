use crate::lf::commands::context::preview::PromptPreview;
use crate::lf::Cli;
use anyhow::{anyhow, Result};
use loopflow_execution::agent::run_agent;
use loopflow_execution::agent::AgentConfig;
use loopflow_execution::agent::ProcessConfig;
use loopflow_execution::graph::Skill;
use loopflow_execution::harness::native_session::{launch_session, NativeOutcome};
use loopflow_execution::harness::{LaunchKind, Mode};
use loopflow_execution::output::{format_context_header, format_reproducible_command, Colors};
use loopflow_execution::process::LfProcessRuntime;
use loopflow_execution::prompt::attribution::attributed_context;
use loopflow_execution::prompt::process::prepare_process_prompt;
use loopflow_execution::prompt::process::ContextSourceOverrides;
use loopflow_execution::prompt::process::ProcessPromptInput;
use loopflow_execution::prompt::PromptComponents;
use loopflow_execution::prompt::Surface;
use loopflow_execution::session_record::{
    AgentProcessRequest, SessionCaptureSpec, SubjectAttribution,
};
use std::path::{Path, PathBuf};
use std::time::Instant;
use tracing::{debug, info, instrument, trace};

/// Unified entry point for running skills, inline prompts, or interactive chat.
///
/// | skill    | message | behavior                              |
/// |---------|---------|---------------------------------------|
/// | Some    | None    | Run named skill                        |
/// | None    | Some    | Run inline prompt                     |
/// | Some    | Some    | Run skill with message as extra context |
/// | None    | None    | Interactive chat                      |
#[instrument(skip(cli, message), fields(skill = ?skill, has_message = message.is_some()))]
pub fn run(
    kind: LaunchKind,
    repo_root: &Path,
    skill: Option<&str>,
    message: Option<&str>,
    cli: &Cli,
) -> Result<()> {
    if let Some(binding) = implicit_binding(cli)? {
        return run_bound_prompt(kind, skill, message, cli, &binding).map(|_| ());
    }
    let mut built = build_prompt(kind.mode(), repo_root, skill, message, cli)?;
    built.subjects = cli.work_subject_selector().into_iter().collect();
    // `--wave` resolved its Wave once for this process.
    built.work = cli.wave.as_ref().and_then(|_| {
        let id = std::env::var(loopflow_execution::work::wave::context::WAVE_ID_ENV).ok()?;
        Some(loopflow_execution::session::SessionWork {
            task_id: None,
            wave_id: Some(loopflow_model::id::WaveId::parse(&id).ok()?),
            source: loopflow_execution::session::WorkSource::Declared,
        })
    });

    print_context_header(&built, cli);
    run_prompt(built, cli, kind).map(|_| ())
}

pub(crate) fn preview_prompt(
    cwd: &Path,
    skill: Option<&str>,
    message: Option<&str>,
    cli: &Cli,
    binding: Option<&loopflow_execution::ops::WorkBinding>,
    captured: bool,
) -> Result<PromptPreview> {
    anyhow::ensure!(
        operator_scope(skill, cli, cwd, binding)?.is_none(),
        "context unavailable before the operator's scope checkout is prepared; preview from its existing checkout"
    );
    let mode = loopflow_execution::transport::LaunchInput {
        batch: cli.batch,
        interactive: cli.interactive,
        has_prompt: skill.is_some() || message.is_some(),
    }
    .mode();
    let built = match binding {
        Some(binding) => build_bound_prompt(mode, skill, message, cli, binding)?,
        None => build_prompt_at(mode, skill, message, cli, cwd.to_path_buf(), None, captured)?,
    };
    Ok(PromptPreview {
        checkout: built.repo_root,
        system_prompt: loopflow_execution::agent::system_prompt_with_structured_replies(
            &built.agent_config,
        ),
        task_prompt: built.agent_config.task_prompt,
        skill_invocation: built.agent_config.skill_invocation,
        references: built
            .components
            .docs
            .iter()
            .chain(&built.components.summaries)
            .filter(|doc| doc.source != loopflow_execution::prompt::DocumentSource::Docs)
            .map(
                |doc| crate::lf::commands::context::preview::ContextReference {
                    path: doc.path.clone(),
                    content: doc.content.clone(),
                },
            )
            .collect(),
    })
}

/// Whether this process is the Graph that ran `session` as its step: a Graph
/// corrects its own step's answer in that step's worker, and nobody else
/// continues a worker.
pub(crate) fn continues_own_step(session: &loopflow_execution::session::LfSession) -> bool {
    loopflow_execution::journal::current_parent_lf_process_id()
        .is_some_and(|graph| session.graph_lf_process_id.as_deref() == Some(graph.as_str()))
}

/// `lf -b steer SESSION TEXT`: one more headless turn of a conversation,
/// with `message` as its prompt and the provider's own history, without
/// re-executing its skill.
pub fn resume(
    store: &loopflow_execution::store::sqlite::SqliteStore,
    session: &loopflow_execution::session::LfSession,
    message: &str,
    cli: &Cli,
    kind: LaunchKind,
) -> Result<()> {
    let mut resolved = loopflow_execution::agent::resolve_session_config(
        session,
        cli.setting_values()?,
        cli.output_schema.as_ref(),
    )?;
    let mut turn = cli.process_options();
    turn.agent = Some(resolved.agent.to_string());
    let message = if session.captured.is_some()
        && session.provider.as_deref() != Some(resolved.agent.harness.as_str())
    {
        let request =
            loopflow_execution::ops::human_session::conversation_log::continuation(store, session)?;
        format!("{request}\n\n{message}")
    } else {
        message.to_owned()
    };
    turn.resume = Some(session.id.to_string());
    turn.skill_input = None;
    turn.resolved_invocation = None;
    turn.task = session.task_id.as_ref().map(ToString::to_string);
    turn.wave = session
        .wave_id
        .as_ref()
        .map(|id| store.get_wave(id))
        .transpose()?
        .flatten()
        .map(|wave| wave.slug().to_string());
    // Continuation belongs to the saved conversation, even when invoked from
    // another Task's checkout. Do not rediscover Work from the caller's cwd.
    let mut built = build_prompt_at(
        Mode::Headless,
        None,
        Some(&message),
        &turn,
        session.cwd.clone(),
        None,
        false,
    )?;
    let prepared = built.agent_config;
    let conversation_context = match (
        resolved.conversation_context.take(),
        prepared.conversation_context,
    ) {
        (Some(mut saved), Some(current)) => {
            let submitted = saved.submitted_docs.get_or_insert_with(Vec::new);
            for path in current.submitted_docs.into_iter().flatten() {
                if !submitted.contains(&path) {
                    submitted.push(path);
                }
            }
            Some(saved)
        }
        (saved, current) => saved.or(current),
    };
    built.agent_config = AgentConfig {
        system_prompt: prepared.system_prompt,
        conversation_context,
        task_prompt: prepared.task_prompt,
        structured_replies: prepared.structured_replies,
        env: prepared.env,
        ..resolved
    };
    print_context_header(&built, &turn);
    run_prompt(built, &turn, kind).map(|_| ())
}

#[doc(hidden)]
pub fn run_bound(
    kind: LaunchKind,
    skill: Option<&str>,
    message: Option<&str>,
    cli: &Cli,
    binding: &loopflow_execution::ops::WorkBinding,
) -> Result<()> {
    run_bound_prompt(kind, skill, message, cli, binding).map(|_| ())
}

/// Run a channel request through the ordinary attributed launch and settlement
/// path, and read its answer from the provider's transcript.
pub(crate) fn answer_bound(
    message: &str,
    cli: &Cli,
    binding: &loopflow_execution::ops::WorkBinding,
) -> Result<Option<String>> {
    match run_bound_prompt(LaunchKind::Worker, None, Some(message), cli, binding)? {
        Some(dir) => loopflow_execution::session_record::final_answer(&dir),
        None => Ok(None),
    }
}

fn run_bound_prompt(
    kind: LaunchKind,
    skill: Option<&str>,
    message: Option<&str>,
    cli: &Cli,
    binding: &loopflow_execution::ops::WorkBinding,
) -> Result<Option<std::path::PathBuf>> {
    let mut scoped;
    let binding = if let Some(scope) = operator_scope(skill, cli, &binding.cwd, Some(binding))? {
        scoped = binding.clone();
        scoped.cwd =
            loopflow_execution::ops::human_session::ensure_scope_worktree(&binding.cwd, &scope)?
                .path;
        &scoped
    } else {
        binding
    };
    let built = build_bound_prompt(kind.mode(), skill, message, cli, binding)?;
    print_context_header(&built, cli);
    run_prompt(built, cli, kind)
}

/// Preview and launch enrich the same selected Work; only launch prepares a checkout.
fn build_bound_prompt(
    mode: Mode,
    skill: Option<&str>,
    message: Option<&str>,
    cli: &Cli,
    binding: &loopflow_execution::ops::WorkBinding,
) -> Result<PromptBuild> {
    let mut launch = cli.process_options();
    launch.wave = binding.wave_name.clone();
    let arguments = message.unwrap_or_default();
    let message = if let loopflow_model::durable::WorkRef::Task(id) = &binding.work {
        launch.task = Some(id.to_string());
        message.unwrap_or_default().to_owned()
    } else {
        bound_message(binding, message)
    };
    let cli = &launch;
    let mut built = build_bound_prompt_at(mode, skill, &message, arguments, cli, &binding.cwd)?;
    built.agent_config.cwd = Some(binding.cwd.clone());
    if let Some(wave) = &binding.wave_id {
        built.agent_config.env.insert(
            loopflow_execution::work::wave::context::WAVE_ID_ENV.to_string(),
            wave.to_string(),
        );
    } else {
        built
            .agent_config
            .env
            .remove(loopflow_execution::work::wave::context::WAVE_ID_ENV);
    }
    built.subjects = binding.subjects.clone();
    built.work = Some(loopflow_execution::session::SessionWork {
        task_id: match &binding.work {
            loopflow_model::durable::WorkRef::Task(id) => Some(id.clone()),
            loopflow_model::durable::WorkRef::Wave(_)
            | loopflow_model::durable::WorkRef::Project(_) => None,
        },
        wave_id: binding.wave_id.clone(),
        source: binding.source,
    });

    Ok(built)
}

/// An `lf` launch inside a registered Task's checkout binds to that Task unless
/// the caller selected Work explicitly. Otherwise the Task whose checkout is
/// the directory's Git worktree wins over an ancestor's explicit declaration. An unavailable
/// registry leaves context unresolved; it never reconstructs Task identity
/// from process ancestry.
pub fn implicit_binding(cli: &Cli) -> Result<Option<loopflow_execution::ops::WorkBinding>> {
    if cli.work_subject_selector().is_some() {
        return Ok(None);
    }
    let repo = std::env::current_dir()?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let Some(store) = loopflow_execution::store::open_existing_store().await? else {
            return Ok(None);
        };
        loopflow_execution::ops::resolve_execution_binding(&std::sync::Arc::new(store), &repo)
            .await
            .map_err(anyhow::Error::from)
    })
}

pub(crate) fn bound_message(
    binding: &loopflow_execution::ops::WorkBinding,
    message: Option<&str>,
) -> String {
    let mut context = format!(
        "<lf:work kind=\"{}\" id=\"{}\">\n{}\n</lf:work>",
        binding.work.kind(),
        binding.work.id(),
        binding.context,
    );
    if let Some(message) = message.filter(|message| !message.trim().is_empty()) {
        context.push_str("\n\n");
        context.push_str(message);
    }
    context
}

struct PromptBuild {
    repo_root: PathBuf,
    agent_config: AgentConfig,
    process: ProcessConfig,
    components: PromptComponents,
    context: loopflow_execution::trace::PreparedTurnContext,
    skill_name: Option<String>,
    /// SHA-256 of the resolved skill text, recorded on the run.
    skill_version: Option<String>,
    log_name: String,
    subjects: Vec<String>,
    work: Option<loopflow_execution::session::SessionWork>,
}

fn build_prompt(
    mode: Mode,
    repo_root: &Path,
    skill: Option<&str>,
    message: Option<&str>,
    cli: &Cli,
) -> Result<PromptBuild> {
    let start = Instant::now();
    let repo_root = match operator_scope(skill, cli, repo_root, None)? {
        Some(scope) => {
            loopflow_execution::ops::human_session::ensure_scope_worktree(repo_root, &scope)?.path
        }
        None => repo_root.to_path_buf(),
    };
    debug!(elapsed_ms = start.elapsed().as_millis(), "found repo root");
    build_prompt_at(
        mode,
        skill,
        message,
        cli,
        repo_root,
        None,
        cli.skill_input.is_some(),
    )
}

/// Decide checkout preparation from the resolved skill, not its CLI alias.
/// Preview observes this same decision but never creates the scope checkout.
fn operator_scope(
    skill: Option<&str>,
    cli: &Cli,
    cwd: &Path,
    binding: Option<&loopflow_execution::ops::WorkBinding>,
) -> Result<Option<loopflow_execution::session::PrimaryScope>> {
    use loopflow_execution::session::PrimaryScope;
    use loopflow_model::durable::WorkRef;

    if cli.cwd.is_some() || cli.task.is_some() || cli.wt.is_some() {
        return Ok(None);
    }
    let skill = resolve_skill(skill, cli, cwd)?;
    let scope = match (skill.as_ref().map(|skill| skill.name.as_str()), binding) {
        (Some("repo-operate"), None) => {
            PrimaryScope::Repository(loopflow_execution::repository::discover(cwd)?)
        }
        (Some("wave-operate"), Some(binding)) => match &binding.work {
            WorkRef::Wave(id) => PrimaryScope::Wave(id.clone()),
            _ => return Ok(None),
        },
        _ => return Ok(None),
    };
    Ok(
        (loopflow_execution::repository::discover(cwd)?.as_path() == cwd.canonicalize()?)
            .then_some(scope),
    )
}

/// Captured identity and body take precedence, including during scope selection.
fn resolve_skill(name: Option<&str>, cli: &Cli, repo: &Path) -> Result<Option<Skill>> {
    match &cli.resolved_invocation {
        Some(invocation) => Ok(Some(invocation.skill.clone())),
        None => Ok(name
            .map(|name| loopflow_execution::graph::load_skill(name, repo))
            .transpose()?),
    }
}

fn build_bound_prompt_at(
    mode: Mode,
    skill: Option<&str>,
    message: &str,
    arguments: &str,
    cli: &Cli,
    repo_root: &Path,
) -> Result<PromptBuild> {
    let mut built = build_prompt_at(
        mode,
        skill,
        Some(arguments),
        cli,
        repo_root.to_path_buf(),
        Some((
            loopflow_execution::trace::ContextAssetKind::Goal,
            loopflow_execution::trace::ContextScope::Task,
        )),
        cli.skill_input.is_some(),
    )?;
    if message != arguments && !cli.context {
        let path = loopflow_execution::prompt::write_prompt_log(
            repo_root,
            message,
            "work-direction",
            None,
        )?;
        if let Some(context) = &mut built.agent_config.conversation_context {
            context.references.push(path);
        }
    }
    if message != arguments && cli.context {
        built
            .components
            .summaries
            .push(loopflow_execution::prompt::Document {
                path: "work-direction (saved at launch)".into(),
                content: message.into(),
                source: loopflow_execution::prompt::DocumentSource::Summary,
            });
    }
    Ok(built)
}

fn prepare_task_input(
    cli: &Cli,
) -> Result<
    Option<(
        loopflow_execution::store::SharedStore,
        loopflow_execution::ops::task_input::TaskSeed,
    )>,
> {
    let Some(id) = &cli.task else { return Ok(None) };
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = std::sync::Arc::new(if cli.context {
            loopflow_execution::store::read_existing_registry()?
                .ok_or_else(|| anyhow!("Task input unavailable: no local registry"))?
        } else {
            loopflow_execution::store::open_store(
                &loopflow_execution::store::storage_config_from_env()?,
            )
            .await?
        });
        // The launch reads local state; each turn end's planning refresh
        // syncs this Task with its tracker.
        let task = store
            .get_task_by_issue(id)
            .await?
            .ok_or_else(|| anyhow!("Task {id} is missing"))?;
        let wave = match &task.wave_id {
            Some(id) => store.get_wave(id).await?,
            None => None,
        };
        let seed = loopflow_execution::ops::task_input::read_seed(
            &store,
            &task,
            wave.as_ref().map(|w| w.slug()),
        )
        .await?;
        Ok(Some((store, seed)))
    })
}

fn build_prompt_at(
    mode: Mode,
    skill: Option<&str>,
    message: Option<&str>,
    cli: &Cli,
    repo_root: PathBuf,
    message_context: Option<(
        loopflow_execution::trace::ContextAssetKind,
        loopflow_execution::trace::ContextScope,
    )>,
    captured: bool,
) -> Result<PromptBuild> {
    let _settings = loopflow_execution::config::settings::select_invocation(cli.setting_values()?);
    let is_interactive = mode == Mode::Interactive;
    let subject = cli.task.as_deref().unwrap_or_default();
    loopflow_execution::agent_input_timing::record("prompt_begin", subject);
    let task_input = prepare_task_input(cli)?;
    let task_message = task_input.as_ref().map(|(_, seed)| seed.message.clone());
    let wave = cli.wave.clone().or_else(|| {
        cli.resume
            .is_none()
            .then(loopflow_execution::work::wave::context::resolve_ambient_wave_name)
            .flatten()
    });
    let config_start = Instant::now();
    let config = loopflow_execution::config::load_wave_config(Some(&repo_root), wave.as_deref())?
        .unwrap_or_default();
    debug!(
        elapsed_ms = config_start.elapsed().as_millis(),
        "loaded config"
    );
    trace!(
        agent = ?config.agent(),
        ?config.yolo,
        "loaded config"
    );

    let discover_start = Instant::now();
    let invocation = cli.resolved_invocation.as_ref();
    let discovered_skill = resolve_skill(skill, cli, &repo_root)?;
    let arguments = invocation.map_or(message.unwrap_or_default(), |invocation| {
        invocation.arguments.as_str()
    });
    debug!(
        elapsed_ms = discover_start.elapsed().as_millis(),
        "discovered skill"
    );

    info!("preparing launch prompt");
    let prepare_start = Instant::now();
    let surface = if is_interactive {
        Surface::Cli
    } else {
        Surface::Headless
    };

    if let Some(skill) = &discovered_skill {
        match loopflow_definitions::name::portable_name(&skill.name).as_str() {
            "task-session" => anyhow::ensure!(
                task_input.is_some(),
                "Select a Task with `lf resume --task ISSUE`"
            ),
            "wave-session" => {
                anyhow::ensure!(wave.is_some(), "Select a Wave with `lf resume --wave NAME`")
            }
            _ => {}
        }
    }
    // Ordinary third-party skills need their own instructions, not the Work
    // operating manual. Captured Graphs and attributed Work retain that guidance.
    let standalone_native = task_input.is_none()
        && wave.is_none()
        && !captured
        && message_context.is_none()
        && discovered_skill.as_ref().is_some_and(|skill| {
            skill.source.as_ref().is_some_and(|source| {
                source.dialect != loopflow_execution::skills::catalog::SkillDialect::Loopflow
            })
        });
    let prepare = if cli.context {
        loopflow_execution::prompt::process::preview_process_prompt
    } else {
        prepare_process_prompt
    };
    let prepared = prepare(
        &config,
        ProcessPromptInput {
            repo_root: repo_root.clone(),
            skill: skill.map(|value| value.to_string()),
            resolved_skill: discovered_skill.clone(),
            surface,
            docs: Vec::new(),
            wave,
            message: message.map(|value| value.to_string()),
            skill_arguments: arguments.to_string(),
            no_loopflow: standalone_native,
            source_overrides: ContextSourceOverrides::default(),
            summary: None,
            references: task_message
                .as_ref()
                .map(|text| vec![("Task brief".into(), text.clone())])
                .unwrap_or_default(),
            client_context: Default::default(),
            related_repos: Vec::new(),
        },
    )?;
    debug!(
        elapsed_ms = prepare_start.elapsed().as_millis(),
        "prepared launch prompt"
    );
    loopflow_execution::agent_input_timing::record("prompt_end", subject);
    let skill_name = discovered_skill
        .as_ref()
        .map(|skill| skill.name.clone())
        .or_else(|| skill.map(|value| value.to_string()));
    let log_name = skill_name
        .as_deref()
        .unwrap_or(if message.is_some() { "inline" } else { "chat" })
        .to_string();
    let process = ProcessConfig {
        task_input: task_input
            .map(|(store, seed)| loopflow_execution::ops::task_input::TaskInput::new(store, seed)),
        stream: !is_interactive,
        wait_for_capacity: !is_interactive && loopflow_execution::ops::graph_process::in_graph(),
        ..Default::default()
    };
    let mut agent_config = prepared.config;
    agent_config.output_schema = cli.output_schema.clone();

    let mut components = prepared.components;
    components.message_context = message_context.or_else(|| {
        task_message.as_ref().map(|_| {
            (
                loopflow_execution::trace::ContextAssetKind::Goal,
                loopflow_execution::trace::ContextScope::Task,
            )
        })
    });
    let deduplication_decisions = prepared.deduplication_decisions;
    let effective_system =
        loopflow_execution::agent::system_prompt_with_structured_replies(&agent_config);
    let context = attributed_context(
        &components,
        &effective_system,
        &agent_config.task_prompt,
        &deduplication_decisions,
        Some(arguments),
    );
    Ok(PromptBuild {
        repo_root,
        agent_config,
        process,
        components,
        context,
        skill_name,
        skill_version: discovered_skill
            .as_ref()
            .map(loopflow_execution::graph::Skill::version),
        log_name,
        subjects: Vec::new(),
        work: None,
    })
}

fn print_context_header(built: &PromptBuild, cli: &Cli) {
    if !cli.verbose {
        return;
    }
    let colors = Colors::new();
    let header = format_context_header(&built.context, &built.components);
    let command = format_reproducible_command(
        built.skill_name.as_deref(),
        built.components.wave.as_deref(),
        &cli.docs,
        cli.clipboard,
        Some(&built.agent_config.agent.to_string()),
    );
    eprintln!(
        "{dim}{header}\n\n  {command}{reset}",
        dim = colors.dim,
        header = header,
        command = command,
        reset = colors.reset,
    );
}

/// Run the prompt; a headless run returns where its input was recorded.
fn run_prompt(
    mut built: PromptBuild,
    cli: &Cli,
    kind: LaunchKind,
) -> Result<Option<std::path::PathBuf>> {
    if kind.mode() == Mode::Interactive {
        info!("launching interactive vendor session");
        let runtime = begin_process_runtime(&built, "tui", None, cli.prepared_capture.as_deref())?;
        let mut environment = built.agent_config.env.clone();
        environment.extend(runtime.environment());
        let result = (|| {
            let mut config = built.agent_config.clone();
            config.env = environment.clone();
            config.launch_lock = cli.launch_lock.clone();
            let context_file =
                loopflow_execution::agent::write_system_prompt_file(&config, &built.log_name)?;
            launch_session(&config, kind, context_file.as_deref(), Some(&runtime))
        })();
        if let Some(provider_session) =
            loopflow_execution::session_record::read_provider_session(&runtime.artifact_dir())
                .map_err(|error| anyhow!("failed to read provider session: {error}"))?
        {
            runtime.observe_provider(Some(provider_session.agent_session));
        }
        runtime.finish(
            result
                .as_ref()
                .map_or("failed", NativeOutcome::capture_outcome),
        )?;
        return result
            .and_then(|outcome| {
                let harness = built.agent_config.agent.harness;
                let assigned = loopflow_execution::harness::launcher(harness)
                    .capabilities()
                    .thread
                    .for_kind(kind)
                    == loopflow_execution::harness::ThreadName::Assigned;
                outcome.result(harness, assigned, kind)
            })
            .map(|()| None);
    }

    // Native terminals own their turn status. A separate lf record would remain
    // Working at their idle prompt until the provider process exits.
    loopflow_execution::transport::report(
        loopflow_model::program_status::State::Working,
        false,
        None,
    );
    let cli_check_start = Instant::now();
    loopflow_execution::agent::start_on_installed_agent(&mut built.agent_config, &built.process)
        .map_err(|missing| anyhow!(missing))?;
    debug!(
        elapsed_ms = cli_check_start.elapsed().as_millis(),
        "checked cli availability"
    );

    let runtime = begin_process_runtime(&built, "headless", cli.resume.as_deref(), None)?;

    let result = run_headless_prompt(&built, &runtime, kind);
    // Explicit native resume can retire this provider while its executor waits.
    // Settle our capture as interrupted; a failed Graph step would replay it.
    if runtime.attachment_replaced()? {
        runtime.finish("interrupted")?;
        return Err(loopflow_execution::process::CommandExit(130).into());
    }
    let outcome = if result.is_ok() {
        "completed"
    } else {
        "failed"
    };
    let settlement = runtime.finish(outcome);
    match (result, settlement) {
        (Err(error), Err(settlement)) => Err(error.context(format!(
            "Session execution also failed to settle: {settlement}"
        ))),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(anyhow!("Session completed but did not settle: {error}")),
        (Ok(()), Ok(())) => Ok(Some(runtime.artifact_dir())),
    }
}

fn run_headless_prompt(
    built: &PromptBuild,
    runtime: &LfProcessRuntime,
    kind: LaunchKind,
) -> Result<()> {
    let context_file_start = Instant::now();
    let context_file =
        loopflow_execution::agent::write_system_prompt_file(&built.agent_config, &built.log_name)?;
    debug!(
        elapsed_ms = context_file_start.elapsed().as_millis(),
        "wrote context log"
    );

    let mut process = built.process.clone();
    process.context_file = context_file;
    process.runtime = Some(runtime.clone());

    // Set up directive relay so agent skills can issue shell directives
    // (e.g. `cd` after `lf wt switch`).
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

    info!(
        harness = built.agent_config.agent.harness.as_str(),
        "launching agent"
    );
    let process_start = Instant::now();
    let result = run_agent(&agent_config, &process, kind);

    // Relay safe directives from the agent back to the invoking shell.
    if let (Some(relay), Some(ref target)) = (relay_path, directive_file) {
        relay_directives(&relay, target);
    }

    let result = result?;
    debug!(
        elapsed_ms = process_start.elapsed().as_millis(),
        "agent finished"
    );
    debug!(exit_code = result.exit_code, "agent completed");
    if result.exit_code == 0 {
        Ok(())
    } else if result.exit_code == 130 {
        eprintln!("The agent's turn was interrupted.");
        Err(loopflow_execution::process::CommandExit(130).into())
    } else if let Some(failure) = &result.failure {
        let reason = result.stderr.trim();
        let reason = if reason.is_empty() {
            String::new()
        } else {
            format!("\n{reason}")
        };
        Err(anyhow!(
            "agent stopped after {failure}.{reason}\nCheck {} for details.",
            runtime.artifact_dir().display()
        ))
    } else {
        let reason = loopflow_execution::os_process::exit_reason(&result.stderr);
        Err(anyhow!(
            "agent exited with code {}{reason}\nCheck {} for details.",
            result.exit_code,
            runtime.artifact_dir().display()
        ))
    }
}

fn begin_process_runtime(
    built: &PromptBuild,
    surface: &str,
    resume: Option<&str>,
    prepared_capture: Option<&str>,
) -> Result<LfProcessRuntime> {
    let started = Instant::now();
    let cwd = built
        .agent_config
        .cwd
        .clone()
        .unwrap_or_else(|| built.repo_root.clone());
    let subjects = built
        .subjects
        .iter()
        .cloned()
        .map(SubjectAttribution::declared)
        .collect::<Vec<_>>();
    let spec = SessionCaptureSpec {
        harness: built.agent_config.agent.harness.as_str().to_owned(),
        model: built.agent_config.agent.model.clone(),
        surface: surface.to_string(),
        cwd,
        repo: Some(built.repo_root.clone()),
        worktree: Some(built.repo_root.clone()),
        skill: built.skill_name.clone(),
        subjects,
        graph: loopflow_execution::session_record::SessionGraphMembership::Independent,
        work: built.work.clone(),
    };
    let runtime = if let Some(session) = resume {
        LfProcessRuntime::continue_with_context(
            session,
            spec,
            &built.context,
            AgentProcessRequest::from_prepared(&built.agent_config)?,
        )
    } else if let Some(id) = prepared_capture {
        loopflow_execution::session_record::SessionRecorder::start_prepared(
            &loopflow_execution::store::lf_home_dir(),
            id,
            spec,
            &built.context,
            Some(AgentProcessRequest::from_prepared(&built.agent_config)?),
        )
        .map(loopflow_execution::process::LfProcessRuntime::new)
    } else {
        let launch = Some(AgentProcessRequest::from_prepared(&built.agent_config)?);
        loopflow_execution::session_record::SessionRecorder::begin_with_context(
            spec,
            &built.context,
            launch,
        )
        .map(loopflow_execution::process::LfProcessRuntime::new)
    }
    .map_err(|error| {
        anyhow!("failed to publish Session capture manifest before agent launch: {error}")
    })?;
    runtime.claim_session_attachment()?;
    if let Some(version) = &built.skill_version {
        runtime.record_skill_version(version);
    }
    debug!(
        elapsed_ms = started.elapsed().as_millis(),
        "published Session capture"
    );
    Ok(runtime)
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

#[cfg(test)]
mod tests {
    use super::{
        attributed_context, begin_process_runtime, build_bound_prompt_at, build_prompt,
        build_prompt_at, run_headless_prompt, run_prompt, PromptBuild,
    };

    use crate::lf::Cli;
    use crate::test_ambient::EnvGuard;
    use clap::Parser;
    use loopflow_execution::agent::{run_agent, AgentConfig, ProcessConfig};
    use loopflow_execution::harness::{LaunchKind, Mode};
    use loopflow_execution::prompt::PromptComponents;
    use loopflow_execution::skills::catalog::SkillCatalog;
    use loopflow_execution::trace::ContextAssetKind;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn context_choices_override_config_and_omission_inherits() {
        let _lock = loopflow_execution::journal::test_env_lock();
        let _restore = EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_branch("context-choice");
        repo.create_file("changed.txt", "changed file body\n");
        repo.stage_all();
        repo.commit("Add changed content");
        for configured in [false, true] {
            repo.create_file(
                ".lf/config.yaml",
                &format!(
                "diff: {configured}\ndiff_files: {configured}\nchrome: {configured}\npaste: false\n"
            ),
            );
            for (choice, files, patch) in [
                (None, configured, configured),
                (Some("files"), true, false),
                (Some("patch"), false, true),
                (Some("both"), true, true),
                (Some("none"), false, false),
            ] {
                let mut args = vec!["lf", "--batch", "-a", "claude"];
                if let Some(choice) = choice {
                    args.extend(["--diff", choice]);
                }
                for browser in [None, Some("on"), Some("off")] {
                    let mut args = args.clone();
                    if let Some(browser) = browser {
                        args.extend(["--chrome", browser]);
                    }
                    let cli = Cli::parse_from(args);
                    let built = build_bound_prompt_at(
                        Mode::Headless,
                        None,
                        "inspect changes",
                        "",
                        &cli,
                        repo.path(),
                    )
                    .unwrap();
                    assert_eq!(
                        built
                            .components
                            .diff_files
                            .iter()
                            .any(|file| file.content.contains("changed file body")),
                        files
                    );
                    assert_eq!(
                        built
                            .components
                            .change
                            .as_ref()
                            .and_then(|change| change.patch.as_deref())
                            .is_some_and(|patch| patch.contains("+changed file body")),
                        patch
                    );
                    assert_eq!(
                        built.agent_config.chrome,
                        Some(browser.map_or(configured, |value| value == "on"))
                    );
                }
            }
        }
    }

    #[test]
    fn preferred_name_survives_fresh_launches_and_corrections() {
        let _lock = loopflow_execution::journal::test_env_lock();
        let _restore = EnvGuard::clear(&[
            "LF_HOME",
            "LF_USER_NAME",
            "GIT_CONFIG_COUNT",
            "GIT_CONFIG_KEY_0",
            "GIT_CONFIG_VALUE_0",
        ]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        std::env::set_var("GIT_CONFIG_COUNT", "1");
        std::env::set_var("GIT_CONFIG_KEY_0", "user.name");
        std::env::set_var("GIT_CONFIG_VALUE_0", "Git User");
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(
            ".lf/config.yaml",
            "diff: false\ndiff_files: false\npaste: false\n",
        );
        let cli = Cli::parse_from(["lf", "--batch", "-a", "claude"]);

        for name in ["Jack", "Jacqueline", "  "] {
            std::fs::write(
                home.path().join("config.yaml"),
                format!("user:\n  name: '{name}'\n"),
            )
            .unwrap();
            // Each assembly reloads personal config, including the correction.
            for _ in 0..2 {
                let built = build_bound_prompt_at(
                    Mode::Headless,
                    None,
                    "choose the prototype path",
                    "",
                    &cli,
                    repo.path(),
                )
                .unwrap();
                assert_eq!(
                    built.components.user_name.as_deref(),
                    if name.trim().is_empty() {
                        Some("Git User")
                    } else {
                        Some(name)
                    }
                );
                let context = loopflow_execution::prompt::render_user_context(
                    built.components.user_name.as_deref(),
                );
                assert_eq!(
                    loopflow_execution::prompt::format_prompt(&built.components)
                        .matches("<lf:user>")
                        .count(),
                    usize::from(!context.is_empty())
                );
                assert!(built.agent_config.system_prompt.contains(&context));
                assert!(!built
                    .agent_config
                    .system_prompt
                    .contains("display name is \"Repository Owner\""));
            }
        }
        std::fs::remove_file(home.path().join("config.yaml")).unwrap();
        let built =
            build_bound_prompt_at(Mode::Headless, None, "continue", "", &cli, repo.path()).unwrap();
        assert_eq!(built.components.user_name.as_deref(), Some("Git User"));
    }

    #[test]
    fn preferred_name_uses_remote_caller_and_leaves_background_work_unattributed() {
        let _lock = loopflow_execution::journal::test_env_lock();
        let _restore = EnvGuard::clear(&["LF_HOME", "LF_USER_NAME"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        std::fs::write(
            home.path().join("config.yaml"),
            "user:\n  name: Host Owner\n",
        )
        .unwrap();
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(
            ".lf/config.yaml",
            "diff: false\ndiff_files: false\npaste: false\n",
        );
        let cli = Cli::parse_from(["lf", "--batch", "-a", "claude"]);
        for (caller, expected) in [("Jack", "Jack"), ("", "Host Owner")] {
            std::env::set_var("LF_USER_NAME", caller);
            let built =
                build_bound_prompt_at(Mode::Headless, None, "continue", "", &cli, repo.path())
                    .unwrap();
            assert_eq!(built.components.user_name.as_deref(), Some(expected));
            assert!(built.agent_config.system_prompt.contains(expected));
            assert_eq!(built.agent_config.env["LF_USER_NAME"], expected);
        }
        std::env::set_var("LF_USER_NAME", "Jack");
    }

    #[cfg(unix)]
    #[test]
    fn ad_hoc_batch_launch_captures_session_without_planning_registry() {
        let _lock = loopflow_execution::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let evidence = home.path().join("provider-run");
        let implicit_evidence = home.path().join("implicit-provider-run");
        let provider = bin.join("claude");
        std::fs::write(
            &provider,
            r#"#!/usr/bin/python3
import json, os, sys
from pathlib import Path
if '--version' in sys.argv:
    print('fixture')
    sys.exit(0)
for line in sys.stdin:
    message = json.loads(line)
    message['session_id'] = 'fixture-thread'
    print(json.dumps(message), flush=True)
    print(json.dumps({'type': 'system', 'subtype': 'init', 'session_id': 'fixture-thread'}), flush=True)
    with open(os.environ['LF_TEST_RUN_EVIDENCE'], 'a') as evidence:
        evidence.write('|'.join(os.environ.get(key, 'unset') for key in ['LF_CAPTURE_KEY', 'LF_RUN_DIR', 'LF_TRACE_ID', 'LF_PROCESS_ID']) + '\n')
    attempt = os.environ.get('LF_TEST_ATTEMPT_FILE')
    failed = attempt is not None and not Path(attempt).exists()
    if failed:
        Path(attempt).touch()
    print(json.dumps({'type': 'result', 'uuid': 'fixture-result', 'session_id': 'fixture-thread', 'subtype': 'error_during_execution' if failed else 'success', 'is_error': failed, 'result': 'service unavailable' if failed else 'done', 'usage': {'input_tokens': 7, 'output_tokens': 3}}), flush=True)
"#,
        )
        .unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();

        let keys = [
            "PATH",
            "LF_BIN",
            "LF_HOME",
            loopflow_execution::journal::LF_TRACE_ID_ENV,
            loopflow_execution::journal::LF_PROCESS_ID_ENV,
            loopflow_execution::session_record::CAPTURE_KEY_ENV,
            "LF_RUN_DIR",
        ];
        let path = format!(
            "{}:{}",
            bin.display(),
            std::env::var("PATH").unwrap_or_default()
        );
        let _environment = EnvGuard::clear(&keys);
        std::env::set_var("PATH", path);
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        std::env::set_var("LF_HOME", home.path());
        let registry = home.path().join("loopflow.db");
        std::env::set_var(loopflow_execution::journal::LF_TRACE_ID_ENV, "trace_stale");
        std::env::set_var(
            loopflow_execution::journal::LF_PROCESS_ID_ENV,
            "process_stale",
        );
        std::env::set_var("LF_RUN_DIR", home.path().join("stale-run"));

        let task = "prove the captured Session launch";
        let context = loopflow_execution::trace::PreparedTurnContext::from_prompts("", task);
        let mut env = std::collections::BTreeMap::new();
        env.insert(
            "LF_TEST_RUN_EVIDENCE".to_string(),
            evidence.display().to_string(),
        );
        let built = PromptBuild {
            repo_root: home.path().to_path_buf(),
            agent_config: AgentConfig {
                task_prompt: task.to_string(),
                cwd: Some(home.path().to_path_buf()),
                yolo: true,
                env,
                ..AgentConfig::new(loopflow_execution::config::Agent::parse("claude").unwrap())
            },
            process: ProcessConfig::default(),
            components: PromptComponents::default(),
            context,
            skill_name: Some("implement".to_string()),
            skill_version: None,
            log_name: "generic-run-proof".to_string(),
            subjects: vec!["task:LOO-265".to_string()],
            work: None,
        };
        // Capture admission attaches the recorded lf invocation, as the CLI does.
        let command = vec!["lf".to_string(), "implement".to_string()];
        let admitted = || {
            loopflow_execution::journal::with_runtime(home.path(), &command, || {
                let runtime = begin_process_runtime(&built, "headless", None, None)?;
                let artifact_key = runtime.artifact_key();
                let run_dir = runtime.artifact_dir();

                assert!(run_dir.join("manifest.json").is_file());
                let manifest = std::fs::read_to_string(run_dir.join("manifest.json"))?;
                assert!(manifest.contains("task:LOO-265"));
                assert!(!run_dir.join("terminal.json").exists());
                let result = run_headless_prompt(&built, &runtime, LaunchKind::Batch);
                runtime.finish(if result.is_ok() {
                    "completed"
                } else {
                    "failed"
                })?;
                result?;
                Ok((artifact_key, run_dir))
            })
        };
        // The invocation and its capture share this fixture's store.
        let (artifact_key, run_dir) =
            loopflow_execution::journal::with_test_ledger(registry.clone(), admitted).unwrap();

        let provider_identity = std::fs::read_to_string(evidence).unwrap();
        assert_eq!(
            provider_identity.trim(),
            format!("{}|unset|unset|unset", artifact_key)
        );
        assert!(run_dir.join("terminal.json").is_file());
        assert!(!run_dir.join("owner.json").exists());

        let mut implicit_launch = built.agent_config.clone();
        implicit_launch.env.insert(
            "LF_TEST_RUN_EVIDENCE".to_string(),
            implicit_evidence.display().to_string(),
        );
        implicit_launch.env.insert(
            "LF_TEST_ATTEMPT_FILE".to_string(),
            home.path().join("implicit-attempt").display().to_string(),
        );
        let result = loopflow_execution::journal::with_test_ledger(registry.clone(), || {
            run_agent(&implicit_launch, &built.process, LaunchKind::Batch)
        })
        .unwrap();
        assert_eq!(result.exit_code, 0);
        let implicit_identities = std::fs::read_to_string(implicit_evidence).unwrap();
        let identities = implicit_identities.lines().collect::<Vec<_>>();
        assert_eq!(identities.len(), 2, "transient failure should retry once");
        assert_eq!(
            identities[0], identities[1],
            "retry must stay in one Session"
        );
        let fields = identities[0].split('|').collect::<Vec<_>>();
        assert_eq!(&fields[2..], ["unset", "unset"]);
        let implicit_run_id =
            loopflow_execution::session_record::parse_artifact_key(fields[0]).unwrap();
        assert_eq!(fields[1], "unset");
        let implicit_run_dir =
            loopflow_execution::session_record::record_dir(home.path(), &implicit_run_id).unwrap();
        assert_eq!(
            implicit_run_dir.file_name().and_then(|name| name.to_str()),
            Some(implicit_run_id.as_str())
        );
        assert!(implicit_run_dir.join("manifest.json").is_file());
        assert!(implicit_run_dir.join("terminal.json").is_file());
        assert!(!implicit_run_dir.join("owner.json").exists());
        // Completion does not wait for all telemetry. The provider evidence above
        // proves retry identity; session_record tests own usage and account events.
        assert!(registry.is_file());
        let db = rusqlite::Connection::open(&registry).unwrap();
        let tasks: i64 = db
            .query_row("SELECT count(*) FROM tasks", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            tasks, 0,
            "agent work needs storage, not registered planning Work"
        );
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
                agent_config: AgentConfig {
                    task_prompt: task.to_string(),
                    cwd: Some(repo.to_path_buf()),
                    yolo: true,
                    env,
                    ..AgentConfig::new(loopflow_execution::config::Agent::parse("claude").unwrap())
                },
                process: ProcessConfig::default(),
                components: PromptComponents::default(),
                context: loopflow_execution::trace::PreparedTurnContext::from_prompts("", task),
                skill_name: Some("research".to_string()),
                skill_version: Some("research-text".to_string()),
                log_name: log_name.to_string(),
                subjects: vec!["task:LOO-267".to_string()],
                work: None,
            }
        }

        let _lock = loopflow_execution::journal::test_env_lock();
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let provider = bin.join("claude");
        std::fs::write(
            &provider,
            r#"#!/usr/bin/env python3
import json
import os
import sys
import time
from pathlib import Path

if "--version" in sys.argv:
    print("claude test")
    sys.exit(0)
for line in sys.stdin:
    message = json.loads(line)
    if message.get("type") != "user":
        continue
    message["session_id"] = "research-thread"
    print(json.dumps(message), flush=True)
    time.sleep(float(os.environ["LF_TEST_RESEARCH_DELAY"]))
    output = Path(os.environ["LF_TEST_RESEARCH_OUTPUT"])
    output.parent.mkdir(parents=True, exist_ok=True)
    temporary = output.with_name(output.name + "." + os.environ["LF_CAPTURE_KEY"] + ".tmp")
    temporary.write_text(os.environ["LF_TEST_RESEARCH_CONTENT"] + "\n")
    temporary.replace(output)
    print(json.dumps({"type": "result", "uuid": message["uuid"],
        "session_id": "research-thread", "subtype": "success", "result": "Done",
        "usage": {"input_tokens": 7, "output_tokens": 3}}), flush=True)
"#,
        )
        .unwrap();
        std::fs::set_permissions(&provider, std::fs::Permissions::from_mode(0o755)).unwrap();

        let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        )))
        .unwrap();
        let _ambient = EnvGuard::new();
        let _environment = EnvGuard::clear(&["PATH", "LF_BIN", "LF_HOME"]);
        std::env::set_var("PATH", path);
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        std::env::set_var("LF_HOME", home.path());

        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(".lf/skills/proof.md", "inspect every research artifact");
        repo.stage_all();
        repo.commit("test basis");
        let head = loopflow_execution::git::rev_parse(repo.path(), "HEAD").unwrap();
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
        let cli = Cli {
            agent: Some("claude".into()),
            ..Cli::default()
        };

        // Each run is its own recorded lf invocation, as from the CLI.
        let command = vec!["lf".to_string(), "research".to_string()];
        let ledger = loopflow_execution::store::database_path_from_env().unwrap();
        let admitted = |built: PromptBuild| {
            loopflow_execution::journal::with_test_ledger(ledger.clone(), || {
                loopflow_execution::journal::with_runtime(repo.path(), &command, || {
                    run_prompt(built, &cli, LaunchKind::Batch)
                })
            })
        };
        std::thread::scope(|scope| {
            let first = scope.spawn(|| admitted(first));
            let second = scope.spawn(|| admitted(second));
            first.join().unwrap().unwrap();
            second.join().unwrap().unwrap();
        });

        // Each run recorded the skill text it launched with.
        let versions: Vec<Option<String>> = rusqlite::Connection::open(&ledger)
            .unwrap()
            .prepare("SELECT skill_version FROM agent_sessions")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(versions, vec![Some("research-text".to_string()); 2]);
        assert_eq!(
            std::fs::read_to_string(&runtime).unwrap(),
            "runtime evidence bytes\n"
        );
        assert_eq!(
            std::fs::read_to_string(&handoff).unwrap(),
            "handoff evidence bytes\n"
        );
        assert_eq!(
            loopflow_execution::git::rev_parse(repo.path(), "HEAD").unwrap(),
            head
        );
        assert!(std::process::Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(repo.path())
            .status()
            .unwrap()
            .success());

        let built = build_bound_prompt_at(
            Mode::Headless,
            Some("proof"),
            "reconcile",
            "",
            &cli,
            repo.path(),
        )
        .unwrap();
        let context = built
            .agent_config
            .conversation_context
            .as_ref()
            .unwrap()
            .block(loopflow_execution::context_block::ContextMoment::Start)
            .unwrap()
            .text;
        assert!(context.contains("runtime evidence bytes"));
        assert!(context.contains("handoff evidence bytes"));
    }

    #[test]
    fn worktree_harness_preloads_committed_and_untracked_scratch_with_provenance() {
        let _lock = loopflow_execution::journal::test_env_lock();
        let _env = EnvGuard::clear(&["LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(".lf/skills/proof.md", "inspect the complete basis");
        repo.create_file("scratch/a-committed.md", "committed evidence bytes");
        repo.stage_all();
        repo.commit("committed basis");
        repo.create_file("scratch/z-untracked.md", "untracked evidence bytes");
        let store =
            loopflow_execution::store::opening::open_ephemeral(&home.path().join("loopflow.db"))
                .unwrap();
        loopflow_execution::work::wave::bootstrap::ensure_wave(
            &store,
            repo.path().to_str().unwrap(),
            "ship",
        )
        .unwrap();

        let cli = Cli {
            batch: true,
            agent: Some("claude".into()),
            wave: Some("ship".to_string()),
            ..Cli::default()
        };
        let built = build_bound_prompt_at(
            Mode::Headless,
            Some("proof"),
            "continue",
            "",
            &cli,
            repo.path(),
        )
        .unwrap();

        let context = built
            .agent_config
            .conversation_context
            .as_ref()
            .unwrap()
            .block(loopflow_execution::context_block::ContextMoment::Start)
            .unwrap()
            .text;
        assert!(context.contains("committed evidence bytes"));
        assert!(context.contains("untracked evidence bytes"));
        assert!(!built
            .context
            .system
            .as_ref()
            .unwrap()
            .assets
            .iter()
            .any(|asset| asset.kind == ContextAssetKind::Scratch));
    }

    #[test]
    fn retained_skill_context_keeps_its_source_after_catalog_selection_changes() {
        let _lock = loopflow_execution::journal::test_env_lock();
        let _ambient = EnvGuard::new();
        let _home = EnvGuard::clear(&["LF_HOME", "HOME"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        std::env::set_var("HOME", home.path());
        let repo = tempfile::tempdir().unwrap();
        let native = repo.path().join(".claude/skills/audit/SKILL.md");
        std::fs::create_dir_all(native.parent().unwrap()).unwrap();
        std::fs::write(&native, "Original audit instructions").unwrap();
        let catalog = SkillCatalog::load(Some(repo.path()), None, false).unwrap();
        let skill = catalog.resolve("audit").unwrap().unwrap().load().unwrap();
        std::fs::remove_file(&native).unwrap();
        let invocation = loopflow_execution::skills::invocation::SkillInvocation {
            skill,
            arguments: "  exact \"arguments\"  ".into(),
        };
        let cli = Cli {
            batch: true,
            agent: Some("claude".into()),
            // Target selection already consumed this now-absent transport file.
            skill_input: Some(repo.path().join("removed-input.json")),
            resolved_invocation: Some(invocation.clone()),
            ..Cli::default()
        };
        let missing = build_prompt(Mode::Headless, repo.path(), Some("audit"), None, &cli).unwrap();
        assert_eq!(
            missing.agent_config.skill_invocation,
            Some(invocation.clone())
        );

        let replacement = repo.path().join(".lf/skills/audit.md");
        std::fs::create_dir_all(replacement.parent().unwrap()).unwrap();
        std::fs::write(replacement, "---\nagent: [\n---\nReplacement instructions").unwrap();
        let built = build_prompt(
            Mode::Headless,
            repo.path(),
            Some("audit"),
            Some("Retain this Work direction"),
            &cli,
        )
        .unwrap();
        assert_eq!(built.agent_config.skill_invocation, Some(invocation));
        assert!(built
            .agent_config
            .task_prompt
            .contains("Retain this Work direction"));
        assert!(!built
            .agent_config
            .task_prompt
            .contains("Replacement instructions"));
        // Inline attribution uses the same retained components; native input
        // records its source on the invocation instead of the context message.
        let context = attributed_context(
            &built.components,
            "",
            "Original audit instructions",
            &[],
            None,
        );
        let asset = context
            .task
            .assets
            .iter()
            .find(|asset| asset.kind == ContextAssetKind::SkillInstructions)
            .unwrap();
        assert_eq!(asset.source_path.as_deref(), native.to_str());
    }

    #[test]
    fn interactive_bound_skill_refreshes_scratch_and_keeps_saved_work_direction() {
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(".lf/skills/proof.md", "inspect the complete basis");
        repo.create_file("scratch/research-runtime.md", "runtime evidence bytes");
        repo.stage_all();
        repo.commit("bound basis");
        let cli = Cli {
            interactive: true,
            agent: Some("claude".into()),
            ..Cli::default()
        };

        let built = build_bound_prompt_at(
            Mode::Interactive,
            Some("proof"),
            "<lf:work kind=\"task\" id=\"task_test\">Task seed</lf:work>",
            "",
            &cli,
            repo.path(),
        )
        .unwrap();
        repo.create_file(
            "scratch/research-runtime.md",
            "evidence published after launch",
        );

        let delivery = built.agent_config.conversation_context.as_ref().unwrap();
        let context = delivery
            .block(loopflow_execution::context_block::ContextMoment::Start)
            .unwrap()
            .text;
        assert!(!context.contains("runtime evidence bytes"));
        assert!(context.contains("evidence published after launch"));
        assert!(built
            .agent_config
            .task_prompt
            .contains("inspect the complete basis"));
        assert!(!built.agent_config.task_prompt.contains("Task seed"));
        assert!(delivery
            .references
            .iter()
            .any(|path| std::fs::read_to_string(path).unwrap().contains("Task seed")));
    }

    #[test]
    fn explicit_interactive_skill_launch_uses_the_assembled_prompt() {
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(
            ".lf/skills/proof.md",
            "# Proof\n\nInstructions that must reach the provider.",
        );
        let cli = Cli::parse_from(["lf", "-i", "-a", "claude", "proof"]);

        let built = build_prompt_at(
            Mode::Interactive,
            Some("proof"),
            Some("verify the result"),
            &cli,
            repo.path().to_path_buf(),
            None,
            false,
        )
        .unwrap();

        assert!(built
            .agent_config
            .task_prompt
            .contains("Instructions that must reach the provider."));
        assert!(loopflow_execution::prompt::format_prompt(&built.components)
            .contains("verify the result"));
        assert!(
            !loopflow_execution::prompt::format_prompt(&built.components).starts_with("/proof")
        );
        assert!(
            !loopflow_execution::prompt::format_prompt(&built.components).starts_with("$proof")
        );
    }

    #[test]
    fn terminal_wave_skill_launch_delivers_the_authored_goal() {
        let _lock = loopflow_execution::journal::test_env_lock();
        let _restore = EnvGuard::clear(&["HOME", "LF_HOME"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("HOME", home.path());
        std::env::set_var("LF_HOME", home.path().join(".lf"));
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(
            ".lf/config.yaml",
            "diff: false\ndiff_files: false\npaste: false\n",
        );
        let goal =
            "## Objective\nShip a reliable release.\n\n## Bounds\nKeep rollback available.\n";
        repo.create_file(".lf/wave/release/GOAL.md", goal);
        let store = loopflow_execution::store::opening::open_ephemeral(
            &home.path().join(".lf/loopflow.db"),
        )
        .unwrap();
        loopflow_execution::work::wave::bootstrap::ensure_wave(
            &store,
            repo.path().canonicalize().unwrap().to_str().unwrap(),
            "release",
        )
        .unwrap();
        let cli = Cli::parse_from(["lf", "-i", "-a", "claude", "--wave", "release", "design"]);
        let built = build_prompt_at(
            Mode::Interactive,
            Some("design"),
            Some("plan the release"),
            &cli,
            repo.path().to_path_buf(),
            None,
            false,
        )
        .unwrap();

        assert_eq!(
            built
                .agent_config
                .system_prompt
                .matches("<lf:loopflow>")
                .count(),
            1
        );
        assert!(!built.agent_config.system_prompt.contains(goal));
        let context = built
            .agent_config
            .conversation_context
            .as_ref()
            .unwrap()
            .block(loopflow_execution::context_block::ContextMoment::Start)
            .unwrap()
            .text;
        assert_eq!(context.matches(goal).count(), 1);
    }
}

#[cfg(test)]
mod launch_policy_tests;
