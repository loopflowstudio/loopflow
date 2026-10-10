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
    pub openings: Vec<DesktopOpening>,
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
    pub opening: Option<DesktopOpening>,
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
}

// Raw event spelling avoids loading an installed scripting dictionary during
// compilation. Checking running state precedes the tell: inspection never opens
// an app or an unrelated workspace. No caller-controlled text enters the script.
const DESKTOP_EVENT: &str = r#"
on run argv
    if application id "com.loopflow.mac" is not running then
        error "Loopflow Desktop is not running. Open it with lf desktop open, or inspect Work with lf task status <task>."
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

pub fn require_supported() -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("Loopflow Desktop launching and control require macOS; use `lf task status <task>` or `lf --task <task>` in this terminal instead");
    }

    Ok(())
}

/// LaunchServices acceptance is opening, never proof of a usable native target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopOpening {
    pub url: String,
    pub status: DesktopOpeningStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DesktopOpeningStatus {
    Opening,
    Usable,
    Failed,
}

/// A proposed opening, not a LaunchServices or native-readiness receipt.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DesktopOpenExplanation {
    pub resolution: crate::ops::context::ContextExplanation,
    pub url: Option<String>,
    pub impediments: Vec<String>,
    pub unavailable: Vec<String>,
}

impl DesktopOpenExplanation {
    pub fn render(&self) -> String {
        let mut text = self.resolution.render();
        if let Some(url) = &self.url {
            text.push_str(&format!(
                "\nOpen on this Machine, reusing the repository window: {url}"
            ));
        }
        for reason in &self.impediments {
            text.push_str(&format!("\nImpediment: {reason}"));
        }
        for reason in &self.unavailable {
            text.push_str(&format!("\nUnavailable: {reason}"));
        }
        text.push_str("\nNothing was executed; no app, checkout or Session was prepared.");
        text
    }
}

pub fn explain_open(
    cli: &crate::lf::Cli,
    session: Option<&str>,
    diff: bool,
) -> DesktopOpenExplanation {
    let mut impediments = Vec::new();
    if let Err(error) = require_supported() {
        impediments.push(error.to_string());
    }
    let (resolution, url) = match opening_context(cli, session) {
        Ok(resolution) => {
            let url = match opening_url(&resolution, diff) {
                Ok(url) => Some(url.into()),
                Err(error) => {
                    impediments.push(error.to_string());
                    None
                }
            };
            (resolution, url)
        }
        Err(error) => {
            impediments.push(error.to_string());
            (
                crate::ops::context::ContextExplanation::unavailable(error),
                None,
            )
        }
    };
    DesktopOpenExplanation {
        resolution,
        url,
        impediments,
        unavailable: vec![
            "Desktop installation, retained windows and native readiness were not inspected. Opening re-resolves Work; a URL does not reserve a window or an input target.".into(),
        ],
    }
}

fn open_work(cli: &crate::lf::Cli, session: Option<&str>, diff: bool, json: bool) -> Result<()> {
    let resolution = opening_context(cli, session)?;
    let url = opening_url(&resolution, diff)?;
    let status = std::process::Command::new("open")
        .args(["-a", "Loopflow", url.as_str()])
        .status()
        .context("launch Loopflow.app with the macOS `open` command")?;
    if !status.success() {
        bail!("cannot open Work in Loopflow.app: macOS open exited with {status}");
    }
    let opening = DesktopOpening {
        url: url.into(),
        status: DesktopOpeningStatus::Opening,
        reason: None,
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&opening)?);
    } else {
        println!("Opening {}\nInspect with `lf desktop list --json`; launch acceptance does not establish usability.", opening.url);
    }
    Ok(())
}

