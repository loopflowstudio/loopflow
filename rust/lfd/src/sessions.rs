use std::io::Read;
use std::path::PathBuf;

use portable_pty::{CommandBuilder, NativePtySystem, PtySize, PtySystem};

#[derive(Debug, thiserror::Error)]
pub enum SessionError {
    #[error("pty error: {0}")]
    Pty(String),
}

#[derive(Debug, Clone)]
pub struct PtyCommand {
    program: String,
    args: Vec<String>,
    cwd: Option<PathBuf>,
}

impl PtyCommand {
    pub fn new(program: impl Into<String>) -> Self {
        Self {
            program: program.into(),
            args: Vec::new(),
            cwd: None,
        }
    }

    pub fn arg(mut self, value: impl Into<String>) -> Self {
        self.args.push(value.into());
        self
    }

    pub fn cwd(mut self, cwd: impl Into<PathBuf>) -> Self {
        self.cwd = Some(cwd.into());
        self
    }
}

pub fn run_pty_command(command: PtyCommand) -> Result<i32, SessionError> {
    let pty_system = NativePtySystem::default();
    let pair = pty_system
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|err: anyhow::Error| SessionError::Pty(err.to_string()))?;

    let mut cmd = CommandBuilder::new(command.program);
    for arg in command.args {
        cmd.arg(arg);
    }
    if let Some(cwd) = command.cwd {
        cmd.cwd(cwd);
    }

    let mut child = pair
        .slave
        .spawn_command(cmd)
        .map_err(|err: anyhow::Error| SessionError::Pty(err.to_string()))?;

    let mut reader = pair
        .master
        .try_clone_reader()
        .map_err(|err: anyhow::Error| SessionError::Pty(err.to_string()))?;

    let output_handle = std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    let status = child
        .wait()
        .map_err(|err: std::io::Error| SessionError::Pty(err.to_string()))?;
    let _ = output_handle.join();

    Ok(status.exit_code() as i32)
}

#[cfg(test)]
mod tests {
    use super::{run_pty_command, PtyCommand};

    #[test]
    fn run_pty_command_returns_exit_code() {
        let command = PtyCommand::new("sh").arg("-c").arg("exit 0");
        let exit_code = run_pty_command(command).unwrap();
        assert_eq!(exit_code, 0);
    }
}
