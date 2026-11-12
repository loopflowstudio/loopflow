//! Headless work that outlives its caller: a plain `lf` process in its own
//! session, with no owner, holder or terminal between it and its launcher.
//!
//! The launcher hands the child one pipe, named by `LF_LAUNCHER_FD`. The child
//! writes its LfProcess ID on that pipe once it is admitted, and `released`
//! when it lets its launcher go (a Task run whose Graph registered, a ship
//! that announced itself). The pipe closes when the child exits. A pipe that
//! closes before the ID arrives leaves delivery unknown; nothing is started
//! again.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Duration;

use anyhow::{Context, Result};

use super::{execution_context, shell_escape, DISCORD_TOKEN_ENV};
use crate::id::LfProcessId;

const LAUNCHER_ENV: &str = "LF_LAUNCHER_FD";
const RELEASED: &str = "released";

/// How often a waiting launcher copies new output.
const TAIL_INTERVAL: Duration = Duration::from_millis(50);

/// This process's end of its launcher's pipe, until it lets the launcher go.
static LAUNCHER: OnceLock<Mutex<Option<File>>> = OnceLock::new();

/// Take the launcher's pipe. Nested processes never inherit it.
pub(crate) fn initialize() {
    let Some(fd) = std::env::var(LAUNCHER_ENV)
        .ok()
        .and_then(|fd| fd.parse::<RawFd>().ok())
    else {
        std::env::remove_var(LAUNCHER_ENV);
        return;
    };
    std::env::remove_var(LAUNCHER_ENV);
    // SAFETY: fcntl on a number we were handed; an invalid one fails and is ignored.
    if unsafe { libc::fcntl(fd, libc::F_SETFD, libc::FD_CLOEXEC) } < 0 {
        return;
    }
    // SAFETY: the launcher opened this descriptor for this process alone.
    let file = unsafe { File::from_raw_fd(fd) };
    let _ = LAUNCHER.set(Mutex::new(Some(file)));
}

/// Whether a launcher started this process detached and still waits on it.
pub(crate) fn launched() -> bool {
    LAUNCHER
        .get()
        .is_some_and(|launcher| launcher.lock().expect("launcher lock").is_some())
}

/// Tell the launcher which Process this is. A launcher that stopped listening
/// changes nothing here.
pub(crate) fn acknowledge(id: &LfProcessId) {
    tell(&format!("{id}\n"), false);
}

/// Let the launcher go; it stops waiting and returns success.
pub(crate) fn release() {
    tell(&format!("{RELEASED}\n"), true);
}

fn tell(line: &str, last: bool) {
    let Some(launcher) = LAUNCHER.get() else {
        return;
    };
    let mut launcher = launcher.lock().expect("launcher lock");
    if let Some(pipe) = launcher.as_mut() {
        let _ = pipe.write_all(line.as_bytes());
    }
    if last {
        *launcher = None;
    }
}

/// A started detached process and its pipe.
#[derive(Debug)]
pub(crate) struct Launched {
    child: Child,
    lines: BufReader<io::PipeReader>,
}

