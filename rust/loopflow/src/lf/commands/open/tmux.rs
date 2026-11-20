//! tmux adapter: open and focus conversation windows in the caller's session.
//! Task split layouts are handled only by Rex, Herdr and cmux.
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

use super::{run_host, viewed_placement, OpenHost, Resolved};

use loopflow_execution::os_process::which_on_path;
use loopflow_execution::terminal_host::TmuxLocation;

/// A tmux client for the server at `location`, else the default server.
/// `tmux` is resolved here because `open` runs its client with no `PATH`.
fn host_command(location: Option<&TmuxLocation>) -> Command {
    let tmux = which_on_path(Path::new("tmux")).unwrap_or_else(|| "tmux".into());
    let mut command = Command::new(tmux);
    if let Some(location) = location {
        command.args(["-S", &location.socket]);
    }
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
    // tmux starts a window on the PATH of the client that asked for it. This
    // client has none, so the window starts on the server's, as a pane does
    // in every other host.
    let mut command = host_command(caller.as_ref());
    command.env_remove("PATH").arg("new-window");
    if let Some(location) = &caller {
        // `new-window -t` names a window: tmux 3.7 refuses a pane there.
        let mut lookup = host_command(Some(location));
        lookup.args([
            "display-message",
            "-p",
            "-t",
            &location.pane,
            "#{window_id}",
        ]);
        command.args(["-a", "-t", run_host(lookup, "tmux")?.trim()]);
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
    let mut command = host_command(Some(&location));
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
    let mut switch = host_command(Some(&location));
    switch.args(["switch-client", "-t", &location.pane]);
    run_host(switch, OpenHost::Tmux.label()).map(drop)
}
