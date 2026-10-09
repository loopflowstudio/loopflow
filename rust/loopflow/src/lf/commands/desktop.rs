//! Passive control-plane reads use Desktop's existing Apple event boundary.
//! No file-backed layout cache, socket server, or Work mutation participates.
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

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
    pub selected_session: Option<String>,
    pub reading: String,
    pub reason: Option<String>,
    pub recommended_action: Option<String>,
    pub action_reason: Option<String>,
    pub supported_operations: Vec<String>,
    pub workspaces: Vec<DesktopWorkspaceInspection>,
    pub layouts: Vec<DesktopWorktreeInspection>,
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

// Raw event spelling avoids loading an installed scripting dictionary during
// compilation. Checking running state precedes the tell: inspection never opens
// an app or an unrelated workspace. No caller-controlled text enters the script.
const INSPECT: &str = r#"
if application id "com.loopflow.mac" is not running then
    error "Loopflow Desktop is not running. Open it with lf open, or inspect Work with lf task status <task>."
end if
with timeout of 5 seconds
    tell application id "com.loopflow.mac" to «event CNRTinsp»
end timeout
"#;

pub fn inspect(json: bool) -> Result<()> {
    super::open::require_supported()?;
    let output = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", INSPECT])
        .output()
        .context("read Loopflow Desktop via macOS automation")?;
    if !output.status.success() {
        bail!("Desktop inspection unavailable: {}. No workspace was opened or changed; `lf task status <task>` remains available in this terminal.", String::from_utf8_lossy(&output.stderr).trim());
    }
    let reading: DesktopInspection = serde_json::from_slice(&output.stdout)
        .context("Desktop returned an invalid inspection reading")?;
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
                window.selected_session.as_deref().unwrap_or("unbound")
            ));
            if let Some(reason) = &window.reason {
                lines.push(format!("  {reason}"));
            }
            if let Some(action) = &window.recommended_action {
                lines.push(format!(
                    "  Rust action: {action} · {}",
                    window.action_reason.as_deref().unwrap_or("")
                ));
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
                "{:indent$}{pane}: {} {}",
                "",
                self.content.as_deref().unwrap_or("unknown"),
                self.subject.as_deref().unwrap_or("")
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
