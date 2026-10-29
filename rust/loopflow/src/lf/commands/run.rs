use crate::agent::check_cli_available;
use crate::agent::missing_agent_message;
use crate::agent::run_agent;
use crate::agent::AgentConfig;
use crate::agent::ProcessConfig;
use crate::config::parse_agent;
use crate::graph::Skill;
use crate::lf::commands::context::preview::PromptPreview;
use crate::lf::commands::util::launch_session;
use crate::lf::output::{format_context_header, format_reproducible_command, Colors};
use crate::lf::Cli;
use crate::prompt::process::prepare_process_prompt;
use crate::prompt::process::ContextSourceOverrides;
use crate::prompt::process::ProcessPromptInput;
use crate::prompt::PromptComponents;
use crate::prompt::Surface;
use crate::session_record::{
    AgentProcessRequest, CaptureHandle, SessionCaptureSpec, SubjectAttribution,
};
use anyhow::{anyhow, Result};
use std::io::IsTerminal;
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
pub fn run(repo_root: &Path, skill: Option<&str>, message: Option<&str>, cli: &Cli) -> Result<()> {
    if let Some(binding) = implicit_binding(cli)? {
        return run_bound_prompt(skill, message, cli, &binding).map(|_| ());
    }
    let mut built = build_prompt(repo_root, skill, message, cli)?;
    built.subjects = cli.work_subject_selector().into_iter().collect();
    // `--wave` resolved its Wave once for this process.
    built.work = cli.wave.as_ref().and_then(|_| {
        let id = std::env::var(crate::work::wave::context::WAVE_ID_ENV).ok()?;
        Some(crate::session::SessionWork {
            task_id: None,
            wave_id: Some(crate::id::WaveId::parse(&id).ok()?),
            source: crate::session::WorkSource::Declared,
        })
    });

    print_context_header(&built, cli);
    run_prompt(&built, cli).map(|_| ())
}

pub(crate) fn preview_prompt(
    cwd: &Path,
    skill: Option<&str>,
    message: Option<&str>,
    cli: &Cli,
    binding: Option<&crate::ops::WorkBinding>,
    captured: bool,
) -> Result<PromptPreview> {
    anyhow::ensure!(
        operator_scope(skill, cli, cwd, binding)?.is_none(),
        "context unavailable before the operator's scope checkout is prepared; preview from its existing checkout"
    );
    let built = match binding {
        Some(binding) => build_bound_prompt(skill, message, cli, binding)?,
        None => build_prompt_at(
            skill,
            message,
            message.unwrap_or_default(),
            cli,
            cwd.to_path_buf(),
            None,
            captured,
        )?,
    };
    Ok(PromptPreview {
        checkout: built.repo_root,
        system_prompt: crate::agent::system_prompt_with_structured_replies(&built.agent_config),
        task_prompt: built.agent_config.task_prompt,
        skill_invocation: built.agent_config.skill_invocation,
        references: built
            .components
            .docs
            .iter()
            .chain(&built.components.summaries)
            .filter(|doc| doc.source != crate::prompt::DocumentSource::Docs)
            .map(
                |doc| crate::lf::commands::context::preview::ContextReference {
                    path: doc.path.clone(),
                    content: doc.content.clone(),
                },
            )
            .collect(),
    })
}

