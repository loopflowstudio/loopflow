use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::{
    enqueue_pending_activation, spawn_immediate_activation, ActivationEnvelope, ImmediateActivation,
};
use crate::engine::worktrees::worktree_path;
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{Trigger, Wave, WaveStatus};
use crate::lfd::wave::registry::live_brain_after_probe;

pub fn spawn_loop_ticker(
    scheduler: Arc<Scheduler>,
    executor: WaveExecutor,
    store: SharedStore,
    event_hub: EventHub,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("loop_ticker shutting down");
                    break;
                }
                _ = interval.tick() => {
                    tick_loop_waves(&scheduler, &executor, &store, &event_hub).await;
                }
            }
        }
    })
}

async fn tick_loop_waves(
    scheduler: &Arc<Scheduler>,
    executor: &WaveExecutor,
    store: &SharedStore,
    event_hub: &EventHub,
) {
    let waves = match store.list_loopable_waves().await {
        Ok(waves) => waves,
        Err(err) => {
            tracing::error!(error = %err, "failed to list loopable waves");
            return;
        }
    };

    for wave in waves {
        if wave.status() == WaveStatus::Paused {
            continue;
        }
        if scheduler.has_active_session(wave.id().as_str()) {
            continue;
        }

        // One brain per wave: a live WaveAgent session — lfd-launched or a
        // self-registered `lf wave` server — IS the wave's brain; the ticker
        // must not double-drive it. Same fact `run_wave_handler` checks.
        // The probing form closes ghost rows on the spot (a SIGKILL'd
        // `lf wave` server whose pid is dead), so a crashed brain cannot
        // silently stall the loop until the next lfd boot.
        match live_brain_after_probe(store, wave.id()).await {
            Ok(Some(session)) => {
                tracing::debug!(
                    wave = %wave.name(),
                    session_id = %session.id,
                    "wave has a live wave-agent session; loop tick skipped"
                );
                continue;
            }
            Ok(None) => {}
            Err(err) => {
                tracing::error!(wave_id = %wave.id(), error = %err, "failed to check wave-agent session");
                continue;
            }
        }

        let active_runs = match store.count_active_runs(wave.id()).await {
            Ok(count) => count,
            Err(err) => {
                tracing::error!(wave_id = %wave.id(), error = %err, "failed to count active loop runs");
                continue;
            }
        };
        let pending_count = match store.list_pending_activations(wave.id()).await {
            Ok(pending) => pending.len() as u32,
            Err(err) => {
                tracing::error!(wave_id = %wave.id(), error = %err, "failed to list pending activations");
                continue;
            }
        };
        if active_runs + pending_count >= wave.workers() {
            continue;
        }

        let worktree = worktree_path(Path::new(wave.repo()), wave.name());
        let wave_dir = worktree.join("wave").join(wave.name());

        // Skip if wave dir was removed (cycle complete).
        if worktree.exists() && !wave_dir.exists() {
            tracing::info!(wave = %wave.name(), "wave dir removed, skipping loop tick");
            continue;
        }

        // Safety valve: check max_iterations on any trigger for this wave.
        if let Ok(triggers) = store.list_triggers(Some(wave.id())).await {
            if let Some(trigger) = triggers.iter().find(|t| t.max_iterations.is_some()) {
                if should_pause_for_max_iterations(trigger, &wave) {
                    tracing::warn!(
                        wave = %wave.name(),
                        iteration = wave.iteration(),
                        max = trigger.max_iterations.unwrap_or(0),
                        "max iterations exceeded, pausing wave"
                    );
                    let mut paused_wave = wave.clone();
                    paused_wave.set_status(WaveStatus::Paused);
                    if let Err(err) = store.update_wave(&paused_wave).await {
                        tracing::error!(
                            wave_id = %paused_wave.id,
                            error = %err,
                            "failed to pause wave after max iterations"
                        );
                    }
                    continue;
                }
            }
        }

        let envelope = ActivationEnvelope::new(
            wave.id(),
            None,
            "loop ticker observed idle wave",
            "",
            "",
            "main",
        );
        if wave.workers() == 1 {
            let _ = enqueue_pending_activation(store, event_hub, envelope).await;
        } else if let Err(err) = spawn_immediate_activation(
            store,
            executor,
            scheduler,
            event_hub,
            ImmediateActivation {
                wave: &wave,
                flow_override: Some(wave.primary_flow().to_string()),
                roadmap_item: None,
                force_parallel: false,
                envelope,
            },
        )
        .await
        {
            tracing::warn!(wave_id = %wave.id(), error = %err, "loop activation failed");
        }
    }
}

