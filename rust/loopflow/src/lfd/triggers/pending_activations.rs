use std::collections::HashSet;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::common::{create_wave_run_with_id, spawn_run_task_with_slot};
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::id::LfdId;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
use crate::lfd::types::WaveStatus;

pub fn spawn_pending_activation_drain(
    store: SharedStore,
    executor: WaveExecutor,
    scheduler: std::sync::Arc<Scheduler>,
    event_hub: EventHub,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("pending_activation_drain shutting down");
                    break;
                }
                _ = interval.tick() => {
                    drain_pending_activations_once(&store, &executor, &scheduler, &event_hub).await;
                }
            }
        }
    })
}

pub(super) async fn drain_pending_activations_once(
    store: &SharedStore,
    executor: &WaveExecutor,
    scheduler: &std::sync::Arc<Scheduler>,
    event_hub: &EventHub,
) {
    let stimuli = match store.list_stimuli(None).await {
        Ok(stimuli) => stimuli,
        Err(err) => {
            tracing::error!(error = %err, "failed to list stimuli for pending activation drain");
            return;
        }
    };
    let mut started = HashSet::new();

    for stimulus in stimuli {
        if !stimulus.enabled || started.contains(&stimulus.wave_id) {
            continue;
        }
        let has_pending = match store
            .get_pending_for_stimulus(&stimulus.wave_id, &stimulus.id)
            .await
        {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(err) => {
                tracing::warn!(
                    wave_id = %stimulus.wave_id,
                    stimulus_id = %stimulus.id,
                    error = %err,
                    "pending activation lookup failed"
                );
                false
            }
        };
        if !has_pending {
            continue;
        }

        let wave = match store.get_wave(&stimulus.wave_id).await {
            Ok(Some(wave)) => wave,
            Ok(None) => continue,
            Err(err) => {
                tracing::warn!(
                    wave_id = %stimulus.wave_id,
                    error = %err,
                    "failed to load wave during pending activation drain"
                );
                continue;
            }
        };
        if wave.status() == WaveStatus::Paused {
            continue;
        }
        if store
            .get_active_wave_run(wave.id())
            .await
            .ok()
            .flatten()
            .is_some()
        {
            continue;
        }

        let run_id = LfdId::new();
        let slot_guard = match scheduler.acquire_guard(run_id.as_str()).await {
            Ok(guard) => guard,
            Err(_) => continue,
        };

        let run = match create_wave_run_with_id(store, &wave, &run_id).await {
            Ok(run) => run,
            Err(err) => {
                tracing::warn!(wave_id = %wave.id(), error = %err, "failed to create run while draining pending activations");
                continue;
            }
        };

        if let Err(err) = store.delete_pending_activations(wave.id()).await {
            tracing::warn!(
                wave_id = %wave.id(),
                error = %err,
                "failed deleting pending activations after drain start"
            );
        }

        started.insert(wave.id().clone());
        spawn_run_task_with_slot(
            store.clone(),
            executor.clone(),
            event_hub.clone(),
            run,
            slot_guard,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::sync::Arc;

    use anyhow::Result;
    use async_trait::async_trait;
    use chrono::Utc;
    use loopflow_test_support::TestRepo;
    use tempfile::tempdir;
    use time::OffsetDateTime;

    use super::*;
    use crate::lfd::executor::AgentExecutor;
    use crate::lfd::output::OutputHub;
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{
        ActivationSource, PendingActivation, Stimulus, StimulusKind, Wave, WaveRun, WaveRunKind,
        WaveRunStatus, WaveStatus,
    };

    struct MockRunner;

    #[async_trait]
    impl AgentExecutor for MockRunner {
        async fn run(
            &self,
            _cmd: Vec<String>,
            _cwd: &Path,
            _context: crate::lfd::executor::AgentRunContext<'_>,
        ) -> Result<i32> {
            Ok(0)
        }

        async fn terminate(&self, _agent_id: &str) -> Result<()> {
            Ok(())
        }
    }

    fn make_wave(name: &str, repo: &str) -> Wave {
        Wave {
            id: LfdId::new(),
            name: name.to_string(),
            repo: repo.to_string(),
            flow: "test-flow".to_string(),
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
        }
    }

    #[tokio::test]
    async fn listen_queue_drain_starts_pending_wave_run() {
        let repo = TestRepo::new();
        repo.create_file(".lf/flows/test-flow.yaml", "- step-a\n");
        repo.create_file(".lf/steps/step-a.md", "do step-a");
        repo.stage_all();
        repo.commit("add flow fixtures");

        let db_path = tempdir().expect("tempdir").path().join("test.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );

        let source_wave = make_wave("infra", &repo.path().to_string_lossy());
        let listening_wave = make_wave("designer", &repo.path().to_string_lossy());
        store
            .create_wave(&source_wave)
            .await
            .expect("create source wave");
        store
            .create_wave(&listening_wave)
            .await
            .expect("create listening wave");

        let stimulus = Stimulus {
            id: LfdId::new(),
            wave_id: listening_wave.id().clone(),
            source_wave_id: Some(source_wave.id().clone()),
            kind: StimulusKind::Listen,
            cron: None,
            last_main_sha: None,
            last_triggered_at: None,
            created_at: Some(OffsetDateTime::now_utc()),
            enabled: true,
        };
        store
            .create_stimulus(&stimulus)
            .await
            .expect("create listen stimulus");
        let activation = PendingActivation {
            id: LfdId::new(),
            wave_id: listening_wave.id().clone(),
            stimulus_id: stimulus.id.clone(),
            source: ActivationSource::Listen,
            reason: "listen completion".to_string(),
            from_sha: String::new(),
            to_sha: String::new(),
            queued_at: Utc::now().timestamp(),
        };
        store
            .create_pending_activation(&activation)
            .await
            .expect("queue pending activation");

        let scheduler = Arc::new(Scheduler::new(1));
        let output_dir = tempdir().expect("output dir");
        let output = OutputHub::new(16, output_dir.path().to_path_buf());
        let event_hub = EventHub::new(64);
        let executor = WaveExecutor::with_runner(
            store.clone(),
            scheduler.clone(),
            output,
            event_hub.clone(),
            Arc::new(MockRunner),
        );

        drain_pending_activations_once(&store, &executor, &scheduler, &event_hub).await;

        let pending = store
            .list_pending_activations(listening_wave.id())
            .await
            .expect("list pending activations");
        assert!(pending.is_empty(), "pending activation should be drained");

        let runs = store
            .list_wave_runs(Some(listening_wave.id()), None)
            .await
            .expect("list listener runs");
        assert!(
            runs.iter()
                .any(|run: &WaveRun| run.run_kind == WaveRunKind::Main),
            "listener run should be created"
        );

        tokio::time::sleep(Duration::from_millis(50)).await;
        let latest = store
            .get_latest_wave_run(listening_wave.id())
            .await
            .expect("latest run query should succeed")
            .expect("latest run should exist");
        assert!(
            matches!(
                latest.status,
                WaveRunStatus::Running | WaveRunStatus::Completed
            ),
            "listener run should have started"
        );
    }
}
