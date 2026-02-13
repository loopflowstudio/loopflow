use std::time::Duration;

use time::OffsetDateTime;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::engine::platform::kill_process;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{AgentStatus, WaveRunStatus, WaveStatus};

pub fn spawn_recovery_loop(
    store: SharedStore,
    executor: WaveExecutor,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("recovery_loop shutting down");
                    break;
                }
                _ = interval.tick() => {
                    recover_stuck_runs(&store, &executor).await;
                }
            }
        }
    })
}

async fn recover_stuck_runs(store: &SharedStore, executor: &WaveExecutor) {
    let stuck_threshold = 4 * 60 * 60;
    let stuck_runs = match store.get_stuck_agents(stuck_threshold) {
        Ok(runs) => runs,
        Err(err) => {
            tracing::error!(error = %err, "failed to query stuck runs");
            return;
        }
    };

    for agent in stuck_runs {
        tracing::warn!(agent_id = %agent.id, pid = ?agent.pid, "step run stuck >4h, terminating");

        if let Err(err) = executor.terminate_agent(&agent.id).await {
            tracing::warn!(agent_id = %agent.id, error = %err, "failed to terminate via executor");
        }

        if let Some(pid) = agent.pid {
            kill_process(pid);
        }

        let now = OffsetDateTime::now_utc().unix_timestamp();
        let _ = store.end_agent(&agent.id, AgentStatus::Failed.as_i32(), now);

        if let Some(ref run_id) = agent.wave_run_id {
            if let Ok(Some(mut run)) = store.get_wave_run(run_id) {
                run.status = WaveRunStatus::Failed;
                run.error = Some("agent stuck >4h".to_string());
                run.ended_at = Some(OffsetDateTime::now_utc());
                if let Err(err) = store.update_wave_run(&run) {
                    tracing::error!(run_id = %run.id, error = %err, "failed to update stuck run status");
                }
                if let Ok(Some(mut wave)) = store.get_wave(&run.wave_id) {
                    wave.status = WaveStatus::Failed;
                    if let Err(err) = store.update_wave(&wave) {
                        tracing::error!(wave_id = %run.wave_id, error = %err, "failed to update wave status");
                    }
                }
            }
        }
    }
}