fn opening_context(
    cli: &crate::lf::Cli,
    session: Option<&str>,
) -> Result<crate::ops::context::ContextExplanation> {
    // A plain Git repository can open before planning has ever been initialized.
    // Only an absent registry allows this path; unreadable is not absent.
    if session.is_none()
        && cli.task.is_none()
        && cli.wave.is_none()
        && crate::store::read_existing_registry()?.is_none()
    {
        let repo = crate::repository::CanonicalRepo::current()?
            .context("select a Git repository with --repo before opening Desktop")?;
        let mut resolution = crate::ops::context::ContextExplanation::empty();
        resolution.repository_path = crate::ops::context::ContextFact::Bound {
            value: repo.to_string(),
            source: "local_locator".into(),
        };
        return Ok(resolution);
    }
    use crate::ops::context::ContextFact;
    let mut resolution = if cli.task.is_none() && session.is_some() {
        let selected = super::context::explain(None, None, session, None)?;
        if let Some(wave) = cli.wave.as_deref() {
            let work = super::context::explain(Some(wave), None, None, None)?;
            anyhow::ensure!(
                matches!((&work.wave, &selected.wave),
                (ContextFact::Bound { value: wave, .. }, ContextFact::Bound { value: session_wave, .. }) if wave == session_wave),
                "Session does not belong to the selected Work. No app was opened."
            );
        }
        selected
    } else {
        super::context::explain(cli.wave.as_deref(), cli.task.as_deref(), None, None)?
    };
    if cli.repo.is_some() || cli.repository.is_some() {
        if let crate::ops::context::ContextFact::Bound { value, .. } = &resolution.repository_path {
            anyhow::ensure!(
                crate::repository::CanonicalRepo::current()?.as_ref()
                    == Some(&crate::repository::CanonicalRepo::discover(
                        std::path::Path::new(value)
                    )?),
                "Work does not belong to the selected repository. No app was opened."
            );
        }
    }
    if let crate::ops::context::ContextFact::Bound { value, .. } = &resolution.task {
        let task = crate::durable::TaskId::parse(value)?;
        if let Some(store) = crate::store::read_existing_registry()? {
            // The shared reader retains failures in the explanation; opening_url
            // refuses unavailable execution without losing the other Work facts.
            let _ = tokio::runtime::Runtime::new()?.block_on(crate::ops::task_location::explain(
                &store.sqlite,
                &task,
                &mut resolution,
            ));
        }
    }
    if let Some(session) = session.filter(|_| cli.task.is_some()) {
        let remote = matches!((&resolution.execution_machine, &resolution.machine),
            (ContextFact::Bound { value: owner, .. }, ContextFact::Bound { value: local, .. }) if owner != local);
        if remote {
            // Native opening validates membership on this owner. A Session ID
            // absent from the presentation Machine is not a missing conversation.
            resolution.session = ContextFact::Bound {
                value: session.into(),
                source: "explicit_session_on_execution_machine".into(),
            };
        } else {
            let selected = super::context::explain(None, None, Some(session), None)?;
            anyhow::ensure!(
                matches!((&resolution.task, &selected.task),
                (ContextFact::Bound { value: task, .. }, ContextFact::Bound { value: session_task, .. }) if task == session_task),
                "Session does not belong to the selected Work. No app was opened."
            );
            resolution.session = selected.session;
            resolution.process = selected.process;
        }
    }
    Ok(resolution)
}

