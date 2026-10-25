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

/// One `lf desktop events` frame: changed windows, whole, and closed window IDs.
/// The first frame after subscribing carries every window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopChange {
    pub observed_at: i64,
    pub windows: Vec<DesktopWindowInspection>,
    pub closed: Vec<String>,
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
    Entered {
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
    /// Shell tabs with a live surface; `None` before it starts and for browser tabs.
    pub commands: Option<DesktopCommandStatus>,
}

/// A shell's commands as its shell integration reports them: lines `run`
/// submitted, commands finished, and the last one's exit status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopCommandStatus {
    pub submitted: u32,
    pub finished: u32,
    pub exit_status: Option<i32>,
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

#[derive(
    Debug, Clone, Copy, PartialEq, Serialize, Deserialize, clap::ValueEnum, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DesktopDirection {
    Left,
    Right,
    Up,
    Down,
}

impl DesktopDirection {
    fn as_str(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Right => "right",
            Self::Up => "up",
            Self::Down => "down",
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
    /// Not running, not macOS, or not answering its socket.
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
        ["desktop", "events"] => Some(schema::<DesktopChange>()),
        ["desktop", "wait"] => None,
        // Saved-intent commands answered above; `rename` writes a shared owner's record.
        ["desktop", verb] if !READING_VERBS.contains(verb) && *verb != "rename" => {
            Some(schema::<DesktopVerbResult>())
        }
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
    let json = cli.json || verb.target.json;
    let mut desktop = DesktopConnection::open()?;
    match verb.kind {
        VerbKind::State => {
            let reading: DesktopInspection = desktop.call("inspect", serde_json::json!({}))?;
            if json {
                println!("{}", serde_json::to_string_pretty(&reading)?);
            } else {
                println!("{}", reading.render());
            }
            return Ok(());
        }
        VerbKind::Events => return stream_events(desktop, json),
        _ => {}
    }
    // One live reading supplies every lifetime token; dispatch never
    // reinterprets the request using later focus.
    let resolution = super::open::selected_context(cli, None)?;
    let reading: DesktopInspection = desktop.call("inspect", serde_json::json!({}))?;
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
        VerbKind::State | VerbKind::Events => unreachable!("answered above"),
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
                .pane(window, verb.target.pane.as_deref(), PaneNeed::Terminal)
                .map_err(DesktopError::target)?;
            let request = DesktopTextRequest {
                target: pane.target(window),
                surface: pane.surface()?,
                region: *region,
                max_bytes: *max_bytes,
            };
            read_text(&mut desktop, request, json, *trim)
        }
        VerbKind::Wait { timeout } => {
            let pane = scope
                .pane(window, verb.target.pane.as_deref(), PaneNeed::Any)
                .map_err(DesktopError::target)?;
            let watched = WaitTarget::of(window, pane);
            drop(desktop);
            wait(watched, *timeout, json)
        }
        VerbKind::Action { name, args } => {
            let descriptor = window
                .actions
                .iter()
                .find(|action| action.name == *name)
                .ok_or_else(|| {
                    DesktopError::Argument(format!(
                        "This Desktop has no verb {name}; see `lf desktop actions`, or update Desktop. Nothing changed."
                    ))
                })?;
            let mut args = args.clone();
            let (pane, resolved) = match descriptor.scope {
                DesktopActionScope::Window => (
                    None,
                    DesktopResolution {
                        window: window.window.clone(),
                        work: None,
                        pane: None,
                        tab: None,
                        rule: DesktopTargetRule::Window,
                    },
                ),
                DesktopActionScope::Pane => {
                    let pane = scope
                        .pane(
                            window,
                            verb.target.pane.as_deref(),
                            PaneNeed::for_action(name),
                        )
                        .map_err(DesktopError::target)?;
                    // Pane arguments are Rex selectors in the same workspace.
                    for key in ["destination", "toward"] {
                        if let Some(selector) = args.get(key) {
                            let peer = scope.peer(pane, selector).map_err(DesktopError::target)?;
                            args.insert(key.into(), peer);
                        }
                    }
                    if matches!(name.as_str(), "send" | "send-key") {
                        args.insert("surface".into(), pane.surface()?);
                    }
                    let resolved = DesktopResolution {
                        window: window.window.clone(),
                        work: Some(pane.workspace.work.clone()),
                        pane: pane.leaf.pane.clone(),
                        tab: pane.tab.map(|tab| tab.id.clone()),
                        rule: pane.rule,
                    };
                    (Some(pane.target(window)), resolved)
                }
            };
            perform(
                &mut desktop,
                DesktopActionRequest {
                    repository: window.repository.clone(),
                    window: window.window.clone(),
                    pane,
                    action: name.clone(),
                    args,
                },
                resolved,
                json,
            )
        }
    }
}

