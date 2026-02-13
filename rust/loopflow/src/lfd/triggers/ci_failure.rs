use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::lfd::events::EventHub;
use crate::lfd::executor::{CiFailure, WaveExecutor};
use crate::lfd::types::Event;

pub fn spawn_ci_failure_handler(
    executor: WaveExecutor,
    event_hub: EventHub,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    let mut rx = event_hub.subscribe();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("ci_failure_handler shutting down");
                    break;
                }
                event = rx.recv() => {
                    match event {
                        Ok(Event::CiFailure {
                            wave_id,
                            wave_run_id,
                            pr_number,
                            branch,
                            commit_sha,
                            check_name,
                            logs_url,
                            ..
                        }) => {
                            let executor = executor.clone();
                            let failure = CiFailure {
                                wave_id,
                                wave_run_id,
                                pr_number,
                                branch,
                                commit_sha,
                                check_name,
                                logs_url,
                            };
                            tokio::spawn(async move {
                                if let Err(err) = executor.spawn_ci_fix_agent(&failure).await {
                                    tracing::warn!(
                                        wave_id = %failure.wave_id,
                                        branch = %failure.branch,
                                        commit_sha = %failure.commit_sha,
                                        error = %err,
                                        "CI fix agent failed"
                                    );
                                }
                            });
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                        _ => {}
                    }
                }
            }
        }
    })
}
