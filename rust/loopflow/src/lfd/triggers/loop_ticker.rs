use std::ffi::OsStr;
use std::path::Path;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::common::{create_wave_run_with_id, spawn_run_task_with_slot};
use crate::engine::worktrees::worktree_path;
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::id::LfdId;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{StimulusKind, WaveStatus};

pub fn spawn_loop_ticker(
    scheduler: std::sync::Arc<Scheduler>,
    store: SharedStore,
    executor: WaveExecutor,
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
                    tick_loop_waves(&scheduler, &store, &executor, &event_hub).await;
                }
            }
        }
    })
}

async fn tick_loop_waves(
    scheduler: &std::sync::Arc<Scheduler>,
    store: &SharedStore,
    executor: &WaveExecutor,
    event_hub: &EventHub,
) {
    let stimuli = match store
        .list_stimuli_by_kind(StimulusKind::Loop.as_i32())
        .await
    {
        Ok(stimuli) => stimuli,
        Err(err) => {
            tracing::error!(error = %err, "failed to list loop stimuli");
            return;
        }
    };

    for stimulus in stimuli {
        if !stimulus.enabled {
            continue;
        }

        let wave = match store.get_wave(&stimulus.wave_id).await {
            Ok(Some(wave)) => wave,
            Ok(None) => continue,
            Err(err) => {
                tracing::error!(wave_id = %stimulus.wave_id, error = %err, "failed to load wave");
                continue;
            }
        };

        if wave.status() == WaveStatus::Paused {
            continue;
        }

        if scheduler.has_active_session(wave.id().as_str()) {
            continue;
        }

        if let Ok(Some(_)) = store.get_active_wave_run(&stimulus.wave_id).await {
            continue;
        }

        let worktree = worktree_path(Path::new(wave.repo()), wave.name());
        if worktree.exists() && wave_backlog_empty(&worktree, wave.name()) {
            tracing::info!(wave = %wave.name(), "wave backlog empty, skipping loop tick");
            continue;
        }

        let run_id = LfdId::new();
        let slot_guard = match scheduler.acquire_guard(run_id.as_str()).await {
            Ok(guard) => guard,
            Err(reason) => {
                tracing::warn!(wave_id = %wave.id(), %reason, "scheduler at capacity; loop tick deferred");
                continue;
            }
        };

        let run = match create_wave_run_with_id(store, &wave, &run_id).await {
            Ok(run) => run,
            Err(err) => {
                tracing::error!(wave_id = %wave.id(), error = %err, "failed to create wave run");
                continue;
            }
        };

        spawn_run_task_with_slot(
            store.clone(),
            executor.clone(),
            event_hub.clone(),
            run,
            slot_guard,
        );
    }
}

fn wave_backlog_empty(worktree: &Path, wave_name: &str) -> bool {
    let backlog_dir = worktree.join("wave").join(wave_name);
    let entries = match std::fs::read_dir(&backlog_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return true,
        Err(err) => {
            tracing::warn!(
                path = %backlog_dir.display(),
                error = %err,
                "failed to read wave backlog"
            );
            return false;
        }
    };

    !entries
        .flatten()
        .any(|entry| is_actionable_wave_item(&entry.path()))
}

fn is_actionable_wave_item(path: &Path) -> bool {
    if !path.is_file() {
        return false;
    }

    if path.file_name() == Some(OsStr::new("README.md")) {
        return false;
    }

    path.extension() == Some(OsStr::new("md"))
}

#[cfg(test)]
mod tests {
    use super::wave_backlog_empty;
    use std::fs;

    #[test]
    fn wave_backlog_empty_returns_true_for_missing_wave_dir() {
        let temp = tempfile::tempdir().expect("tempdir");
        assert!(wave_backlog_empty(temp.path(), "demo"));
    }

    #[test]
    fn wave_backlog_empty_ignores_readme_and_yaml_files() {
        let temp = tempfile::tempdir().expect("tempdir");
        let wave_dir = temp.path().join("wave").join("demo");
        fs::create_dir_all(&wave_dir).expect("create wave dir");
        fs::write(wave_dir.join("README.md"), "# demo").expect("write readme");
        fs::write(wave_dir.join("demo.yaml"), "flow: build").expect("write yaml");

        assert!(wave_backlog_empty(temp.path(), "demo"));
    }

    #[test]
    fn wave_backlog_empty_detects_actionable_markdown_items() {
        let temp = tempfile::tempdir().expect("tempdir");
        let wave_dir = temp.path().join("wave").join("demo");
        fs::create_dir_all(&wave_dir).expect("create wave dir");
        fs::write(wave_dir.join("01-next.md"), "next item").expect("write item");

        assert!(!wave_backlog_empty(temp.path(), "demo"));
    }
}
