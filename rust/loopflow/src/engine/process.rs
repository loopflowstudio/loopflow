use std::path::{Path, PathBuf};
use std::process::{Child, ExitStatus};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{anyhow, Result};

pub(crate) const DISCORD_TOKEN_ENV: &str = "LF_DISCORD_TOKEN";

/// Wait for an owned child, returning its exit status and whether it timed out.
pub(crate) fn wait_for_exit(
    child: &mut Child,
    timeout: Option<Duration>,
    mut on_tick: impl FnMut(),
) -> std::io::Result<(ExitStatus, bool)> {
    let timeout_at = timeout.map(|value| Instant::now() + value);
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok((status, false));
        }
        if timeout_at.is_some_and(|value| Instant::now() >= value) {
            let _ = child.kill();
            return Ok((child.wait()?, true));
        }
        on_tick();
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Owns a child process group until its work is known to be complete.
///
/// The child must be spawned into a fresh process group whose id is its pid.
/// Interrupt cleanup is registered because the CLI signal handler exits the
/// process without running Rust destructors.
#[derive(Debug)]
pub(crate) struct ProcessGroupGuard {
    pid: Arc<AtomicU32>,
}

impl ProcessGroupGuard {
    pub(crate) fn new(pid: u32) -> Self {
        assert!(pid > 1, "owned process group must have a child pid");
        let pid = Arc::new(AtomicU32::new(pid));
        let interrupt_pid = Arc::clone(&pid);
        crate::engine::agent::register_interrupt_cleanup(move || {
            kill_process_group(interrupt_pid.swap(0, Ordering::AcqRel));
        });
        Self { pid }
    }

    pub(crate) fn terminate(&self) {
        kill_process_group(self.pid.swap(0, Ordering::AcqRel));
    }

    pub(crate) fn disarm(&self) {
        self.pid.store(0, Ordering::Release);
    }
}

impl Drop for ProcessGroupGuard {
    fn drop(&mut self) {
        self.terminate();
    }
}

/// SIGKILL every member of a process group the caller created. A provider
/// entry point is often a shim whose real server is a grandchild in the same
/// group; killing only the direct child leaves that server running.
pub(crate) fn kill_process_group(pid: u32) {
    if pid <= 1 {
        return;
    }

    #[cfg(unix)]
    if let Ok(pid) = i32::try_from(pid) {
        // SAFETY: callers spawn the child into a fresh process group whose id
        // is its pid; a negative pid targets only that owned group.
        unsafe {
            libc::kill(-pid, libc::SIGKILL);
        }
    }
    #[cfg(not(unix))]
    crate::engine::platform::kill_process(pid);
}

const TERMINATE_GRACE: Duration = Duration::from_secs(2);

