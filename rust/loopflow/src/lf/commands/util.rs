use anyhow::{anyhow, bail, Context, Result};
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};

use crate::agent::missing_agent_message;
use crate::agent::{
    build_session_command, session_environment, AgentConfig, NativeConnection, ProviderInvocation,
    SessionCommand,
};
use crate::id::AgentSessionId;
use crate::provider_auth::Provider;
use crate::store::sqlite::SqliteStore;

pub fn find_repo_root() -> Result<PathBuf> {
    crate::repo::find_repo_root()
}

pub(crate) fn parse_since(value: &str, now: OffsetDateTime) -> Result<OffsetDateTime> {
    if let Ok(timestamp) = OffsetDateTime::parse(value, &Rfc3339) {
        return Ok(timestamp);
    }
    let (amount, unit) = value.split_at(value.len().saturating_sub(1));
    let amount: i64 = amount
        .parse()
        .map_err(|_| anyhow!("invalid --since '{value}'; use 7d, 24h, 30m, or RFC3339"))?;
    if amount < 0 {
        return Err(anyhow!("--since duration must be non-negative"));
    }
    let seconds_per_unit = match unit {
        "d" => 86_400,
        "h" => 3_600,
        "m" => 60,
        _ => {
            return Err(anyhow!(
                "invalid --since '{value}'; use 7d, 24h, 30m, or RFC3339"
            ));
        }
    };
    let seconds = amount
        .checked_mul(seconds_per_unit)
        .ok_or_else(|| anyhow!("--since duration is too large"))?;
    now.checked_sub(Duration::seconds(seconds))
        .ok_or_else(|| anyhow!("--since duration is too large"))
}

pub(crate) fn launch_session(
    config: &AgentConfig,
    agent_session: Option<&AgentSessionId>,
    context_file: Option<&Path>,
) -> Result<()> {
    let command = build_session_command(
        config,
        ProviderInvocation::Terminal {
            session_id: agent_session,
        },
        context_file,
    )?;
    let environment = session_environment(config, context_file)?;
    spawn_session_command_with_env(
        &command,
        &environment,
        agent_session,
        config.provider_account_id.as_ref(),
        None,
    )
}

pub(crate) fn resume_session(
    artifact_key: &String,
    provider_session: &crate::session_record::ProviderSessionRef,
) -> Result<()> {
    resume_session_with_env(artifact_key, Some(provider_session), &BTreeMap::new(), None)
}

