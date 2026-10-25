//! Shared selection and saved intent use lfdb; live pane control uses Desktop's Apple events.
use crate::work::selection::{WorkWorkspaceKey, WorkWorkspaceKind};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::ops::human_session::SessionAction;
use crate::ops::task_actions::TaskActionModel;
use crate::ops::task_run::TaskRunControl;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopInspection {
    pub observed_at: i64,
    pub windows: Vec<DesktopWindowInspection>,
    pub openings: Vec<DesktopOpening>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopWindowInspection {
    pub repository: String,
    pub window: String,
    pub path: Option<String>,
    pub destination: DesktopDestination,
    pub history: DesktopHistory,
    pub reading: String,
    pub reason: Option<String>,
    pub task: Option<DesktopTaskInspection>,
    pub session: Option<DesktopSessionInspection>,
    /// The sidebar's Tasks in ⌘1–9 order; `-t N` selects position N.
    pub list: Vec<DesktopListEntry>,
    /// Every action `lf desktop do` can name, from Desktop's registry.
    pub actions: Vec<DesktopActionDescriptor>,
    pub workspaces: Vec<DesktopWorkspaceInspection>,
    /// Work shown in this window, in slot fill order; retained workspaces outside it
    /// remain inspectable.
    pub open_work: Vec<WorkWorkspaceKey>,
    pub view: Option<crate::work::selection::ViewPlan>,
    pub opening: Option<DesktopOpening>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopListEntry {
    pub position: u32,
    pub work: WorkWorkspaceKey,
    pub identifier: Option<String>,
    pub title: String,
}

/// A registry action: its name, arguments and default keys.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopActionDescriptor {
    pub name: String,
    pub title: String,
    pub scope: DesktopActionScope,
    pub arguments: Vec<DesktopActionArgument>,
    pub keys: Vec<String>,
}

/// `window` actions need no pane; `pane` actions address one.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DesktopActionScope {
    Window,
    Pane,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopActionArgument {
    pub name: String,
    pub kind: String,
    pub required: bool,
}

/// One gesture for an exact window and, for pane actions, an exact pane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopActionRequest {
    pub repository: String,
    pub window: String,
    pub pane: Option<DesktopPaneTarget>,
    pub action: String,
    pub args: std::collections::BTreeMap<String, String>,
}

/// What the window reads after the action; the next `state` reports the same.
/// It acknowledges the view change, not terminal or provider readiness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopActionReceipt {
    pub repository: String,
    pub window: String,
    pub action: String,
    pub destination: DesktopDestination,
    pub history: DesktopHistory,
    pub pane: Option<String>,
    pub tab: Option<String>,
    pub workspace: Option<DesktopWorkspaceInspection>,
    pub observed_at: i64,
}