/// A parsed `lf desktop` live command: what it reads or which verb it performs.
#[derive(Debug)]
struct Verb {
    kind: VerbKind,
    /// `--json` rides in the target arguments for every verb.
    target: crate::lf::DesktopTargetArgs,
}

#[derive(Debug)]
enum VerbKind {
    State,
    Ls,
    Actions,
    Events,
    Wait {
        timeout: Option<std::time::Duration>,
    },
    Capture {
        region: DesktopTextRegion,
        max_bytes: usize,
        trim: bool,
    },
    /// A registry action; its name is the verb.
    Action {
        name: String,
        args: std::collections::BTreeMap<String, String>,
    },
}

/// Verbs that read rather than perform a registry action.
pub const READING_VERBS: &[&str] = &["ls", "state", "actions", "events", "wait", "capture"];

impl Verb {
    fn parse(command: &crate::lf::DesktopCommand) -> Result<Self> {
        use crate::lf::{DesktopCommand, DesktopTargetArgs, DesktopWindowArgs};
        let argument =
            |message: String| -> anyhow::Error { DesktopError::Argument(message).into() };
        let action = |name: &str, args: Vec<(&str, String)>| VerbKind::Action {
            name: name.to_owned(),
            args: args
                .into_iter()
                .map(|(key, value)| (key.to_owned(), value))
                .collect(),
        };
        let bare = |json: bool| DesktopTargetArgs {
            json,
            ..Default::default()
        };
        let window = |args: &DesktopWindowArgs| DesktopTargetArgs {
            pane: None,
            window: args.window.clone(),
            json: args.json,
        };
        let (kind, target) = match command {
            DesktopCommand::State { json } => (VerbKind::State, bare(*json)),
            DesktopCommand::Ls { window, json } => (
                VerbKind::Ls,
                DesktopTargetArgs {
                    window: window.clone(),
                    ..bare(*json)
                },
            ),
            DesktopCommand::Actions { json } => (VerbKind::Actions, bare(*json)),
            DesktopCommand::Events { json } => (VerbKind::Events, bare(*json)),
            DesktopCommand::Wait { timeout, target } => {
                (VerbKind::Wait { timeout: *timeout }, target.clone())
            }
            DesktopCommand::Back { target } => (action("back", vec![]), window(target)),
            DesktopCommand::Forward { target } => (action("forward", vec![]), window(target)),
            DesktopCommand::Next { target } => (action("next", vec![]), window(target)),
            DesktopCommand::Previous { target } => (action("previous", vec![]), window(target)),
            DesktopCommand::WorkingTasks { target } => {
                (action("working-tasks", vec![]), window(target))
            }
            DesktopCommand::Goto { index, target } => (
                action("goto", vec![("index", index.to_string())]),
                window(target),
            ),
            DesktopCommand::Enter { task, target } => (
                action("enter", vec![("task", task.clone())]),
                window(target),
            ),
            DesktopCommand::Send { text, target } => {
                let text = match text {
                    Some(text) => text.clone(),
                    None => std::io::read_to_string(std::io::stdin().lock())
                        .context("read text to send from stdin")?,
                };
                validate_literal_text(&text)?;
                (action("send", vec![("text", text)]), target.clone())
            }
            DesktopCommand::SendKey { key, target } => (
                action("send-key", vec![("key", key.as_str().to_owned())]),
                target.clone(),
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
                    target.clone(),
                )
            }
            DesktopCommand::Run { line, target } => {
                if let Some(line) = line {
                    validate_literal_text(line)?;
                }
                (
                    action(
                        "run",
                        line.iter().map(|line| ("command", line.clone())).collect(),
                    ),
                    target.clone(),
                )
            }
            DesktopCommand::Zoom { on, target, .. } => {
                (action("zoom", vec![("on", on.to_string())]), target.clone())
            }
            DesktopCommand::Focus { direction, target } => (
                action(
                    "focus",
                    direction
                        .iter()
                        .map(|direction| ("direction", direction.as_str().to_owned()))
                        .collect(),
                ),
                target.clone(),
            ),
            DesktopCommand::Hide { target } => (action("hide", vec![]), target.clone()),
            DesktopCommand::Show { target } => (action("show", vec![]), target.clone()),
            DesktopCommand::Close { target } => (action("close", vec![]), target.clone()),
            DesktopCommand::Reopen { target } => (action("reopen", vec![]), target.clone()),
            DesktopCommand::Files { target } => (action("files", vec![]), target.clone()),
            DesktopCommand::GraphLog { target } => (action("graph-log", vec![]), target.clone()),
            DesktopCommand::PageBack { target } => (action("page-back", vec![]), target.clone()),
            DesktopCommand::PageForward { target } => {
                (action("page-forward", vec![]), target.clone())
            }
            DesktopCommand::Reload { target } => (action("reload", vec![]), target.clone()),
            DesktopCommand::Browse { url, target } => {
                (action("browse", vec![("url", url.clone())]), target.clone())
            }
            DesktopCommand::Switch { session, target } => (
                action("switch", vec![("session", session.clone())]),
                target.clone(),
            ),
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
                target.clone(),
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
                    target.clone(),
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
        Ok(Self { kind, target })
    }
}

