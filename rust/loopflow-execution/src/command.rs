use std::fmt;
use std::io;
use std::process::{Command, Output};

#[derive(Debug)]
pub struct CommandError {
    pub command: String,
    pub args: Vec<String>,
    pub status: Option<i32>,
    pub stderr: String,
    pub message: String,
}

impl CommandError {
    pub fn command_line(&self) -> String {
        if self.args.is_empty() {
            self.command.clone()
        } else {
            format!("{} {}", self.command, self.args.join(" "))
        }
    }

    fn io(command: String, args: Vec<String>, err: io::Error) -> Self {
        Self {
            command,
            args,
            status: None,
            stderr: String::new(),
            message: err.to_string(),
        }
    }

    fn from_output(command: String, args: Vec<String>, output: &Output) -> Self {
        Self {
            command,
            args,
            status: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            message: "command failed".to_string(),
        }
    }
}

impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.stderr.is_empty() {
            write!(f, "{} ({})", self.command_line(), self.message)
        } else {
            write!(f, "{}: {}", self.command_line(), self.stderr)
        }
    }
}

impl std::error::Error for CommandError {}

pub fn run_command(cmd: &mut Command) -> Result<Output, CommandError> {
    let command = cmd.get_program().to_string_lossy().to_string();
    let args = cmd
        .get_args()
        .map(|arg| arg.to_string_lossy().to_string())
        .collect::<Vec<_>>();

    let output = match cmd.output() {
        Ok(output) => output,
        Err(err) => return Err(CommandError::io(command, args, err)),
    };
    if output.status.success() {
        Ok(output)
    } else {
        Err(CommandError::from_output(command, args, &output))
    }
}

/// A command's streams belong to its caller, never to the server process.
pub struct CommandIo<'a> {
    pub input: &'a mut dyn io::Read,
    pub output: &'a mut dyn io::Write,
    pub error: &'a mut dyn io::Write,
}

impl std::fmt::Debug for CommandIo<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CommandIo").finish_non_exhaustive()
    }
}

pub fn lf_command_label(command: Option<&str>) -> String {
    let Some(command) = command else {
        return "lf".to_string();
    };
    let Ok(argv) = serde_json::from_str::<Vec<String>>(command) else {
        return "lf".to_string();
    };
    let Some(command) = argv.get(1).filter(|value| !value.starts_with('-')) else {
        return "lf".to_string();
    };
    let grouped = [
        "machine",
        "repository",
        "repo",
        "pm",
        "pr",
        "project",
        "radio",
        "task",
        "wave",
        "work",
    ];
    let operation = grouped
        .contains(&command.as_str())
        .then(|| argv.get(2))
        .flatten()
        .filter(|value| safe_command_word(value));
    match operation {
        Some(operation) => format!("lf {command} {operation}"),
        None => format!("lf {command}"),
    }
}

pub fn safe_command_word(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

#[cfg(test)]
mod tests {
    use super::lf_command_label;

    #[test]
    fn saved_command_labels_show_the_operation_without_its_arguments() {
        for (saved, label) in [
            (None, "lf"),
            (Some("not JSON"), "lf"),
            (
                Some(r#"["lf","--task","private-selector","task","auto","on"]"#),
                "lf",
            ),
            (
                Some(r#"["lf","task","run","private-selector"]"#),
                "lf task run",
            ),
            (
                Some(r#"["lf","steer","private-selector","private-message"]"#),
                "lf steer",
            ),
        ] {
            assert_eq!(lf_command_label(saved), label);
        }
    }
}
