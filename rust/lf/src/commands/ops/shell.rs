use crate::ShellCommand;
use anyhow::{anyhow, Result};

pub fn run(cmd: &ShellCommand) -> Result<()> {
    match cmd {
        ShellCommand::Init { shell } => init(shell.as_deref()),
        ShellCommand::Install { shell } => install(shell.as_deref()),
        ShellCommand::Directive { command } => directive(command),
    }
}

fn init(_shell: Option<&str>) -> Result<()> {
    Err(anyhow!("shell init not yet implemented"))
}

fn install(_shell: Option<&str>) -> Result<()> {
    Err(anyhow!("shell install not yet implemented"))
}

fn directive(_command: &[String]) -> Result<()> {
    Err(anyhow!("shell directive not yet implemented"))
}