/// What the window shows, as the sidebar, crumbs and tile clicks select it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum DesktopDestination {
    /// Full Work; the repository conversation has a `repository` key.
    Work {
        work: WorkWorkspaceKey,
    },
    Dashboard {
        id: String,
    },
    WorkingTasks,
    /// An unsaved query; the window's `view` carries its plan.
    View {
        selection: crate::work::selection::Selection,
    },
    /// A Dashboard member entered as full Work; back returns to its Dashboard.
    Zoomed {
        from: DesktopBoard,
        work: WorkWorkspaceKey,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DesktopBoard {
    Dashboard(String),
    WorkingTasks,
}

/// Destinations back and forward reach, as the window's chevrons do.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopHistory {
    pub back: u32,
    pub forward: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopTaskInspection {
    pub id: String,
    pub reading: String,
    pub reason: Option<String>,
    pub roadmap_generated_at: Option<String>,
    pub condition_observed_at: Option<String>,
    pub actions: Option<TaskActionModel>,
    pub run_control: Option<TaskRunControl>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopSessionInspection {
    pub id: String,
    pub machine_id: Option<String>,
    pub reading: String,
    pub reason: Option<String>,
    pub observed_at: Option<String>,
    pub actions: Option<Vec<SessionAction>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopContentLocation {
    pub machine_id: String,
    pub worktree: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopWorkspaceInspection {
    pub work: WorkWorkspaceKey,
    pub title: String,
    pub is_open: bool,
    pub selected: bool,
    pub layout: DesktopLayoutInspection,
    pub focused_pane: String,
    pub zoomed_pane: Option<String>,
    pub hidden_panes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopLayoutInspection {
    pub pane: Option<String>,
    pub incarnation: Option<String>,
    pub surface: Option<String>,
    pub content: Option<String>,
    pub subject: Option<String>,
    pub axis: Option<String>,
    pub ratio: Option<f64>,
    pub children: Vec<DesktopLayoutInspection>,
    pub location: Option<DesktopContentLocation>,
    pub tabs: Vec<DesktopTabInspection>,
    pub selected_tab: Option<String>,
}

/// A shell tab carries its own occurrence and surface; a browser tab shares its pane's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopTabInspection {
    pub id: String,
    pub kind: DesktopTabKind,
    pub title: String,
    pub incarnation: String,
    pub surface: Option<String>,
    pub location: Option<DesktopContentLocation>,
    pub url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DesktopTabKind {
    Shell,
    Files,
    GraphLog,
    Url,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopPaneTarget {
    pub repository: String,
    pub window: String,
    pub work: WorkWorkspaceKey,
    pub pane: String,
    pub incarnation: String,
    pub tab: Option<String>,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum, schemars::JsonSchema,
)]
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

#[derive(
    Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DesktopSplitAxis {
    Horizontal,
    Vertical,
}

impl DesktopKey {
    fn as_str(self) -> &'static str {
        match self {
            Self::Enter => "enter",
            Self::Tab => "tab",
            Self::Escape => "escape",
            Self::Backspace => "backspace",
            Self::Delete => "delete",
            Self::Left => "left",
            Self::Right => "right",
            Self::Up => "up",
            Self::Down => "down",
            Self::Home => "home",
            Self::End => "end",
        }
    }
}

impl DesktopSplitAxis {
    fn as_str(self) -> &'static str {
        match self {
            Self::Horizontal => "horizontal",
            Self::Vertical => "vertical",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopTextRequest {
    pub target: DesktopPaneTarget,
    pub surface: String,
    pub region: DesktopTextRegion,
    pub max_bytes: usize,
}

#[derive(
    Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DesktopTextRegion {
    Screen,
    Scrollback,
    Selection,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopTextReading {
    pub request: DesktopTextRequest,
    pub observed_at: i64,
    pub hidden: bool,
    pub result: DesktopTextResult,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
#[non_exhaustive]
pub enum DesktopTextResult {
    Available { text: String, truncated: bool },
    Unavailable { reason: DesktopTextUnavailable },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DesktopTextUnavailable {
    MissingSurface,
    NotTerminal,
}

// Raw event spelling avoids loading an installed scripting dictionary during
// compilation. Checking running state precedes the tell: inspection never opens
// an app or an unrelated workspace. No caller-controlled text enters the script.
// An app that is not running raises -600, as an event to a quit app does.
const DESKTOP_EVENT: &str = r#"
on run argv
    set bundle to item 1 of argv
    set eventKind to item 2 of argv
    try
        set appRunning to application id bundle is running
    on error
        set appRunning to false
    end try
    if not appRunning then
        error "Loopflow Desktop (" & bundle & ") is not running. Open it with lf open, or inspect Work with lf task status <task>." number -600
    end if
    with timeout of 5 seconds
        if eventKind is "inspect" then
            tell application id bundle to return «event CNRTinsp»
        else if eventKind is "text" then
            tell application id bundle to return «event CNRTtext» (item 3 of argv)
        else
            tell application id bundle to return «event CNRTactn» (item 3 of argv)
        end if
    end timeout
end run
"#;

/// Desktop's script error numbers, mirrored in `DesktopInspection.swift`:
/// an unknown action or bad argument, and a stale or missing target.
const ARGUMENT_SCRIPT_ERROR: i32 = 1002;
const TARGET_SCRIPT_ERROR: i32 = 1003;

/// Failures a caller can act on by status; everything else exits 1.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum DesktopError {
    /// Unknown action, malformed `key=value` or a bad action argument. Nothing changed.
    #[error("{0}")]
    Argument(String),
    /// Missing, ambiguous or replaced Work, window or pane. Nothing changed.
    #[error("{0}")]
    Target(String),
    /// Not running, not macOS, or Automation consent pending or denied.
    #[error("{0}")]
    Unreachable(String),
}

impl DesktopError {
    fn target(error: anyhow::Error) -> anyhow::Error {
        Self::Target(format!("{error:#}")).into()
    }

    fn exit_status(&self) -> u8 {
        match self {
            Self::Argument(_) => 2,
            Self::Target(_) => 3,
            Self::Unreachable(_) => 4,
        }
    }
}

/// One Apple event and its JSON request.
#[derive(Debug, Clone, Copy)]
enum DesktopEvent<'a> {
    Inspect,
    Action(&'a str),
    Text(&'a str),
}

/// Output schemas are generated from the same DTOs that decode Desktop and opening replies.
/// Serialization context keeps nullable fields present, matching emitted JSON.
pub(crate) fn result_schema(path: &[String]) -> Option<schemars::Schema> {
    fn schema<T: schemars::JsonSchema>() -> schemars::Schema {
        schemars::generate::SchemaSettings::draft2020_12()
            .for_serialize()
            .into_generator()
            .into_root_schema_for::<T>()
    }
    match path
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        ["desktop", "state"] => Some(schema::<DesktopInspection>()),
        ["desktop", "view"] => Some(schema::<DesktopViewResult<'_>>()),
        ["desktop", "query" | "pin" | "unpin" | "arrange" | "slots"] => {
            Some(schema::<crate::work::selection::Dashboard>())
        }
        ["desktop", "order"] => Some(schema::<DesktopOrderResult>()),
        ["desktop", "layout"] => Some(schema::<Option<crate::work::selection::WorkPaneLayout>>()),
        ["open"] => Some(schema::<super::open::OpenReport>()),
        ["desktop", "ls"] => Some(schema::<DesktopWindowInspection>()),
        ["desktop", "actions"] => Some(schema::<Vec<DesktopActionDescriptor>>()),
        ["desktop", "capture"] => Some(schema::<DesktopTextReading>()),
        ["desktop", "do" | "send" | "send-key" | "run" | "zoom" | "focus" | "hide" | "restore" | "move"
        | "resize"] => Some(schema::<DesktopActionReceipt>()),
        _ => None,
    }
}

pub fn require_supported() -> Result<()> {
    if !cfg!(target_os = "macos") {
        return Err(DesktopError::Unreachable("Loopflow Desktop launching and control require macOS; use `lf task status <task>` or `lf --task <task>` in this terminal instead".into()).into());
    }

    Ok(())
}

/// LaunchServices acceptance is opening, never proof of a usable native target.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopOpening {
    pub url: String,
    pub status: DesktopOpeningStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DesktopOpeningStatus {
    Opening,
    Usable,
    Failed,
}

/// A passive plan, or the acceptance receipt for opening its native destination.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
enum DesktopViewResult<'a> {
    Plan(&'a crate::work::selection::ViewPlan),
    Opening(&'a DesktopOpening),
}

/// The repository's Task order after a move, or the Dashboard edited with --query.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
enum DesktopOrderResult {
    Tasks(Vec<String>),
    Dashboard(Box<crate::work::selection::Dashboard>),
}

fn open_url(mut url: reqwest::Url, json: bool) -> Result<()> {
    // URLComponents treats '+' literally; retain spaces across the native link.
    let query = url.query().map(|query| query.replace('+', "%20"));
    url.set_query(query.as_deref());
    let status = std::process::Command::new("open")
        .arg("-b")
        .arg(super::open::desktop_bundle())
        .arg(url.as_str())
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
        println!(
            "{}",
            serde_json::to_string_pretty(&DesktopViewResult::Opening(&opening))?
        );
    } else {
        println!("Opening {}\nInspect with `lf desktop state --json`; launch acceptance does not establish usability.", opening.url);
    }
    Ok(())
}

/// One exit status for every `lf desktop` command: 2 bad arguments, 3 missing or
/// ambiguous target, 4 Desktop unreachable, otherwise 1. Clap exits 2 and Ctrl-C 130.
fn exit_status(error: anyhow::Error) -> anyhow::Error {
    use crate::work::selection::SelectionError;
    let selection = error
        .chain()
        .find_map(|cause| cause.downcast_ref::<SelectionError>())
        .or_else(|| match error.downcast_ref::<crate::store::StoreError>() {
            Some(crate::store::StoreError::Selection(selection)) => Some(selection),
            _ => None,
        });
    let code = match selection {
        Some(SelectionError::Ambiguous(_)) => Some(3),
        Some(SelectionError::Query(_)) => Some(2),
        _ => error
            .chain()
            .find_map(|cause| cause.downcast_ref::<DesktopError>())
            .map(DesktopError::exit_status)
            .or_else(|| error.downcast_ref::<serde_json::Error>().map(|_| 2))
            .or_else(|| {
                matches!(
                    error.downcast_ref::<crate::store::StoreError>(),
                    Some(crate::store::StoreError::NotFound)
                )
                .then_some(3)
            }),
    };
    if let Some(code) = code {
        eprintln!("Error: {error:#}");
        crate::process::CommandExit(code).into()
    } else {
        error
    }
}

fn selection_repository(
    cli: &crate::lf::Cli,
    store: &crate::store::sqlite::SqliteStore,
) -> Result<crate::durable::RepositoryId> {
    if let Some(repository) = &cli.repository {
        return Ok(repository.clone());
    }
    let repo = crate::repository::CanonicalRepo::current()?
        .context("select a repository with --repo or --repository")?;
    store
        .repository_id(&repo.to_string())?
        .ok_or(crate::store::StoreError::NotFound)
        .context("repository has no recorded Work identity")
}

fn view_plan(
    cli: &crate::lf::Cli,
    expression: Option<&str>,
    passive: bool,
    json: bool,
) -> Result<()> {
    if let Some(expression) = expression {
        crate::work::selection::Selection::parse(expression, None)?;
    }
    let store = crate::store::read_existing_registry()?
        .ok_or(crate::store::StoreError::NotFound)
        .context("Work registry is unavailable")?;
    let repository = selection_repository(cli, &store.sqlite)?;
    // Explicit Work context selects the same default model without preparing a checkout.
    let plan = if expression.is_none() && (cli.task.is_some() || cli.wave.is_some()) {
        use crate::work::selection::{Selection, SelectionExpression, SelectionId};
        let resolution =
            super::context::explain(cli.wave.as_deref(), cli.task.as_deref(), None, None)?;
        let expression = if cli.task.is_some() {
            SelectionExpression::Work {
                work: WorkWorkspaceKey {
                    repository: repository.to_string(),
                    kind: WorkWorkspaceKind::Task,
                    id: bound(&resolution.task, "Task")?.into(),
                },
            }
        } else {
            SelectionExpression::Wave {
                wave: crate::id::WaveId::parse(bound(&resolution.wave, "Wave")?)?,
            }
        };
        store.sqlite.plan_selection(
            &repository,
            Selection {
                id: SelectionId("work-default".into()),
                expression,
            },
        )?
    } else {
        store.sqlite.view_plan(&repository, expression)?
    };
    if passive {
        println!(
            "{}",
            serde_json::to_string(&DesktopViewResult::Plan(&plan))?
        );
    }
    if plan.resolution.members.is_empty() && !plan.resolution.unavailable.is_empty() {
        return Err(crate::process::CommandExit(3).into());
    }
    if !passive {
        require_supported()?;
        let Some(expression) = expression else {
            // Default Work opens through its public address, like `lf open`.
            return super::open::run(
                cli,
                &super::open::OpenArgs {
                    url: None,
                    host: super::open::OpenHost::Desktop,
                    available: false,
                    session: None,
                    diff: false,
                    link: false,
                    format: None,
                    json,
                },
            );
        };
        let resolution = super::open::selected_context(cli, None)?;
        let repo = bound(&resolution.repository_path, "repository path")?;
        let mut url = reqwest::Url::parse("loopflow://open")?;
        url.query_pairs_mut()
            .append_pair("repo", repo)
            .append_pair("view", expression);
        return open_url(url, json);
    }
    Ok(())
}

fn work_key(repository: &crate::durable::RepositoryId, selector: &str) -> Result<WorkWorkspaceKey> {
    let (kind, id) = if selector == "repository" {
        (WorkWorkspaceKind::Repository, repository.as_str())
    } else if let Some(id) = selector.strip_prefix("task:") {
        (WorkWorkspaceKind::Task, id)
    } else if let Some(id) = selector.strip_prefix("wave:") {
        (WorkWorkspaceKind::Wave, id)
    } else {
        return Err(crate::work::selection::SelectionError::Query(
            "expected repository, task:<durable-id>, or wave:<durable-id>".into(),
        )
        .into());
    };
    Ok(WorkWorkspaceKey {
        repository: repository.to_string(),
        kind,
        id: id.into(),
    })
}

fn saved_intent(cli: &crate::lf::Cli, command: &crate::lf::DesktopCommand) -> Result<()> {
    use crate::lf::DesktopCommand;
    use crate::work::selection::{ArrangementEdit, DashboardId, WorkPaneLayout};
    if let DesktopCommand::Query { expression, .. } = command {
        crate::work::selection::Selection::parse(expression, None)?;
    }
    let reader = crate::store::read_existing_registry()?
        .ok_or(crate::store::StoreError::NotFound)
        .context("Work registry is unavailable")?;
    let repository = selection_repository(cli, &reader.sqlite)?;
    if let DesktopCommand::Layout {
        work, save: false, ..
    } = command
    {
        println!(
            "{}",
            serde_json::to_string(&reader.sqlite.pane_layout(&work_key(&repository, work)?)?)?
        );
        return Ok(());
    }
    drop(reader);
    let store = crate::store::sqlite::SqliteStore::new(&crate::store::database_path_from_env()?)?;
    let dashboard_id = |id: &str| DashboardId(id.strip_prefix("query:").unwrap_or(id).into());
    let result = match command {
        DesktopCommand::Query {
            container,
            expression,
            revision,
            ..
        } => serde_json::to_value(store.edit_dashboard(
            &repository,
            &dashboard_id(container),
            expression,
            *revision,
        )?)?,
        DesktopCommand::Pin {
            query, revision, ..
        }
        | DesktopCommand::Unpin {
            query, revision, ..
        } => serde_json::to_value(store.set_dashboard_metadata(
            &repository,
            &dashboard_id(query),
            *revision,
            None,
            Some(matches!(command, DesktopCommand::Pin { .. })),
        )?)?,
        DesktopCommand::Rename {
            target,
            name,
            revision,
            ..
        } => {
            if let Some(task) = target.strip_prefix("task:") {
                let key = work_key(&repository, target)?;
                // The shared reader checks repository ownership before the Task writer runs.
                let resolved = store.resolve_selection(
                    &repository,
                    &crate::work::selection::Selection {
                        id: crate::work::selection::SelectionId("rename-target".into()),
                        expression: crate::work::selection::SelectionExpression::Work { work: key },
                    },
                )?;
                if resolved.members.is_empty() {
                    return Err(crate::store::StoreError::NotFound.into());
                }
                serde_json::to_value(store.edit_task(
                    &crate::durable::TaskId::parse(task)?,
                    *revision,
                    &crate::work::task::TaskUpdate {
                        title: Some(name.clone()),
                        ..Default::default()
                    },
                )?)?
            } else {
                serde_json::to_value(store.set_dashboard_metadata(
                    &repository,
                    &dashboard_id(target),
                    i64::try_from(*revision)?,
                    Some(name),
                    None,
                )?)?
            }
        }
        DesktopCommand::Arrange {
            query, revision, ..
        } => {
            let id = dashboard_id(query);
            let reading = store
                .read_dashboard(&repository, &id)?
                .ok_or(crate::store::StoreError::NotFound)?;
            // The shared writer validates the order against current membership and decides mode.
            let edit: ArrangementEdit = serde_json::from_reader(std::io::stdin().lock())?;
            serde_json::to_value(store.save_dashboard(
                &repository,
                &id,
                Some(*revision),
                &reading.saved.selection,
                &edit,
            )?)?
        }
        DesktopCommand::Slots {
            query, revision, ..
        } => {
            let slots: Option<crate::work::selection::SlotLayout> =
                serde_json::from_reader(std::io::stdin().lock())?;
            serde_json::to_value(store.set_dashboard_slots(
                &repository,
                &dashboard_id(query),
                *revision,
                slots.as_ref(),
            )?)?
        }
        DesktopCommand::Order {
            member,
            to,
            query: Some(query),
            revision: Some(revision),
            ..
        } => serde_json::to_value(DesktopOrderResult::Dashboard(Box::new(
            store.reorder_dashboard(
                &repository,
                &dashboard_id(query),
                *revision,
                &work_key(&repository, member)?,
                &work_key(&repository, to)?,
            )?,
        )))?,
        DesktopCommand::Order { member, to, .. } => {
            let (member, to) = (work_key(&repository, member)?, work_key(&repository, to)?);
            if member.kind != WorkWorkspaceKind::Task || to.kind != WorkWorkspaceKind::Task {
                bail!("the repository order holds Tasks; pass --query to order a Dashboard");
            }
            serde_json::to_value(DesktopOrderResult::Tasks(store.reorder_task(
                &repository,
                &member.id,
                &to.id,
            )?))?
        }
        DesktopCommand::Layout {
            work, save: true, ..
        } => {
            let layout: WorkPaneLayout = serde_json::from_reader(std::io::stdin().lock())?;
            if layout.work != work_key(&repository, work)? {
                bail!("layout belongs to different Work");
            }
            serde_json::to_value(store.save_pane_layout(&layout)?)?
        }
        _ => unreachable!("saved intent command"),
    };
    println!("{}", serde_json::to_string(&result)?);
    Ok(())
}

pub fn run(cli: &crate::lf::Cli, command: &crate::lf::DesktopCommand) -> Result<()> {
    dispatch(cli, command).map_err(exit_status)
}

fn dispatch(cli: &crate::lf::Cli, command: &crate::lf::DesktopCommand) -> Result<()> {
    use crate::lf::DesktopCommand;
    match command {
        DesktopCommand::Query { .. }
        | DesktopCommand::Pin { .. }
        | DesktopCommand::Unpin { .. }
        | DesktopCommand::Rename { .. }
        | DesktopCommand::Arrange { .. }
        | DesktopCommand::Slots { .. }
        | DesktopCommand::Order { .. }
        | DesktopCommand::Layout { .. } => return saved_intent(cli, command),
        DesktopCommand::View {
            expression,
            plan,
            json,
        } => return view_plan(cli, expression.as_deref(), *plan, *json || cli.json),
        _ => {}
    }
    // Arguments are checked before Desktop is contacted.
    let verb = Verb::parse(command)?;
    require_supported()?;
    let json = cli.json || verb.json;
    let inspection = || -> Result<DesktopInspection> {
        serde_json::from_slice(&contact(DesktopEvent::Inspect)?)
            .context("Desktop returned an invalid reading; inspect again before retrying")
    };
    if let VerbKind::State = verb.kind {
        let reading = inspection()?;
        if json {
            println!("{}", serde_json::to_string_pretty(&reading)?);
        } else {
            println!("{}", reading.render());
        }
        return Ok(());
    }
    // One live reading supplies every lifetime token; dispatch never
    // reinterprets the request using later focus.
    let resolution = super::open::selected_context(cli, None)?;
    let reading = inspection()?;
    let scope = Scope {
        inspection: &reading,
        resolution: &resolution,
        explicit_work: cli.task.is_some() || cli.wave.is_some(),
        caller: std::env::var("LF_TERMINAL_ID")
            .ok()
            .filter(|id| !id.is_empty()),
    };
    let window = scope
        .window(verb.target.window.as_deref())
        .map_err(DesktopError::target)?;
    match &verb.kind {
        VerbKind::State => unreachable!("state returned above"),
        VerbKind::Ls => {
            if json {
                println!("{}", serde_json::to_string_pretty(window)?);
            } else {
                println!("{}", window.render_list());
            }
            Ok(())
        }
        VerbKind::Actions => {
            if json {
                println!("{}", serde_json::to_string_pretty(&window.actions)?);
            } else {
                for action in &window.actions {
                    println!("{}", action.render());
                }
            }
            Ok(())
        }
        VerbKind::Capture {
            region,
            max_bytes,
            trim,
        } => {
            let pane = scope
                .pane(window, &verb.target, PaneNeed::Terminal)
                .map_err(DesktopError::target)?;
            let request = DesktopTextRequest {
                target: pane.target(window),
                surface: pane.surface()?,
                region: *region,
                max_bytes: *max_bytes,
            };
            read_text(request, json, *trim)
        }
        VerbKind::Action { name, args } => {
            let descriptor = window
                .actions
                .iter()
                .find(|action| action.name == *name)
                .ok_or_else(|| {
                    DesktopError::Argument(format!(
                        "Desktop has no action {name}; see `lf desktop actions`. Nothing changed."
                    ))
                })?;
            let mut args = args.clone();
            let pane = match descriptor.scope {
                DesktopActionScope::Window => {
                    if verb.target.task.is_some() || verb.target.block.is_some() {
                        return Err(DesktopError::Argument(format!(
                            "{name} acts on the window; drop -t/-b. Nothing changed."
                        ))
                        .into());
                    }
                    None
                }
                DesktopActionScope::Pane => {
                    let pane = scope
                        .pane(window, &verb.target, PaneNeed::for_action(name))
                        .map_err(DesktopError::target)?;
                    // Pane arguments are Rex selectors in the same workspace.
                    for key in ["destination", "toward"] {
                        if let Some(selector) = args.get(key) {
                            let peer = scope.peer(pane, selector).map_err(DesktopError::target)?;
                            args.insert(key.into(), peer);
                        }
                    }
                    if matches!(name.as_str(), "pane.send" | "pane.send_key") {
                        args.insert("surface".into(), pane.surface()?);
                    }
                    Some(pane.target(window))
                }
            };
            perform(
                DesktopActionRequest {
                    repository: window.repository.clone(),
                    window: window.window.clone(),
                    pane,
                    action: name.clone(),
                    args,
                },
                json,
            )
        }
    }
}

/// A parsed `lf desktop` live command: what it reads or which action it performs.
#[derive(Debug)]
struct Verb {
    kind: VerbKind,
    target: crate::lf::DesktopTargetArgs,
    json: bool,
}

#[derive(Debug)]
enum VerbKind {
    State,
    Ls,
    Actions,
    Capture {
        region: DesktopTextRegion,
        max_bytes: usize,
        trim: bool,
    },
    Action {
        name: String,
        args: std::collections::BTreeMap<String, String>,
    },
}

/// Every action a verb is sugar for; each must be in Desktop's registry.
pub const VERB_ACTIONS: &[(&str, &str)] = &[
    ("send", "pane.send"),
    ("send-key", "pane.send_key"),
    ("run", "shell.new"),
    ("zoom", "pane.zoom"),
    ("focus", "pane.focus"),
    ("hide", "pane.hide"),
    ("restore", "pane.restore"),
    ("move", "pane.move"),
    ("resize", "pane.resize"),
];

fn verb_action(verb: &str) -> String {
    VERB_ACTIONS
        .iter()
        .find(|(name, _)| *name == verb)
        .map(|(_, action)| (*action).to_owned())
        .expect("every pane verb names a registry action")
}

impl Verb {
    fn parse(command: &crate::lf::DesktopCommand) -> Result<Self> {
        use crate::lf::DesktopCommand;
        let bare = &crate::lf::DesktopTargetArgs::default();
        let argument =
            |message: String| -> anyhow::Error { DesktopError::Argument(message).into() };
        let action = |verb: &str, args: Vec<(&str, String)>| VerbKind::Action {
            name: verb_action(verb),
            args: args
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        };
        let (kind, target, json) = match command {
            DesktopCommand::State { json } => (VerbKind::State, bare, *json),
            DesktopCommand::Ls { window, json } => {
                return Ok(Self {
                    kind: VerbKind::Ls,
                    target: crate::lf::DesktopTargetArgs {
                        window: window.clone(),
                        ..Default::default()
                    },
                    json: *json,
                })
            }
            DesktopCommand::Actions { json } => (VerbKind::Actions, bare, *json),
            DesktopCommand::Do {
                action: name,
                args,
                target,
            } => {
                let mut parsed = std::collections::BTreeMap::new();
                for pair in args {
                    let (key, value) = pair.split_once('=').ok_or_else(|| {
                        argument(format!(
                            "Expected key=value, got {pair:?}; nothing changed."
                        ))
                    })?;
                    if key.is_empty() {
                        return Err(argument(format!(
                            "Expected key=value, got {pair:?}; nothing changed."
                        )));
                    }
                    parsed.insert(key.to_owned(), value.to_owned());
                }
                if let Some(text) = parsed.get("text") {
                    validate_literal_text(text)?;
                }
                (
                    VerbKind::Action {
                        name: name.clone(),
                        args: parsed,
                    },
                    target,
                    target.json,
                )
            }
            DesktopCommand::Send { text, target } => {
                let text = match text {
                    Some(text) => text.clone(),
                    None => std::io::read_to_string(std::io::stdin().lock())
                        .context("read text to send from stdin")?,
                };
                validate_literal_text(&text)?;
                (action("send", vec![("text", text)]), target, target.json)
            }
            DesktopCommand::SendKey { key, target } => (
                action("send-key", vec![("key", key.as_str().to_owned())]),
                target,
                target.json,
            ),
            DesktopCommand::Capture {
                region,
                max_bytes,
                trim,
                target,
            } => {
                if !(1..=1_048_576).contains(max_bytes) {
                    return Err(argument(
                        "Text byte limit must be between 1 and 1048576".into(),
                    ));
                }
                (
                    VerbKind::Capture {
                        region: *region,
                        max_bytes: *max_bytes,
                        trim: *trim,
                    },
                    target,
                    target.json,
                )
            }
            DesktopCommand::Run { command, target } => {
                let args = if command.is_empty() {
                    vec![]
                } else {
                    vec![(
                        "command",
                        shlex::try_join(command.iter().map(String::as_str)).map_err(|error| {
                            argument(format!("Cannot quote the command: {error}"))
                        })?,
                    )]
                };
                (action("run", args), target, target.json)
            }
            DesktopCommand::Zoom { on, off, target } => {
                let args = match (on, off) {
                    (true, _) => vec![("on", "true".to_owned())],
                    (_, true) => vec![("on", "false".to_owned())],
                    _ => vec![],
                };
                (action("zoom", args), target, target.json)
            }
            DesktopCommand::Focus { target } => (action("focus", vec![]), target, target.json),
            DesktopCommand::Hide { target } => (action("hide", vec![]), target, target.json),
            DesktopCommand::Restore { target } => (action("restore", vec![]), target, target.json),
            DesktopCommand::Move {
                destination,
                axis,
                target,
            } => (
                action(
                    "move",
                    vec![
                        ("destination", destination.clone()),
                        ("axis", axis.as_str().to_owned()),
                    ],
                ),
                target,
                target.json,
            ),
            DesktopCommand::Resize {
                toward,
                ratio,
                target,
            } => {
                if !(ratio.is_finite() && (0.1..=0.9).contains(ratio)) {
                    return Err(argument("Split ratio must be between 0.1 and 0.9".into()));
                }
                (
                    action(
                        "resize",
                        vec![("toward", toward.clone()), ("ratio", ratio.to_string())],
                    ),
                    target,
                    target.json,
                )
            }
            DesktopCommand::View { .. }
            | DesktopCommand::Query { .. }
            | DesktopCommand::Pin { .. }
            | DesktopCommand::Unpin { .. }
            | DesktopCommand::Rename { .. }
            | DesktopCommand::Arrange { .. }
            | DesktopCommand::Slots { .. }
            | DesktopCommand::Order { .. }
            | DesktopCommand::Layout { .. } => unreachable!("saved intent is not a live verb"),
        };
        Ok(Self {
            kind,
            target: target.clone(),
            json,
        })
    }
}

fn perform(request: DesktopActionRequest, json: bool) -> Result<()> {
    let output = contact(DesktopEvent::Action(&serde_json::to_string(&request)?))?;
    let receipt: DesktopActionReceipt = serde_json::from_slice(&output)
        .context("Desktop returned an invalid receipt; inspect before retrying")?;
    if json {
        println!("{}", serde_json::to_string_pretty(&receipt)?);
    } else if let Some(pane) = &receipt.pane {
        println!("{}", receipt.tab.as_deref().unwrap_or(pane));
    } else {
        println!("{}", receipt.destination);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Targets: Rex order — full ID, case-insensitive label, 1-based position, ID
// suffix (≥4 characters). Absent or ambiguous changes nothing and exits 3.
// Nothing ever follows focus.
// ---------------------------------------------------------------------------

/// The first selector kind that matches decides; more than one match is ambiguous.
fn resolve<T>(
    noun: &str,
    selector: &str,
    candidates: &[T],
    ids: impl Fn(&T) -> Vec<String>,
    labels: impl Fn(&T) -> Vec<String>,
    describe: impl Fn(&T) -> String,
) -> Result<usize> {
    let choose = |matches: Vec<usize>| -> Result<Option<usize>> {
        match matches.as_slice() {
            [] => Ok(None),
            [index] => Ok(Some(*index)),
            _ => bail!(
                "{noun} {selector} is ambiguous: {}. Nothing changed.",
                matches
                    .iter()
                    .map(|index| describe(&candidates[*index]))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    };
    let matching = |test: &dyn Fn(&T) -> bool| {
        (0..candidates.len())
            .filter(|index| test(&candidates[*index]))
            .collect::<Vec<_>>()
    };
    if let Some(index) = choose(matching(&|candidate| {
        ids(candidate).iter().any(|id| id == selector)
    }))? {
        return Ok(index);
    }
    if let Some(index) = choose(matching(&|candidate| {
        labels(candidate)
            .iter()
            .any(|label| label.eq_ignore_ascii_case(selector))
    }))? {
        return Ok(index);
    }
    // A number past the end is not a position; it may still be an ID suffix.
    let position = selector.parse::<usize>().ok();
    if let Some(index) = position
        .and_then(|position| position.checked_sub(1))
        .filter(|index| *index < candidates.len())
    {
        return Ok(index);
    }
    if selector.chars().count() >= 4 {
        if let Some(index) = choose(matching(&|candidate| {
            ids(candidate).iter().any(|id| id.ends_with(selector))
        }))? {
            return Ok(index);
        }
    }
    if let Some(position) = position {
        bail!(
            "No {noun} at position {position}; there are {}. Nothing changed.",
            candidates.len()
        );
    }
    bail!(
        "No {noun} {selector}; choose one of: {}. Nothing changed.",
        candidates
            .iter()
            .map(&describe)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

/// What a pane command can address when no `-b` names a pane.
#[derive(Debug, Clone, Copy, PartialEq)]
enum PaneNeed {
    Any,
    Kind(&'static str),
    Terminal,
    Hidden,
}

impl PaneNeed {
    fn for_action(action: &str) -> Self {
        match action.split_once('.').map_or(action, |(group, _)| group) {
            "shell" => Self::Kind("shell"),
            "browser" => Self::Kind("browser"),
            "session" => Self::Kind("session"),
            _ if matches!(action, "pane.send" | "pane.send_key") => Self::Terminal,
            _ if action == "pane.restore" => Self::Hidden,
            _ => Self::Any,
        }
    }
}

/// One addressed pane, and for shells and browsers optionally one tab.
#[derive(Debug, Clone, Copy)]
struct ResolvedPane<'a> {
    workspace: &'a DesktopWorkspaceInspection,
    leaf: &'a DesktopLayoutInspection,
    tab: Option<&'a DesktopTabInspection>,
}

impl ResolvedPane<'_> {
    fn surface(&self) -> Result<String> {
        if self.leaf.content.as_deref() == Some("browser") {
            bail!("The addressed pane is not a terminal; nothing was sent or read.");
        }
        self.tab
            .map_or(&self.leaf.surface, |tab| &tab.surface)
            .clone()
            .ok_or_else(|| {
                DesktopError::Target(
                    "The addressed terminal has no native surface (a Task without a conversation shows its start state). Nothing changed.".into(),
                )
                .into()
            })
    }

    fn target(&self, window: &DesktopWindowInspection) -> DesktopPaneTarget {
        DesktopPaneTarget {
            repository: window.repository.clone(),
            window: window.window.clone(),
            work: self.workspace.work.clone(),
            pane: self.leaf.pane.clone().unwrap_or_default(),
            incarnation: self
                .tab
                .map(|tab| tab.incarnation.clone())
                .or_else(|| self.leaf.incarnation.clone())
                .unwrap_or_default(),
            tab: self.tab.map(|tab| tab.id.clone()),
        }
    }
}

/// Everything a target resolves against: one reading, the selected Work and the caller.
struct Scope<'a> {
    inspection: &'a DesktopInspection,
    resolution: &'a crate::ops::context::ContextExplanation,
    /// Root `--task`/`--wave` were given; they outrank the caller's own pane.
    explicit_work: bool,
    /// `LF_TERMINAL_ID`, trusted for targeting: the terminal this command came from.
    caller: Option<String>,
}

impl<'a> Scope<'a> {
    /// The caller's window and pane, when its terminal is still retained.
    fn caller(&self) -> Option<(&'a DesktopWindowInspection, ResolvedPane<'a>)> {
        let id = self.caller.as_deref()?;
        self.inspection.windows.iter().find_map(|window| {
            window.workspaces.iter().find_map(|workspace| {
                workspace.layout.leaves().into_iter().find_map(|leaf| {
                    if leaf.content.as_deref() == Some("session")
                        && leaf.surface.as_deref() == Some(id)
                    {
                        return Some((
                            window,
                            ResolvedPane {
                                workspace,
                                leaf,
                                tab: None,
                            },
                        ));
                    }
                    let tab = leaf
                        .tabs
                        .iter()
                        .find(|tab| tab.kind == DesktopTabKind::Shell && tab.id == id)?;
                    Some((
                        window,
                        ResolvedPane {
                            workspace,
                            leaf,
                            tab: Some(tab),
                        },
                    ))
                })
            })
        })
    }

    fn window(&self, selector: Option<&str>) -> Result<&'a DesktopWindowInspection> {
        let repository = match &self.resolution.repository {
            crate::ops::context::ContextFact::Bound { value, .. } => Some(value.as_str()),
            _ => None,
        };
        let caller = self.caller();
        let windows = self
            .inspection
            .windows
            .iter()
            .filter(|window| {
                repository.is_none_or(|repository| window.repository == repository)
                    || caller.is_some_and(|(caller, _)| caller.window == window.window)
            })
            .collect::<Vec<_>>();
        if let Some(selector) = selector {
            let index = resolve(
                "window",
                selector,
                &windows,
                |window| vec![window.window.clone()],
                |window| {
                    window
                        .path
                        .iter()
                        .map(|path| path.rsplit('/').next().unwrap_or(path).to_owned())
                        .collect()
                },
                |window| short(&window.window).to_owned(),
            )?;
            return Ok(windows[index]);
        }
        if let Some((window, _)) = caller {
            return Ok(window);
        }
        match windows.as_slice() {
            [window] => Ok(window),
            [] => bail!(
                "No Desktop window shows {}. Open one with `lf open`; nothing changed.",
                repository.unwrap_or("this repository")
            ),
            _ => bail!(
                "{} Desktop windows show this repository: {}. Choose one with --window; nothing changed.",
                windows.len(),
                windows.iter().map(|window| short(&window.window)).collect::<Vec<_>>().join(", ")
            ),
        }
    }

    /// `-t`, then root Work flags, then the caller's workspace, then the checkout's binding.
    fn workspace(
        &self,
        window: &'a DesktopWindowInspection,
        task: Option<&str>,
    ) -> Result<&'a DesktopWorkspaceInspection> {
        use crate::ops::context::ContextFact;
        let find = |work: &WorkWorkspaceKey| {
            window.workspaces.iter().find(|workspace| workspace.work == *work).ok_or_else(|| {
                anyhow::anyhow!(
                    "{} has no workspace in this Desktop window yet; open it first. Nothing changed.",
                    work.id
                )
            })
        };
        if let Some(selector) = task {
            let index = resolve(
                "Task",
                selector,
                &window.list,
                |entry| vec![entry.work.id.clone()],
                |entry| {
                    entry
                        .identifier
                        .iter()
                        .cloned()
                        .chain([entry.title.clone()])
                        .collect()
                },
                |entry| {
                    format!(
                        "{} {}",
                        entry.position,
                        entry.identifier.as_deref().unwrap_or(&entry.title)
                    )
                },
            )?;
            return find(&window.list[index].work);
        }
        let selected = || match (&self.resolution.task, &self.resolution.wave) {
            (ContextFact::Bound { value, .. }, _) => {
                Some((WorkWorkspaceKind::Task, value.as_str()))
            }
            (_, ContextFact::Bound { value, .. }) => {
                Some((WorkWorkspaceKind::Wave, value.as_str()))
            }
            _ => None,
        };
        let by_selection = |(kind, id): (WorkWorkspaceKind, &str)| {
            window
                .workspaces
                .iter()
                .find(|workspace| workspace.work.kind == kind && workspace.work.id == id)
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "{id} has no workspace in this Desktop window; nothing changed."
                    )
                })
        };
        if self.explicit_work {
            if let Some(selection) = selected() {
                return by_selection(selection);
            }
        }
        if let Some(id) = &self.caller {
            return match self.caller() {
                Some((caller, pane)) if caller.window == window.window => Ok(pane.workspace),
                Some(_) => bail!("This terminal belongs to another Desktop window; pass -t. Nothing changed."),
                None => bail!("This terminal ({id}) is no longer in Desktop; pass -t and -b. Nothing changed."),
            };
        }
        match selected() {
            Some(selection) => by_selection(selection),
            None => bail!("No Task selected: pass -t, or run from a Task's checkout or Desktop terminal. Nothing changed."),
        }
    }

    fn pane(
        &self,
        window: &'a DesktopWindowInspection,
        target: &crate::lf::DesktopTargetArgs,
        need: PaneNeed,
    ) -> Result<ResolvedPane<'a>> {
        let workspace = self.workspace(window, target.task.as_deref())?;
        let leaves = workspace.layout.leaves();
        let Some(selector) = target.block.as_deref() else {
            // Without -b, a command from one of this workspace's terminals acts on that terminal.
            if let Some((_, caller)) = self.caller() {
                let own = caller.workspace.work == workspace.work
                    && match need {
                        PaneNeed::Any | PaneNeed::Terminal => true,
                        PaneNeed::Kind(kind) => caller.leaf.content.as_deref() == Some(kind),
                        PaneNeed::Hidden => false,
                    };
                if own {
                    return Ok(caller);
                }
            }
            let candidates = leaves
                .iter()
                .flat_map(|leaf| match need {
                    PaneNeed::Any => vec![ResolvedPane {
                        workspace,
                        leaf,
                        tab: None,
                    }],
                    PaneNeed::Kind(kind) => (leaf.content.as_deref() == Some(kind))
                        .then_some(ResolvedPane {
                            workspace,
                            leaf,
                            tab: None,
                        })
                        .into_iter()
                        .collect(),
                    PaneNeed::Hidden => leaf
                        .pane
                        .as_ref()
                        .filter(|pane| workspace.hidden_panes.contains(pane))
                        .map(|_| ResolvedPane {
                            workspace,
                            leaf,
                            tab: None,
                        })
                        .into_iter()
                        .collect(),
                    // Retained shells, including background tabs, without consulting selection.
                    PaneNeed::Terminal => match leaf.content.as_deref() {
                        Some("session") => vec![ResolvedPane {
                            workspace,
                            leaf,
                            tab: None,
                        }],
                        Some("shell") => leaf
                            .tabs
                            .iter()
                            .map(|tab| ResolvedPane {
                                workspace,
                                leaf,
                                tab: Some(tab),
                            })
                            .collect(),
                        _ => vec![],
                    },
                })
                .collect::<Vec<_>>();
            return match candidates.as_slice() {
                [pane] => Ok(*pane),
                [] => bail!("No eligible pane in {}; nothing changed.", workspace.title),
                _ => bail!(
                    "Several panes are eligible: {}. Choose one with -b; nothing changed.",
                    candidates
                        .iter()
                        .map(ResolvedPane::describe)
                        .collect::<Vec<_>>()
                        .join(", ")
                ),
            };
        };
        // `shell:2`, `browser:1` name a tab of a designated pane by position.
        if let Some((kind, position)) = selector.split_once(':') {
            if let (Some(leaf), Ok(position)) = (
                leaves.iter().find(|leaf| {
                    leaf.content
                        .as_deref()
                        .is_some_and(|content| content.eq_ignore_ascii_case(kind))
                }),
                position.parse::<usize>(),
            ) {
                let tab = position
                    .checked_sub(1)
                    .and_then(|index| leaf.tabs.get(index))
                    .ok_or_else(|| {
                        anyhow::anyhow!(
                            "{kind} has no tab {position}; it has {}. Nothing changed.",
                            leaf.tabs.len()
                        )
                    })?;
                return Ok(ResolvedPane {
                    workspace,
                    leaf,
                    tab: Some(tab),
                });
            }
        }
        let candidates = leaves
            .iter()
            .flat_map(|leaf| {
                std::iter::once(ResolvedPane {
                    workspace,
                    leaf,
                    tab: None,
                })
                .chain(leaf.tabs.iter().map(move |tab| ResolvedPane {
                    workspace,
                    leaf,
                    tab: Some(tab),
                }))
            })
            .collect::<Vec<_>>();
        // Positions count panes in layout order; tabs are reached with `kind:N`.
        let panes = candidates.iter().filter(|pane| pane.tab.is_none()).count();
        if let Ok(position) = selector.parse::<usize>() {
            return position
                .checked_sub(1)
                .filter(|index| *index < panes)
                .map(|index| {
                    candidates
                        .iter()
                        .filter(|pane| pane.tab.is_none())
                        .nth(index)
                        .copied()
                        .expect("index within panes")
                })
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "No pane at position {position}; there are {panes}. Nothing changed."
                    )
                });
        }
        let index = resolve(
            "pane",
            selector,
            &candidates,
            |pane| {
                vec![pane.tab.map_or_else(
                    || pane.leaf.pane.clone().unwrap_or_default(),
                    |tab| tab.id.clone(),
                )]
            },
            |pane| match pane.tab {
                None => pane.leaf.content.iter().cloned().collect(),
                Some(_) => vec![],
            },
            ResolvedPane::describe,
        )?;
        Ok(candidates[index])
    }

    /// A move/resize peer in the same workspace, as Desktop's pane ID.
    fn peer(&self, pane: ResolvedPane<'a>, selector: &str) -> Result<String> {
        let leaves = pane.workspace.layout.leaves();
        let index = resolve(
            "pane",
            selector,
            &leaves,
            |leaf| leaf.pane.iter().cloned().collect(),
            |leaf| leaf.content.iter().cloned().collect(),
            |leaf| leaf.pane.clone().unwrap_or_default(),
        )?;
        let peer = leaves[index].pane.clone().unwrap_or_default();
        anyhow::ensure!(
            Some(&peer) != pane.leaf.pane.as_ref(),
            "Choose two distinct panes; nothing changed."
        );
        Ok(peer)
    }
}

impl ResolvedPane<'_> {
    fn describe(&self) -> String {
        let pane = self.leaf.pane.as_deref().unwrap_or("unavailable");
        match self.tab {
            Some(tab) => format!("{pane}/{} ({})", tab.id, tab.title),
            None => format!(
                "{pane} ({})",
                self.leaf.content.as_deref().unwrap_or("unknown")
            ),
        }
    }
}

fn bound<'a>(fact: &'a crate::ops::context::ContextFact, name: &str) -> Result<&'a str> {
    use crate::ops::context::ContextFact;
    match fact {
        ContextFact::Bound { value, .. } => Ok(value),
        ContextFact::Unbound => bail!("{name} is unbound; select Work with --repo/--task."),
        ContextFact::Unavailable { reason } => bail!("{name} unavailable: {reason}."),
    }
}

fn short(id: &str) -> &str {
    &id[..id.len().min(8)]
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
fn read_text(request: DesktopTextRequest, json: bool, trim: bool) -> Result<()> {
    let encoded = serde_json::to_string(&request)?;
    let output = contact(DesktopEvent::Text(&encoded))?;
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
                if trim {
                    println!("{}", trim_capture(text));
                } else {
                    print!("{text}");
                }
                if *truncated {
                    eprintln!("\n[Desktop text truncated]");
                }
            }
            DesktopTextResult::Unavailable { reason } => bail!("{}", reason.message()),
        }
    }
    Ok(())
}

