use std::process::Command;

use anyhow::{bail, Context};

pub fn require_supported() -> anyhow::Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("Loopflow Desktop launching and control require macOS; use `lf task status <task>` or `lf --task <task>` in this terminal instead");
    }

    Ok(())
}

pub fn run() -> anyhow::Result<()> {
    require_supported()?;
    let status = Command::new("open")
        .args(["-a", "Loopflow"])
        .status()
        .context("launch Loopflow.app with the macOS `open` command")?;
    if !status.success() {
        bail!("cannot launch Loopflow.app: `open -a Loopflow` exited with {status}");
    }
    Ok(())
}