/// SIGTERM a process group, wait, then SIGKILL the stragglers. Returns true
/// when no member remains. The caller proves the group is one Loopflow made.
#[cfg(unix)]
pub(crate) fn terminate_process_group(pgid: u32) -> bool {
    if pgid <= 1 || current_process_group_id() == Some(pgid) {
        return false;
    }
    let Ok(group) = i32::try_from(pgid) else {
        return false;
    };
    for signal in [libc::SIGTERM, libc::SIGKILL] {
        // SAFETY: a negative pid signals only the named process group.
        if unsafe { libc::kill(-group, signal) } != 0 {
            return std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH);
        }
        let deadline = Instant::now() + TERMINATE_GRACE;
        while Instant::now() < deadline {
            // SAFETY: WNOHANG collects the leader only if it is this
            // process's exited child; otherwise it fails without effect.
            unsafe {
                libc::waitpid(group, std::ptr::null_mut(), libc::WNOHANG);
            }
            // SAFETY: signal 0 probes the group and delivers nothing.
            if unsafe { libc::kill(-group, 0) } != 0
                && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
            {
                return true;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    false
}

#[cfg(not(unix))]
pub(crate) fn terminate_process_group(_pgid: u32) -> bool {
    false
}

mod lifeline;
pub(crate) use lifeline::{
    agent_process_lifeline_path, hold_agent_process_lifeline, spawn_agent_process,
};

pub(crate) fn current_process_group_id() -> Option<u32> {
    // SAFETY: getpgrp has no preconditions and does not dereference memory.
    let process_group = unsafe { libc::getpgrp() };
    u32::try_from(process_group).ok().filter(|id| *id > 1)
}

pub(crate) fn resolve_lf_binary() -> PathBuf {
    if crate::store::custom_home_selected() {
        if let Some(path) = std::env::var_os("LF_BIN").filter(|value| !value.is_empty()) {
            return PathBuf::from(path);
        }
        if let Some(path) = std::env::var_os("CARGO_BIN_EXE_lf") {
            return PathBuf::from(path);
        }
    } else if let Ok(Some(cli)) =
        crate::installation::root().and_then(|root| crate::installation::installed_cli(&root))
    {
        return cli.path;
    }
    if let Ok(current) = std::env::current_exe() {
        if current.file_name().is_some_and(|name| name == "lf") {
            return current;
        }
        if let Some(parent) = current.parent() {
            let sibling = parent.join("lf");
            if sibling.is_file() {
                return sibling;
            }
        }
    }
    which_on_path(Path::new("lf")).unwrap_or_else(|| PathBuf::from("lf"))
}

/// Resolve the `lf` a Work launch will use: an absolute path that exists.
///
/// `resolve_lf_binary` may hand back the bare name `lf`, which a child resolves
/// against the *login* shell's PATH inside tmux — a third binary, chosen by
/// neither the Work nor its launcher. Work that cannot name its own
/// executable is not created.
pub(crate) fn resolve_pinned_lf_binary() -> Result<PathBuf> {
    if !crate::store::custom_home_selected() {
        if let Some(cli) = crate::installation::installed_cli(&crate::installation::root()?)? {
            // One read resolves this for every Task; hash unchanged bytes once.
            static VERIFIED: Mutex<Option<(PathBuf, String, u64, SystemTime)>> = Mutex::new(None);
            let metadata = std::fs::metadata(&cli.path)?;
            let observed = Some((
                cli.path.clone(),
                cli.sha256.clone(),
                metadata.len(),
                metadata.modified()?,
            ));
            let mut verified = VERIFIED.lock().expect("verified CLI mutex poisoned");
            if *verified != observed {
                cli.verify()?;
                *verified = observed;
            }
            return Ok(cli.path);
        }
    }
    let candidate = resolve_lf_binary();
    if candidate.is_absolute() {
        return if candidate.exists() {
            Ok(candidate)
        } else {
            Err(anyhow!(
                "lf binary {} does not exist; set LF_BIN to the lf this Work should run",
                candidate.display()
            ))
        };
    }
    which_on_path(&candidate).ok_or_else(|| {
        anyhow!(
            "cannot resolve an absolute path for `{}`; set LF_BIN to the lf this Work should run",
            candidate.display()
        )
    })
}

/// Pin one process generation to immutable executable bytes.
///
/// The installed `lf` is normally a mutable symlink. Exact-frontier promotion
/// may repoint it while a body is running, so the body carries the
/// canonical target in `LF_BIN`. A later body launch deliberately
/// resolves the current Machine again and picks up the promoted binary.
pub(crate) fn pin_control_binary(lf_bin: &Path) -> PathBuf {
    std::fs::canonicalize(lf_bin).unwrap_or_else(|_| lf_bin.to_path_buf())
}

/// Capture the resolved CLI and Machine for a provider child.
pub(crate) fn execution_context() -> Result<crate::child::ChildExecutionContext> {
    crate::store::database_path_from_env()
        .map_err(|error| anyhow!("cannot resolve the Run database path: {error}"))?;
    Ok(crate::child::ChildExecutionContext {
        lf_bin: resolve_pinned_lf_binary()?,
        lf_home: crate::store::lf_home_dir(),
    })
}

pub(crate) fn which_on_path(name: &Path) -> Option<PathBuf> {
    if name.components().count() > 1 {
        return executable_file(name).then(|| name.to_path_buf());
    }
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| executable_file(candidate))
}

fn executable_file(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o111 == 0 {
            return false;
        }
    }
    metadata.is_file()
}