/// Trailing spaces on each line and trailing blank lines are screen padding.
fn trim_capture(text: &str) -> String {
    let lines = text.lines().map(str::trim_end).collect::<Vec<_>>();
    let end = lines
        .iter()
        .rposition(|line| !line.is_empty())
        .map_or(0, |index| index + 1);
    lines[..end].join("\n")
}

fn validate_literal_text(text: &str) -> Result<()> {
    if text.chars().any(char::is_control) {
        return Err(DesktopError::Argument("Literal text cannot contain control characters; use `lf desktop send-key` for explicit keys. No input was sent.".into()).into());
    }
    Ok(())
}

fn contact(event: DesktopEvent) -> Result<Vec<u8>> {
    let (kind, request) = match event {
        DesktopEvent::Inspect => ("inspect", None),
        DesktopEvent::Action(request) => ("action", Some(request)),
        DesktopEvent::Text(request) => ("text", Some(request)),
    };
    let output = std::process::Command::new("/usr/bin/osascript")
        .args(["-e", DESKTOP_EVENT, "--"])
        .arg(super::open::desktop_bundle())
        .arg(kind)
        .args(request)
        .output()
        .context("contact Loopflow Desktop via macOS automation")?;
    if !output.status.success() {
        return Err(contact_failure(&String::from_utf8_lossy(&output.stderr)));
    }
    Ok(output.stdout)
}

