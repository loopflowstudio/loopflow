use anyhow::{anyhow, Context};
use serde::Serialize;

use crate::durable::{
    AuthenticatedRequest, ControlCtx, EpochReceipt, InterruptReceipt, ProjectId, Review, Run,
    SteerReceipt, TaskId, WorkRef, WorkStatus,
};
use crate::id::WaveId;
use crate::lf::WorkCommand;
use crate::store::{open_store, storage_config_from_env, Store};

#[derive(Debug, Serialize)]
struct WorkProjection {
    work: WorkRef,
    basis: crate::durable::Basis,
    status: WorkStatus,
    run: Option<Run>,
    review: Option<Review>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WorkReceipt {
    Steer(SteerReceipt),
    ReviewClosed { status: WorkStatus },
    Interrupted(InterruptReceipt),
    Abandoned(EpochReceipt),
}

pub fn run(command: &WorkCommand) -> anyhow::Result<()> {
    tokio::runtime::Runtime::new()?.block_on(run_async(command))
}

async fn run_async(command: &WorkCommand) -> anyhow::Result<()> {
    let store = open_shared_store().await?;
    match command {
        WorkCommand::Status { kind, id, json } => {
            let work = parse_work(kind, id)?;
            let projection = projection(&store, &work).await?;
            print_projection(&projection, *json)?;
        }
        WorkCommand::Steer {
            kind,
            id,
            message,
            json,
        } => {
            let work = parse_work(kind, id)?;
            let receipt = if let Some(lease) = crate::ops::ambient_run_lease(&store).await? {
                store
                    .steer(&ControlCtx::Run(&lease), &work, message, None)
                    .await?
            } else {
                let request = AuthenticatedRequest::cli();
                store
                    .steer(&ControlCtx::User(&request), &work, message, None)
                    .await?
            };
            print_receipt(&WorkReceipt::Steer(receipt), *json)?;
        }
        WorkCommand::Close { kind, id, json } => {
            let work = parse_work(kind, id)?;
            let review = store
                .review(&work)
                .await?
                .ok_or_else(|| anyhow!("{} {} has no current Review", work.kind(), work.id()))?;
            let status = if let Some(lease) = crate::ops::ambient_run_lease(&store).await? {
                store
                    .close_review(&ControlCtx::Run(&lease), &work, &review.basis)
                    .await?
            } else {
                let request = AuthenticatedRequest::cli();
                store
                    .close_review(&ControlCtx::User(&request), &work, &review.basis)
                    .await?
            };
            print_receipt(&WorkReceipt::ReviewClosed { status }, *json)?;
        }
        WorkCommand::Interrupt { kind, id, json } => {
            let work = parse_work(kind, id)?;
            let run = store
                .current_run(&work)
                .await?
                .ok_or_else(|| anyhow!("{} {} has no active Run", work.kind(), work.id()))?;
            let receipt = if let Some(lease) = crate::ops::ambient_run_lease(&store).await? {
                store
                    .interrupt(&ControlCtx::Run(&lease), &work, &run.id)
                    .await?
            } else {
                let request = AuthenticatedRequest::cli();
                store
                    .interrupt(&ControlCtx::User(&request), &work, &run.id)
                    .await?
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
            if crate::ops::ambient_run_lease(&store).await?.is_some() {
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

async fn projection(store: &Store, work: &WorkRef) -> anyhow::Result<WorkProjection> {
    Ok(WorkProjection {
        work: work.clone(),
        basis: store.current_epoch(work).await?.current_basis,
        status: store.work_status(work).await?,
        run: store.current_run(work).await?,
        review: store.review(work).await?,
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

fn print_projection(projection: &WorkProjection, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(projection)?);
    } else {
        println!(
            "{} {}  {:?}\n  basis: {}:{}\n  run: {}\n  attention: {}",
            projection.work.kind(),
            projection.work.id(),
            projection.status,
            projection.basis.epoch_id,
            projection.basis.revision,
            projection
                .run
                .as_ref()
                .map_or("none", |run| run.id.as_str()),
            projection.review.as_ref().map_or("none", |review| {
                match (&review.attention, review.attention_at.is_some()) {
                    (crate::durable::AttentionRoute::User, true) => "user (pending)",
                    (crate::durable::AttentionRoute::User, false) => "user (parked)",
                    (crate::durable::AttentionRoute::Parent(_), true) => "parent (pending)",
                    (crate::durable::AttentionRoute::Parent(_), false) => "parent (parked)",
                }
            }),
        );
    }
    Ok(())
}

fn print_receipt(receipt: &WorkReceipt, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(receipt)?);
    } else {
        match receipt {
            WorkReceipt::Steer(receipt) => println!("steered {}", receipt.steer.id),
            WorkReceipt::ReviewClosed { status } => println!("closed Review: {status:?}"),
            WorkReceipt::Interrupted(receipt) => println!("interrupted {}", receipt.run_id),
            WorkReceipt::Abandoned(receipt) => println!("abandoned {}", receipt.epoch.id),
        }
    }
    Ok(())
}
