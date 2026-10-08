use std::process::Command;

use anyhow::{bail, Context};

pub fn run() -> anyhow::Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("Loopflow.app is available on macOS only");
    }

    let status = Command::new("open")
        .args(["-a", "Loopflow"])
        .status()
        .context("launch Loopflow.app with the macOS `open` command")?;
    if !status.success() {
        bail!("cannot launch Loopflow.app: `open -a Loopflow` exited with {status}");
    }
    Ok(())
}
