use std::process::Command;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::executor::WaveExecutor;
use crate::id::LfdId;
use crate::proto::control::{AgentStatus, WaveRunStatus};
use crate::store::SharedStore;

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

        if let Some(run_id) = run.wave_run_id.as_deref() {
            if let Ok(run_id) = LfdId::parse(run_id) {
                if let Ok(Some(mut run)) = store.get_wave_run(&run_id) {
                    run.status = WaveRunStatus::WaveRunFailed as i32;
                    run.error = Some("agent stuck >4h".to_string());
                    run.ended_at = Some(now_timestamp());
                    let _ = store.update_wave_run(&run);
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

fn now_timestamp() -> prost_types::Timestamp {
    let now = time::OffsetDateTime::now_utc();
    prost_types::Timestamp {
        seconds: now.unix_timestamp(),
        nanos: 0,
    }
}
