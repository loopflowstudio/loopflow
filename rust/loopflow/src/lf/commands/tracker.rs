//! How the repository's plan travels and what is still unsent.

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
/// exchange by each carrier its plan has.
pub(super) fn refresh(repo: &Path) -> Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_store().await?;
        loopflow_execution::ops::plan_carriers::sync(&store, &repo.to_string_lossy()).await
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
    if cmd.is_none() {
        loopflow_execution::ops::plan_carriers::before_first_read(repo).await;
    }
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
/// it changes. It reads the store only; the store is opened read-only.
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
            async move { status(store.as_deref(), &repo_key) }
        },
        transport,
    )
}

/// What `repo.tracker` answers: the tracker, or for a repository without
/// one how its plan travels, and every open and lost tracker action.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(crate) struct TrackerStatus {
    /// `linear` or `asana`; `null` when the plan is in Loopflow's store.
    tracker: Option<String>,
    /// How a plan without a tracker travels; `null` with a tracker.
    plan: Option<PlanCarriers>,
    /// What the tracker has yet to confirm (open: `queued`, `sent`), in send
    /// order, and each value it kept over a local one (lost: `conflicted`)
    /// until it is answered.
    actions: Vec<TrackerActionStatus>,
}

/// The ways a plan without a tracker reaches other Machines. Every Machine
/// holds the whole plan; with none of these it stays on this one.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(crate) struct PlanCarriers {
    /// The repository's remote; `null` without one.
    git: Option<GitCarrier>,
    /// The Machine `lf machine hub` names; `null` without one.
    hub: Option<HubCarrier>,
}

/// Git's line: `Plan: Git origin, failing since Nov 18 14:02, 3 unsent.`
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(crate) struct GitCarrier {
    /// The remote's name.
    remote: String,
    #[serde(flatten)]
    standing: CarrierStanding,
}

/// The hub's line: `Hub mini, failing since Nov 18 14:02, 2 unsent.`
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(crate) struct HubCarrier {
    /// The hub's name.
    machine: String,
    #[serde(flatten)]
    standing: CarrierStanding,
}

/// What a carrier has yet to take, and why when it cannot.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(crate) struct CarrierStanding {
    /// How many records changed here the carrier has not taken.
    unsent: usize,
    /// The first of them in change order: a Task's identifier, `wave <name>`,
    /// `project <name>` or `comment <id>`.
    records: Vec<String>,
    /// Why the last exchange failed, while it still fails.
    error: Option<String>,
    /// When the carrier stopped working (Unix seconds), while it still does not.
    failing_since: Option<i64>,
    /// What the carrier's last read brought that this Machine could not
    /// write, and reads again at each exchange. `null` when it took
    /// everything.
    not_taken: Option<NotTaken>,
}

#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(crate) struct NotTaken {
    /// How many records.
    count: usize,
    /// Why the first was not, and what to do about it when something can be
    /// done: `wave design from mini has the name of a Wave here; …`.
    reason: String,
}

/// How many unsent records a line names.
const NAMED: usize = 20;

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
        plan: Some(plan_carriers(store, repo)?),
        actions: Vec::new(),
    })
}

fn plan_carriers(store: &Store, repo: &str) -> Result<PlanCarriers> {
    use loopflow_execution::store::sqlite::plan_carriers::Carrier;
    let standing = |carrier| -> Result<CarrierStanding> {
        let held = store.sqlite.plan_carrier(repo, carrier)?;
        let unsent = store.sqlite.plan_unsent(repo, carrier, NAMED)?;
        let failing_since = held.as_ref().and_then(|held| held.failing_since);
        let not_taken = held
            .as_ref()
            .and_then(|held| held.not_taken.clone())
            .map(|not_taken| NotTaken {
                count: not_taken.count,
                reason: not_taken.reason,
            });
        Ok(CarrierStanding {
            unsent: unsent.count,
            records: unsent
                .first
                .iter()
                .map(|(table, id)| record_name(store, table, id))
                .collect::<Result<_>>()?,
            error: failing_since.and(held.and_then(|held| held.error)),
            failing_since,
            not_taken,
        })
    };
    Ok(PlanCarriers {
        git: loopflow_execution::ops::plan_git::remote(Path::new(repo))
            .map(|remote| {
                Ok::<_, anyhow::Error>(GitCarrier {
                    remote,
                    standing: standing(Carrier::Git)?,
                })
            })
            .transpose()?,
        hub: loopflow_execution::ops::plan_hub::hub(&store.sqlite)?
            .map(|hub| {
                Ok::<_, anyhow::Error>(HubCarrier {
                    machine: hub.name().to_owned(),
                    standing: standing(Carrier::Hub)?,
                })
            })
            .transpose()?,
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

/// One line per carrier, `Plan: Git origin.` and `Hub mini.`, each with its
/// unsent records by name, why it did not take them, and what it brought
/// that could not be written here; `Plan: this Machine.` for a repository
/// with no remote.
fn print_plan(plan: &PlanCarriers) {
    match &plan.git {
        Some(git) => print_carrier(format!("Plan: Git {}", git.remote), &git.standing),
        None => println!("Plan: this Machine."),
    }
    if let Some(hub) = &plan.hub {
        print_carrier(format!("Hub {}", hub.machine), &hub.standing);
    }
}

fn print_carrier(mut line: String, standing: &CarrierStanding) {
    if let Some(since) = standing.failing_since {
        line.push_str(&format!(
            ", failing since {}",
            loopflow_execution::output::format_time(since)
        ));
    }
    if standing.unsent > 0 {
        line.push_str(&format!(", {} unsent", standing.unsent));
    }
    if let Some(not_taken) = &standing.not_taken {
        line.push_str(&format!(", {} not taken", not_taken.count));
    }
    println!("{line}.");
    for record in &standing.records {
        println!("  {record}");
    }
    if standing.unsent > standing.records.len() {
        println!("  and {} more", standing.unsent - standing.records.len());
    }
    if let Some(error) = &standing.error {
        println!("  {error}");
    }
    if let Some(not_taken) = &standing.not_taken {
        println!("  {}", not_taken.reason);
    }
}

/// A Task's identifier, `wave <name>`, `project <name>` or `comment <id>`.
fn record_name(store: &Store, table: &str, id: &str) -> Result<String> {
    Ok(match table {
        "tasks" => store
            .sqlite
            .task(&loopflow_model::durable::TaskId::from_raw(id))?
            .map_or_else(|| id.to_owned(), |task| task.identifier),
        "waves" => format!(
            "wave {}",
            store
                .sqlite
                .get_wave(&loopflow_model::id::WaveId::parse(id)?)?
                .map_or_else(|| id.to_owned(), |wave| wave.name().to_owned())
        ),
        "projects" => format!(
            "project {}",
            store
                .sqlite
                .project(&loopflow_model::durable::ProjectId::from_raw(id))?
                .map_or_else(|| id.to_owned(), |project| project.name)
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
