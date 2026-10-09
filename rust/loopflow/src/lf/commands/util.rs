use anyhow::{anyhow, bail, Context, Result};
use fs2::FileExt;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};

use crate::engine::{codex_permission_args, missing_agent_message, workspace_add_dirs};
use crate::provider_auth::Provider;
use crate::session_record::{ProviderClientRef, ProviderClientStopReason};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SessionCommand {
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    pub(crate) cwd: PathBuf,
}

#[allow(clippy::too_many_arguments)] // Provider inputs plus native skill flags and context file.
pub(crate) fn launch_session(
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    prompt: &str,
    environment: &BTreeMap<String, String>,
    provider_session_id: Option<&str>,
    flags: &[String],
    context_file: Option<&Path>,
) -> Result<()> {
    let worktree = absolute_path(worktree);
    let mut command = build_session_command(
        harness,
        model,
        &worktree,
        prompt,
        provider_session_id,
        context_file,
    )?;
    command.args.splice(0..0, flags.iter().cloned());
    spawn_session_command_with_env(&command, environment, provider_session_id, None, None)
}

pub(crate) fn build_session_command(
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    prompt: &str,
    provider_session_id: Option<&str>,
    context_file: Option<&Path>,
) -> Result<SessionCommand> {
    let worktree_arg = worktree.to_string_lossy().to_string();

    let args = match harness {
        "codex" => {
            let mut args = vec!["-C".to_string(), worktree_arg];
            if let Some(model) = model {
                args.push("-c".to_string());
                args.push(format!("model=\"{model}\""));
            }
            for dir in workspace_add_dirs(worktree) {
                args.push("--add-dir".to_string());
                args.push(dir.to_string_lossy().to_string());
            }
            args.extend(codex_permission_args(Some(worktree), false, false));
            if let Some(path) = context_file {
                args.push("-c".to_string());
                args.push(format!(
                    "model_instructions_file={}",
                    serde_json::to_string(&path.to_string_lossy())?
                ));
            }
            // Ported skill frontmatter starts with `---`, which is prompt data.
            args.push("--".to_string());
            args.push(prompt.to_string());
            args
        }
        "claude" => {
            let mut args = Vec::new();
            if let Some(model) = model {
                args.push("--model".to_string());
                args.push(model.to_string());
            }
            for dir in workspace_add_dirs(worktree) {
                args.push("--add-dir".to_string());
                args.push(dir.to_string_lossy().to_string());
            }
            if let Some(provider_session_id) = provider_session_id {
                args.push("--session-id".to_string());
                args.push(provider_session_id.to_string());
            }
            if let Some(path) = context_file {
                args.push("--append-system-prompt-file".to_string());
                args.push(path.to_string_lossy().to_string());
            }
            // Claude's variadic --add-dir otherwise consumes the positional prompt.
            args.push("--".to_string());
            args.push(prompt.to_string());
            args
        }
        "opencode" => {
            let mut args = vec![worktree_arg, "--prompt".to_string(), prompt.to_string()];
            if let Some(model) = model {
                args.push("--model".to_string());
                args.push(model.to_string());
            }
            args
        }
        _ => bail!(
            "unsupported session launcher harness '{}'. Use claude, codex, or opencode.",
            harness
        ),
    };
    Ok(SessionCommand {
        program: harness.to_string(),
        args,
        cwd: worktree.to_path_buf(),
    })
}

pub(crate) fn resume_session(
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    artifact_key: &String,
    provider_session: &crate::session_record::ProviderSessionRef,
) -> Result<()> {
    resume_session_with_env(
        harness,
        model,
        worktree,
        artifact_key,
        provider_session,
        &BTreeMap::new(),
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)] // Native launch inputs plus its startup exclusion lock.
pub(crate) fn resume_session_with_env(
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    artifact_key: &String,
    provider_session: &crate::session_record::ProviderSessionRef,
    extra_environment: &BTreeMap<String, String>,
    launch_lock: Option<File>,
    remote: Option<&Path>,
) -> Result<()> {
    let user_name = crate::engine::config::participant_name()?;
    let mut command = build_resume_session_command(
        harness,
        model,
        worktree,
        &provider_session.provider_session_id,
    )?;
    if let Some(remote) = remote {
        if harness != "codex" {
            bail!("This provider has no native remote connection");
        }
        command.args.splice(
            1..1,
            ["--remote".into(), format!("unix://{}", remote.display())],
        );
    }
    let mut environment = BTreeMap::from([(
        crate::session_record::CAPTURE_KEY_ENV.to_string(),
        artifact_key.to_string(),
    )]);
    environment.extend(extra_environment.clone());
    environment.insert(
        crate::engine::config::USER_NAME_ENV.to_string(),
        user_name.unwrap_or_default(),
    );
    // A native resume has no CaptureHandle, but still owns an exact driver.
    // Remote connections already claimed their live engine's driver.
    let owned = if remote.is_none() {
        if let Some(process) = crate::journal::current_process_lfid() {
            let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
            let session = store
                .session_for_artifact(artifact_key)?
                .ok_or_else(|| anyhow!("Session input {artifact_key} is not recorded"))?;
            let driver =
                crate::session_record::claim_provider_driver(&store, &session.id, &process)?;
            environment.insert(
                crate::process::AGENT_CALLER_ENV.into(),
                serde_json::to_string(&driver.caller(session.id.clone()))?,
            );
            crate::session_record::register_session_driver_interrupt(
                &store,
                session.id.clone(),
                driver.clone(),
            );
            Some((store, session.id, driver))
        } else {
            None
        }
    } else {
        None
    };
    let result = spawn_session_command_with_env(
        &command,
        &environment,
        Some(&provider_session.provider_session_id),
        provider_session.account_id.as_ref(),
        launch_lock,
    );
    if let Some((store, session, driver)) = owned {
        let outcome = if result.is_ok() {
            "completed"
        } else {
            "failed"
        };
        match crate::session_record::finish_session_driver(&store, &session, &driver, outcome) {
            Ok(()) | Err(crate::store::StoreError::InvalidAuthority(_)) => {}
            Err(error) => return Err(error.into()),
        }
    }
    result
}