fn opening_url(
    resolution: &crate::ops::context::ContextExplanation,
    diff: bool,
) -> Result<reqwest::Url> {
    use crate::ops::context::ContextFact;
    let ContextFact::Bound { value: repo, .. } = &resolution.repository_path else {
        bail!("Repository location is unavailable; select a local repository with --repo. No app was opened.");
    };
    let task = match &resolution.task {
        ContextFact::Bound { value, .. } => Some(value.as_str()),
        ContextFact::Unbound => None,
        ContextFact::Unavailable { reason } => {
            bail!("Task resolution unavailable: {reason}. No app was opened.")
        }
    };
    let session = match &resolution.session {
        ContextFact::Bound { value, .. } => Some(value.as_str()),
        ContextFact::Unbound => None,
        ContextFact::Unavailable { reason } => {
            bail!("Session resolution unavailable: {reason}. No app was opened.")
        }
    };
    anyhow::ensure!(
        !diff || task.is_some(),
        "--diff needs a Task; select --task or a Task-associated --session. No app was opened."
    );
    anyhow::ensure!(session.is_none() || task.is_some(), "This Session has no single Task workspace; use `lf session connect` in a terminal. No app was opened.");
    let remote = if task.is_some() {
        match (&resolution.execution_machine, &resolution.machine) {
            (ContextFact::Bound { value: owner, .. }, ContextFact::Bound { value: local, .. })
                if owner != local =>
            {
                let ContextFact::Bound {
                    value: repository, ..
                } = &resolution.repository
                else {
                    bail!("Shared repository identity unavailable; no app was opened.");
                };
                anyhow::ensure!(
                    matches!(&resolution.checkout, ContextFact::Bound { .. }),
                    "Recorded checkout unavailable on Machine {owner}; no app was opened."
                );
                Some((owner, repository))
            }
            (ContextFact::Unavailable { reason }, _) => {
                bail!("Task execution location unavailable: {reason}. No app was opened.");
            }
            _ => None,
        }
    } else {
        None
    };
    let mut url = reqwest::Url::parse(if task.is_some() {
        "loopflow://task"
    } else {
        "loopflow://open"
    })?;
    if let Some(task) = task {
        url.path_segments_mut()
            .expect("Task URL supports paths")
            .push(task);
    }
    url.query_pairs_mut().append_pair("repo", repo);
    if let Some((owner, repository)) = remote {
        url.query_pairs_mut()
            .append_pair("machine", owner)
            .append_pair("repository", repository);
    }
    if let Some(session) = session {
        url.query_pairs_mut().append_pair("session", session);
    }
    if diff {
        url.query_pairs_mut().append_pair("diff", "true");
    }
    // Foundation URLComponents follows URI encoding, not form encoding:
    // '+' is literal there. Preserve spaces as %20 across the native boundary.
    let query = url.query().map(|query| query.replace('+', "%20"));
    url.set_query(query.as_deref());
    Ok(url)
}

pub fn run(cli: &crate::lf::Cli, command: &crate::lf::DesktopCommand) -> Result<()> {
    use crate::lf::DesktopCommand;
    require_supported()?;
    match command {
        DesktopCommand::Open {
            session,
            diff,
            json,
        } => return open_work(cli, session.as_deref(), *diff, *json || cli.json),
        DesktopCommand::List { json } => return invoke(None, *json || cli.json),
        _ => {}
    }
    // Work selects the repository/checkout. One live reading supplies every
    // lifetime token; dispatch never reinterprets this request using later focus.
    let resolution = opening_context(cli, None)?;
    let inspection: DesktopInspection = serde_json::from_slice(&contact(None, false)?)
        .context("Desktop returned an invalid reading")?;
    let selected = inspection.resolve_pane(&resolution, command)?;
    let target = selected.target()?;
    let (action, json) = match command {
        DesktopCommand::Open { .. } | DesktopCommand::List { .. } => unreachable!("handled above"),
        DesktopCommand::Text { text, json, .. } => {
            validate_literal_text(text)?;
            (
                DesktopPaneAction::Text {
                    surface: selected.surface()?,
                    text: text.clone(),
                },
                json,
            )
        }
        DesktopCommand::Key { key, json, .. } => (
            DesktopPaneAction::Key {
                surface: selected.surface()?,
                key: *key,
            },
            json,
        ),
        DesktopCommand::Read {
            region,
            max_bytes,
            json,
            ..
        } => {
            anyhow::ensure!(
                (1..=1_048_576).contains(max_bytes),
                "Text byte limit must be between 1 and 1048576"
            );
            return read_text(
                DesktopTextRequest {
                    target,
                    surface: selected.surface()?,
                    region: *region,
                    max_bytes: *max_bytes,
                },
                *json || cli.json,
            );
        }
        DesktopCommand::Hide { json, .. } => (DesktopPaneAction::Hide, json),
        DesktopCommand::Restore { json, .. } => (DesktopPaneAction::Restore, json),
        DesktopCommand::Focus { json, .. } => (DesktopPaneAction::Focus, json),
        DesktopCommand::Shell { json, .. } => (DesktopPaneAction::Shell, json),
        DesktopCommand::Files { json, .. } => (
            DesktopPaneAction::Files {
                task: bound(&resolution.task, "Task")?.into(),
            },
            json,
        ),
        DesktopCommand::FlowLog { json, .. } => (
            DesktopPaneAction::FlowLog {
                task: bound(&resolution.task, "Task")?.into(),
            },
            json,
        ),
        DesktopCommand::Split { axis, json, .. } => {
            (DesktopPaneAction::Split { axis: *axis }, json)
        }
        DesktopCommand::Move {
            destination,
            axis,
            json,
            ..
        } => (
            DesktopPaneAction::Move {
                destination: selected.peer(destination)?.target()?,
                axis: *axis,
            },
            json,
        ),
        DesktopCommand::Resize {
            toward,
            ratio,
            json,
            ..
        } => {
            anyhow::ensure!(
                ratio.is_finite() && (0.1..=0.9).contains(ratio),
                "Split ratio must be between 0.1 and 0.9"
            );
            (
                DesktopPaneAction::Resize {
                    toward: selected.peer(toward)?.target()?,
                    ratio: *ratio,
                },
                json,
            )
        }
        DesktopCommand::Zoom { off, json, .. } => (DesktopPaneAction::Zoom { enabled: !off }, json),
    };
    invoke(
        Some(&serde_json::to_string(&DesktopPaneCommand {
            target,
            action,
        })?),
        *json || cli.json,
    )
}

