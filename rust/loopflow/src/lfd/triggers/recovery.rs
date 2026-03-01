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
    let stuck_runs = match store.get_stuck_agents(stuck_threshold).await {
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
        let _ = store
            .end_agent(&agent.id, AgentStatus::Failed.as_i32(), now)
            .await;

        if let Some(ref run_id) = agent.wave_run_id {
            if let Ok(Some(mut run)) = store.get_wave_run(run_id).await {
                run.status = WaveRunStatus::Failed;
                run.error = Some("agent stuck >4h".to_string());
                run.ended_at = Some(OffsetDateTime::now_utc());
                if let Err(err) = store.update_wave_run(&run).await {
                    tracing::error!(run_id = %run.id, error = %err, "failed to update stuck run status");
                }
                if let Ok(Some(mut wave)) = store.get_wave(&run.wave_id).await {
                    wave.status = WaveStatus::Failed;
                    if let Err(err) = store.update_wave(&wave).await {
                        tracing::error!(wave_id = %run.wave_id, error = %err, "failed to update wave status");
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::lfd::id::LfdId;
    use crate::lfd::store::{open_store, SharedStore, StorageConfig};
    use crate::lfd::types::{AgentRun, AgentStatus};
    use std::sync::Arc;
    use tempfile::tempdir;
    use time::{Duration, OffsetDateTime};

    fn make_agent(hours_ago: i64) -> AgentRun {
        AgentRun {
            id: LfdId::new(),
            step: "implement".to_string(),
            repo: "/tmp/repo".to_string(),
            worktree: "/tmp/worktree".to_string(),
            wave_run_id: None,
            status: AgentStatus::Running,
            started_at: Some(OffsetDateTime::now_utc() - Duration::hours(hours_ago)),
            ended_at: None,
            pid: None,
            container_id: None,
            agent: "claude".to_string(),
            run_mode: "headless".to_string(),
        }
    }

    async fn test_store() -> SharedStore {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        )
    }

    #[tokio::test]
    async fn agent_over_threshold_returned() {
        let store = test_store().await;
        let old_agent = make_agent(5);
        store
            .start_agent(&old_agent)
            .await
            .expect("insert old agent");

        let stuck = store
            .get_stuck_agents(4 * 60 * 60)
            .await
            .expect("query stuck agents");
        assert_eq!(stuck.len(), 1);
        assert_eq!(stuck[0].id, old_agent.id);
    }

    #[tokio::test]
    async fn agent_under_threshold_excluded() {
        let store = test_store().await;
        let recent_agent = make_agent(3);
        store
            .start_agent(&recent_agent)
            .await
            .expect("insert recent agent");

        let stuck = store
            .get_stuck_agents(4 * 60 * 60)
            .await
            .expect("query stuck agents");
        assert!(stuck.is_empty());
    }

    #[tokio::test]
    async fn multiple_agents_independent() {
        let store = test_store().await;
        let old_agent = make_agent(5);
        let recent_agent = make_agent(3);
        store
            .start_agent(&old_agent)
            .await
            .expect("insert old agent");
        store
            .start_agent(&recent_agent)
            .await
            .expect("insert recent agent");

        let stuck = store
            .get_stuck_agents(4 * 60 * 60)
            .await
            .expect("query stuck agents");
        assert_eq!(stuck.len(), 1);
        assert_eq!(stuck[0].id, old_agent.id);
    }
}
