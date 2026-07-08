use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use crate::ops::{OpsError, OpsResult};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PassOptions {
    pub timeout: Duration,
    pub max_turns: Option<u32>,
}

/// One bounded, headless pass of ANY flow in a worktree — the flowloop
/// primitive. The tiers bind their `<tier>-pass` flows here, but a skill
/// loop or a scan flow loops the same way: `lf -b <flow>`, killed on
/// timeout.
pub fn run_pass(worktree: &Path, flow: &str, seed: &str, options: &PassOptions) -> OpsResult<()> {
    let mut cmd = lf_command();
    cmd.arg("-b");
    if let Some(max_turns) = options.max_turns {
        cmd.arg("--max-turns").arg(max_turns.to_string());
    }
    // The explicit verb: bare flow names (`task`, `wave`) collide with
    // subcommands, `lf flow <name>` never does.
    cmd.arg("flow");
    cmd.arg(flow);
    cmd.arg(seed);
    cmd.current_dir(worktree);

    let output = run_with_timeout(cmd, options.timeout)?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("lf -b flow {flow}"),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
        });
    }

    Ok(())
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
                "flowloop pass timed out after {}s",
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
    use std::process::Command;
    use std::time::Duration;

    use super::run_with_timeout;

    #[test]
    fn pass_runner_kills_on_timeout() {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", "sleep 2"]);

        let err = run_with_timeout(cmd, Duration::from_millis(50)).expect_err("timeout");

        assert!(err.to_string().contains("timed out"));
    }
}