fn bound<'a>(fact: &'a crate::ops::context::ContextFact, name: &str) -> Result<&'a str> {
    use crate::ops::context::ContextFact;
    match fact {
        ContextFact::Bound { value, .. } => Ok(value),
        ContextFact::Unbound => {
            bail!("{name} is unbound; select Work with --repo/--task. No pane was changed.")
        }
        ContextFact::Unavailable { reason } => {
            bail!("{name} unavailable: {reason}. No pane was changed.")
        }
    }
}

impl DesktopInspection {
    fn resolve_pane(
        &self,
        work: &crate::ops::context::ContextExplanation,
        command: &crate::lf::DesktopCommand,
    ) -> Result<ResolvedPane<'_>> {
        use crate::lf::DesktopCommand;
        use crate::ops::context::ContextFact;
        let (pane, terminal, peer) = match command {
            DesktopCommand::Text { pane, .. }
            | DesktopCommand::Key { pane, .. }
            | DesktopCommand::Read { pane, .. } => (pane, true, None),
            DesktopCommand::Move {
                pane, destination, ..
            } => (pane, false, Some(destination.as_str())),
            DesktopCommand::Resize { pane, toward, .. } => (pane, false, Some(toward.as_str())),
            DesktopCommand::Hide { pane, .. }
            | DesktopCommand::Restore { pane, .. }
            | DesktopCommand::Focus { pane, .. }
            | DesktopCommand::Shell { pane, .. }
            | DesktopCommand::Files { pane, .. }
            | DesktopCommand::FlowLog { pane, .. }
            | DesktopCommand::Split { pane, .. }
            | DesktopCommand::Zoom { pane, .. } => (pane, false, None),
            DesktopCommand::Open { .. } | DesktopCommand::List { .. } => {
                bail!("This command does not address a pane")
            }
        };
        let repository = bound(&work.repository, "Repository identity")?;
        let windows = self
            .windows
            .iter()
            .filter(|window| window.repository == repository)
            .collect::<Vec<_>>();
        anyhow::ensure!(
            windows.len() == 1,
            "Expected one open window for repository {repository}; found {}. No pane was changed.",
            windows.len()
        );
        let window = windows[0];
        let checkout = match &work.task {
            ContextFact::Unbound => None,
            _ => {
                bound(&work.task, "Task")?;
                Some((
                    bound(&work.execution_machine, "Execution Machine")?,
                    bound(&work.checkout, "Task checkout")?,
                ))
            }
        };
        let mut candidates = Vec::new();
        for workspace in &window.workspaces {
            if checkout.is_some_and(|(machine, path)| {
                workspace.machine_id != machine || workspace.worktree != path
            }) {
                continue;
            }
            for leaf in workspace.layout.leaves() {
                if leaf.pane.as_deref() == peer && peer.is_some() {
                    continue;
                }
                if pane
                    .as_ref()
                    .is_some_and(|id| leaf.pane.as_ref() != Some(id))
                {
                    continue;
                }
                if terminal
                    && (!matches!(leaf.content.as_deref(), Some("shell" | "session"))
                        || leaf.surface.is_none())
                {
                    continue;
                }
                if pane.is_none()
                    && matches!(command, DesktopCommand::Restore { .. })
                    && !leaf
                        .pane
                        .as_ref()
                        .is_some_and(|id| workspace.hidden_panes.contains(id))
                {
                    continue;
                }
                candidates.push((workspace, leaf));
            }
        }
        match candidates.as_slice() {
            [(workspace, leaf)] => Ok(ResolvedPane {
                window,
                workspace,
                leaf,
            }),
            [] => bail!(
                "No eligible pane{} in the selected Work. Inspect with `lf desktop list`; no pane was changed.",
                pane.as_ref().map(|id| format!(" {id}")).unwrap_or_default()
            ),
            _ => {
                let choices = candidates
                    .iter()
                    .map(|(_, leaf)| {
                        format!(
                            "{} ({})",
                            leaf.pane.as_deref().unwrap_or("unavailable"),
                            leaf.content.as_deref().unwrap_or("empty")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                bail!("Multiple eligible panes: {choices}. Choose --pane; no pane was changed.")
            }
        }
    }
}

/// A selection borrows one inspection, including the workspace for a move/resize
/// peer. Wire targets are captured here, never re-resolved from focus at dispatch.
#[derive(Debug)]
struct ResolvedPane<'a> {
    window: &'a DesktopWindowInspection,
    workspace: &'a DesktopWorkspaceInspection,
    leaf: &'a DesktopLayoutInspection,
}

impl ResolvedPane<'_> {
    fn surface(&self) -> Result<String> {
        self.leaf
            .surface
            .clone()
            .context("Terminal surface unavailable")
    }