/// `lf -b connect ID --message TEXT`: one more headless turn of a conversation,
/// using `message` and the provider's own history without re-executing its skill.
pub fn resume(id: &str, message: &str, cli: &Cli) -> Result<()> {
    if crate::transport::delegate_current(true)? {
        return Ok(());
    }
    let store = crate::store::sqlite::SqliteStore::new(&crate::store::database_path_from_env()?)?;
    let session = store
        .session(id)?
        .ok_or_else(|| anyhow!("session {id:?} was not found"))?;
    let mut resolved = crate::agent::resolve_session_config(&session, cli)?;
    let mut turn = cli.process_options();
    turn.agent = resolved.agent.clone();
    let message = if session.captured.is_some()
        && session.provider.as_deref()
            != Some(crate::config::parse_agent(resolved.agent()).0.as_str())
    {
        let request = crate::ops::human_session::conversation_log::continuation(&store, &session)?;
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
        None,
        Some(&message),
        &message,
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
    run_prompt(&built, &turn).map(|_| ())
}

#[doc(hidden)]
pub fn run_bound(
    skill: Option<&str>,
    message: Option<&str>,
    cli: &Cli,
    binding: &crate::ops::WorkBinding,
) -> Result<()> {
    run_bound_prompt(skill, message, cli, binding).map(|_| ())
}

/// Run a channel request through the ordinary attributed launch and settlement path.
pub(crate) fn answer_bound(
    message: &str,
    cli: &Cli,
    binding: &crate::ops::WorkBinding,
) -> Result<Option<String>> {
    run_bound_prompt(None, Some(message), cli, binding)
}

fn run_bound_prompt(
    skill: Option<&str>,
    message: Option<&str>,
    cli: &Cli,
    binding: &crate::ops::WorkBinding,
) -> Result<Option<String>> {
    let mut scoped;
    let binding = if let Some(scope) = operator_scope(skill, cli, &binding.cwd, Some(binding))? {
        scoped = binding.clone();
        scoped.cwd = crate::ops::human_session::ensure_scope_worktree(&binding.cwd, &scope)?.path;
        &scoped
    } else {
        binding
    };
    let built = build_bound_prompt(skill, message, cli, binding)?;
    print_context_header(&built, cli);
    run_prompt(&built, cli)
}

/// Preview and launch enrich the same selected Work; only launch prepares a checkout.
fn build_bound_prompt(
    skill: Option<&str>,
    message: Option<&str>,
    cli: &Cli,
    binding: &crate::ops::WorkBinding,
) -> Result<PromptBuild> {
    let mut launch = cli.process_options();
    launch.wave = binding.wave_name.clone();
    launch.agent = binding.agent.clone().or(launch.agent);
    let arguments = message.unwrap_or_default();
    let message = if let crate::durable::WorkRef::Task(id) = &binding.work {
        launch.task = Some(id.to_string());
        message.unwrap_or_default().to_owned()
    } else {
        bound_message(binding, message)
    };
    let cli = &launch;
    let mut built = build_bound_prompt_at(skill, &message, arguments, cli, &binding.cwd)?;
    built.agent_config.cwd = Some(binding.cwd.clone());
    if let Some(wave) = &binding.wave_id {
        built.agent_config.env.insert(
            crate::work::wave::context::WAVE_ID_ENV.to_string(),
            wave.to_string(),
        );
    } else {
        built
            .agent_config
            .env
            .remove(crate::work::wave::context::WAVE_ID_ENV);
    }
    built.subjects = binding.subjects.clone();
    built.work = Some(crate::session::SessionWork {
        task_id: match &binding.work {
            crate::durable::WorkRef::Task(id) => Some(id.clone()),
            crate::durable::WorkRef::Wave(_) | crate::durable::WorkRef::Project(_) => None,
        },
        wave_id: binding.wave_id.clone(),
        source: binding.source,
    });

    Ok(built)
}

/// An `lf` launch inside a registered Task's checkout binds to that Task unless
/// the caller selected Work explicitly. Otherwise checkout ownership wins over
/// an ancestor's explicit declaration. An unavailable registry leaves context
/// unresolved; it never reconstructs Task identity from process ancestry.
pub fn implicit_binding(cli: &Cli) -> Result<Option<crate::ops::WorkBinding>> {
    if cli.work_subject_selector().is_some() {
        return Ok(None);
    }
    let repo = std::env::current_dir()?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let Some(store) = crate::store::open_existing_store().await else {
            return Ok(None);
        };
        crate::ops::resolve_execution_binding(&std::sync::Arc::new(store), &repo)
            .await
            .map_err(anyhow::Error::from)
    })
}

pub(crate) fn bound_message(binding: &crate::ops::WorkBinding, message: Option<&str>) -> String {
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
    context: crate::trace::PreparedTurnContext,
    harness: String,
    model: Option<String>,
    skill_name: Option<String>,
    log_name: String,
    subjects: Vec<String>,
    work: Option<crate::session::SessionWork>,
}

fn build_prompt(
    repo_root: &Path,
    skill: Option<&str>,
    message: Option<&str>,
    cli: &Cli,
) -> Result<PromptBuild> {
    let start = Instant::now();
    let repo_root = match operator_scope(skill, cli, repo_root, None)? {
        Some(scope) => crate::ops::human_session::ensure_scope_worktree(repo_root, &scope)?.path,
        None => repo_root.to_path_buf(),
    };
    debug!(elapsed_ms = start.elapsed().as_millis(), "found repo root");
    build_prompt_at(
        skill,
        message,
        message.unwrap_or_default(),
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
    binding: Option<&crate::ops::WorkBinding>,
) -> Result<Option<crate::session::PrimaryScope>> {
    use crate::durable::WorkRef;
    use crate::session::PrimaryScope;

    if cli.cwd.is_some() || cli.task.is_some() || cli.wt.is_some() {
        return Ok(None);
    }
    let skill = resolve_skill(skill, cli, cwd)?;
    let scope = match (skill.as_ref().map(|skill| skill.name.as_str()), binding) {
        (Some("repo-operate"), None) => {
            PrimaryScope::Repository(crate::repository::CanonicalRepo::discover(cwd)?)
        }
        (Some("wave-operate"), Some(binding)) => match &binding.work {
            WorkRef::Wave(id) => PrimaryScope::Wave(id.clone()),
            _ => return Ok(None),
        },
        _ => return Ok(None),
    };
    Ok(
        (crate::repository::CanonicalRepo::discover(cwd)?.as_path() == cwd.canonicalize()?)
            .then_some(scope),
    )
}

/// Captured identity and body take precedence, including during scope selection.
fn resolve_skill(name: Option<&str>, cli: &Cli, repo: &Path) -> Result<Option<Skill>> {
    match &cli.resolved_invocation {
        Some(invocation) => Ok(Some(invocation.skill.clone())),
        None => Ok(name
            .map(|name| crate::graph::load_skill(name, repo))
            .transpose()?),
    }
}

fn build_bound_prompt_at(
    skill: Option<&str>,
    message: &str,
    arguments: &str,
    cli: &Cli,
    repo_root: &Path,
) -> Result<PromptBuild> {
    let mut built = build_prompt_at(
        skill,
        Some(arguments),
        arguments,
        cli,
        repo_root.to_path_buf(),
        Some((
            crate::trace::ContextAssetKind::Goal,
            crate::trace::ContextScope::Task,
        )),
        cli.skill_input.is_some(),
    )?;
    if message != arguments && !cli.context {
        let path = crate::prompt::write_prompt_log(repo_root, message, "work-direction", None)?;
        if let Some(context) = &mut built.agent_config.conversation_context {
            context.references.push(path);
        }
    }
    if message != arguments && cli.context {
        built.components.summaries.push(crate::prompt::Document {
            path: "work-direction (saved at launch)".into(),
            content: message.into(),
            source: crate::prompt::DocumentSource::Summary,
        });
    }
    Ok(built)
}

// Keep the existing unattended Task confinement while applying it equally to
// direct skills and Graphs. The broader checkout-only versus unattended-only
// policy remains Jack's choice; neither is selected by this intersection.
fn confine_checkout_agent(task_checkout: bool, interactive: bool) -> bool {
    task_checkout && !interactive
}

fn prepare_task_input(
    cli: &Cli,
) -> Result<Option<(crate::store::SharedStore, crate::ops::task_input::TaskSeed)>> {
    let Some(id) = &cli.task else { return Ok(None) };
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = std::sync::Arc::new(if cli.context {
            crate::store::read_existing_registry()?
                .ok_or_else(|| anyhow!("Task input unavailable: no local registry"))?
        } else {
            crate::store::open_store(&crate::store::storage_config_from_env()?).await?
        });
        let task = store
            .get_task_by_issue(id)
            .await?
            .ok_or_else(|| anyhow!("Task {id} is missing"))?;
        let repo = crate::ops::task::repository_for_task(&store, &task)?;
        if !cli.context {
            crate::ops::linear_observe::sync_repository(&store, &repo.to_string_lossy()).await;
        }
        let task = store
            .get_task(&task.id)
            .await?
            .ok_or_else(|| anyhow!("Task is missing"))?;
        let wave = match &task.wave_id {
            Some(id) => store.get_wave(id).await?,
            None => None,
        };
        let seed =
            crate::ops::task_input::read_seed(&store, &task, wave.as_ref().map(|w| w.slug()))
                .await?;
        Ok(Some((store, seed)))
    })
}

