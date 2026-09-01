use anyhow::{anyhow, Context};
use serde::Serialize;
use std::path::Path;

use crate::durable::{AbandonReceipt, Placement, ProjectId, Steer, TaskId, WorkRef, WorkStatus};
use crate::id::WaveId;
use crate::lf::WorkCommand;
use crate::store::{open_store, storage_config_from_env, Store};

#[derive(Debug, Serialize)]
struct WorkProjection {
    work: WorkRef,
    placement: Placement,
    status: WorkStatus,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WorkReceipt {
    Placed(Placement),
    Relocated(crate::controller::wave::relocate::WaveRelocationReceipt),
    Enabled(Placement),
    Disabled(Placement),
    Steer(Steer),
    Abandoned(AbandonReceipt),
}

pub fn run(command: &WorkCommand, repo: &Path) -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(run_async(command, repo))
}

async fn run_async(command: &WorkCommand, repo: &Path) -> anyhow::Result<()> {
    let store = open_shared_store().await?;
    match command {
        WorkCommand::Status { kind, id, json } => {
            let work = parse_work(kind, id)?;
            require_work_repository(&store, &work, repo).await?;
            let projection = projection(&store, &work).await?;
            print_projection(&projection, *json)?;
        }
        WorkCommand::Place {
            kind,
            id,
            home_id,
            json,
        } => {
            let work = parse_work(kind, id)?;
            require_work_repository(&store, &work, repo).await?;
            if !matches!(work, WorkRef::Wave(_)) {
                return Err(anyhow!(
                    "only Wave Work has independently movable placement"
                ));
            }
            let placement = store.place_work(&work, home_id).await?;
            print_receipt(&WorkReceipt::Placed(placement), *json)?;
        }
        WorkCommand::Relocate {
            kind,
            id,
            repo: target_repo,
            name,
            json,
        } => {
            if kind != "wave" {
                return Err(anyhow!("only Wave Work has a repository locator"));
            }
            let wave_id = WaveId::parse(id)?;
            let receipt = crate::controller::wave::relocate::relocate_wave(
                &store,
                &wave_id,
                repo,
                target_repo.as_deref(),
                name.as_deref(),
            )
            .await?;
            print_receipt(&WorkReceipt::Relocated(receipt), *json)?;
        }
        WorkCommand::Enable { kind, id, json } => {
            let work = parse_work(kind, id)?;
            require_work_repository(&store, &work, repo).await?;
            let placement = set_local_work_enabled(&store, &work, true).await?;
            print_receipt(&WorkReceipt::Enabled(placement), *json)?;
        }
        WorkCommand::Disable { kind, id, json } => {
            let work = parse_work(kind, id)?;
            require_disable_repository(&store, &work, repo).await?;
            let placement = set_local_work_enabled(&store, &work, false).await?;
            print_receipt(&WorkReceipt::Disabled(placement), *json)?;
        }
        WorkCommand::Steer {
            kind,
            id,
            message,
            json,
        } => {
            let work = parse_work(kind, id)?;
            require_work_repository(&store, &work, repo).await?;
            let author = crate::ops::ambient_author()?;
            let steer = store.append_steer(&work, author, message).await?;
            print_receipt(&WorkReceipt::Steer(steer), *json)?;
        }
        WorkCommand::Interrupt { kind, id, .. } => {
            let work = parse_work(kind, id)?;
            require_work_repository(&store, &work, repo).await?;
            return Err(anyhow!(
                "cannot interrupt {} {}: no exact process owner is recorded",
                work.kind(),
                work.id()
            ));
        }
        WorkCommand::Abandon {
            kind,
            id,
            reason,
            json,
        } => {
            let work = parse_work(kind, id)?;
            require_work_repository(&store, &work, repo).await?;
            let receipt = store.abandon(&work, reason).await?;
            print_receipt(&WorkReceipt::Abandoned(receipt), *json)?;
        }
    }
    Ok(())
}

async fn open_shared_store() -> anyhow::Result<Store> {
    let config = storage_config_from_env().context("resolve the shared Loopflow store")?;
    open_store(&config)
        .await
        .context("open the shared Loopflow store")
}

async fn set_local_work_enabled(
    store: &Store,
    work: &WorkRef,
    enabled: bool,
) -> anyhow::Result<Placement> {
    let placement = store.placement(work).await?;
    let local = store.local_home().await?;
    if placement.home_id != local.id {
        return Err(anyhow!(
            "{} {} is placed on {}; run this command through that Home",
            work.kind(),
            work.id(),
            placement.home_id
        ));
    }
    store
        .set_work_enabled(work, enabled)
        .await
        .map_err(anyhow::Error::from)
}

async fn projection(store: &Store, work: &WorkRef) -> anyhow::Result<WorkProjection> {
    let status = store.work_status(work).await?;
    Ok(WorkProjection {
        work: work.clone(),
        placement: store.placement(work).await?,
        status,
    })
}

fn parse_work(kind: &str, id: &str) -> anyhow::Result<WorkRef> {
    match kind {
        "wave" => Ok(WorkRef::Wave(WaveId::parse(id)?)),
        "project" => Ok(WorkRef::Project(ProjectId::parse(id)?)),
        "task" => Ok(WorkRef::Task(TaskId::parse(id)?)),
        value => Err(anyhow!("invalid Work kind {value:?}")),
    }
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

async fn require_disable_repository(
    store: &Store,
    work: &WorkRef,
    repo: &Path,
) -> anyhow::Result<()> {
    if let WorkRef::Wave(wave_id) = work {
        let wave = store
            .get_wave(wave_id)
            .await?
            .ok_or_else(|| anyhow!("Wave {wave_id} is not registered"))?;
        if crate::repository::CanonicalRepo::discover(Path::new(wave.repo())).is_err() {
            crate::repository::CanonicalRepo::discover(repo)?;
            return Ok(());
        }
    }
    require_work_repository(store, work, repo).await
}

fn print_projection(projection: &WorkProjection, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(projection)?);
    } else {
        println!(
            "{} {}  {}\n  enabled: {}\n  home: {}",
            projection.work.kind(),
            projection.work.id(),
            projection.status,
            projection.placement.enabled,
            projection.placement.home_id,
        );
    }
    Ok(())
}

fn print_receipt(receipt: &WorkReceipt, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(receipt)?);
    } else {
        match receipt {
            WorkReceipt::Placed(placement) => println!(
                "{} {}  ->  {}",
                placement.work.kind(),
                placement.work.id(),
                placement.home_id
            ),
            WorkReceipt::Relocated(relocation) => println!(
                "Wave {}  {}/{}  ->  {}/{}",
                relocation.wave_id,
                relocation.from_repo,
                relocation.from_name,
                relocation.to_repo,
                relocation.to_name
            ),
            WorkReceipt::Enabled(placement) => println!(
                "enabled {} {} on {}",
                placement.work.kind(),
                placement.work.id(),
                placement.home_id
            ),
            WorkReceipt::Disabled(placement) => println!(
                "disabled {} {} on {}",
                placement.work.kind(),
                placement.work.id(),
                placement.home_id
            ),
            WorkReceipt::Steer(steer) => println!("steered {}", steer.id),
            WorkReceipt::Abandoned(receipt) => println!("abandoned {}", receipt.work.id()),
        }
    }
    Ok(())
}
