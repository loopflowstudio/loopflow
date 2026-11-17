//! Inspect configured planning and recover personal Git identity.

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::lf::RepoPlanCommand as PlanCommand;
use loopflow_execution::store::planning_sync::{ActionOwnerKind, TrackerActionStatus, TrackerKind};
use loopflow_execution::store::sqlite::tracker_actions::ActionPhase;
use loopflow_execution::store::Store;

/// `lf repo plan [key|journal]`; bare, the planning status.
pub fn run(cmd: Option<&PlanCommand>, json: bool, subscribe: bool) -> Result<()> {
    let repo = status_repository(&std::env::current_dir()?)?;
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

/// Whether the repository containing `checkout` plans through a Git ref.
pub fn plans_through_git(checkout: &Path) -> Result<bool> {
    let repo = loopflow_execution::git::worktrees::main_repo_root(checkout)?;
    let config = loopflow_execution::config::load_config(Some(&repo))?.unwrap_or_default();
    Ok(matches!(
        config.planning_transport(),
        loopflow_execution::config::PlanningConfig::Git { .. }
    ))
}

/// Exchange the repository's Git planning ref now. The exchange is per
/// repository, so a Wave selection has no effect.
pub fn refresh(checkout: &Path) -> Result<()> {
    let repo = loopflow_execution::git::worktrees::main_repo_root(checkout)?;
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_store().await?;
        let repo_key = repo.to_string_lossy();
        // Acquisition records its own failure and never waits on publication.
        // A refresh waits out another process's publication, then sends this
        // Machine's pending changes out.
        let acquired =
            loopflow_execution::ops::planning_sync::acquire_repository(&store, &repo_key).await;
        loopflow_execution::ops::planning_sync::publish_repository_now(&store, &repo_key).await?;
        acquired?;
        let destinations =
            loopflow_execution::ops::planning_sync::peer_status(&store, &repo_key).await?;
        let Some(active) = destinations.into_iter().find(|d| d.active) else {
            return Err(anyhow!(
                "no Git planning destination is configured; see `lf config planning status`"
            ));
        };
        if let Some(error) = active.acquisition_error {
            return Err(anyhow!("planning refresh failed: {error}"));
        }
        Ok(())
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

async fn run_async(repo: &Path, cmd: Option<&PlanCommand>, status_json: bool) -> Result<()> {
    let store = open_store().await?;
    let repo_key = repo.to_string_lossy();
    match cmd {
        Some(PlanCommand::Key { new, recover }) => {
            let saved = store.planning_user_key().await?;
            let key = match (recover, saved, new) {
                (Some(key), _, _) => key.clone(),
                (None, Some(key), _) => key,
                (None, None, true) => uuid::Uuid::new_v4().to_string(),
                _ => return Err(anyhow!(
                    "No planning user key. Use `lf repo plan key --new` once, then recover that key on other machines with `--recover <UUID>`."
                )),
            };
            if *new || recover.is_some() {
                store.provision_planning_user_key(&key).await?;
            }
            println!("{key}");
        }
        None => print_status(&store, &repo_key, status_json).await?,
        Some(PlanCommand::Journal { json }) => print_journal(&store, &repo_key, *json).await?,
    }
    Ok(())
}

/// `lf repo plan --subscribe`: `lf repo plan --json` now and each time
/// it changes. The subscription reads stored status; `lf wave status --sync` and
/// foreground providers exchange planning.
pub(crate) fn subscribe_status(
    repo: &Path,
    transport: loopflow_execution::read_stream::Transport,
) -> Result<Option<loopflow_execution::read_stream::Subscription>> {
    let repo_key = repo.to_string_lossy().into_owned();
    loopflow_execution::read_stream::start(
        (),
        move |store, ()| {
            let repo_key = repo_key.clone();
            async move { Ok(status(store.as_deref(), &repo_key).await?.report()) }
        },
        transport,
    )
}

/// What `repo.plan` answers: the tracker, for Git its destinations with
/// their import and publication evidence, and every open and lost tracker
/// action.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(crate) struct PlanStatus {
    /// `git`, `linear` or `asana`.
    tracker: String,
    destinations: Vec<loopflow_execution::store::PeerPlanningStatus>,
    /// What the tracker has yet to confirm (open: `queued`, `sent`), in send
    /// order, and each value it kept over a local one (lost: `conflicted`)
    /// until it is answered. On Git, only lost values: Git keeps no queue.
    actions: Vec<TrackerActionStatus>,
}

/// Where a repository's planning is exchanged.
enum Status {
    /// The tracker its config names, and its open and lost actions.
    Remote(TrackerKind, Vec<TrackerActionStatus>),
    /// Git's destinations, and each value its journal lost.
    Git(
        Vec<loopflow_execution::store::PeerPlanningStatus>,
        Vec<TrackerActionStatus>,
    ),
}

impl Status {
    /// What `lf repo plan --json` prints.
    fn report(self) -> PlanStatus {
        match self {
            Self::Remote(tracker, actions) => PlanStatus {
                tracker: tracker.as_str().to_owned(),
                destinations: Vec::new(),
                actions,
            },
            Self::Git(destinations, actions) => PlanStatus {
                tracker: TrackerKind::Git.as_str().to_owned(),
                destinations,
                actions,
            },
        }
    }
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
        .remote_name()
        .map(|name| TrackerKind::parse(&name.to_ascii_lowercase()))
        .transpose()?)
}

