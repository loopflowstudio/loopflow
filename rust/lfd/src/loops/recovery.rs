use std::process::Command;
use std::time::Duration;

use time::OffsetDateTime;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::executor::WaveExecutor;
use crate::store::SharedStore;
use crate::types::{AgentStatus, WaveRunStatus, WaveStatus};

pub fn spawn_recovery_loop(
    store: SharedStore,
    _executor: WaveExecutor,
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
                    recover_stuck_runs(&store);
                }
            }
        }
    })
}

fn recover_stuck_runs(store: &SharedStore) {
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

        if let Some(pid) = agent.pid {
            let _ = kill_process(pid);
        }

        let now = OffsetDateTime::now_utc().unix_timestamp();
        let _ = store.end_agent(&agent.id, AgentStatus::Failed.as_i32(), now);

        if let Some(ref run_id) = agent.wave_run_id {
            if let Ok(Some(mut run)) = store.get_wave_run(run_id) {
                run.status = WaveRunStatus::Failed;
                run.error = Some("agent stuck >4h".to_string());
                run.ended_at = Some(OffsetDateTime::now_utc());
                let _ = store.update_wave_run(&run);
                if let Ok(Some(mut wave)) = store.get_wave(&run.wave_id) {
                    wave.status = WaveStatus::Failed;
                    let _ = store.update_wave(&wave);
                }
            }
        }
    }
}

fn kill_process(pid: u32) -> std::io::Result<()> {
    Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .status()?;
    Ok(())
}
