use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use super::{dispatch_or_enqueue_activation, ActivationEnvelope};
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::store::SharedStore;
use crate::lfd::types::{Signal, Trigger, Wave, WaveStatus};

pub fn spawn_watch_poller(
    store: SharedStore,
    executor: WaveExecutor,
    scheduler: Arc<Scheduler>,
    event_hub: EventHub,
    cancel: CancellationToken,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(30));
        loop {
            tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::info!("watch_poller shutting down");
                    break;
                }
                _ = interval.tick() => {
                    check_repo_triggers(&store, &executor, &scheduler, &event_hub).await;
                }
            }
        }
    })
}

async fn check_repo_triggers(
    store: &SharedStore,
    executor: &WaveExecutor,
    scheduler: &Arc<Scheduler>,
    event_hub: &EventHub,
) {
    let triggers = match store.list_triggers_by_signal(Signal::Repo.as_i32()).await {
        Ok(triggers) => triggers,
        Err(err) => {
            tracing::error!(error = %err, "failed to list repo triggers");
            return;
        }
    };

    for trigger in triggers {
        if !trigger.enabled {
            continue;
        }

        let wave = match store.get_wave(&trigger.wave_id).await {
            Ok(Some(wave)) => wave,
            Ok(None) => continue,
            Err(err) => {
                tracing::error!(trigger_id = %trigger.id, error = %err, "failed to get wave");
                continue;
            }
        };

        if wave.status() == WaveStatus::Paused {
            continue;
        }

        match check_repo_trigger(&wave, &trigger) {
            Ok(result) => {
                if result.update_sha {
                    let mut trigger = trigger.clone();
                    trigger.last_main_sha = Some(result.current_sha.clone());
                    let _ = store.update_trigger(&trigger).await;
                }

                if !result.trigger {
                    continue;
                }
                let reason = format!(
                    "origin/main advanced {}..{}",
                    result.from_sha, result.current_sha
                );
                let envelope = ActivationEnvelope::new(
                    &trigger.wave_id,
                    Some(&trigger.id),
                    reason,
                    &result.from_sha,
                    &result.current_sha,
                    "main",
                );
                let _ = dispatch_or_enqueue_activation(
                    store,
                    executor,
                    scheduler,
                    event_hub,
                    &wave,
                    trigger.flow.clone(),
                    envelope,
                )
                .await;
            }
            Err(err) => {
                tracing::warn!(wave_id = %wave.id(), trigger_id = %trigger.id, error = %err, "repo trigger check failed");
            }
        }
    }
}

struct WatchCheck {
    from_sha: String,
    current_sha: String,
    trigger: bool,
    update_sha: bool,
}

fn paths_match_areas(areas: &[String], paths: &[&Path]) -> bool {
    if areas.is_empty() {
        return true;
    }

    paths
        .iter()
        .any(|path| areas.iter().any(|area| path.starts_with(area)))
}

fn check_repo_trigger(
    wave: &Wave,
    trigger: &Trigger,
) -> Result<WatchCheck, crate::engine::error::GitError> {
    use crate::engine::git;

    let repo = Path::new(wave.repo());
    git::fetch(repo, "origin", "main")?;
    let current_sha = git::rev_parse(repo, "refs/remotes/origin/main")?;

    let last_sha = trigger.last_main_sha.as_deref();
    if last_sha.is_none() {
        return Ok(WatchCheck {
            from_sha: String::new(),
            current_sha,
            trigger: false,
            update_sha: true,
        });
    }

    if Some(current_sha.as_str()) == last_sha {
        return Ok(WatchCheck {
            from_sha: String::new(),
            current_sha,
            trigger: false,
            update_sha: false,
        });
    }

    let prev = last_sha.expect("checked above");
    let changed_paths = match git::diff_names(repo, prev, &current_sha) {
        Ok(paths) => paths,
        Err(_) => {
            return Ok(WatchCheck {
                from_sha: prev.to_string(),
                current_sha,
                trigger: false,
                update_sha: true,
            })
        }
    };

    let path_refs: Vec<&Path> = changed_paths.iter().map(|p| p.as_path()).collect();
    let area_match = paths_match_areas(wave.area(), &path_refs);

    Ok(WatchCheck {
        from_sha: prev.to_string(),
        current_sha,
        trigger: area_match,
        update_sha: true,
    })
}

#[cfg(test)]
mod tests {
    use super::paths_match_areas;
    use std::path::Path;

    #[test]
    fn empty_areas_matches_everything() {
        let paths = [Path::new("docs/README.md")];
        assert!(paths_match_areas(&[], &paths));
    }

    #[test]
    fn prefix_match() {
        let areas = vec!["src/api/".to_string()];
        let paths = [Path::new("src/api/handler.rs")];
        assert!(paths_match_areas(&areas, &paths));
    }

    #[test]
    fn no_match() {
        let areas = vec!["src/api/".to_string()];
        let paths = [Path::new("docs/README.md")];
        assert!(!paths_match_areas(&areas, &paths));
    }

    #[test]
    fn nested_path_matches_parent_area() {
        let areas = vec!["src/".to_string()];
        let paths = [Path::new("src/api/deep/file.rs")];
        assert!(paths_match_areas(&areas, &paths));
    }

    #[test]
    fn multiple_areas_any_match() {
        let areas = vec!["src/".to_string(), "docs/".to_string()];
        let paths = [Path::new("docs/README.md")];
        assert!(paths_match_areas(&areas, &paths));
    }
}
