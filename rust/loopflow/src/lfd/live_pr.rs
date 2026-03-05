use std::collections::{HashMap, HashSet};
use std::path::Path;

use secrecy::ExposeSecret;
use time::OffsetDateTime;

use crate::lfd::config::GitHubConfig;
use crate::lfd::github;
use crate::lfd::store::{SharedStore, StoreError};
use crate::lfd::types::{LivePrState, LivePullRequestState, WaveRun};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LivePrKey {
    pub repo_id: String,
    pub pr_number: u32,
}

#[derive(Debug, Clone, Default)]
pub struct LivePrSnapshot {
    pub live_states: HashMap<LivePrKey, LivePullRequestState>,
    pub stale_keys: HashSet<LivePrKey>,
}

impl LivePrSnapshot {
    pub fn state_for_run(&self, run: &WaveRun) -> Option<&LivePullRequestState> {
        let key = run_live_pr_key(run)?;
        self.live_states.get(&key)
    }

    pub fn stale_for_run(&self, run: &WaveRun) -> bool {
        let Some(key) = run_live_pr_key(run) else {
            return false;
        };
        self.stale_keys.contains(&key)
    }

    pub fn open_pr_count(&self) -> u32 {
        self.live_states
            .values()
            .filter(|state| state.state == LivePrState::Open)
            .count() as u32
    }

    pub fn has_stale_pr_state(&self) -> bool {
        !self.stale_keys.is_empty()
    }
}

pub fn run_live_pr_key(run: &WaveRun) -> Option<LivePrKey> {
    let pr_number = run.snapshot.pr.as_ref()?.number?;
    Some(LivePrKey {
        repo_id: run.snapshot.repo.clone(),
        pr_number,
    })
}

pub async fn build_live_pr_snapshot(
    store: &SharedStore,
    github_config: &GitHubConfig,
    runs: &[WaveRun],
) -> Result<LivePrSnapshot, StoreError> {
    let targets: HashSet<LivePrKey> = runs.iter().filter_map(run_live_pr_key).collect();
    let mut stale_keys: HashSet<LivePrKey> = HashSet::new();

    if !targets.is_empty() {
        let token = github_config
            .token
            .as_ref()
            .map(|token| token.expose_secret().trim())
            .unwrap_or_default();
        if token.is_empty() {
            stale_keys.extend(targets.iter().cloned());
        } else {
            let mut repo_targets: HashMap<String, Vec<LivePrKey>> = HashMap::new();
            for key in &targets {
                repo_targets
                    .entry(key.repo_id.clone())
                    .or_default()
                    .push(key.clone());
            }

            let mut repo_lookup: HashMap<String, Option<String>> = HashMap::new();
            for repo_id in repo_targets.keys() {
                let repo_path = repo_id.clone();
                let repo_full_name = tokio::task::spawn_blocking(move || {
                    github::github_repo_from_local(Path::new(&repo_path))
                })
                .await
                .ok()
                .flatten();
                repo_lookup.insert(repo_id.clone(), repo_full_name);
            }

            for (repo_id, keys) in &repo_targets {
                let repo_full_name = repo_lookup.get(repo_id).cloned().flatten();
                let Some(repo_full_name) = repo_full_name else {
                    stale_keys.extend(keys.iter().cloned());
                    continue;
                };

                for key in keys {
                    match github::fetch_pull_request(&repo_full_name, key.pr_number, token).await {
                        Ok(Some(pull_request)) => {
                            let live_state = github::into_live_pull_request_state(
                                key.repo_id.clone(),
                                pull_request,
                                OffsetDateTime::now_utc(),
                            );
                            store.upsert_live_pr_state(&live_state).await?;
                        }
                        Ok(None) | Err(_) => {
                            stale_keys.insert(key.clone());
                        }
                    }
                }
            }
        }
    }

    let mut live_states = HashMap::new();
    for key in &targets {
        let state = store.get_live_pr_state(&key.repo_id, key.pr_number).await?;
        if let Some(state) = state {
            live_states.insert(key.clone(), state);
        } else {
            stale_keys.insert(key.clone());
        }
    }

    Ok(LivePrSnapshot {
        live_states,
        stale_keys,
    })
}