    fn peer(&self, selector: &str) -> Result<Self> {
        anyhow::ensure!(
            self.leaf.pane.as_deref() != Some(selector),
            "Choose two distinct panes; no pane was changed."
        );
        let panes = self
            .workspace
            .layout
            .leaves()
            .into_iter()
            .filter(|leaf| leaf.pane.as_deref() == Some(selector))
            .collect::<Vec<_>>();
        anyhow::ensure!(panes.len() == 1, "Destination pane {selector} is unavailable in the same workspace; no pane was changed.");
        Ok(Self {
            leaf: panes[0],
            ..*self
        })
    }

    fn target(&self) -> Result<DesktopPaneTarget> {
        Ok(DesktopPaneTarget {
            repository: self.window.repository.clone(),
            window: self.window.window.clone(),
            machine_id: self.workspace.machine_id.clone(),
            worktree: self.workspace.worktree.clone(),
            pane: self
                .leaf
                .pane
                .clone()
                .context("Pane identity unavailable")?,
            incarnation: self
                .leaf
                .incarnation
                .clone()
                .context("Pane content identity unavailable")?,
        })
    }
}

impl DesktopLayoutInspection {
    fn leaves(&self) -> Vec<&Self> {
        if self.pane.is_some() {
            vec![self]
        } else {
            self.children.iter().flat_map(Self::leaves).collect()
        }
    }
}

