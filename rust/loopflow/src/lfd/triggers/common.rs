use time::OffsetDateTime;

use crate::lfd::events::EventHub;
use crate::lfd::executor::wave::classify_repair_flow;
use crate::lfd::executor::WaveExecutor;
pub use crate::lfd::executor::{create_parallel_wave_run, create_wave_run_with_id};
use crate::lfd::scheduler::SchedulerSlotGuard;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{Event, WaveRunStatus, WaveStatus};

/// Spawn a task that executes a wave run and releases a scheduler slot on completion.
pub fn spawn_run_task_with_slot(
    store: SharedStore,
    executor: WaveExecutor,
    event_hub: EventHub,
    run: crate::lfd::types::WaveRun,
    slot_guard: SchedulerSlotGuard,
) {
    event_hub.send(Event::wave_started(run.wave_id.clone(), run.id.clone()));
    tokio::spawn(async move {
        let _slot_guard = slot_guard;
        execute_run_inner(&store, &executor, &event_hub, &run).await;
    });
}

async fn execute_run_inner(
    store: &SharedStore,
    executor: &WaveExecutor,
    event_hub: &EventHub,
    run: &crate::lfd::types::WaveRun,
) {
    if let Err(err) = executor.execute(&run.id).await {
        tracing::error!(run_id = %run.id, error = %err, "run execution failed");
        if let Ok(Some(mut run)) = store.get_wave_run(&run.id).await {
            run.status = WaveRunStatus::Failed;
            run.error = Some(err.to_string());
            run.ended_at = Some(OffsetDateTime::now_utc());
            if let Err(err) = store.update_wave_run(&run).await {
                tracing::error!(run_id = %run.id, error = %err, "failed to update wave run status");
            }
            if let Ok(Some(mut wave)) = store.get_wave(&run.wave_id).await {
                wave.status = WaveStatus::Failed;
                if let Err(err) = store.update_wave(&wave).await {
                    tracing::error!(wave_id = %run.wave_id, error = %err, "failed to update wave status");
                }
                event_hub.send(Event::wave_updated(run.wave_id.clone()));
            }
        }
        return;
    }

    // executor.execute() returned Ok — check if the run ended in failure.
    // fail_run sets the status but defers repair dispatch to us (avoids
    // recursive-async Send issues with the sqlite mutex).
    let run = match store.get_wave_run(&run.id).await {
        Ok(Some(r)) => r,
        _ => return,
    };
    if run.status != WaveRunStatus::Failed || run.repair_of.is_some() {
        return;
    }

    // First failure, no prior repair attempt — try headless repair.
    let wave = match store.get_wave(&run.wave_id).await {
        Ok(Some(w)) => w,
        _ => return,
    };
    let repair_flow = classify_repair_flow(&run);
    tracing::info!(
        run_id = %run.id,
        wave_id = %wave.id(),
        repair_flow = %repair_flow,
        "dispatching headless repair attempt"
    );
    match executor.create_repair_run(&wave, &run, &repair_flow).await {
        Ok(repair_run) => {
            event_hub.send(Event::wave_started(
                repair_run.wave_id.clone(),
                repair_run.id.clone(),
            ));
            if let Err(err) = executor.execute(&repair_run.id).await {
                tracing::error!(
                    repair_run_id = %repair_run.id,
                    error = %err,
                    "repair run execution failed"
                );
            }
        }
        Err(err) => {
            tracing::error!(
                run_id = %run.id,
                error = %err,
                "failed to create repair run"
            );
        }
    }
}