fn should_pause_for_max_iterations(trigger: &Trigger, wave: &Wave) -> bool {
    let Some(max) = trigger.max_iterations else {
        return false;
    };
    let cycle_iterations = wave
        .iteration()
        .saturating_sub(wave.cycle_start_iteration())
        + 1;
    max > 0 && cycle_iterations >= max
}

#[cfg(test)]
mod tests {
    use super::{should_pause_for_max_iterations, tick_loop_waves};
    use crate::lfd::events::EventHub;
    use crate::lfd::executor::{AgentExecutor, ExecutionContext, WaveExecutor};
    use crate::lfd::id::LfdId;
    use crate::lfd::output::OutputHub;
    use crate::lfd::scheduler::Scheduler;
    use crate::lfd::store::{open_store, SharedStore, StorageConfig};
    use crate::lfd::types::{
        RepoWork, Session, SessionStatus, SessionUse, Signal, Trigger, Wave, WaveMode, WaveStatus,
        WAVE_SERVER_PID_ENV, WAVE_SERVER_SOURCE,
    };
    use std::collections::BTreeMap;
    use std::path::Path;
    use std::sync::Arc;
    use time::OffsetDateTime;

    fn make_trigger(max_iterations: Option<u32>) -> Trigger {
        let mut trigger = Trigger::new(LfdId::new(), LfdId::new(), Signal::Repo);
        trigger.max_iterations = max_iterations;
        trigger
    }

    fn make_wave(iteration: u32, cycle_start_iteration: u32) -> Wave {
        Wave {
            id: LfdId::new(),
            name: "loop-wave".to_string(),
            mode: WaveMode::Loop,
            primary_flow: "ship-roadmap".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            crons: Vec::new(),
            repos: vec![RepoWork {
                repo: "/tmp/repo".to_string(),
                worktree: String::new(),
                branch: String::new(),
                status: WaveStatus::Idle,
                iteration,
                cycle_start_iteration,
                position: 0,
            }],
            direction: Vec::new(),
            area: Vec::new(),
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
            parent_wave_id: None,
        }
    }

    #[test]
    fn no_max_iterations() {
        let trigger = make_trigger(None);
        let wave = make_wave(2, 0);
        assert!(!should_pause_for_max_iterations(&trigger, &wave));
    }

    #[test]
    fn max_zero() {
        let trigger = make_trigger(Some(0));
        let wave = make_wave(2, 0);
        assert!(!should_pause_for_max_iterations(&trigger, &wave));
    }

    #[test]
    fn below_limit() {
        let trigger = make_trigger(Some(5));
        let wave = make_wave(2, 0);
        assert!(!should_pause_for_max_iterations(&trigger, &wave));
    }

    #[test]
    fn at_limit() {
        let trigger = make_trigger(Some(5));
        let wave = make_wave(4, 0);
        assert!(should_pause_for_max_iterations(&trigger, &wave));
    }

    #[test]
    fn with_offset() {
        let trigger = make_trigger(Some(5));
        let wave = make_wave(7, 5);
        assert!(!should_pause_for_max_iterations(&trigger, &wave));
    }

    struct NoopRunner;

    #[async_trait::async_trait]
    impl AgentExecutor for NoopRunner {
        async fn run(
            &self,
            _cmd: Vec<String>,
            _cwd: &Path,
            _context: ExecutionContext,
        ) -> anyhow::Result<i32> {
            Ok(0)
        }

        async fn terminate(&self, _agent_id: &str) -> anyhow::Result<()> {
            Ok(())
        }
    }

    struct Ticker {
        store: SharedStore,
        scheduler: Arc<Scheduler>,
        executor: WaveExecutor,
        event_hub: EventHub,
        _tmp: tempfile::TempDir,
    }

