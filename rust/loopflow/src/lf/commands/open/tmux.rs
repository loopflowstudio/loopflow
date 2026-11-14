//! tmux adapter: open and focus conversation windows in the caller's session.
//! Task split layouts are handled only by Rex, Herdr and cmux.
use std::process::Command;

use anyhow::{Context, Result};

use super::{run_host, viewed_placement, OpenHost, Resolved};

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct TmuxLocation {
    socket: String,
    pane: String,
}

impl TmuxLocation {
    pub(super) fn current() -> Option<Self> {
        let tmux = std::env::var("TMUX").ok()?;
        Some(Self {
            socket: tmux.rsplitn(3, ',').nth(2)?.into(),
            pane: std::env::var("TMUX_PANE").ok()?,
        })
    }

    fn command(&self) -> Command {
        let mut command = Command::new("tmux");
        command.args(["-S", &self.socket]);
        command
    }
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
    let window = |after: Option<&str>| {
        let mut command = caller
            .as_ref()
            .map(TmuxLocation::command)
            .unwrap_or_else(|| Command::new("tmux"));
        command.arg("new-window");
        if let Some(pane) = after {
            command.args(["-a", "-t", pane]);
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
        run_host(command, "tmux")
    };
    let output = window(caller.as_ref().map(|location| location.pane.as_str()))?;
    let pane = output.trim();
    anyhow::ensure!(!pane.is_empty(), "tmux reply named no pane");
    Ok(pane.to_owned())
}

pub(super) fn focus(destination: &str) -> Result<()> {
    let location: TmuxLocation =
        serde_json::from_str(destination).context("Recorded tmux view is unreadable")?;
    let mut command = location.command();
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
    let mut switch = location.command();
    switch.args(["switch-client", "-t", &location.pane]);
    run_host(switch, OpenHost::Tmux.label()).map(drop)
}
