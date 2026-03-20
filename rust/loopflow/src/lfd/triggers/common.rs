use std::time::Duration;

use time::OffsetDateTime;

use crate::lfd::attention::create_step_failure_attention;
use crate::lfd::events::EventHub;
use crate::lfd::executor::wave::classify_repair_flow;
use crate::lfd::executor::WaveExecutor;
pub use crate::lfd::executor::{create_parallel_wave_run, create_wave_run_with_id};
use crate::lfd::scheduler::SchedulerSlotGuard;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{Event, WaveRun, WaveRunStatus, WaveStatus};

/// Fixed backoff delays between repair attempts, indexed by chain depth.
/// The array length defines the maximum number of repair attempts before
/// escalating to an algedonic signal.
const REPAIR_DELAYS: [Duration; 3] = [
    Duration::from_secs(30),
    Duration::from_secs(60),
    Duration::from_secs(120),
];

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
    if run.status != WaveRunStatus::Failed {
        return;
    }

    let wave = match store.get_wave(&run.wave_id).await {
        Ok(Some(w)) => w,
        _ => return,
    };

    // Loop: check depth → backoff → repair → re-check. Exits on success,
    // escalation (depth >= limit), or unrecoverable error.
    let mut failed_run = run;
    loop {
        let depth = count_repair_chain(store, &failed_run).await;
        if depth >= REPAIR_DELAYS.len() {
            let step_name = failed_run.error.as_deref().unwrap_or("unknown");
            tracing::info!(
                run_id = %failed_run.id,
                wave_id = %wave.id(),
                depth,
                "repair limit reached, creating algedonic signal"
            );
            match create_step_failure_attention(
                store,
                &wave,
                &failed_run,
                "repair",
                &format!("repair failed after {depth} attempts: {step_name}"),
            )
            .await
            {
                Ok(item) => event_hub.send(Event::attention_created(item)),
                Err(err) => {
                    tracing::warn!(
                        wave_id = %wave.id(),
                        error = %err,
                        "failed to create algedonic signal after repair limit"
                    );
                }
            }
            return;
        }

        let delay = REPAIR_DELAYS[depth];
        tracing::info!(
            run_id = %failed_run.id,
            wave_id = %wave.id(),
            depth,
            delay_secs = delay.as_secs(),
            "waiting before dispatching repair attempt"
        );
        tokio::time::sleep(delay).await;

        let repair_flow = classify_repair_flow(&failed_run);
        tracing::info!(
            run_id = %failed_run.id,
            wave_id = %wave.id(),
            repair_flow = %repair_flow,
            depth = depth + 1,
            "dispatching headless repair attempt"
        );
        let repair_run = match executor
            .create_repair_run(&wave, &failed_run, &repair_flow)
            .await
        {
            Ok(r) => r,
            Err(err) => {
                tracing::error!(
                    run_id = %failed_run.id,
                    error = %err,
                    "failed to create repair run"
                );
                return;
            }
        };

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
            return;
        }

        // Re-read the repair run to check its outcome.
        let repair_run = match store.get_wave_run(&repair_run.id).await {
            Ok(Some(r)) => r,
            _ => return,
        };
        if repair_run.status != WaveRunStatus::Failed {
            return; // Repair succeeded (or is in a non-failed terminal state).
        }

        // Repair failed — loop back to check depth and try again.
        failed_run = repair_run;
    }
}

/// Walk the `repair_of` chain backwards to count how many repair attempts
/// have been made for the original failure. Returns 0 for original runs,
/// 1 for the first repair, etc.
async fn count_repair_chain(store: &SharedStore, run: &WaveRun) -> usize {
    let mut depth = 0usize;
    let mut current_repair_of = run.repair_of.clone();
    while let Some(parent_id) = current_repair_of {
        depth += 1;
        match store.get_wave_run(&parent_id).await {
            Ok(Some(parent)) => current_repair_of = parent.repair_of.clone(),
            _ => break,
        }
    }
    depth
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::Arc;

    use time::OffsetDateTime;

    use crate::lfd::id::LfdId;
    use crate::lfd::store::{open_store, SharedStore, StorageConfig};
    use crate::lfd::types::{
        Wave, WaveMode, WaveRun, WaveRunSnapshot, WaveRunStackStatus, WaveStatus,
    };

    async fn test_store() -> SharedStore {
        let db_path = std::env::temp_dir().join(format!("lfd-test-{}.db", LfdId::new()));
        Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        )
    }

    fn make_wave() -> Wave {
        let id = LfdId::new();
        Wave {
            id: id.clone(),
            name: format!("wave-{id}"),
            repo: ".".to_string(),
            mode: WaveMode::Manual,
            primary_flow: "build".to_string(),
            cron: None,
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Failed,
            iteration: 0,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
        }
    }

    fn make_run(wave: &Wave) -> WaveRun {
        WaveRun {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            snapshot: WaveRunSnapshot {
                repo: ".".to_string(),
                flow: "build".to_string(),
                direction: Vec::new(),
                area: Vec::new(),
            },
            iteration: 0,
            step_index: 0,
            status: WaveRunStatus::Failed,
            worktree: ".".to_string(),
            branch: "test".to_string(),
            started_at: Some(OffsetDateTime::now_utc()),
            ended_at: None,
            error: Some("test error".to_string()),
            flow_parents: Vec::new(),
            activation_log_id: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: wave.id().to_string(),
            stack_status: WaveRunStackStatus::Active,
            lineage_inferred: false,
            target_branch: "main".to_string(),
            repair_of: None,
            pr: None,
        }
    }

    #[tokio::test]
    async fn count_repair_chain_zero_for_original() {
        let store = test_store().await;
        let wave = make_wave();
        store.create_wave(&wave).await.unwrap();
        let run = make_run(&wave);
        store.create_wave_run(&run).await.unwrap();

        assert_eq!(count_repair_chain(&store, &run).await, 0);
    }

    #[tokio::test]
    async fn count_repair_chain_counts_depth() {
        let store = test_store().await;
        let wave = make_wave();
        store.create_wave(&wave).await.unwrap();

        // Original run
        let original = make_run(&wave);
        store.create_wave_run(&original).await.unwrap();

        // Repair 1
        let mut repair1 = make_run(&wave);
        repair1.repair_of = Some(original.id.clone());
        store.create_wave_run(&repair1).await.unwrap();

        // Repair 2
        let mut repair2 = make_run(&wave);
        repair2.repair_of = Some(repair1.id.clone());
        store.create_wave_run(&repair2).await.unwrap();

        // Repair 3
        let mut repair3 = make_run(&wave);
        repair3.repair_of = Some(repair2.id.clone());
        store.create_wave_run(&repair3).await.unwrap();

        assert_eq!(count_repair_chain(&store, &original).await, 0);
        assert_eq!(count_repair_chain(&store, &repair1).await, 1);
        assert_eq!(count_repair_chain(&store, &repair2).await, 2);
        assert_eq!(count_repair_chain(&store, &repair3).await, 3);
    }
}
