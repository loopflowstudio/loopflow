//! Inspect configured planning and recover personal Git identity.

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::lf::RepoPlanCommand as PlanCommand;
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

/// `lf planning status --subscribe`: `lf planning status --json` now and each time
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

/// What `repo.plan` answers: the transport, for Git the destinations with
/// their import and publication evidence, and for a tracker its queue.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(crate) struct PlanStatus {
    /// `git`, or the tracker's lowercased name.
    transport: String,
    destinations: Vec<loopflow_execution::store::PeerPlanningStatus>,
    /// Changes waiting for the tracker, in send order.
    effects: Vec<QueuedEffect>,
}

/// One change waiting for the tracker.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
pub(crate) struct QueuedEffect {
    /// Send order across the repository.
    seq: i64,
    /// `task`, `project` or `wave`.
    owner_kind: String,
    owner_id: String,
    /// The Task identifier or Project name people read.
    owner: String,
    /// `create`, `field`, `order`, `state`, `comment`, `delete` or `link`.
    kind: String,
    /// What changes, as a person names it: `title`, `complete`, `comment`.
    change: String,
    /// The field or comment the change addresses.
    subject: Option<String>,
    /// `pending`, or `attempted`: sent with its reply unknown.
    phase: String,
    /// The tracker it is addressed to.
    provider: String,
    /// Why the last request did not settle it.
    error: Option<String>,
}

/// What an effect changes, as a person names it: `title`, `complete`, `comment`.
fn change(effect: &loopflow_execution::store::sqlite::tracker_effects::TrackerEffect) -> String {
    use loopflow_execution::store::sqlite::tracker_effects::EffectKind;
    match (effect.kind, effect.subject.as_deref()) {
        (EffectKind::Field, Some(field)) => field
            .trim_start_matches("task_")
            .trim_start_matches("project_")
            .to_owned(),
        (EffectKind::State, _) => match effect.value["value"]["planning_state"].as_str() {
            Some("completed") => "complete",
            Some("canceled") => "cancel",
            _ => "reopen",
        }
        .to_owned(),
        (kind, _) => kind.as_str().to_owned(),
    }
}

/// Where a repository's planning is exchanged.
enum Status {
    /// The tracker its config names, and what waits to reach it.
    Remote(&'static str, Vec<QueuedEffect>),
    Git(Vec<loopflow_execution::store::PeerPlanningStatus>),
}

impl Status {
    /// What `lf planning status --json` prints.
    fn report(self) -> PlanStatus {
        match self {
            Self::Remote(provider, effects) => PlanStatus {
                transport: provider.to_ascii_lowercase(),
                destinations: Vec::new(),
                effects,
            },
            Self::Git(destinations) => PlanStatus {
                transport: "git".to_owned(),
                destinations,
                effects: Vec::new(),
            },
        }
    }
}

fn queued(store: &Store, repo: &str) -> Result<Vec<QueuedEffect>> {
    use loopflow_execution::store::sqlite::tracker_effects::EffectOwner;
    let mut queued = Vec::new();
    for effect in store.sqlite.unsettled_tracker_effects(repo)? {
        let (owner_kind, owner_id, owner) = match &effect.owner {
            EffectOwner::Task(id) => (
                "task",
                id.to_string(),
                store
                    .sqlite
                    .task(id)?
                    .map_or_else(|| id.to_string(), |task| task.identifier),
            ),
            EffectOwner::Project(id) => (
                "project",
                id.to_string(),
                store
                    .sqlite
                    .project(id)?
                    .map_or_else(|| id.to_string(), |project| project.name),
            ),
            EffectOwner::Wave(id) => ("wave", id.to_string(), id.to_string()),
        };
        queued.push(QueuedEffect {
            seq: effect.seq,
            owner_kind: owner_kind.to_owned(),
            owner_id,
            owner,
            kind: effect.kind.as_str().to_owned(),
            change: change(&effect),
            subject: effect.subject,
            phase: effect.phase.as_str().to_owned(),
            provider: effect.provider.as_str().to_owned(),
            error: effect.error,
        });
    }
    Ok(queued)
}

fn remote_transport(repo: &str) -> Result<Option<&'static str>> {
    let config =
        loopflow_execution::config::load_config(Some(Path::new(repo)))?.unwrap_or_default();
    Ok(config.planning_transport().remote_name())
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
    if let Some(provider) = remote_transport(repo)? {
        let effects = match store {
            Some(store) => queued(store, repo)?,
            None => Vec::new(),
        };
        return Ok(Status::Remote(provider, effects));
    }
    Ok(Status::Git(match store {
        Some(store) => loopflow_execution::ops::planning_sync::peer_status(store, repo).await?,
        None => Vec::new(),
    }))
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
    let destinations = match status {
        Status::Remote(provider, effects) => {
            if effects.is_empty() {
                println!("Planning: {provider}. Nothing waiting to send.");
                return Ok(());
            }
            println!(
                "Planning: {provider}. {} change{} waiting to send.",
                effects.len(),
                if effects.len() == 1 { "" } else { "s" }
            );
            let width = effects.iter().map(|e| e.owner.len()).max().unwrap_or(0);
            for effect in &effects {
                let state = if effect.phase == "attempted" {
                    format!("sent, waiting for {provider} to confirm")
                } else {
                    "pending".to_owned()
                };
                println!("  {:width$}  {:9} {state}", effect.owner, effect.change);
            }
            return Ok(());
        }
        Status::Git(destinations) => destinations,
    };
    if !destinations.iter().any(|d| d.active) {
        println!("Planning: Git (configured remote; local saves wait when unavailable)");
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
