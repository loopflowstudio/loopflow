//! Native provider launch and continuation of a recorded conversation.

use crate::session::Holder;
use anyhow::{anyhow, bail, Result};
use std::collections::BTreeMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{Launch, Mode, Start};
use crate::agent::missing_agent_message;
use crate::agent::AgentConfig;
use crate::harness::native_exit::{attachment_lost, departure};
use crate::harness::Admitted;
use crate::process::LfProcessRuntime;
use loopflow_model::id::AgentSessionId;
use loopflow_storage::sqlite::SqliteStore;

#[derive(Debug)]
pub struct NativeClient {
    pub process: Command,
    pub remote: Option<PathBuf>,
    pub thread: Option<AgentSessionId>,
    pub companion: Option<Companion>,
    pub(crate) title: Option<crate::terminal_title::TerminalTitle>,
    pub reported_thread: Option<PathBuf>,
    pub stderr: Option<File>,
}

pub struct Companion {
    pub closed: Option<tokio::sync::oneshot::Sender<()>>,
    pub worker: Option<std::thread::JoinHandle<Result<()>>>,
}

impl std::fmt::Debug for Companion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Companion").finish_non_exhaustive()
    }
}
impl Companion {
    pub fn finish(&mut self) -> Result<()> {
        if let Some(closed) = self.closed.take() {
            let _ = closed.send(());
        }
        if let Some(worker) = self.worker.take() {
            worker
                .join()
                .map_err(|_| anyhow!("provider observer panicked"))??;
        }
        Ok(())
    }
}
impl Drop for Companion {
    fn drop(&mut self) {
        if let Err(error) = self.finish() {
            tracing::warn!(%error, "cannot stop provider companion");
        }
    }
}

impl NativeClient {
    pub(crate) fn new(program: &str, launch: &Launch<'_>) -> Result<Self> {
        let cwd = launch
            .config
            .cwd
            .clone()
            .map(Ok)
            .unwrap_or_else(std::env::current_dir)?;
        let mut process = Command::new(program);
        process
            .current_dir(cwd)
            .env_remove("LOOPFLOW_DIRECTIVE_FILE")
            .envs(&launch.config.env);
        crate::provider_auth::apply_provider_env_to_command(program, &mut process);
        let title = crate::terminal_title::TerminalTitle::prepare(
            &launch.config.env,
            program,
            &mut process,
        );
        process.env_remove(crate::context_block::CONTEXT_DELIVERY_ENV);
        Ok(Self {
            process,
            title,
            remote: None,
            thread: launch.thread()?,
            companion: None,
            reported_thread: None,
            stderr: None,
        })
    }
}

pub fn launch_session(
    config: &AgentConfig,
    holder: Holder,
    context_file: Option<&Path>,
    capture: Option<&LfProcessRuntime>,
) -> Result<NativeOutcome> {
    let cwd = config
        .cwd
        .clone()
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)?;
    crate::journal::with_runtime(&cwd, &std::env::args().collect::<Vec<_>>(), || {
        let mut environment = config.env.clone();
        let (admitted, created) = match capture {
            Some(capture) => {
                environment.extend(capture.environment());
                (Admitted::from_runtime(capture)?, None)
            }
            None => admit(config.agent()?, &cwd, &mut environment)?,
        };
        let result = {
            let mut config = config.clone();
            config.env = environment.clone();
            run(
                &Launch {
                    config: &config,
                    mode: Mode::Interactive,
                    holder,
                    admitted: &admitted,
                    start: Start::New,
                    context_file,
                },
                None,
                capture.or(created.as_ref()),
            )
        };
        if capture.is_none() {
            finish_admission(&admitted, created, &result);
        }
        result
    })
}

pub(crate) fn resume_session(
    artifact_key: &String,
    provider_session: &crate::session_record::ProviderSessionRef,
    named_work: bool,
) -> Result<()> {
    resume_session_with_env(
        artifact_key,
        Some(provider_session),
        &BTreeMap::new(),
        None,
        named_work,
    )
}

/// Continue a recorded conversation. `named_work` is the resume command's: a
/// Task or Wave resume starts only beside the Machine server.
pub fn resume_session_with_env(
    artifact_key: &String,
    provider_session: Option<&crate::session_record::ProviderSessionRef>,
    extra_environment: &BTreeMap<String, String>,
    launch_lock: Option<File>,
    named_work: bool,
) -> Result<()> {
    let store = crate::store::open_sqlite(&crate::store::database_path_from_env()?)?;
    let session = store
        .session_for_artifact(artifact_key)?
        .ok_or_else(|| anyhow!("Session input {artifact_key} is not recorded"))?;
    let mut config =
        crate::agent::resolve_session_config(&session, serde_yaml_ng::Value::Null, None, false)?;
    // Reopening a conversation recorded as Task, Wave or repository work is
    // Work whatever command reopened it.
    let holder = crate::transport::holder(store.is_work(named_work, Some(&session))?, None);
    anyhow::ensure!(
        session.provider.as_deref() == Some(crate::config::parse_agent(config.agent()?).0.as_str()),
        "native history cannot resume in {}; continue it with lf resume -a {}",
        config.agent()?,
        config.agent()?,
    );
    // A held Codex sends its first turn only to a thread with none; any other
    // resumed thread already has it.
    let held_codex = session
        .provider
        .as_deref()
        .and_then(super::HarnessKind::parse)
        .is_some_and(|kind| kind.harness().initial_prompt_on_resume())
        && holder != Holder::Caller;
    if provider_session.is_some() && !held_codex {
        config.task_prompt.clear();
    }
    config.env.extend(extra_environment.clone());
    config.env.insert(
        crate::session_record::CAPTURE_KEY_ENV.into(),
        artifact_key.clone(),
    );
    config.env.insert(
        crate::config::USER_NAME_ENV.into(),
        crate::config::participant_name()?.unwrap_or_default(),
    );
    let context_file = crate::agent::write_system_prompt_file(&config, "resume-instructions")?;
    let cwd = config
        .cwd
        .as_deref()
        .expect("resolved session config has a working directory");
    crate::journal::with_runtime(cwd, &std::env::args().collect::<Vec<_>>(), || {
        let mut environment = config.env.clone();
        let (admitted, capture) = admit(config.agent()?, cwd, &mut environment)?;
        let mut config = config.clone();
        config.env = environment;
        let result = run(
            &Launch {
                config: &config,
                mode: Mode::Interactive,
                holder,
                admitted: &admitted,
                start: provider_session.map_or(Start::New, |provider| Start::Resume {
                    thread: &provider.agent_session,
                }),
                context_file: context_file.as_deref(),
            },
            launch_lock,
            capture.as_ref(),
        );
        finish_admission(&admitted, capture, &result);
        result?.result(
            super::HarnessKind::parse(&crate::config::parse_agent(config.agent()?).0)
                .expect("resolved harness"),
            provider_session.is_some(),
            holder,
        )
    })
}

