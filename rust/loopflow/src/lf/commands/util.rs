use anyhow::{anyhow, bail, Context, Result};
use fs2::FileExt;
use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};

use crate::engine::{
    check_cli_available, codex_permission_args, missing_agent_message, workspace_add_dirs,
    LaunchTarget,
};
use crate::provider_auth::Provider;
use crate::run_record::{ProviderClientRef, ProviderClientStopReason};
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionLaunch {
    command: SessionCommand,
    ide_url: Option<String>,
}

pub fn launch_session(
    target: LaunchTarget,
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    prompt: &str,
) -> Result<()> {
    launch_session_with_env(
        target,
        harness,
        model,
        worktree,
        prompt,
        &BTreeMap::new(),
        None,
    )
}

pub(crate) fn launch_session_with_env(
    target: LaunchTarget,
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    prompt: &str,
    environment: &BTreeMap<String, String>,
    provider_session_id: Option<&str>,
) -> Result<()> {
    let launch = build_session_launch(
        target,
        harness,
        model,
        worktree,
        prompt,
        provider_session_id,
    )?;

    if target == LaunchTarget::Ide {
        if let Some(url) = launch.ide_url.as_deref() {
            match crate::engine::platform::open_url_checked(url) {
                Ok(()) => return Ok(()),
                Err(err) => eprintln!("Could not open vendor app ({err}); falling back to TUI."),
            }
        } else if harness == "opencode" {
            eprintln!("OpenCode has no standalone app; opening the TUI.");
        }
    }

    spawn_session_command_with_env(
        &launch.command,
        environment,
        provider_session_id,
        None,
        None,
    )
}

fn build_session_launch(
    target: LaunchTarget,
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    prompt: &str,
    provider_session_id: Option<&str>,
) -> Result<SessionLaunch> {
    let worktree = absolute_path(worktree);
    let command = build_session_command(harness, model, &worktree, prompt, provider_session_id)?;
    let ide_url = if target == LaunchTarget::Ide {
        build_ide_url(harness, &worktree, prompt)
    } else {
        None
    };

    Ok(SessionLaunch { command, ide_url })
}

fn build_ide_url(harness: &str, worktree: &Path, prompt: &str) -> Option<String> {
    let worktree = percent_encode(&worktree.to_string_lossy());
    let prompt = percent_encode(prompt);
    match harness {
        "codex" => Some(format!(
            "codex://threads/new?path={worktree}&prompt={prompt}"
        )),
        "claude" => Some(format!("claude://code/new?folder={worktree}&q={prompt}")),
        "opencode" => None,
        _ => None,
    }
}

pub(crate) fn build_session_command(
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    prompt: &str,
    provider_session_id: Option<&str>,
) -> Result<SessionCommand> {
    let cwd = worktree.to_path_buf();
    let worktree_arg = worktree.to_string_lossy().to_string();

    match harness {
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
            args.push(prompt.to_string());
            Ok(SessionCommand {
                program: "codex".to_string(),
                args,
                cwd,
            })
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
            // Claude's variadic --add-dir otherwise consumes the positional prompt.
            args.push("--".to_string());
            args.push(prompt.to_string());
            Ok(SessionCommand {
                program: "claude".to_string(),
                args,
                cwd,
            })
        }
        "opencode" => {
            let mut args = vec![worktree_arg, "--prompt".to_string(), prompt.to_string()];
            if let Some(model) = model {
                args.push("--model".to_string());
                args.push(model.to_string());
            }
            Ok(SessionCommand {
                program: "opencode".to_string(),
                args,
                cwd,
            })
        }
        _ => Err(anyhow!(
            "unsupported session launcher harness '{}'. Use claude, codex, or opencode.",
            harness
        )),
    }
}

