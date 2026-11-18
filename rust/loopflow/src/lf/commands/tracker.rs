//! Where the repository's plan lives and what is still on its way there.

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::lf::RepoTrackerCommand;
use loopflow_execution::store::planning_sync::{
    ActionOwnerKind, LostAnswer, TrackerActionStatus, TrackerKind,
};
use loopflow_execution::store::sqlite::tracker_actions::ActionPhase;
use loopflow_execution::store::Store;

/// `lf repo tracker [answer]`; bare, the planning status.
pub fn run(cmd: Option<&RepoTrackerCommand>, json: bool, subscribe: bool) -> Result<()> {
    let directory = std::env::current_dir()?;
    if let Some(RepoTrackerCommand::Answer {
        id,
        keep_theirs,
        json,
        ..
    }) = cmd
    {
        return answer(&directory, id, *keep_theirs, *json);
    }
    let repo = status_repository(&directory)?;
    if cmd.is_none() && subscribe {
        return subscribe_status(&repo, loopflow_execution::read_stream::Transport::Stdio)
            .map(drop);
    }
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run_async(&repo, cmd, json))
}

/// The repository whose planning `directory` reads. A Work folder without Git
/// still has stored planning status.
pub(crate) fn status_repository(directory: &Path) -> Result<std::path::PathBuf> {
    Ok(
        match loopflow_execution::repo::discover_repo_root(directory)
            .ok()
            .flatten()
        {
            Some(root) => loopflow_execution::git::worktrees::main_repo_root(&root)?,
            None => directory.to_owned(),
        },
    )
}

/// `lf wave status --sync` in a repository without Linear or Asana: one
/// exchange with this Machine's hub, when it has one.
pub fn refresh(checkout: &Path) -> Result<()> {
    let repo = loopflow_execution::git::worktrees::main_repo_root(checkout)?;
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_store().await?;
        loopflow_execution::ops::plan_hub::sync(&store, &repo.to_string_lossy(), false).await
    })
}

async fn open_store() -> Result<Store> {
    loopflow_execution::store::open_existing_store()
        .await
        .map_err(|error| anyhow!("Machine registry cannot be opened: {error}"))?
        .ok_or_else(|| {
            anyhow!(
                "Machine registry is missing at {}; initialize or restore it before planning setup",
                loopflow_execution::store::default_db_path().display()
            )
        })
}

async fn run_async(repo: &Path, cmd: Option<&RepoTrackerCommand>, status_json: bool) -> Result<()> {
    let store = open_store().await?;
    let repo_key = repo.to_string_lossy();
    match cmd {
        None => print_status(&store, &repo_key, status_json).await?,
        Some(RepoTrackerCommand::Answer { .. }) => {
            unreachable!("answering runs before the status read")
        }
    }
    Ok(())
}

/// `lf repo tracker answer <id> --keep-theirs | --use-mine`.
fn answer(directory: &Path, id: &str, keep_theirs: bool, json: bool) -> Result<()> {
    let answer = if keep_theirs {
        LostAnswer::KeepTheirs
    } else {
        LostAnswer::UseMine
    };
    let line = loopflow_execution::ops::planning_sync::answer_lost(directory, id, answer)?;
    if json {
        println!("{}", serde_json::to_string(&line)?);
    } else if answer == LostAnswer::KeepTheirs {
        println!("Kept {}'s {}.", line.tracker.display_name(), line.change);
    } else {
        println!("Saved your {} again.", line.change);
    }
    Ok(())
}

/// `lf repo tracker --subscribe`: `lf repo tracker --json` now and each time
/// it changes. With a hub it also exchanges with the hub while it runs.
pub(crate) fn subscribe_status(
    repo: &Path,
    transport: loopflow_execution::read_stream::Transport,
) -> Result<Option<loopflow_execution::read_stream::Subscription>> {
    let repo_key = repo.to_string_lossy().into_owned();
    loopflow_execution::read_stream::start(
        loopflow_execution::read_stream::Watch::State,
        (),
        move |store, ()| {
            let repo_key = repo_key.clone();
            async move {
                if let Some(store) = store.as_deref() {
                    if let Err(error) =
                        loopflow_execution::ops::plan_hub::sync(store, &repo_key, false).await
                    {
                        tracing::debug!(error = %format!("{error:#}"), "hub exchange pending");
                    }
                }
                status(store.as_deref(), &repo_key)
            }
        },
        transport,
    )
}