pub(crate) fn require_provider_session_process(dir: &Path) -> Result<()> {
    let input = crate::session_record::input_id_from_dir(dir)?;
    let store = SqliteStore::open_processes_read_only(
        &crate::store::database_path_from_env()?,
        crate::store::token_key::encryption_key,
    )?;
    let session = store
        .session_for_artifact(&input)?
        .ok_or_else(|| anyhow!("Input {input} is not recorded on this Machine"))?;
    require_session_task(&store, &session)
}

pub(crate) fn require_session_task(
    store: &SqliteStore,
    session: &crate::session::LfSession,
) -> Result<()> {
    let Some(task_id) = &session.task_id else {
        return Ok(());
    };
    let task = store
        .task_by_issue(task_id.as_str())?
        .ok_or_else(|| anyhow!("Task {task_id} is not registered"))?;
    if store.task_deleted(&task)? {
        bail!(
            "Task {} was deleted and cannot resume execution",
            task.identifier
        );
    }
    Ok(())
}

pub(crate) fn departure_notice(
    store: &SqliteStore,
    session: &str,
    departure: crate::process::Departure,
) -> Result<String> {
    use crate::process::Departure;
    Ok(match departure {
        Departure::Terminal { to: Some(to) } => {
            format!("This conversation moved to {to}: lf resume {session}")
        }
        Departure::Terminal { to: None } => {
            "This conversation moved to another terminal on this Machine.".into()
        }
        Departure::Machine { machine, task } => {
            let label = crate::store::machines::machine_by_id(store, &machine)?
                .map_or_else(|| machine.to_string(), |machine| machine.name());
            // A Task that went with its conversation is resumed by its name.
            match task.map(|task| store.task(&task)).transpose()?.flatten() {
                Some(task) => format!(
                    "This conversation moved to {label}: lf resume --task {}",
                    task.identifier
                ),
                None => format!("This conversation moved to {label}: lf resume {session}"),
            }
        }
    })
}

#[derive(Debug)]
pub struct NativeOutcome {
    pub status: std::process::ExitStatus,
    pub interrupted: bool,
    /// Where the conversation went when a mover took it from this launch.
    pub moved: Option<String>,
}

impl NativeOutcome {
    pub fn capture_outcome(&self) -> &'static str {
        if self.status.success() || self.interrupted {
            "completed"
        } else {
            "failed"
        }
    }

    /// Whether the launch failed: an interrupt or a mover ends it as a success.
    pub fn failed(&self) -> bool {
        self.moved.is_none() && !self.interrupted && !self.status.success()
    }

    /// The status a bare launch exits with: the provider's own, as its shell
    /// would see it alone. lf names a failure the person did not cause.
    pub(crate) fn bare_exit_code(&self, kind: super::HarnessKind) -> u8 {
        let program = kind.as_str();
        if !self.status.success() && !self.interrupted {
            eprintln!("{program} exited with {}", self.status);
        }
        crate::transport::provider_exit_code(self.status)
    }

    /// A bare launch ends as its provider would alone, with the provider's
    /// own status, unless a mover took its conversation; only a held
    /// conversation can have another client.
    pub fn result(&self, kind: super::HarnessKind, resuming: bool, holder: Holder) -> Result<()> {
        let program = kind.as_str();
        if holder == Holder::Caller && self.moved.is_none() {
            match self.bare_exit_code(kind) {
                0 => Ok(()),
                code => Err(crate::process::CommandExit(code).into()),
            }
        } else if !self.failed() {
            Ok(())
        } else if resuming {
            Err(anyhow!(
                "{program} exited with {}. If another client still owns this session, close it and run `lf resume` again.",
                self.status,
            ))
        } else {
            Err(anyhow!(
                "session launcher exited with status {}",
                self.status
            ))
        }
    }
}

fn record_interactive_opened(environment: &BTreeMap<String, String>) -> Result<()> {
    let Some(input) = environment.get(crate::session_record::CAPTURE_KEY_ENV) else {
        return Ok(());
    };
    let Some(process) = crate::journal::current_lf_process_id() else {
        return Ok(());
    };
    let store = crate::store::open_sqlite(&crate::store::database_path_from_env()?)?;
    let session = store
        .session_for_artifact(input)?
        .ok_or_else(|| anyhow!("Session input {input} is not recorded"))?;
    if !session.interactive {
        return Ok(());
    }
    let now = time::OffsetDateTime::now_utc();
    store.retain_session_observation(&session, &crate::session::SessionObservation {
        artifact_key: crate::session_record::parse_artifact_key(input)?,
        source: format!("interactive_opened:{process}:{}", session.id),
        observed_at: now.unix_timestamp(),
        task_id: session.task_id.clone(),
        wave_id: session.wave_id.clone(),
        payload: serde_json::json!({"type":"interactive_opened", "opened_at_ms": now.unix_timestamp_nanos() / 1_000_000}),
    })?;
    Ok(())
}

fn validate_launch(launch: &Launch<'_>) -> Result<()> {
    let Admitted {
        store,
        session,
        attachment,
    } = launch.admitted;
    let caller = launch
        .config
        .env
        .get(crate::process::AGENT_CALLER_ENV)
        .ok_or_else(|| anyhow!("Native launch has no admitted conversation"))?;
    let caller: crate::process::AgentCaller = serde_json::from_str(caller)?;
    let process = crate::journal::current_lf_process_id()
        .ok_or_else(|| anyhow!("Native launch has no admitted invocation"))?;
    if attachment.lf_process_id.as_ref() != Some(&process)
        || attachment.caller(session.clone().into()) != caller
    {
        bail!("Session attachment differs from native launch provenance");
    }
    store.with_session_attachment(session, attachment, || Ok(()))?;
    Ok(())
}

fn native_provider_attachment(
    launch: &Launch<'_>,
    remote: Option<&Path>,
) -> Result<crate::harness::Admitted> {
    let Admitted {
        store,
        session,
        attachment,
    } = launch.admitted;
    let process = crate::journal::current_lf_process_id()
        .ok_or_else(|| anyhow!("Native launch has no admitted invocation"))?;
    store.with_session_attachment(session, attachment, || {
        if let Some(remote) = remote {
            let endpoint = store
                .agent_endpoint(&attachment.agent_process_id)?
                .ok_or_else(|| {
                    crate::store::StoreError::InvalidAuthority(
                        "Remote client has no recorded AgentProcess endpoint".into(),
                    )
                })?;
            if Path::new(&endpoint) != remote {
                return Err(crate::store::StoreError::InvalidAuthority(
                    "Remote client endpoint differs from its AgentProcess".into(),
                ));
            }
        } else if attachment.provider_lf_process_id != process {
            return Err(crate::store::StoreError::InvalidAuthority(
                "Native provider launch does not own the admitted AgentProcess".into(),
            ));
        }
        Ok(())
    })?;
    Ok(launch.admitted.clone())
}

