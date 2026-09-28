use crate::durable::WorkRef;
use crate::id::WaveId;
use crate::lf::WaveCommand;
use crate::store::{open_store, storage_config_from_env, Store};
use anyhow::{anyhow, Context};
use std::path::Path;
use std::sync::Arc;

pub fn wave(repo: &Path, command: &WaveCommand) -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(async {
        let store = open_shared_store().await?;
        let name = match command {
            WaveCommand::Forget { name, .. }
            | WaveCommand::Place { name, .. }
            | WaveCommand::Retire { name, .. } => name,
            WaveCommand::Relocate { wave, .. } => wave,
            _ => unreachable!("Wave placement dispatcher"),
        };
        let wave_id = match WaveId::parse(name) {
            Ok(id) => id,
            Err(_) => crate::work::wave::context::resolve_managed_wave(
                Some(&store),
                Some(repo),
                Some(name),
                None,
            )
            .await?
            .id()
            .clone(),
        };
        let work = WorkRef::Wave(wave_id.clone());
        if !matches!(command, WaveCommand::Relocate { .. }) {
            require_work_repository(&store, &work, repo).await?;
        }
        match command {
            WaveCommand::Forget { dry_run, json, .. } => {
                let wave = store
                    .get_wave(&wave_id)
                    .await?
                    .ok_or_else(|| anyhow!("Wave {name} not found"))?;
                let snapshot = crate::lf::commands::waves::snapshot_wave(&store, &wave).await?;
                if Path::new(wave.repo())
                    .join("wave")
                    .join(wave.name())
                    .join("GOAL.md")
                    .exists()
                {
                    return Err(anyhow!(
                        "Wave {} still has an authored GOAL.md",
                        wave.name()
                    ));
                }
                store.forget_wave(&wave_id, *dry_run).await?;
                print(
                    &serde_json::json!({"wave": snapshot, "forgotten": !dry_run}),
                    *json,
                    &format!(
                        "{} Wave {}",
                        if *dry_run { "Would forget" } else { "Forgot" },
                        wave.name()
                    ),
                )
            }
            WaveCommand::Place { home_id, json, .. } => print(
                &store.place_work(&work, home_id).await?,
                *json,
                &format!("Wave {name}: placed on {home_id}"),
            ),
            WaveCommand::Relocate {
                repo: target,
                name,
                json,
                ..
            } => print(
                &crate::work::wave::relocate::relocate_wave(
                    &store,
                    &wave_id,
                    repo,
                    target.as_deref(),
                    name.as_deref(),
                )
                .await?,
                *json,
                &format!("Wave {wave_id}: relocated"),
            ),
            WaveCommand::Retire { reason, json, .. } => print(
                &store.abandon(&work, reason).await?,
                *json,
                &format!("Wave {name}: retired"),
            ),
            _ => unreachable!("Wave placement dispatcher"),
        }
    })
}

fn print(value: &impl serde::Serialize, json: bool, summary: &str) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(value)?);
    } else {
        println!("{summary}");
    }
    Ok(())
}

async fn open_shared_store() -> anyhow::Result<Arc<Store>> {
    let config = storage_config_from_env().context("resolve the shared Loopflow store")?;
    open_store(&config)
        .await
        .map(Arc::new)
        .context("open the shared Loopflow store")
}

async fn require_work_repository(store: &Store, work: &WorkRef, repo: &Path) -> anyhow::Result<()> {
    let wave_id = match work {
        WorkRef::Wave(wave_id) => wave_id.clone(),
        WorkRef::Project(project_id) => {
            store
                .get_project(project_id)
                .await?
                .ok_or_else(|| anyhow!("Project {project_id} is not registered"))?
                .wave_id
        }
        WorkRef::Task(task_id) => {
            store
                .get_task(task_id)
                .await?
                .ok_or_else(|| anyhow!("Task {task_id} is not registered"))?
                .wave_id
        }
    };
    let wave = store
        .get_wave(&wave_id)
        .await?
        .ok_or_else(|| anyhow!("Wave {wave_id} is not registered"))?;
    let locator = crate::work::wave::WaveLocator::discover(repo, wave.name())?;
    let local = store.get_wave_at(&locator).await?;
    if local.as_ref().map(crate::work::wave::Wave::id) != Some(&wave_id) {
        return Err(anyhow!(
            "{} {} belongs to repository {}, not invoking repository {}",
            work.kind(),
            work.id(),
            wave.repo(),
            locator.repo()
        ));
    }
    Ok(())
}