/// How a command's target was chosen, so a script can pin it by ID next time.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum DesktopTargetRule {
    /// A window-level verb: `--window`, the caller's window or the only one.
    Window,
    /// `-p` named it.
    Pane,
    /// Root `--task`/`--wave` chose the view; the pane was the only eligible one.
    Work,
    /// The terminal the command ran in (`LF_TERMINAL_ID`).
    Caller,
    /// The checkout's Task binding chose the view; the pane was the only eligible one.
    Binding,
}

/// Full IDs of what a verb acted on and the rule that chose them.
#[derive(Debug, Clone, PartialEq, Serialize, schemars::JsonSchema)]
pub struct DesktopResolution {
    pub window: String,
    pub work: Option<WorkWorkspaceKey>,
    pub pane: Option<String>,
    pub tab: Option<String>,
    pub rule: DesktopTargetRule,
}

/// A verb's output: Desktop's receipt plus how lf resolved the target.
#[derive(Debug, Serialize, schemars::JsonSchema)]
struct DesktopVerbResult {
    #[serde(flatten)]
    receipt: DesktopActionReceipt,
    target: DesktopResolution,
}

fn perform(
    desktop: &mut DesktopConnection,
    request: DesktopActionRequest,
    target: DesktopResolution,
    json: bool,
) -> Result<()> {
    let receipt: DesktopActionReceipt = desktop.call("perform", &request)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&DesktopVerbResult { receipt, target })?
        );
    } else if let Some(pane) = &receipt.pane {
        // Full IDs, so the line can be passed back as -p.
        println!("{}", receipt.tab.as_deref().unwrap_or(pane));
    } else {
        println!("{}", receipt.destination);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Events and waits: one socket subscription; `state` is the snapshot they update.
// ---------------------------------------------------------------------------

fn stream_events(desktop: DesktopConnection, json: bool) -> Result<()> {
    use std::io::Write;
    let mut events = desktop.events()?;
    let mut stdout = std::io::stdout().lock();
    while let Some(change) = events.next(None)? {
        if json {
            writeln!(stdout, "{}", serde_json::to_string(&change)?)?;
        } else {
            for window in &change.windows {
                writeln!(
                    stdout,
                    "{} {}",
                    short(&window.window),
                    window.render_change()
                )?;
            }
            for window in &change.closed {
                writeln!(stdout, "{} closed", short(window))?;
            }
        }
        stdout.flush()?;
    }
    Ok(())
}

/// What `wait` watches, by full ID so a later reading cannot retarget it.
#[derive(Debug, Clone, PartialEq)]
struct WaitTarget {
    window: String,
    work: WorkWorkspaceKey,
    pane: String,
    tab: Option<String>,
}

/// One observation of the watched target.
#[derive(Debug, Clone, Copy, PartialEq)]
enum WaitState {
    /// Not there yet, or a terminal without a running surface.
    Pending,
    /// A pane that is ready, or a tab that closed: exit 0.
    Ready,
    /// A shell's commands: lines `run` submitted, finished, last status.
    Commands {
        submitted: u32,
        finished: u32,
        exit_status: Option<i32>,
    },
}

impl WaitTarget {
    fn of(window: &DesktopWindowInspection, pane: ResolvedPane<'_>) -> Self {
        Self {
            window: window.window.clone(),
            work: pane.workspace.work.clone(),
            pane: pane.leaf.pane.clone().unwrap_or_default(),
            tab: pane.tab.map(|tab| tab.id.clone()),
        }
    }

    fn observe(&self, window: &DesktopWindowInspection) -> WaitState {
        let Some(leaf) = window
            .workspaces
            .iter()
            .find(|workspace| workspace.work == self.work)
            .and_then(|workspace| {
                workspace
                    .layout
                    .leaves()
                    .into_iter()
                    .find(|leaf| leaf.pane.as_deref() == Some(&self.pane))
            })
        else {
            return WaitState::Pending;
        };
        let Some(tab) = &self.tab else {
            // A terminal pane is ready when its surface runs; a browser at once.
            return match leaf.content.as_deref() {
                Some("session") if leaf.surface.is_none() => WaitState::Pending,
                _ => WaitState::Ready,
            };
        };
        match leaf.tabs.iter().find(|candidate| candidate.id == *tab) {
            // A closed tab, or a shell that exited, has finished.
            None => WaitState::Ready,
            Some(tab) => match &tab.commands {
                Some(commands) => WaitState::Commands {
                    submitted: commands.submitted,
                    finished: commands.finished,
                    exit_status: commands.exit_status,
                },
                None if tab.kind == DesktopTabKind::Shell => WaitState::Pending,
                None => WaitState::Ready,
            },
        }
    }
}

/// The exit status once the wait is over: lines `run` submitted have all finished,
/// else a command finished after the wait began; a ready pane or closed tab is 0.
fn wait_outcome(baseline: Option<u32>, state: WaitState) -> Option<u8> {
    match state {
        WaitState::Pending => None,
        WaitState::Ready => Some(0),
        WaitState::Commands {
            submitted,
            finished,
            exit_status,
        } => {
            let done = if submitted > 0 {
                finished >= submitted
            } else {
                baseline.is_some_and(|baseline| finished > baseline)
            };
            done.then(|| exit_status.map_or(0, |status| status.clamp(0, 255) as u8))
        }
    }
}

fn wait(target: WaitTarget, timeout: Option<std::time::Duration>, json: bool) -> Result<()> {
    let deadline = timeout.map(|timeout| std::time::Instant::now() + timeout);
    let mut events = DesktopConnection::open()?.events()?;
    let mut baseline = None;
    loop {
        let remaining = match deadline {
            Some(deadline) => match deadline.checked_duration_since(std::time::Instant::now()) {
                Some(remaining) => Some(remaining),
                None => break,
            },
            None => None,
        };
        let Some(change) = events.next(remaining)? else {
            break;
        };
        if change.closed.contains(&target.window) {
            return Err(DesktopError::Target(
                "The window closed while waiting; nothing is left to wait for.".into(),
            )
            .into());
        }
        let Some(window) = change
            .windows
            .iter()
            .find(|window| window.window == target.window)
        else {
            continue;
        };
        let state = target.observe(window);
        if let WaitState::Commands { finished, .. } = state {
            baseline.get_or_insert(finished);
        }
        if let Some(status) = wait_outcome(baseline, state) {
            if json {
                println!(
                    "{}",
                    serde_json::json!({ "pane": target.pane, "tab": target.tab, "status": status })
                );
            }
            return match status {
                0 => Ok(()),
                status => Err(crate::process::CommandExit(status).into()),
            };
        }
    }
    bail!(
        "{} did not finish within {}.",
        target.tab.as_deref().unwrap_or(&target.pane),
        timeout.map_or_else(String::new, |timeout| format!("{}s", timeout.as_secs()))
    )
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
            _ if matches!(action, "send" | "send-key") => Self::Terminal,
            _ if action == "show" => Self::Hidden,
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
    rule: DesktopTargetRule,
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
                                rule: DesktopTargetRule::Caller,
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
                            rule: DesktopTargetRule::Caller,
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

    /// A tile of the view (`-p 3`, `-p LOO-519`), then root Work flags, then the
    /// caller's workspace, then the checkout's binding.
    fn workspace(
        &self,
        window: &'a DesktopWindowInspection,
        tile: Option<&str>,
    ) -> Result<(&'a DesktopWorkspaceInspection, DesktopTargetRule)> {
        use crate::ops::context::ContextFact;
        if let Some(selector) = tile {
            return Ok((self.tile(window, selector)?, DesktopTargetRule::Pane));
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
                return Ok((by_selection(selection)?, DesktopTargetRule::Work));
            }
        }
        if let Some(id) = &self.caller {
            return match self.caller() {
                Some((caller, pane)) if caller.window == window.window => Ok((pane.workspace, DesktopTargetRule::Caller)),
                Some(_) => bail!("This terminal belongs to another Desktop window; pass --task or -p TILE/PANE. Nothing changed."),
                None => bail!("This terminal ({id}) is no longer in Desktop; pass --task or -p TILE/PANE. Nothing changed."),
            };
        }
        match selected() {
            Some(selection) => Ok((by_selection(selection)?, DesktopTargetRule::Binding)),
            None => bail!("No Task selected: pass --task or -p TILE/PANE, or run from a Task's checkout or Desktop terminal. Nothing changed."),
        }
    }

    /// The window shows a Dashboard (saved, Working Tasks or an unsaved query):
    /// its tiles are the view's parts.
    fn shows_board(window: &DesktopWindowInspection) -> bool {
        matches!(
            window.destination,
            DesktopDestination::Dashboard { .. }
                | DesktopDestination::WorkingTasks
                | DesktopDestination::View { .. }
        )
    }

    /// A tile: on a Dashboard its members in slot order, else the sidebar's Tasks (⌘N).
    fn tile(
        &self,
        window: &'a DesktopWindowInspection,
        selector: &str,
    ) -> Result<&'a DesktopWorkspaceInspection> {
        let tiles: Vec<&WorkWorkspaceKey> = if Self::shows_board(window) {
            window.open_work.iter().collect()
        } else {
            window.list.iter().map(|entry| &entry.work).collect()
        };
        let label = |work: &WorkWorkspaceKey| {
            let entry = window.list.iter().find(|entry| entry.work == *work);
            let title = window
                .workspaces
                .iter()
                .find(|workspace| workspace.work == *work)
                .map(|workspace| workspace.title.clone());
            entry
                .and_then(|entry| entry.identifier.clone())
                .into_iter()
                .chain(entry.map(|entry| entry.title.clone()))
                .chain(title)
                .collect::<Vec<_>>()
        };
        let index = resolve(
            "tile",
            selector,
            &tiles,
            |work| vec![work.id.clone()],
            |work| label(work),
            |work| {
                label(work)
                    .into_iter()
                    .next()
                    .unwrap_or_else(|| work.id.clone())
            },
        )?;
        let work = tiles[index];
        window
            .workspaces
            .iter()
            .find(|workspace| workspace.work == *work)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "{} has no workspace in this Desktop window yet; open it first. Nothing changed.",
                    work.id
                )
            })
    }

    /// `-p` is `[TILE/]PANE`; on a Dashboard a lone part is a tile first, then a pane.
    fn pane(
        &self,
        window: &'a DesktopWindowInspection,
        path: Option<&str>,
        need: PaneNeed,
    ) -> Result<ResolvedPane<'a>> {
        let (tile, selector) = match path {
            Some(path) => match path.split_once('/') {
                Some((tile, pane)) => (Some(tile), Some(pane).filter(|pane| !pane.is_empty())),
                None if Self::shows_board(window) && self.tile(window, path).is_ok() => {
                    (Some(path), None)
                }
                None => (None, Some(path)),
            },
            None => (None, None),
        };
        let (workspace, rule) = self.workspace(window, tile)?;
        let rule = if selector.is_some() {
            DesktopTargetRule::Pane
        } else {
            rule
        };
        let leaves = workspace.layout.leaves();
        let pane = |leaf, tab| ResolvedPane {
            workspace,
            leaf,
            tab,
            rule,
        };
        let Some(selector) = selector else {
            // Without a pane part, a command from one of this workspace's terminals acts on that terminal.
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
                .flat_map(|leaf| -> Vec<ResolvedPane<'a>> {
                    let content = leaf.content.as_deref();
                    match need {
                        PaneNeed::Any => vec![pane(leaf, None)],
                        PaneNeed::Kind(kind) if content == Some(kind) => vec![pane(leaf, None)],
                        PaneNeed::Hidden
                            if leaf
                                .pane
                                .as_ref()
                                .is_some_and(|id| workspace.hidden_panes.contains(id)) =>
                        {
                            vec![pane(leaf, None)]
                        }
                        // Retained shells, including background tabs, without consulting selection.
                        PaneNeed::Terminal if content == Some("session") => vec![pane(leaf, None)],
                        PaneNeed::Terminal if content == Some("shell") => {
                            leaf.tabs.iter().map(|tab| pane(leaf, Some(tab))).collect()
                        }
                        _ => vec![],
                    }
                })
                .collect::<Vec<_>>();
            return match candidates.as_slice() {
                [pane] => Ok(*pane),
                [] => bail!("No eligible pane in {}; nothing changed.", workspace.title),
                _ => bail!(
                    "Several panes are eligible: {}. Choose one with -p; nothing changed.",
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
                return Ok(pane(leaf, Some(tab)));
            }
        }
        // Positions count panes in layout order; tabs are reached with `kind:N`.
        if let Ok(position) = selector.parse::<usize>() {
            return position
                .checked_sub(1)
                .and_then(|index| leaves.get(index))
                .map(|leaf| pane(leaf, None))
                .ok_or_else(|| {
                    anyhow::anyhow!(
                        "No pane at position {position}; there are {}. Nothing changed.",
                        leaves.len()
                    )
                });
        }
        let candidates = leaves
            .iter()
            .flat_map(|leaf| {
                std::iter::once(pane(leaf, None))
                    .chain(leaf.tabs.iter().map(|tab| pane(leaf, Some(tab))))
            })
            .collect::<Vec<_>>();
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
fn read_text(
    desktop: &mut DesktopConnection,
    request: DesktopTextRequest,
    json: bool,
    trim: bool,
) -> Result<()> {
    let reading: DesktopTextReading = desktop.call("read_text", &request)?;
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

// ---------------------------------------------------------------------------
// Desktop's socket (`DesktopServer.swift`): newline-delimited JSON at
// `$LF_HOME/run/desktop/<bundle-id>.sock`. A typed error code, never the
// message, decides the exit status.
// ---------------------------------------------------------------------------

/// Mirrors `DesktopServer.version`; a mismatch names the fix instead of guessing.
const DESKTOP_API_VERSION: u32 = 1;

/// The socket the caller's Desktop serves: `LF_DESKTOP`'s bundle, else the installed app.
pub(crate) fn socket_path() -> std::path::PathBuf {
    crate::store::lf_home_dir()
        .join("run")
        .join("desktop")
        .join(format!("{}.sock", super::open::desktop_bundle()))
}

/// One client connection; requests are answered in order.
struct DesktopConnection {
    reader: std::io::BufReader<std::os::unix::net::UnixStream>,
    writer: std::os::unix::net::UnixStream,
    next_id: u64,
}

#[derive(Deserialize)]
struct DesktopReply {
    id: Option<u64>,
    result: Option<serde_json::Value>,
    error: Option<DesktopReplyError>,
}

#[derive(Deserialize)]
struct DesktopReplyError {
    code: String,
    message: String,
}

#[derive(Deserialize)]
struct DesktopEventFrame {
    event: DesktopChange,
}

impl DesktopConnection {
    fn open() -> Result<Self> {
        let path = socket_path();
        let bundle = super::open::desktop_bundle();
        let stream = std::os::unix::net::UnixStream::connect(&path).map_err(|error| {
            DesktopError::Unreachable(format!(
                "Loopflow Desktop ({bundle}) is not running or not serving {} ({error}). Open it with lf open, or inspect Work with lf task status <task>.",
                path.display()
            ))
        })?;
        // Desktop answers on its main thread; a hung app is unreachable, not a failure.
        stream.set_read_timeout(Some(std::time::Duration::from_secs(10)))?;
        Ok(Self {
            reader: std::io::BufReader::new(stream.try_clone()?),
            writer: stream,
            next_id: 1,
        })
    }

    fn call<R: serde::de::DeserializeOwned>(
        &mut self,
        method: &str,
        params: impl Serialize,
    ) -> Result<R> {
        use std::io::{BufRead, Write};
        let id = self.next_id;
        self.next_id += 1;
        let mut line = serde_json::to_vec(&serde_json::json!({
            "v": DESKTOP_API_VERSION, "id": id, "method": method, "params": params,
        }))?;
        line.push(b'\n');
        self.writer.write_all(&line).map_err(|error| {
            DesktopError::Unreachable(format!("Desktop closed its socket: {error}"))
        })?;
        let mut reply = String::new();
        match self.reader.read_line(&mut reply) {
            Ok(0) => return Err(lost_reply("Desktop closed the connection")),
            Ok(_) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                return Err(DesktopError::Unreachable(
                    "Desktop did not answer within 10 seconds. Inspect again before retrying."
                        .into(),
                )
                .into())
            }
            Err(error) => return Err(lost_reply(&error.to_string())),
        }
        let reply: DesktopReply =
            serde_json::from_str(&reply).context("Desktop returned an unreadable reply")?;
        if let Some(error) = reply.error {
            return Err(reply_failure(error));
        }
        anyhow::ensure!(reply.id == Some(id), "Desktop answered a different request");
        serde_json::from_value(reply.result.unwrap_or_default()).with_context(|| {
            format!("Desktop returned an invalid {method} reply; inspect before retrying")
        })
    }

    /// Subscribe; the first change carries every window, later ones what changed.
    fn events(mut self) -> Result<DesktopEvents> {
        let _: serde_json::Value = self.call("events", serde_json::json!({}))?;
        Ok(DesktopEvents {
            reader: self.reader,
        })
    }
}

