use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::ops::{OpsError, OpsResult};
use crate::{
    journal::{LF_PROCESS_ID_ENV, LF_RUN_ID_ENV},
    lfd::id::LfdId,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassOptions {
    pub timeout: Duration,
    pub max_turns: Option<u32>,
}

/// One bounded, headless pass of ANY flow in a worktree — the loop
/// primitive. The tiers bind their `<tier>-pass` flows here, but a skill
/// loop or a scan flow loops the same way: `lf --yolo -b <flow>`, killed on
/// timeout.
pub fn run_pass(
    worktree: &Path,
    flow: &str,
    seed: &str,
    trace_id: &LfdId,
    options: &PassOptions,
) -> OpsResult<()> {
    let cmd = pass_command(worktree, flow, seed, trace_id, options);

    let output = run_with_timeout(cmd, options.timeout)?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("lf -b flow {flow}"),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    Ok(())
}

fn pass_command(
    worktree: &Path,
    flow: &str,
    seed: &str,
    trace_id: &LfdId,
    options: &PassOptions,
) -> Command {
    let mut cmd = lf_command();
    // Loopflow-owned workers already have an isolated worktree. Give the
    // worker direct authority there instead of routing privileged operations
    // through a second exec service.
    cmd.arg("--yolo");
    cmd.arg("-b");
    if let Some(max_turns) = options.max_turns {
        cmd.arg("--max-turns").arg(max_turns.to_string());
    }
    // The explicit verb keeps pass execution independent of whether a flow
    // name collides with a top-level command.
    cmd.arg("flow");
    cmd.arg(flow);
    cmd.arg(seed);
    cmd.current_dir(worktree);
    // The registry run is the trace. Each pass inherits that trace while its
    // own journal runtime replaces LF_PROCESS_ID with a fresh span id.
    cmd.env(LF_RUN_ID_ENV, trace_id.as_str());
    cmd.env_remove(LF_PROCESS_ID_ENV);
    cmd
}

pub(crate) fn run_with_timeout(
    mut cmd: Command,
    timeout: Duration,
) -> OpsResult<std::process::Output> {
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn()?;
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output().map_err(OpsError::from);
        }
        if started.elapsed() >= timeout {
            let _ = child.kill();
            let _ = child.wait();
            return Err(OpsError::Message(format!(
                "loop pass timed out after {}s",
                timeout.as_secs()
            )));
        }
        thread::sleep(Duration::from_millis(100));
    }
}

pub(crate) fn lf_command() -> Command {
    if let Ok(path) = std::env::current_exe() {
        return Command::new(path);
    }
    Command::new("lf")
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::path::Path;
    use std::process::Command;
    use std::time::Duration;

    use crate::lfd::id::LfdId;

    use super::{pass_command, run_with_timeout, PassOptions};

    #[test]
    fn foreground_pass_uses_the_registry_run_as_its_trace() {
        let run_id = LfdId::new();
        let options = PassOptions {
            timeout: Duration::from_secs(1),
            max_turns: None,
        };
        let command = pass_command(
            Path::new("/tmp/worktree"),
            "task",
            "ship it",
            &run_id,
            &options,
        );
        let trace = command
            .get_envs()
            .find(|(key, _)| *key == OsStr::new(crate::journal::LF_RUN_ID_ENV))
            .and_then(|(_, value)| value)
            .expect("pass trace env");

        assert_eq!(trace, OsStr::new(run_id.as_str()));
        assert!(command
            .get_args()
            .any(|argument| argument == OsStr::new("--yolo")));
        let inherited_process = command
            .get_envs()
            .find(|(key, _)| *key == OsStr::new(crate::journal::LF_PROCESS_ID_ENV))
            .expect("pass process env override");
        assert_eq!(inherited_process.1, None, "pass mints a fresh process id");
    }

    #[test]
    fn pass_runner_kills_on_timeout() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 2"]);

        let err = run_with_timeout(cmd, Duration::from_millis(50)).expect_err("timeout");

        assert!(err.to_string().contains("timed out"));
    }
}