pub(crate) fn shell_escape(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

pub(crate) async fn start_lf_session_with_env(
    session: &str,
    cwd: &Path,
    argv: &[String],
    env: &[(&str, &str)],
) -> Result<()> {
    let context = execution_context()?;
    start_session_with_context(session, cwd, argv, env, context).await
}

pub(crate) async fn start_lf_session_inheriting(
    session: &str,
    cwd: &Path,
    argv: &[String],
    env: &[(&str, &str)],
    inherit: &(dyn Fn(&mut std::process::Command) + Send + Sync),
) -> Result<()> {
    let context = execution_context()?;
    let environment = session_environment(env, &context);
    let environment = environment
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect::<Vec<_>>();
    let shell = lf_session_shell_command(cwd, argv, &environment);
    let logs = context.lf_home.join("logs");
    std::fs::create_dir_all(&logs)?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs.join(format!("{session}.log")))?;
    let mut command = std::process::Command::new("sh");
    command
        .args(["-c", &shell])
        .current_dir(cwd)
        .stdin(std::process::Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    inherit(&mut command);
    // SAFETY: setsid affects only the child and is async-signal-safe. The repair
    // Process keeps its own lifetime after the release controller exits.
    unsafe {
        std::os::unix::process::CommandExt::pre_exec(&mut command, || {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let mut child = command.spawn()?;
    std::thread::spawn(move || {
        let _ = child.wait();
    });
    Ok(())
}

async fn start_session_with_context(
    session: &str,
    cwd: &Path,
    argv: &[String],
    env: &[(&str, &str)],
    context: crate::child::ChildExecutionContext,
) -> Result<()> {
    let child_env = session_environment(env, &context);
    let environment = child_env
        .iter()
        .map(|(key, value)| (key.as_str(), value.as_str()))
        .collect::<Vec<_>>();
    let shell_command = lf_session_shell_command(cwd, argv, &environment);
    start_tmux_session(session, &cwd.display().to_string(), &shell_command).await
}

fn session_environment(
    env: &[(&str, &str)],
    context: &crate::child::ChildExecutionContext,
) -> Vec<(String, String)> {
    let inherited_context = [
        "LF_TRACE_ID",
        "LF_PROCESS_LFID",
        crate::lf::WORK_DECLARATION_ENV,
    ]
    .into_iter()
    .filter(|key| !env.iter().any(|(explicit, _)| explicit == key))
    .filter_map(|key| std::env::var(key).ok().map(|value| (key, value)))
    .collect::<Vec<_>>();
    let mut child_env = env
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect::<Vec<_>>();
    child_env.extend(
        inherited_context
            .iter()
            .map(|(key, value)| ((*key).to_string(), value.clone())),
    );
    extend_session_control_context(&mut child_env, context);
    child_env
}

fn extend_session_control_context(
    child_env: &mut Vec<(String, String)>,
    context: &crate::child::ChildExecutionContext,
) {
    let pinned = [
        ("LF_BIN", context.lf_bin.to_string_lossy().to_string()),
        ("LF_HOME", context.lf_home.to_string_lossy().to_string()),
    ];
    for (key, value) in pinned {
        if !child_env.iter().any(|(existing, _)| existing == key) {
            child_env.push((key.to_string(), value));
        }
    }
}

pub(crate) fn lf_session_shell_command(
    cwd: &Path,
    argv: &[String],
    env: &[(&str, &str)],
) -> String {
    let command = argv
        .iter()
        .map(|arg| shell_escape(arg))
        .collect::<Vec<_>>()
        .join(" ");
    let env = env
        .iter()
        .filter(|(key, value)| *key != crate::lf::WORK_DECLARATION_ENV || !value.is_empty())
        .map(|(key, value)| format!("{}={}", shell_escape(key), shell_escape(value)))
        .collect::<Vec<_>>()
        .join(" ");
    let clear_context = format!(
        "unset {} {}; export LF_USER_NAME=\"\"",
        PROCESS_CONTEXT_ENV.join(" "),
        SESSION_AUTH_ENV.join(" "),
    );
    // A long-lived tmux server can retain a deleted cwd despite new-session -c.
    let enter_directory = format!("cd -- {} || exit", shell_escape(&cwd.to_string_lossy()));
    if env.is_empty() {
        format!("{enter_directory}; {clear_context}; exec {command}")
    } else {
        format!("{enter_directory}; {clear_context}; exec env {env} {command}")
    }
}

pub(crate) async fn start_tmux_session(
    session: &str,
    cwd: &str,
    shell_command: &str,
) -> Result<()> {
    let mut command = tokio::process::Command::new("tmux");
    command.current_dir(cwd);
    command.process_group(0);
    // This client may start the tmux server, whose environment every later
    // session inherits, including ones a person opens by hand.
    for name in PROCESS_CONTEXT_ENV {
        command.env_remove(name);
    }
    command.env_remove(crate::engine::config::USER_NAME_ENV);
    for name in SESSION_AUTH_ENV {
        command.env_remove(name);
    }
    let status = command
        .args([
            "new-session",
            "-d",
            "-s",
            session,
            "-c",
            cwd,
            "/bin/sh",
            "-lc",
            shell_command,
        ])
        .status()
        .await
        .map_err(|err| anyhow!("tmux failed to spawn in {cwd}: {err}"))?;
    if !status.success() {
        return Err(anyhow!("tmux failed to launch session '{session}'"));
    }
    let _ = tokio::process::Command::new("tmux")
        .args(["set-option", "-t", session, "mouse", "on"])
        .status()
        .await;
    Ok(())
}

/// What one lf process is: its Machine, binary, Process, Flow step and claims. A new
/// session starts without any of it and receives only what its launch names.
const PROCESS_CONTEXT_ENV: &[&str] = &[
    crate::lf::WORK_DECLARATION_ENV,
    crate::ops::human_session::HUMAN_SESSION_ENV,
    crate::ops::human_session::PREPARED_CAPTURE_ENV,
    "LF_RUN_DIR",
    "LF_RUN_ID",
    "LF_HUMAN_SESSION_RUN",
    "LF_REVIEW_RUN_RESERVATION",
    crate::journal::LF_TRACE_ID_ENV,
    crate::journal::LF_PROCESS_LFID_ENV,
    crate::work::wave::context::WAVE_ID_ENV,
    crate::session_record::CAPTURE_KEY_ENV,
    crate::process::AGENT_CALLER_ENV,
    crate::ops::git_operation::LF_GIT_OPERATION_ID_ENV,
    crate::session_record::PROVIDER_ACCOUNT_ID_ENV,
    crate::ops::flow_process::FLOW_ID_ENV,
    crate::installation::INSTALL_SWITCH_ENV,
    crate::lf::commands::ssh::EXPECTED_MACHINE_ID_ENV,
    "LF_TERMINAL_ID",
    "LF_TERMINAL_TTY",
    "LOOPFLOW_DIRECTIVE_FILE",
    "LOOPFLOW_FLOW_NAME",
    "LF_BIN",
    "LF_HOME",
];

/// Credentials and account choices a new Session must receive explicitly.
const SESSION_AUTH_ENV: &[&str] = &[
    crate::provider_account::selection::ACCOUNT_SELECTION_ENV,
    crate::provider_account::activation::ACCOUNT_ISOLATION_ENV,
    DISCORD_TOKEN_ENV,
    "GH_TOKEN",
    "OPENCODE_API_KEY",
    "CLAUDE_CODE_OAUTH_TOKEN",
    "ANTHROPIC_API_KEY",
    "CODEX_ACCESS_TOKEN",
    "OPENAI_API_KEY",
];

#[cfg(test)]
mod tests {

    use std::os::unix::process::CommandExt;
    use std::process::Command;
    use std::time::Duration;

    use super::{lf_session_shell_command, pin_control_binary, terminate_process_group};

    #[test]
    fn terminating_a_group_reaches_a_grandchild_that_outlived_its_leader() {
        let dir = tempfile::tempdir().unwrap();
        let flag = dir.path().join("survived");
        let mut leader = Command::new("sh")
            .arg("-c")
            .arg(format!("(sleep 1 && touch {}) &", flag.display()))
            .process_group(0)
            .spawn()
            .unwrap();
        let group = leader.id();
        leader.wait().unwrap();

        assert!(terminate_process_group(group));

        std::thread::sleep(Duration::from_millis(1500));
        assert!(!flag.exists(), "grandchild outlived its group");
    }

    #[test]
    fn detached_child_enters_quoted_directory_and_reports_missing_directory() {
        let directory = tempfile::tempdir().unwrap();
        let cwd = directory.path().join("checkout with 'quotes'");
        std::fs::create_dir(&cwd).unwrap();
        let command = lf_session_shell_command(&cwd, &["pwd".into(), "-P".into()], &[]);
        let output = std::process::Command::new("/bin/sh")
            .args(["-c", &command])
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(
            String::from_utf8(output.stdout).unwrap().trim(),
            cwd.canonicalize().unwrap().to_str().unwrap()
        );
        std::fs::remove_dir(&cwd).unwrap();
        let output = std::process::Command::new("/bin/sh")
            .args(["-c", &command])
            .output()
            .unwrap();
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(!output.stderr.is_empty());
    }

    #[test]
    #[ignore = "requires a local tmux executable; uses an isolated server"]
    fn detached_child_recovers_deleted_tmux_server_directory() {
        let directory = tempfile::Builder::new()
            .prefix("lf-tmux-")
            .tempdir_in("/tmp")
            .unwrap();
        let socket = directory.path().join("socket");
        let original = directory.path().join("deleted");
        let cwd = directory.path().join("checkout with 'quotes'");
        let output_path = directory.path().join("pwd");
        std::fs::create_dir(&original).unwrap();
        std::fs::create_dir(&cwd).unwrap();
        let tmux = || {
            let mut command = std::process::Command::new("tmux");
            command
                .env_remove("TMUX")
                .args(["-S"])
                .arg(&socket)
                .args(["-f", "/dev/null"]);
            command
        };
        assert!(tmux()
            .current_dir(&original)
            .args(["new-session", "-d", "-s", "anchor", "sleep 15"])
            .status()
            .unwrap()
            .success());
        std::fs::remove_dir(&original).unwrap();
        let command = lf_session_shell_command(
            &cwd,
            &[
                "/bin/sh".into(),
                "-c".into(),
                format!(
                    "pwd -P > {}",
                    super::shell_escape(output_path.to_str().unwrap())
                ),
            ],
            &[],
        );
        let launched = tmux()
            .args(["new-session", "-d", "-s", "child", "-c"])
            .arg(&cwd)
            .args(["/bin/sh", "-lc", &command])
            .status()
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while !output_path.exists() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let output = std::fs::read_to_string(&output_path);
        let _ = tmux().arg("kill-server").status();
        assert!(launched.success());
        assert_eq!(
            output.unwrap().trim(),
            cwd.canonicalize().unwrap().to_str().unwrap()
        );
    }

    #[test]
    fn a_body_generation_keeps_one_binary_across_a_global_repoint() {
        let dir = tempfile::tempdir().unwrap();
        let old = dir.path().join("lf-old");
        let new = dir.path().join("lf-new");
        let installed = dir.path().join("lf");
        std::fs::write(&old, b"old").unwrap();
        std::fs::write(&new, b"new").unwrap();
        std::os::unix::fs::symlink(&old, &installed).unwrap();

        let pinned = pin_control_binary(&installed);
        assert_eq!(pinned, std::fs::canonicalize(&old).unwrap());
        std::fs::remove_file(&installed).unwrap();
        std::os::unix::fs::symlink(&new, &installed).unwrap();

        assert_eq!(std::fs::read(&pinned).unwrap(), b"old");
        assert_eq!(
            std::fs::read(std::fs::canonicalize(&installed).unwrap()).unwrap(),
            b"new"
        );
    }

    #[test]
    fn lf_session_clears_parent_identity_and_exports_its_own() {
        let argv = vec![
            "lf".to_string(),
            "work".to_string(),
            "execute".to_string(),
            "task".to_string(),
            "tsk_123".to_string(),
        ];
        let command =
            lf_session_shell_command(std::path::Path::new("."), &argv, &[("LF_WAVE_ID", "infra")]);

        assert!(command.contains("unset "));
        assert!(command.contains("LF_ACCOUNT_SELECTION LF_ACCOUNT_ISOLATION"));
        assert!(command.contains("LF_DISCORD_TOKEN"));
        assert!(command.contains("GH_TOKEN OPENCODE_API_KEY"));
        assert!(command
            .ends_with("exec env 'LF_WAVE_ID'='infra' 'lf' 'work' 'execute' 'task' 'tsk_123'"));
    }

    #[test]
    fn lf_session_drops_a_stale_run_step_binary_and_home() {
        let argv = vec![
            "sh".into(),
            "-c".into(),
            "printf '%s' \"${LF_RUN_ID-}${LF_RUN_DIR-}${LF_FLOW_ID-}${LF_BIN-}${LF_HOME-unset}\""
                .into(),
        ];
        let command = lf_session_shell_command(std::path::Path::new("."), &argv, &[]);
        let output = std::process::Command::new("sh")
            .args(["-c", &command])
            .env("LF_RUN_ID", "run_dead")
            .env("LF_RUN_DIR", "/dead/run")
            .env("LF_FLOW_ID", "stale-flow")
            .env("LF_BIN", "/stale/lf")
            .env("LF_HOME", "/stale/home")
            .output()
            .unwrap();
        assert!(output.status.success());
        assert_eq!(String::from_utf8(output.stdout).unwrap(), "unset");
    }

    #[test]
    fn lf_session_without_explicit_identity_does_not_inherit_its_parent() {
        let argv = vec!["lf".to_string(), "wave".to_string(), "child".to_string()];

        let command = lf_session_shell_command(std::path::Path::new("."), &argv, &[]);

        assert!(command.contains("LF_WAVE_ID LF_CAPTURE_KEY "));
        assert!(command.contains("LF_ACCOUNT_SELECTION LF_ACCOUNT_ISOLATION"));
        assert!(command.ends_with("exec 'lf' 'wave' 'child'"));
    }

    #[test]
    fn preferred_name_in_durable_sessions_requires_explicit_context() {
        let argv = vec![
            "sh".into(),
            "-c".into(),
            "printf '%s' \"${LF_USER_NAME-unset}\"".into(),
        ];
        for name in [None, Some("Maya")] {
            let env = name
                .map(|name| vec![(crate::engine::config::USER_NAME_ENV, name)])
                .unwrap_or_default();
            let command = lf_session_shell_command(std::path::Path::new("."), &argv, &env);
            let output = std::process::Command::new("sh")
                .args(["-c", &command])
                .env(crate::engine::config::USER_NAME_ENV, "Jack")
                .output()
                .unwrap();
            assert!(output.status.success());
            assert_eq!(
                String::from_utf8(output.stdout).unwrap(),
                name.unwrap_or_default()
            );
        }
    }

    #[test]
    fn durable_session_drops_parent_credentials_and_account_choices() {
        let command = lf_session_shell_command(
            std::path::Path::new("."),
            &[
                "/bin/sh".into(),
                "-c".into(),
                "printf '%s' \"${LF_DISCORD_TOKEN-}${GH_TOKEN-}${LF_ACCOUNT_SELECTION-}${LF_ACCOUNT_ISOLATION-}\"".into(),
            ],
            &[],
        );
        let output = std::process::Command::new("/bin/sh")
            .args(["-c", &command])
            .env("LF_DISCORD_TOKEN", "fixture-chat-token")
            .env("GH_TOKEN", "fixture-github-token")
            .env("LF_ACCOUNT_SELECTION", "parent-selection")
            .env("LF_ACCOUNT_ISOLATION", "isolated")
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(output.stdout.is_empty());
    }

    #[test]
    fn lf_session_replaces_tmux_invocation_context() {
        let argv = vec![
            "lf".to_string(),
            "work".to_string(),
            "execute".to_string(),
            "task".to_string(),
            "tsk_123".to_string(),
        ];
        let command = lf_session_shell_command(
            std::path::Path::new("."),
            &argv,
            &[
                ("LF_TRACE_ID", "run-1"),
                ("LF_PROCESS_LFID", "process-1"),
                ("LF_HOME", "/tmp/lf"),
            ],
        );

        assert!(command.contains("LF_WAVE_ID LF_CAPTURE_KEY "));
        assert!(command.contains("LF_ACCOUNT_SELECTION LF_ACCOUNT_ISOLATION"));
        assert!(command.ends_with(
            "exec env 'LF_TRACE_ID'='run-1' 'LF_PROCESS_LFID'='process-1' 'LF_HOME'='/tmp/lf' 'lf' 'work' 'execute' 'task' 'tsk_123'"
        ));
    }
}

#[cfg(not(test))]
pub(crate) async fn start_home_session(session: &str, cwd: &Path, argv: &[String]) -> Result<()> {
    start_lf_session_with_env(session, cwd, argv, &[]).await
}