/// A subscribed connection, read one change at a time.
struct DesktopEvents {
    reader: std::io::BufReader<std::os::unix::net::UnixStream>,
}

impl DesktopEvents {
    /// The next change, or `None` when `timeout` elapses first.
    fn next(&mut self, timeout: Option<std::time::Duration>) -> Result<Option<DesktopChange>> {
        use std::io::BufRead;
        // A zero timeout means none to the socket; the caller checks its deadline first.
        let timeout = timeout.map(|timeout| timeout.max(std::time::Duration::from_millis(1)));
        self.reader.get_ref().set_read_timeout(timeout)?;
        let mut line = String::new();
        match self.reader.read_line(&mut line) {
            Ok(0) => Err(DesktopError::Unreachable(
                "Desktop quit; no more changes will arrive.".into(),
            )
            .into()),
            Ok(_) => Ok(Some(
                serde_json::from_str::<DesktopEventFrame>(&line)
                    .context("Desktop sent an unreadable change")?
                    .event,
            )),
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) =>
            {
                Ok(None)
            }
            Err(error) => Err(lost_reply(&error.to_string())),
        }
    }
}

/// A lost reply does not prove the request had no effect. In particular,
/// repeating `run` allocates another tab; inspect before retrying.
fn lost_reply(reason: &str) -> anyhow::Error {
    anyhow::anyhow!("Desktop request failed: {reason}. Inspect again before retrying; `lf task status <task>` remains available in this terminal.")
}