pub(crate) fn active_provider_clients(dir: &Path, harness: &str) -> Result<Vec<ProviderClientRef>> {
    let clients = crate::session_record::read_provider_clients(dir)
        .map_err(|error| anyhow!("cannot read provider clients: {error}"))?;
    clients
        .into_iter()
        .filter_map(|client| match provider_client_is_live(&client, harness) {
            Ok(true) => Some(Ok(client)),
            Ok(false) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
}

pub(crate) fn replace_provider_clients(
    dir: &Path,
    harness: &str,
    clients: &[ProviderClientRef],
    reason: ProviderClientStopReason,
) -> Result<()> {
    let _launch = lock_provider_clients(dir)?;
    replace_provider_clients_locked(dir, harness, clients, reason)
}

fn lock_provider_clients(dir: &Path) -> Result<File> {
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(dir.join("provider-clients.lock"))?;
    FileExt::lock_exclusive(&file).context("lock native Session launch")?;
    Ok(file)
}

pub(crate) fn require_provider_session_process(dir: &Path) -> Result<()> {
    let input = crate::session_record::input_id_from_dir(dir)?;
    let store = SqliteStore::open_processes_read_only(&crate::store::database_path_from_env()?)?;
    let session = store
        .session_for_artifact(&input)?
        .ok_or_else(|| anyhow!("Input {input} is not recorded on this Machine"))?;
    let Some(task_id) = session.task_id else {
        return Ok(());
    };
    let task = store
        .task_by_issue(task_id.as_str())?
        .ok_or_else(|| anyhow!("Task {task_id} is not registered"))?;
    if store.task_deleted(&task)? {
        bail!(
            "Task {} was deleted and cannot resume execution",
            task.plan.identifier
        );
    }
    Ok(())
}

fn replace_provider_clients_locked(
    dir: &Path,
    harness: &str,
    clients: &[ProviderClientRef],
    reason: ProviderClientStopReason,
) -> Result<()> {
    require_unchanged_provider_clients(dir, clients)?;
    for client in clients {
        // The PID may have been reused since the caller collected its clients.
        if !provider_client_is_live(client, harness)? {
            continue;
        }
        crate::session_record::write_provider_client_stop(dir, client.pid, reason)
            .context("record why the provider client is stopping")?;
        if let Err(error) = signal_provider_client(client.pid, libc::SIGTERM) {
            let _ = crate::session_record::remove_provider_client_stop(dir, client.pid);
            return Err(error);
        }
    }
    for _ in 0..20 {
        if provider_clients_have_exited(clients, harness)? {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    for client in clients {
        if provider_client_is_live(client, harness)? {
            signal_provider_client(client.pid, libc::SIGKILL)?;
        }
    }
    for _ in 0..20 {
        if provider_clients_have_exited(clients, harness)? {
            require_unchanged_provider_clients(dir, clients)?;
            if !active_provider_clients(dir, harness)?.is_empty() {
                bail!("a provider client started while stopping; retry with its current identity");
            }
            for client in clients {
                crate::session_record::remove_provider_client(dir, client.pid)?;
            }
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    bail!("the existing provider client did not exit; resume was not started")
}

fn require_unchanged_provider_clients(dir: &Path, clients: &[ProviderClientRef]) -> Result<()> {
    let current = crate::session_record::read_provider_clients(dir)?;
    for client in clients {
        if current
            .iter()
            .any(|saved| saved.pid == client.pid && saved != client)
        {
            bail!(
                "provider client {} changed while stopping; retry with its current identity",
                client.pid
            );
        }
    }
    Ok(())
}

fn provider_clients_have_exited(clients: &[ProviderClientRef], harness: &str) -> Result<bool> {
    for client in clients {
        if provider_client_is_live(client, harness)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn provider_client_is_live(client: &ProviderClientRef, harness: &str) -> Result<bool> {
    let output = Command::new("ps")
        .args([
            "-p",
            &client.pid.to_string(),
            "-o",
            "etime=",
            "-o",
            "command=",
        ])
        .output()
        .with_context(|| format!("cannot inspect provider client {}", client.pid))?;
    if !output.status.success() {
        if output.status.code() == Some(1) && output.stdout.is_empty() && output.stderr.is_empty() {
            return Ok(false);
        }
        bail!(
            "cannot inspect provider client {}: process query failed",
            client.pid
        );
    }
    let line = String::from_utf8_lossy(&output.stdout);
    let mut fields = line.split_whitespace();
    let elapsed = fields.next().and_then(elapsed_seconds).ok_or_else(|| {
        anyhow!(
            "cannot inspect provider client {}: invalid process age",
            client.pid
        )
    })?;
    let command = fields.collect::<Vec<_>>().join(" ");
    if command.is_empty() {
        bail!(
            "cannot inspect provider client {}: missing process command",
            client.pid
        );
    }
    let elapsed = i64::try_from(elapsed).context("provider client process age exceeds i64")?;
    let expected_start = OffsetDateTime::now_utc()
        .unix_timestamp()
        .saturating_sub(elapsed);
    Ok(crate::session_record::provider_client_matches(
        client,
        harness,
        client.pid,
        expected_start,
        &command,
    ))
}

fn elapsed_seconds(value: &str) -> Option<u64> {
    let (days, clock) = match value.split_once('-') {
        Some((days, clock)) => (days.parse::<u64>().ok()?, clock),
        None => (0, value),
    };
    let parts = clock
        .split(':')
        .map(str::parse::<u64>)
        .collect::<std::result::Result<Vec<_>, _>>()
        .ok()?;
    let clock = match parts.as_slice() {
        [minutes, seconds] => minutes.checked_mul(60)?.checked_add(*seconds)?,
        [hours, minutes, seconds] => hours
            .checked_mul(3_600)?
            .checked_add(minutes.checked_mul(60)?)?
            .checked_add(*seconds)?,
        _ => return None,
    };
    days.checked_mul(86_400)?.checked_add(clock)
}

#[cfg(unix)]
fn signal_provider_client(pid: u32, signal: libc::c_int) -> Result<()> {
    let pid = libc::pid_t::try_from(pid).context("provider pid does not fit this platform")?;
    // SAFETY: a positive, receipt-verified pid targets exactly one provider process.
    let result = unsafe { libc::kill(pid, signal) };
    if result == 0 {
        return Ok(());
    }
    let error = std::io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ESRCH) {
        Ok(())
    } else {
        Err(error).context("stop the existing provider client")
    }
}

#[cfg(not(unix))]
fn signal_provider_client(_pid: u32, _signal: libc::c_int) -> Result<()> {
    bail!("--replace is not supported on this platform")
}

fn build_resume_session_command(
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    provider_session_id: &str,
) -> Result<SessionCommand> {
    let cwd = absolute_path(worktree);
    let worktree_arg = cwd.to_string_lossy().to_string();
    let args = match harness {
        "claude" => {
            let mut args = Vec::new();
            if let Some(model) = model {
                args.extend(["--model".to_string(), model.to_string()]);
            }
            for dir in workspace_add_dirs(&cwd) {
                args.extend(["--add-dir".to_string(), dir.to_string_lossy().to_string()]);
            }
            args.extend(["--resume".to_string(), provider_session_id.to_string()]);
            args
        }
        "codex" => {
            let mut args = vec!["resume".to_string(), "-C".to_string(), worktree_arg];
            if let Some(model) = model {
                args.extend(["--model".to_string(), model.to_string()]);
            }
            for dir in workspace_add_dirs(&cwd) {
                args.extend(["--add-dir".to_string(), dir.to_string_lossy().to_string()]);
            }
            args.extend(codex_permission_args(Some(&cwd), false, false));
            args.extend(["--".to_string(), provider_session_id.to_string()]);
            args
        }
        "opencode" => {
            let mut args = vec![
                worktree_arg,
                "--session".to_string(),
                provider_session_id.to_string(),
            ];
            if let Some(model) = model {
                args.extend(["--model".to_string(), model.to_string()]);
            }
            args
        }
        _ => {
            return Err(anyhow!(
                "unsupported session launcher harness '{}'. Use claude, codex, or opencode.",
                harness
            ));
        }
    };
    Ok(SessionCommand {
        program: harness.to_string(),
        args,
        cwd,
    })
}

fn spawn_session_command_with_env(
    command: &SessionCommand,
    environment: &BTreeMap<String, String>,
    provider_session_id: Option<&str>,
    exact_account_id: Option<&crate::store::ProviderAccountId>,
    launch_lock: Option<File>,
) -> Result<()> {
    let _planning_sync =
        crate::ops::linear_observe::PlanningSync::start_for_directory(&command.cwd)?;
    let outcome = session_command_status_with_env(
        command,
        environment,
        provider_session_id,
        exact_account_id,
        launch_lock,
    )?;
    if let Some(reason) = outcome.stop_reason {
        eprintln!("{}", provider_client_stop_message(reason));
        Ok(())
    } else if outcome.status.success() {
        Ok(())
    } else if provider_session_id.is_some() {
        Err(anyhow!(
            "{} could not open this session (status {}). If another client still owns it, close that client or use `lf session connect --replace` for a Loopflow-owned client.",
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

fn provider_client_stop_message(reason: ProviderClientStopReason) -> &'static str {
    match reason {
        ProviderClientStopReason::Retired => "Review retired by Task restart.",
        ProviderClientStopReason::Moved => "Session moved to another terminal.",
        ProviderClientStopReason::Completed => "Session completed elsewhere.",
    }
}

#[derive(Debug)]
struct SessionCommandOutcome {
    status: std::process::ExitStatus,
    stop_reason: Option<ProviderClientStopReason>,
}

fn record_interactive_opened(environment: &BTreeMap<String, String>) -> Result<()> {
    let Some(input) = environment.get(crate::session_record::CAPTURE_KEY_ENV) else {
        return Ok(());
    };
    let Some(process) = crate::journal::current_process_lfid() else {
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

fn native_provider_driver(
    environment: &BTreeMap<String, String>,
) -> Result<Option<(SqliteStore, String, crate::process::SessionDriver)>> {
    let Some(caller) = environment.get(crate::process::AGENT_CALLER_ENV) else {
        return Ok(None);
    };
    let caller: crate::process::AgentCaller = serde_json::from_str(caller)?;
    let Some(process) = crate::journal::current_process_lfid() else {
        return Ok(None);
    };
    let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
    let driver = store
        .session_driver(&caller.session_id)?
        .ok_or_else(|| anyhow!("Session has no admitted driver"))?;
    if driver.process_lfid.as_ref() != Some(&process)
        || driver.caller(caller.session_id.clone()) != caller
    {
        bail!("Session driver changed before provider launch");
    }
    if driver.provider_process_lfid != process {
        return Ok(None);
    }
    Ok(Some((store, caller.session_id, driver)))
}

fn session_command_status_with_env(
    command: &SessionCommand,
    environment: &BTreeMap<String, String>,
    provider_session_id: Option<&str>,
    exact_account_id: Option<&crate::store::ProviderAccountId>,
    launch_lock: Option<File>,
) -> Result<SessionCommandOutcome> {
    let started = std::time::Instant::now();
    // Keep admission and client publication on the same side of Session stop.
    // Release before waiting for the child, so stop can settle that client.
    let capture_dir = environment
        .get(crate::session_record::CAPTURE_KEY_ENV)
        .map(|key| crate::session_record::capture_dir(key))
        .transpose()?;
    let launch = capture_dir
        .as_deref()
        .map(|dir| -> Result<File> {
            let launch = lock_provider_clients(dir)?;
            require_provider_session_process(dir)?;
            Ok(launch)
        })
        .transpose()?;
    let provider = match command.program.as_str() {
        "claude" => Some(Provider::Claude),
        "codex" => Some(Provider::Codex),
        _ => None,
    };
    let account_route = provider
        .map(|provider| {
            crate::provider_account::resolve_provider_account_exact_blocking(
                provider,
                provider_session_id.map(str::to_string),
                exact_account_id.cloned(),
            )
        })
        .transpose()
        .map_err(|error| anyhow!("failed to select provider account: {error}"))?
        .flatten();

    let mut process = Command::new(&command.program);
    process
        .env_remove("LOOPFLOW_DIRECTIVE_FILE")
        .envs(environment);
    let mut title = crate::engine::terminal_title::TerminalTitle::prepare(
        environment,
        &command.program,
        &mut process,
    );
    if let Some(route) = &account_route {
        process.args(route.provider_args());
    }
    let observed_capture = capture_dir
        .clone()
        .filter(|_| command.program == "opencode" && provider_session_id.is_none());
    if observed_capture.is_some() {
        process.args(["--print-logs", "--log-level", "INFO"]);
        process.stderr(Stdio::piped());
    }
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
        route.record_process_blocking(provider_session_id.map(str::to_string), None)?;
    }
    let codex_profile =
        if command.program == "codex" && provider_session_id.is_none() && capture_dir.is_some() {
            Some(prepare_codex_capture(&mut process)?)
        } else {
            None
        };
    process.args(&command.args);
    if let (Some(capture_dir), Some(provider_session_id)) =
        (capture_dir.as_deref(), provider_session_id)
    {
        crate::session_record::write_provider_session(
            capture_dir,
            provider_session_id,
            account_route
                .as_ref()
                .map(|route| route.account_id().clone()),
        )?;
    }
    // A remote terminal is only a client of the surviving engine. A local
    // terminal owns the provider generation created by this exact Process.
    let owned = native_provider_driver(environment)?;
    if let Some((store, session, driver)) = &owned {
        store.record_session_provider_launch(session, driver, true)?;
    }
    tracing::debug!(
        elapsed_ms = started.elapsed().as_millis(),
        "prepared native provider launch"
    );
    let mut child = match process.spawn() {
        Ok(child) => child,
        Err(error) => {
            if let Some((store, session, driver)) = &owned {
                store.record_native_provider_exit(session, driver, false)?;
            }
            if error.kind() == std::io::ErrorKind::NotFound {
                return Err(anyhow!(missing_agent_message(&command.program)));
            }
            return Err(error.into());
        }
    };
    if let Some((store, session, driver)) = &owned {
        let recorded = (|| -> Result<()> {
            if let Some(started) = crate::journal::process_started_at(child.id())? {
                store.record_session_provider_process(session, driver, child.id(), started)?;
            }
            Ok(())
        })();
        if let Err(error) = recorded {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    }

    drop(activation);
    let client = match ProviderClientGuard::publish(capture_dir.as_deref(), child.id()) {
        Ok(client) => client,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    if let Err(error) = record_interactive_opened(environment) {
        let _ = child.kill();
        let _ = child.wait();
        return Err(error);
    }
    // Completion and another Open can proceed once exact client ownership is visible.
    drop(launch_lock);
    drop(launch);
    let observer = observed_capture.map(|capture_dir| {
        let stderr = child
            .stderr
            .take()
            .expect("piped OpenCode stderr is available");
        std::thread::spawn(move || observe_opencode_session(&capture_dir, stderr))
    });
    let status = if let Some(title) = &mut title {
        crate::engine::process::wait_for_exit(&mut child, None, || title.refresh())?.0
    } else {
        child.wait()?
    };
    if let Some((store, session, driver)) = &owned {
        store.record_native_provider_exit(session, driver, true)?;
    }
    if let Some(observer) = observer {
        observer
            .join()
            .map_err(|_| anyhow!("OpenCode session observer panicked"))??;
    }
    // Record which home this conversation lives in, so reopening returns there.
    if let (Some(route), Some(capture_dir)) = (&account_route, capture_dir.as_deref()) {
        if let Ok(Some(session)) = crate::session_record::read_provider_session(capture_dir) {
            if let Err(error) =
                route.record_process_blocking(Some(session.provider_session_id), None)
            {
                tracing::warn!(%error, "failed to record the provider session's account home");
            }
        }
    }
    let stop_reason = client
        .as_ref()
        .map(ProviderClientGuard::take_stop_reason)
        .transpose()?
        .flatten();
    if status.success() && stop_reason.is_none() && codex_profile.is_some() {
        let capture_dir = capture_dir
            .as_deref()
            .expect("Codex capture has a directory");
        if crate::session_record::read_provider_session(capture_dir)?.is_none() {
            bail!("Codex exited without recording its native Session ID. Launch evidence remains at {}. Inspect the provider's hook diagnostic before retrying; no unrelated Session was attached.", capture_dir.display());
        }
    }
    Ok(SessionCommandOutcome {
        status,
        stop_reason,
    })
}

#[derive(Debug)]
struct ProviderClientGuard {
    capture_dir: PathBuf,
    pid: u32,
}

impl ProviderClientGuard {
    fn publish(capture_dir: Option<&Path>, pid: u32) -> Result<Option<Self>> {
        let Some(capture_dir) = capture_dir else {
            return Ok(None);
        };
        let capture_dir = capture_dir.to_path_buf();
        crate::session_record::write_provider_client(&capture_dir, pid)
            .map_err(|error| anyhow!("cannot record active provider client: {error}"))?;
        Ok(Some(Self { capture_dir, pid }))
    }

    fn take_stop_reason(&self) -> Result<Option<ProviderClientStopReason>> {
        let reason = crate::session_record::read_provider_client_stop(&self.capture_dir, self.pid)?;
        if reason.is_some() {
            crate::session_record::remove_provider_client_stop(&self.capture_dir, self.pid)?;
        }
        Ok(reason)
    }
}

impl Drop for ProviderClientGuard {
    fn drop(&mut self) {
        if let Err(error) =
            crate::session_record::remove_provider_client(&self.capture_dir, self.pid)
        {
            tracing::warn!(pid = self.pid, %error, "failed to clear provider client receipt");
        }
        if let Err(error) =
            crate::session_record::remove_provider_client_stop(&self.capture_dir, self.pid)
        {
            tracing::warn!(pid = self.pid, %error, "failed to clear provider client stop");
        }
    }
}

fn prepare_codex_capture(process: &mut Command) -> Result<tempfile::NamedTempFile> {
    let home = process
        .get_envs()
        .find(|(key, _)| *key == "CODEX_HOME")
        .map(|(_, value)| value.map(PathBuf::from))
        .unwrap_or_else(|| std::env::var_os("CODEX_HOME").map(PathBuf::from))
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".codex")
        });
    std::fs::create_dir_all(&home)?;
    let mut profile = tempfile::Builder::new()
        .prefix("lf-capture-")
        .suffix(".config.toml")
        .tempfile_in(home)?;
    let executable = std::env::current_exe()
        .map_err(|error| anyhow!("cannot resolve lf for Codex session capture: {error}"))?;
    let command = format!(
        "{} __provider-session",
        crate::engine::process::shell_escape(&executable.to_string_lossy())
    );
    let command = serde_json::to_string(&command).expect("shell command serializes as TOML string");
    write!(profile, "hooks={{ SessionStart = [{{ matcher = \"startup\", hooks = [{{ type = \"command\", command = {command}, timeout = 5 }}] }}] }}")?;
    profile.flush()?;
    if !codex_supplies_hook_trust(process)? {
        process.arg("--dangerously-bypass-hook-trust");
    }
    let name = profile
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix(".config.toml"))
        .expect("generated Codex profile has a valid name");
    process.args(["--profile", name]);
    Ok(profile)
}

fn codex_supplies_hook_trust(process: &Command) -> Result<bool> {
    let mut probe = Command::new(process.get_program());
    if let Some(cwd) = process.get_current_dir() {
        probe.current_dir(cwd);
    }
    for (key, value) in process.get_envs() {
        match value {
            Some(value) => {
                probe.env(key, value);
            }
            None => {
                probe.env_remove(key);
            }
        }
    }
    // Help can bypass wrapper injection. An incomplete option reaches the
    // ordinary parser, but cannot start a provider or consume terminal input.
    let mut stderr = tempfile::tempfile()?;
    probe
        .args(["--dangerously-bypass-hook-trust", "--model"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(stderr.try_clone()?);
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        probe.process_group(0);
    }
    let mut child = probe.spawn()?;
    let group = crate::engine::process::ProcessGroupGuard::new(child.id());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while child.try_wait()?.is_none() {
        if std::time::Instant::now() >= deadline {
            group.terminate();
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            bail!("Codex argument probe timed out before launch");
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    group.terminate();
    stderr.seek(SeekFrom::Start(0))?;
    let mut diagnostic = String::new();
    stderr.take(65536).read_to_string(&mut diagnostic)?;
    if diagnostic
        .contains("the argument '--dangerously-bypass-hook-trust' cannot be used multiple times")
    {
        return Ok(true);
    }
    if diagnostic.contains("a value is required for '--model <MODEL>'") {
        return Ok(false);
    }
    bail!(
        "Codex argument probe failed before launch: {}",
        diagnostic.trim()
    )
}

fn observe_opencode_session(capture_dir: &Path, stderr: impl Read) -> std::io::Result<()> {
    let mut write_error = None;
    let mut observed = false;
    for line in BufReader::new(stderr).lines() {
        let line = line?;
        if !observed {
            if let Some(provider_session_id) = parse_opencode_session_id(&line) {
                observed = true;
                if let Err(error) = crate::session_record::write_provider_session(
                    capture_dir,
                    provider_session_id,
                    None,
                ) {
                    write_error = Some(error);
                }
            }
        }
        if line.contains("level=ERROR") {
            eprintln!("{line}");
        }
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

fn absolute_path(path: &Path) -> PathBuf {
    if let Ok(canonical) = path.canonicalize() {
        return canonical;
    }
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(path))
        .unwrap_or_else(|_| path.to_path_buf())
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
    use std::process::{Child, Stdio};

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

    fn path() -> PathBuf {
        PathBuf::from("/tmp/loop flow")
    }

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| value.to_string()).collect()
    }

    fn git_worktree_fixture() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let main = tmp.path().join("repo");
        let worktree = tmp.path().join("repo.feature");
        std::fs::create_dir(&main).expect("create repo dir");
        std::fs::write(main.join("README.md"), "hello\n").expect("write file");

        git(&main, &["init", "-b", "main"]);
        git(&main, &["config", "user.email", "test@example.com"]);
        git(&main, &["config", "user.name", "Test User"]);
        git(&main, &["add", "."]);
        git(&main, &["commit", "-m", "init"]);
        git(
            &main,
            &[
                "worktree",
                "add",
                "-b",
                "feature",
                worktree.to_str().expect("utf8 worktree"),
            ],
        );

        (tmp, main, worktree)
    }

    fn git(repo: &Path, args: &[&str]) {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(repo)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git -C {} {} failed:\n{}",
            repo.display(),
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
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

    struct NativeClient {
        child: Child,
        capture: crate::session_record::CaptureHandle,
        temp: tempfile::TempDir,
        _environment: EnvRestore,
    }

    impl NativeClient {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let names = [
                "LF_HOME",
                "LF_RUN_ID",
                "LF_RUN_DIR",
                "LF_WAVE_ID",
                "LF_HUMAN_SESSION",
            ];
            let environment = EnvRestore::capture(&names);
            for name in names {
                std::env::remove_var(name);
            }
            std::env::set_var("LF_HOME", temp.path());
            let provider = fake_provider(
                &temp,
                "trap '' TERM\nprintf ready > \"$1\"\nwhile :; do /bin/sleep 0.05; done",
            );
            let capture = crate::session_record::CaptureHandle::begin_at(
                temp.path(),
                crate::session_record::SessionCaptureSpec {
                    harness: "fake-provider".into(),
                    model: None,
                    surface: "tui".into(),
                    cwd: temp.path().to_path_buf(),
                    repo: None,
                    worktree: None,
                    skill: None,
                    subjects: Vec::new(),
                    work: None,
                    flow: crate::session_record::SessionFlowMembership::Independent,
                },
            )
            .unwrap();
            let ready = temp.path().join("ready");
            let child = Command::new(provider)
                .arg(&ready)
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            let fixture = Self {
                child,
                capture,
                temp,
                _environment: environment,
            };
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !ready.exists() {
                assert!(
                    std::time::Instant::now() < deadline,
                    "provider did not start"
                );
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            crate::session_record::write_provider_client(
                &fixture.capture.artifact_dir(),
                fixture.child.id(),
            )
            .unwrap();
            fixture
        }

        fn mock_ps(&self, body: &str) {
            let bin = self.temp.path().join("bin");
            std::fs::create_dir_all(&bin).unwrap();
            let ps = bin.join("ps");
            std::fs::write(&ps, format!("#!/bin/sh\n{body}\n")).unwrap();
            std::fs::set_permissions(&ps, std::fs::Permissions::from_mode(0o755)).unwrap();
            std::env::set_var("PATH", bin);
        }
    }

    impl Drop for NativeClient {
        fn drop(&mut self) {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }

    #[test]
    fn provider_client_stop_preserves_unknown_process_evidence() {
        let _lock = crate::journal::test_env_lock();
        let _env = EnvRestore::capture(&["PATH"]);
        let mut fixture = NativeClient::new();
        let dir = fixture.capture.artifact_dir();
        let clients = active_provider_clients(&dir, "fake-provider").unwrap();
        assert_eq!(clients.len(), 1);
        for query in ["exit 2", "echo invalid-age fake-provider", "echo 00:00"] {
            fixture.mock_ps(query);
            assert!(active_provider_clients(&dir, "fake-provider").is_err());
            assert!(replace_provider_clients(
                &dir,
                "fake-provider",
                &clients,
                ProviderClientStopReason::Moved,
            )
            .is_err());
            assert!(fixture.child.try_wait().unwrap().is_none());
            assert_eq!(
                crate::session_record::read_provider_clients(&dir).unwrap(),
                clients
            );
            assert!(
                crate::session_record::read_provider_client_stop(&dir, fixture.child.id())
                    .unwrap()
                    .is_none()
            );
        }
        std::env::set_var("PATH", fixture.temp.path().join("missing"));
        assert!(active_provider_clients(&dir, "fake-provider").is_err());
        assert!(replace_provider_clients(
            &dir,
            "fake-provider",
            &clients,
            ProviderClientStopReason::Moved,
        )
        .is_err());
        assert!(fixture.child.try_wait().unwrap().is_none());
    }

    #[test]
    fn provider_client_stop_rechecks_identity_before_signaling() {
        let _lock = crate::journal::test_env_lock();
        let mut fixture = NativeClient::new();
        let dir = fixture.capture.artifact_dir();
        let mut clients = active_provider_clients(&dir, "fake-provider").unwrap();
        // Simulate a caller retaining an older process at this now-reused PID.
        clients[0].started_at -= time::Duration::hours(1);
        let current = crate::session_record::read_provider_clients(&dir).unwrap();
        let receipt = dir
            .join("provider-clients")
            .join(format!("{}.json", fixture.child.id()));
        std::fs::write(&receipt, serde_json::to_vec(&clients[0]).unwrap()).unwrap();
        replace_provider_clients(
            &dir,
            "fake-provider",
            &clients,
            ProviderClientStopReason::Moved,
        )
        .unwrap();
        assert!(fixture.child.try_wait().unwrap().is_none());
        // A newly published receipt also survives an older caller's cleanup.
        std::fs::write(&receipt, serde_json::to_vec(&current[0]).unwrap()).unwrap();
        assert!(replace_provider_clients(
            &dir,
            "fake-provider",
            &clients,
            ProviderClientStopReason::Moved,
        )
        .is_err());
        assert!(fixture.child.try_wait().unwrap().is_none());
        assert_eq!(
            crate::session_record::read_provider_clients(&dir)
                .unwrap()
                .len(),
            1
        );
        assert!(
            crate::session_record::read_provider_client_stop(&dir, fixture.child.id())
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn provider_client_stop_keeps_a_new_client_outside_the_observed_set() {
        let _lock = crate::journal::test_env_lock();
        let mut fixture = NativeClient::new();
        let dir = fixture.capture.artifact_dir();
        assert!(replace_provider_clients(
            &dir,
            "fake-provider",
            &[],
            ProviderClientStopReason::Moved,
        )
        .is_err());
        assert!(fixture.child.try_wait().unwrap().is_none());
        assert_eq!(
            active_provider_clients(&dir, "fake-provider")
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn provider_client_move_preserves_unknown_exit_and_native_history() {
        let _lock = crate::journal::test_env_lock();
        let _env = EnvRestore::capture(&["PATH", "LF_HOME"]);
        let original_path = std::env::var_os("PATH").unwrap();
        let mut fixture = NativeClient::new();
        std::env::set_var("LF_HOME", fixture.temp.path());
        let dir = fixture.capture.artifact_dir();
        crate::session_record::write_provider_session(&dir, "native-history", None).unwrap();
        let clients = active_provider_clients(&dir, "fake-provider").unwrap();
        fixture.mock_ps("echo unreadable fake-provider");
        assert!(replace_provider_clients(
            &dir,
            "fake-provider",
            &clients,
            ProviderClientStopReason::Moved
        )
        .is_err());
        assert!(fixture.child.try_wait().unwrap().is_none());
        assert!(!crate::session_record::read_provider_clients(&dir)
            .unwrap()
            .is_empty());
        assert_eq!(
            crate::session_record::read_provider_clients(&dir)
                .unwrap()
                .len(),
            1
        );

        // Losing inspection after SIGTERM must not resolve this still-live client.
        fixture.mock_ps(&format!(
            "if [ -e '{}' ]; then exit 2; fi\nexec /bin/ps \"$@\"",
            dir.join("provider-client-stops")
                .join(format!("{}.json", fixture.child.id()))
                .display(),
        ));
        assert!(replace_provider_clients(
            &dir,
            "fake-provider",
            &clients,
            ProviderClientStopReason::Moved
        )
        .is_err());
        assert!(fixture.child.try_wait().unwrap().is_none());
        assert!(!crate::session_record::read_provider_clients(&dir)
            .unwrap()
            .is_empty());
        assert_eq!(
            crate::session_record::read_provider_clients(&dir)
                .unwrap()
                .len(),
            1
        );

        std::env::set_var("PATH", original_path);
        replace_provider_clients(
            &dir,
            "fake-provider",
            &clients,
            ProviderClientStopReason::Moved,
        )
        .unwrap();
        assert!(!fixture.child.wait().unwrap().success());
        assert!(crate::session_record::read_provider_clients(&dir)
            .unwrap()
            .is_empty());
        assert_eq!(
            crate::session_record::read_provider_session(&dir)
                .unwrap()
                .unwrap()
                .provider_session_id,
            "native-history"
        );
    }

    #[test]
    fn provider_client_stop_confirms_exit_after_ignored_termination() {
        let _lock = crate::journal::test_env_lock();
        let mut fixture = NativeClient::new();
        let dir = fixture.capture.artifact_dir();
        let clients = active_provider_clients(&dir, "fake-provider").unwrap();
        replace_provider_clients(
            &dir,
            "fake-provider",
            &clients,
            ProviderClientStopReason::Moved,
        )
        .unwrap();
        assert!(!fixture.child.wait().unwrap().success());
        assert!(active_provider_clients(&dir, "fake-provider")
            .unwrap()
            .is_empty());
        assert!(crate::session_record::read_provider_clients(&dir)
            .unwrap()
            .is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn provider_client_move_retains_history_published_after_client_exit() {
        let _environment = crate::journal::test_env_lock();
        let mut fixture = NativeClient::new();
        let dir = fixture.capture.artifact_dir();
        assert!(crate::session_record::read_provider_session(&dir)
            .unwrap()
            .is_none());

        let clients = active_provider_clients(&dir, "fake-provider").unwrap();
        replace_provider_clients(
            &dir,
            "fake-provider",
            &clients,
            ProviderClientStopReason::Moved,
        )
        .unwrap();
        assert!(!fixture.child.wait().unwrap().success());
        // The startup observer may drain buffered logs after stop has returned.
        observe_opencode_session(
            &dir,
            &b"level=INFO message=created id=ses_delayed directory=/tmp/repo\n"[..],
        )
        .unwrap();

        assert_eq!(
            crate::session_record::read_provider_session(&dir)
                .unwrap()
                .unwrap()
                .provider_session_id,
            "ses_delayed"
        );
        assert!(crate::session_record::read_provider_clients(&dir)
            .unwrap()
            .is_empty());
        assert!(!dir.join("session-resolution.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn intentional_session_move_exits_cleanly() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempfile::tempdir().unwrap();
        let _home = EnvRestore::capture(&["LF_HOME"]);
        std::env::set_var("LF_HOME", temp.path());
        let provider = fake_provider(&temp, "trap 'exit 143' TERM\ni=0; while [ \"$i\" -lt 100 ]; do sleep 0.05; i=$((i + 1)); done");
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
        let capture_dir = capture.artifact_dir();
        let stop_dir = capture_dir.clone();
        let lock_path = temp.path().join("launch.lock");
        let launch_lock = File::create(&lock_path).unwrap();
        fs2::FileExt::lock_exclusive(&launch_lock).unwrap();
        let stop = std::thread::spawn(move || {
            let lock = File::open(lock_path).unwrap();
            fs2::FileExt::lock_exclusive(&lock).unwrap();
            // Release must follow receipt publication but precede provider exit.
            let clients = crate::session_record::read_provider_clients(&stop_dir).unwrap();
            assert_eq!(
                clients.len(),
                1,
                "launch exclusion ended without a live receipt"
            );
            let pid = clients[0].pid;
            replace_provider_clients(
                &stop_dir,
                "fake-provider",
                &clients,
                ProviderClientStopReason::Moved,
            )
            .unwrap();
            pid
        });
        let command = SessionCommand {
            program: provider.display().to_string(),
            args: Vec::new(),
            cwd: temp.path().to_path_buf(),
        };

        let result = spawn_session_command_with_env(
            &command,
            &capture.environment(),
            None,
            None,
            Some(launch_lock),
        );
        let pid = stop.join().unwrap();

        assert!(result.is_ok());
        assert_eq!(
            provider_client_stop_message(ProviderClientStopReason::Moved),
            "Session moved to another terminal."
        );
        assert_eq!(
            provider_client_stop_message(ProviderClientStopReason::Completed),
            "Session completed elsewhere."
        );
        assert_eq!(
            crate::session_record::read_provider_client_stop(&capture_dir, pid).unwrap(),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn provider_sigterm_without_stop_intent_remains_an_error() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempfile::tempdir().unwrap();
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
            program: provider.display().to_string(),
            args: Vec::new(),
            cwd: temp.path().to_path_buf(),
        };

        let error =
            spawn_session_command_with_env(&command, &capture.environment(), None, None, None)
                .expect_err("unexplained SIGTERM must remain an error");

        assert!(error.to_string().contains("signal: 15"));
        assert!(
            crate::session_record::read_provider_clients(&capture.artifact_dir())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn session_launch_tui_codex_sets_worktree_model_and_prompt() {
        let launch = build_session_command("codex", Some("o3"), &path(), "fix it", None, None)
            .expect("build launch");

        assert_eq!(launch.program, "codex");
        assert_eq!(launch.cwd, path());
        assert!(launch
            .args
            .starts_with(&args(&["-C", "/tmp/loop flow", "-c", "model=\"o3\""])));
        assert_eq!(launch.args.last().map(String::as_str), Some("fix it"));
        assert_eq!(
            launch.args.contains(&"--sandbox".to_string()),
            crate::engine::codex_permission_args(Some(&path()), false, false)
                .contains(&"--sandbox".to_string())
        );
    }

    #[test]
    fn bare_tui_harnesses_do_not_select_a_model() {
        for agent in ["claude", "codex", "opencode"] {
            let (harness, model) = crate::engine::parse_agent(agent);
            let launch =
                build_session_command(&harness, model.as_deref(), &path(), "test", None, None)
                    .expect("build bare harness launch");
            assert!(
                !launch
                    .args
                    .iter()
                    .any(|arg| { arg == "--model" || arg == "-m" || arg.starts_with("model=") }),
                "bare {agent} selected a model: {:?}",
                launch.args
            );
        }
    }

    #[test]
    fn session_launch_tui_codex_adds_main_repo_for_worktree_metadata() {
        let (_tmp, main, worktree) = git_worktree_fixture();

        let launch = build_session_command("codex", None, &worktree, "fix it", None, None)
            .expect("build launch");

        let idx = launch
            .args
            .iter()
            .position(|arg| arg == "--add-dir")
            .expect("add-dir flag");
        assert_eq!(
            PathBuf::from(&launch.args[idx + 1]).canonicalize().unwrap(),
            main.canonicalize().unwrap()
        );
    }

    #[test]
    fn session_launch_tui_claude_runs_in_worktree_with_model_and_prompt() {
        let launch = build_session_command("claude", Some("sonnet"), &path(), "fix it", None, None)
            .expect("build launch");

        assert_eq!(
            launch,
            SessionCommand {
                program: "claude".to_string(),
                args: args(&["--model", "sonnet", "--", "fix it"]),
                cwd: path(),
            }
        );
    }

    #[test]
    fn session_launch_tui_claude_assigns_a_resumable_provider_session() {
        let launch = build_session_command(
            "claude",
            None,
            &path(),
            "test",
            Some("01234567-89ab-cdef-0123-456789abcdef"),
            None,
        )
        .expect("build launch");

        assert_eq!(
            launch.args,
            args(&[
                "--session-id",
                "01234567-89ab-cdef-0123-456789abcdef",
                "--",
                "test",
            ])
        );
    }

    #[cfg(unix)]
    #[test]
    fn preferred_name_resume_opens_every_provider_without_a_prompt() {
        let temp = tempfile::tempdir().unwrap();
        let provider = fake_provider(&temp, "for arg do printf '%s\\0' \"$arg\"; done > received");
        for harness in ["claude", "codex", "opencode"] {
            let command =
                build_resume_session_command(harness, None, temp.path(), "recorded-session")
                    .unwrap();
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
        let _lock = crate::journal::test_env_lock();
        let temp = tempfile::tempdir().unwrap();
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
        crate::session_record::write_provider_session(&capture_dir, "ses_original", None).unwrap();
        let session = crate::session_record::read_provider_session(&capture_dir)
            .unwrap()
            .unwrap();
        for (saved, forwarded, expected) in [
            ("Jack", None, Some("Jack")),
            ("Maya", None, Some("Maya")),
            ("Host Owner", Some("Jack"), Some("Jack")),
            ("Host Owner", Some(""), Some("Host Owner")),
        ] {
            std::fs::write(
                temp.path().join("config.yaml"),
                format!("user:\n  name: {saved}\n"),
            )
            .unwrap();
            match forwarded {
                Some(name) => std::env::set_var("LF_USER_NAME", name),
                None => std::env::remove_var("LF_USER_NAME"),
            }
            resume_session_with_env(
                "opencode",
                None,
                temp.path(),
                &capture.artifact_key(),
                &session,
                &BTreeMap::new(),
                None,
                None,
            )
            .unwrap();
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
        let _lock = crate::journal::test_env_lock();
        let temp = tempfile::tempdir().unwrap();
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
                .map(|session| session.provider_session_id),
            Some("ses_native".to_string())
        );
    }

    #[test]
    fn session_launch_tui_claude_adds_main_repo_for_worktree_metadata() {
        let (_tmp, main, worktree) = git_worktree_fixture();

        let launch =
            build_session_command("claude", Some("sonnet"), &worktree, "fix it", None, None)
                .expect("build launch");

        let idx = launch
            .args
            .iter()
            .position(|arg| arg == "--add-dir")
            .expect("add-dir flag");
        assert_eq!(
            PathBuf::from(&launch.args[idx + 1]).canonicalize().unwrap(),
            main.canonicalize().unwrap()
        );
        assert!(launch.args.ends_with(&args(&["--", "fix it"])));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn session_launch_tui_claude_signs_its_native_home_in_as_a_healthy_managed_login() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempfile::tempdir().unwrap();
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
            "claude",
            None,
            temp.path(),
            "review it",
            &BTreeMap::new(),
            None,
            &[],
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
        let _lock = crate::journal::test_env_lock();
        let temp = tempfile::tempdir().unwrap();
        let _restore = EnvRestore::capture(&[
            "LF_HOME",
            "LF_TEST_SESSION_ENV",
            "OPENCODE_API_KEY",
            "CODEX_ACCESS_TOKEN",
            "PATH",
        ]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::set_var("OPENCODE_API_KEY", "ambient-key");
        std::env::remove_var("CODEX_ACCESS_TOKEN");

        let bin = temp.path().join("bin");
        std::fs::create_dir(&bin).unwrap();
        let opencode = bin.join("opencode");
        std::fs::write(
            &opencode,
            "#!/bin/sh\nprintf '%s' \"$OPENCODE_API_KEY\" > \"$LF_TEST_SESSION_ENV\"\n",
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
            "opencode",
            None,
            temp.path(),
            "review it",
            &BTreeMap::new(),
            None,
            &[],
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
        launch_session(
            "codex",
            None,
            temp.path(),
            "review it",
            &BTreeMap::new(),
            None,
            &[],
            None,
        )
        .unwrap();
    }

    #[test]
    fn session_launch_tui_opencode_sets_worktree_prompt_and_model() {
        let launch = build_session_command(
            "opencode",
            Some("moonshotai/kimi-k2"),
            &path(),
            "fix it",
            None,
            None,
        )
        .expect("build launch");

        assert_eq!(
            launch,
            SessionCommand {
                program: "opencode".to_string(),
                args: args(&[
                    "/tmp/loop flow",
                    "--prompt",
                    "fix it",
                    "--model",
                    "moonshotai/kimi-k2",
                ]),
                cwd: path(),
            }
        );
    }

    #[test]
    fn process_elapsed_time_accepts_ps_formats() {
        assert_eq!(elapsed_seconds("02:03"), Some(123));
        assert_eq!(elapsed_seconds("01:02:03"), Some(3_723));
        assert_eq!(elapsed_seconds("2-01:02:03"), Some(176_523));
    }
}
