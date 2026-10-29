//! Inspect configured planning and recover personal Git identity.

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::lf::PlanningCommand;
use crate::store::{RegistryUnavailable, Store};

pub fn run(cmd: &PlanningCommand) -> Result<()> {
    let repo = status_repository(&std::env::current_dir()?)?;
    if let PlanningCommand::Status { watch: true, .. } = cmd {
        return watch_status(&repo, super::watch::Transport::Stdio).map(drop);
    }
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run_async(&repo, cmd))
}

/// The repository whose planning `directory` reads. A Work folder without Git
/// still has stored planning status.
pub(crate) fn status_repository(directory: &Path) -> Result<std::path::PathBuf> {
    Ok(
        match crate::repo::discover_repo_root(directory).ok().flatten() {
            Some(root) => crate::git::worktrees::main_repo_root(&root)?,
            None => directory.to_owned(),
        },
    )
}

/// Whether the repository containing `checkout` plans through a Git ref.
pub fn plans_through_git(checkout: &Path) -> Result<bool> {
    let repo = crate::git::worktrees::main_repo_root(checkout)?;
    let config = crate::config::load_config(Some(&repo))?.unwrap_or_default();
    Ok(matches!(
        config.planning_transport(),
        crate::config::PlanningConfig::Git { .. }
    ))
}

/// Exchange the repository's Git planning ref now. The exchange is per
/// repository, so a Wave selection has no effect.
pub fn refresh(checkout: &Path) -> Result<()> {
    let repo = crate::git::worktrees::main_repo_root(checkout)?;
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_store().await?;
        let repo_key = repo.to_string_lossy();
        // Publication acquires first, so one exchange brings teammates' changes
        // in and sends this Machine's pending ones out.
        crate::ops::planning_sync::publish_repository(&store, &repo_key).await?;
        let destinations = crate::ops::planning_sync::peer_status(&store, &repo_key).await?;
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
    crate::store::open_registry_for_authority()
        .await
        .map_err(|error| match error {
            RegistryUnavailable::MissingFile { path } => anyhow!(
                "Machine registry is missing at {}; initialize or restore it before planning setup",
                path.display()
            ),
            RegistryUnavailable::Unresolved { error } => {
                anyhow!("Machine registry path cannot be resolved: {error}")
            }
            RegistryUnavailable::Incompatible { path, error } => anyhow!(
                "Machine registry at {} is incompatible: {error}; run `lf doctor`",
                path.display()
            ),
        })
}

async fn run_async(repo: &Path, cmd: &PlanningCommand) -> Result<()> {
    let store = open_store().await?;
    let repo_key = repo.to_string_lossy();
    match cmd {
        PlanningCommand::Key { new, recover } => {
            let saved = store.planning_user_key().await?;
            let key = match (recover, saved, new) {
                (Some(key), _, _) => key.clone(),
                (None, Some(key), _) => key,
                (None, None, true) => uuid::Uuid::new_v4().to_string(),
                _ => return Err(anyhow!(
                    "No planning user key. Use `lf config planning key --new` once, then recover that key on other machines with `--recover <UUID>`."
                )),
            };
            if *new || recover.is_some() {
                store.provision_planning_user_key(&key).await?;
            }
            println!("{key}");
        }
        PlanningCommand::Status { json, .. } => print_status(&store, &repo_key, *json).await?,
        PlanningCommand::Journal { json } => print_journal(&store, &repo_key, *json).await?,
    }
    Ok(())
}

/// Transport selection lives in config, so it changes without a store write.
const STATUS_CLOCK: std::time::Duration = std::time::Duration::from_secs(5);

/// `lf planning status --watch`: the repository's planning is exchanged while
/// the watch runs, and `lf planning status --json` printed now and each time it
/// changes. Exchanges commit to the store and wake the watch like any writer.
pub(crate) fn watch_status(
    repo: &Path,
    transport: super::watch::Transport,
) -> Result<Option<super::watch::Subscription>> {
    let repo_key = repo.to_string_lossy().into_owned();
    let runtime = tokio::runtime::Runtime::new()?;
    let database = crate::store::database_path_from_env()?;
    // Nothing here creates a store; without one there is nothing to exchange.
    let sync = database
        .exists()
        .then(|| {
            runtime.block_on(async {
                let store = std::sync::Arc::new(
                    crate::store::open_store(&crate::store::StorageConfig::sqlite(database))
                        .await?,
                );
                crate::ops::planning_sync::PlanningSync::start_watch(store, repo_key.clone())
                    .map_err(anyhow::Error::from)
            })
        })
        .transpose()
        .inspect_err(|error| tracing::warn!(%error, "cannot start repository planning sync"))
        .ok()
        .flatten();
    // The exchange runs as long as the watch: its read owns it.
    let exchange = (sync, runtime);
    super::watch::start(
        (),
        super::watch::Watched::every(STATUS_CLOCK),
        move |store, ()| {
            let _ = &exchange;
            let repo_key = repo_key.clone();
            async move {
                let status = match remote_transport(&repo_key)? {
                    Some(provider) => Status::Remote(provider),
                    None => Status::Git(match store {
                        Some(store) => {
                            crate::ops::planning_sync::peer_status(&store, &repo_key).await?
                        }
                        None => Vec::new(),
                    }),
                };
                Ok(status.to_json())
            }
        },
        transport,
    )
}

/// Where a repository's planning is exchanged.
enum Status {
    /// The tracker its config names; local saves wait while it is unavailable.
    Remote(&'static str),
    Git(Vec<crate::store::PeerPlanningStatus>),
}

impl Status {
    /// What `lf planning status --json` prints.
    fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Remote(provider) => serde_json::json!({
                "transport": provider.to_ascii_lowercase(),
                "destinations": [],
            }),
            Self::Git(destinations) => {
                serde_json::json!({"transport": "git", "destinations": destinations})
            }
        }
    }
}

fn remote_transport(repo: &str) -> Result<Option<&'static str>> {
    let config = crate::config::load_config(Some(Path::new(repo)))?.unwrap_or_default();
    Ok(config.planning_transport().remote_name())
}

/// What `lf --cwd <directory> config planning status --json` prints.
pub(crate) fn status_json(directory: &Path) -> Result<String> {
    let repo = status_repository(directory)?;
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_store().await?;
        Ok(serde_json::to_string(
            &status(&store, &repo.to_string_lossy()).await?.to_json(),
        )?)
    })
}

async fn status(store: &Store, repo: &str) -> Result<Status> {
    if let Some(provider) = remote_transport(repo)? {
        return Ok(Status::Remote(provider));
    }
    crate::ops::planning_sync::configured_destination(store, repo).await?;
    Ok(Status::Git(
        crate::ops::planning_sync::peer_status(store, repo).await?,
    ))
}

async fn print_status(store: &Store, repo: &str, json: bool) -> Result<()> {
    let status = status(store, repo).await?;
    if json {
        println!("{}", serde_json::to_string(&status.to_json())?);
        return Ok(());
    }
    let destinations = match status {
        Status::Remote(provider) => {
            println!("Planning: {provider}. Unavailable transport leaves local saves pending.");
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
    let journals = crate::ops::planning_sync::peer_journal(store, repo).await?;
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
