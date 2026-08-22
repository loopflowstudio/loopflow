use anyhow::{anyhow, Context};
use serde::Serialize;
use std::path::Path;

use crate::durable::{
    EpochReceipt, InterruptReceipt, Placement, ProjectId, Run, SteerReceipt, TaskId, WorkRef,
    WorkStatus,
};
use crate::id::WaveId;
use crate::lf::WorkCommand;
use crate::store::{open_store, storage_config_from_env, Store};

#[derive(Debug, Serialize)]
struct WorkProjection {
    work: WorkRef,
    basis: crate::durable::Basis,
    placement: Placement,
    status: WorkStatus,
    current: crate::child::CurrentWorkObservation,
    run: Option<Run>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WorkReceipt {
    Placed(Placement),
    Relocated(crate::wave::relocate::WaveRelocationReceipt),
    Enabled(Placement),
    Disabled(Placement),
    Steer(SteerReceipt),
    Interrupted(InterruptReceipt),
    Abandoned(EpochReceipt),
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
                    "only Wave Work can move until Project and Task execution uses the shared Run supervisor"
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
            let receipt = crate::wave::relocate::relocate_wave(
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
            let lease = crate::ops::ambient_run_context(&store).await?;
            require_work_repository(&store, &work, repo).await?;
            let receipt = if let Some(lease) = lease {
                store.steer(Some(&lease), &work, message, None).await?
            } else {
                store.steer(None, &work, message, None).await?
            };
            print_receipt(&WorkReceipt::Steer(receipt), *json)?;
        }
        WorkCommand::Asks { .. } => {
            return Err(anyhow!("`lf work asks` retired; use `lf ask list`"));
        }
        WorkCommand::Answer { ask_id, .. } => {
            return Err(anyhow!(
                "`lf work answer` retired; use `lf ask open {ask_id}` and settle from the Ask session"
            ));
        }
        WorkCommand::Interrupt { kind, id, json } => {
            let work = parse_work(kind, id)?;
            require_work_repository(&store, &work, repo).await?;
            let run = store
                .current_run(&work)
                .await?
                .ok_or_else(|| anyhow!("{} {} has no active Run", work.kind(), work.id()))?;
            let receipt = if let Some(lease) = crate::ops::ambient_run_context(&store).await? {
                store.interrupt(Some(&lease), &work, &run.id).await?
            } else {
                store.interrupt(None, &work, &run.id).await?
            };
            print_receipt(&WorkReceipt::Interrupted(receipt), *json)?;
        }
        WorkCommand::Abandon {
            kind,
            id,
            reason,
            json,
        } => {
            let work = parse_work(kind, id)?;
            require_work_repository(&store, &work, repo).await?;
            if crate::ops::ambient_run_context(&store).await?.is_some() {
                return Err(anyhow!(
                    "Run callers cannot abandon Work; use the authenticated User surface"
                ));
            }
            let basis = store.current_epoch(&work).await?.current_basis;
            let receipt = store.abandon(&work, reason, &basis).await?;
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
    let run = store.current_run(work).await?;
    let current =
        crate::child::observe_current_work(store, work, &status, time::OffsetDateTime::now_utc())
            .await?;
    Ok(WorkProjection {
        work: work.clone(),
        basis: store.current_epoch(work).await?.current_basis,
        placement: store.placement(work).await?,
        status,
        current,
        run,
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
    let locator = crate::wave::WaveLocator::discover(repo, wave.name())?;
    let local = store.get_wave_at(&locator).await?;
    if local.as_ref().map(crate::wave::Wave::id) != Some(&wave_id) {
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
            "{} {}  {}\n  enabled: {}\n  home: {}\n  basis: {}:{}\n  run: {}",
            projection.work.kind(),
            projection.work.id(),
            projection.current.state,
            projection.placement.enabled,
            projection.placement.home_id,
            projection.basis.epoch_id,
            projection.basis.revision,
            projection
                .run
                .as_ref()
                .map_or("none", |run| run.id.as_str()),
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
            WorkReceipt::Steer(receipt) => println!("steered {}", receipt.steer.id),
            WorkReceipt::Interrupted(receipt) => println!("interrupted {}", receipt.run_id),
            WorkReceipt::Abandoned(receipt) => println!("abandoned {}", receipt.epoch.id),
        }
    }
    Ok(())
}