fn build_prompt_at(
    skill: Option<&str>,
    message: Option<&str>,
    arguments: &str,
    cli: &Cli,
    repo_root: PathBuf,
    message_context: Option<(crate::trace::ContextAssetKind, crate::trace::ContextScope)>,
    captured: bool,
) -> Result<PromptBuild> {
    let is_interactive = is_interactive_run(cli, skill, message);
    let task_input = prepare_task_input(cli)?;
    let task_checkout = match &task_input {
        Some((_, seed)) => {
            std::fs::canonicalize(seed.task.worktree()?)? == std::fs::canonicalize(&repo_root)?
        }
        None => false,
    };
    let confine = confine_checkout_agent(task_checkout, is_interactive);
    let task_message = task_input.as_ref().map(|(_, seed)| seed.message.clone());
    let wave = cli.wave.clone().or_else(|| {
        cli.resume
            .is_none()
            .then(crate::work::wave::context::resolve_ambient_wave_name)
            .flatten()
    });
    let config_start = Instant::now();
    let config =
        crate::config::load_wave_config(Some(&repo_root), wave.as_deref())?.unwrap_or_default();
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
    let invocation = cli.resolved_invocation.as_ref();
    let discovered_skill = resolve_skill(skill, cli, &repo_root)?;
    let arguments = invocation.map_or(arguments, |invocation| invocation.arguments.as_str());
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
        match crate::definition::name::portable_name(&skill.name).as_str() {
            "task-session" => anyhow::ensure!(
                task_input.is_some(),
                "Select a Task with `lf connect --task ISSUE`"
            ),
            "wave-session" => anyhow::ensure!(
                wave.is_some(),
                "Select a Wave with `lf connect --wave NAME`"
            ),
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
                source.dialect != crate::skills::catalog::SkillDialect::Loopflow
            })
        });
    let prepare = if cli.context {
        crate::prompt::process::preview_process_prompt
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
            docs: cli.docs.clone(),
            wave,
            message: message.map(|value| value.to_string()),
            skill_arguments: arguments.to_string(),
            no_loopflow: standalone_native,
            work: cli.task.clone().map(crate::prompt::WorkContext::Task),
            agent: task_input
                .as_ref()
                .and_then(|(_, seed)| seed.task.agent.clone())
                .or_else(|| cli.agent.clone()),
            yolo_mode: cli.yolo || crate::config::yolo_override() || config.yolo,
            source_overrides: ContextSourceOverrides {
                diff_files: cli.diff_files_setting(),
                diff: cli.diff_setting(),
                clipboard: if cli.clipboard { Some(true) } else { None },
            },
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
    let agent = prepared
        .config
        .agent
        .clone()
        .expect("prepare_process_prompt always sets agent");
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
        task_input: task_input
            .map(|(store, seed)| crate::ops::task_input::TaskInput::new(store, seed)),
        auto: !is_interactive,
        stream: !is_interactive,
        ..Default::default()
    };
    let mut agent_config = prepared.config;
    agent_config.chrome = cli.chrome_setting().unwrap_or(config.chrome);
    agent_config.output_schema = cli.output_schema.clone();
    if confine && !cli.context {
        agent_config.write_scope = crate::agent::AgentWriteScope::Worktree;
        agent_config.execution_boundary = Some(crate::agent::checkout_execution_boundary(
            &repo_root,
            agent_config.agent(),
        )?);
        agent_config.skip_permissions = true;
    }

    let mut components = prepared.components;
    components.message_context = message_context.or_else(|| {
        task_message.as_ref().map(|_| {
            (
                crate::trace::ContextAssetKind::Goal,
                crate::trace::ContextScope::Task,
            )
        })
    });
    let deduplication_decisions = prepared.deduplication_decisions;
    let effective_system = crate::agent::system_prompt_with_structured_replies(&agent_config);
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
        harness,
        model,
        skill_name,
        log_name,
        subjects: Vec::new(),
        work: None,
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
    if cli.batch {
        return false;
    }
    cli.interactive || attached_tty || (skill.is_none() && message.is_none())
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
        built.agent_config.agent.as_deref(),
    );
    eprintln!(
        "{dim}{header}\n\n  {command}{reset}",
        dim = colors.dim,
        header = header,
        command = command,
        reset = colors.reset,
    );
}