/// What `lf --cwd <directory> config planning status --json` prints.
pub(crate) fn read_status(directory: &Path) -> Result<PlanStatus> {
    let repo = status_repository(directory)?;
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_store().await?;
        Ok(status(Some(&store), &repo.to_string_lossy())
            .await?
            .report())
    })
}

/// Stored status only: an exchange binds the configured destination.
async fn status(store: Option<&Store>, repo: &str) -> Result<Status> {
    let tracker = remote_transport(repo)?;
    let actions = match store {
        Some(store) => store
            .sqlite
            .repository_tracker_actions(repo, tracker.unwrap_or(TrackerKind::Git))?,
        None => Vec::new(),
    };
    if let Some(tracker) = tracker {
        return Ok(Status::Remote(tracker, actions));
    }
    Ok(Status::Git(
        match store {
            Some(store) => loopflow_execution::ops::planning_sync::peer_status(store, repo).await?,
            None => Vec::new(),
        },
        actions,
    ))
}

/// The command binds the configured destination first, so a changed
/// selection shows at once; the in-app read and the subscription show stored status.
async fn print_status(store: &Store, repo: &str, json: bool) -> Result<()> {
    loopflow_execution::ops::planning_sync::configured_destination(store, repo).await?;
    let status = status(Some(store), repo).await?;
    if json {
        println!(
            "{}",
            serde_json::to_string(&loopflow_execution::read_stream::selected_answer(
                &status.report()
            )?)?
        );
        return Ok(());
    }
    let (destinations, lost) = match status {
        Status::Remote(tracker, actions) => {
            let open = actions
                .iter()
                .filter(|action| action.phase != ActionPhase::Conflicted)
                .count();
            let lost = actions.len() - open;
            if actions.is_empty() {
                println!("Tracker: {}. Nothing open.", tracker.as_str());
                return Ok(());
            }
            println!("Tracker: {}. {open} open · {lost} lost.", tracker.as_str());
            // Open lines in send order, then lost ones.
            let mut actions = actions;
            actions.sort_by_key(|action| action.phase == ActionPhase::Conflicted);
            print_actions(store, &actions)?;
            return Ok(());
        }
        Status::Git(destinations, lost) => (destinations, lost),
    };
    if !destinations.iter().any(|d| d.active) {
        println!("Tracker: git (configured remote; local saves wait when unavailable)");
    }
    for destination in destinations {
        println!(
            "{}  {}{}  {} records",
            destination.id,
            destination.reference,
            if destination.active {
                " (configured)"
            } else {
                ""
            },
            destination.selected_records
        );
        println!(
            "  Retained import: {}",
            destination.imported_revision.as_deref().unwrap_or("none")
        );
        println!(
            "  Fetched: {}",
            destination.fetched_revision.as_deref().unwrap_or("none")
        );
        println!(
            "  Publication: {} ({}){}",
            destination
                .publication_state
                .as_deref()
                .unwrap_or("not attempted"),
            destination
                .publication_revision
                .as_deref()
                .unwrap_or("none"),
            if destination.pending_local {
                "; local changes pending"
            } else {
                ""
            }
        );
        for error in [destination.acquisition_error, destination.publication_error]
            .into_iter()
            .flatten()
        {
            println!("  {error}");
        }
        for conflict in destination.conflicts {
            println!("  Held {}: {}", conflict.target, conflict.reason);
        }
    }
    println!("Import retention is not publication; `lf planning journal` lists retained values.");
    if !lost.is_empty() {
        println!("Tracker: git. {} lost.", lost.len());
        print_actions(store, &lost)?;
    }
    Ok(())
}

/// One line per action under its owner's name, and how to answer a lost one.
fn print_actions(store: &Store, actions: &[TrackerActionStatus]) -> Result<()> {
    let owners = actions
        .iter()
        .map(|action| owner_name(store, action))
        .collect::<Result<Vec<_>>>()?;
    let width = owners.iter().map(String::len).max().unwrap_or(0);
    for (owner, action) in owners.iter().zip(actions) {
        println!("  {owner:width$}  {}", action.line());
    }
    if actions
        .iter()
        .any(|action| action.phase == ActionPhase::Conflicted)
    {
        println!("Answer a lost line: lf task sync <task> --keep-theirs, or --use-mine.");
    }
    Ok(())
}

async fn print_journal(store: &Store, repo: &str, json: bool) -> Result<()> {
    let journals = loopflow_execution::ops::planning_sync::peer_journal(store, repo).await?;
    if json {
        println!("{}", serde_json::to_string(&journals)?);
        return Ok(());
    }
    for journal in journals {
        println!("{}", journal.destination);
        for (id, change) in journal.changes {
            println!(
                "  {} {} [{}; {}]: {}",
                change.edit.target,
                change.edit.change.field(),
                id,
                if journal.candidates.contains(&id) {
                    "candidate, not confirmed"
                } else {
                    "retained alternative"
                },
                change.edit.change.value()?
            );
        }
    }
    println!("Copy a retained value into an ordinary edit to save it again.");
    Ok(())
}
