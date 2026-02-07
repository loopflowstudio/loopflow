use std::sync::Arc;

use time::OffsetDateTime;

use crate::executor::WaveExecutor;
use crate::id::LfdId;
use crate::scheduler::Scheduler;
use crate::store::SharedStore;
use crate::types::{Wave, WaveRun, WaveRunSnapshot, WaveRunStatus, WaveStatus};

pub fn create_wave_run_with_id(
    store: &SharedStore,
    wave: &Wave,
    run_id: &LfdId,
) -> anyhow::Result<WaveRun> {
    let last_run = store
        .list_wave_runs(Some(&wave.id), Some(1))?
        .into_iter()
        .next();
    let iteration = last_run.map(|run| run.iteration + 1).unwrap_or(0);

    let run = WaveRun {
        id: run_id.clone(),
        wave_id: wave.id.clone(),
        snapshot: WaveRunSnapshot {
            repo: wave.repo.clone(),
            flow: wave.flow.clone(),
            direction: wave.direction.clone(),
            area: wave.area.clone(),
            pr: None,
        },
        iteration,
        step_index: 0,
        status: WaveRunStatus::Running,
        worktree: wave.repo.clone(),
        branch: String::new(),
        started_at: Some(OffsetDateTime::now_utc()),
        ended_at: None,
        error: None,
        flow_parents: Vec::new(),
    };
    store.create_wave_run(&run)?;
    if let Ok(Some(mut wave)) = store.get_wave(&wave.id) {
        wave.status = WaveStatus::Running;
        wave.iteration = iteration;
        let _ = store.update_wave(&wave);
    }
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
        scheduler.release(run_id_for_release.as_str());
    });
}

async fn execute_run_inner(store: &SharedStore, executor: &WaveExecutor, run: &WaveRun) {
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