fn run_prompt(built: &PromptBuild, cli: &Cli) -> Result<Option<String>> {
    crate::transport::report(crate::program_status::State::Working, false, None);
    if !built.process.auto {
        info!("launching interactive vendor session");
        let capture = begin_capture(built, "tui", &built.agent_config, None)?;
        let agent_session: Option<crate::id::AgentSessionId> = if built.harness == "claude" {
            let artifact_key = capture.artifact_key();
            let raw_id = artifact_key
                .as_str()
                .strip_prefix("run_")
                .unwrap_or(&artifact_key);
            Some(
                uuid::Uuid::parse_str(raw_id)
                    .expect("capture keys always carry a UUID")
                    .to_string()
                    .into(),
            )
        } else {
            None
        };
        let mut environment = built.agent_config.env.clone();
        environment.extend(capture.environment());
        let result = (|| {
            let mut config = built.agent_config.clone();
            config.env = environment.clone();
            let context_file = crate::agent::write_system_prompt_file(&config, &built.log_name)?;
            config.session_attachment = capture.session_attachment();
            launch_session(&config, agent_session.as_ref(), context_file.as_deref())
        })();
        if let Some(provider_session) =
            crate::session_record::read_provider_session(&capture.artifact_dir())
                .map_err(|error| anyhow!("failed to read provider session: {error}"))?
        {
            capture.observe_provider(
                Some(provider_session.agent_session),
                provider_session.account_id,
            );
        }
        capture.finish(if result.is_ok() {
            "completed"
        } else {
            "failed"
        })?;
        return result.map(|_| None);
    }

    let cli_check_start = Instant::now();
    if !check_cli_available(&built.harness) {
        return Err(anyhow!(missing_agent_message(&built.harness)));
    }
    debug!(
        elapsed_ms = cli_check_start.elapsed().as_millis(),
        "checked cli availability"
    );

    let agent_config = built.agent_config.clone();
    let capture = begin_capture(built, "headless", &agent_config, cli.resume.as_deref())?;

    let result = run_headless_prompt(built, &capture, &agent_config);
    // Explicit native resume can retire this provider while its executor waits.
    // Settle our capture as interrupted; a failed Graph step would replay it.
    if capture.attachment_replaced()? {
        capture.finish("interrupted")?;
        return Err(crate::process::CommandExit(130).into());
    }
    let outcome = if result.is_ok() {
        "completed"
    } else {
        "failed"
    };
    let settlement = capture.finish(outcome);
    match (result, settlement) {
        (Err(error), Err(settlement)) => Err(error.context(format!(
            "Session execution also failed to settle: {settlement}"
        ))),
        (Err(error), Ok(())) => Err(error),
        (Ok(()), Err(error)) => Err(anyhow!("Session completed but did not settle: {error}")),
        (Ok(()), Ok(())) => Ok(capture.final_answer()?),
    }
}

fn run_headless_prompt(
    built: &PromptBuild,
    capture: &CaptureHandle,
    prepared_config: &AgentConfig,
) -> Result<()> {
    let context_file_start = Instant::now();
    let context_file = crate::agent::write_system_prompt_file(prepared_config, &built.log_name)?;
    debug!(
        elapsed_ms = context_file_start.elapsed().as_millis(),
        "wrote context log"
    );

    let mut process = built.process.clone();
    process.context_file = context_file;
    process.capture = Some(capture.clone().into());

    // Set up directive relay so agent skills can issue shell directives
    // (e.g. `cd` after `lf wt switch`).
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

    info!(harness = built.harness, "launching agent");
    let process_start = Instant::now();
    let result = run_agent(&agent_config, &process);

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
        Err(crate::process::CommandExit(130).into())
    } else if let Some(failure) = &result.failure {
        Err(anyhow!(
            "agent stopped after {failure}. Check {} for details.",
            capture.artifact_dir().display()
        ))
    } else {
        let reason = crate::os_process::exit_reason(&result.stderr);
        Err(anyhow!(
            "agent exited with code {}{reason}. Check {} for details.",
            result.exit_code,
            capture.artifact_dir().display()
        ))
    }
}

