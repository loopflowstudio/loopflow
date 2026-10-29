use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{anyhow, Result};

pub(crate) const DISCORD_TOKEN_ENV: &str = "LF_DISCORD_TOKEN";

/// Bound both execution and inherited output pipes, then reap the owned group.
pub(crate) fn bounded_output(command: &mut Command, timeout: Duration) -> std::io::Result<Output> {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let group = ProcessGroupGuard::new(child.id());
    let deadline = Instant::now() + timeout;
    let (sender, receiver) = mpsc::channel();
    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");
    for (index, mut pipe) in [
        (0, Box::new(stdout) as Box<dyn Read + Send>),
        (1, Box::new(stderr)),
    ] {
        let sender = sender.clone();
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let result = pipe.read_to_end(&mut bytes).map(|_| bytes);
            let _ = sender.send((index, result));
        });
    }
    drop(sender);
    let result = (|| {
        let mut streams = [None, None];
        let mut status = None;
        loop {
            while let Ok((index, bytes)) = receiver.try_recv() {
                streams[index] = Some(bytes?);
            }
            if status.is_none() {
                status = child.try_wait()?;
            }
            if let Some(status) = status {
                if streams.iter().all(Option::is_some) {
                    return Ok(Output {
                        status,
                        stdout: streams[0].take().expect("stdout completed"),
                        stderr: streams[1].take().expect("stderr completed"),
                    });
                }
            }
            if Instant::now() >= deadline {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "command inspection deadline exceeded",
                ));
            }
            thread::sleep(Duration::from_millis(10));
        }
    })();
    // Kill the owned group even if its leader has exited but descendants retain pipes.
    group.terminate();
    let cleanup_deadline = Instant::now() + Duration::from_secs(1);
    while child.try_wait()?.is_none() {
        if Instant::now() >= cleanup_deadline {
            // Retain a reaper if the OS cannot settle the killed child promptly.
            thread::spawn(move || {
                let _ = child.wait();
            });
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "command child cleanup deadline exceeded",
            ));
        }
        thread::sleep(Duration::from_millis(10));
    }
    result
}

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
        crate::agent::register_interrupt_cleanup(move || {
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
    crate::platform::kill_process(pid);
}

/// SIGTERM gets a short grace; SIGKILL waits for the kernel to finish tearing
/// the group down, which takes seconds for large address spaces under load.
const TERMINATE_GRACE: Duration = Duration::from_secs(2);
const KILL_GRACE: Duration = Duration::from_secs(10);

/// SIGTERM a process group, wait, then SIGKILL the stragglers. Succeeds when
/// every member has exited: zombies await their own parent's wait and cannot
/// keep an AgentProcess alive. The caller proves this is an owned group.
#[cfg(unix)]
pub(crate) fn terminate_process_group(pgid: u32) -> Result<()> {
    let group = i32::try_from(pgid)
        .ok()
        .filter(|_| pgid > 1 && current_process_group_id() != Some(pgid))
        .ok_or_else(|| anyhow!("process group {pgid} is not an owned group"))?;
    let mut survivors = Vec::new();
    for (signal, grace) in [
        (libc::SIGTERM, TERMINATE_GRACE),
        (libc::SIGKILL, KILL_GRACE),
    ] {
        // SAFETY: a negative pid signals only the named process group.
        if unsafe { libc::kill(-group, signal) } != 0 {
            let error = std::io::Error::last_os_error();
            if error.raw_os_error() == Some(libc::ESRCH) {
                return Ok(());
            }
            return Err(anyhow!("cannot signal process group {pgid}: {error}"));
        }
        let deadline = Instant::now() + grace;
        // Observe before checking the deadline so the last look follows it.
        loop {
            // SAFETY: WNOHANG collects the leader only if it is this
            // process's exited child; otherwise it fails without effect.
            unsafe {
                libc::waitpid(group, std::ptr::null_mut(), libc::WNOHANG);
            }
            survivors = crate::journal::OsProcess::group(pgid)
                .map_err(|error| anyhow!("cannot observe process group {pgid}: {error}"))?;
            if survivors.is_empty() {
                return Ok(());
            }
            if Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    Err(anyhow!(
        "process group {pgid} survives SIGKILL: {}",
        crate::journal::OsProcess::describe(&survivors)
    ))
}

#[cfg(not(unix))]
pub(crate) fn terminate_process_group(pgid: u32) -> Result<()> {
    Err(anyhow!(
        "process group {pgid} cannot be signaled on this platform"
    ))
}

#[cfg(test)]
mod installation_tests;
mod lifeline;
pub(crate) use lifeline::{
    agent_process_lifeline_path, hold_agent_process_lifeline, release_agent_lifeline,
    spawn_agent_process, spawn_native_agent_process, watch_tool_lifeline,
};

pub(crate) fn current_process_group_id() -> Option<u32> {
    // SAFETY: getpgrp has no preconditions and does not dereference memory.
    let process_group = unsafe { libc::getpgrp() };
    u32::try_from(process_group).ok().filter(|id| *id > 1)
}

/// Resolve a stable installed entry, the Desktop helper running this process,
/// or an explicit private-Home executable.
/// Keep symlinks intact so later launches pick up installation changes.
pub(crate) fn resolve_lf_binary() -> Result<PathBuf> {
    let candidate = if crate::store::custom_home_selected() {
        std::env::var_os("LF_BIN")
            .filter(|value| !value.is_empty())
            .or_else(|| std::env::var_os("CARGO_BIN_EXE_lf"))
            .map(PathBuf::from)
    } else if let Some(helper) = crate::installation::desktop_helper() {
        Some(helper)
    } else {
        // The gate selects and verifies the installed artifact when executed.
        let gate = crate::installation::entry_gate_path(
            &crate::installation::root()?,
            &crate::installation::ArtifactRole::Cli,
        )?;
        let installed = crate::installation::account_home()?.join(".local/bin/lf");
        [gate, installed].into_iter().find(|path| path.is_file())
    };
    let candidate = candidate
        .or_else(|| {
            let current = std::env::current_exe().ok()?;
            if current.file_name().is_some_and(|name| name == "lf") {
                return Some(current);
            }
            let sibling = current.parent()?.join("lf");
            sibling.is_file().then_some(sibling)
        })
        .unwrap_or_else(|| PathBuf::from("lf"));
    if candidate.is_absolute() {
        anyhow::ensure!(
            candidate.is_file(),
            "lf binary {} does not exist",
            candidate.display()
        );
        return Ok(candidate);
    }
    which_on_path(&candidate).ok_or_else(|| {
        anyhow!(
            "cannot resolve an executable path for `{}`",
            candidate.display()
        )
    })
}

/// Capture the resolved CLI and Machine for a provider child.
pub(crate) fn execution_context() -> Result<crate::child::ChildExecutionContext> {
    Ok(crate::child::ChildExecutionContext {
        lf_bin: resolve_lf_binary()?,
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

/// A failed process's own last word, formatted to follow its exit status.
pub(crate) fn exit_reason(stderr: &str) -> String {
    stderr
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(|line| format!(": {line}"))
        .unwrap_or_default()
}

pub(crate) fn shell_escape(value: &str) -> String {
    let escaped = value.replace('\'', "'\\''");
    format!("'{escaped}'")
}

pub(crate) fn start_lf_session(
    cwd: &Path,
    argv: &[String],
    env: &[(&str, &str)],
    inherit: &dyn Fn(&mut std::process::Command),
) -> Result<crate::id::LfProcessId> {
    let context = execution_context()?;
    let environment = session_environment(env, &context);
    let environment = environment
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect::<Vec<_>>();
    let shell = lf_session_shell_command(cwd, argv, &environment);
    crate::transport::start(&["/bin/sh".into(), "-c".into(), shell], false, cwd, inherit)
}

fn session_environment(
    env: &[(&str, &str)],
    context: &crate::child::ChildExecutionContext,
) -> Vec<(String, String)> {
    let inherited_context = [
        crate::config::AGENT_OVERRIDE_ENV,
        "LF_TRACE_ID",
        "LF_PROCESS_ID",
        crate::lf::WORK_DECLARATION_ENV,
    ]
    .into_iter()
    .filter_map(|key| std::env::var(key).ok().map(|value| (key, value)))
    .chain([
        ("LF_BIN", context.lf_bin.to_string_lossy().to_string()),
        ("LF_HOME", context.lf_home.to_string_lossy().to_string()),
    ])
    .filter(|(key, _)| !env.iter().any(|(explicit, _)| explicit == key))
    .map(|(key, value)| (key.to_string(), value));
    env.iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .chain(inherited_context)
        .collect()
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
    // Resolve cwd in the child before executing the prepared command.
    let enter_directory = format!("cd -- {} || exit", shell_escape(&cwd.to_string_lossy()));
    if env.is_empty() {
        format!("{enter_directory}; {clear_context}; exec {command}")
    } else {
        format!("{enter_directory}; {clear_context}; exec env {env} {command}")
    }
}

/// What one lf process is: its Machine, binary, Process, Graph step and claims. A new
/// session starts without any of it and receives only what its launch names.
const PROCESS_CONTEXT_ENV: &[&str] = &[
    crate::config::AGENT_OVERRIDE_ENV,
    crate::lf::WORK_DECLARATION_ENV,
    crate::ops::human_session::HUMAN_SESSION_ENV,
    crate::ops::human_session::PREPARED_CAPTURE_ENV,
    "LF_RUN_DIR",
    "LF_RUN_ID",
    "LF_HUMAN_SESSION_RUN",
    "LF_REVIEW_RUN_RESERVATION",
    crate::journal::LF_TRACE_ID_ENV,
    crate::journal::LF_PROCESS_ID_ENV,
    crate::work::wave::context::WAVE_ID_ENV,
    crate::session_record::CAPTURE_KEY_ENV,
    crate::process::AGENT_CALLER_ENV,
    crate::session_record::PROVIDER_ACCOUNT_ID_ENV,
    crate::ops::graph_process::GRAPH_ID_ENV,
    crate::installation::INSTALL_SWITCH_ENV,
    crate::lf::commands::ssh::EXPECTED_MACHINE_ID_ENV,
    "LF_TERMINAL_ID",
    "LF_TERMINAL_TTY",
    "LOOPFLOW_DIRECTIVE_FILE",
    "LOOPFLOW_GRAPH_NAME",
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

    use super::{lf_session_shell_command, terminate_process_group};

    #[test]
    fn private_home_keeps_explicit_binary_selection() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        let home = tempfile::tempdir().unwrap();
        std::env::set_var("LF_HOME", home.path());
        let binary = std::env::current_exe().unwrap();
        std::env::set_var("LF_BIN", &binary);
        assert_eq!(super::resolve_lf_binary().unwrap(), binary);
        crate::installation::canonicalize_selected_home().unwrap();
        std::env::set_var("LF_BIN", "/missing/selected/lf");
        assert!(super::resolve_lf_binary().is_err());
    }

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

        terminate_process_group(group).unwrap();

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
            "printf '%s' \"${LF_RUN_ID-}${LF_RUN_DIR-}${LF_GRAPH_ID-}${LF_BIN-}${LF_HOME-unset}\""
                .into(),
        ];
        let command = lf_session_shell_command(std::path::Path::new("."), &argv, &[]);
        let output = std::process::Command::new("sh")
            .args(["-c", &command])
            .env("LF_RUN_ID", "run_dead")
            .env("LF_RUN_DIR", "/dead/run")
            .env("LF_GRAPH_ID", "stale-graph")
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
                .map(|name| vec![(crate::config::USER_NAME_ENV, name)])
                .unwrap_or_default();
            let command = lf_session_shell_command(std::path::Path::new("."), &argv, &env);
            let output = std::process::Command::new("sh")
                .args(["-c", &command])
                .env(crate::config::USER_NAME_ENV, "Jack")
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
                "printf '%s' \"${LF_AGENT_OVERRIDE-}${LF_DISCORD_TOKEN-}${GH_TOKEN-}${LF_ACCOUNT_SELECTION-}${LF_ACCOUNT_ISOLATION-}\"".into(),
            ],
            &[],
        );
        let output = std::process::Command::new("/bin/sh")
            .args(["-c", &command])
            .env("LF_AGENT_OVERRIDE", "claude")
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
    fn detached_session_replaces_inherited_invocation_context() {
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
                ("LF_PROCESS_ID", "process-1"),
                ("LF_HOME", "/tmp/lf"),
            ],
        );

        assert!(command.contains("LF_WAVE_ID LF_CAPTURE_KEY "));
        assert!(command.contains("LF_ACCOUNT_SELECTION LF_ACCOUNT_ISOLATION"));
        assert!(command.ends_with(
            "exec env 'LF_TRACE_ID'='run-1' 'LF_PROCESS_ID'='process-1' 'LF_HOME'='/tmp/lf' 'lf' 'work' 'execute' 'task' 'tsk_123'"
        ));
    }
}