/// Decode only a bounded reply for this exact request; never follow focus or
/// fall back to an unbounded clipboard read.
fn read_text(request: DesktopTextRequest, json: bool) -> Result<()> {
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
    if json {
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
    Ok(())
}

fn validate_literal_text(text: &str) -> Result<()> {
    if text.chars().any(char::is_control) {
        bail!("Literal text cannot contain control characters; use `lf desktop key` for explicit keys. No input was sent.");
    }
    Ok(())
}

fn contact(request: Option<&str>, text_read: bool) -> Result<Vec<u8>> {
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
        for opening in &self.openings {
            lines.push(format!(
                "Repository opening {:?}: {}{}",
                opening.status,
                opening.url,
                opening
                    .reason
                    .as_ref()
                    .map(|reason| format!(" · {reason}"))
                    .unwrap_or_default()
            ));
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
            if let Some(opening) = &window.opening {
                lines.push(format!(
                    "  Opening {:?}: {} · {}",
                    opening.status,
                    opening.url,
                    opening
                        .reason
                        .as_deref()
                        .unwrap_or("native usability not yet observed")
                ));
            }
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
            Self::MissingSurface => {
                "The retained pane has no native surface; no client was acquired."
            }
            Self::NotTerminal => "The addressed pane is not a terminal.",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{opening_url, validate_literal_text};
    use crate::ops::context::{ContextExplanation, ContextFact};

    fn pane_fixture() -> (super::DesktopInspection, ContextExplanation) {
        let inspection: super::DesktopInspection = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/desktop_inspection.json"
        ))
        .unwrap();
        let window = &inspection.windows[0];
        let workspace = &window.workspaces[0];
        let fact = |value: &str| ContextFact::Bound {
            value: value.into(),
            source: "selected_work".into(),
        };
        let mut work = ContextExplanation::empty();
        work.repository = fact(&window.repository);
        work.task = fact(window.task.as_ref().unwrap().id.as_str());
        work.checkout = fact(&workspace.worktree);
        work.execution_machine = fact(&workspace.machine_id);
        (inspection, work)
    }

    #[test]
    fn pane_defaults_require_one_eligible_target_and_never_use_focus() {
        use crate::lf::DesktopCommand;
        let (mut inspection, work) = pane_fixture();
        let text = DesktopCommand::Text {
            pane: None,
            text: "draft".into(),
            json: true,
        };
        let selected = inspection.resolve_pane(&work, &text).unwrap();
        let target = selected.target().unwrap();
        let surface = selected.surface().unwrap();
        assert_eq!(target.pane, "session-pane");
        assert_eq!(surface, "native-surface-incarnation");
        let workspace = &mut inspection.windows[0].workspaces[0];
        let mut other = workspace.layout.children[0].clone();
        other.pane = Some("second-terminal".into());
        other.incarnation = Some("second-content".into());
        other.surface = Some("second-surface".into());
        workspace.layout.children.push(other);
        workspace.focused_pane = "second-terminal".into();
        let error = inspection
            .resolve_pane(&work, &text)
            .unwrap_err()
            .to_string();
        assert!(
            error.contains("session-pane")
                && error.contains("second-terminal")
                && error.contains("Choose --pane"),
            "{error}"
        );
        let explicit = DesktopCommand::Text {
            pane: Some("session-pane".into()),
            text: "draft".into(),
            json: false,
        };
        let explicit_selection = inspection.resolve_pane(&work, &explicit).unwrap();
        assert_eq!(explicit_selection.target().unwrap(), target);
        assert_eq!(explicit_selection.surface().unwrap(), surface);
        // The already resolved transport request retains the old lifetime, not
        // the replacement or the newly focused surface. Desktop validates it.
        inspection.windows[0].workspaces[0].layout.children[0].incarnation =
            Some("replacement".into());
        inspection.windows[0].workspaces[0].layout.children[0].surface =
            Some("replacement-surface".into());
        let replacement = inspection.resolve_pane(&work, &explicit).unwrap();
        assert_ne!(replacement.target().unwrap(), target);
        assert_ne!(replacement.surface().unwrap(), surface);
        assert_eq!(target.incarnation, "occurrence-session-pane");
    }

    #[test]
    fn pane_scope_refuses_other_work_and_reports_absence_before_dispatch() {
        use crate::lf::DesktopCommand;
        let (inspection, mut work) = pane_fixture();
        let focus = DesktopCommand::Focus {
            pane: None,
            json: false,
        };
        assert!(inspection
            .resolve_pane(&work, &focus)
            .unwrap_err()
            .to_string()
            .contains("Multiple eligible"));
        let missing = DesktopCommand::Focus {
            pane: Some("absent".into()),
            json: false,
        };
        assert!(inspection
            .resolve_pane(&work, &missing)
            .unwrap_err()
            .to_string()
            .contains("No eligible pane"));
        let files = DesktopCommand::Text {
            pane: Some("files-pane".into()),
            text: "no input".into(),
            json: false,
        };
        assert!(inspection.resolve_pane(&work, &files).is_err());
        let restore = DesktopCommand::Restore {
            pane: None,
            json: false,
        };
        assert_eq!(
            inspection
                .resolve_pane(&work, &restore)
                .unwrap()
                .target()
                .unwrap()
                .pane,
            "files-pane"
        );
        work.checkout = ContextFact::Bound {
            value: "/other-task".into(),
            source: "recorded".into(),
        };
        assert!(inspection.resolve_pane(&work, &restore).is_err());
        work.checkout = ContextFact::Unavailable {
            reason: "owner offline".into(),
        };
        assert!(inspection
            .resolve_pane(&work, &restore)
            .unwrap_err()
            .to_string()
            .contains("owner offline"));
    }

    #[test]
    fn pane_destinations_are_plain_selectors_in_the_same_worktree() {
        use crate::lf::DesktopCommand;
        let (mut inspection, work) = pane_fixture();
        let mut other = inspection.windows[0].workspaces[0].clone();
        other.machine_id = "other-machine".into();
        other.layout.children[1].pane = Some("other-workspace".into());
        inspection.windows[0].workspaces.push(other);
        let command = DesktopCommand::Move {
            pane: None,
            destination: "files-pane".into(),
            axis: super::DesktopSplitAxis::Vertical,
            json: false,
        };
        let selected = inspection.resolve_pane(&work, &command).unwrap();
        let target = selected.target().unwrap();
        assert_eq!(target.pane, "session-pane");
        let destination = selected.peer("files-pane").unwrap().target().unwrap();
        assert_eq!(destination.incarnation, "occurrence-files-pane");
        assert_eq!(destination.window, target.window);
        assert_eq!(destination.machine_id, target.machine_id);
        assert_eq!(destination.worktree, target.worktree);
        assert!(selected.peer("session-pane").is_err());
        assert!(selected.peer("other-workspace").is_err());
    }

    #[test]
    fn opening_keeps_exact_work_and_literal_locators_without_claiming_usability() {
        let mut resolution = ContextExplanation::empty();
        resolution.repository_path = ContextFact::Bound {
            value: "/src/a #?&% repo".into(),
            source: "local_locator".into(),
        };
        resolution.task = ContextFact::Bound {
            value: "task_exact".into(),
            source: "explicit".into(),
        };
        resolution.session = ContextFact::Bound {
            value: "session+&%".into(),
            source: "explicit".into(),
        };
        let url = opening_url(&resolution, true).unwrap();
        assert_eq!(url.path(), "/task_exact");
        assert!(!url.as_str().contains('+'));
        assert!(url.as_str().contains("a%20%23%3F%26%25%20repo"));
        assert_eq!(
            url.query_pairs().collect::<Vec<_>>(),
            vec![
                ("repo".into(), "/src/a #?&% repo".into()),
                ("session".into(), "session+&%".into()),
                ("diff".into(), "true".into())
            ]
        );
        resolution.task = ContextFact::Unbound;
        assert!(opening_url(&resolution, true).is_err());
        assert!(opening_url(&resolution, false).is_err());
        resolution.session = ContextFact::Unbound;
        assert_eq!(
            opening_url(&resolution, false).unwrap().host_str(),
            Some("open")
        );
        resolution.task = ContextFact::Unavailable {
            reason: "offline".into(),
        };
        assert!(opening_url(&resolution, false).is_err());
    }

    #[test]
    fn remote_opening_carries_the_validated_owner_and_repository() {
        let (_, mut work) = pane_fixture();
        work.repository_path = ContextFact::Bound {
            value: "/presentation/repo".into(),
            source: "local_locator".into(),
        };
        work.machine = ContextFact::Bound {
            value: "presentation-machine".into(),
            source: "local".into(),
        };
        let url = opening_url(&work, true).unwrap();
        let query = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(query["repo"], "/presentation/repo");
        assert_eq!(
            query["machine"],
            super::bound(&work.execution_machine, "Machine").unwrap()
        );
        assert_eq!(
            query["repository"],
            super::bound(&work.repository, "Repository").unwrap()
        );
        assert_eq!(query["diff"], "true");

        work.checkout = ContextFact::Unavailable {
            reason: "owner offline".into(),
        };
        assert!(opening_url(&work, true)
            .unwrap_err()
            .to_string()
            .contains("Recorded checkout unavailable"));
        work.repository = ContextFact::Unbound;
        assert!(opening_url(&work, true)
            .unwrap_err()
            .to_string()
            .contains("Shared repository identity unavailable"));
        work.execution_machine = ContextFact::Unavailable {
            reason: "conflicting owners".into(),
        };
        assert!(opening_url(&work, true)
            .unwrap_err()
            .to_string()
            .contains("conflicting owners"));
    }

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
