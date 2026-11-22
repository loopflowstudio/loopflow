use std::process::{Command, Output};

pub fn command_exists(name: &str) -> bool {
    Command::new(name)
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

pub fn stderr_from_output(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).trim().to_string()
}
