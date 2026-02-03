use crate::WtCommand;
use anyhow::{anyhow, Result};

pub fn run(cmd: &WtCommand) -> Result<()> {
    match cmd {
        WtCommand::Create { name, base, stack } => create(name, base.as_deref(), *stack),
        WtCommand::Switch { name } => switch(name),
        WtCommand::List { format, full, sync } => list(format.as_deref(), *full, *sync),
        WtCommand::Prune {
            dry_run,
            force,
            debug,
        } => prune(*dry_run, *force, *debug),
        WtCommand::Ci { watch, logs } => ci(*watch, *logs),
    }
}

fn create(_name: &str, _base: Option<&str>, _stack: bool) -> Result<()> {
    Err(anyhow!("wt create not yet implemented"))
}

fn switch(_name: &str) -> Result<()> {
    Err(anyhow!("wt switch not yet implemented"))
}

fn list(_format: Option<&str>, _full: bool, _sync: bool) -> Result<()> {
    Err(anyhow!("wt list not yet implemented"))
}

fn prune(_dry_run: bool, _force: bool, _debug: bool) -> Result<()> {
    Err(anyhow!("wt prune not yet implemented"))
}

fn ci(_watch: bool, _logs: bool) -> Result<()> {
    Err(anyhow!("wt ci not yet implemented"))
}