/// What `repo.tracker` answers: the tracker, or for a repository without
/// one where its plan lives, and every open and lost tracker action.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(crate) struct TrackerStatus {
    /// `linear` or `asana`; `null` when the plan is in Loopflow's store.
    tracker: Option<String>,
    /// Where a plan without a tracker lives; `null` with a tracker.
    plan: Option<PlanHome>,
    /// What the tracker has yet to confirm (open: `queued`, `sent`), in send
    /// order, and each value it kept over a local one (lost: `conflicted`)
    /// until it is answered.
    actions: Vec<TrackerActionStatus>,
}

/// The Machine that holds a plan without a tracker.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(crate) struct PlanHome {
    /// The hub's name when the plan is on another Machine; `null` when it is
    /// on this one.
    hub: Option<String>,
    /// When the hub stopped answering (Unix seconds), while it still does not.
    unreachable_since: Option<i64>,
    /// Why the last exchange with the hub failed.
    error: Option<String>,
    /// Each record with values the hub has yet to confirm, in send order.
    open: Vec<HubOpen>,
}

/// One record's values on their way to the hub: `3e1c9a07  title · queued`.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(crate) struct HubOpen {
    /// A Task's identifier, `wave <name>`, `project <name>` or `comment <id>`.
    owner: String,
    /// The open fields without their record prefix: `title`, `title, notes`,
    /// or `14 values` for a record the hub has yet to see.
    field: String,
    /// `queued`, or `sent` while every answer is outstanding.
    phase: String,
}

/// The name a person reads for an action's owner: a Task's identifier, a
/// Project's name.
fn owner_name(store: &Store, action: &TrackerActionStatus) -> Result<String> {
    let id = &action.owner_id;
    Ok(match action.owner_kind {
        ActionOwnerKind::Task => store
            .sqlite
            .task(&loopflow_model::durable::TaskId::from_raw(id))?
            .map_or_else(|| id.clone(), |task| task.identifier),
        ActionOwnerKind::Project => store
            .sqlite
            .project(&loopflow_model::durable::ProjectId::from_raw(id))?
            .map_or_else(|| id.clone(), |project| project.name),
        _ => id.clone(),
    })
}

fn remote_transport(repo: &str) -> Result<Option<TrackerKind>> {
    let config =
        loopflow_execution::config::load_config(Some(Path::new(repo)))?.unwrap_or_default();
    Ok(config
        .planning_transport()
        .map(|tracker| TrackerKind::parse(&tracker.name().to_ascii_lowercase()))
        .transpose()?)
}

/// What `lf --cwd <directory> repo tracker --json` prints.
pub(crate) fn read_status(directory: &Path) -> Result<TrackerStatus> {
    let repo = status_repository(directory)?;
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_store().await?;
        status(Some(&store), &repo.to_string_lossy())
    })
}

/// Stored status only; exchanges happen around commands.
fn status(store: Option<&Store>, repo: &str) -> Result<TrackerStatus> {
    let tracker = remote_transport(repo)?;
    let Some(store) = store else {
        return Ok(TrackerStatus {
            tracker: tracker.map(|tracker| tracker.as_str().to_owned()),
            plan: None,
            actions: Vec::new(),
        });
    };
    if let Some(tracker) = tracker {
        return Ok(TrackerStatus {
            tracker: Some(tracker.as_str().to_owned()),
            plan: None,
            actions: store.sqlite.repository_tracker_actions(repo)?,
        });
    }
    Ok(TrackerStatus {
        tracker: None,
        plan: Some(plan_home(store, repo)?),
        actions: Vec::new(),
    })
}

