//! Explicit planning setup. A connection pins routing; it never enrolls existing
//! records, contacts a remote, or claims that planning has been published.

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::engine::planning_git::PlanningDestination;
use crate::id::WaveId;
use crate::lf::PlanningCommand;
use crate::store::Store;

pub fn run(cmd: &PlanningCommand) -> Result<()> {
    let repo = crate::engine::worktrees::main_repo_root(&super::util::find_repo_root()?)?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run_async(&repo, cmd))
}

async fn run_async(repo: &Path, cmd: &PlanningCommand) -> Result<()> {
    let store = crate::store::open_registry_for_authority().await?;
    let repo_key = repo.to_string_lossy();
    match cmd {
        PlanningCommand::Key { new, recover } => {
            let saved = store.planning_user_key().await?;
            let key = match (recover, saved, new) {
                (Some(key), _, _) => key.clone(),
                (None, Some(key), _) => key,
                (None, None, true) => uuid::Uuid::new_v4().to_string(),
                _ => return Err(anyhow!(
                    "No planning user key. Use `lf planning key --new` once, then recover that key on other machines with `--recover <UUID>`."
                )),
            };
            if *new || recover.is_some() {
                store.provision_planning_user_key(&key).await?;
            }
            println!("{key}");
        }
        PlanningCommand::Connect { remote, shared } => {
            let reference = match shared {
                Some(name) => format!("refs/loopflow/planning/shared/{name}"),
                None => {
                    let key = store.planning_user_key().await?.ok_or_else(|| {
                        anyhow!("Create or recover a planning user key before connecting.")
                    })?;
                    format!("refs/loopflow/planning/users/{key}")
                }
            };
            let destination = PlanningDestination::resolve(repo, remote, &reference)?;
            let id = store.bind_peer_planning(&repo_key, &destination).await?;
            println!("{id}");
            eprintln!("Pinned {reference}. Existing selection unchanged; nothing published. Ref separation is not privacy: use a controlled remote.");
        }
        PlanningCommand::Use { destination } => {
            store
                .use_peer_planning(
                    &repo_key,
                    (destination != "local").then_some(destination.as_str()),
                )
                .await?;
            println!("Future root Waves: {destination}. Existing selection unchanged.");
        }
        PlanningCommand::Select { destination, waves } => {
            let waves = waves
                .iter()
                .map(|wave| WaveId::parse(wave))
                .collect::<Result<Vec<_>, _>>()?;
            store
                .select_peer_waves(&repo_key, destination, &waves)
                .await?;
            println!("Selected {} Wave(s) and descendants. Retained history may now be shared; nothing published by this command.", waves.len());
        }
        PlanningCommand::Status { json } => print_status(&store, &repo_key, *json).await?,
    }
    Ok(())
}

async fn print_status(store: &Store, repo: &str, json: bool) -> Result<()> {
    let destinations = store.peer_planning_status(repo).await?;
    if json {
        println!(
            "{}",
            serde_json::to_string(&serde_json::json!({
                "destinations": destinations,
            }))?
        );
        return Ok(());
    }
    if !destinations.iter().any(|d| d.active) {
        println!("Future root Waves: local");
    }
    for destination in destinations {
        println!(
            "{}  {}{}  {} selected records",
            destination.id,
            destination.reference,
            if destination.active {
                " (future root Waves)"
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
            match destination.pending_local {
                Some(true) => "; local changes pending",
                Some(false) => "",
                None => "; local changes unknown",
            }
        );
        for error in [
            destination.local_error,
            destination.acquisition_error,
            destination.publication_error,
        ]
        .into_iter()
        .flatten()
        {
            println!("  {error}");
        }
        for conflict in destination.conflicts {
            println!(
                "  Held {} {}: {}",
                conflict.object.kind.as_str(),
                conflict.object.id,
                conflict.reason
            );
        }
    }
    println!("Import retention is not publication or convergence. Linear delivery is separate.");
    Ok(())
}
