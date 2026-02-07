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