    impl Ticker {
        async fn tick(&self) {
            tick_loop_waves(
                &self.scheduler,
                &self.executor,
                &self.store,
                &self.event_hub,
            )
            .await;
        }
    }

    async fn ticker() -> Ticker {
        let tmp = tempfile::tempdir().expect("tempdir");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(tmp.path().join("lfd.db")))
                .await
                .expect("open sqlite store"),
        );
        let scheduler = Arc::new(Scheduler::new(1));
        let output_hub = OutputHub::new(128, tmp.path().join("output"));
        let event_hub = EventHub::new(128);
        let executor = WaveExecutor::with_runner(
            store.clone(),
            scheduler.clone(),
            output_hub,
            event_hub.clone(),
            Arc::new(NoopRunner),
        );
        Ticker {
            store,
            scheduler,
            executor,
            event_hub,
            _tmp: tmp,
        }
    }

    fn loop_wave() -> Wave {
        make_wave(0, 0)
    }

    /// A `lf wave` server's self-registered WaveAgent session, recording
    /// `pid` as the running server (the ticker's brain check probes it).
    fn registered_server_session(wave: &Wave, pid: u32) -> Session {
        Session {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: None,
            parent_session_id: None,
            session_use: SessionUse::WaveAgent,
            step: "mind".to_string(),
            agent: "lf".to_string(),
            cwd: "/tmp/repo".to_string(),
            argv: vec!["lf".to_string(), "wave".to_string(), wave.name().clone()],
            env: BTreeMap::from([(WAVE_SERVER_PID_ENV.to_string(), pid.to_string())]),
            source: WAVE_SERVER_SOURCE.to_string(),
            tmux_name: String::new(),
            status: SessionStatus::Running,
            attached_at: None,
            started_at: Some(OffsetDateTime::now_utc()),
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        }
    }

    #[tokio::test]
    async fn tick_skips_wave_with_live_registered_wave_server() {
        let ticker = ticker().await;
        let wave = loop_wave();
        ticker.store.create_wave(&wave).await.expect("create wave");
        // A live server: its recorded pid is this test process — alive.
        let session = registered_server_session(&wave, std::process::id());
        ticker
            .store
            .create_control_session(&session)
            .await
            .expect("register server session");

        ticker.tick().await;
        let pending = ticker
            .store
            .list_pending_activations(wave.id())
            .await
            .expect("list pending");
        assert!(
            pending.is_empty(),
            "loop ticker must not double-drive a wave with a live registered brain"
        );

        // The server deregisters (session goes terminal) → the ticker resumes.
        let mut ended = session;
        assert!(ended.complete(0));
        ticker
            .store
            .update_control_session(&ended)
            .await
            .expect("end session");
        ticker.tick().await;
        let pending = ticker
            .store
            .list_pending_activations(wave.id())
            .await
            .expect("list pending");
        assert_eq!(pending.len(), 1, "no live brain → the loop drives again");
    }

    /// A SIGKILL'd `lf wave` server leaves a Running WaveAgent row behind.
    /// The ticker's probe must close the ghost on the spot and keep looping —
    /// not trust the row and silently stall the wave until the next lfd boot.
    #[tokio::test]
    async fn tick_closes_dead_wave_server_row_and_keeps_looping() {
        let ticker = ticker().await;
        let wave = loop_wave();
        ticker.store.create_wave(&wave).await.expect("create wave");
        // A crashed server: the recorded pid is long dead.
        let ghost = registered_server_session(&wave, 4_000_000);
        ticker
            .store
            .create_control_session(&ghost)
            .await
            .expect("register ghost session");

        ticker.tick().await;

        let closed = ticker
            .store
            .get_control_session(&ghost.id)
            .await
            .expect("lookup")
            .expect("row kept");
        assert_eq!(
            closed.status,
            SessionStatus::Failed,
            "the dead brain's row is closed by the probe"
        );
        let pending = ticker
            .store
            .list_pending_activations(wave.id())
            .await
            .expect("list pending");
        assert_eq!(
            pending.len(),
            1,
            "a dead brain must not stall the loop while lfd runs"
        );
    }
}
