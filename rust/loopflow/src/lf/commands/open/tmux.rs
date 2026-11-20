//! tmux adapter: open and focus conversation windows in the caller's session.
//! Task split layouts are handled only by Rex, Herdr and cmux.
use std::process::Command;

use anyhow::{Context, Result};

use super::{run_host, viewed_placement, OpenHost, Resolved};

use loopflow_execution::terminal_host::TmuxLocation;

fn host_command(location: &TmuxLocation) -> Command {
    let mut command = Command::new("tmux");
    command.args(["-S", &location.socket]);
    command
}

/// A new window right after the caller's, named for the Work. Its pane is the
/// destination: pane IDs are unique for the server's life, unlike indexes.
pub(super) fn open(target: &Resolved, focus: bool, argv: &[String]) -> Result<String> {
    let shell = shlex::try_join(argv.iter().map(String::as_str))?;
    let caller = match viewed_placement() {
        Some((OpenHost::Tmux, value)) => Some(
            serde_json::from_str::<TmuxLocation>(&value)
                .context("Recorded tmux view is unreadable")?,
        ),
        _ => TmuxLocation::current(),
    };
    let tmux = || {
        caller
            .as_ref()
            .map(host_command)
            .unwrap_or_else(|| Command::new("tmux"))
    };
    // `new-window -t` names a window: tmux 3.7 refuses a pane there.
    let after = match &caller {
        Some(location) => {
            let mut command = tmux();
            command.args([
                "display-message",
                "-p",
                "-t",
                &location.pane,
                "#{window_id}",
            ]);
            Some(run_host(command, "tmux")?.trim().to_owned())
        }
        None => None,
    };
    let mut command = tmux();
    command.arg("new-window");
    if let Some(window) = &after {
        command.args(["-a", "-t", window]);
    }
    if !focus {
        command.arg("-d");
    }
    command
        .args([
            "-P",
            "-F",
            "#{pane_id}",
            "-n",
            &target.destination_name(),
            "-c",
        ])
        .arg(target.cwd())
        .arg(&shell);
    let output = run_host(command, "tmux")?;
    let pane = output.trim();
    anyhow::ensure!(!pane.is_empty(), "tmux reply named no pane");
    Ok(pane.to_owned())
}

pub(super) fn focus(destination: &str) -> Result<()> {
    let location: TmuxLocation =
        serde_json::from_str(destination).context("Recorded tmux view is unreadable")?;
    let mut command = host_command(&location);
    command.args([
        "select-window",
        "-t",
        &location.pane,
        ";",
        "select-pane",
        "-t",
        &location.pane,
    ]);
    run_host(command, OpenHost::Tmux.label())?;
    let mut switch = host_command(&location);
    switch.args(["switch-client", "-t", &location.pane]);
    run_host(switch, OpenHost::Tmux.label()).map(drop)
}
