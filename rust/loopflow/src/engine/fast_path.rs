use std::fmt;
use std::path::Path;
use std::process::Command;

#[derive(Debug)]
pub enum FastPathResult {
    Success,
    Failed {
        exit_code: i32,
        stdout: String,
        stderr: String,
    },
}

/// Formatted failure context for injecting into an agent prompt.
#[derive(Debug)]
pub struct FailureContext<'a> {
    pub cmd: &'a str,
    pub exit_code: i32,
    pub stdout: &'a str,
    pub stderr: &'a str,
}

impl fmt::Display for FailureContext<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "<lf:fast-path-failure>\nCommand: {}\nExit code: {}\n\nstdout:\n{}\nstderr:\n{}\n</lf:fast-path-failure>\n\n",
            self.cmd, self.exit_code, self.stdout, self.stderr
        )
    }
}

/// Try running a fast-path command before spinning up an agent.
///
/// Returns `Success` if the command exits 0, `Failed` with output otherwise.
/// The command runs via `sh -c` with the given working directory.
pub fn try_fast_path(cmd: &str, cwd: &Path) -> std::io::Result<FastPathResult> {
    let mut command = Command::new("sh");
    command.args(["-c", cmd]).current_dir(cwd);

    // Propagate LOOPFLOW_DIRECTIVE_FILE so the command can issue shell directives.
    if let Ok(directive) = std::env::var("LOOPFLOW_DIRECTIVE_FILE") {
        command.env("LOOPFLOW_DIRECTIVE_FILE", directive);
    }

    let output = command.output()?;
    if output.status.success() {
        Ok(FastPathResult::Success)
    } else {
        Ok(FastPathResult::Failed {
            exit_code: output.status.code().unwrap_or(1),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }
}
