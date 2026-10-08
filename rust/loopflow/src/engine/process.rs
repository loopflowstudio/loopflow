use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use std::time::SystemTime;

use anyhow::{anyhow, Result};

pub(crate) const DISCORD_TOKEN_ENV: &str = "LF_DISCORD_TOKEN";

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
            terminate_process_group(interrupt_pid.swap(0, Ordering::AcqRel));
        });
        Self { pid }
    }

    pub(crate) fn terminate(&self) {
        terminate_process_group(self.pid.swap(0, Ordering::AcqRel));
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

fn terminate_process_group(pid: u32) {
    if pid == 0 {
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
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|candidate| candidate.is_file())
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
        "if [ -n \"${{LF_FORWARDED_SECRET_NAMES:-}}\" ]; then unset $LF_FORWARDED_SECRET_NAMES; fi; unset {} {}; export LF_USER_NAME=\"\"",
        PROCESS_CONTEXT_ENV.join(" "),
        FORWARDED_AUTHORITY_ENV.join(" "),
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
    for name in forwarded_authority_env_names() {
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
