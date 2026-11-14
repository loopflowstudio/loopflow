//! cmux through its CLI (0.65.0): a group is the repository/Wave container and
//! each Task is a workspace with a `--layout` split tree. Surface commands are
//! typed into fresh shells, so the renderer quotes every argument. A Task opened
//! inside cmux joins the group its caller's workspace is in now, read from cmux.
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::work::selection::SplitAxis;

/// The app bundles its CLI without adding it to PATH.
const BUNDLED: &str = "/Applications/cmux.app/Contents/Resources/bin/cmux";

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub(super) struct CmuxLocation {
    socket: String,
    workspace: String,
}

impl CmuxLocation {
    pub(super) fn current() -> Option<Self> {
        Some(Self {
            socket: std::env::var("CMUX_SOCKET_PATH").ok()?,
            workspace: std::env::var("CMUX_WORKSPACE_ID").ok()?,
        })
    }
}

pub(super) struct Cmux {
    cli: PathBuf,
    location: Option<CmuxLocation>,
}

impl Cmux {
    pub(super) fn local() -> Result<Self> {
        let location = match super::viewed_placement() {
            Some((super::OpenHost::Cmux, value)) => {
                Some(serde_json::from_str(&value).context("cmux viewer placement is unavailable")?)
            }
            _ => CmuxLocation::current(),
        };
        Ok(Self {
            cli: cli()?,
            location,
        })
    }

    fn run(&self, args: &[&str]) -> Result<String> {
        let mut command = Command::new(&self.cli);
        command.env("CMUX_QUIET", "1").args(args);
        if let Some(location) = &self.location {
            command.env("CMUX_SOCKET_PATH", &location.socket);
        }
        super::run_host(command, "cmux")
    }

    /// The group holding the caller's workspace now; `None` when it is ungrouped
    /// or the caller is outside cmux. Native lookup follows moves since launch.
    pub(super) fn caller_group(&self) -> Result<Option<String>> {
        let Some(location) = &self.location else {
            return Ok(None);
        };
        let output = self
            .run(&["workspace-group", "list", "--json"])
            .context("cmux did not report the caller's group; nothing was opened")?;
        let list: Value = serde_json::from_str(&output)
            .context("cmux group list is unreadable; nothing was opened")?;
        group_of(&list, &location.workspace)
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

    pub(super) fn focus_view(placement: &str) -> Result<()> {
        let location: CmuxLocation = serde_json::from_str(placement)
            .context("The recorded cmux view has no socket and workspace identity")?;
        let workspace = location.workspace.clone();
        Self {
            cli: cli()?,
            location: Some(location),
        }
        .focus(&workspace)
    }

    pub(super) fn focus(&self, workspace: &str) -> Result<()> {
        self.run(&["workspace", "select", workspace]).map(drop)
    }
}

// Native layout rendering.
pub(super) fn terminal(label: &str, cwd: &Path, argv: Vec<String>) -> Value {
    // cmux types into a fresh shell. Enter the checkout and quote each argument.
    json!({"pane":{"surfaces":[{"type":"terminal","name":label,
        "command":format!("cd {} && {}", shell_quote(&cwd.to_string_lossy()),
            argv.iter().map(|word| shell_quote(word)).collect::<Vec<_>>().join(" "))}]}})
}

pub(super) fn split(axis: SplitAxis, ratio: f64, before: Value, after: Value) -> Value {
    json!({
        "direction": if axis == SplitAxis::Vertical { "horizontal" } else { "vertical" },
        "split": ratio, "children": [before, after],
    })
}

fn shell_quote(word: &str) -> String {
    format!("'{}'", word.replace('\'', r"'\''"))
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

/// The group whose anchor or members include `workspace`, from a
/// `workspace.group.list` payload.
fn group_of(list: &Value, workspace: &str) -> Result<Option<String>> {
    let groups = list
        .get("groups")
        .or_else(|| list.pointer("/result/groups"))
        .and_then(Value::as_array)
        .context("cmux group list omitted groups; membership is unknown")?;
    let member = |group: &&Value| {
        group["anchor_workspace_id"] == workspace
            || group["member_workspace_ids"]
                .as_array()
                .is_some_and(|ids| ids.iter().any(|id| id == workspace))
    };
    groups
        .iter()
        .find(member)
        .map(|group| {
            group["id"]
                .as_str()
                .map(str::to_owned)
                .context("cmux group omitted its id")
        })
        .transpose()
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
    use super::{group_of, reference, split, terminal};
    use serde_json::json;
    use std::path::Path;

    #[test]
    fn caller_joins_the_group_holding_its_workspace() {
        let list = json!({"window_id":"w","groups":[
            {"id":"g1","anchor_workspace_id":"a","member_workspace_ids":["a","b"]},
            {"id":"g2","anchor_workspace_id":"c","member_workspace_ids":["c"]}
        ]});
        assert_eq!(group_of(&list, "b").unwrap().as_deref(), Some("g1"));
        assert_eq!(group_of(&list, "c").unwrap().as_deref(), Some("g2"));
        assert_eq!(group_of(&list, "z").unwrap(), None);
        assert!(group_of(&json!({}), "b").is_err());
    }

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
    #[test]
    fn cmux_surfaces_enter_the_checkout_with_quoted_argv() {
        let pane = terminal(
            "Files",
            Path::new("/repo/it's"),
            vec!["lf".into(), "task".into(), "a b".into()],
        );
        assert_eq!(
            pane["pane"]["surfaces"][0]["command"],
            r"cd '/repo/it'\''s' && 'lf' 'task' 'a b'"
        );
        let split = split(
            crate::work::selection::SplitAxis::Vertical,
            0.6,
            pane.clone(),
            pane,
        );
        assert_eq!(split["direction"], "horizontal");
        assert_eq!(split["children"].as_array().unwrap().len(), 2);
    }
}
