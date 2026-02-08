use std::sync::Arc;

use time::OffsetDateTime;

pub use crate::lfd::executor::create_wave_run_with_id;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{WaveRunStatus, WaveStatus};

/// Spawn a task that executes a wave run and releases a scheduler slot on completion.
pub fn spawn_run_task_with_slot(
    store: SharedStore,
    executor: WaveExecutor,
    scheduler: Arc<Scheduler>,
    run: crate::lfd::types::WaveRun,
) {
    let run_id_for_release = run.id.clone();
    tokio::spawn(async move {
        execute_run_inner(&store, &executor, &run).await;
        scheduler.release(run_id_for_release.as_str());
    });
}

async fn execute_run_inner(
    store: &SharedStore,
    executor: &WaveExecutor,
    run: &crate::lfd::types::WaveRun,
) {
    if let Err(err) = executor.execute(&run.id).await {
        tracing::error!(run_id = %run.id, error = %err, "run execution failed");
        if let Ok(Some(mut run)) = store.get_wave_run(&run.id) {
            run.status = WaveRunStatus::Failed;
            run.error = Some(err.to_string());
            run.ended_at = Some(OffsetDateTime::now_utc());
            let _ = store.update_wave_run(&run);
            if let Ok(Some(mut wave)) = store.get_wave(&run.wave_id) {
                wave.status = WaveStatus::Failed;
                let _ = store.update_wave(&wave);
            }
        }
    }
}