/// Admit a native invocation before constructing its provider command.
pub fn admit(
    program: &str,
    cwd: &Path,
    environment: &mut BTreeMap<String, String>,
) -> Result<(Admitted, Option<LfProcessRuntime>)> {
    let mut capture = None;
    if !environment.contains_key(crate::session_record::CAPTURE_KEY_ENV) {
        let context = crate::trace::PreparedTurnContext::from_prompts("", "");
        let created = crate::session_record::SessionRecorder::begin_with_context(
            crate::session_record::SessionCaptureSpec {
                harness: crate::config::parse_agent(program).0,
                model: None,
                surface: "tui".into(),
                cwd: cwd.into(),
                repo: None,
                worktree: Some(cwd.into()),
                skill: None,
                subjects: Vec::new(),
                graph: crate::session_record::SessionGraphMembership::Independent,
                work: None,
            },
            &context,
            None,
        )
        .map(LfProcessRuntime::new)?;
        environment.extend(created.environment());
        capture = Some(created);
    }
    let store = crate::store::open_sqlite(&crate::store::database_path_from_env()?)?;
    let input = environment
        .get(crate::session_record::CAPTURE_KEY_ENV)
        .expect("native admission has a capture");
    let session = store
        .session_for_artifact(input)?
        .ok_or_else(|| anyhow!("Session input {input} is not recorded"))?;
    let process = crate::journal::current_lf_process_id()
        .ok_or_else(|| anyhow!("Native launch has no admitted invocation"))?;
    let attachment = crate::process::resume_session_agent_process(&store, &session.id, &process)?;
    store.make_session_interactive(&session.id, &attachment)?;
    if let Some(terminal) = crate::transport::current() {
        store.bind_session_terminal(&session.id, &attachment, terminal)?;
    }
    environment.insert(
        crate::process::AGENT_CALLER_ENV.into(),
        serde_json::to_string(&attachment.caller(session.id.clone()))?,
    );
    crate::process::register_session_attachment_interrupt(
        &store,
        session.id.to_string(),
        attachment.clone(),
    );
    Ok((
        Admitted {
            store,
            session: session.id.to_string(),
            attachment,
        },
        capture,
    ))
}

fn finish_admission(
    admitted: &Admitted,
    capture: Option<LfProcessRuntime>,
    result: &Result<NativeOutcome>,
) {
    let outcome = result
        .as_ref()
        .map_or("failed", NativeOutcome::capture_outcome);
    let Admitted {
        store,
        session,
        attachment,
    } = admitted;
    match crate::process::finish_session_attachment(store, session, attachment, outcome) {
        Ok(()) | Err(crate::store::StoreError::InvalidAuthority(_)) => {}
        Err(error) => {
            tracing::warn!(%error, %session, "cannot record the Session attachment's end")
        }
    }
    crate::transcript::facts::record_session(
        store,
        session,
        crate::transcript::facts::Moment::Exit,
    );
    if let Some(capture) = capture {
        if let Err(error) = capture.finish(outcome) {
            tracing::warn!(%error, "cannot settle the Session capture at exit");
        }
    }
}

/// Claude's folder-trust question is the provider asking a person, before any
/// hook can say so; a hidden pane would otherwise hold it unseen. The provider's
/// first reading after an answer replaces this one.
pub(crate) fn report_folder_trust_dialog(attachment: &crate::harness::Admitted) {
    let Admitted {
        store,
        session,
        attachment: expected,
    } = attachment;
    let reading = crate::session::SessionActivity {
        preview: None,
        observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
        open_tools: 0,
        pending_input: 1,
        yielded: false,
    };
    if let Err(error) = store.record_session_activity(session, expected, &reading) {
        tracing::warn!(%error, session, "failed to report Claude's folder-trust dialog");
    }
}

pub fn run(
    launch: &Launch<'_>,
    launch_lock: Option<File>,
    capture: Option<&LfProcessRuntime>,
) -> Result<NativeOutcome> {
    anyhow::ensure!(
        launch.mode == Mode::Interactive,
        "native client requires an interactive launch"
    );
    let cwd = launch
        .config
        .cwd
        .clone()
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)?;
    let _planning_sync = crate::ops::planning_sync::PlanningSync::start_for_directory(&cwd)?;
    let harness = launch.harness()?;
    let program = harness.kind().as_str();
    let mut progress = crate::startup::Startup::new(format!("Preparing {program}"));
    // Admission and Task checks precede provider setup, including private servers
    // and Claude's folder-trust write.
    let owner = launch.admitted;
    validate_launch(launch)?;
    if let Some(key) = launch
        .config
        .env
        .get(crate::session_record::CAPTURE_KEY_ENV)
    {
        require_provider_session_process(&crate::session_record::capture_dir(key)?)?;
    }
    owner.store.record_session_terminal(
        &owner.session,
        &owner.attachment,
        crate::session_record::current_terminal_id()
            .or_else(crate::terminal_host::host_placement)
            .as_deref(),
    )?;
    let mut client = harness.open(launch)?;
    let result = run_client(launch, &mut client, launch_lock, capture, &mut progress);
    if let Some(companion) = &mut client.companion {
        companion.finish()?;
    }
    if let Ok(outcome) = &result {
        if let Some(moved) = &outcome.moved {
            eprintln!("{moved}");
        }
    }
    result
}

