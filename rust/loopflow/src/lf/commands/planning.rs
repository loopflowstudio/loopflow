//! Inspect configured planning and recover personal Git identity.

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::lf::PlanningCommand;
use crate::store::{RegistryUnavailable, Store};

pub fn run(cmd: &PlanningCommand) -> Result<()> {
    // A Work folder without Git still has stored planning status.
    let repo = match super::util::find_repo_root() {
        Ok(root) => crate::git::worktrees::main_repo_root(&root)?,
        Err(_) => std::env::current_dir()?,
    };
    if let PlanningCommand::Status { watch: true, .. } = cmd {
        return watch_status(&repo);
    }
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run_async(&repo, cmd))
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
fn watch_status(repo: &Path) -> Result<()> {
    let repo_key = repo.to_string_lossy().into_owned();
    let runtime = tokio::runtime::Runtime::new()?;
    let database = crate::store::database_path_from_env()?;
    // Nothing here creates a store; without one there is nothing to exchange.
    let _sync = database
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
    super::watch::run(
        (),
        super::watch::Watched::every(STATUS_CLOCK),
        |store, ()| {
            let repo_key = repo_key.clone();
            async move {
                let config =
                    crate::config::load_config(Some(Path::new(&repo_key)))?.unwrap_or_default();
                if let Some(provider) = config.planning_transport().remote_name() {
                    return Ok(remote_status(provider));
                }
                let destinations = match store {
                    Some(store) => {
                        crate::ops::planning_sync::peer_status(&store, &repo_key).await?
                    }
                    None => Vec::new(),
                };
                Ok(serde_json::json!({"transport": "git", "destinations": destinations}))
            }
        },
    )
}

fn remote_status(provider: &str) -> serde_json::Value {
    serde_json::json!({"transport": provider.to_ascii_lowercase(), "destinations": []})
}

async fn print_status(store: &Store, repo: &str, json: bool) -> Result<()> {
    let config = crate::config::load_config(Some(Path::new(repo)))?.unwrap_or_default();
    if let Some(provider) = config.planning_transport().remote_name() {
        if json {
            println!("{}", remote_status(provider));
        } else {
            println!("Planning: {provider}. Unavailable transport leaves local saves pending.");
        }
        return Ok(());
    }
    crate::ops::planning_sync::configured_destination(store, repo).await?;
    let destinations = crate::ops::planning_sync::peer_status(store, repo).await?;
    if json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "transport":"git",
                "destinations": destinations,
            }))?
        );
        return Ok(());
    }
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