fn begin_capture(
    built: &PromptBuild,
    surface: &str,
    prepared_config: &AgentConfig,
    resume: Option<&str>,
) -> Result<CaptureHandle> {
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
        harness: built.harness.clone(),
        model: built.model.clone(),
        surface: surface.to_string(),
        cwd,
        repo: Some(built.repo_root.clone()),
        worktree: Some(built.repo_root.clone()),
        skill: built.skill_name.clone(),
        subjects,
        graph: crate::session_record::SessionGraphMembership::Independent,
        work: built.work.clone(),
    };
    let capture = if let Some(session) = resume {
        CaptureHandle::continue_with_context(
            session,
            spec,
            &built.context,
            AgentProcessRequest::from_prepared(prepared_config),
        )
    } else if let Some(id) = crate::ops::human_session::prepared_artifact_key()? {
        CaptureHandle::start_prepared(
            &crate::store::lf_home_dir(),
            &id,
            spec,
            &built.context,
            Some(AgentProcessRequest::from_prepared(prepared_config)),
        )
    } else {
        let launch = Some(AgentProcessRequest::from_prepared(prepared_config));
        CaptureHandle::begin_with_context(spec, &built.context, launch)
    }
    .map_err(|error| {
        anyhow!("failed to publish Session capture manifest before agent launch: {error}")
    })?;
    capture.claim_session_attachment()?;
    debug!(
        elapsed_ms = started.elapsed().as_millis(),
        "published Session capture"
    );
    Ok(capture)
}