/// osascript reports `NN:NN: execution error: <message> (<number>)`; the number,
/// never the message, decides the status.
fn contact_failure(stderr: &str) -> anyhow::Error {
    let stderr = stderr.trim();
    let (message, number) = stderr
        .strip_suffix(')')
        .and_then(|rest| rest.rsplit_once(" ("))
        .and_then(|(message, number)| Some((message, number.parse::<i32>().ok()?)))
        .unwrap_or((stderr, 0));
    let message = message
        .split_once("execution error: ")
        .map_or(message, |(_, message)| message);
    let consent =
        "Allow Automation for Loopflow in System Settings › Privacy & Security › Automation";
    match number {
        ARGUMENT_SCRIPT_ERROR => DesktopError::Argument(message.into()).into(),
        TARGET_SCRIPT_ERROR => DesktopError::Target(message.into()).into(),
        // Event not understood: this Desktop predates the lf that asked.
        -1708 => anyhow::anyhow!("Loopflow Desktop does not understand this request; update Desktop to match this lf ({message})."),
        // Not running, or quit mid-request.
        -600 | -609 => DesktopError::Unreachable(message.into()).into(),
        -1712 => DesktopError::Unreachable(format!(
            "Desktop did not answer within 5 seconds; an Automation consent prompt may be waiting. {consent}, then inspect again before retrying."
        ))
        .into(),
        -1743 | -1744 => DesktopError::Unreachable(format!(
            "This terminal may not control Loopflow Desktop. {consent}."
        ))
        .into(),
        // A lost reply does not prove the command had no effect. In particular,
        // repeating shell creation allocates another tab; inspect before retrying.
        _ => anyhow::anyhow!("Desktop request failed: {stderr}. Inspect again before retrying; `lf task status <task>` remains available in this terminal."),
    }
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
                "  Showing: {} · back {} · forward {} · Session: {}",
                window.destination,
                window.history.back,
                window.history.forward,
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
                "  Actions: {} (lf desktop actions)",
                window.actions.len()
            ));
            for workspace in &window.workspaces {
                lines.push(format!(
                    "  {:?}:{} · focused {} · zoom {} · hidden [{}]",
                    workspace.work.kind,
                    workspace.title,
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

impl DesktopWindowInspection {
    /// `lf desktop ls`: the window, its sidebar Tasks by position, and each retained Task's panes.
    fn render_list(&self) -> String {
        let mut lines = vec![format!(
            "Window {} · {} · showing {}",
            short(&self.window),
            self.path.as_deref().unwrap_or(&self.repository),
            self.destination
        )];
        if self.list.is_empty() {
            lines.push("  No Working Tasks.".into());
        }
        for entry in &self.list {
            let panes = self
                .workspaces
                .iter()
                .find(|workspace| workspace.work == entry.work)
                .map(|workspace| {
                    workspace
                        .layout
                        .leaves()
                        .iter()
                        .map(|leaf| {
                            let pane = leaf.pane.as_deref().unwrap_or_default();
                            let mut name = leaf.content.clone().unwrap_or_else(|| pane.to_owned());
                            if leaf.tabs.len() > 1 {
                                name.push_str(&format!("[{}]", leaf.tabs.len()));
                            }
                            if workspace.hidden_panes.iter().any(|hidden| hidden == pane) {
                                name = format!("({name})");
                            } else if workspace.zoomed_pane.as_deref() == Some(pane) {
                                name.push_str(" zoomed");
                            } else if workspace.focused_pane == pane {
                                name.push('*');
                            }
                            name
                        })
                        .collect::<Vec<_>>()
                        .join(" ")
                })
                .unwrap_or_else(|| "not open".into());
            lines.push(format!(
                "  {}  {:<9} {:.48}  {}",
                entry.position,
                entry.identifier.as_deref().unwrap_or(short(&entry.work.id)),
                entry.title,
                panes
            ));
        }
        lines.join("\n")
    }
}

impl DesktopActionDescriptor {
    fn render(&self) -> String {
        let arguments = self
            .arguments
            .iter()
            .map(|argument| {
                if argument.required {
                    format!("{}=<{}>", argument.name, argument.kind)
                } else {
                    format!("[{}=<{}>]", argument.name, argument.kind)
                }
            })
            .collect::<Vec<_>>()
            .join(" ");
        let keys = if self.keys.is_empty() {
            String::new()
        } else if self.keys.len() > 2 {
            format!("  {}…{}", self.keys[0], self.keys[self.keys.len() - 1])
        } else {
            format!("  {}", self.keys.join(" / "))
        };
        format!("{:<24} {:<34} {}{keys}", self.name, arguments, self.title)
    }
}

impl std::fmt::Display for DesktopDestination {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let selector = |work: &WorkWorkspaceKey| match work.kind {
            WorkWorkspaceKind::Repository => "repository".to_owned(),
            WorkWorkspaceKind::Task => format!("task:{}", work.id),
            WorkWorkspaceKind::Wave => format!("wave:{}", work.id),
        };
        match self {
            Self::Work { work } => write!(f, "{}", selector(work)),
            Self::Dashboard { id } => write!(f, "query:{id}"),
            Self::WorkingTasks => write!(f, "working-tasks"),
            Self::View { .. } => write!(f, "unsaved view"),
            Self::Zoomed { from, work } => write!(f, "{} entered from {from}", selector(work)),
        }
    }
}

impl std::fmt::Display for DesktopBoard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dashboard(id) => write!(f, "query:{id}"),
            Self::WorkingTasks => write!(f, "working-tasks"),
        }
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
            for (index, tab) in self.tabs.iter().enumerate() {
                lines.push(format!(
                    "{:indent$}  {}:{} {}{} · {}",
                    "",
                    self.content.as_deref().unwrap_or("tab"),
                    index + 1,
                    tab.id,
                    if self.selected_tab.as_ref() == Some(&tab.id) {
                        " (selected)"
                    } else {
                        ""
                    },
                    tab.url.as_deref().unwrap_or(&tab.title)
                ));
            }
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
    use super::{
        trim_capture, validate_literal_text, DesktopError, DesktopInspection, PaneNeed, Scope,
    };
    use crate::lf::DesktopTargetArgs;
    use crate::ops::context::{ContextExplanation, ContextFact};

    const ACTIONS: &str = include_str!("../../../../../tests/fixtures/dto/desktop_action.json");

    /// One window listing two Tasks; the second has the designated
    /// session/shell/browser workspace from the action fixture.
    fn fixture() -> (DesktopInspection, ContextExplanation) {
        let mut inspection: DesktopInspection = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/desktop_inspection.json"
        ))
        .unwrap();
        let action: serde_json::Value = serde_json::from_str(ACTIONS).unwrap();
        let workspace: super::DesktopWorkspaceInspection =
            serde_json::from_value(action["receipt"]["workspace"].clone()).unwrap();
        let window = &mut inspection.windows[0];
        let mut other = window.list[0].clone();
        other.position = 1;
        other.identifier = Some("LOO-576".into());
        other.title = "Sidebar scope".into();
        other.work.id = "task_99999999999999999999999999990576".into();
        window.list[0].position = 2;
        window.list.insert(0, other);
        window.workspaces = vec![workspace];
        let mut work = ContextExplanation::empty();
        work.repository = ContextFact::Bound {
            value: window.repository.clone(),
            source: "selected_work".into(),
        };
        (inspection, work)
    }

    fn scope<'a>(
        inspection: &'a DesktopInspection,
        work: &'a ContextExplanation,
        caller: Option<&str>,
    ) -> Scope<'a> {
        Scope {
            inspection,
            resolution: work,
            explicit_work: false,
            caller: caller.map(str::to_owned),
        }
    }

    fn target(task: Option<&str>, block: Option<&str>) -> DesktopTargetArgs {
        DesktopTargetArgs {
            task: task.map(str::to_owned),
            block: block.map(str::to_owned),
            ..Default::default()
        }
    }

    fn pane(
        scope: &Scope,
        task: Option<&str>,
        block: Option<&str>,
        need: PaneNeed,
    ) -> anyhow::Result<String> {
        let window = scope.window(None)?;
        let pane = scope.pane(window, &target(task, block), need)?;
        Ok(pane.describe())
    }

    #[test]
    fn tasks_resolve_by_id_label_position_then_suffix() {
        let (inspection, work) = fixture();
        let scope = scope(&inspection, &work, None);
        let window = scope.window(None).unwrap();
        let task = |selector: &str| {
            scope
                .workspace(window, Some(selector))
                .map(|w| w.work.id.clone())
        };
        let fixture_task = "task_44444444444444444444444444444444";
        assert_eq!(task("loo-519").unwrap(), fixture_task);
        assert_eq!(task("2").unwrap(), fixture_task);
        assert_eq!(task(fixture_task).unwrap(), fixture_task);
        assert_eq!(task("4444").unwrap(), fixture_task);
        // Task 1 is listed but has no workspace open; nothing is opened for it.
        assert!(task("1").unwrap_err().to_string().contains("no workspace"));
        assert!(task("99").unwrap_err().to_string().contains("position 99"));
        let (mut inspection, work) = fixture();
        inspection.windows[0].list[0].title = "Fixture Task".into();
        let scope = super::tests::scope(&inspection, &work, None);
        let window = scope.window(None).unwrap();
        let ambiguous = scope
            .workspace(window, Some("fixture task"))
            .unwrap_err()
            .to_string();
        assert!(
            ambiguous.contains("ambiguous") && ambiguous.contains("LOO-576"),
            "{ambiguous}"
        );
    }

    #[test]
    fn panes_resolve_by_kind_tab_position_id_and_suffix() {
        let (inspection, mut work) = fixture();
        work.task = ContextFact::Bound {
            value: "task_44444444444444444444444444444444".into(),
            source: "checkout".into(),
        };
        let scope = scope(&inspection, &work, None);
        let any = PaneNeed::Any;
        assert_eq!(
            pane(&scope, None, Some("SHELL"), any).unwrap(),
            "shell (shell)"
        );
        assert_eq!(
            pane(&scope, None, Some("browser:2"), any).unwrap(),
            "browser/page-1 (localhost)"
        );
        assert_eq!(
            pane(&scope, None, Some("3"), any).unwrap(),
            "browser (browser)"
        );
        assert_eq!(
            pane(&scope, None, Some("shell-tab"), any).unwrap(),
            "shell/shell-tab (Shell)"
        );
        assert!(pane(&scope, Some("2"), Some("e-1"), any)
            .unwrap_err()
            .to_string()
            .contains("No pane e-1"));
        assert_eq!(
            pane(&scope, None, Some("ge-1"), any).unwrap(),
            "browser/page-1 (localhost)"
        );
        assert!(pane(&scope, None, Some("shell:3"), any)
            .unwrap_err()
            .to_string()
            .contains("no tab 3"));
        // Without -b or a caller nothing follows focus: several panes are ambiguous.
        let error = pane(&scope, None, None, any).unwrap_err().to_string();
        assert!(
            error.contains("session") && error.contains("browser") && error.contains("-b"),
            "{error}"
        );
        assert_eq!(
            pane(&scope, None, None, PaneNeed::Kind("shell")).unwrap(),
            "shell (shell)"
        );
        assert!(pane(&scope, None, None, PaneNeed::Terminal).is_err());
    }

    #[test]
    fn a_bare_command_acts_on_its_own_terminal_and_a_gone_one_changes_nothing() {
        let (inspection, work) = fixture();
        let session = scope(&inspection, &work, Some("native-surface-incarnation"));
        assert_eq!(
            pane(&session, None, None, PaneNeed::Any).unwrap(),
            "session (session)"
        );
        // `run` from the session pane adds to that Task's shell.
        assert_eq!(
            pane(&session, None, None, PaneNeed::Kind("shell")).unwrap(),
            "shell (shell)"
        );
        let shell = scope(&inspection, &work, Some("shell-tab"));
        assert_eq!(
            pane(&shell, None, None, PaneNeed::Terminal).unwrap(),
            "shell/shell-tab (Shell)"
        );
        // Explicit selectors win over the caller.
        assert_eq!(
            pane(&shell, None, Some("browser"), PaneNeed::Any).unwrap(),
            "browser (browser)"
        );
        let gone = scope(&inspection, &work, Some("closed-terminal"));
        assert!(pane(&gone, None, None, PaneNeed::Any)
            .unwrap_err()
            .to_string()
            .contains("no longer in Desktop"));
        assert_eq!(
            pane(&gone, Some("LOO-519"), Some("session"), PaneNeed::Any).unwrap(),
            "session (session)"
        );
    }

    #[test]
    fn browser_tabs_are_not_terminals_and_peers_stay_in_the_workspace() {
        let (inspection, work) = fixture();
        let scope = scope(&inspection, &work, Some("shell-tab"));
        let window = scope.window(None).unwrap();
        let browser = scope
            .pane(window, &target(None, Some("browser:1")), PaneNeed::Terminal)
            .unwrap();
        assert!(browser
            .surface()
            .unwrap_err()
            .to_string()
            .contains("not a terminal"));
        let shell = scope
            .pane(window, &target(None, None), PaneNeed::Any)
            .unwrap();
        assert_eq!(scope.peer(shell, "session").unwrap(), "session");
        assert!(scope.peer(shell, "shell").is_err());
        assert!(scope.peer(shell, "elsewhere").is_err());
    }

    #[test]
    fn windows_resolve_by_id_or_suffix_and_several_need_a_choice() {
        let (mut inspection, work) = fixture();
        let mut second = inspection.windows[0].clone();
        second.window = "66666666-6666-6666-6666-666666666666".into();
        inspection.windows.push(second);
        let bare = scope(&inspection, &work, None);
        assert!(bare
            .window(None)
            .unwrap_err()
            .to_string()
            .contains("--window"));
        assert_eq!(
            bare.window(Some("6666")).unwrap().window,
            inspection.windows[2].window
        );
        assert_eq!(
            bare.window(Some("1")).unwrap().window,
            inspection.windows[0].window
        );
        // A caller's own window needs no choice.
        let caller = scope(&inspection, &work, Some("shell-tab"));
        assert_eq!(
            caller.window(None).unwrap().window,
            inspection.windows[0].window
        );
    }

    #[test]
    fn every_verb_names_a_registry_action() {
        let fixture: serde_json::Value = serde_json::from_str(ACTIONS).unwrap();
        let names = fixture["actions"]
            .as_array()
            .unwrap()
            .iter()
            .map(|action| action["name"].as_str().unwrap())
            .collect::<Vec<_>>();
        for (verb, action) in super::VERB_ACTIONS {
            assert!(names.contains(action), "{verb} → {action}");
        }
    }

    #[test]
    fn desktop_failures_take_their_status_from_the_script_error_number() {
        let status = |stderr: &str| {
            let error = super::contact_failure(stderr);
            error
                .downcast_ref::<DesktopError>()
                .map(|error| (error.exit_status(), error.to_string()))
        };
        let (code, message) = status("88:188: execution error: Loopflow Desktop (com.loopflow.mac.dev) is not running. (-600)\n").unwrap();
        assert_eq!(code, 4);
        assert!(
            message.starts_with("Loopflow Desktop (com.loopflow.mac.dev)"),
            "{message}"
        );
        let (code, message) = status(
            "12:40: execution error: Loopflow Dev got an error: AppleEvent timed out. (-1712)",
        )
        .unwrap();
        assert_eq!(code, 4);
        assert!(
            message.contains("Privacy & Security › Automation"),
            "{message}"
        );
        assert_eq!(
            status(
                "1:2: execution error: Not authorized to send Apple events to Loopflow. (-1743)"
            )
            .unwrap()
            .0,
            4
        );
        let (code, message) = status("1:2: execution error: Loopflow got an error: The pane content was replaced; inspect Desktop again. (1003)").unwrap();
        assert_eq!(code, 3);
        assert!(message.ends_with("inspect Desktop again."), "{message}");
        let (code, _) = status("1:2: execution error: Loopflow got an error: Unknown action pane.spin; see lf desktop actions. Nothing changed. (1002)").unwrap();
        assert_eq!(code, 2);
        // An older Desktop exits 1 and names the fix.
        let older = super::contact_failure(
            "1:2: execution error: Loopflow got an error: AppleEvent handler failed. (-1708)",
        );
        assert!(older.downcast_ref::<DesktopError>().is_none());
        assert!(older.to_string().contains("update Desktop"));
        // Other failures, including a message that only mentions a number, stay 1.
        assert!(status(
            "1:2: execution error: Loopflow got an error: decoding failed (1003 bytes) (-10000)"
        )
        .is_none());
        assert!(status("osascript crashed").is_none());
    }

    #[test]
    fn capture_trim_drops_screen_padding_only() {
        assert_eq!(trim_capture("a  \n  b\n\n   \n"), "a\n  b");
        assert_eq!(trim_capture("\n\n"), "");
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
            let error = validate_literal_text(text).unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<DesktopError>()
                    .map(DesktopError::exit_status),
                Some(2),
                "{text:?}"
            );
        }
    }
}
