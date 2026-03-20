use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};

use crate::engine::worktrees::{list_worktrees, main_repo_root, wave_name_from_worktree_and_main};
use crate::journal::{events_path, read_events, runs_root, LfEvent};
use crate::lfd::events::EventHub;
use crate::lfd::store::SharedStore;
use crate::lfd::types::Event;

const POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Default)]
struct LfObserver {
    line_cursors: HashMap<PathBuf, usize>,
}

impl LfObserver {
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

            let event_path = events_path(&path);
            if !event_path.exists() {
                continue;
            }

            if let Err(err) = read_events(&path) {
                debug!(error = %err, run_dir = %path.display(), "skipping invalid journal");
                continue;
            }

            self.replay_new_lines(&path, event_hub)?;
        }
        Ok(())
    }

    fn replay_new_lines(&mut self, run_dir: &Path, event_hub: &EventHub) -> Result<(), String> {
        let events_path = events_path(run_dir);
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
            let event: LfEvent = serde_json::from_str(line)
                .map_err(|err| format!("invalid journal event: {err}"))?;
            event_hub.send(Event::from(event));
        }
        self.line_cursors.insert(run_dir.to_path_buf(), processed);
        Ok(())
    }
}

pub fn spawn(store: SharedStore, event_hub: EventHub, cancel: CancellationToken) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut observer = LfObserver::default();
        let mut interval = tokio::time::interval(POLL_INTERVAL);
        interval.tick().await;

        loop {
            tokio::select! {
                _ = cancel.cancelled() => break,
                _ = interval.tick() => {
                    if let Err(err) = observer.scan(&store, &event_hub).await {
                        warn!(error = %err, "journal scan failed");
                    }
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::LfObserver;
    use crate::journal::{emit, LfEventFields, LfEventType, LfNode};
    use crate::lfd::events::EventHub;
    use loopflow_test_support::TestRepo;
    use time::OffsetDateTime;

    fn started_fields(
        command: &[String],
        worktree: &std::path::Path,
        wave_name: &str,
    ) -> LfEventFields {
        LfEventFields {
            wave_name: Some(wave_name.to_string()),
            worktree: Some(worktree.display().to_string()),
            command: Some(command.to_vec()),
            ..LfEventFields::default()
        }
    }

    #[test]
    fn observer_replays_new_journal_events() {
        let repo = TestRepo::new();
        let worktree = repo.create_wave_worktree("observe");
        let command = vec!["lf".to_string(), "build".to_string()];

        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Started,
            started_fields(&command, &worktree, "observe"),
        );
        emit(
            &worktree,
            LfNode::Flow,
            LfEventType::Started,
            LfEventFields {
                flow: Some("build".to_string()),
                ..LfEventFields::default()
            },
        );
        emit(
            &worktree,
            LfNode::Step,
            LfEventType::Started,
            LfEventFields {
                step: Some("implement".to_string()),
                index: Some(0),
                ..LfEventFields::default()
            },
        );
        emit(
            &worktree,
            LfNode::Step,
            LfEventType::Completed,
            LfEventFields {
                step: Some("implement".to_string()),
                index: Some(0),
                ..LfEventFields::default()
            },
        );
        emit(
            &worktree,
            LfNode::Flow,
            LfEventType::Completed,
            LfEventFields::default(),
        );
        emit(
            &worktree,
            LfNode::Run,
            LfEventType::Completed,
            LfEventFields::default(),
        );

        let hub = EventHub::new(8);
        let mut rx = hub.subscribe();
        let mut observer = LfObserver::default();
        observer
            .scan_worktree(&worktree, &hub)
            .expect("scan journal worktree");

        let event_types = (0..6)
            .map(|_| {
                let event = rx.try_recv().expect("event");
                let json = serde_json::to_value(&event).expect("serialize");
                json["type"].as_str().expect("type").to_string()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            event_types,
            vec![
                "run.started",
                "flow.started",
                "step.started",
                "step.completed",
                "flow.completed",
                "run.completed",
            ]
        );

        observer
            .scan_worktree(&worktree, &hub)
            .expect("rescan journal worktree");
        assert!(
            rx.try_recv().is_err(),
            "no duplicate events after replay cursor"
        );
    }

    #[test]
    fn journal_replay_preserves_original_timestamp() {
        let timestamp = OffsetDateTime::parse(
            "2026-03-20T18:30:45Z",
            &time::format_description::well_known::Rfc3339,
        )
        .expect("timestamp");
        let event = crate::journal::LfEvent {
            run_id: "2b2a93a3-59fd-413a-a95a-c9fb41351d4e"
                .parse()
                .expect("uuid"),
            ts: timestamp,
            node: LfNode::Run,
            event: LfEventType::Completed,
            wave_name: None,
            worktree: None,
            command: None,
            flow: None,
            step: None,
            index: None,
            error: None,
            signal: None,
        };

        let mapped = crate::lfd::types::Event::from(event);
        let json = serde_json::to_value(&mapped).expect("serialize");
        assert_eq!(json["type"], "run.completed");
        assert_eq!(json["timestamp"], "2026-03-20T18:30:45Z");
    }
}