pub(crate) fn resume_session_with_env(
    artifact_key: &String,
    provider_session: Option<&crate::session_record::ProviderSessionRef>,
    extra_environment: &BTreeMap<String, String>,
    launch_lock: Option<File>,
) -> Result<()> {
    let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
    let session = store
        .session_for_artifact(artifact_key)?
        .ok_or_else(|| anyhow!("Session input {artifact_key} is not recorded"))?;
    let mut config = crate::agent::resolve_session_config(&session, &crate::lf::Cli::default())?;
    anyhow::ensure!(
        session.provider.as_deref() == Some(crate::config::parse_agent(config.agent()).0.as_str()),
        "native history cannot resume in {}; continue it with lf resume -a {}",
        config.agent(),
        config.agent(),
    );
    if let Some(provider) = provider_session {
        config.task_prompt.clear();
        config.provider_account_id = provider.account_id.clone().or(config.provider_account_id);
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
    let command = build_session_command(
        &config,
        match provider_session {
            Some(provider) => ProviderInvocation::Resume {
                session_id: &provider.agent_session,
            },
            None => ProviderInvocation::Terminal { session_id: None },
        },
        context_file.as_deref(),
    )?;
    let environment = session_environment(&config, context_file.as_deref())?;
    spawn_session_command_with_env(
        &command,
        &environment,
        provider_session.map(|provider| &provider.agent_session),
        config.provider_account_id.as_ref(),
        launch_lock,
    )
}

pub(crate) fn require_provider_session_process(dir: &Path) -> Result<()> {
    let input = crate::session_record::input_id_from_dir(dir)?;
    let store = SqliteStore::open_processes_read_only(&crate::store::database_path_from_env()?)?;
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

fn spawn_session_command_with_env(
    command: &SessionCommand,
    environment: &BTreeMap<String, String>,
    agent_session: Option<&AgentSessionId>,
    exact_account_id: Option<&crate::store::ProviderAccountId>,
    launch_lock: Option<File>,
) -> Result<()> {
    let _planning_sync =
        crate::ops::planning_sync::PlanningSync::start_for_directory(&command.cwd)?;
    let outcome = session_command_status_with_env(
        command,
        environment,
        agent_session,
        exact_account_id,
        launch_lock,
    )?;
    if outcome.status.success() || outcome.interrupted {
        Ok(())
    } else if agent_session.is_some() {
        Err(anyhow!(
            "{} could not open this session (status {}). If another client still owns it, close that client or use `lf session connect --takeover` for a Loopflow-owned client.",
            command.program,
            outcome.status,
        ))
    } else {
        Err(anyhow!(
            "session launcher exited with status {}",
            outcome.status
        ))
    }
}

#[derive(Debug)]
struct SessionCommandOutcome {
    status: std::process::ExitStatus,
    interrupted: bool,
}

fn record_interactive_opened(environment: &BTreeMap<String, String>) -> Result<()> {
    let Some(input) = environment.get(crate::session_record::CAPTURE_KEY_ENV) else {
        return Ok(());
    };
    let Some(process) = crate::journal::current_lf_process_id() else {
        return Ok(());
    };
    let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
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

fn native_provider_attachment(
    command: &SessionCommand,
    environment: &BTreeMap<String, String>,
) -> Result<crate::harness::agent_process::AttachmentOwner> {
    let (session, attachment) = command
        .attachment
        .as_ref()
        .ok_or_else(|| anyhow!("Native launch has no admitted attachment"))?;
    let caller = environment
        .get(crate::process::AGENT_CALLER_ENV)
        .ok_or_else(|| anyhow!("Native launch has no admitted conversation"))?;
    let caller: crate::process::AgentCaller = serde_json::from_str(caller)?;
    let process = crate::journal::current_lf_process_id()
        .ok_or_else(|| anyhow!("Native launch has no admitted invocation"))?;
    if attachment.lf_process_id.as_ref() != Some(&process)
        || attachment.caller(session.clone()) != caller
    {
        bail!("Session attachment differs from native launch provenance");
    }
    let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
    store.with_session_attachment(session, attachment, || {
        if let Some(remote) = &command.remote {
            let endpoint = store
                .agent_endpoint(&attachment.agent_process_id)?
                .ok_or_else(|| {
                    crate::store::StoreError::InvalidAuthority(
                        "Remote client has no recorded AgentProcess endpoint".into(),
                    )
                })?;
            if Path::new(&endpoint) != remote.upstream {
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
    Ok((store, session.clone(), attachment.clone()))
}

fn session_command_status_with_env(
    command: &SessionCommand,
    environment: &BTreeMap<String, String>,
    agent_session: Option<&AgentSessionId>,
    exact_account_id: Option<&crate::store::ProviderAccountId>,
    launch_lock: Option<File>,
) -> Result<SessionCommandOutcome> {
    let mut progress = crate::startup::Startup::new(format!("Preparing {}", command.program));
    let argv = std::env::args().collect::<Vec<_>>();
    crate::journal::with_runtime(&command.cwd, &argv, || {
        let mut command = command.clone();
        let mut environment = environment.clone();
        let mut capture = None;
        let admitted = if command.attachment.is_none() {
            if environment.contains_key(crate::process::AGENT_CALLER_ENV) {
                bail!("Native launch provenance has no admitted attachment");
            }
            if command.remote.is_some() {
                bail!("Remote client has no admitted attachment");
            }
            if !environment.contains_key(crate::session_record::CAPTURE_KEY_ENV) {
                let context = crate::trace::PreparedTurnContext::from_prompts("", "");
                let created = crate::session_record::CaptureHandle::begin_with_context(
                    crate::session_record::SessionCaptureSpec {
                        harness: command.program.clone(),
                        model: None,
                        surface: "tui".into(),
                        cwd: command.cwd.clone(),
                        repo: None,
                        worktree: Some(command.cwd.clone()),
                        skill: None,
                        subjects: Vec::new(),
                        flow: crate::session_record::SessionFlowMembership::Independent,
                        work: None,
                    },
                    &context,
                    None,
                )?;
                environment.extend(created.environment());
                capture = Some(created);
            }
            let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
            let input = environment
                .get(crate::session_record::CAPTURE_KEY_ENV)
                .expect("native launch has a capture");
            let session = store
                .session_for_artifact(input)?
                .ok_or_else(|| anyhow!("Session input {input} is not recorded"))?;
            let process = crate::journal::current_lf_process_id()
                .ok_or_else(|| anyhow!("Native launch has no admitted invocation"))?;
            let attachment = crate::session_record::resume_session_agent_process(
                &store,
                &session.id.to_string(),
                &process,
            )?;
            store.make_session_interactive(&session.id.to_string(), &attachment)?;
            if let Some(owner) = crate::transport::current() {
                store.bind_session_transport(&session.id.to_string(), &attachment, owner)?;
            }
            environment.insert(
                crate::process::AGENT_CALLER_ENV.into(),
                serde_json::to_string(&attachment.caller(session.id.to_string()))?,
            );
            crate::session_record::register_session_attachment_interrupt(
                &store,
                session.id.to_string(),
                attachment.clone(),
            );
            command.attachment = Some((session.id.to_string(), attachment.clone()));
            Some((store, session.id.to_string(), attachment))
        } else {
            None
        };
        let result = run_native_session(
            &command,
            &environment,
            agent_session,
            exact_account_id,
            launch_lock,
            &mut progress,
        );
        let outcome = if result
            .as_ref()
            .is_ok_and(|outcome| outcome.interrupted || outcome.status.success())
        {
            "completed"
        } else {
            "failed"
        };
        if let Some((store, session, attachment)) = admitted {
            match crate::session_record::finish_session_attachment(
                &store,
                &session,
                &attachment,
                outcome,
            ) {
                Ok(()) | Err(crate::store::StoreError::InvalidAuthority(_)) => {}
                Err(error) => return Err(error.into()),
            }
        }
        if let Some(capture) = capture {
            capture.finish(outcome)?;
        }
        result
    })
}

fn run_native_session(
    command: &SessionCommand,
    environment: &BTreeMap<String, String>,
    agent_session: Option<&AgentSessionId>,
    exact_account_id: Option<&crate::store::ProviderAccountId>,
    launch_lock: Option<File>,
    progress: &mut crate::startup::Startup,
) -> Result<SessionCommandOutcome> {
    let started = std::time::Instant::now();
    let attachment = native_provider_attachment(command, environment)?;
    let capture_dir = environment
        .get(crate::session_record::CAPTURE_KEY_ENV)
        .map(|key| crate::session_record::capture_dir(key))
        .transpose()?;
    if let Some(dir) = &capture_dir {
        require_provider_session_process(dir)?;
    }
    attachment.0.record_session_terminal(
        &attachment.1,
        &attachment.2,
        crate::session_record::current_terminal_id().as_deref(),
    )?;
    if command.program == "codex" && command.remote.is_none() {
        return run_codex_native(
            command,
            environment,
            agent_session,
            exact_account_id,
            launch_lock,
            progress,
            attachment,
        );
    }
    let provider = match command.program.as_str() {
        "claude" => Some(Provider::Claude),
        "codex" => Some(Provider::Codex),
        _ => None,
    };
    let account_route = provider
        .map(|provider| {
            crate::provider_account::resolve_provider_account_exact_blocking(
                provider,
                agent_session.cloned(),
                exact_account_id.cloned(),
            )
        })
        .transpose()
        .map_err(|error| anyhow!("failed to select provider account: {error}"))?
        .flatten();

    progress.stage(format!("Opening {} terminal", command.program));
    progress.finish();
    let mut process = Command::new(&command.program);
    process
        .env_remove("LOOPFLOW_DIRECTIVE_FILE")
        .envs(environment);
    let mut title =
        crate::terminal_title::TerminalTitle::prepare(environment, &command.program, &mut process);
    if let Some(route) = &account_route {
        process.args(route.provider_args());
    }
    let opencode_capture = capture_dir
        .as_ref()
        .filter(|_| command.program == "opencode")
        .map(|dir| {
            let path = dir.join("opencode.stderr.log");
            let mut options = OpenOptions::new();
            options.create(true).append(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let file = options
                .open(&path)
                .with_context(|| format!("open OpenCode diagnostics at {}", path.display()))?;
            eprintln!("OpenCode diagnostics: {}", path.display());
            process.args(["--print-logs", "--log-level", "INFO"]);
            process.stderr(Stdio::piped());
            Ok::<_, anyhow::Error>((dir.clone(), file))
        })
        .transpose()?;
    process.current_dir(&command.cwd);
    crate::provider_auth::apply_provider_env_to_command(&command.program, &mut process);
    let mut activation = None;
    if let Some(route) = &account_route {
        tracing::info!(provider = %command.program, "selected managed provider account");
        activation = route
            .launch_as_blocking(&mut process)
            .map_err(|error| anyhow!("failed to activate provider account: {error}"))?;
        process.env(
            crate::session_record::PROVIDER_ACCOUNT_ID_ENV,
            route.account_id().as_str(),
        );
        route.pin_session_blocking(agent_session.cloned())?;
    }
    process.env_remove(crate::context_block::CONTEXT_DELIVERY_ENV);
    if let Some(context) = &command.context {
        context.configure_hooks(&mut process)?;
    }
    process.args(crate::harness::context::native_args(
        &command.program,
        command.context.as_ref(),
    )?);
    process.args(&command.args);
    if command.program == "claude" {
        if let Some(input) = &command.input {
            crate::agent::configure_native_input(&mut process, input)?;
        }
    }
    if let (Some(capture_dir), Some(agent_session)) = (capture_dir.as_deref(), agent_session) {
        crate::session_record::write_provider_session(
            capture_dir,
            agent_session,
            account_route
                .as_ref()
                .map(|route| route.account_id().clone()),
        )?;
    }
    // Only an explicit remote connection is a client of a surviving AgentProcess.
    let owned = command.remote.is_none().then_some(&attachment);
    tracing::debug!(
        elapsed_ms = started.elapsed().as_millis(),
        "prepared native provider launch"
    );
    let spawned = match owned {
        Some(owner) => crate::harness::agent_process::spawn_native(process, owner),
        None => {
            let (store, session, expected) = &attachment;
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
            anyhow!(missing_agent_message(&command.program))
        } else {
            error
        }
    })?;

    drop(activation);
    let setup = record_interactive_opened(environment);
    if let Err(error) = setup {
        if let Err(cleanup) = crate::harness::agent_process::stop_native(&mut child, owned) {
            tracing::warn!(%cleanup, "native setup cleanup remains unresolved");
        }
        return Err(error);
    }
    // The process record now owns the launch identity.
    drop(launch_lock);
    let observer = opencode_capture.map(|(capture_dir, diagnostics)| {
        let stderr = child
            .stderr
            .take()
            .expect("piped OpenCode stderr is available");
        let discover_session = agent_session.is_none();
        std::thread::spawn(move || {
            observe_opencode_session(&capture_dir, stderr, diagnostics, discover_session)
                .with_context(|| {
                    format!(
                        "OpenCode diagnostics: {}",
                        capture_dir.join("opencode.stderr.log").display()
                    )
                })
        })
    });
    let status = if let Some(title) = &mut title {
        crate::os_process::wait_for_exit(&mut child, None, || title.refresh())?.0
    } else {
        child.wait()?
    };
    let interrupted = if let Some((store, session, attachment)) = owned {
        match store.with_session_attachment(session, attachment, || {
            store.record_agent_process_exit(session, attachment, true)
        }) {
            Ok(()) | Err(crate::store::StoreError::InvalidAuthority(_)) => {}
            Err(error) => return Err(error.into()),
        }
        store
            .process(&attachment.agent_process_id)?
            .is_some_and(|process| {
                process.completed_at.is_some() && process.outcome.as_deref() == Some("interrupted")
            })
    } else {
        false
    };
    if let Some(observer) = observer {
        observer
            .join()
            .map_err(|_| anyhow!("OpenCode session observer panicked"))??;
    }
    // Record which home this conversation lives in, so reopening returns there.
    if let (Some(route), Some(capture_dir)) = (&account_route, capture_dir.as_deref()) {
        if let Ok(Some(session)) = crate::session_record::read_provider_session(capture_dir) {
            if let Err(error) = route.pin_session_blocking(Some(session.agent_session)) {
                tracing::warn!(%error, "failed to record the provider session's account home");
            }
        }
    }
    Ok(SessionCommandOutcome {
        status,
        interrupted,
    })
}

pub(crate) fn run_native_codex_agent(
    launch: &crate::agent::AgentConfig,
) -> Result<crate::agent::AgentProcessResult> {
    let command = build_session_command(
        launch,
        match launch.resume_token.as_ref() {
            Some(session_id) => ProviderInvocation::Resume { session_id },
            None => ProviderInvocation::Terminal { session_id: None },
        },
        None,
    )?;
    let outcome = session_command_status_with_env(
        &command,
        &launch.env,
        launch.resume_token.as_ref(),
        launch.provider_account_id.as_ref(),
        None,
    )?;
    let agent_session =
        launch
            .env
            .get(crate::session_record::CAPTURE_KEY_ENV)
            .map(|input| -> Result<_> {
                Ok(crate::session_record::read_provider_session(
                    &crate::session_record::capture_dir(input)?,
                )?
                .map(|provider| provider.agent_session))
            })
            .transpose()?
            .flatten();
    Ok(crate::agent::AgentProcessResult {
        exit_code: if outcome.interrupted {
            0
        } else {
            outcome.status.code().unwrap_or(1)
        },
        stdout: String::new(),
        stderr: String::new(),
        agent_session,
        failure: None,
    })
}

fn run_codex_native(
    command: &SessionCommand,
    environment: &BTreeMap<String, String>,
    agent_session: Option<&AgentSessionId>,
    exact_account_id: Option<&crate::store::ProviderAccountId>,
    launch_lock: Option<File>,
    progress: &mut crate::startup::Startup,
    owner: crate::harness::agent_process::AttachmentOwner,
) -> Result<SessionCommandOutcome> {
    progress.stage("Starting Codex");
    let mut launch = command
        .codex
        .as_ref()
        .expect("native Codex command carries its launch configuration")
        .clone();
    launch.env = environment.clone();
    launch.session_attachment = command.attachment.clone();
    launch.conversation_context = command.context.clone();
    launch.resume_token = agent_session.cloned();
    launch.provider_account_id = exact_account_id.cloned();
    if let Some(skill) = launch.skill_invocation.take() {
        launch.task_prompt = skill.terminal_input("codex", &launch)?.1;
    }
    let (ready_tx, ready_rx) = std::sync::mpsc::sync_channel(1);
    let (closed_tx, closed_rx) = tokio::sync::oneshot::channel();

    // Keep the terminal on its invoking thread and foreground process group.
    // The protocol worker owns only startup and the fenced native relay.
    std::thread::scope(|scope| {
        let worker = scope.spawn(move || -> Result<()> {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            runtime.block_on(serve_codex_native(launch, owner, ready_tx, closed_rx))
        });
        let prepared = ready_rx.recv();
        let result = match prepared {
            Ok((thread, remote, account)) => {
                let client = crate::agent::native_connection_command(
                    command.cwd.clone(),
                    thread.as_ref(),
                    remote,
                    command.attachment.clone(),
                );
                run_native_session(
                    &client,
                    environment,
                    thread.as_ref(),
                    account.as_ref(),
                    launch_lock,
                    progress,
                )
            }
            Err(_) => Err(anyhow!(
                "Codex startup worker ended before the terminal could attach"
            )),
        };
        let _ = closed_tx.send(());
        worker
            .join()
            .map_err(|_| anyhow!("Codex startup worker panicked"))??;
        result
    })
}

async fn serve_codex_native(
    launch: crate::agent::AgentConfig,
    owner: crate::harness::agent_process::AttachmentOwner,
    ready_tx: std::sync::mpsc::SyncSender<(
        Option<AgentSessionId>,
        NativeConnection,
        Option<crate::store::ProviderAccountId>,
    )>,
    closed_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<()> {
    use crate::harness::{
        codex::CodexHarness, codex_connection::CodexConnection, ApprovalPolicy, Harness,
    };

    let (events, _events) = tokio::sync::mpsc::unbounded_channel();
    let mut harness = CodexHarness::new(events, ApprovalPolicy::NativeClient);
    harness.set_agent_session(launch.resume_token.clone());
    harness.set_provider_account_id(launch.provider_account_id.clone());
    // No turn is sent on this bootstrap connection. Close it before
    // the UI connects so it cannot answer the UI's approvals.
    match tokio::time::timeout(std::time::Duration::from_secs(20), harness.start(&launch)).await {
        Ok(result) => result?,
        Err(_) => {
            harness.stop().await?;
            bail!("Codex conversation startup timed out after 20s; the owned provider was stopped");
        }
    }
    let account = harness.provider_account_id();
    let (endpoint, params) = harness
        .native_start()
        .ok_or_else(|| anyhow!("Codex startup ended without its native launch configuration"))?;
    let thread = params["threadId"].as_str().map(AgentSessionId::from);
    harness.stop().await?;
    let (store, session, attachment) = &owner;
    let connection = CodexConnection {
        store: store.clone(),
        session_id: session.clone(),
        thread_id: thread.clone(),
        attachment: Some(attachment.clone()),
        account_id: account.clone(),
    };
    // Recover saved history before opening the UI, without replaying its input.
    if thread.is_some() {
        tokio::time::timeout(
            std::time::Duration::from_secs(20),
            connection.recover_history(&endpoint),
        )
        .await
        .context("Codex native history recovery timed out")??;
    }
    let initial = std::sync::Arc::new(tokio::sync::Mutex::new(
        crate::harness::codex_connection::NativeInput {
            params: Some(params),
            text: (thread.is_none() && !launch.task_prompt.is_empty())
                .then(|| launch.task_prompt.clone()),
            context: launch.conversation_context.clone(),
        },
    ));
    let directory = tempfile::tempdir()?;
    let relay = directory.path().join("client.sock");
    let listener = tokio::net::UnixListener::bind(&relay)?;
    ready_tx
        .send((
            thread,
            NativeConnection {
                relay,
                upstream: endpoint.clone(),
            },
            account,
        ))
        .map_err(|_| anyhow!("Native terminal launch was cancelled"))?;
    let mut clients = tokio::task::JoinSet::new();
    tokio::pin!(closed_rx);
    loop {
        tokio::select! {
            _ = &mut closed_rx => break,
            accepted = listener.accept() => {
                let (client, _) = accepted?;
                let connection = connection.clone();
                let endpoint = endpoint.clone();
                let initial = initial.clone();
                clients.spawn(async move {
                    connection.serve_with_input(client, &endpoint, Some(initial)).await
                });
            }
            result = clients.join_next(), if !clients.is_empty() => {
                if let Some(result) = result { result?.context("Codex native terminal connection failed")?; }
            }
        }
    }
    if initial.lock().await.text.is_some() {
        bail!(
            "Codex terminal exited before submitting the saved initial request; reconnect to retry"
        );
    }
    Ok(())
}

fn observe_opencode_session(
    capture_dir: &Path,
    stderr: impl Read,
    mut diagnostics: File,
    discover_session: bool,
) -> std::io::Result<()> {
    let mut write_error = None;
    let mut observed = !discover_session;
    let mut reader = BufReader::new(stderr);
    let mut line = Vec::new();
    while reader.read_until(b'\n', &mut line)? != 0 {
        // Never write behind the native renderer. Drain even after a disk error
        // so diagnostic failure cannot block the provider or discard its input.
        if let Err(error) = diagnostics.write_all(&line) {
            write_error.get_or_insert(error);
        }
        if !observed {
            if let Some(agent_session) = parse_opencode_session_id(&String::from_utf8_lossy(&line))
            {
                observed = true;
                if let Err(error) = crate::session_record::write_provider_session(
                    capture_dir,
                    &agent_session.into(),
                    None,
                ) {
                    write_error.get_or_insert(error);
                }
            }
        }
        line.clear();
    }
    match write_error {
        Some(error) => Err(error),
        None if observed => Ok(()),
        None => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "OpenCode did not report a resumable session",
        )),
    }
}

fn parse_opencode_session_id(line: &str) -> Option<&str> {
    if !line
        .split_whitespace()
        .any(|field| field == "message=created")
    {
        return None;
    }
    line.split_whitespace()
        .find_map(|field| field.strip_prefix("id="))
        .filter(|id| id.starts_with("ses_") && id.len() > 4)
}

pub(crate) fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::EmailAddress;
    use crate::provider_account::new_account;
    use crate::store::{CredentialType, ProviderAccountId, ProviderToken, StorageConfig};
    use std::ffi::OsString;
    use std::os::unix::fs::PermissionsExt;

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

    #[cfg(unix)]
    #[test]
    fn provider_sigterm_without_stop_intent_remains_an_error() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let temp = tempfile::tempdir().unwrap();
        ledger.set_db_path(temp.path().join("loopflow.db"));
        let _home = EnvRestore::capture(&["LF_HOME"]);
        std::env::set_var("LF_HOME", temp.path());
        let provider = fake_provider(&temp, "kill -TERM $$");
        let capture = crate::session_record::CaptureHandle::begin_at(
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
                flow: crate::session_record::SessionFlowMembership::Independent,
                work: None,
            },
        )
        .unwrap();
        let command = SessionCommand {
            codex: None,
            input: None,
            context: None,
            attachment: None,
            remote: None,
            program: provider.display().to_string(),
            args: Vec::new(),
            cwd: temp.path().to_path_buf(),
        };

        let error =
            spawn_session_command_with_env(&command, &capture.environment(), None, None, None)
                .expect_err("unexplained SIGTERM must remain an error");

        assert!(error.to_string().contains("signal: 15"));
    }

    #[cfg(unix)]
    #[test]
    fn resume_retains_native_identity_without_sending_a_prompt() {
        let temp = tempfile::tempdir().unwrap();
        let provider = fake_provider(&temp, "for arg do printf '%s\\0' \"$arg\"; done > received");
        for harness in ["claude", "codex", "opencode"] {
            let command = build_session_command(
                &AgentConfig {
                    agent: Some(harness.to_string()),
                    cwd: Some(temp.path().to_path_buf()),
                    ..Default::default()
                },
                ProviderInvocation::Resume {
                    session_id: &"recorded-session".into(),
                },
                None,
            )
            .unwrap();
            assert!(command.input.is_none());
            if let Some(launch) = command.codex {
                assert_eq!(
                    launch.resume_token.as_ref().map(AgentSessionId::as_str),
                    Some("recorded-session")
                );
                assert!(launch.task_prompt.is_empty());
                continue;
            }
            let status = Command::new(&provider)
                .args(&command.args)
                .current_dir(temp.path())
                .status()
                .unwrap();
            assert!(status.success());
            let received = std::fs::read_to_string(temp.path().join("received")).unwrap();
            let arguments = received.split_terminator('\0').collect::<Vec<_>>();
            assert_eq!(arguments.last(), Some(&"recorded-session"));
            assert!(!arguments.contains(&"--prompt"));
            assert!(!received.contains("<lf:user>"));
        }
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
        std::fs::rename(provider, temp.path().join("opencode")).unwrap();
        let path = std::env::var_os("PATH").unwrap_or_default();
        std::env::set_var(
            "PATH",
            std::env::join_paths(
                std::iter::once(temp.path().to_path_buf()).chain(std::env::split_paths(&path)),
            )
            .unwrap(),
        );
        let capture = crate::session_record::CaptureHandle::begin_at(
            temp.path(),
            crate::session_record::SessionCaptureSpec {
                harness: "opencode".into(),
                model: None,
                surface: "tui".into(),
                cwd: temp.path().to_path_buf(),
                repo: None,
                worktree: None,
                skill: None,
                subjects: Vec::new(),
                flow: crate::session_record::SessionFlowMembership::Independent,
                work: None,
            },
        )
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
            let store = SqliteStore::new(&temp.path().join("loopflow.db")).unwrap();
            let rows = store.processes_since(0).unwrap();
            let agents = rows
                .iter()
                .filter(|row| row.kind == crate::process::ProcessKind::Agent)
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
                    .filter(|row| row.kind == crate::process::ProcessKind::Lf)
                    .count(),
                index + 1
            );
            let attachment = store.session_attachment(&saved.id).unwrap().unwrap();
            assert!(attachment.lf_process_id.is_none());
            assert!(rows
                .iter()
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

    #[test]
    fn opencode_startup_log_identifies_only_a_created_session() {
        let line = "timestamp=2026-08-28T18:56:34Z level=INFO run=tui message=created id=ses_012345 directory=/tmp/repo";

        assert_eq!(parse_opencode_session_id(line), Some("ses_012345"));
        assert_eq!(
            parse_opencode_session_id(
                "timestamp=2026-08-28T18:56:34Z level=INFO message=loaded id=ses_other"
            ),
            None
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn opencode_tui_records_its_native_session_without_wrapping_stdout() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let temp = tempfile::tempdir().unwrap();
        ledger.set_db_path(temp.path().join("loopflow.db"));
        let _home = EnvRestore::capture(&["LF_HOME"]);
        std::env::set_var("LF_HOME", temp.path());
        let _restore = EnvRestore::capture(&["PATH"]);
        let bin = temp.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let opencode = bin.join("opencode");
        std::fs::write(
            &opencode,
            "#!/bin/sh\nprintf '%s\\n' 'timestamp=2026-08-28T18:56:34Z level=INFO run=tui message=created id=ses_native directory=/tmp/repo' >&2\n",
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
        let capture = crate::session_record::CaptureHandle::begin_at(
            temp.path(),
            crate::session_record::SessionCaptureSpec {
                harness: "opencode".to_string(),
                model: None,
                surface: "tui".to_string(),
                cwd: temp.path().to_path_buf(),
                repo: None,
                worktree: None,
                skill: None,
                subjects: Vec::new(),
                flow: crate::session_record::SessionFlowMembership::Independent,
                work: None,
            },
        )
        .unwrap();
        let command = SessionCommand {
            codex: None,
            input: None,
            context: None,
            attachment: None,
            remote: None,
            program: "opencode".to_string(),
            args: vec![temp.path().display().to_string()],
            cwd: temp.path().to_path_buf(),
        };

        let outcome =
            session_command_status_with_env(&command, &capture.environment(), None, None, None)
                .unwrap();

        assert!(outcome.status.success());
        assert_eq!(
            crate::session_record::read_provider_session(&capture.artifact_dir())
                .unwrap()
                .map(|session| session.agent_session),
            Some("ses_native".into())
        );
    }

    #[test]
    fn recorded_native_move_returns_cleanly_without_stop_receipts() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let home = ledger.home();
        let store = SqliteStore::new(&home.join("loopflow.db")).unwrap();
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
                    );
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "native launch was not recorded"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        });
        let command = SessionCommand {
            codex: None,
            input: None,
            context: None,
            program: "/bin/sleep".into(),
            args: vec!["10".into()],
            cwd: home.into(),
            attachment: None,
            remote: None,
        };
        let result = spawn_session_command_with_env(&command, &BTreeMap::new(), None, None, None);
        stop.join().unwrap().unwrap();
        result.unwrap();
        assert!(!home.join("provider-client-stops").exists());
    }

    #[test]
    fn native_helper_admits_its_invocation_and_records_spawn_failure() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let home = ledger.home();
        let mut command = SessionCommand {
            codex: None,
            input: None,
            program: "/bin/sh".into(),
            args: vec!["-c".into(), "printf '%s' \"$$\" > pid".into()],
            cwd: home.into(),
            context: None,
            attachment: None,
            remote: None,
        };
        assert!(
            session_command_status_with_env(&command, &BTreeMap::new(), None, None, None)
                .unwrap()
                .status
                .success()
        );
        let pid: u32 = std::fs::read_to_string(home.join("pid"))
            .unwrap()
            .parse()
            .unwrap();
        command.program = home.join("absent").display().to_string();
        assert!(
            session_command_status_with_env(&command, &BTreeMap::new(), None, None, None)
                .unwrap_err()
                .to_string()
                .contains("not installed")
        );
        let store = SqliteStore::new(&home.join("loopflow.db")).unwrap();
        let rows = store.processes_since(0).unwrap();
        let agents = rows
            .iter()
            .filter(|row| row.kind == crate::process::ProcessKind::Agent)
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
                .process(agent.parent_lf_process_id.as_ref().unwrap())
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
            let store = SqliteStore::new(&home.join("loopflow.db"))?;
            let session = store.test_session("remote", &crate::session_record::new_artifact_key());
            let original = store.claim_session_attachment(
                &session.id.to_string(),
                None,
                &crate::id::LfProcessId::new(),
                true,
            )?;
            let mut command = Command::new("/bin/sleep");
            command.env_clear().arg("60");
            let mut provider = crate::harness::agent_process::spawn_native(
                command,
                &(store.clone(), session.id.to_string(), original.clone()),
            )?;
            let endpoint = home.join("provider.sock");
            store.record_session_connection(
                &session.id.to_string(),
                &original,
                endpoint.to_str().unwrap(),
                &"thread".into(),
            )?;
            let attached = store.claim_session_attachment(
                &session.id.to_string(),
                Some(&original),
                &crate::journal::current_lf_process_id().unwrap(),
                false,
            )?;
            let environment = BTreeMap::from([(
                crate::process::AGENT_CALLER_ENV.into(),
                serde_json::to_string(&attached.caller(session.id.to_string()))?,
            )]);
            let before = store.process(&attached.agent_process_id)?.unwrap();
            let mut command = SessionCommand {
                codex: None,
                input: None,
                program: "/usr/bin/true".into(),
                args: vec![],
                cwd: home.into(),
                context: None,
                attachment: Some((session.id.to_string(), attached.clone())),
                remote: Some(NativeConnection {
                    relay: home.join("relay.sock"),
                    upstream: endpoint.clone(),
                }),
            };
            let result = session_command_status_with_env(&command, &environment, None, None, None);
            let alive = provider.try_wait()?.is_none();
            let unchanged = store.process(&attached.agent_process_id)? == Some(before);
            command.remote = None;
            let local = session_command_status_with_env(&command, &environment, None, None, None);
            command.remote = Some(NativeConnection {
                relay: home.join("relay.sock"),
                upstream: home.join("other.sock"),
            });
            let wrong_endpoint =
                session_command_status_with_env(&command, &environment, None, None, None);
            command.attachment = None;
            let unadmitted =
                session_command_status_with_env(&command, &BTreeMap::new(), None, None, None);
            crate::harness::agent_process::stop_native(
                &mut provider,
                Some(&(store, session.id.to_string(), attached)),
            )?;
            assert!(result?.status.success());
            assert!(alive && unchanged);
            assert!(local.unwrap_err().to_string().contains("does not own"));
            assert!(wrong_endpoint
                .unwrap_err()
                .to_string()
                .contains("endpoint differs"));
            assert!(unadmitted
                .unwrap_err()
                .to_string()
                .contains("no admitted attachment"));
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
            let capture = crate::session_record::CaptureHandle::begin_at(
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
                    flow: crate::session_record::SessionFlowMembership::Independent,
                    work: None,
                },
            )?;
            capture.claim_session_attachment()?;
            let (session, attachment) = capture.session_attachment().unwrap();
            let store = SqliteStore::new(&home.join("loopflow.db"))?;
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
            let command = SessionCommand {
                codex: None,
                input: None,
                context: None,
                attachment: capture.session_attachment(),
                remote: None,
                program: "/bin/sleep".into(),
                args: vec!["60".into()],
                cwd: home.into(),
            };
            let error =
                session_command_status_with_env(&command, &capture.environment(), None, None, None)
                    .unwrap_err();
            assert!(
                error.to_string().contains("conflicting interactive_opened"),
                "{error:#}"
            );
            let ended = store.process(&attachment.agent_process_id)?.unwrap();
            assert!(ended.completed_at.is_some());
            assert_eq!(
                crate::journal::process_identity_evidence(
                    ended.pid.unwrap(),
                    ended.os_started_at.unwrap()
                ),
                crate::journal::ProcessIdentityEvidence::Dead,
            );
            Ok(())
        })
        .unwrap();
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn session_launch_tui_claude_signs_its_native_home_in_as_a_healthy_managed_login() {
        let ledger = crate::journal::TestLedgerGuard::new();
        let _ambient = crate::test_ambient::EnvGuard::new();
        let temp = tempfile::tempdir().unwrap();
        ledger.set_db_path(temp.path().join("loopflow.db"));
        let _restore = EnvRestore::capture(&[
            "LF_HOME",
            "LF_TEST_SESSION_ENV",
            "CLAUDE_CONFIG_DIR",
            "PATH",
        ]);
        std::env::set_var("LF_HOME", temp.path());
        let native = temp.path().join("native");
        std::env::set_var("CLAUDE_CONFIG_DIR", &native);

        let bin = temp.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let claude = bin.join("claude");
        std::fs::write(
            &claude,
            "#!/bin/sh\nprintf '%s' \"$CLAUDE_CONFIG_DIR\" > \"$LF_TEST_SESSION_ENV\"\n",
        )
        .unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let mut permissions = std::fs::metadata(&claude).unwrap().permissions();
            permissions.set_mode(0o755);
            std::fs::set_permissions(&claude, permissions).unwrap();
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
        let account_home = temp.path().join("accounts/claude/jackstah");
        let mut account = new_account(
            Provider::Claude,
            ProviderAccountId::parse("jackstah").unwrap(),
            account_home.clone(),
            Some(EmailAddress::parse("jackstah@gmail.com").unwrap()),
        );
        crate::provider_account::identity::tests::write_claude_identity(&mut account);
        store.upsert_provider_account(&account).await.unwrap();

        launch_session(
            &AgentConfig {
                agent: Some("claude".to_string()),
                cwd: Some(temp.path().to_path_buf()),
                task_prompt: "review it".into(),
                env: BTreeMap::new(),
                ..Default::default()
            },
            None,
            None,
        )
        .unwrap();

        assert_eq!(
            std::fs::read_to_string(capture).unwrap(),
            native.to_string_lossy()
        );
        assert_eq!(
            std::fs::read(native.join(".credentials.json")).unwrap(),
            std::fs::read(account_home.join(".credentials.json")).unwrap()
        );
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
        std::env::set_var("CODEX_HOME", temp.path().join("codex-home"));
        std::env::set_var("OPENCODE_API_KEY", "ambient-key");
        std::env::remove_var("CODEX_ACCESS_TOKEN");

        let bin = temp.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let opencode = bin.join("opencode");
        std::fs::write(
            &opencode,
            "#!/bin/sh\nprintf '%s' \"$OPENCODE_API_KEY\" > \"$LF_TEST_SESSION_ENV\"\nprintf '%s\\n' 'timestamp=2026-10-09T00:00:00Z level=INFO run=tui message=created id=ses_native directory=/tmp/repo' >&2\n",
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
