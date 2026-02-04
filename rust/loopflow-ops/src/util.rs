use std::path::Path;
use std::process::{Command, Output};

use crate::error::{OpsError, OpsResult};

pub fn command_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn run_command(repo: &Path, program: &str, args: &[&str]) -> OpsResult<Output> {
    let output = Command::new(program)
        .args(args)
        .current_dir(repo)
        .output()?;
    Ok(output)
}

pub fn run_command_checked(repo: &Path, program: &str, args: &[&str]) -> OpsResult<Output> {
    let output = run_command(repo, program, args)?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: format!("{} {}", program, args.join(" ")),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }
    Ok(output)
}

pub fn stdout_from_output(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

pub fn stderr_from_output(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}
