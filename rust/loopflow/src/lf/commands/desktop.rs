//! Passive control-plane reads use Desktop's existing Apple event boundary.
//! No file-backed layout cache, socket server, or Work mutation participates.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::ops::human_session::SessionAction;
use crate::ops::task_actions::TaskActionModel;
use crate::ops::task_run::TaskRunControl;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopInspection {
    pub observed_at: i64,
    pub windows: Vec<DesktopWindowInspection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopWindowInspection {
    pub repository: String,
    pub window: String,
    pub path: Option<String>,
    pub selection_kind: Option<String>,
    pub selection_id: Option<String>,
    pub reading: String,
    pub reason: Option<String>,
    pub task: Option<DesktopTaskInspection>,
    pub session: Option<DesktopSessionInspection>,
    pub supported_operations: Vec<String>,
    pub workspaces: Vec<DesktopWorkspaceInspection>,
    pub layouts: Vec<DesktopWorktreeInspection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopTaskInspection {
    pub id: String,
    pub reading: String,
    pub reason: Option<String>,
    pub roadmap_generated_at: Option<String>,
    pub condition_observed_at: Option<String>,
    pub actions: Option<TaskActionModel>,
    pub run_control: Option<TaskRunControl>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopSessionInspection {
    pub id: String,
    pub machine_id: Option<String>,
    pub reading: String,
    pub reason: Option<String>,
    pub observed_at: Option<String>,
    pub actions: Option<Vec<SessionAction>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopWorkspaceInspection {
    pub machine_id: String,
    pub worktree: String,
    pub layout: DesktopLayoutInspection,
    pub focused_pane: String,
    pub zoomed_pane: Option<String>,
    pub hidden_panes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopLayoutInspection {
    pub pane: Option<String>,
    pub incarnation: Option<String>,
    pub content: Option<String>,
    pub subject: Option<String>,
    pub axis: Option<String>,
    pub ratio: Option<f64>,
    pub children: Vec<DesktopLayoutInspection>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopWorktreeInspection {
    pub machine_id: String,
    pub repository_path: String,
    pub focused_slot: String,
    pub layout: DesktopWorktreeNode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopWorktreeNode {
    pub slot: Option<String>,
    pub machine_id: Option<String>,
    pub worktree: Option<String>,
    pub axis: Option<String>,
    pub children: Vec<DesktopWorktreeNode>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopPaneTarget {
    pub repository: String,
    pub window: String,
    pub machine_id: String,
    pub worktree: String,
    pub pane: String,
    pub incarnation: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopPaneVisibility {
    pub target: DesktopPaneTarget,
    pub hidden: bool,
}

// Raw event spelling avoids loading an installed scripting dictionary during
// compilation. Checking running state precedes the tell: inspection never opens
// an app or an unrelated workspace. No caller-controlled text enters the script.
const DESKTOP_EVENT: &str = r#"
on run argv
    if application id "com.loopflow.mac" is not running then
        error "Loopflow Desktop is not running. Open it with lf open, or inspect Work with lf task status <task>."
    end if
    with timeout of 5 seconds
        if (count of argv) is 0 then
            tell application id "com.loopflow.mac" to return «event CNRTinsp»
        else
            set request to item 1 of argv
            tell application id "com.loopflow.mac" to return «event CNRTpvis» request
        end if
    end timeout
end run
"#;

pub fn inspect(json: bool) -> Result<()> {
    invoke(None, json)
}

pub fn set_visibility(target: &str, hidden: bool, json: bool) -> Result<()> {
    super::open::require_supported()?;
    let target = serde_json::from_str::<DesktopPaneTarget>(target)
        .context("expected the exact pane target from `lf desktop inspect --json`")?;
    let request = serde_json::to_string(&DesktopPaneVisibility { target, hidden })?;
    invoke(Some(&request), json)
}

fn invoke(request: Option<&str>, json: bool) -> Result<()> {
    super::open::require_supported()?;
    let output = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", DESKTOP_EVENT, "--"])
        .args(request)
        .output()
        .context("contact Loopflow Desktop via macOS automation")?;
    if !output.status.success() {
        // A lost reply does not prove the command had no effect. Visibility is
        // idempotent; inspect again before deciding whether to repeat it.
        bail!("Desktop request failed: {}. Inspect again before retrying; `lf task status <task>` remains available in this terminal.", String::from_utf8_lossy(&output.stderr).trim());
    }
    let reading: DesktopInspection = serde_json::from_slice(&output.stdout)
        .context("Desktop returned an invalid reading; inspect again before retrying")?;
    if json {
        println!("{}", serde_json::to_string_pretty(&reading)?);
    } else {
        println!("{}", reading.render());
    }
    Ok(())
}

impl DesktopInspection {
    pub fn render(&self) -> String {
        let mut lines = vec![format!("Desktop reading: {}", self.observed_at)];
        if self.windows.is_empty() {
            lines.push("No repository workspace windows are open.".into());
        }
        for window in &self.windows {
            lines.push(format!(
                "Repository {} · window {} · {}",
                window.repository, window.window, window.reading
            ));
            lines.push(format!(
                "  Work: {} {} · Session: {}",
                window.selection_kind.as_deref().unwrap_or("unbound"),
                window.selection_id.as_deref().unwrap_or(""),
                window
                    .session
                    .as_ref()
                    .map(|session| session.id.as_str())
                    .unwrap_or("unbound")
            ));
            if let Some(reason) = &window.reason {
                lines.push(format!("  {reason}"));
            }
            if let Some(task) = &window.task {
                lines.push(format!(
                    "  Task {}: {} · {}",
                    task.id,
                    task.reading,
                    task.reason.as_deref().unwrap_or("")
                ));
                lines.push(format!(
                    "    Roadmap generated: {} · condition observed: {}",
                    task.roadmap_generated_at
                        .as_deref()
                        .unwrap_or("unavailable"),
                    task.condition_observed_at
                        .as_deref()
                        .unwrap_or("unavailable")
                ));
                if let Some(actions) = &task.actions {
                    lines.push(format!(
                        "    Recommended: {} · {}",
                        actions
                            .recommended
                            .map(|action| action.as_str())
                            .unwrap_or("none"),
                        actions.reason
                    ));
                }
                if let Some(control) = &task.run_control {
                    lines.push(format!(
                        "    Run: {}",
                        control.unavailable.as_deref().unwrap_or("available")
                    ));
                }
            }
            if let Some(session) = &window.session {
                lines.push(format!(
                    "  Session {} on {}: {} · {}",
                    session.id,
                    session.machine_id.as_deref().unwrap_or("unavailable"),
                    session.reading,
                    session.reason.as_deref().unwrap_or("")
                ));
                lines.push(format!(
                    "    Observed: {}",
                    session.observed_at.as_deref().unwrap_or("unavailable")
                ));
                for action in session.actions.iter().flatten() {
                    lines.push(format!(
                        "    {}: {} · {}",
                        action.label,
                        action.unavailable_reason.as_deref().unwrap_or("available"),
                        action.help
                    ));
                }
            }
            lines.push(format!(
                "  Supported: {}",
                window.supported_operations.join(", ")
            ));
            for layout in &window.layouts {
                lines.push(format!(
                    "  Workspace slots for {}:{} · focused {}",
                    layout.machine_id, layout.repository_path, layout.focused_slot
                ));
                layout.layout.render(4, &mut lines);
            }
            for workspace in &window.workspaces {
                lines.push(format!(
                    "  {}:{} · focused {} · zoom {} · hidden [{}]",
                    workspace.machine_id,
                    workspace.worktree,
                    workspace.focused_pane,
                    workspace.zoomed_pane.as_deref().unwrap_or("none"),
                    workspace.hidden_panes.join(", ")
                ));
                workspace.layout.render(4, &mut lines);
            }
        }
        lines.join("\n")
    }
}

impl DesktopLayoutInspection {
    fn render(&self, indent: usize, lines: &mut Vec<String>) {
        if let Some(pane) = &self.pane {
            lines.push(format!(
                "{:indent$}{pane}: {} {} · content occurrence {}",
                "",
                self.content.as_deref().unwrap_or("unknown"),
                self.subject.as_deref().unwrap_or(""),
                self.incarnation.as_deref().unwrap_or("unavailable")
            ));
        } else {
            lines.push(format!(
                "{:indent$}split {} ratio {}",
                "",
                self.axis.as_deref().unwrap_or("unknown"),
                self.ratio
                    .map(|ratio| ratio.to_string())
                    .unwrap_or_else(|| "unknown".into())
            ));
            for child in &self.children {
                child.render(indent + 2, lines);
            }
        }
    }
}

impl DesktopWorktreeNode {
    fn render(&self, indent: usize, lines: &mut Vec<String>) {
        if let Some(slot) = &self.slot {
            lines.push(format!(
                "{:indent$}{slot}: {}:{}",
                "",
                self.machine_id.as_deref().unwrap_or("unbound"),
                self.worktree.as_deref().unwrap_or("unbound")
            ));
        } else {
            lines.push(format!(
                "{:indent$}split {}",
                "",
                self.axis.as_deref().unwrap_or("unknown")
            ));
            for child in &self.children {
                child.render(indent + 2, lines);
            }
        }
    }
}
