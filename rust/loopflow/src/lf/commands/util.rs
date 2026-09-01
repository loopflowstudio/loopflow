use anyhow::{anyhow, bail, Context, Result};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};

use crate::engine::{check_cli_available, codex_permission_args, workspace_add_dirs, LaunchTarget};
use crate::provider_auth::Provider;
use crate::run_record::ProviderClientRef;

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

/// Message text from the args (joined) or stdin (heredoc-friendly). The
/// Message commands take text arguments or read stdin when omitted.
pub(crate) fn message_text(args: &[String], mut stdin: impl Read) -> Result<String> {
    let joined = args.join(" ").trim().to_string();
    if !joined.is_empty() {
        return Ok(joined);
    }
    let mut buffer = String::new();
    stdin.read_to_string(&mut buffer)?;
    let text = buffer.trim().to_string();
    if text.is_empty() {
        bail!("no message text: pass TEXT or pipe it on stdin");
    }
    Ok(text)
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

    spawn_session_command_with_env(&launch.command, environment, provider_session_id, None)
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
    run_id: &crate::durable::RunId,
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
    )
}

pub(crate) fn resume_session_with_env(
    harness: &str,
    model: Option<&str>,
    worktree: &Path,
    run_id: &crate::durable::RunId,
    run_dir: &Path,
    provider_session: &crate::run_record::ProviderSessionRef,
    extra_environment: &BTreeMap<String, String>,
) -> Result<()> {
    let command = build_resume_session_command(
        harness,
        model,
        worktree,
        &provider_session.provider_session_id,
    )?;
    let mut environment = BTreeMap::from([
        (crate::durable::RUN_ID_ENV.to_string(), run_id.to_string()),
        (
            crate::run_record::RUN_DIR_ENV.to_string(),
            run_dir.display().to_string(),
        ),
    ]);
    environment.extend(extra_environment.clone());
    spawn_session_command_with_env(
        &command,
        &environment,
        Some(&provider_session.provider_session_id),
        provider_session.account_id.as_ref(),
    )
}

pub(crate) fn active_provider_clients(dir: &Path, harness: &str) -> Result<Vec<ProviderClientRef>> {
    let clients = crate::run_record::read_provider_clients(dir)
        .map_err(|error| anyhow!("cannot read provider clients: {error}"))?;
    Ok(clients
        .into_iter()
        .filter(|client| provider_client_is_live(client, harness))
        .collect())
}

