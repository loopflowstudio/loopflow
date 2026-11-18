//! Inspect configured planning and recover personal Git identity.

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::lf::RepoTrackerCommand;
use loopflow_execution::store::planning_sync::{
    ActionOwnerKind, LostAnswer, TrackerActionStatus, TrackerKind,
};
use loopflow_execution::store::sqlite::tracker_actions::ActionPhase;
use loopflow_execution::store::Store;

/// `lf repo tracker [key|journal]`; bare, the planning status.
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

/// Whether the repository containing `checkout` plans through a Git ref.
pub fn plans_through_git(checkout: &Path) -> Result<bool> {
    let repo = loopflow_execution::git::worktrees::main_repo_root(checkout)?;
    let config = loopflow_execution::config::load_config(Some(&repo))?.unwrap_or_default();
    Ok(matches!(
        config.planning_transport(),
        loopflow_execution::config::TrackerConfig::Git { .. }
    ))
}

/// Exchange the repository's Git planning ref now. The exchange is per
/// repository, so a Wave selection has no effect.
pub fn refresh(checkout: &Path) -> Result<()> {
    let repo = loopflow_execution::git::worktrees::main_repo_root(checkout)?;
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_store().await?;
        let repo_key = repo.to_string_lossy();
        // Refresh records its own failure and never waits on publication.
        // A refresh waits out another process's publication, then sends this
        // Machine's pending changes out.
        let fetched =
            loopflow_execution::ops::planning_sync::fetch_repository(&store, &repo_key).await;
        loopflow_execution::ops::planning_sync::publish_repository_now(&store, &repo_key).await?;
        fetched?;
        let destinations =
            loopflow_execution::ops::planning_sync::git_destinations(&store, &repo_key).await?;
        let Some(active) = destinations.into_iter().find(|d| d.active) else {
            return Err(anyhow!(
                "no Git planning destination is configured; see `lf repo tracker`"
            ));
        };
        if let Some(error) = active.fetch_error {
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

async fn run_async(repo: &Path, cmd: Option<&RepoTrackerCommand>, status_json: bool) -> Result<()> {
    let store = open_store().await?;
    let repo_key = repo.to_string_lossy();
    match cmd {
        Some(RepoTrackerCommand::Key { new, recover }) => {
            let saved = store.planning_user_key().await?;
            let key = match (recover, saved, new) {
                (Some(key), _, _) => key.clone(),
                (None, Some(key), _) => key,
                (None, None, true) => uuid::Uuid::new_v4().to_string(),
                _ => return Err(anyhow!(
                    "No planning user key. Use `lf repo tracker key --new` once, then recover that key on other machines with `--recover <UUID>`."
                )),
            };
            if *new || recover.is_some() {
                store.provision_planning_user_key(&key).await?;
            }
            println!("{key}");
        }
        None => print_status(&store, &repo_key, status_json).await?,
        Some(RepoTrackerCommand::Journal { json }) => {
            print_journal(&store, &repo_key, *json).await?
        }
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
/// it changes. The subscription reads stored status; `lf wave status --sync` and
/// foreground providers exchange planning.
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
            async move { Ok(status(store.as_deref(), &repo_key).await?.report()) }
        },
        transport,
    )
}

/// What `repo.tracker` answers: the tracker, for Git its destinations with
/// their import and publication evidence, and every open and lost tracker
/// action.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(crate) struct TrackerStatus {
    /// `git`, `linear` or `asana`.
    tracker: String,
    destinations: Vec<loopflow_execution::store::GitDestination>,
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
        Vec<loopflow_execution::store::GitDestination>,
        Vec<TrackerActionStatus>,
    ),
}

impl Status {
    /// What `lf repo tracker --json` prints.
    fn report(self) -> TrackerStatus {
        match self {
            Self::Remote(tracker, actions) => TrackerStatus {
                tracker: tracker.as_str().to_owned(),
                destinations: Vec::new(),
                actions,
            },
            Self::Git(destinations, actions) => TrackerStatus {
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

/// What `lf --cwd <directory> repo tracker --json` prints.
pub(crate) fn read_status(directory: &Path) -> Result<TrackerStatus> {
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
            Some(store) => {
                loopflow_execution::ops::planning_sync::git_destinations(store, repo).await?
            }
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
    if let Some(hub) = loopflow_execution::ops::plan_hub::hub(&store.sqlite)? {
        return print_hub(store, repo, &hub);
    }
    if !destinations.iter().any(|d| d.active) {
        println!("Tracker: git. Saves stay on this Machine until its remote takes them.");
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
        for error in [destination.fetch_error, destination.publication_error]
            .into_iter()
            .flatten()
        {
            println!("  {error}");
        }
        for conflict in destination.conflicts {
            println!("  Held {}: {}", conflict.target, conflict.reason);
        }
    }
    println!(
        "Import retention is not publication; `lf repo tracker journal` lists retained values."
    );
    if !lost.is_empty() {
        println!("Tracker: git. {} lost.", lost.len());
        print_actions(store, &lost)?;
    }
    Ok(())
}

/// `Plan: hub mini. 2 open.`, then each open value under its owner's name.
fn print_hub(store: &Store, repo: &str, hub: &loopflow_model::durable::Machine) -> Result<()> {
    let plan = store.sqlite.hub_plan(repo)?;
    let sends = store.sqlite.open_hub_sends(repo)?;
    let failing = plan.as_ref().and_then(|plan| {
        plan.failing_since
            .map(|since| (since, plan.error.clone().unwrap_or_default()))
    });
    let place = match &failing {
        Some((since, _)) => format!(
            "hub {}, unreachable since {}",
            hub.name(),
            loopflow_execution::output::format_time(*since)
        ),
        None => format!("hub {}", hub.name()),
    };
    if sends.is_empty() {
        println!("Plan: {place}. Nothing open.");
        if let Some((_, error)) = &failing {
            println!("  {error}");
        }
        return Ok(());
    }
    println!("Plan: {place}. {} open.", sends.len());
    let lines = sends
        .iter()
        .map(|send| {
            // `task_title` reads `title` under its Task.
            let field = send.edit.change.field();
            let field = ["task_", "wave_", "project_", "comment_"]
                .iter()
                .find_map(|prefix| field.strip_prefix(prefix))
                .unwrap_or(field);
            let mut line = format!(
                "{field} · {}",
                if send.sent_at.is_some() {
                    "sent"
                } else {
                    "queued"
                }
            );
            if let Some((_, error)) = &failing {
                line.push_str(&format!(" · {error}"));
            }
            Ok((hub_owner_name(store, send)?, line))
        })
        .collect::<Result<Vec<_>>>()?;
    let width = lines
        .iter()
        .map(|(owner, _)| owner.len())
        .max()
        .unwrap_or(0);
    for (owner, line) in lines {
        println!("  {owner:width$}  {line}");
    }
    Ok(())
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

async fn print_journal(store: &Store, repo: &str, json: bool) -> Result<()> {
    let journals = loopflow_execution::ops::planning_sync::git_journal(store, repo).await?;
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