fn run_client(
    launch: &Launch<'_>,
    client: &mut NativeClient,
    launch_lock: Option<File>,
    capture: Option<&LfProcessRuntime>,
    progress: &mut crate::startup::Startup,
) -> Result<NativeOutcome> {
    let environment = &launch.config.env;
    let program = launch.harness()?.kind().as_str();
    let cwd = launch
        .config
        .cwd
        .clone()
        .map(Ok)
        .unwrap_or_else(std::env::current_dir)?;
    let agent_session = client.thread.as_ref();
    let started = std::time::Instant::now();
    // A connect that exec'd into this launch keeps its Session launch lock held
    // until the provider is recorded.
    let launch_lock = launch_lock.or_else(crate::ops::human_session::inherited_launch_lock);
    let attachment = native_provider_attachment(launch, client.remote.as_deref())?;
    let capture_dir = environment
        .get(crate::session_record::CAPTURE_KEY_ENV)
        .map(|key| crate::session_record::capture_dir(key))
        .transpose()?;
    if let (Some(provider), Some(thread)) = (launch.harness()?.kind().provider(), agent_session) {
        crate::transcript::adopt_native(provider, thread)?;
    }
    progress.stage(format!("Opening {program} terminal"));
    progress.finish();
    let process = &mut client.process;
    let title = &mut client.title;
    // Only an explicit remote connection is a client of a surviving AgentProcess.
    let owned = (client.remote.is_none() && client.companion.is_none()).then_some(&attachment);
    // Refused before the provider conversation is recorded, so the next launch
    // starts it fresh instead of resuming one that never ran. A Session's
    // first launch (`lf resume` starting a repository conversation) names no
    // Work on its command line; its recorded Session does.
    if let Some(Admitted { store, session, .. }) = owned {
        let named_work = launch.holder == Holder::Server;
        if store.is_work(named_work, store.session(session)?.as_ref())? {
            crate::harness::agent_process::require_server()?;
        }
    }
    if let (Some(capture_dir), Some(agent_session)) = (capture_dir.as_deref(), agent_session) {
        crate::session_record::write_provider_session(capture_dir, agent_session, None)?;
    }
    tracing::debug!(
        elapsed_ms = started.elapsed().as_millis(),
        "prepared native provider launch"
    );
    let output = crate::terminal_output::TerminalOutput::enter()?;
    output.configure_child(process)?;
    if let Some(stderr) = client.stderr.take() {
        process.stderr(stderr);
    }
    record_interactive_opened(environment)?;
    if client.companion.is_none() && crate::transport::current().is_some() {
        // Hand the wait to the execution owner and become the provider, or its
        // client: no launcher sits beside the agent holding the heap that built it.
        if let Some(capture) = capture {
            capture.observe_provider(agent_session.cloned());
        }
        let exit = crate::harness::native_exit::NativeExit {
            session: attachment.session.clone(),
            attachment: attachment.attachment.clone(),
            provider: program.to_string(),
            client: owned.is_none(),
            cwd: cwd.clone(),
            capture: capture.map(LfProcessRuntime::hand_off),
            diagnostics: output.diagnostics(),
        };
        match crate::transport::hand_exit_record(&exit) {
            Ok(true) => {
                let _native = native_return(&attachment);
                let error = match owned {
                    Some(owner) => crate::harness::agent_process::exec_native(
                        std::mem::replace(process, Command::new(program)),
                        owner,
                        launch.holder,
                        launch_lock,
                    ),
                    None => {
                        crate::transport::report_directory(&cwd);
                        crate::harness::agent_process::exec_client(
                            std::mem::replace(process, Command::new(program)),
                            launch_lock,
                        )
                    }
                };
                crate::transport::drop_exit_record();
                return Err(if error.kind() == std::io::ErrorKind::NotFound {
                    anyhow!(missing_agent_message(program))
                } else {
                    error.into()
                });
            }
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(%error, "execution owner did not take the provider wait; waiting here");
            }
        }
    }
    let reported_thread = &client.reported_thread;
    let foreground = crate::os_process::foreground::hand_to(process);
    let spawned = match owned {
        Some(owner) => crate::harness::agent_process::spawn_native(
            std::mem::replace(process, Command::new(program)),
            owner,
            launch.holder,
        ),
        None => {
            let Admitted {
                store,
                session,
                attachment: expected,
            } = &attachment;
            store
                .with_session_attachment(session, expected, || Ok(process.spawn()))?
                .map_err(Into::into)
        }
    };
    let mut child = spawned.map_err(|error| {
        if error
            .downcast_ref::<std::io::Error>()
            .is_some_and(|error| error.kind() == std::io::ErrorKind::NotFound)
        {
            anyhow!(missing_agent_message(program))
        } else {
            error
        }
    })?;

    if owned.is_none() {
        crate::transport::report_directory(&cwd);
    }
    if let Some(foreground) = &foreground {
        foreground.lead(child.id());
    }
    let _native = native_return(&attachment);
    // The process record now owns the launch identity.
    drop(launch_lock);
    let pid = child.id();
    // A client's conversation can move to another attachment; this client then
    // stops, so only one writes to it.
    let client = owned.is_none().then_some(pid);
    let mut checked = std::time::Instant::now();
    let status = crate::os_process::wait_for_exit(&mut child, None, || {
        if let Some(foreground) = &foreground {
            foreground.follow_stop(pid);
        }
        if let Some(title) = title {
            title.refresh();
        }
        let Some(pid) = client.filter(|_| checked.elapsed() >= CLIENT_ATTACHMENT_CHECK) else {
            return;
        };
        checked = std::time::Instant::now();
        if attachment_lost(&attachment).unwrap_or(false) {
            // SAFETY: kill reads no memory; `pid` is this process's unreaped child.
            unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) };
        }
    })
    .map(|result| result.0);
    let status = match status {
        Ok(status) => status,
        Err(error) => {
            crate::harness::agent_process::stop_native(&mut child, owned)?;
            return Err(error.into());
        }
    };
    if let Some(owner) = owned {
        if let Err(error) = crate::harness::native_exit::record_exit(owner) {
            tracing::warn!(%error, "cannot record the provider's exit");
        }
    }
    let interrupted = foreground
        .as_ref()
        .is_some_and(|foreground| foreground.interrupted(&status));
    // A mover ended this attachment; the conversation's home is now its business.
    let lost = attachment_lost(&attachment).unwrap_or_else(|error| {
        tracing::warn!(%error, "cannot read this launch's Session attachment");
        false
    });
    if let (Some(path), Some(capture_dir)) = (&reported_thread, capture_dir.as_deref()) {
        if let Err(error) = record_reported_thread(path, capture_dir) {
            tracing::warn!(%error, "Codex reported no conversation; it cannot be resumed");
        }
    }
    let moved = lost.then(|| departure(&attachment)).flatten();
    Ok(NativeOutcome {
        status,
        interrupted: interrupted || lost,
        moved,
    })
}

/// Record the thread a bare launch's start hook reported as the conversation
/// `lf resume` reopens.
fn record_reported_thread(path: &Path, capture_dir: &Path) -> Result<()> {
    let thread = std::fs::read_to_string(path)?;
    let _ = std::fs::remove_file(path);
    crate::session_record::write_provider_session(
        capture_dir,
        &AgentSessionId::from(thread.trim()),
        None,
    )?;
    Ok(())
}

/// How often a native client checks that its Session is still attached here.
const CLIENT_ATTACHMENT_CHECK: std::time::Duration = std::time::Duration::from_secs(2);