pub(crate) fn attributed_context(
    components: &PromptComponents,
    system_prompt: &str,
    task_prompt: &str,
    deduplication_decisions: &[crate::trace::ContextDecision],
    request: Option<&str>,
) -> crate::trace::PreparedTurnContext {
    use crate::prompt::{DiffTier, DocumentSource};
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
        // Attribute the submitted reference bytes, while components retain the
        // authored source. Otherwise escaped references become anonymous assembly.
        let content = match included_by {
            "wave" | "docs" | "diff_files" | "diff" | "summary" | "clipboard" => {
                crate::prompt::escape_reference(content)
            }
            "message" => crate::prompt::render_message(content),
            "steers" => crate::prompt::render_reference(content),
            _ => content.to_string(),
        };
        for channel in [ContextChannel::System, ContextChannel::Task]
            .into_iter()
            .filter(|channel| match channel {
                ContextChannel::System => system_prompt.contains(&content),
                ContextChannel::Task => task_prompt.contains(&content),
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
                match_all_occurrences: !matches!(
                    included_by,
                    "message" | "steers" | "vendor_skill"
                ),
            });
        }
    };

    if components.operate {
        let source_path = std::path::Path::new(&components.repo_root)
            .join("rust/loopflow/src/builtins/LOOPFLOW.md");
        push(
            &crate::prompt::loopflow_section(),
            Kind::OperatingInstructions,
            Scope::Global,
            "LOOPFLOW.md".to_string(),
            source_path
                .is_file()
                .then(|| source_path.to_string_lossy().to_string()),
            "operate",
        );
    }
    if let Some(guidance) = tagged_block(system_prompt, "<lf:work-guidance>", "</lf:work-guidance>")
    {
        push(
            guidance,
            Kind::OperatingInstructions,
            Scope::Global,
            "work".to_string(),
            None,
            "work",
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
    if let Some(wave) = &components.wave {
        let open = format!("<lf:wave name=\"{wave}\">");
        let goal = tagged_block(system_prompt, &open, "</lf:wave>")
            .or_else(|| tagged_block(task_prompt, &open, "</lf:wave>"))
            .unwrap_or(open.as_str());
        push(goal, Kind::Goal, Scope::Wave, wave.clone(), None, "wave");
    }
    for document in &components.docs {
        let kind = if document.source == DocumentSource::Scratch {
            Kind::Scratch
        } else if document.source == DocumentSource::RepoMemory
            || (document.source == DocumentSource::Wave && document.path.ends_with("/MEMORY.md"))
        {
            Kind::Memory
        } else if document.path.ends_with("AGENTS.md") || document.path.ends_with("CLAUDE.md") {
            Kind::RepoInstructions
        } else {
            Kind::Document
        };
        let content = if document.source == DocumentSource::Docs {
            crate::prompt::format_document(document)
        } else {
            document.content.clone()
        };
        push(
            &content,
            kind,
            if document.source == DocumentSource::Wave {
                Scope::Wave
            } else {
                Scope::Repo
            },
            document.path.clone(),
            Some(document.path.clone()),
            if document.source == DocumentSource::Wave {
                "wave"
            } else {
                "docs"
            },
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
            push(
                content,
                Kind::SkillInstructions,
                Scope::Step,
                skill.name.clone(),
                skill
                    .source
                    .as_ref()
                    .map(|source| source.path.to_string_lossy().to_string()),
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
    // Bound launches append the caller's request to inherited Work context.
    // Attribute it separately before the enclosing Goal claims the same bytes.
    if let Some(request) = request {
        push(
            request,
            Kind::UserMessage,
            Scope::User,
            "user message".into(),
            None,
            "message",
        );
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

    let mut decisions = deduplication_decisions.to_vec();
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

    let mut context = crate::trace::PreparedTurnContext::from_attributed_prompts(
        system_prompt,
        task_prompt,
        specs,
        decisions,
    );
    for decision in &mut context.decisions {
        if decision.decision != ContextDecisionKind::Included {
            continue;
        }
        match decision.kind {
            Kind::OperatingInstructions if decision.label == "LOOPFLOW.md" => {
                decision.reason = "default launcher guidance".into();
            }
            Kind::OperatingInstructions if decision.label == "work" => {
                if let Some(work) = &components.work {
                    decision.reason = work.reason();
                }
            }
            Kind::SkillInstructions => {
                decision.reason = format!("selected skill: {}", decision.label);
            }
            Kind::SurfaceInstructions => {
                decision.reason = format!("{:?} conversation surface", components.surface);
            }
            Kind::RepoInstructions => {
                decision.reason = "repository guide included in this input".into();
            }
            _ => {}
        }
    }
    let mut omitted = |label: &str, decision, reason: &str| {
        context.decisions.push(ContextDecision {
            position: context.decisions.len() as u32,
            kind: Kind::OperatingInstructions,
            scope: Scope::Global,
            label: label.into(),
            source_path: None,
            decision,
            reason: reason.into(),
            original_bytes: None,
            original_tokens: None,
            asset_position: None,
        });
    };
    if !components.operate {
        omitted(
            "LOOPFLOW.md",
            ContextDecisionKind::Excluded,
            "automatic guidance disabled for this input",
        );
        omitted(
            "work",
            ContextDecisionKind::Excluded,
            "automatic guidance disabled for this input",
        );
    } else if components.work.is_none() {
        omitted(
            "work",
            ContextDecisionKind::Excluded,
            "no Task or Wave association",
        );
    } else if components
        .skill
        .as_ref()
        .is_some_and(|skill| skill.name == "work")
    {
        omitted(
            "work",
            ContextDecisionKind::Deduplicated,
            "the selected work skill supplies this guidance",
        );
    }
    context
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

#[cfg(test)]
mod tests {
    use super::{
        attributed_context, begin_capture, build_bound_prompt_at, build_prompt, build_prompt_at,
        is_interactive_run, is_interactive_run_with_tty, run_headless_prompt, run_prompt,
        PromptBuild,
    };

    use crate::agent::{run_agent, AgentConfig, ProcessConfig};
    use crate::lf::Cli;
    use crate::prompt::{Document, DocumentSource, PromptComponents};
    use crate::skills::catalog::SkillCatalog;
    use crate::test_ambient::EnvGuard;
    use crate::trace::{ContextAssetKind, ContextScope};
    use clap::Parser;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn context_choices_override_config_and_omission_inherits() {
        let _lock = crate::journal::test_env_lock();
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
                let mut args = vec!["lf", "--batch"];
                if let Some(choice) = choice {
                    args.extend(["--diff", choice]);
                }
                for browser in [None, Some("on"), Some("off")] {
                    let mut args = args.clone();
                    if let Some(browser) = browser {
                        args.extend(["--chrome", browser]);
                    }
                    let cli = Cli::parse_from(args);
                    let built =
                        build_bound_prompt_at(None, "inspect changes", "", &cli, repo.path())
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
                            .diff
                            .as_ref()
                            .is_some_and(|diff| diff.contains("+changed file body")),
                        patch
                    );
                    assert_eq!(
                        built.agent_config.chrome,
                        browser.map_or(configured, |value| value == "on")
                    );
                }
            }
        }
    }

    #[test]
    fn preferred_name_survives_fresh_launches_and_corrections() {
        let _lock = crate::journal::test_env_lock();
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
            "user:\n  name: Repository Owner\ndiff: false\ndiff_files: false\npaste: false\n",
        );
        let cli = Cli::parse_from(["lf", "--batch"]);

        for name in ["Jack", "Jacqueline", "  "] {
            std::fs::write(
                home.path().join("config.yaml"),
                format!("user:\n  name: '{name}'\n"),
            )
            .unwrap();
            // Each assembly reloads personal config, including the correction.
            for _ in 0..2 {
                let built =
                    build_bound_prompt_at(None, "choose the prototype path", "", &cli, repo.path())
                        .unwrap();
                assert_eq!(
                    built.components.user_name.as_deref(),
                    if name.trim().is_empty() {
                        Some("Git User")
                    } else {
                        Some(name)
                    }
                );
                let context =
                    crate::prompt::render_user_context(built.components.user_name.as_deref());
                assert_eq!(
                    crate::prompt::format_prompt(&built.components)
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
        let built = build_bound_prompt_at(None, "continue", "", &cli, repo.path()).unwrap();
        assert_eq!(built.components.user_name.as_deref(), Some("Git User"));
    }

    #[test]
    fn preferred_name_uses_remote_caller_and_leaves_background_work_unattributed() {
        let _lock = crate::journal::test_env_lock();
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
        let cli = Cli::parse_from(["lf", "--batch"]);
        for (caller, expected) in [("Jack", "Jack"), ("", "Host Owner")] {
            std::env::set_var("LF_USER_NAME", caller);
            let built = build_bound_prompt_at(None, "continue", "", &cli, repo.path()).unwrap();
            assert_eq!(built.components.user_name.as_deref(), Some(expected));
            assert!(built.agent_config.system_prompt.contains(expected));
            assert_eq!(built.agent_config.env["LF_USER_NAME"], expected);
        }
        std::env::set_var("LF_USER_NAME", "Jack");
    }

    #[cfg(unix)]
    #[test]
    fn ad_hoc_batch_launch_captures_session_without_planning_registry() {
        let _lock = crate::journal::test_env_lock();
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
            crate::journal::LF_TRACE_ID_ENV,
            crate::journal::LF_PROCESS_ID_ENV,
            crate::session_record::CAPTURE_KEY_ENV,
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
        std::env::set_var(crate::journal::LF_TRACE_ID_ENV, "trace_stale");
        std::env::set_var(crate::journal::LF_PROCESS_ID_ENV, "process_stale");
        std::env::set_var("LF_RUN_DIR", home.path().join("stale-run"));

        let task = "prove the captured Session launch";
        let context = crate::trace::PreparedTurnContext::from_prompts("", task);
        let mut env = std::collections::BTreeMap::new();
        env.insert(
            "LF_TEST_RUN_EVIDENCE".to_string(),
            evidence.display().to_string(),
        );
        let built = PromptBuild {
            repo_root: home.path().to_path_buf(),
            agent_config: AgentConfig {
                task_prompt: task.to_string(),
                agent: Some("claude".to_string()),
                cwd: Some(home.path().to_path_buf()),
                skip_permissions: true,
                env,
                ..AgentConfig::default()
            },
            process: ProcessConfig {
                auto: true,
                ..ProcessConfig::default()
            },
            components: PromptComponents::default(),
            context,
            harness: "claude".to_string(),
            model: None,
            skill_name: Some("implement".to_string()),
            log_name: "generic-run-proof".to_string(),
            subjects: vec!["task:LOO-265".to_string()],
            work: None,
        };
        // Capture admission attaches the recorded lf invocation, as the CLI does.
        let command = vec!["lf".to_string(), "implement".to_string()];
        let admitted = || {
            crate::journal::with_runtime(home.path(), &command, || {
                let capture = begin_capture(&built, "headless", &built.agent_config, None)?;
                let artifact_key = capture.artifact_key();
                let run_dir = capture.artifact_dir();

                assert!(run_dir.join("manifest.json").is_file());
                let manifest = std::fs::read_to_string(run_dir.join("manifest.json"))?;
                assert!(manifest.contains("task:LOO-265"));
                assert!(!run_dir.join("terminal.json").exists());
                let result = run_headless_prompt(&built, &capture, &built.agent_config);
                capture.finish(if result.is_ok() {
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
            crate::journal::with_test_ledger(registry.clone(), admitted).unwrap();

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
        let result = crate::journal::with_test_ledger(registry.clone(), || {
            run_agent(&implicit_launch, &built.process)
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
        let implicit_run_id = crate::session_record::parse_artifact_key(fields[0]).unwrap();
        assert_eq!(fields[1], "unset");
        let implicit_run_dir =
            crate::session_record::record_dir(home.path(), &implicit_run_id).unwrap();
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
                    agent: Some("claude".to_string()),
                    cwd: Some(repo.to_path_buf()),
                    skip_permissions: true,
                    env,
                    ..AgentConfig::default()
                },
                process: ProcessConfig {
                    auto: true,
                    ..ProcessConfig::default()
                },
                components: PromptComponents::default(),
                context: crate::trace::PreparedTurnContext::from_prompts("", task),
                harness: "claude".to_string(),
                model: None,
                skill_name: Some("research".to_string()),
                log_name: log_name.to_string(),
                subjects: vec!["task:LOO-267".to_string()],
                work: None,
            }
        }

        let _lock = crate::journal::test_env_lock();
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
        let head = crate::git::rev_parse(repo.path(), "HEAD").unwrap();
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

        // Each run is its own recorded lf invocation, as from the CLI.
        let command = vec!["lf".to_string(), "research".to_string()];
        let ledger = crate::store::database_path_from_env().unwrap();
        let admitted = |built: &PromptBuild| {
            crate::journal::with_test_ledger(ledger.clone(), || {
                crate::journal::with_runtime(repo.path(), &command, || run_prompt(built, &cli))
            })
        };
        std::thread::scope(|scope| {
            let first = scope.spawn(|| admitted(&first));
            let second = scope.spawn(|| admitted(&second));
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
        assert_eq!(crate::git::rev_parse(repo.path(), "HEAD").unwrap(), head);
        assert!(std::process::Command::new("git")
            .args(["diff", "--cached", "--quiet"])
            .current_dir(repo.path())
            .status()
            .unwrap()
            .success());

        let built =
            build_bound_prompt_at(Some("proof"), "reconcile", "", &cli, repo.path()).unwrap();
        let context = built
            .agent_config
            .conversation_context
            .as_ref()
            .unwrap()
            .block(crate::context_block::ContextMoment::Start)
            .unwrap()
            .text;
        assert!(context.contains("runtime evidence bytes"));
        assert!(context.contains("handoff evidence bytes"));
    }

    #[test]
    fn worktree_harness_preloads_committed_and_untracked_scratch_with_provenance() {
        let _lock = crate::journal::test_env_lock();
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
            crate::store::sqlite::SqliteStore::open_ephemeral(&home.path().join("loopflow.db"))
                .unwrap();
        store
            .ensure_wave(repo.path().to_str().unwrap(), "ship")
            .unwrap();

        let cli = Cli {
            batch: true,
            wave: Some("ship".to_string()),
            ..Cli::default()
        };
        let built =
            build_bound_prompt_at(Some("proof"), "continue", "", &cli, repo.path()).unwrap();

        let context = built
            .agent_config
            .conversation_context
            .as_ref()
            .unwrap()
            .block(crate::context_block::ContextMoment::Start)
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
        let _lock = crate::journal::test_env_lock();
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
        let invocation = crate::skills::invocation::SkillInvocation {
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
        let missing = build_prompt(repo.path(), Some("audit"), None, &cli).unwrap();
        assert_eq!(
            missing.agent_config.skill_invocation,
            Some(invocation.clone())
        );

        let replacement = repo.path().join(".lf/skills/audit.md");
        std::fs::create_dir_all(replacement.parent().unwrap()).unwrap();
        std::fs::write(replacement, "---\nagent: [\n---\nReplacement instructions").unwrap();
        let built = build_prompt(
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
            ..Cli::default()
        };

        let built = build_bound_prompt_at(
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
            .block(crate::context_block::ContextMoment::Start)
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
    fn explicit_interactive_runs_without_a_tty() {
        let cli = Cli::parse_from(["lf", "-i", "gate"]);

        assert!(is_interactive_run_with_tty(&cli, Some("gate"), None, false));
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
    fn explicit_interactive_skill_launch_uses_the_assembled_prompt() {
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(
            ".lf/skills/proof.md",
            "# Proof\n\nInstructions that must reach the provider.",
        );
        let cli = Cli::parse_from(["lf", "-i", "proof"]);

        let built = build_prompt_at(
            Some("proof"),
            Some("verify the result"),
            "",
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
        assert!(crate::prompt::format_prompt(&built.components).contains("verify the result"));
        assert!(!crate::prompt::format_prompt(&built.components).starts_with("/proof"));
        assert!(!crate::prompt::format_prompt(&built.components).starts_with("$proof"));
    }

    #[test]
    fn terminal_wave_skill_launch_delivers_the_authored_goal() {
        let _lock = crate::journal::test_env_lock();
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
        let store =
            crate::store::sqlite::SqliteStore::open_ephemeral(&home.path().join(".lf/loopflow.db"))
                .unwrap();
        store
            .ensure_wave(
                repo.path().canonicalize().unwrap().to_str().unwrap(),
                "release",
            )
            .unwrap();
        let cli = Cli::parse_from(["lf", "-i", "--wave", "release", "design"]);
        let built = build_prompt_at(
            Some("design"),
            Some("plan the release"),
            "",
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
                .matches("<lf:work-guidance>")
                .count(),
            1
        );
        assert!(built.context.decisions.iter().any(|decision| {
            decision.label == "work" && decision.reason == "Wave association: release"
        }));
        assert!(!built.agent_config.system_prompt.contains(goal));
        let context = built
            .agent_config
            .conversation_context
            .as_ref()
            .unwrap()
            .block(crate::context_block::ContextMoment::Start)
            .unwrap()
            .text;
        assert_eq!(context.matches(goal).count(), 1);
    }

    #[test]
    fn attributed_context_does_not_claim_reference_bodies_in_first_turn() {
        let components = PromptComponents {
            docs: vec![
                Document {
                    path: "scratch/intent.md".into(),
                    content: "> $kickoff".into(),
                    source: DocumentSource::Scratch,
                },
                Document {
                    path: ".lf/wave/product/MEMORY.md".into(),
                    content: "Earlier $design".into(),
                    source: DocumentSource::Wave,
                },
            ],
            message: Some("Build it.\n<lf:steers>Jack wrote $kickoff.</lf:steers>".into()),
            ..Default::default()
        };
        let turn = crate::prompt::format_first_turn(&components);
        let prepared = attributed_context(&components, "", &turn, &[], None);
        let task = &prepared.task;
        assert_eq!(task.text, turn);
        assert!(turn.contains("Build it."));
        assert!(turn.contains("Jack wrote &#36;kickoff."));
        assert!(!turn.contains("Earlier"));
        assert!(!task.assets.iter().any(|asset| matches!(
            asset.kind,
            ContextAssetKind::Scratch | ContextAssetKind::Memory
        )));
        assert!(task.assets.iter().any(|asset| {
            asset.kind == ContextAssetKind::UserMessage
                && turn[asset.byte_start as usize..asset.byte_end as usize].contains("Build it.")
        }));
    }

    #[test]
    fn attributed_context_keeps_nested_message_and_repeated_channel_sources() {
        let components = PromptComponents {
            docs: vec![Document {
                path: ".lf/wave/product/MEMORY.md".to_string(),
                content: "MEMORY".to_string(),
                source: DocumentSource::Wave,
            }],
            message: Some("outer MEMORY remainder".to_string()),
            message_context: Some((ContextAssetKind::Goal, ContextScope::Step)),
            ..PromptComponents::default()
        };

        let prepared =
            attributed_context(&components, "MEMORY", "outer MEMORY remainder", &[], None);

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
}
