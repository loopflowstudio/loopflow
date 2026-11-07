//! tmux adapter: open and focus conversation windows in the caller's session.
//! Task split layouts are handled only by Rex, Herdr and cmux.
use std::process::Command;

use super::{run_host, viewed_placement, OpenHost, Opened, Resolved, RouteError, TMUX_PANE};

/// A new window right after the caller's, named for the Work. Its pane is the
/// destination: pane IDs are unique for the server's life, unlike indexes.
pub(super) fn open(target: &Resolved, focus: bool, lf: &str) -> Result<Opened, RouteError> {
    let shell = target.resume_shell(lf)?;
    let window = |after: Option<&str>| {
        let mut command = Command::new("tmux");
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
    // A pane recorded at launch may be gone; the server's current session remains.
    let caller = match viewed_placement() {
        Some((OpenHost::Tmux, pane)) => Some(pane),
        _ => std::env::var(TMUX_PANE)
            .ok()
            .filter(|pane| !pane.is_empty()),
    };
    let output = match caller.as_deref().map(|pane| window(Some(pane))) {
        Some(Err(RouteError::Definite(_))) | None => window(None)?,
        Some(result) => result?,
    };
    let pane = output.trim();
    if pane.is_empty() {
        return Err(RouteError::Unknown("tmux reply named no pane".into()));
    }
    Ok(Opened::opening(Some(pane.to_owned())))
}

pub(super) fn focus(destination: &str) -> Result<(), RouteError> {
    // Selecting proves the pane still exists; a client in another session
    // follows it when one is attached.
    let mut command = Command::new("tmux");
    command.args(["select-window", "-t", destination, ";"]);
    command.args(["select-pane", "-t", destination]);
    let selected = run_host(command, OpenHost::Tmux.label()).map(drop);
    if selected.is_ok() {
        let mut switch = Command::new("tmux");
        switch.args(["switch-client", "-t", destination]);
        let _ = run_host(switch, OpenHost::Tmux.label());
    }
    selected.inspect_err(|error| {
        tracing::debug!(reason = error.reason(), "cannot focus the destination");
    })
}
