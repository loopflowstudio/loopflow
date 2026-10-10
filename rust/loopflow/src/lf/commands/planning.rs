//! Inspect configured planning and recover personal Git identity.

use std::path::Path;

use anyhow::{anyhow, Result};

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
        PlanningCommand::Status { json } => print_status(&store, &repo_key, *json).await?,
    }
    Ok(())
}

async fn print_status(store: &Store, repo: &str, json: bool) -> Result<()> {
    let config = crate::engine::config::load_config(Some(Path::new(repo)))?.unwrap_or_default();
    if matches!(
        config.planning_transport(),
        crate::engine::config::PlanningConfig::Linear {}
    ) {
        if json {
            println!(
                "{}",
                serde_json::json!({"transport":"linear","destinations":[]})
            );
        } else {
            println!("Planning: Linear. Unavailable transport leaves local saves pending.");
        }
        return Ok(());
    }
    crate::ops::planning_sync::configured_destination(store, repo).await?;
    let destinations = store.peer_planning_status(repo).await?;
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
        println!("Planning: user-keyed Git (configured remote; local saves wait when unavailable)");
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
                    Some(id) if id == destination.id => "configured here".to_string(),
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
    println!("Copy a retained value into an ordinary edit to save it again. Import retention is not publication.");
    Ok(())
}
