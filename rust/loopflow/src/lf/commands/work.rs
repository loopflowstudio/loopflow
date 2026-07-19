use anyhow::{anyhow, Context};
use serde::Serialize;

use crate::durable::{
    Answer, AskExchange, AuthenticatedRequest, ControlCtx, EpochReceipt, InterruptReceipt,
    Placement, ProjectId, Run, SteerReceipt, TaskId, WorkRef, WorkStatus,
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
}

#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum WorkReceipt {
    Placed(Placement),
    Steer(SteerReceipt),
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
        WorkCommand::Place {
            kind,
            id,
            home_id,
            json,
        } => {
            let work = parse_work(kind, id)?;
            if !matches!(work, WorkRef::Wave(_)) {
                return Err(anyhow!(
                    "only Wave Work can move until Project and Task execution uses the shared Run supervisor"
                ));
            }
            let placement = store.place_work(&work, home_id).await?;
            print_receipt(&WorkReceipt::Placed(placement), *json)?;
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
        WorkCommand::Asks { kind, id, json } => {
            let lease = crate::ops::ambient_run_lease(&store).await?;
            let asks = match (lease.as_ref(), kind.as_deref(), id.as_deref()) {
                (Some(lease), None, None) => store.pending_asks_for_parent(&lease.work).await?,
                (Some(lease), Some(kind), Some(id)) => {
                    let parent = parse_work(kind, id)?;
                    if parent != lease.work {
                        return Err(anyhow!(
                            "ambient Run owns {} {}, not {} {}",
                            lease.work.kind(),
                            lease.work.id(),
                            parent.kind(),
                            parent.id()
                        ));
                    }
                    store.pending_asks_for_parent(&parent).await?
                }
                (None, None, None) => store.pending_user_asks().await?,
                (None, Some(_), Some(_)) => {
                    return Err(anyhow!(
                        "parent-routed Asks require that parent Work's active Run lease"
                    ))
                }
                (_, _, _) => return Err(anyhow!("pass both Work kind and id, or neither")),
            };
            print_asks(&asks, *json)?;
        }
        WorkCommand::Answer { ask_id, text, json } => {
            let answer = if let Some(lease) = crate::ops::ambient_run_lease(&store).await? {
                store
                    .answer_ask(&ControlCtx::Run(&lease), ask_id, text)
                    .await?
            } else {
                let request = AuthenticatedRequest::cli();
                store
                    .answer_ask(&ControlCtx::User(&request), ask_id, text)
                    .await?
            };
            print_answer(&answer, *json)?;
            if let Err(error) = crate::ops::publish_pending_ask_comments(&store).await {
                tracing::warn!(%error, "Ask comment outbox publication failed");
            }
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

fn print_asks(asks: &[AskExchange], json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(asks)?);
    } else if asks.is_empty() {
        println!("No pending Asks.");
    } else {
        for ask in asks {
            println!("{}  {}", ask.id, ask.question);
        }
    }
    Ok(())
}

fn print_answer(answer: &Answer, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(answer)?);
    } else {
        println!("answered {}", answer.ask_id);
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
            "{} {}  {:?}\n  basis: {}:{}\n  run: {}",
            projection.work.kind(),
            projection.work.id(),
            projection.status,
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
            WorkReceipt::Steer(receipt) => println!("steered {}", receipt.steer.id),
            WorkReceipt::Interrupted(receipt) => println!("interrupted {}", receipt.run_id),
            WorkReceipt::Abandoned(receipt) => println!("abandoned {}", receipt.epoch.id),
        }
    }
    Ok(())
}
