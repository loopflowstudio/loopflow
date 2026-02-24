use time::OffsetDateTime;

use crate::lfd::events::EventHub;
pub use crate::lfd::executor::create_wave_run_with_id;
use crate::lfd::executor::WaveExecutor;
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
                wave.data_mut().status = WaveStatus::Failed;
                if let Err(err) = store.update_wave(&wave).await {
                    tracing::error!(wave_id = %run.wave_id, error = %err, "failed to update wave status");
                }
                event_hub.send(Event::wave_updated(run.wave_id.clone()));
            }
        }
    }
}
