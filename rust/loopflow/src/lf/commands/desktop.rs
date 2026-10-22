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
    pub selection_kind: Option<String>,
    pub selection_id: Option<String>,
    pub reading: String,
    pub reason: Option<String>,
    pub task: Option<DesktopTaskInspection>,
    pub session: Option<DesktopSessionInspection>,
    pub supported_operations: Vec<String>,
    pub workspaces: Vec<DesktopWorkspaceInspection>,
    /// Work shown in this window, in slot fill order; retained workspaces outside it
    /// remain inspectable.
    pub open_work: Vec<WorkWorkspaceKey>,
    pub view: Option<crate::work::selection::ViewPlan>,
    pub opening: Option<DesktopOpening>,
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
    pub tabs: Vec<DesktopShellInspection>,
    pub selected_tab: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopShellInspection {
    pub id: String,
    pub incarnation: String,
    pub surface: Option<String>,
    pub location: Option<DesktopContentLocation>,
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

/// Identifies the affected pane without implying a rendered or ready terminal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopPaneReceipt {
    pub repository: String,
    pub window: String,
    pub work: WorkWorkspaceKey,
    pub pane: String,
    pub tab: Option<String>,
    pub observed_at: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DesktopPaneCommand {
    pub target: DesktopPaneTarget,
    pub action: DesktopPaneAction,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
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
    GraphLog {
        task: String,
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
        ["desktop", "read"] => Some(schema::<DesktopTextReading>()),
        ["desktop", "text" | "key" | "hide" | "restore" | "focus" | "shell" | "files" | "graph-log"
        | "move" | "resize" | "zoom"] => Some(schema::<DesktopPaneReceipt>()),
        _ => None,
    }
}

pub fn require_supported() -> Result<()> {
    if !cfg!(target_os = "macos") {
        bail!("Loopflow Desktop launching and control require macOS; use `lf task status <task>` or `lf --task <task>` in this terminal instead");
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

/// A passive plan or the acceptance receipt for opening its native destination.
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
        .args(["-b", super::open::DESKTOP_BUNDLE, url.as_str()])
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

fn selection_failure(error: anyhow::Error) -> anyhow::Error {
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
        _ if error.downcast_ref::<serde_json::Error>().is_some() => Some(2),
        _ if matches!(
            error.downcast_ref::<crate::store::StoreError>(),
            Some(crate::store::StoreError::NotFound)
        ) =>
        {
            Some(3)
        }
        _ => None,
    };
    if let Some(code) = code {
        eprintln!("Error: {error}");
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
    use crate::work::selection::{Arrangement, DashboardId, WorkPaneLayout};
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
            let arrangement: Arrangement = serde_json::from_reader(std::io::stdin().lock())?;
            serde_json::to_value(store.save_dashboard(
                &repository,
                &id,
                Some(*revision),
                &reading.saved.selection,
                &arrangement,
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
    use crate::lf::DesktopCommand;
    match command {
        DesktopCommand::Query { .. }
        | DesktopCommand::Pin { .. }
        | DesktopCommand::Unpin { .. }
        | DesktopCommand::Rename { .. }
        | DesktopCommand::Arrange { .. }
        | DesktopCommand::Slots { .. }
        | DesktopCommand::Order { .. }
        | DesktopCommand::Layout { .. } => {
            return saved_intent(cli, command).map_err(selection_failure)
        }
        DesktopCommand::View {
            expression,
            plan,
            json,
        } => {
            return view_plan(cli, expression.as_deref(), *plan, *json || cli.json)
                .map_err(selection_failure)
        }
        _ => {}
    }
    require_supported()?;
    if let DesktopCommand::State { json } = command {
        return inspect(*json || cli.json);
    }
    // Work selects the repository/checkout. One live reading supplies every
    // lifetime token; dispatch never reinterprets this request using later focus.
    let resolution = super::open::selected_context(cli, None)?;
    let inspection: DesktopInspection = serde_json::from_slice(&contact(None, false)?)
        .context("Desktop returned an invalid reading")?;
    let caller = crate::session_record::current_terminal_id();
    let selected = inspection.resolve_pane(&resolution, command, caller.as_deref())?;
    let target = selected.target()?;
    let (action, json) = match command {
        DesktopCommand::Pin { .. }
        | DesktopCommand::Unpin { .. }
        | DesktopCommand::Rename { .. }
        | DesktopCommand::Arrange { .. }
        | DesktopCommand::Slots { .. }
        | DesktopCommand::Order { .. }
        | DesktopCommand::Layout { .. }
        | DesktopCommand::View { .. }
        | DesktopCommand::Query { .. }
        | DesktopCommand::State { .. } => unreachable!("handled above"),
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
        DesktopCommand::GraphLog { json, .. } => (
            DesktopPaneAction::GraphLog {
                task: bound(&resolution.task, "Task")?.into(),
            },
            json,
        ),
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
    let output = contact(
        Some(&serde_json::to_string(&DesktopPaneCommand {
            target,
            action,
        })?),
        false,
    )?;
    let receipt: DesktopPaneReceipt = serde_json::from_slice(&output)
        .context("Desktop returned an invalid receipt; inspect before retrying")?;
    if *json || cli.json {
        println!("{}", serde_json::to_string_pretty(&receipt)?);
    } else {
        println!("{}", receipt.pane);
    }
    Ok(())
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
        caller: Option<&str>,
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
            | DesktopCommand::GraphLog { pane, .. }
            | DesktopCommand::Zoom { pane, .. } => (pane, false, None),
            DesktopCommand::Pin { .. }
            | DesktopCommand::Unpin { .. }
            | DesktopCommand::Rename { .. }
            | DesktopCommand::Arrange { .. }
            | DesktopCommand::Slots { .. }
            | DesktopCommand::Order { .. }
            | DesktopCommand::Layout { .. }
            | DesktopCommand::View { .. }
            | DesktopCommand::Query { .. }
            | DesktopCommand::State { .. } => {
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
        let subject = match (&work.task, &work.wave) {
            (ContextFact::Bound { value, .. }, _) => Some((WorkWorkspaceKind::Task, value)),
            (ContextFact::Unbound, ContextFact::Bound { value, .. }) => {
                Some((WorkWorkspaceKind::Wave, value))
            }
            (ContextFact::Unbound, ContextFact::Unbound) => None,
            _ => bail!("Work identity is unavailable; no pane was changed"),
        };
        if matches!(command, DesktopCommand::Shell { .. })
            && !matches!(subject, Some((WorkWorkspaceKind::Task, _)))
        {
            bail!("Shells require an explicit Task; no shell was opened");
        }
        let mut candidates = Vec::new();
        for workspace in &window.workspaces {
            if workspace.work.repository != repository
                || subject.as_ref().is_some_and(|(kind, id)| {
                    &workspace.work.kind != kind || &workspace.work.id != *id
                })
            {
                continue;
            }
            for leaf in workspace.layout.leaves() {
                if leaf.pane.as_deref() == peer && peer.is_some() {
                    continue;
                }
                let exact_tab = pane.as_ref().and_then(|id| {
                    leaf.tabs
                        .iter()
                        .find(|tab| tab.id == id.strip_prefix("shell:").unwrap_or(id))
                });
                if pane
                    .as_ref()
                    .is_some_and(|id| leaf.pane.as_ref() != Some(id))
                    && exact_tab.is_none()
                {
                    continue;
                }
                if pane.is_none()
                    && matches!(command, DesktopCommand::Shell { .. })
                    && leaf.pane.as_deref() != Some(&workspace.focused_pane)
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
                if terminal && leaf.content.as_deref() == Some("shell") {
                    // A tool pane is not a terminal target. Enumerate retained shells,
                    // including background tabs, without consulting tab selection.
                    for tab in &leaf.tabs {
                        if exact_tab.is_some_and(|exact| exact.id != tab.id)
                            || tab.surface.is_none()
                        {
                            continue;
                        }
                        candidates.push(ResolvedPane {
                            window,
                            workspace,
                            leaf,
                            tab: Some(tab),
                        });
                    }
                } else {
                    if terminal
                        && (leaf.content.as_deref() != Some("session") || leaf.surface.is_none())
                    {
                        continue;
                    }
                    candidates.push(ResolvedPane {
                        window,
                        workspace,
                        leaf,
                        tab: exact_tab,
                    });
                }
            }
        }
        if pane.is_none() {
            if let Some(caller) = caller {
                let own = candidates.iter().filter(|candidate| {
                    let location = candidate.tab.map_or(&candidate.leaf.location, |tab| &tab.location);
                    matches!((&work.machine, location), (ContextFact::Bound { value, .. }, Some(location)) if value == &location.machine_id)
                        && candidate.tab.map_or_else(|| candidate.leaf.surface.as_deref() == Some(caller), |tab| tab.id == caller)
                }).collect::<Vec<_>>();
                if let [candidate] = own.as_slice() {
                    return Ok(**candidate);
                }
            }
        }
        match candidates.as_slice() {
            [candidate] => Ok(*candidate),
            [] => bail!(
                "No eligible pane{} in the selected Work. Inspect with `lf desktop state`; no pane was changed.",
                pane.as_ref().map(|id| format!(" {id}")).unwrap_or_default()
            ),
            _ => {
                let choices = candidates
                    .iter()
                    .map(|candidate| {
                        let leaf = candidate.leaf;
                        format!(
                            "{} ({})",
                            candidate.tab.map(|tab| tab.id.as_str()).or(leaf.pane.as_deref()).unwrap_or("unavailable"),
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
#[derive(Debug, Clone, Copy)]
struct ResolvedPane<'a> {
    window: &'a DesktopWindowInspection,
    workspace: &'a DesktopWorkspaceInspection,
    leaf: &'a DesktopLayoutInspection,
    tab: Option<&'a DesktopShellInspection>,
}

impl ResolvedPane<'_> {
    fn surface(&self) -> Result<String> {
        self.tab
            .map_or(&self.leaf.surface, |tab| &tab.surface)
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
            tab: None,
            ..*self
        })
    }

    fn target(&self) -> Result<DesktopPaneTarget> {
        Ok(DesktopPaneTarget {
            repository: self.window.repository.clone(),
            window: self.window.window.clone(),
            work: self.workspace.work.clone(),
            pane: self
                .leaf
                .pane
                .clone()
                .context("Pane identity unavailable")?,
            incarnation: self
                .tab
                .map(|tab| tab.incarnation.clone())
                .or_else(|| self.leaf.incarnation.clone())
                .context("Pane content identity unavailable")?,
            tab: self.tab.map(|tab| tab.id.clone()),
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
        // repeating shell creation allocates another tab; inspect before retrying.
        bail!("Desktop request failed: {}. Inspect again before retrying; `lf task status <task>` remains available in this terminal.", String::from_utf8_lossy(&output.stderr).trim());
    }
    Ok(output.stdout)
}

fn inspect(json: bool) -> Result<()> {
    let output = contact(None, false)?;
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
    use super::validate_literal_text;
    use crate::ops::context::{ContextExplanation, ContextFact};

    fn pane_fixture() -> (super::DesktopInspection, ContextExplanation) {
        let inspection: super::DesktopInspection = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/desktop_inspection.json"
        ))
        .unwrap();
        let window = &inspection.windows[0];
        let fact = |value: &str| ContextFact::Bound {
            value: value.into(),
            source: "selected_work".into(),
        };
        let mut work = ContextExplanation::empty();
        work.repository = fact(&window.repository);
        work.task = fact(window.task.as_ref().unwrap().id.as_str());
        work.checkout = fact("/repo/task");
        work.execution_machine = fact(
            &window.workspaces[0].layout.children[0]
                .location
                .as_ref()
                .unwrap()
                .machine_id,
        );
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
        let selected = inspection.resolve_pane(&work, &text, None).unwrap();
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
            .resolve_pane(&work, &text, None)
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
        let explicit_selection = inspection.resolve_pane(&work, &explicit, None).unwrap();
        assert_eq!(explicit_selection.target().unwrap(), target);
        assert_eq!(explicit_selection.surface().unwrap(), surface);
        // The already resolved transport request retains the old lifetime, not
        // the replacement or the newly focused surface. Desktop validates it.
        inspection.windows[0].workspaces[0].layout.children[0].incarnation =
            Some("replacement".into());
        inspection.windows[0].workspaces[0].layout.children[0].surface =
            Some("replacement-surface".into());
        let replacement = inspection.resolve_pane(&work, &explicit, None).unwrap();
        assert_ne!(replacement.target().unwrap(), target);
        assert_ne!(replacement.surface().unwrap(), surface);
        assert_eq!(target.incarnation, "occurrence-session-pane");
    }

    #[test]
    fn caller_pane_stays_with_its_machine_and_explicit_selectors_win() {
        use crate::lf::DesktopCommand;
        let (mut inspection, mut work) = pane_fixture();
        work.machine = work.execution_machine.clone();
        let workspace = &mut inspection.windows[0].workspaces[0];
        let mut shell = workspace.layout.children[0].clone();
        shell.pane = Some("caller-shell".into());
        shell.content = Some("shell".into());
        shell.surface = Some("shell-surface".into());
        shell.tabs = vec![super::DesktopShellInspection {
            id: "caller-shell".into(),
            incarnation: "shell-tab-occurrence".into(),
            surface: shell.surface.clone(),
            location: shell.location.clone(),
        }];
        shell.selected_tab = Some("caller-shell".into());
        workspace.layout.children.push(shell);
        workspace.focused_pane = "caller-shell".into();
        let text = DesktopCommand::Text {
            pane: None,
            text: "draft".into(),
            json: true,
        };
        let selected = inspection
            .resolve_pane(&work, &text, Some("native-surface-incarnation"))
            .unwrap();
        assert_eq!(selected.target().unwrap().pane, "session-pane");
        let selected = inspection
            .resolve_pane(&work, &text, Some("caller-shell"))
            .unwrap();
        assert_eq!(selected.target().unwrap().pane, "caller-shell");
        let explicit = DesktopCommand::Text {
            pane: Some("session-pane".into()),
            text: "draft".into(),
            json: true,
        };
        assert_eq!(
            inspection
                .resolve_pane(&work, &explicit, Some("caller-shell"))
                .unwrap()
                .target()
                .unwrap()
                .pane,
            "session-pane"
        );
        // A marker from another Machine or a replaced Session cannot resolve
        // ambiguous panes, even when one of them has focus.
        assert!(inspection
            .resolve_pane(&work, &text, Some("old-surface"))
            .is_err());
        work.machine = crate::ops::context::ContextFact::Bound {
            value: "other-machine".into(),
            source: "observer".into(),
        };
        assert!(inspection
            .resolve_pane(&work, &text, Some("caller-shell"))
            .is_err());
    }

    #[test]
    fn shell_targets_ignore_selected_tabs_and_creation_requires_a_task() {
        use crate::lf::DesktopCommand;
        let (mut inspection, mut work) = pane_fixture();
        let shell = &mut inspection.windows[0].workspaces[0].layout.children[0];
        shell.content = Some("shell".into());
        shell.tabs = ["first", "second"]
            .into_iter()
            .map(|id| super::DesktopShellInspection {
                id: id.into(),
                incarnation: format!("{id}-occurrence"),
                surface: Some(format!("{id}-surface")),
                location: shell.location.clone(),
            })
            .collect();
        shell.selected_tab = Some("second".into());
        let command = DesktopCommand::Text {
            pane: Some("shell:first".into()),
            text: "draft".into(),
            json: true,
        };
        let selected = inspection.resolve_pane(&work, &command, None).unwrap();
        assert_eq!(selected.target().unwrap().tab.as_deref(), Some("first"));
        assert_eq!(selected.target().unwrap().incarnation, "first-occurrence");
        assert_eq!(selected.surface().unwrap(), "first-surface");
        let create = DesktopCommand::Shell {
            pane: None,
            json: true,
        };
        assert!(inspection.resolve_pane(&work, &create, None).is_ok());
        work.task = ContextFact::Unbound;
        assert!(inspection.resolve_pane(&work, &create, None).is_err());
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
            .resolve_pane(&work, &focus, None)
            .unwrap_err()
            .to_string()
            .contains("Multiple eligible"));
        let missing = DesktopCommand::Focus {
            pane: Some("absent".into()),
            json: false,
        };
        assert!(inspection
            .resolve_pane(&work, &missing, None)
            .unwrap_err()
            .to_string()
            .contains("No eligible pane"));
        let files = DesktopCommand::Text {
            pane: Some("files-pane".into()),
            text: "no input".into(),
            json: false,
        };
        assert!(inspection.resolve_pane(&work, &files, None).is_err());
        let restore = DesktopCommand::Restore {
            pane: None,
            json: false,
        };
        assert_eq!(
            inspection
                .resolve_pane(&work, &restore, None)
                .unwrap()
                .target()
                .unwrap()
                .pane,
            "files-pane"
        );
        // Retained Work is addressable with no current execution-location reading.
        work.checkout = ContextFact::Unavailable {
            reason: "owner offline".into(),
        };
        assert!(inspection.resolve_pane(&work, &restore, None).is_ok());
        work.task = ContextFact::Unavailable {
            reason: "Task identity unavailable".into(),
        };
        assert!(inspection.resolve_pane(&work, &restore, None).is_err());
    }

    #[test]
    fn pane_destinations_are_plain_selectors_in_the_same_work() {
        use crate::lf::DesktopCommand;
        let (mut inspection, work) = pane_fixture();
        let mut other = inspection.windows[0].workspaces[0].clone();
        other.work.id = "other-task".into();
        other.layout.children[1].pane = Some("other-workspace".into());
        inspection.windows[0].workspaces.push(other);
        let command = DesktopCommand::Move {
            pane: None,
            destination: "files-pane".into(),
            axis: super::DesktopSplitAxis::Vertical,
            json: false,
        };
        let selected = inspection.resolve_pane(&work, &command, None).unwrap();
        let target = selected.target().unwrap();
        assert_eq!(target.pane, "session-pane");
        let destination = selected.peer("files-pane").unwrap().target().unwrap();
        assert_eq!(destination.incarnation, "occurrence-files-pane");
        assert_eq!(destination.window, target.window);
        assert_eq!(destination.work, target.work);
        assert!(selected.peer("session-pane").is_err());
        assert!(selected.peer("other-workspace").is_err());
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