/// Start `command` in its own session with stdin from `/dev/null`, dropping
/// every descriptor this process inherited except those `inherit` hands on.
/// The caller sets its output.
pub(crate) fn spawn(mut command: Command, inherit: &dyn Fn(&mut Command)) -> Result<Launched> {
    let (reader, writer) = io::pipe()?;
    let pipe = writer.as_raw_fd();
    command
        .stdin(Stdio::null())
        .env(LAUNCHER_ENV, pipe.to_string());
    let descriptors = inherited_descriptors();
    // SAFETY: only async-signal-safe syscalls are used between fork and exec.
    unsafe {
        command.pre_exec(move || {
            if libc::setsid() < 0 {
                return Err(io::Error::last_os_error());
            }
            for fd in &descriptors {
                libc::fcntl(*fd, libc::F_SETFD, libc::FD_CLOEXEC);
            }
            if libc::fcntl(pipe, libc::F_SETFD, 0) < 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    // After the sweep, so descriptors the caller hands on stay open.
    inherit(&mut command);
    let child = command.spawn()?;
    drop(command);
    drop(writer);
    Ok(Launched {
        child,
        lines: BufReader::new(reader),
    })
}

impl Launched {
    /// The child's LfProcess, once it is admitted. Its child is reaped in the
    /// background; this process no longer waits on it.
    pub(crate) fn acknowledged(mut self) -> Result<LfProcessId> {
        let mut line = String::new();
        let read = self.lines.read_line(&mut line);
        let mut child = self.child;
        std::thread::spawn(move || {
            let _ = child.wait();
        });
        if read.unwrap_or(0) == 0 {
            anyhow::bail!(
                "The process exited before acknowledging execution; delivery is unknown, do not replay"
            );
        }
        LfProcessId::parse(line.trim())
            .with_context(|| format!("The process acknowledged {:?}, not a Process", line.trim()))
    }
}

/// Start `lf` again with `argv` in `cwd`, detached, and print its output here
/// until it exits or lets this process go. Killing this process leaves the
/// work running. Returns the exit code to report.
pub(crate) fn run(argv: &[String], cwd: &Path, inherit: &dyn Fn(&mut Command)) -> Result<u8> {
    let directory = crate::transport::private_directory()?;
    let stdout = tempfile::NamedTempFile::new_in(&directory)?;
    let stderr = tempfile::NamedTempFile::new_in(&directory)?;
    let mut tails = [
        (stdout.reopen()?, Output::Stdout),
        (stderr.reopen()?, Output::Stderr),
    ];
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .current_dir(cwd)
        .stdout(stdout.as_file().try_clone()?)
        .stderr(stderr.as_file().try_clone()?);
    let launched = spawn(command, inherit)?;
    // The open descriptors keep the output; no file is left behind.
    drop((stdout, stderr));
    let Launched { mut child, lines } = launched;
    let (signal, signals) = mpsc::channel();
    std::thread::spawn(move || {
        for line in lines.lines() {
            let Ok(line) = line else { break };
            if line == RELEASED {
                let _ = signal.send(());
            }
        }
    });
    loop {
        let released = signals.try_recv().is_ok();
        let exited = child.try_wait()?;
        for (file, output) in &mut tails {
            output.copy(file)?;
        }
        if released {
            return Ok(0);
        }
        if let Some(status) = exited {
            return Ok(crate::transport::provider_exit_code(status));
        }
        std::thread::sleep(TAIL_INTERVAL);
    }
}

#[derive(Debug, Clone, Copy)]
enum Output {
    Stdout,
    Stderr,
}

impl Output {
    /// Copy what the child wrote since the last copy.
    fn copy(self, file: &mut File) -> io::Result<()> {
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;
        if bytes.is_empty() {
            return Ok(());
        }
        match self {
            Self::Stdout => {
                let mut out = io::stdout().lock();
                out.write_all(&bytes)?;
                out.flush()
            }
            Self::Stderr => io::stderr().lock().write_all(&bytes),
        }
    }
}

/// Descriptors above stderr, such as a terminal host's pipes to its other
/// children that this process inherited. Detached work outlives that host, so
/// it must not hold them: a reader waiting for the host's end to close would
/// wait for as long as the work runs. Release locks and worktree leases this
/// process inherited stay: a release's surviving child keeps its target and
/// checkout exclusive after the release controller dies. Marking an already
/// close-on-exec or closed number (the listing's own) again is harmless.
pub(crate) fn inherited_descriptors() -> Vec<RawFd> {
    let Ok(entries) = std::fs::read_dir("/dev/fd") else {
        return Vec::new();
    };
    let held: Vec<RawFd> = ["LF_RELEASE_LOCK_FD", "LF_WORKTREE_LEASE_FD"]
        .iter()
        .filter_map(|name| std::env::var(name).ok()?.parse().ok())
        .collect();
    entries
        .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
        .filter(|fd| *fd > libc::STDERR_FILENO && !held.contains(fd))
        .collect()
}

/// Start `lf` with `argv` in `cwd` as a plain detached process that outlives
/// this one, and return its Process once it acknowledges. Its output is
/// appended to `logs/detached.log`.
pub(crate) fn start_lf(
    cwd: &Path,
    argv: &[String],
    env: &[(&str, &str)],
    inherit: &dyn Fn(&mut std::process::Command),
) -> Result<LfProcessId> {
    let context = execution_context()?;
    let environment = launch_environment(env, &context);
    let environment = environment
        .iter()
        .map(|(k, v)| (k.as_str(), v.as_str()))
        .collect::<Vec<_>>();
    let shell = shell_command(cwd, argv, &environment);
    let logs = crate::store::lf_home_dir().join("logs");
    std::fs::create_dir_all(&logs)
        .with_context(|| format!("Cannot open the detached log in {}", logs.display()))?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(logs.join("detached.log"))
        .with_context(|| format!("Cannot open the detached log in {}", logs.display()))?;
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", &shell])
        .current_dir(cwd)
        .stdout(log.try_clone()?)
        .stderr(log)
        .env_remove("LF_SERVER_STATUS")
        .env_remove(crate::transport::STREAM_ENV)
        // It names a descriptor the child does not inherit.
        .env_remove(crate::ops::human_session::LAUNCH_LOCK_ENV);
    spawn(command, inherit)?.acknowledged()
}

fn launch_environment(
    env: &[(&str, &str)],
    context: &crate::child::ChildExecutionContext,
) -> Vec<(String, String)> {
    let inherited_context = [
        crate::config::AGENT_OVERRIDE_ENV,
        "LF_SETTINGS",
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

fn shell_command(
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
    use super::*;

    fn shell(script: &str) -> Command {
        let mut command = Command::new("/bin/sh");
        command.args(["-c", script]);
        command
    }

    #[test]
    fn acknowledgement_names_the_process_the_child_reports() {
        let id = LfProcessId::new();
        let launched = spawn(
            shell(&format!("echo {id} >&$LF_LAUNCHER_FD; sleep 0.1")),
            &|_| {},
        )
        .unwrap();
        assert_eq!(launched.acknowledged().unwrap(), id);
    }

    #[test]
    fn a_child_that_exits_unacknowledged_leaves_delivery_unknown() {
        let error = spawn(shell("exit 3"), &|_| {})
            .unwrap()
            .acknowledged()
            .unwrap_err();
        assert!(format!("{error:#}").contains("do not replay"), "{error:#}");
    }

    #[test]
    fn the_child_leads_its_own_process_group_without_stdin() {
        let id = LfProcessId::new();
        let launched = spawn(
            shell(&format!(
                "[ \"$(ps -o pgid= -p $$ | tr -d ' ')\" = \"$$\" ] || exit 1; \
             read line && exit 1; echo {id} >&$LF_LAUNCHER_FD"
            )),
            &|_| {},
        )
        .unwrap();
        assert_eq!(launched.acknowledged().unwrap(), id);
    }

    #[test]
    fn detached_child_enters_quoted_directory_and_reports_missing_directory() {
        let directory = tempfile::tempdir().unwrap();
        let cwd = directory.path().join("checkout with 'quotes'");
        std::fs::create_dir(&cwd).unwrap();
        let command = shell_command(&cwd, &["pwd".into(), "-P".into()], &[]);
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
    fn detached_lf_clears_parent_identity_and_exports_its_own() {
        let argv = vec![
            "lf".to_string(),
            "work".to_string(),
            "execute".to_string(),
            "task".to_string(),
            "tsk_123".to_string(),
        ];
        let command =
            shell_command(std::path::Path::new("."), &argv, &[("LF_WAVE_ID", "infra")]);

        assert!(command.contains("unset "));
        assert!(command.contains("LF_ACCOUNT_SELECTION LF_DISCORD_TOKEN"));
        assert!(command.contains("LF_DISCORD_TOKEN"));
        assert!(command.contains("GH_TOKEN OPENCODE_API_KEY"));
        assert!(command
            .ends_with("exec env 'LF_WAVE_ID'='infra' 'lf' 'work' 'execute' 'task' 'tsk_123'"));
    }

    #[test]
    fn detached_lf_drops_a_stale_run_step_binary_and_home() {
        let argv = vec![
            "sh".into(),
            "-c".into(),
            "printf '%s' \"${LF_RUN_ID-}${LF_RUN_DIR-}${LF_GRAPH_ID-}${LF_BIN-}${LF_HOME-unset}\""
                .into(),
        ];
        let command = shell_command(std::path::Path::new("."), &argv, &[]);
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
    fn detached_lf_without_explicit_identity_does_not_inherit_its_parent() {
        let argv = vec!["lf".to_string(), "wave".to_string(), "child".to_string()];

        let command = shell_command(std::path::Path::new("."), &argv, &[]);

        assert!(command.contains("LF_WAVE_ID LF_CAPTURE_KEY "));
        assert!(command.contains("LF_ACCOUNT_SELECTION LF_DISCORD_TOKEN"));
        assert!(command.ends_with("exec 'lf' 'wave' 'child'"));
    }

    #[test]
    fn preferred_name_in_detached_lf_requires_explicit_context() {
        let argv = vec![
            "sh".into(),
            "-c".into(),
            "printf '%s' \"${LF_USER_NAME-unset}\"".into(),
        ];
        for name in [None, Some("Maya")] {
            let env = name
                .map(|name| vec![(crate::config::USER_NAME_ENV, name)])
                .unwrap_or_default();
            let command = shell_command(std::path::Path::new("."), &argv, &env);
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
    fn detached_lf_drops_parent_credentials_and_account_choices() {
        let command = shell_command(
            std::path::Path::new("."),
            &[
                "/bin/sh".into(),
                "-c".into(),
                "printf '%s' \"${LF_AGENT_OVERRIDE-}${LF_DISCORD_TOKEN-}${GH_TOKEN-}${LF_ACCOUNT_SELECTION-}\"".into(),
            ],
            &[],
        );
        let output = std::process::Command::new("/bin/sh")
            .args(["-c", &command])
            .env("LF_AGENT_OVERRIDE", "claude")
            .env("LF_DISCORD_TOKEN", "fixture-chat-token")
            .env("GH_TOKEN", "fixture-github-token")
            .env("LF_ACCOUNT_SELECTION", "parent-selection")
            .output()
            .unwrap();
        assert!(output.status.success());
        assert!(output.stdout.is_empty());
    }

    #[test]
    fn detached_lf_replaces_inherited_invocation_context() {
        let argv = vec![
            "lf".to_string(),
            "work".to_string(),
            "execute".to_string(),
            "task".to_string(),
            "tsk_123".to_string(),
        ];
        let command = shell_command(
            std::path::Path::new("."),
            &argv,
            &[
                ("LF_TRACE_ID", "run-1"),
                ("LF_PROCESS_ID", "process-1"),
                ("LF_HOME", "/tmp/lf"),
            ],
        );

        assert!(command.contains("LF_WAVE_ID LF_CAPTURE_KEY "));
        assert!(command.contains("LF_ACCOUNT_SELECTION LF_DISCORD_TOKEN"));
        assert!(command.ends_with(
            "exec env 'LF_TRACE_ID'='run-1' 'LF_PROCESS_ID'='process-1' 'LF_HOME'='/tmp/lf' 'lf' 'work' 'execute' 'task' 'tsk_123'"
        ));
    }
}
