//! cmux through its CLI (0.65.0): a group is the repository/Wave container and
//! each Task is a workspace with a `--layout` split tree. Surface commands are
//! typed into fresh shells, so the renderer quotes every argument.
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde_json::Value;

/// The app bundles its CLI without adding it to PATH.
const BUNDLED: &str = "/Applications/cmux.app/Contents/Resources/bin/cmux";

pub(super) struct Cmux {
    cli: PathBuf,
}

impl Cmux {
    pub(super) fn local() -> Result<Self> {
        Ok(Self { cli: cli()? })
    }

    fn run(&self, args: &[&str]) -> Result<String> {
        let mut command = Command::new(&self.cli);
        command.env("CMUX_QUIET", "1").args(args);
        super::super::run_host(command, "cmux")
            .map_err(|error| anyhow::anyhow!("{}", error.reason()))
    }

    pub(super) fn create_group(&self, name: &str, cwd: &Path) -> Result<String> {
        let output = self.run(&[
            "workspace-group",
            "create",
            "--name",
            name,
            "--cwd",
            &cwd.to_string_lossy(),
        ])?;
        reference(&output, "workspace_group:")
            .context("cmux did not name the created group; outcome is unknown")
    }

    pub(super) fn create_workspace(
        &self,
        group: Option<&str>,
        name: &str,
        cwd: &Path,
        layout: &Value,
    ) -> Result<String> {
        let cwd = cwd.to_string_lossy();
        let layout = layout.to_string();
        let mut args = vec![
            "new-workspace",
            "--name",
            name,
            "--cwd",
            &cwd,
            "--layout",
            &layout,
            "--focus",
            "false",
        ];
        if let Some(group) = group {
            args.extend(["--group", group, "--group-placement", "end"]);
        }
        let output = self.run(&args)?;
        reference(&output, "workspace:")
            .context("cmux did not name the created workspace; outcome is unknown")
    }

    pub(super) fn focus(&self, workspace: &str) -> Result<()> {
        self.run(&["workspace", "select", workspace]).map(drop)
    }
}

/// `cmux` on PATH, else the CLI inside the installed app.
fn cli() -> Result<PathBuf> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join("cmux"))
        .chain([PathBuf::from(BUNDLED)])
        .find(|path| path.is_file())
        .context("cmux CLI is not installed; nothing was opened")
}

/// The first `prefix`-ref or UUID the CLI printed.
fn reference(output: &str, prefix: &str) -> Option<String> {
    let words = || {
        output
            .split(|c: char| c.is_whitespace() || c == '"' || c == ',')
            .filter(|word| !word.is_empty())
    };
    words()
        .find(|word| word.starts_with(prefix))
        .or_else(|| words().find(|word| uuid::Uuid::parse_str(word).is_ok()))
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::reference;

    #[test]
    fn created_references_are_read_from_cli_output() {
        assert_eq!(
            reference("OK workspace:7 created", "workspace:").as_deref(),
            Some("workspace:7")
        );
        assert_eq!(
            reference(
                "Created group 0b9c1f5e-2a1c-4c55-9a77-4a0d4e3b6f10",
                "workspace_group:"
            )
            .as_deref(),
            Some("0b9c1f5e-2a1c-4c55-9a77-4a0d4e3b6f10")
        );
        assert_eq!(reference("done", "workspace:"), None);
    }
}