/// lf's line for leaving a native client. The client's own exit text names
/// commands outside lf. Leaving ends the invocation and its AgentProcess on
/// every harness; only detaching keeps it.
fn native_return(
    Admitted { store, session, .. }: &Admitted,
) -> Option<crate::transport::NativeClient> {
    let name = store
        .session(session)
        .ok()
        .flatten()
        .map(|session| session.title)
        .filter(|title| !title.is_empty())
        .map_or_else(
            || "the conversation".to_string(),
            |title| format!("“{title}”"),
        );
    let command = format!("lf resume {session}");
    crate::transport::native_client(
        format!("Left {name}. Continue with: {command}  (Ctrl+] steps away without stopping)"),
        format!("Detached from {name}; it keeps running. Return with: {command}"),
    )
}

#[cfg(test)]
mod tests {
    use crate::harness::Admitted;
    use crate::process::attachment;

    use super::{departure_notice, launch_session, resume_session_with_env};
    use super::{Launch, Mode, NativeClient, Start};
    use crate::agent::AgentConfig;
    use crate::provider_auth::Provider;
    use crate::session::Holder;
    use crate::store::{CredentialType, ProviderToken, StorageConfig};
    use std::collections::BTreeMap;
    use std::ffi::OsString;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::process::Command;

    #[test]
    fn native_resume_does_not_replace_a_work_read_failure_with_server_ownership() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let temp = tempfile::tempdir().unwrap();
        let database = temp.path().join("loopflow.db");
        ledger.set_db_path(database.clone());
        let _restore = EnvRestore::capture(&["LF_HOME"]);
        std::env::set_var("LF_HOME", temp.path());
        let capture = crate::session_record::SessionRecorder::begin_at(
            temp.path(),
            crate::session_record::SessionCaptureSpec {
                harness: "claude".into(),
                model: None,
                surface: "tui".into(),
                cwd: temp.path().to_path_buf(),
                repo: None,
                worktree: None,
                skill: None,
                subjects: Vec::new(),
                graph: crate::session_record::SessionGraphMembership::Independent,
                work: None,
            },
        )
        .unwrap();
        // Session and settings remain readable; only the Work lookup fails.
        let db = rusqlite::Connection::open(&database).unwrap();
        db.execute("UPDATE agent_sessions SET primary_scope='wave'", [])
            .unwrap();
        let before: i64 = db
            .query_row("SELECT count(*) FROM processes", [], |row| row.get(0))
            .unwrap();
        let error =
            resume_session_with_env(&capture.artifact_key(), None, &BTreeMap::new(), None, false)
                .unwrap_err();
        assert!(
            format!("{error:#}").contains("unsupported primary scope \"wave\""),
            "{error:#}"
        );
        let after: i64 = db
            .query_row("SELECT count(*) FROM processes", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            after, before,
            "a failed Work lookup must not admit a new invocation"
        );
    }

    #[test]
    fn a_terminal_move_names_the_terminal_it_went_to() {
        use crate::process::Departure;
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
        let notice = |to: Option<&str>| {
            departure_notice(
                &store,
                "s1",
                Departure::Terminal {
                    to: to.map(str::to_owned),
                },
            )
            .unwrap()
        };
        assert_eq!(
            notice(Some("Desktop")),
            "This conversation moved to Desktop: lf resume s1"
        );
        assert_eq!(
            notice(None),
            "This conversation moved to another terminal on this Machine."
        );
    }

    #[test]
    fn a_machine_move_names_the_task_that_went_with_it() {
        use crate::process::Departure;
        use loopflow_model::durable::{MachineId, TaskId};
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
        let repo = home.path().canonicalize().unwrap();
        let repo = repo.to_str().unwrap();
        let task = store
            .create_task(&loopflow_storage::work::task::NewTask {
                repository_id: crate::repository::location::ensure_repository(&store, repo)
                    .unwrap(),
                due_date: None,
                id: TaskId::new(),
                project_id: Some(
                    crate::work::wave::bootstrap::ensure_wave_project(&store, repo, "inbox")
                        .unwrap()
                        .id,
                ),
                title: "Move it".into(),
                description: String::new(),
            })
            .unwrap();
        let machine = MachineId::new();
        let notice = |task| {
            departure_notice(
                &store,
                "s1",
                Departure::Machine {
                    machine: machine.clone(),
                    task,
                },
            )
            .unwrap()
        };
        assert_eq!(
            notice(Some(task.id.clone())),
            format!(
                "This conversation moved to {machine}: lf resume --task {}",
                task.identifier
            )
        );
        assert_eq!(
            notice(None),
            format!("This conversation moved to {machine}: lf resume s1")
        );
    }

    struct EnvRestore(Vec<(&'static str, Option<OsString>)>);

    impl EnvRestore {
        fn capture(names: &[&'static str]) -> Self {
            Self(
                names
                    .iter()
                    .map(|name| (*name, std::env::var_os(name)))
                    .collect(),
            )
        }
    }

    impl Drop for EnvRestore {
        fn drop(&mut self) {
            for (name, value) in &self.0 {
                match value {
                    Some(value) => std::env::set_var(name, value),
                    None => std::env::remove_var(name),
                }
            }
        }
    }

    #[cfg(unix)]
    fn fake_provider(temp: &tempfile::TempDir, body: &str) -> PathBuf {
        let provider = temp.path().join("fake-provider");
        std::fs::write(
            &provider,
            format!("#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then exit 0; fi\n{body}\n"),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&provider).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&provider, permissions).unwrap();
        provider
    }

    fn native_fixture(
        program: &str,
        args: Vec<String>,
        cwd: &std::path::Path,
        environment: &BTreeMap<String, String>,
    ) -> anyhow::Result<super::NativeOutcome> {
        crate::journal::with_runtime(cwd, &["lf".into(), "fixture".into()], || {
            let mut environment = environment.clone();
            let (admitted, capture) = super::admit(program, cwd, &mut environment)?;
            let admitted = admitted.clone();
            let config = AgentConfig {
                agent: Some("claude".into()),
                cwd: Some(cwd.into()),
                env: environment.clone(),
                ..Default::default()
            };
            let launch = Launch {
                config: &config,
                mode: Mode::Interactive,
                holder: Holder::Caller,
                admitted: &admitted,
                start: Start::New,
                context_file: None,
            };
            let mut command = NativeClient::new("claude", &launch)?;
            command.process = Command::new(program);
            command
                .process
                .current_dir(cwd)
                .envs(&environment)
                .args(args);
            command.remote = None;
            let result = super::run_client(
                &launch,
                &mut command,
                None,
                capture.as_ref(),
                &mut crate::startup::Startup::new("fixture"),
            );
            super::finish_admission(&admitted, capture, &result);
            result
        })
    }

    /// A bare launch's provider failure, without a terminal handing it signals.
    fn bare_provider_error(body: &str) -> anyhow::Error {
        let ledger = crate::journal::TestLedgerGuard::new();
        let temp = tempfile::tempdir().unwrap();
        ledger.set_db_path(temp.path().join("loopflow.db"));
        let _home = EnvRestore::capture(&["LF_HOME"]);
        std::env::set_var("LF_HOME", temp.path());
        let provider = fake_provider(&temp, body);
        let capture = crate::session_record::SessionRecorder::begin_at(
            temp.path(),
            crate::session_record::SessionCaptureSpec {
                harness: "fake-provider".to_string(),
                model: None,
                surface: "tui".to_string(),
                cwd: temp.path().to_path_buf(),
                repo: None,
                worktree: None,
                skill: None,
                subjects: Vec::new(),
                graph: crate::session_record::SessionGraphMembership::Independent,
                work: None,
            },
        )
        .map(crate::process::LfProcessRuntime::new)
        .unwrap();
        native_fixture(
            &provider.display().to_string(),
            Vec::new(),
            temp.path(),
            &capture.environment(),
        )
        .and_then(|outcome| {
            outcome.result(crate::harness::HarnessKind::Claude, false, Holder::Caller)
        })
        .expect_err("a failed provider fails its launch")
    }

    fn exit_code(error: &anyhow::Error) -> u8 {
        error
            .downcast_ref::<crate::process::CommandExit>()
            .unwrap_or_else(|| panic!("not the provider's own status: {error:?}"))
            .0
    }

    #[cfg(unix)]
    #[test]
    fn provider_sigterm_without_stop_intent_remains_an_error() {
        assert_eq!(exit_code(&bare_provider_error("kill -TERM $$")), 143);
    }

    #[cfg(unix)]
    #[test]
    fn a_bare_launch_returns_its_providers_own_status() {
        let error = bare_provider_error("exit 7");
        assert_eq!(exit_code(&error), 7);
        assert!(
            !format!("{error:?}").contains("another client"),
            "{error:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn preferred_name_resume_uses_config_when_forwarded_name_is_empty() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let temp = tempfile::tempdir().unwrap();
        ledger.set_db_path(temp.path().join("loopflow.db"));
        let _restore = EnvRestore::capture(&["LF_HOME", "LF_USER_NAME", "PATH"]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::remove_var("LF_USER_NAME");
        let provider = fake_provider(&temp, "printf '%s\\0' \"$LF_USER_NAME\" \"$@\" > received");
        std::fs::rename(provider, temp.path().join("claude")).unwrap();
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            std::env::join_paths(
                std::iter::once(temp.path().to_path_buf()).chain(std::env::split_paths(&path)),
            )
            .unwrap(),
        );
        let capture = crate::session_record::SessionRecorder::begin_at(
            temp.path(),
            crate::session_record::SessionCaptureSpec {
                harness: "claude".into(),
                model: None,
                surface: "tui".into(),
                cwd: temp.path().to_path_buf(),
                repo: None,
                worktree: None,
                skill: None,
                subjects: Vec::new(),
                graph: crate::session_record::SessionGraphMembership::Independent,
                work: None,
            },
        )
        .map(crate::process::LfProcessRuntime::new)
        .unwrap();
        let capture_dir = capture.artifact_dir();
        crate::session_record::write_provider_session(&capture_dir, &"ses_original".into(), None)
            .unwrap();
        let session = crate::session_record::read_provider_session(&capture_dir)
            .unwrap()
            .unwrap();
        for (index, (saved, forwarded, expected)) in [
            ("Jack", None, Some("Jack")),
            ("Maya", None, Some("Maya")),
            ("Host Owner", Some("Jack"), Some("Jack")),
            ("Host Owner", Some(""), Some("Host Owner")),
        ]
        .into_iter()
        .enumerate()
        {
            std::fs::write(
                temp.path().join("config.yaml"),
                format!("user:\n  name: {saved}\n"),
            )
            .unwrap();
            match forwarded {
                Some(name) => std::env::set_var("LF_USER_NAME", name),
                None => std::env::remove_var("LF_USER_NAME"),
            }
            let resume = || {
                resume_session_with_env(
                    &capture.artifact_key(),
                    Some(&session),
                    &BTreeMap::new(),
                    None,
                    false,
                )
            };
            if index % 2 == 0 {
                crate::journal::with_runtime(temp.path(), &["lf".into(), "resume".into()], resume)
            } else {
                resume()
            }
            .unwrap();
            // Both entry paths reuse the saved conversation and reserve exactly
            // one provider under the admitted lf invocation.
            let store = crate::store::open_sqlite(&temp.path().join("loopflow.db")).unwrap();
            let rows = store.processes_since(0).unwrap();
            let agents = rows
                .iter()
                .filter_map(crate::process::ProcessEntry::as_agent)
                .collect::<Vec<_>>();
            let saved = store
                .session_for_artifact(&capture.artifact_key())
                .unwrap()
                .unwrap();
            assert_eq!(agents.len(), index + 1);
            assert!(agents
                .iter()
                .all(|agent| agent.agent_session_id.as_deref() == Some(&saved.id)
                    && agent.completed_at.is_some()));
            assert_eq!(
                rows.iter()
                    .filter_map(crate::process::ProcessEntry::as_lf)
                    .count(),
                index + 1
            );
            let attachment = store.session_attachment(&saved.id).unwrap().unwrap();
            assert!(attachment.lf_process_id.is_none());
            assert!(rows
                .iter()
                .filter_map(crate::process::ProcessEntry::as_lf)
                .any(|row| row.id == attachment.provider_lf_process_id
                    && row.pid == Some(std::process::id())
                    && row.completed_at.is_some()));
            let received = std::fs::read_to_string(temp.path().join("received")).unwrap();
            let arguments = received.split('\0').collect::<Vec<_>>();
            assert_eq!(arguments[0], expected.unwrap_or_default());
            assert!(arguments.contains(&"ses_original"));
            assert!(!arguments.contains(&"--prompt"));
            assert!(!received.contains("<lf:user>"));
            assert_eq!(
                crate::session_record::read_provider_session(&capture_dir)
                    .unwrap()
                    .unwrap(),
                session
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn bare_codex_resume_does_not_resend_its_first_turn() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let temp = tempfile::tempdir().unwrap();
        ledger.set_db_path(temp.path().join("loopflow.db"));
        let _restore = EnvRestore::capture(&["LF_HOME", "PATH"]);
        std::env::set_var("LF_HOME", temp.path());
        let provider = fake_provider(&temp, "printf '%s\\0' \"$@\" > received");
        std::fs::rename(provider, temp.path().join("codex")).unwrap();
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            std::env::join_paths(
                std::iter::once(temp.path().to_path_buf()).chain(std::env::split_paths(&path)),
            )
            .unwrap(),
        );
        let request = crate::session_record::AgentProcessRequest::from_prepared(&AgentConfig {
            agent: Some("codex".into()),
            task_prompt: "the first turn".into(),
            ..Default::default()
        })
        .unwrap();
        let capture = crate::session_record::SessionRecorder::begin_at_with_request(
            temp.path(),
            crate::session_record::SessionCaptureSpec {
                harness: "codex".into(),
                model: None,
                surface: "tui".into(),
                cwd: temp.path().to_path_buf(),
                repo: None,
                worktree: None,
                skill: None,
                subjects: Vec::new(),
                graph: crate::session_record::SessionGraphMembership::Independent,
                work: None,
            },
            request,
        )
        .unwrap();
        let capture_dir = capture.artifact_dir();
        crate::session_record::write_provider_session(&capture_dir, &"thread-1".into(), None)
            .unwrap();
        let session = crate::session_record::read_provider_session(&capture_dir)
            .unwrap()
            .unwrap();

        resume_session_with_env(
            &capture.artifact_key(),
            Some(&session),
            &BTreeMap::new(),
            None,
            false,
        )
        .unwrap();

        let received = std::fs::read_to_string(temp.path().join("received")).unwrap();
        let arguments = received.split_terminator('\0').collect::<Vec<_>>();
        assert!(arguments.ends_with(&["resume", "--", "thread-1"]));
        assert!(!received.contains("the first turn"));
    }

    #[test]
    fn recorded_native_move_returns_cleanly_without_stop_receipts() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let home = ledger.home();
        let store = crate::store::open_sqlite(&home.join("loopflow.db")).unwrap();
        let stop = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
            loop {
                if let Some(agent) = store
                    .agent_processes()
                    .unwrap()
                    .into_iter()
                    .find(|agent| agent.process.pid.is_some())
                {
                    return crate::harness::agent_process::stop_session_agent(
                        &store,
                        agent.process.agent_session_id.as_deref().unwrap(),
                        crate::process::Departure::Terminal { to: None },
                    );
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "native launch was not recorded"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });
        let result = native_fixture("/bin/sleep", vec!["10".into()], home, &BTreeMap::new())
            .and_then(|outcome| {
                outcome.result(crate::harness::HarnessKind::Claude, false, Holder::Caller)
            });
        stop.join().unwrap().unwrap();
        result.unwrap();
        assert!(!home.join("provider-client-stops").exists());
    }

    #[test]
    fn native_helper_admits_its_invocation_and_records_spawn_failure() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let home = ledger.home();
        assert!(native_fixture(
            "/bin/sh",
            vec!["-c".into(), "printf '%s' \"$$\" > pid".into()],
            home,
            &BTreeMap::new()
        )
        .unwrap()
        .status
        .success());
        let pid: u32 = std::fs::read_to_string(home.join("pid"))
            .unwrap()
            .parse()
            .unwrap();
        assert!(native_fixture(
            &home.join("absent").display().to_string(),
            vec![],
            home,
            &BTreeMap::new()
        )
        .unwrap_err()
        .to_string()
        .contains("not installed"));
        let store = crate::store::open_sqlite(&home.join("loopflow.db")).unwrap();
        let rows = store.processes_since(0).unwrap();
        let agents = rows
            .iter()
            .filter_map(crate::process::ProcessEntry::as_agent)
            .collect::<Vec<_>>();
        assert_eq!(agents.len(), 2);
        assert!(agents
            .iter()
            .any(|row| row.pid == Some(pid) && row.completed_at.is_some()));
        assert!(agents.iter().all(|row| row.completed_at.is_some()));
        let sql = rusqlite::Connection::open(home.join("loopflow.db")).unwrap();
        let failed: i64 = sql
            .query_row(
                "SELECT COUNT(*) FROM processes WHERE kind='agent' AND spawn_state='spawn_failed'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(failed, 1);
        for agent in agents {
            let parent = store
                .lf_process(agent.parent_lf_process_id.as_ref().unwrap())
                .unwrap()
                .unwrap();
            assert_eq!(parent.pid, Some(std::process::id()));
            assert!(parent.completed_at.is_some());
            assert!(agent.agent_session_id.is_some());
        }
    }

    #[test]
    fn native_remote_client_preserves_the_provider_and_requires_explicit_endpoint() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let home = ledger.home();
        crate::journal::with_runtime(home, &["lf".into(), "session".into()], || {
            let store = crate::store::open_sqlite(&home.join("loopflow.db"))?;
            let session = store.test_session("remote", &crate::session_record::new_artifact_key());
            let original = attachment::claim_session_attachment(
                &store,
                &session.id,
                None,
                &store.test_lf_process(),
                true,
            )?;
            let mut command = Command::new("/bin/sleep");
            command.env_clear().arg("60");
            let mut provider = crate::harness::agent_process::spawn_native(
                command,
                &crate::harness::Admitted {
                    store: store.clone(),
                    session: session.id.to_string(),
                    attachment: original.clone(),
                },
                Holder::Caller,
            )?;
            let endpoint = home.join("provider.sock");
            store.record_session_connection(
                &session.id,
                &original,
                endpoint.to_str().unwrap(),
                &"thread".into(),
            )?;
            let attached = attachment::claim_session_attachment(
                &store,
                &session.id,
                Some(&original),
                &crate::journal::current_lf_process_id().unwrap(),
                false,
            )?;
            let environment = BTreeMap::from([(
                crate::process::AGENT_CALLER_ENV.into(),
                serde_json::to_string(&attached.caller(session.id.clone()))?,
            )]);
            let before = store.agent_process(&attached.agent_process_id)?.unwrap();
            let admitted = Admitted {
                store: store.clone(),
                session: session.id.to_string(),
                attachment: attached.clone(),
            };
            let config = AgentConfig {
                agent: Some("claude".into()),
                cwd: Some(home.into()),
                env: environment.clone(),
                ..Default::default()
            };
            let launch = Launch {
                config: &config,
                mode: Mode::Interactive,
                holder: Holder::Caller,
                admitted: &admitted,
                start: Start::New,
                context_file: None,
            };
            let mut command = NativeClient::new("claude", &launch)?;
            command.process = Command::new("/usr/bin/true");
            command.process.current_dir(home).envs(&environment);
            command.remote = Some(endpoint.clone());
            let result = super::run_client(
                &launch,
                &mut command,
                None,
                None,
                &mut crate::startup::Startup::new("fixture"),
            );
            let alive = provider.try_wait()?.is_none();
            let unchanged = store.agent_process(&attached.agent_process_id)? == Some(before);
            command.remote = None;
            let local = super::run_client(
                &launch,
                &mut command,
                None,
                None,
                &mut crate::startup::Startup::new("fixture"),
            );
            command.remote = Some(home.join("other.sock"));
            let wrong_endpoint = super::run_client(
                &launch,
                &mut command,
                None,
                None,
                &mut crate::startup::Startup::new("fixture"),
            );
            crate::harness::agent_process::stop_native(
                &mut provider,
                Some(&crate::harness::Admitted {
                    store,
                    session: session.id.to_string(),
                    attachment: attached,
                }),
            )?;
            assert!(result?.status.success());
            assert!(alive && unchanged);
            assert!(local.unwrap_err().to_string().contains("does not own"));
            assert!(wrong_endpoint
                .unwrap_err()
                .to_string()
                .contains("endpoint differs"));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn native_launch_setup_errors_retain_the_error_and_observed_process_exit() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let home = ledger.home();
        crate::journal::with_runtime(home, &["lf".into(), "skill".into()], || {
            let capture = crate::session_record::SessionRecorder::begin_at(
                home,
                crate::session_record::SessionCaptureSpec {
                    harness: "fixture".into(),
                    model: None,
                    surface: "tui".into(),
                    cwd: home.into(),
                    repo: None,
                    worktree: None,
                    skill: None,
                    subjects: Vec::new(),
                    graph: crate::session_record::SessionGraphMembership::Independent,
                    work: None,
                },
            )
            .map(crate::process::LfProcessRuntime::new)?;
            capture.claim_session_attachment()?;
            let (session, attachment) = capture.session_attachment().unwrap();
            let store = crate::store::open_sqlite(&home.join("loopflow.db"))?;
            let process = crate::journal::current_lf_process_id().unwrap();
            store.retain_session_observation(
                &store.session(&session)?.unwrap(),
                &crate::session::SessionObservation {
                    artifact_key: crate::session_record::parse_artifact_key(
                        &capture.artifact_key(),
                    )?,
                    source: format!("interactive_opened:{process}:{session}"),
                    observed_at: 1,
                    task_id: None,
                    wave_id: None,
                    payload: serde_json::json!({"type": "fixture conflict"}),
                },
            )?;
            let environment = capture.environment();
            let admitted = Admitted::from_runtime(&capture)?;
            let config = AgentConfig {
                agent: Some("claude".into()),
                cwd: Some(home.into()),
                env: environment.clone(),
                ..Default::default()
            };
            let launch = Launch {
                config: &config,
                mode: Mode::Interactive,
                holder: Holder::Caller,
                admitted: &admitted,
                start: Start::New,
                context_file: None,
            };
            let mut command = NativeClient::new("claude", &launch)?;
            command.process = Command::new("/bin/sleep");
            command
                .process
                .current_dir(home)
                .envs(&environment)
                .arg("60");
            command.remote = None;
            let error = super::run_client(
                &launch,
                &mut command,
                None,
                None,
                &mut crate::startup::Startup::new("fixture"),
            )
            .unwrap_err();
            assert!(
                error.to_string().contains("conflicting interactive_opened"),
                "{error:#}"
            );
            // The conflict is found before any provider runs.
            let agent = store.agent_process(&attachment.agent_process_id)?;
            assert!(
                agent.as_ref().and_then(|process| process.pid).is_none(),
                "a provider was launched despite the conflict: {agent:?}"
            );
            Ok(())
        })
        .unwrap();
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn session_launch_tui_preserves_native_oauth_and_routes_stored_api_keys() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let temp = tempfile::tempdir().unwrap();
        ledger.set_db_path(temp.path().join("loopflow.db"));
        let _restore = EnvRestore::capture(&[
            "LF_HOME",
            "LF_TEST_SESSION_ENV",
            "OPENCODE_API_KEY",
            "CODEX_ACCESS_TOKEN",
            "CODEX_HOME",
            "PATH",
        ]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::set_var("LF_BIN", std::env::current_exe().unwrap());
        std::env::set_var("CODEX_HOME", temp.path().join("codex-home"));
        std::env::set_var("OPENCODE_API_KEY", "ambient-key");
        std::env::remove_var("CODEX_ACCESS_TOKEN");

        let bin = temp.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let opencode = bin.join("opencode");
        std::fs::write(
            &opencode,
            include_str!("../../tests/support/opencode_server.py").replace("__WAIT__", "False"),
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(&opencode).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&opencode, permissions).unwrap();
        }
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(&path))).unwrap(),
        );
        let capture = temp.path().join("session-env");
        std::env::set_var("LF_TEST_SESSION_ENV", &capture);

        let store = crate::store::open_ephemeral_store(&StorageConfig::sqlite(
            temp.path().join("loopflow.db"),
        ))
        .await
        .unwrap();
        store
            .upsert_provider_token(&ProviderToken {
                provider: Provider::OpenCodeZen.as_str().to_string(),
                access_token: "stored-key".to_string(),
                refresh_token: None,
                oauth_client_id: None,
                expires_at: None,
                login: Some("zen@example.com".to_string()),
                updated_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                credential_type: CredentialType::ApiKey,
            })
            .await
            .unwrap();

        launch_session(
            &AgentConfig {
                agent: Some("opencode".to_string()),
                cwd: Some(temp.path().to_path_buf()),
                task_prompt: "review it".into(),
                env: BTreeMap::new(),
                ..Default::default()
            },
            Holder::Caller,
            None,
            None,
        )
        .unwrap();

        assert_eq!(std::fs::read_to_string(capture).unwrap(), "stored-key");

        // The native CLI rejects an ordinary OAuth token in this agent-identity
        // variable. A prior `lf account` must not poison a working login.
        let codex = temp.path().join("bin/codex");
        std::fs::write(
            &codex,
            "#!/bin/sh\nif [ \"${CODEX_ACCESS_TOKEN+x}\" = x ]; then exit 1; fi\n",
        )
        .unwrap();
        std::fs::set_permissions(&codex, std::fs::metadata(&opencode).unwrap().permissions())
            .unwrap();
        store
            .upsert_provider_token(&ProviderToken {
                provider: Provider::Codex.as_str().to_string(),
                access_token: "ordinary-chatgpt-oauth".to_string(),
                refresh_token: None,
                oauth_client_id: None,
                expires_at: None,
                login: Some("codex@example.com".to_string()),
                updated_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                credential_type: CredentialType::OAuth,
            })
            .await
            .unwrap();
        let mut command = Command::new(&codex);
        crate::provider_auth::apply_provider_env_to_command("codex", &mut command);
        assert!(command.status().unwrap().success());
    }
}