fn plan_home(store: &Store, repo: &str) -> Result<PlanHome> {
    let Some(hub) = loopflow_execution::ops::plan_hub::hub(&store.sqlite)? else {
        return Ok(PlanHome {
            hub: None,
            unreachable_since: None,
            error: None,
            open: Vec::new(),
        });
    };
    let plan = store.sqlite.hub_plan(repo)?;
    let failing = plan.as_ref().and_then(|plan| plan.failing_since);
    let sends = store.sqlite.open_hub_sends(repo)?;
    let open = loopflow_execution::ops::plan_hub::open_by_owner(&sends)
        .into_iter()
        .map(|(send, field, phase)| {
            Ok(HubOpen {
                owner: hub_owner_name(store, send)?,
                field,
                phase: phase.to_owned(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(PlanHome {
        hub: Some(hub.name().to_owned()),
        unreachable_since: failing,
        error: failing.and(plan.and_then(|plan| plan.error)),
        open,
    })
}

async fn print_status(store: &Store, repo: &str, json: bool) -> Result<()> {
    let status = status(Some(store), repo)?;
    if json {
        println!(
            "{}",
            serde_json::to_string(&loopflow_execution::read_stream::selected_answer(&status)?)?
        );
        return Ok(());
    }
    if let Some(plan) = status.plan {
        print_plan(&plan);
        return Ok(());
    }
    let tracker = status.tracker.unwrap_or_default();
    let actions = status.actions;
    let open = actions
        .iter()
        .filter(|action| action.phase != ActionPhase::Conflicted)
        .count();
    let lost = actions.len() - open;
    if actions.is_empty() {
        println!("Tracker: {tracker}. Nothing open.");
        return Ok(());
    }
    println!("Tracker: {tracker}. {open} open · {lost} lost.");
    // Open lines in send order, then lost ones.
    let mut actions = actions;
    actions.sort_by_key(|action| action.phase == ActionPhase::Conflicted);
    print_actions(store, &actions)
}

/// `Plan: hub mini. 2 open.`, then each record with open values, then why
/// the hub did not take them.
fn print_plan(plan: &PlanHome) {
    let Some(hub) = &plan.hub else {
        println!("Plan: this Machine.");
        return;
    };
    let place = match plan.unreachable_since {
        Some(since) => format!(
            "hub {hub}, unreachable since {}",
            loopflow_execution::output::format_time(since)
        ),
        None => format!("hub {hub}"),
    };
    if plan.open.is_empty() {
        println!("Plan: {place}. Nothing open.");
        if let Some(error) = &plan.error {
            println!("  {error}");
        }
        return;
    }
    println!("Plan: {place}. {} open.", plan.open.len());
    let width = plan
        .open
        .iter()
        .map(|open| open.owner.len())
        .max()
        .unwrap_or(0);
    for open in &plan.open {
        println!("  {:width$}  {} · {}", open.owner, open.field, open.phase);
    }
    if let Some(error) = &plan.error {
        println!("  {error}");
    }
}

/// A Task's identifier, `wave <name>`, `project <name>` or `comment <id>`.
fn hub_owner_name(
    store: &Store,
    send: &loopflow_execution::store::sqlite::plan_hub::HubSend,
) -> Result<String> {
    let id = &send.edit.target;
    Ok(match send.owner_table.as_str() {
        "tasks" => store
            .sqlite
            .task(&loopflow_model::durable::TaskId::from_raw(id))?
            .map_or_else(|| id.clone(), |task| task.identifier),
        "waves" => format!(
            "wave {}",
            store
                .sqlite
                .get_wave(&loopflow_model::id::WaveId::parse(id)?)?
                .map_or_else(|| id.clone(), |wave| wave.name().to_owned())
        ),
        "projects" => format!(
            "project {}",
            store
                .sqlite
                .project(&loopflow_model::durable::ProjectId::from_raw(id))?
                .map_or_else(|| id.clone(), |project| project.name)
        ),
        _ => format!("comment {}", short_id(id)),
    })
}

/// One line per action under its owner's name, and how to answer a lost one.
fn print_actions(store: &Store, actions: &[TrackerActionStatus]) -> Result<()> {
    let owners = actions
        .iter()
        .map(|action| owner_name(store, action))
        .collect::<Result<Vec<_>>>()?;
    let width = owners.iter().map(String::len).max().unwrap_or(0);
    for (owner, action) in owners.iter().zip(actions) {
        let id = match action.phase {
            ActionPhase::Conflicted => short_id(&action.id),
            _ => "",
        };
        println!("  {owner:width$}  {id:8}  {}", action.line());
    }
    if actions
        .iter()
        .any(|action| action.phase == ActionPhase::Conflicted)
    {
        println!("Answer a lost line: lf repo tracker answer <id> --keep-theirs, or --use-mine.");
    }
    Ok(())
}

/// The prefix `lf repo tracker answer` accepts for a line.
fn short_id(id: &str) -> &str {
    id.get(..8).unwrap_or(id)
}
