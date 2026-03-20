use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::engine::worktrees::{list_worktrees, main_repo_root, wave_name_from_worktree_and_main};
use crate::lfd::events::EventHub;
use crate::lfd::store::SharedStore;
use crate::lfd::types::Event;
use crate::runtime::{read_meta, runs_root, RuntimeEvent};

const POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
struct RuntimeJournalObserver {
    line_cursors: HashMap<PathBuf, usize>,
}

impl RuntimeJournalObserver {
    async fn scan(&mut self, store: &SharedStore, event_hub: &EventHub) -> Result<(), String> {
        let repo_roots = store
            .list_waves(None)
            .await
            .map_err(|err| err.to_string())?
            .into_iter()
            .map(|wave| PathBuf::from(wave.repo()))
            .collect::<HashSet<_>>();

        for repo_root in repo_roots {
            self.scan_repo(&repo_root, event_hub).await?;
        }
        Ok(())
    }

    async fn scan_repo(&mut self, repo_root: &Path, event_hub: &EventHub) -> Result<(), String> {
        let main_repo = main_repo_root(repo_root).map_err(|err| err.to_string())?;
        let worktrees = tokio::task::spawn_blocking({
            let repo = main_repo.clone();
            move || list_worktrees(&repo)
        })
        .await
        .map_err(|err| err.to_string())?
        .map_err(|err| err.to_string())?;

        for worktree in worktrees {
            if wave_name_from_worktree_and_main(&worktree.path, &main_repo).is_none() {
                continue;
            }
            self.scan_worktree(&worktree.path, event_hub)?;
        }
        Ok(())
    }

    fn scan_worktree(&mut self, worktree: &Path, event_hub: &EventHub) -> Result<(), String> {
        let root = runs_root(worktree);
        if !root.exists() {
            return Ok(());
        }

        let entries = std::fs::read_dir(&root)
            .map_err(|err| format!("failed to read {}: {err}", root.display()))?;
        for entry in entries {
            let entry = entry.map_err(|err| err.to_string())?;
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            if let Err(err) = read_meta(&path) {
                debug!(error = %err, run_dir = %path.display(), "skipping invalid runtime journal");
                continue;
            }

            self.replay_new_lines(&path, event_hub)?;
        }
        Ok(())
    }

    fn replay_new_lines(&mut self, run_dir: &Path, event_hub: &EventHub) -> Result<(), String> {
        let events_path = crate::runtime::events_path(run_dir);
        if !events_path.exists() {
            return Ok(());
        }

        let content = std::fs::read_to_string(&events_path)
            .map_err(|err| format!("failed reading {}: {err}", events_path.display()))?;
        let seen = self.line_cursors.get(run_dir).copied().unwrap_or(0);
        let mut processed = 0_usize;
        for line in content.lines() {
            processed += 1;
            if processed <= seen || line.trim().is_empty() {
                continue;
            }
            let event: RuntimeEvent = serde_json::from_str(line)
                .map_err(|err| format!("invalid runtime event: {err}"))?;
            event_hub.send(map_runtime_event(event));
        }
        self.line_cursors.insert(run_dir.to_path_buf(), processed);
        Ok(())
    }
}

fn map_runtime_event(event: RuntimeEvent) -> Event {
    match event {
        RuntimeEvent::RunStarted {
            run_id,
            command,
            worktree,
            wave_name,
            flow,
            step,
            ..
        } => Event::run_started(run_id, wave_name, worktree, command, flow, step),
        RuntimeEvent::StepStarted {
            run_id,
            step,
            index,
            ..
        } => Event::step_started(run_id, step, index),
        RuntimeEvent::StepCompleted {
            run_id,
            step,
            index,
            exit_code,
            ..
        } => Event::step_completed(run_id, step, index, exit_code),
        RuntimeEvent::RunWaiting { run_id, step, .. } => Event::run_waiting(run_id, step),
        RuntimeEvent::RunCompleted {
            run_id, exit_code, ..
        } => Event::run_completed(run_id, exit_code),
        RuntimeEvent::RunFailed {
            run_id,
            exit_code,
            error,
            ..
        } => Event::run_failed(run_id, exit_code, error),
    }
}

pub fn spawn(store: SharedStore, event_hub: EventHub, cancel: CancellationToken) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut observer = RuntimeJournalObserver::default();
        let mut interval = tokio::time::interval(POLL_INTERVAL);
        interval.tick().await;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = interval.tick() => {
                    if let Err(err) = observer.scan(&store, &event_hub).await {
                        warn!(error = %err, "runtime journal scan failed");
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::{map_runtime_event, RuntimeJournalObserver};
    use crate::lfd::events::EventHub;
    use crate::runtime::{RunTarget, RuntimeEvent, RuntimeRun};
    use loopflow_test_support::TestRepo;

    #[test]
    fn runtime_event_maps_to_client_event() {
        let event = map_runtime_event(RuntimeEvent::RunCompleted {
            run_id: "2b2a93a3-59fd-413a-a95a-c9fb41351d4e"
                .parse()
                .expect("uuid"),
            timestamp: time::OffsetDateTime::now_utc(),
            exit_code: 0,
        });
        let json = serde_json::to_value(&event).expect("serialize");
        assert_eq!(json["type"], "run.completed");
        assert_eq!(json["exit_code"], 0);
    }

    #[test]
    fn observer_replays_new_runtime_events() {
        let repo = TestRepo::new();
        let worktree = repo.create_wave_worktree("observe");
        let run = RuntimeRun::maybe_start(
            &worktree,
            &["lf".to_string(), "build".to_string()],
            RunTarget::flow("build"),
        )
        .expect("runtime run");
        run.emit_step_started("implement", 0);
        run.complete_success();

        let hub = EventHub::new(8);
        let mut rx = hub.subscribe();
        let mut observer = RuntimeJournalObserver::default();
        observer
            .scan_worktree(&worktree, &hub)
            .expect("scan runtime worktree");

        let event_types = (0..3)
            .map(|_| {
                let event = rx.try_recv().expect("event");
                let json = serde_json::to_value(&event).expect("serialize");
                json["type"].as_str().expect("type").to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            event_types,
            vec!["run.started", "step.started", "run.completed"]
        );

        observer
            .scan_worktree(&worktree, &hub)
            .expect("rescan runtime worktree");
        assert!(
            rx.try_recv().is_err(),
            "no duplicate events after replay cursor"
        );
    }
}
