//! Retained pane arrangement, explicit input and passive reads use Desktop's existing Apple event boundary.
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
    pub surface: Option<String>,
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
pub struct DesktopPaneCommand {
    pub target: DesktopPaneTarget,
    pub action: DesktopPaneAction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum DesktopPaneAction {
    Text {
        surface: String,
        text: String,
    },
    Key {
        surface: String,
        key: DesktopKey,
    },
    Hide,
    Restore,
    Focus,
    Shell,
    Files {
        task: String,
    },
    FlowLog {
        task: String,
    },
    Split {
        axis: DesktopSplitAxis,
    },
    Move {
        destination: DesktopPaneTarget,
        axis: DesktopSplitAxis,
    },
    Resize {
        toward: DesktopPaneTarget,
        ratio: f64,
    },
    Zoom {
        enabled: bool,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DesktopKey {
    Enter,
    Tab,
    Escape,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DesktopSplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopTextRequest {
    pub target: DesktopPaneTarget,
    pub surface: String,
    pub region: DesktopTextRegion,
    pub max_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DesktopTextRegion {
    Screen,
    Scrollback,
    Selection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopTextReading {
    pub request: DesktopTextRequest,
    pub observed_at: i64,
    pub hidden: bool,
    pub result: DesktopTextResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
#[non_exhaustive]
pub enum DesktopTextResult {
    Available { text: String, truncated: bool },
    Unavailable { reason: DesktopTextUnavailable },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DesktopTextUnavailable {
    MissingSurface,
    NotTerminal,
    BoundedReaderUnavailable,
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
            if (count of argv) is 2 then
                tell application id "com.loopflow.mac" to return «event CNRTtext» request
            else
                tell application id "com.loopflow.mac" to return «event CNRTpane» request
            end if
        end if
    end timeout
end run
"#;

pub fn run(command: &crate::lf::DesktopCommand) -> Result<()> {
    use crate::lf::DesktopCommand;
    // Check before decoding targets or looking up any Work/Machine.
    super::open::require_supported()?;
    let (target, action, json) = match command {
        DesktopCommand::List { json } => return invoke(None, *json),
        DesktopCommand::Text {
            target,
            surface,
            text,
            json,
        } => {
            validate_literal_text(text)?;
            (
                target,
                DesktopPaneAction::Text {
                    surface: surface.clone(),
                    text: text.clone(),
                },
                json,
            )
        }
        DesktopCommand::Key {
            target,
            surface,
            key,
            json,
        } => (
            target,
            DesktopPaneAction::Key {
                surface: surface.clone(),
                key: *key,
            },
            json,
        ),
        DesktopCommand::Read {
            target,
            surface,
            region,
            max_bytes,
            json,
        } => {
            if !(1..=1_048_576).contains(max_bytes) {
                bail!("Text byte limit must be between 1 and 1048576");
            }
            let request = DesktopTextRequest {
                target: parse_target(target)?,
                surface: surface.clone(),
                region: *region,
                max_bytes: *max_bytes,
            };
            let encoded = serde_json::to_string(&request)?;
            let output = contact(Some(&encoded), true)?;
            let reading: DesktopTextReading = serde_json::from_slice(&output)
                .context("Desktop returned an invalid terminal text reading")?;
            if reading.request != request {
                bail!("Desktop returned text for a different request");
            }
            if matches!(&reading.result, DesktopTextResult::Available { text, .. } if text.len() > request.max_bytes)
            {
                bail!("Desktop returned text exceeding the requested byte limit");
            }
            if *json {
                println!("{}", serde_json::to_string_pretty(&reading)?);
            } else {
                match &reading.result {
                    DesktopTextResult::Available { text, truncated } => {
                        print!("{text}");
                        if *truncated {
                            eprintln!("\n[Desktop text truncated]");
                        }
                    }
                    DesktopTextResult::Unavailable { reason } => bail!("{}", reason.message()),
                }
            }
            return Ok(());
        }
        DesktopCommand::Hide { target, json } => (target, DesktopPaneAction::Hide, json),
        DesktopCommand::Restore { target, json } => (target, DesktopPaneAction::Restore, json),
        DesktopCommand::Focus { target, json } => (target, DesktopPaneAction::Focus, json),
        DesktopCommand::Shell { target, json } => (target, DesktopPaneAction::Shell, json),
        DesktopCommand::Files { target, task, json } => (
            target,
            DesktopPaneAction::Files { task: task.clone() },
            json,
        ),
        DesktopCommand::FlowLog { target, task, json } => (
            target,
            DesktopPaneAction::FlowLog { task: task.clone() },
            json,
        ),
        DesktopCommand::Split { target, axis, json } => {
            (target, DesktopPaneAction::Split { axis: *axis }, json)
        }
        DesktopCommand::Move {
            target,
            destination,
            axis,
            json,
        } => (
            target,
            DesktopPaneAction::Move {
                destination: parse_target(destination)?,
                axis: *axis,
            },
            json,
        ),
        DesktopCommand::Resize {
            target,
            toward,
            ratio,
            json,
        } => {
            if !ratio.is_finite() || !(0.1..=0.9).contains(ratio) {
                bail!("Split ratio must be between 0.1 and 0.9");
            }
            (
                target,
                DesktopPaneAction::Resize {
                    toward: parse_target(toward)?,
                    ratio: *ratio,
                },
                json,
            )
        }
        DesktopCommand::Zoom { target, off, json } => {
            (target, DesktopPaneAction::Zoom { enabled: !off }, json)
        }
    };
    let request = serde_json::to_string(&DesktopPaneCommand {
        target: parse_target(target)?,
        action,
    })?;
    invoke(Some(&request), *json)
}

fn validate_literal_text(text: &str) -> Result<()> {
    if text.chars().any(char::is_control) {
        bail!("Literal text cannot contain control characters; use `lf desktop key` for explicit keys. No input was sent.");
    }
    Ok(())
}

fn parse_target(target: &str) -> Result<DesktopPaneTarget> {
    serde_json::from_str(target)
        .context("expected the exact pane target from `lf desktop list --json`")
}

fn contact(request: Option<&str>, text_read: bool) -> Result<Vec<u8>> {
    super::open::require_supported()?;
    let output = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", DESKTOP_EVENT, "--"])
        .args(request)
        .args(text_read.then_some("text"))
        .output()
        .context("contact Loopflow Desktop via macOS automation")?;
    if !output.status.success() {
        // A lost reply does not prove the command had no effect. In particular,
        // repeating a split allocates another pane; inspect before retrying.
        bail!("Desktop request failed: {}. Inspect again before retrying; `lf task status <task>` remains available in this terminal.", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(output.stdout)
}

fn invoke(request: Option<&str>, json: bool) -> Result<()> {
    let output = contact(request, false)?;
    let reading: DesktopInspection = serde_json::from_slice(&output)
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
                "{:indent$}{pane}: {} {} · content occurrence {} · surface {}",
                "",
                self.content.as_deref().unwrap_or("unknown"),
                self.subject.as_deref().unwrap_or(""),
                self.incarnation.as_deref().unwrap_or("unavailable"),
                self.surface.as_deref().unwrap_or("unavailable")
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

impl DesktopTextUnavailable {
    fn message(self) -> &'static str {
        match self {
            Self::MissingSurface => "The retained pane has no native surface; no client was acquired.",
            Self::NotTerminal => "The addressed pane is not a terminal.",
            Self::BoundedReaderUnavailable => "This Desktop build has no verified bounded text reader; no unbounded fallback was used.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::validate_literal_text;

    #[test]
    fn literal_text_preserves_unicode_but_cannot_smuggle_keys() {
        for text in ["", "héλ🙂 e\u{301}", "\"$(echo literal)\"; \\n"] {
            validate_literal_text(text).unwrap();
        }
        for text in [
            "line\nsubmit",
            "\r",
            "\t",
            "\0",
            "\u{1b}[A",
            "\u{7f}",
            "\u{85}",
        ] {
            assert!(validate_literal_text(text).is_err(), "{text:?}");
        }
    }
}
