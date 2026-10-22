//! `lf install agent <codex|claude>`: run a coding tool's official native
//! installer after an explicit request, then verify the executable it produced.
//!
//! The provider owns its binary and later updates. Loopflow only fetches the
//! provider's script to a file (so a failed download cannot run as an empty
//! pipeline), runs it with the terminal attached, and re-probes uncached.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use reqwest::blocking::Client;

use crate::agent::{guided_agent, GuidedAgent, GUIDED_AGENTS};

pub fn install_agent(name: &str) -> Result<()> {
    let agent = guided_agent(name).ok_or_else(|| {
        anyhow!(
            "Loopflow installs {}; install '{name}' with its own instructions",
            GUIDED_AGENTS.map(|agent| agent.name).join(" or ")
        )
    })?;
    let home = dirs::home_dir().context("cannot determine the home directory")?;
    println!(
        "Installing {} from {}",
        agent.display_name, agent.installer_url
    );
    let script = Client::builder()
        .timeout(Duration::from_secs(60))
        .build()?
        .get(agent.installer_url)
        .send()
        .and_then(|response| response.error_for_status())
        .and_then(|response| response.bytes())
        .with_context(|| format!("download the {} installer", agent.display_name))?;
    let executable = run_installer(&agent, &script, &home)?;
    println!(
        "{} is installed: {}\nNext: lf account connect {}",
        agent.display_name,
        executable.display(),
        agent.name
    );
    Ok(())
}

fn run_installer(agent: &GuidedAgent, script: &[u8], home: &Path) -> Result<PathBuf> {
    if script.is_empty() {
        bail!(
            "the {} installer download was empty; nothing was run",
            agent.display_name
        );
    }
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("install.sh");
    std::fs::write(&path, script)?;
    let status = Command::new(agent.shell)
        .arg(&path)
        .current_dir(home)
        .stdin(Stdio::inherit())
        .status()
        .with_context(|| format!("start the {} installer", agent.display_name))?;
    if !status.success() {
        bail!(
            "the {} installer failed ({status}); see its output above, then retry",
            agent.display_name
        );
    }
    installed_executable(agent.name, home).ok_or_else(|| {
        anyhow!(
            "the {} installer finished, but `{}` does not run from PATH or {}",
            agent.display_name,
            agent.name,
            home.join(".local/bin").display()
        )
    })
}

/// An executable that answers `--version`, probed fresh: a process-lifetime
/// cache would still remember it as missing.
fn installed_executable(name: &str, home: &Path) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain([home.join(".local/bin")])
        .map(|directory| directory.join(name))
        .filter(|candidate| candidate.is_file())
        .find(|candidate| {
            Command::new(candidate)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .is_ok_and(|status| status.success())
        })
}

#[cfg(test)]
mod tests {
    use super::{install_agent, run_installer};
    use crate::agent::guided_agent;

    #[test]
    fn verified_executable_is_the_success_signal() {
        let home = tempfile::tempdir().unwrap();
        let mut claude = guided_agent("claude").unwrap();
        claude.name = "lf-test-claude";
        claude.shell = "/bin/sh";
        let installs = b"mkdir -p .local/bin\nprintf '#!/bin/sh\\nexit 0\\n' > .local/bin/lf-test-claude\nchmod +x .local/bin/lf-test-claude\n";
        let installed = run_installer(&claude, installs, home.path()).unwrap();
        assert_eq!(installed, home.path().join(".local/bin/lf-test-claude"));

        let silent = tempfile::tempdir().unwrap();
        let error = run_installer(&claude, b"exit 0\n", silent.path()).unwrap_err();
        assert!(error.to_string().contains("does not run from PATH"));
        let error = run_installer(&claude, b"exit 7\n", silent.path()).unwrap_err();
        assert!(error.to_string().contains("installer failed"));
        let error = run_installer(&claude, b"", silent.path()).unwrap_err();
        assert!(error.to_string().contains("nothing was run"));
    }

    #[test]
    fn unguided_tools_are_refused_before_any_download() {
        let error = install_agent("opencode").unwrap_err();
        assert!(error.to_string().contains("installs codex or claude"));
    }
}
