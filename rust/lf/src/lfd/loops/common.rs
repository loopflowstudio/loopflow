use std::sync::Arc;

use crate::lfd::executor::WaveExecutor;
use crate::lfd::id::LfdId;
use crate::lfd::proto::control::{WaveRun, WaveRunStatus};
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;

pub fn now_timestamp() -> prost_types::Timestamp {
    let now = time::OffsetDateTime::now_utc();
    prost_types::Timestamp {
        seconds: now.unix_timestamp(),
        nanos: 0,
    }
}

pub fn create_wave_run_with_id(
    store: &SharedStore,
    wave: &crate::lfd::proto::control::Wave,
    run_id: &LfdId,
) -> anyhow::Result<WaveRun> {
    let wave_id = LfdId::parse(&wave.id)?;
    let last_run = store
        .list_wave_runs(Some(&wave_id), Some(1))?
        .into_iter()
        .next();
    let iteration = last_run.map(|run| run.iteration + 1).unwrap_or(0);

    let run = WaveRun {
        id: run_id.to_string(),
        wave_id: wave.id.clone(),
        iteration,
        step_index: 0,
        status: WaveRunStatus::WaveRunRunning as i32,
        worktree: wave.repo.clone(),
        branch: String::new(),
        started_at: Some(now_timestamp()),
        ended_at: None,
        error: None,
        flow_parents: Vec::new(),
    };
    store.create_wave_run(&run)?;
    Ok(run)
}

/// Spawn a task that executes a wave run and releases a scheduler slot on completion.
pub fn spawn_run_task_with_slot(
    store: SharedStore,
    executor: WaveExecutor,
    scheduler: Arc<Scheduler>,
    run: WaveRun,
) {
    let run_id_for_release = run.id.clone();
    tokio::spawn(async move {
        execute_run_inner(&store, &executor, &run).await;
        scheduler.release(&run_id_for_release);
    });
}

async fn execute_run_inner(store: &SharedStore, executor: &WaveExecutor, run: &WaveRun) {
    let run_id = LfdId::parse(&run.id).expect("run id should be valid");
    if let Err(err) = executor.execute(&run_id).await {
        tracing::error!(run_id = %run.id, error = %err, "run execution failed");
        if let Ok(Some(mut run)) = store.get_wave_run(&run_id) {
            run.status = WaveRunStatus::WaveRunFailed as i32;
            run.error = Some(err.to_string());
            run.ended_at = Some(now_timestamp());
            let _ = store.update_wave_run(&run);
        }
    }
}
