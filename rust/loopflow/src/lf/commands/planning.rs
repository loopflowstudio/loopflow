//! Explicit planning setup. A connection pins routing; it never enrolls existing
//! records, contacts a remote, or claims that planning has been published.

use std::path::Path;

use anyhow::{anyhow, Result};

use crate::engine::planning_exchange::{PlanningKind, PlanningObject};
use crate::engine::planning_git::PlanningDestination;
use crate::id::WaveId;
use crate::lf::PlanningCommand;
use crate::store::{RegistryUnavailable, Store};

pub fn run(cmd: &PlanningCommand) -> Result<()> {
    let repo = crate::engine::worktrees::main_repo_root(&super::util::find_repo_root()?)?;
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(run_async(&repo, cmd))
}

async fn run_async(repo: &Path, cmd: &PlanningCommand) -> Result<()> {
    let store = crate::store::open_registry_for_authority()
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
        })?;
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
        PlanningCommand::Associate {
            incoming,
            local,
            linear,
        } => {
            let kind = if incoming.starts_with("task_") {
                PlanningKind::Task
            } else if incoming.starts_with("proj_") {
                PlanningKind::Project
            } else {
                return Err(anyhow!(
                    "Use a full incoming Task or Project ID, not an issue name or prefix."
                ));
            };
            store
                .associate_peer_planning(
                    &repo_key,
                    &PlanningObject {
                        kind,
                        id: incoming.clone(),
                    },
                    local,
                    linear,
                )
                .await?;
            println!("{incoming} resolves to local {local}. Neither identity, execution nor sharing selection changed.");
            eprintln!("The next exchange projects selected history; private and rejected records remain held. No provider effect or Git publication was issued.");
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
            destination.recovery_error,
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
        for record in destination.records {
            println!(
                "  {} {} — {}",
                record.object.kind.as_str(),
                record.object.id,
                match record.destination.as_deref() {
                    Some(id) if id == destination.id => "selected here".to_string(),
                    Some(id) => format!("not selected here; selected in {id}"),
                    None => "local only; not selected".to_string(),
                }
            );
            if record.local_id != record.object.id {
                println!(
                    "    Resolves to {} (execution stays local)",
                    record.local_id
                );
            }
            for reference in record.references {
                println!(
                    "    Retained reference: {} {}",
                    reference.kind.as_str(),
                    reference.id
                );
            }
            for value in record.values {
                let author = match value.author {
                    Some(crate::ops::pm::TaskCommentAuthor::Person { name }) => {
                        name.unwrap_or_else(|| "Unknown person".into())
                    }
                    Some(crate::ops::pm::TaskCommentAuthor::Integration) => "Integration".into(),
                    None => "Unknown".into(),
                };
                println!(
                    "    {} [{}; {}; author: {}]: {}",
                    value.field,
                    value.id,
                    if value.candidate {
                        "candidate, not confirmed"
                    } else {
                        "retained alternative"
                    },
                    author,
                    value.value_json
                );
            }
        }
    }
    println!("Recovery inspection changes nothing. Copy a retained value into an ordinary edit to save it again; selecting a Wave can share its retained history.");
    println!("Import retention is not publication or convergence. Linear delivery is separate.");
    Ok(())
}