pub(crate) fn replace_provider_clients(
    dir: &Path,
    harness: &str,
    clients: &[ProviderClientRef],
) -> Result<()> {
    for client in clients {
        signal_provider_client(client.pid, libc::SIGTERM)?;
    }
    for _ in 0..20 {
        if clients
            .iter()
            .all(|client| !provider_client_is_live(client, harness))
        {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    for client in clients
        .iter()
        .filter(|client| provider_client_is_live(client, harness))
    {
        signal_provider_client(client.pid, libc::SIGKILL)?;
    }
    for _ in 0..20 {
        if clients
            .iter()
            .all(|client| !provider_client_is_live(client, harness))
        {
            for client in clients {
                crate::run_record::remove_provider_client(dir, client.pid)?;
            }
            return Ok(());
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    bail!("the existing provider client did not exit; resume was not started")
}

fn provider_client_is_live(client: &ProviderClientRef, harness: &str) -> bool {
    let output = match Command::new("ps")
        .args([
            "-p",
            &client.pid.to_string(),
            "-o",
            "etime=",
            "-o",
            "command=",
        ])
        .output()
    {
        Ok(output) if output.status.success() => output,
        _ => return false,
    };
    let line = String::from_utf8_lossy(&output.stdout);
    let mut fields = line.split_whitespace();
    let Some(elapsed) = fields.next().and_then(elapsed_seconds) else {
        return false;
    };
    let command = fields.collect::<Vec<_>>().join(" ");
    let expected_start = OffsetDateTime::now_utc().unix_timestamp() - elapsed as i64;
    if (expected_start - client.started_at.unix_timestamp()).abs() > 5 {
        return false;
    }
    command.split_whitespace().any(|word| {
        Path::new(word)
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == harness || name.starts_with(&format!("{harness}-")))
            || word.contains(&format!("/{harness}"))
    })
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
            args.push(provider_session_id.to_string());
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
) -> Result<()> {
    let status = session_command_status_with_env(
        command,
        environment,
        provider_session_id,
        exact_account_id,
    )?;
    if status.success() {
        Ok(())
    } else if provider_session_id.is_some() {
        Err(anyhow!(
            "{} could not open this session (status {status}). If another client still owns it, close that client or use `lf session open --replace` for a Loopflow-owned client.",
            command.program
        ))
    } else {
        Err(anyhow!("session launcher exited with status {status}"))
    }
}

fn session_command_status_with_env(
    command: &SessionCommand,
    environment: &BTreeMap<String, String>,
    provider_session_id: Option<&str>,
    exact_account_id: Option<&crate::store::ProviderAccountId>,
) -> Result<std::process::ExitStatus> {
    if !check_cli_available(&command.program) {
        return Err(anyhow!(
            "'{}' CLI not found. Install it and rerun `lf init`.",
            command.program
        ));
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
    if command.program == "opencode"
        && provider_session_id.is_none()
        && environment.contains_key(crate::run_record::RUN_DIR_ENV)
    {
        process.args(["--print-logs", "--log-level", "INFO"]);
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
    if command.program == "opencode"
        && provider_session_id.is_none()
        && environment.contains_key(crate::run_record::RUN_DIR_ENV)
    {
        return run_opencode_with_session_observer(process, environment);
    }
    let mut child = process.spawn()?;
    let _client = match ProviderClientGuard::publish(environment, child.id()) {
        Ok(client) => client,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    child.wait().map_err(Into::into)
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
}

impl Drop for ProviderClientGuard {
    fn drop(&mut self) {
        if let Err(error) = crate::run_record::remove_provider_client(&self.run_dir, self.pid) {
            tracing::warn!(pid = self.pid, %error, "failed to clear provider client receipt");
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

fn run_opencode_with_session_observer(
    mut process: Command,
    environment: &BTreeMap<String, String>,
) -> Result<std::process::ExitStatus> {
    let run_dir = PathBuf::from(
        environment
            .get(crate::run_record::RUN_DIR_ENV)
            .expect("OpenCode observer requires a Run directory"),
    );
    process.stderr(Stdio::piped());
    let mut child = process.spawn()?;
    let _client = match ProviderClientGuard::publish(environment, child.id()) {
        Ok(client) => client,
        Err(error) => {
            let _ = child.kill();
            let _ = child.wait();
            return Err(error);
        }
    };
    let stderr = child
        .stderr
        .take()
        .expect("piped OpenCode stderr is available");
    let observer = std::thread::spawn(move || -> std::io::Result<()> {
        let mut write_error = None;
        let mut observed = false;
        for line in BufReader::new(stderr).lines() {
            let line = line?;
            if !observed {
                if let Some(provider_session_id) = parse_opencode_session_id(&line) {
                    observed = true;
                    if let Err(error) = crate::run_record::write_provider_session(
                        &run_dir,
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
    });
    let status = child.wait()?;
    observer
        .join()
        .map_err(|_| anyhow!("OpenCode session observer panicked"))??;
    Ok(status)
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profile::EmailAddress;
    use crate::provider_account::new_account;
    use crate::store::{
        open_store, CredentialType, ProviderAccountId, ProviderToken, StorageConfig,
        CONTROL_DB_PATH_ENV, CONTROL_HOME_ENV,
    };
    use std::ffi::OsString;

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

    #[test]
    fn message_text_prefers_args_then_stdin_then_errors() {
        let text = message_text(&["hello".into(), "world".into()], std::io::empty()).unwrap();
        assert_eq!(text, "hello world");

        let text = message_text(&[], std::io::Cursor::new("from stdin\n")).unwrap();
        assert_eq!(text, "from stdin");

        let err = message_text(&[], std::io::empty()).unwrap_err();
        assert!(err.to_string().contains("no message text"));
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

    #[test]
    fn claude_resume_reopens_the_native_session_without_a_prompt() {
        let command = build_resume_session_command(
            "claude",
            None,
            &path(),
            "01234567-89ab-cdef-0123-456789abcdef",
        )
        .expect("build resume");

        assert_eq!(
            command,
            SessionCommand {
                program: "claude".to_string(),
                args: args(&["--resume", "01234567-89ab-cdef-0123-456789abcdef"]),
                cwd: path(),
            }
        );
    }

    #[test]
    fn codex_and_opencode_resume_the_recorded_native_session() {
        let codex = build_resume_session_command(
            "codex",
            None,
            &path(),
            "019c57d6-5c06-7a93-8000-0123456789ab",
        )
        .expect("build Codex resume");
        assert_eq!(codex.program, "codex");
        assert_eq!(codex.args.first().map(String::as_str), Some("resume"));
        assert_eq!(
            codex.args.last().map(String::as_str),
            Some("019c57d6-5c06-7a93-8000-0123456789ab")
        );

        let opencode =
            build_resume_session_command("opencode", None, &path(), "ses_0123456789abcdef")
                .expect("build OpenCode resume");
        assert_eq!(
            opencode,
            SessionCommand {
                program: "opencode".to_string(),
                args: args(&["/tmp/loop flow", "--session", "ses_0123456789abcdef"]),
                cwd: path(),
            }
        );
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
            },
        )
        .unwrap();
        let command = SessionCommand {
            program: "opencode".to_string(),
            args: vec![temp.path().display().to_string()],
            cwd: temp.path().to_path_buf(),
        };

        let status =
            session_command_status_with_env(&command, &capture.environment(), None, None).unwrap();

        assert!(status.success());
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

        let store = open_store(&StorageConfig::sqlite(temp.path().join("loopflow.db")))
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
    async fn session_launch_tui_opencode_uses_the_stored_zen_credential() {
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
            "PATH",
        ]);
        std::env::set_var("LF_HOME", temp.path());
        std::env::remove_var("LF_DB_PATH");
        std::env::remove_var(CONTROL_HOME_ENV);
        std::env::remove_var(CONTROL_DB_PATH_ENV);
        std::env::remove_var("LF_ACCOUNT_LEASE");
        std::env::set_var("OPENCODE_API_KEY", "ambient-key");

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

        let store = open_store(&StorageConfig::sqlite(temp.path().join("loopflow.db")))
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