pub(crate) fn resume_session(
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    run_id: &String,
    run_dir: &Path,
    provider_session: &crate::run_record::ProviderSessionRef,
) -> Result<()> {
    resume_session_with_env(
        harness,
        model,
        worktree,
        run_id,
        run_dir,
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
    run_id: &String,
    run_dir: &Path,
    provider_session: &crate::run_record::ProviderSessionRef,
    extra_environment: &BTreeMap<String, String>,
    launch_lock: Option<File>,
    remote: Option<&Path>,
) -> Result<()> {
    let user_name = crate::engine::config::launch_user_name()?;
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
    let mut environment = BTreeMap::from([
        (crate::durable::RUN_ID_ENV.to_string(), run_id.to_string()),
        (
            crate::run_record::RUN_DIR_ENV.to_string(),
            run_dir.display().to_string(),
        ),
    ]);
    environment.extend(extra_environment.clone());
    environment.insert(
        crate::engine::config::USER_NAME_ENV.to_string(),
        user_name.unwrap_or_default(),
    );
    spawn_session_command_with_env(
        &command,
        &environment,
        Some(&provider_session.provider_session_id),
        provider_session.account_id.as_ref(),
        launch_lock,
    )
}

pub(crate) fn active_provider_clients(dir: &Path, harness: &str) -> Result<Vec<ProviderClientRef>> {
    let clients = crate::run_record::read_provider_clients(dir)
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

pub(crate) fn stop_provider_session(dir: &Path, harness: &str) -> Result<()> {
    let _launch = lock_provider_clients(dir)?;
    let clients = active_provider_clients(dir, harness)?;
    replace_provider_clients_locked(dir, harness, &clients, ProviderClientStopReason::Completed)?;
    Ok(())
}

pub(crate) fn require_provider_session_launch(dir: &Path) -> Result<()> {
    let input = crate::run_record::input_id_from_dir(dir)?;
    let store =
        SqliteStore::open_run_ledger_read_only(&crate::store::observability_database_path()?)?;
    let session = store
        .session_for_artifact(&input)?
        .ok_or_else(|| anyhow!("Input {input} is not recorded on this Home"))?;
    let Some(task_id) = session.task_id else {
        return Ok(());
    };
    let task = store
        .task_by_issue(task_id.as_str())?
        .ok_or_else(|| anyhow!("Task {task_id} is not registered"))?;
    if store
        .task_deletion(&task.wave_id, task.plan.id.as_str())?
        .is_some()
    {
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
        crate::run_record::write_provider_client_stop(dir, client.pid, reason)
            .context("record why the provider client is stopping")?;
        if let Err(error) = signal_provider_client(client.pid, libc::SIGTERM) {
            let _ = crate::run_record::remove_provider_client_stop(dir, client.pid);
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
                crate::run_record::remove_provider_client(dir, client.pid)?;
            }
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    bail!("the existing provider client did not exit; resume was not started")
}

fn require_unchanged_provider_clients(dir: &Path, clients: &[ProviderClientRef]) -> Result<()> {
    let current = crate::run_record::read_provider_clients(dir)?;
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
    Ok(crate::run_record::provider_client_matches(
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
            "{} could not open this session (status {}). If another client still owns it, close that client or use `lf session open --replace` for a Loopflow-owned client.",
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
        ProviderClientStopReason::Moved => "Session moved to another terminal.",
        ProviderClientStopReason::Completed => "Session completed elsewhere.",
    }
}

#[derive(Debug)]
struct SessionCommandOutcome {
    status: std::process::ExitStatus,
    stop_reason: Option<ProviderClientStopReason>,
}

fn session_command_status_with_env(
    command: &SessionCommand,
    environment: &BTreeMap<String, String>,
    provider_session_id: Option<&str>,
    exact_account_id: Option<&crate::store::ProviderAccountId>,
    launch_lock: Option<File>,
) -> Result<SessionCommandOutcome> {
    // Keep admission and client publication on the same side of Session stop.
    // Release before waiting for the child, so stop can settle that client.
    let launch = environment
        .get(crate::run_record::RUN_DIR_ENV)
        .map(|dir| -> Result<File> {
            let dir = Path::new(dir);
            let launch = lock_provider_clients(dir)?;
            require_provider_session_launch(dir)?;
            Ok(launch)
        })
        .transpose()?;
    if !check_cli_available(&command.program) {
        return Err(anyhow!(missing_agent_message(&command.program)));
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
                provider_session_id.map(str::to_string),
                exact_account_id.cloned(),
            )
        })
        .transpose()
        .map_err(|error| anyhow!("failed to select provider account: {error}"))?
        .flatten();

    let mut process = Command::new(&command.program);
    if provider == Some(Provider::Codex)
        && account_route
            .as_ref()
            .is_some_and(crate::provider_account::ProviderAccountRoute::uses_native_home)
    {
        process.args(["-c", "cli_auth_credentials_store=\"file\""]);
    }
    if command.program == "codex"
        && provider_session_id.is_none()
        && environment.contains_key(crate::run_record::RUN_DIR_ENV)
    {
        let hook = codex_session_start_hook()?;
        process.args(["--dangerously-bypass-hook-trust", "-c", &hook]);
    }
    let observed_run = environment
        .get(crate::run_record::RUN_DIR_ENV)
        .filter(|_| command.program == "opencode" && provider_session_id.is_none())
        .map(PathBuf::from);
    if observed_run.is_some() {
        process.args(["--print-logs", "--log-level", "INFO"]);
        process.stderr(Stdio::piped());
    }
    process
        .args(&command.args)
        .current_dir(&command.cwd)
        .env_remove("LOOPFLOW_DIRECTIVE_FILE")
        .envs(environment);
    crate::provider_auth::apply_provider_env_to_command(&command.program, &mut process);
    if let Some(route) = &account_route {
        tracing::info!(provider = %command.program, "selected managed provider account");
        route.apply(&mut process);
        process.env(
            crate::run_record::PROVIDER_ACCOUNT_ID_ENV,
            route.account_id().as_str(),
        );
        route.record_launch_blocking(provider_session_id.map(str::to_string), None)?;
    }
    if let (Some(run_dir), Some(provider_session_id)) = (
        environment.get(crate::run_record::RUN_DIR_ENV),
        provider_session_id,
    ) {
        crate::run_record::write_provider_session(
            Path::new(run_dir),
            provider_session_id,
            account_route
                .as_ref()
                .map(|route| route.account_id().clone()),
        )?;
    }
    let mut child = process.spawn()?;
    let client = match ProviderClientGuard::publish(environment, child.id()) {
        Ok(client) => client,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    // Completion and another Open can proceed once exact client ownership is visible.
    drop(launch_lock);
    drop(launch);
    let observer = observed_run.map(|run_dir| {
        let stderr = child
            .stderr
            .take()
            .expect("piped OpenCode stderr is available");
        std::thread::spawn(move || observe_opencode_session(&run_dir, stderr))
    });
    let status = child.wait()?;
    if let Some(observer) = observer {
        observer
            .join()
            .map_err(|_| anyhow!("OpenCode session observer panicked"))??;
    }
    let stop_reason = client
        .as_ref()
        .map(ProviderClientGuard::take_stop_reason)
        .transpose()?
        .flatten();
    Ok(SessionCommandOutcome {
        status,
        stop_reason,
    })
}

#[derive(Debug)]
struct ProviderClientGuard {
    run_dir: PathBuf,
    pid: u32,
}

impl ProviderClientGuard {
    fn publish(environment: &BTreeMap<String, String>, pid: u32) -> Result<Option<Self>> {
        let Some(run_dir) = environment.get(crate::run_record::RUN_DIR_ENV) else {
            return Ok(None);
        };
        let run_dir = PathBuf::from(run_dir);
        crate::run_record::write_provider_client(&run_dir, pid)
            .map_err(|error| anyhow!("cannot record active provider client: {error}"))?;
        Ok(Some(Self { run_dir, pid }))
    }

    fn take_stop_reason(&self) -> Result<Option<ProviderClientStopReason>> {
        let reason = crate::run_record::read_provider_client_stop(&self.run_dir, self.pid)?;
        if reason.is_some() {
            crate::run_record::remove_provider_client_stop(&self.run_dir, self.pid)?;
        }
        Ok(reason)
    }
}

impl Drop for ProviderClientGuard {
    fn drop(&mut self) {
        if let Err(error) = crate::run_record::remove_provider_client(&self.run_dir, self.pid) {
            tracing::warn!(pid = self.pid, %error, "failed to clear provider client receipt");
        }
        if let Err(error) = crate::run_record::remove_provider_client_stop(&self.run_dir, self.pid)
        {
            tracing::warn!(pid = self.pid, %error, "failed to clear provider client stop");
        }
    }
}

fn codex_session_start_hook() -> Result<String> {
    let executable = std::env::current_exe()
        .map_err(|error| anyhow!("cannot resolve lf for Codex session capture: {error}"))?;
    Ok(codex_session_start_hook_for(&executable))
}

fn codex_session_start_hook_for(executable: &Path) -> String {
    let command = format!(
        "{} __provider-session",
        crate::engine::process::shell_escape(&executable.to_string_lossy())
    );
    let command = serde_json::to_string(&command).expect("shell command serializes as TOML string");
    format!(
        "hooks={{ SessionStart = [{{ matcher = \"startup\", hooks = [{{ type = \"command\", command = {command}, timeout = 5 }}] }}] }}"
    )
}

fn observe_opencode_session(run_dir: &Path, stderr: impl Read) -> std::io::Result<()> {
    let mut write_error = None;
    let mut observed = false;
    for line in BufReader::new(stderr).lines() {
        let line = line?;
        if !observed {
            if let Some(provider_session_id) = parse_opencode_session_id(&line) {
                observed = true;
                if let Err(error) =
                    crate::run_record::write_provider_session(run_dir, provider_session_id, None)
                {
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

fn percent_encode(value: &str) -> String {
    let mut encoded = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                encoded.push(byte as char);
            }
            _ => encoded.push_str(&format!("%{byte:02X}")),
        }
    }
    encoded
}

pub(crate) fn short_id(id: &str) -> String {
    id.chars().take(8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::EmailAddress;
    use crate::provider_account::new_account;
    use crate::store::{
        CredentialType, ProviderAccountId, ProviderToken, StorageConfig, CONTROL_DB_PATH_ENV,
        CONTROL_HOME_ENV,
    };
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
        capture: crate::run_record::CaptureHandle,
        temp: tempfile::TempDir,
        _environment: EnvRestore,
    }

    impl NativeClient {
        fn new() -> Self {
            let temp = tempfile::tempdir().unwrap();
            let names = [
                "LF_HOME",
                "LF_DB_PATH",
                "LF_CONTROL_HOME",
                "LF_CONTROL_DB_PATH",
                "LF_RUN_ID",
                "LF_RUN_DIR",
                "LF_RUN_CONTEXT",
                "LF_WORK_ADVANCE_CLAIM",
                "LF_TASK_ORIGIN",
                "LF_WAVE_ID",
                "LF_ACCOUNT_LEASE",
                "LF_HUMAN_SESSION",
            ];
            let environment = EnvRestore::capture(&names);
            for name in names {
                std::env::remove_var(name);
            }
            std::env::set_var("LF_HOME", temp.path());
            std::env::set_var("LF_DB_PATH", temp.path().join("loopflow.db"));
            let provider = fake_provider(
                &temp,
                "trap '' TERM\nprintf ready > \"$1\"\nwhile :; do /bin/sleep 0.05; done",
            );
            let capture = crate::run_record::CaptureHandle::begin_at(
                temp.path(),
                crate::run_record::RunSpec {
                    harness: "fake-provider".into(),
                    model: None,
                    surface: "tui".into(),
                    cwd: temp.path().to_path_buf(),
                    repo: None,
                    worktree: None,
                    skill: None,
                    subjects: Vec::new(),
                    work: None,
                    flow: crate::run_record::RunFlowMembership::Independent,
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
            crate::run_record::write_provider_client(
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
                ProviderClientStopReason::Completed,
            )
            .is_err());
            assert!(fixture.child.try_wait().unwrap().is_none());
            assert_eq!(
                crate::run_record::read_provider_clients(&dir).unwrap(),
                clients
            );
            assert!(
                crate::run_record::read_provider_client_stop(&dir, fixture.child.id())
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
            ProviderClientStopReason::Completed,
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
        let current = crate::run_record::read_provider_clients(&dir).unwrap();
        let receipt = dir
            .join("provider-clients")
            .join(format!("{}.json", fixture.child.id()));
        std::fs::write(&receipt, serde_json::to_vec(&clients[0]).unwrap()).unwrap();
        replace_provider_clients(
            &dir,
            "fake-provider",
            &clients,
            ProviderClientStopReason::Completed,
        )
        .unwrap();
        assert!(fixture.child.try_wait().unwrap().is_none());
        // A newly published receipt also survives an older caller's cleanup.
        std::fs::write(&receipt, serde_json::to_vec(&current[0]).unwrap()).unwrap();
        assert!(replace_provider_clients(
            &dir,
            "fake-provider",
            &clients,
            ProviderClientStopReason::Completed,
        )
        .is_err());
        assert!(fixture.child.try_wait().unwrap().is_none());
        assert_eq!(
            crate::run_record::read_provider_clients(&dir)
                .unwrap()
                .len(),
            1
        );
        assert!(
            crate::run_record::read_provider_client_stop(&dir, fixture.child.id())
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
            ProviderClientStopReason::Completed,
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
    fn session_stop_retries_unknown_native_client_without_resolving_history() {
        let _lock = crate::journal::test_env_lock();
        let _env =
            EnvRestore::capture(&["PATH", "LF_HOME", "LF_CONTROL_HOME", "LF_CONTROL_DB_PATH"]);
        let original_path = std::env::var_os("PATH").unwrap();
        let mut fixture = NativeClient::new();
        std::env::set_var("LF_HOME", fixture.temp.path());
        std::env::remove_var("LF_CONTROL_HOME");
        std::env::remove_var("LF_CONTROL_DB_PATH");
        let dir = fixture.capture.artifact_dir();
        crate::run_record::write_provider_session(&dir, "native-history", None).unwrap();
        let run = fixture.capture.run_id();
        fixture.mock_ps("echo unreadable fake-provider");
        assert!(crate::ops::human_session::stop_run(&run).is_err());
        assert!(fixture.child.try_wait().unwrap().is_none());
        assert!(!crate::run_record::read_provider_clients(&dir)
            .unwrap()
            .is_empty());
        assert_eq!(
            crate::run_record::read_provider_clients(&dir)
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
        assert!(crate::ops::human_session::stop_run(&run).is_err());
        assert!(fixture.child.try_wait().unwrap().is_none());
        assert!(!crate::run_record::read_provider_clients(&dir)
            .unwrap()
            .is_empty());
        assert_eq!(
            crate::run_record::read_provider_clients(&dir)
                .unwrap()
                .len(),
            1
        );

        std::env::set_var("PATH", original_path);
        crate::ops::human_session::stop_run(&run).unwrap();
        assert!(!fixture.child.wait().unwrap().success());
        assert!(crate::run_record::read_provider_clients(&dir)
            .unwrap()
            .is_empty());
        assert_eq!(
            crate::run_record::read_provider_session(&dir)
                .unwrap()
                .unwrap()
                .provider_session_id,
            "native-history"
        );
        crate::ops::human_session::stop_run(&run).unwrap();
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
            ProviderClientStopReason::Completed,
        )
        .unwrap();
        assert!(!fixture.child.wait().unwrap().success());
        assert!(active_provider_clients(&dir, "fake-provider")
            .unwrap()
            .is_empty());
        assert!(crate::run_record::read_provider_clients(&dir)
            .unwrap()
            .is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn session_stop_retains_history_published_after_client_exit() {
        let _environment = crate::journal::test_env_lock();
        let mut fixture = NativeClient::new();
        let dir = fixture.capture.artifact_dir();
        assert!(crate::run_record::read_provider_session(&dir)
            .unwrap()
            .is_none());

        stop_provider_session(&dir, "fake-provider").unwrap();
        assert!(!fixture.child.wait().unwrap().success());
        // The startup observer may drain buffered logs after stop has returned.
        observe_opencode_session(
            &dir,
            &b"level=INFO message=created id=ses_delayed directory=/tmp/repo\n"[..],
        )
        .unwrap();

        assert_eq!(
            crate::run_record::read_provider_session(&dir)
                .unwrap()
                .unwrap()
                .provider_session_id,
            "ses_delayed"
        );
        assert!(crate::run_record::read_provider_clients(&dir)
            .unwrap()
            .is_empty());
        assert!(!dir.join("session-resolution.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn session_stop_waits_for_native_client_publication() {
        let _environment = crate::journal::test_env_lock();
        let mut fixture = NativeClient::new();
        let dir = fixture.capture.artifact_dir();
        crate::run_record::write_provider_session(&dir, "retained-history", None).unwrap();
        let launch = lock_provider_clients(&dir).unwrap();
        // A live child exists, but its launcher has not published ownership yet.
        crate::run_record::remove_provider_client(&dir, fixture.child.id()).unwrap();
        let (entered, ready) = std::sync::mpsc::channel();
        let (finished, result) = std::sync::mpsc::channel();
        let stop_dir = dir.clone();
        let stop = std::thread::spawn(move || {
            entered.send(()).unwrap();
            finished
                .send(stop_provider_session(&stop_dir, "fake-provider"))
                .unwrap();
        });
        ready.recv().unwrap();
        let premature = result.recv_timeout(std::time::Duration::from_millis(100));
        crate::run_record::write_provider_client(&dir, fixture.child.id()).unwrap();
        drop(launch);
        let waited = matches!(premature, Err(std::sync::mpsc::RecvTimeoutError::Timeout));
        let stopped = match premature {
            Ok(value) => value,
            Err(_) => result
                .recv_timeout(std::time::Duration::from_secs(10))
                .unwrap(),
        };
        stop.join().unwrap();
        stopped.unwrap();
        assert!(waited);
        assert!(!fixture.child.wait().unwrap().success());
        assert!(active_provider_clients(&dir, "fake-provider")
            .unwrap()
            .is_empty());
        assert!(crate::run_record::read_provider_clients(&dir)
            .unwrap()
            .is_empty());
        let history = crate::run_record::read_provider_session(&dir)
            .unwrap()
            .unwrap();
        assert_eq!(history.provider_session_id, "retained-history");
        // Deliberate historical resumption remains supported for unrelated Runs.
        let provider = fake_provider(&fixture.temp, "touch resumed");
        let command = SessionCommand {
            program: provider.display().to_string(),
            args: Vec::new(),
            cwd: fixture.temp.path().to_path_buf(),
        };
        spawn_session_command_with_env(
            &command,
            &fixture.capture.environment(),
            Some(&history.provider_session_id),
            None,
            None,
        )
        .unwrap();
        assert!(fixture.temp.path().join("resumed").exists());
    }

    #[cfg(unix)]
    #[test]
    fn intentional_session_move_exits_cleanly() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempfile::tempdir().unwrap();
        let _home = EnvRestore::capture(&[
            "LF_HOME",
            "LF_DB_PATH",
            "LF_CONTROL_HOME",
            "LF_CONTROL_DB_PATH",
        ]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::set_var("LF_DB_PATH", temp.path().join("loopflow.db"));
        std::env::remove_var("LF_CONTROL_HOME");
        std::env::remove_var("LF_CONTROL_DB_PATH");
        let provider = fake_provider(&temp, "trap 'exit 143' TERM\ni=0; while [ \"$i\" -lt 100 ]; do sleep 0.05; i=$((i + 1)); done");
        let capture = crate::run_record::CaptureHandle::begin_at(
            temp.path(),
            crate::run_record::RunSpec {
                harness: "fake-provider".to_string(),
                model: None,
                surface: "tui".to_string(),
                cwd: temp.path().to_path_buf(),
                repo: None,
                worktree: None,
                skill: None,
                subjects: Vec::new(),
                flow: crate::run_record::RunFlowMembership::Independent,
                work: None,
            },
        )
        .unwrap();
        let run_dir = capture.artifact_dir();
        let stop_dir = run_dir.clone();
        let lock_path = temp.path().join("launch.lock");
        let launch_lock = File::create(&lock_path).unwrap();
        fs2::FileExt::lock_exclusive(&launch_lock).unwrap();
        let stop = std::thread::spawn(move || {
            let lock = File::open(lock_path).unwrap();
            fs2::FileExt::lock_exclusive(&lock).unwrap();
            // Release must follow receipt publication but precede provider exit.
            let clients = crate::run_record::read_provider_clients(&stop_dir).unwrap();
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
            crate::run_record::read_provider_client_stop(&run_dir, pid).unwrap(),
            None
        );
    }

    #[cfg(unix)]
    #[test]
    fn provider_sigterm_without_stop_intent_remains_an_error() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempfile::tempdir().unwrap();
        let _home = EnvRestore::capture(&[
            "LF_HOME",
            "LF_DB_PATH",
            "LF_CONTROL_HOME",
            "LF_CONTROL_DB_PATH",
        ]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::set_var("LF_DB_PATH", temp.path().join("loopflow.db"));
        std::env::remove_var("LF_CONTROL_HOME");
        std::env::remove_var("LF_CONTROL_DB_PATH");
        let provider = fake_provider(&temp, "kill -TERM $$");
        let capture = crate::run_record::CaptureHandle::begin_at(
            temp.path(),
            crate::run_record::RunSpec {
                harness: "fake-provider".to_string(),
                model: None,
                surface: "tui".to_string(),
                cwd: temp.path().to_path_buf(),
                repo: None,
                worktree: None,
                skill: None,
                subjects: Vec::new(),
                flow: crate::run_record::RunFlowMembership::Independent,
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
            crate::run_record::read_provider_clients(&capture.artifact_dir())
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn session_launch_tui_codex_sets_worktree_model_and_prompt() {
        let launch = build_session_launch(
            LaunchTarget::Tui,
            "codex",
            Some("o3"),
            &path(),
            "fix it",
            None,
        )
        .expect("build launch");

        assert_eq!(launch.command.program, "codex");
        assert_eq!(launch.command.cwd, path());
        assert!(launch.command.args.starts_with(&args(&[
            "-C",
            "/tmp/loop flow",
            "-c",
            "model=\"o3\""
        ])));
        assert_eq!(
            launch.command.args.last().map(String::as_str),
            Some("fix it")
        );
        assert_eq!(
            launch.command.args.contains(&"--sandbox".to_string()),
            crate::engine::codex_permission_args(Some(&path()), false, false)
                .contains(&"--sandbox".to_string())
        );
        assert_eq!(launch.ide_url, None);
    }

    #[test]
    fn bare_tui_harnesses_do_not_select_a_model() {
        for agent in ["claude", "codex", "opencode"] {
            let (harness, model) = crate::engine::parse_agent(agent);
            let launch = build_session_launch(
                LaunchTarget::Tui,
                &harness,
                model.as_deref(),
                &path(),
                "test",
                None,
            )
            .expect("build bare harness launch");
            assert!(
                !launch
                    .command
                    .args
                    .iter()
                    .any(|arg| { arg == "--model" || arg == "-m" || arg.starts_with("model=") }),
                "bare {agent} selected a model: {:?}",
                launch.command.args
            );
        }
    }

    #[test]
    fn session_launch_tui_codex_adds_main_repo_for_worktree_metadata() {
        let (_tmp, main, worktree) = git_worktree_fixture();

        let launch =
            build_session_launch(LaunchTarget::Tui, "codex", None, &worktree, "fix it", None)
                .expect("build launch");

        let idx = launch
            .command
            .args
            .iter()
            .position(|arg| arg == "--add-dir")
            .expect("add-dir flag");
        assert_eq!(
            PathBuf::from(&launch.command.args[idx + 1])
                .canonicalize()
                .unwrap(),
            main.canonicalize().unwrap()
        );
    }

    #[test]
    fn session_launch_tui_claude_runs_in_worktree_with_model_and_prompt() {
        let launch = build_session_launch(
            LaunchTarget::Tui,
            "claude",
            Some("sonnet"),
            &path(),
            "fix it",
            None,
        )
        .expect("build launch");

        assert_eq!(
            launch.command,
            SessionCommand {
                program: "claude".to_string(),
                args: args(&["--model", "sonnet", "--", "fix it"]),
                cwd: path(),
            }
        );
        assert_eq!(launch.ide_url, None);
    }

    #[test]
    fn session_launch_tui_claude_assigns_a_resumable_provider_session() {
        let launch = build_session_launch(
            LaunchTarget::Tui,
            "claude",
            None,
            &path(),
            "test",
            Some("01234567-89ab-cdef-0123-456789abcdef"),
        )
        .expect("build launch");

        assert_eq!(
            launch.command.args,
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
        let _restore = EnvRestore::capture(&[
            "LF_HOME",
            "LF_USER_NAME",
            "LF_DB_PATH",
            CONTROL_HOME_ENV,
            CONTROL_DB_PATH_ENV,
            "PATH",
        ]);
        std::env::set_var("LF_HOME", temp.path());
        for key in [
            "LF_USER_NAME",
            "LF_DB_PATH",
            CONTROL_HOME_ENV,
            CONTROL_DB_PATH_ENV,
        ] {
            std::env::remove_var(key);
        }
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
        let capture = crate::run_record::CaptureHandle::begin_at(
            temp.path(),
            crate::run_record::RunSpec {
                harness: "opencode".into(),
                model: None,
                surface: "tui".into(),
                cwd: temp.path().to_path_buf(),
                repo: None,
                worktree: None,
                skill: None,
                subjects: Vec::new(),
                flow: crate::run_record::RunFlowMembership::Independent,
                work: None,
            },
        )
        .unwrap();
        let run_dir = capture.artifact_dir();
        crate::run_record::write_provider_session(&run_dir, "ses_original", None).unwrap();
        let session = crate::run_record::read_provider_session(&run_dir)
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
                &capture.run_id(),
                &run_dir,
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
                crate::run_record::read_provider_session(&run_dir)
                    .unwrap()
                    .unwrap(),
                session
            );
        }
    }

    #[test]
    fn codex_session_start_hook_records_the_native_thread() {
        let hook = codex_session_start_hook_for(Path::new("/tmp/lf binary"));

        assert!(hook.contains("SessionStart"));
        assert!(hook.contains("matcher = \"startup\""));
        assert!(hook.contains("'/tmp/lf binary' __provider-session"));
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
        let _home = EnvRestore::capture(&[
            "LF_HOME",
            "LF_DB_PATH",
            "LF_CONTROL_HOME",
            "LF_CONTROL_DB_PATH",
        ]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::set_var("LF_DB_PATH", temp.path().join("loopflow.db"));
        std::env::remove_var("LF_CONTROL_HOME");
        std::env::remove_var("LF_CONTROL_DB_PATH");
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
        let capture = crate::run_record::CaptureHandle::begin_at(
            temp.path(),
            crate::run_record::RunSpec {
                harness: "opencode".to_string(),
                model: None,
                surface: "tui".to_string(),
                cwd: temp.path().to_path_buf(),
                repo: None,
                worktree: None,
                skill: None,
                subjects: Vec::new(),
                flow: crate::run_record::RunFlowMembership::Independent,
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
            crate::run_record::read_provider_session(&capture.artifact_dir())
                .unwrap()
                .map(|session| session.provider_session_id),
            Some("ses_native".to_string())
        );
    }

    #[test]
    fn session_launch_tui_claude_adds_main_repo_for_worktree_metadata() {
        let (_tmp, main, worktree) = git_worktree_fixture();

        let launch = build_session_launch(
            LaunchTarget::Tui,
            "claude",
            Some("sonnet"),
            &worktree,
            "fix it",
            None,
        )
        .expect("build launch");

        let idx = launch
            .command
            .args
            .iter()
            .position(|arg| arg == "--add-dir")
            .expect("add-dir flag");
        assert_eq!(
            PathBuf::from(&launch.command.args[idx + 1])
                .canonicalize()
                .unwrap(),
            main.canonicalize().unwrap()
        );
        assert!(launch.command.args.ends_with(&args(&["--", "fix it"])));
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn session_launch_tui_claude_uses_a_healthy_managed_login() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempfile::tempdir().unwrap();
        let _restore = EnvRestore::capture(&[
            "LF_HOME",
            "LF_DB_PATH",
            CONTROL_HOME_ENV,
            CONTROL_DB_PATH_ENV,
            "LF_ACCOUNT_LEASE",
            "LF_TEST_SESSION_ENV",
            "CLAUDE_CONFIG_DIR",
            "PATH",
        ]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::remove_var("LF_DB_PATH");
        std::env::remove_var(CONTROL_HOME_ENV);
        std::env::remove_var(CONTROL_DB_PATH_ENV);
        std::env::remove_var("LF_ACCOUNT_LEASE");
        std::env::set_var("CLAUDE_CONFIG_DIR", "ambient");

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
        let account = new_account(
            Provider::Claude,
            ProviderAccountId::parse("jackstah").unwrap(),
            account_home.clone(),
            Some(EmailAddress::parse("jackstah@gmail.com").unwrap()),
        );
        store.upsert_provider_account(&account).await.unwrap();

        launch_session(LaunchTarget::Tui, "claude", None, temp.path(), "review it").unwrap();

        assert_eq!(
            std::fs::read_to_string(capture).unwrap(),
            account_home.to_string_lossy()
        );
    }

    #[allow(clippy::await_holding_lock)]
    #[tokio::test]
    async fn session_launch_tui_preserves_native_oauth_and_routes_stored_api_keys() {
        let _lock = crate::journal::test_env_lock();
        let temp = tempfile::tempdir().unwrap();
        let _restore = EnvRestore::capture(&[
            "LF_HOME",
            "LF_DB_PATH",
            CONTROL_HOME_ENV,
            CONTROL_DB_PATH_ENV,
            "LF_ACCOUNT_LEASE",
            "LF_TEST_SESSION_ENV",
            "OPENCODE_API_KEY",
            "CODEX_ACCESS_TOKEN",
            "PATH",
        ]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::remove_var("LF_DB_PATH");
        std::env::remove_var(CONTROL_HOME_ENV);
        std::env::remove_var(CONTROL_DB_PATH_ENV);
        std::env::remove_var("LF_ACCOUNT_LEASE");
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
            LaunchTarget::Tui,
            "opencode",
            None,
            temp.path(),
            "review it",
        )
        .unwrap();

        assert_eq!(std::fs::read_to_string(capture).unwrap(), "stored-key");

        // The native CLI rejects an ordinary OAuth token in this agent-identity
        // variable. A prior `lf auth status` must not poison a working login.
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
        launch_session(LaunchTarget::Tui, "codex", None, temp.path(), "review it").unwrap();
    }

    #[test]
    fn session_launch_tui_opencode_sets_worktree_prompt_and_model() {
        let launch = build_session_launch(
            LaunchTarget::Tui,
            "opencode",
            Some("moonshotai/kimi-k2"),
            &path(),
            "fix it",
            None,
        )
        .expect("build launch");

        assert_eq!(
            launch.command,
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
        assert_eq!(launch.ide_url, None);
    }

    #[test]
    fn session_launch_ide_codex_builds_scheme_with_encoded_path_and_prompt() {
        let launch = build_session_launch(
            LaunchTarget::Ide,
            "codex",
            None,
            &path(),
            "fix & test\nnow",
            None,
        )
        .expect("build launch");

        assert_eq!(
            launch.ide_url.as_deref(),
            Some("codex://threads/new?path=%2Ftmp%2Floop%20flow&prompt=fix%20%26%20test%0Anow")
        );
        assert_eq!(launch.command.program, "codex");
    }

    #[test]
    fn session_launch_ide_claude_builds_code_scheme_with_encoded_folder_and_prompt() {
        let launch = build_session_launch(
            LaunchTarget::Ide,
            "claude",
            None,
            &path(),
            "fix & test\nnow",
            None,
        )
        .expect("build launch");

        assert_eq!(
            launch.ide_url.as_deref(),
            Some("claude://code/new?folder=%2Ftmp%2Floop%20flow&q=fix%20%26%20test%0Anow")
        );
        assert_eq!(launch.command.program, "claude");
    }

    #[test]
    fn session_launch_ide_opencode_falls_back_to_cli_shape() {
        let launch =
            build_session_launch(LaunchTarget::Ide, "opencode", None, &path(), "fix it", None)
                .expect("build launch");

        assert_eq!(launch.command.program, "opencode");
        assert_eq!(launch.ide_url, None);
    }

    #[test]
    fn process_elapsed_time_accepts_ps_formats() {
        assert_eq!(elapsed_seconds("02:03"), Some(123));
        assert_eq!(elapsed_seconds("01:02:03"), Some(3_723));
        assert_eq!(elapsed_seconds("2-01:02:03"), Some(176_523));
    }
}
