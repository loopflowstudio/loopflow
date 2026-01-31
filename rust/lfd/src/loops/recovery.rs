use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::id::LfdId;
use crate::proto::control::{AgentStatus, WaveStatus};
use crate::store::SharedStore;

pub fn spawn_recovery_loop(store: SharedStore, cancel: CancellationToken) -> JoinHandle<()> {
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

    for run in stuck_runs {
        tracing::warn!(agent_id = %run.id, pid = ?run.pid, "step run stuck >4h, terminating");

        if let Some(pid) = run.pid {
            let _ = kill_process(pid);
        }

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_secs() as i64;
        let agent_id = LfdId::from_raw(run.id.clone());
        let _ = store.end_agent(&agent_id, AgentStatus::AgentFailed as i32, now);

        if let Some(wave_id) = run.wave_id.as_deref() {
            let wave_id = match LfdId::parse(wave_id) {
                Ok(id) => id,
                Err(err) => {
                    tracing::warn!(wave_id = %wave_id, error = %err, "invalid wave id");
                    continue;
                }
            };
            if let Ok(Some(mut wave)) = store.get_wave(&wave_id) {
                wave.consecutive_failures += 1;
                if wave.consecutive_failures >= 3 {
                    wave.status = WaveStatus::WaveError as i32;
                    tracing::error!(wave_id = %wave_id, "wave entered error after 3 failures");
                }
                let _ = store.update_wave(&wave);
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