fn reply_failure(error: DesktopReplyError) -> anyhow::Error {
    let DesktopReplyError { code, message } = error;
    match code.as_str() {
        "argument" => DesktopError::Argument(message).into(),
        "target" => DesktopError::Target(message).into(),
        "version" => anyhow::anyhow!("{message} Update Desktop and lf to the same release."),
        _ => anyhow::anyhow!("Desktop failed: {message}. Inspect again before retrying."),
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
    /// One `lf desktop events` line: what the window shows and where focus is.
    fn render_change(&self) -> String {
        let selected = self.workspaces.iter().find(|workspace| workspace.selected);
        match selected {
            Some(workspace) => format!(
                "showing {} · {} focused {}{}",
                self.destination,
                workspace.title,
                workspace.focused_pane,
                workspace
                    .zoomed_pane
                    .as_deref()
                    .map_or_else(String::new, |pane| format!(" · {pane} zoomed"))
            ),
            None => format!("showing {}", self.destination),
        }
    }

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
            Self::Entered { from, work } => write!(f, "{} entered from {from}", selector(work)),
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

    /// `-p TILE/PANE` when a tile is named, else `-p PANE`.
    fn pane(
        scope: &Scope,
        tile: Option<&str>,
        pane: Option<&str>,
        need: PaneNeed,
    ) -> anyhow::Result<String> {
        let window = scope.window(None)?;
        let path = match tile {
            Some(tile) => Some(format!("{tile}/{}", pane.unwrap_or_default())),
            None => pane.map(str::to_owned),
        };
        Ok(scope.pane(window, path.as_deref(), need)?.describe())
    }

    #[test]
    fn tasks_resolve_by_id_label_position_then_suffix() {
        let (inspection, work) = fixture();
        let scope = scope(&inspection, &work, None);
        let window = scope.window(None).unwrap();
        let task = |selector: &str| {
            scope
                .workspace(window, Some(selector))
                .map(|(workspace, _)| workspace.work.id.clone())
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
        // Without -p or a caller nothing follows focus: several panes are ambiguous.
        let error = pane(&scope, None, None, any).unwrap_err().to_string();
        assert!(
            error.contains("session") && error.contains("browser") && error.contains("-p"),
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
            .pane(window, Some("browser:1"), PaneNeed::Terminal)
            .unwrap();
        assert!(browser
            .surface()
            .unwrap_err()
            .to_string()
            .contains("not a terminal"));
        let shell = scope.pane(window, None, PaneNeed::Any).unwrap();
        assert_eq!(scope.peer(shell, "session").unwrap(), "session");
        assert!(scope.peer(shell, "shell").is_err());
        assert!(scope.peer(shell, "elsewhere").is_err());
    }

    #[test]
    fn on_a_dashboard_a_lone_part_is_a_tile_and_a_slash_reaches_inside_it() {
        let (mut inspection, work) = fixture();
        let window = &mut inspection.windows[0];
        window.destination = super::DesktopDestination::WorkingTasks;
        window.open_work = vec![window.workspaces[0].work.clone()];
        let scope = scope(&inspection, &work, None);
        // `-p 1` is the first tile; its only shell answers a shell verb.
        assert_eq!(
            pane(&scope, None, Some("1"), PaneNeed::Kind("shell")).unwrap(),
            "shell (shell)"
        );
        assert_eq!(
            pane(&scope, Some("LOO-519"), Some("shell:1"), PaneNeed::Any).unwrap(),
            "shell/shell-tab (Shell)"
        );
        let window = scope.window(None).unwrap();
        let tile = scope
            .pane(window, Some("loo-519/browser"), PaneNeed::Any)
            .unwrap();
        assert_eq!(tile.rule, super::DesktopTargetRule::Pane);
    }

    #[test]
    fn wait_returns_the_submitted_command_status_or_the_next_finish() {
        use super::{wait_outcome, WaitState};
        let commands = |submitted, finished, exit_status| WaitState::Commands {
            submitted,
            finished,
            exit_status,
        };
        // `run 'false'`: one line submitted, still running, then finished with 1.
        assert_eq!(wait_outcome(Some(0), commands(1, 0, None)), None);
        assert_eq!(wait_outcome(Some(0), commands(1, 1, Some(1))), Some(1));
        // Already finished before the wait began: answered at once.
        assert_eq!(wait_outcome(Some(1), commands(1, 1, Some(0))), Some(0));
        // A tab nobody ran into waits for the next finish after the wait began.
        assert_eq!(wait_outcome(Some(3), commands(0, 3, Some(0))), None);
        assert_eq!(wait_outcome(Some(3), commands(0, 4, Some(2))), Some(2));
        assert_eq!(wait_outcome(None, WaitState::Pending), None);
        assert_eq!(wait_outcome(None, WaitState::Ready), Some(0));
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
        let cli = <crate::lf::Cli as clap::CommandFactory>::command();
        let desktop = cli.find_subcommand("desktop").unwrap();
        let saved = [
            "view", "query", "pin", "unpin", "rename", "arrange", "slots", "order", "layout",
        ];
        for verb in desktop.get_subcommands().map(clap::Command::get_name) {
            if saved.contains(&verb) || super::READING_VERBS.contains(&verb) {
                continue;
            }
            assert!(names.contains(&verb), "{verb} is not a registry action");
        }
    }

    #[test]
    fn desktop_failures_take_their_status_from_the_error_code() {
        let status = |code: &str| {
            super::reply_failure(super::DesktopReplyError {
                code: code.into(),
                message: "No pane 9. Nothing changed.".into(),
            })
            .downcast_ref::<DesktopError>()
            .map(DesktopError::exit_status)
        };
        assert_eq!(status("argument"), Some(2));
        assert_eq!(status("target"), Some(3));
        assert_eq!(status("internal"), None);
        let older = super::reply_failure(super::DesktopReplyError {
            code: "version".into(),
            message: "This Desktop speaks API version 2, not 1.".into(),
        });
        assert!(older.to_string().contains("Update Desktop"));
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
