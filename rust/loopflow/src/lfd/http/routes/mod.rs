pub mod flows;
pub mod hooks;
pub mod system;
pub mod wave_runs;
pub mod waves;
pub mod worktrees;
pub mod ws;

use crate::lfd::http::dto::{
    format_datetime, stimulus_dto, wave_run_dto, CommitEntryDto, ErrorResponse, WaveDto,
};
use crate::lfd::http::run_store;
use crate::lfd::id::LfdId;
use crate::lfd::store::{SharedStore, StoreError};
use crate::lfd::types::{LivePrState, LivePullRequestState, Wave, WaveRun};
use crate::lfd::{config::GitHubConfig, github};
use axum::http::StatusCode;
use axum::Json;
use std::collections::{HashMap, HashSet};
use time::OffsetDateTime;

pub async fn resolve_wave_id(
    state: &crate::lfd::http::HttpState,
    value: &str,
) -> Result<crate::lfd::id::LfdId, (StatusCode, Json<ErrorResponse>)> {
    if let Ok(id) = value.parse::<crate::lfd::id::LfdId>() {
        return Ok(id);
    }

    let name = value.to_string();
    let wave = run_store(&state.store, move |store| store.get_wave_by_name(&name))
        .await
        .map_err(crate::lfd::http::map_store_error)?;
    wave.map(|wave| wave.id)
        .ok_or_else(|| crate::lfd::http::api_error(StatusCode::NOT_FOUND, "wave not found"))
}

pub async fn build_wave_dtos(
    store: &SharedStore,
    github_config: &GitHubConfig,
    waves: Vec<Wave>,
    include_active_run: bool,
) -> Result<Vec<WaveDto>, StoreError> {
    let mut views = Vec::with_capacity(waves.len());
    for wave in waves {
        views.push(build_wave_dto(store, github_config, wave, include_active_run).await?);
    }
    Ok(views)
}

pub async fn build_wave_dto(
    store: &SharedStore,
    github_config: &GitHubConfig,
    wave: Wave,
    include_active_run: bool,
) -> Result<WaveDto, StoreError> {
    let wave_id = wave.id.clone();
    let latest = run_store(store, move |store| store.get_latest_wave_run(&wave_id)).await?;
    let wave_id = wave.id.clone();
    let stack_runs = run_store(store, move |store| store.list_stack_runs(&wave_id)).await?;
    let live_pr_projection =
        build_wave_live_pr_projection(store, github_config, &stack_runs).await?;
    let repo = wave.repo.clone();
    let name = wave.name.clone();
    let flow_name = wave.flow.clone();
    let flow_repo = wave.repo.clone();
    let (git_state, flow_steps) = tokio::join!(
        async {
            tokio::task::spawn_blocking(move || infer_wave_git_state(&repo, &name))
                .await
                .ok()
                .flatten()
        },
        async {
            tokio::task::spawn_blocking(move || {
                flows::load_flow_steps(&flow_name, std::path::Path::new(&flow_repo))
                    .unwrap_or_default()
            })
            .await
            .unwrap_or_default()
        }
    );

    let wave_id_stim = wave.id.clone();
    let stimuli_list = run_store(store, move |store| store.list_stimuli(Some(&wave_id_stim)))
        .await
        .unwrap_or_default();
    let stimuli = stimuli_list.into_iter().map(stimulus_dto).collect();

    let active_run = if include_active_run {
        latest.map(|run| {
            let (live_pr_state, pr_state_stale) = live_pr_for_run(&live_pr_projection, &run);
            wave_run_dto(run, live_pr_state, pr_state_stale)
        })
    } else {
        None
    };
    let (local_worktree, remote_branch, commits, diff_stat) = match git_state {
        Some(state) => (
            Some(state.worktree),
            state.branch,
            state.commits,
            state.diff_stat,
        ),
        None => (None, None, Vec::new(), None),
    };

    Ok(WaveDto {
        id: wave.id.to_string(),
        object: "wave".to_string(),
        name: wave.name,
        repo: wave.repo,
        flow: wave.flow,
        direction: wave.direction,
        area: wave.area,
        created_at: format_datetime(wave.created_at),
        status: wave.status.as_str().to_string(),
        iteration: wave.iteration,
        local_worktree,
        remote_branch,
        commits,
        diff_stat,
        flow_steps,
        open_pr_count: live_pr_projection.open_pr_count,
        stack_count: stack_runs.len() as u32,
        has_stale_pr_state: live_pr_projection.has_stale_pr_state,
        stimuli,
        active_run,
    })
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct LivePrKey {
    pub(crate) repo_id: String,
    pub(crate) pr_number: u32,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct WaveLivePrProjection {
    pub live_states: HashMap<LivePrKey, LivePullRequestState>,
    pub stale_keys: HashSet<LivePrKey>,
    pub open_pr_count: u32,
    pub has_stale_pr_state: bool,
}

pub(crate) fn run_live_pr_key(run: &WaveRun) -> Option<LivePrKey> {
    let pr_number = run.snapshot.pr.as_ref()?.number?;
    Some(LivePrKey {
        repo_id: run.snapshot.repo.clone(),
        pr_number,
    })
}

pub(crate) fn live_pr_for_run<'a>(
    projection: &'a WaveLivePrProjection,
    run: &WaveRun,
) -> (Option<&'a LivePullRequestState>, bool) {
    let Some(key) = run_live_pr_key(run) else {
        return (None, false);
    };
    let live = projection.live_states.get(&key);
    let stale = projection.stale_keys.contains(&key);
    (live, stale)
}

pub(crate) async fn build_wave_live_pr_projection(
    store: &SharedStore,
    github_config: &GitHubConfig,
    runs: &[WaveRun],
) -> Result<WaveLivePrProjection, StoreError> {
    let targets: HashSet<LivePrKey> = runs.iter().filter_map(run_live_pr_key).collect();

    let mut stale_keys: HashSet<LivePrKey> = HashSet::new();
    if !targets.is_empty() {
        let token = github_config
            .token
            .as_deref()
            .map(str::trim)
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
                    github::github_repo_from_local(std::path::Path::new(&repo_path))
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
                            let live_state = LivePullRequestState {
                                repo_id: key.repo_id.clone(),
                                pr_number: pull_request.number,
                                state: pull_request.state,
                                is_draft: pull_request.is_draft,
                                head_ref: pull_request.head_ref,
                                head_sha: pull_request.head_sha,
                                base_ref: pull_request.base_ref,
                                updated_at: pull_request.updated_at,
                                merged_at: pull_request.merged_at,
                                synced_at: OffsetDateTime::now_utc(),
                            };
                            let state_for_store = live_state.clone();
                            run_store(store, move |store| {
                                store.upsert_live_pr_state(&state_for_store)
                            })
                            .await?;
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
        let repo_id = key.repo_id.clone();
        let pr_number = key.pr_number;
        let state = run_store(store, move |store| {
            store.get_live_pr_state(&repo_id, pr_number)
        })
        .await?;
        if let Some(state) = state {
            live_states.insert(key.clone(), state);
        } else {
            stale_keys.insert(key.clone());
        }
    }

    let open_pr_count = live_states
        .values()
        .filter(|state| state.state == LivePrState::Open)
        .count() as u32;
    let has_stale_pr_state = !stale_keys.is_empty();

    Ok(WaveLivePrProjection {
        live_states,
        stale_keys,
        open_pr_count,
        has_stale_pr_state,
    })
}

/// Cursor-based pagination over a list of items with an `id: LfdId` field.
pub fn paginate<T>(
    mut items: Vec<T>,
    limit: Option<u32>,
    starting_after: Option<&str>,
    ending_before: Option<&str>,
    id: fn(&T) -> &LfdId,
) -> (Vec<T>, bool) {
    if let Some(cursor) = starting_after {
        if let Some(pos) = items.iter().position(|item| id(item).as_str() == cursor) {
            items = items.split_off(pos + 1);
        }
    }
    if let Some(cursor) = ending_before {
        if let Some(pos) = items.iter().position(|item| id(item).as_str() == cursor) {
            items.truncate(pos);
        }
    }
    let mut has_more = false;
    if let Some(limit) = limit {
        let limit = limit as usize;
        if items.len() > limit {
            items.truncate(limit);
            has_more = true;
        }
    }
    (items, has_more)
}

struct WaveGitState {
    worktree: String,
    branch: Option<String>,
    commits: Vec<CommitEntryDto>,
    diff_stat: Option<String>,
}

fn infer_wave_git_state(repo: &str, wave_name: &str) -> Option<WaveGitState> {
    let repo_path = std::path::Path::new(repo);
    let worktree = crate::engine::worktrees::worktree_path(repo_path, wave_name);
    if !worktree.exists() {
        return None;
    }
    let branch = crate::engine::git::current_branch(&worktree).ok().flatten();

    let diff_ref = nearest_base_ref(&worktree, wave_name);

    let commits = git_commit_log(&worktree, &diff_ref);
    let diff_stat = git_diff_stat(&worktree, &diff_ref);

    Some(WaveGitState {
        worktree: worktree.to_string_lossy().to_string(),
        branch,
        commits,
        diff_stat,
    })
}

fn is_open_pr_state(state: Option<&str>) -> bool {
    match state {
        Some(state) => state.eq_ignore_ascii_case("open") || state.eq_ignore_ascii_case("draft"),
        None => false,
    }
}

/// Find the nearest ancestor branch for diff comparison.
///
/// Candidates are the default branch plus any remote branches belonging to this wave
/// (matched by the sanitized wave name in the branch). For each candidate, compute
/// merge-base with HEAD and pick the closest one (fewest commits away). This handles
/// stacking: after `next`, the previous iteration branch is a closer ancestor than main.
/// After the parent merges and you rebase onto main, main becomes closest again.
fn nearest_base_ref(worktree: &std::path::Path, wave_name: &str) -> String {
    let main_repo = crate::engine::worktrees::main_repo_root(worktree)
        .unwrap_or_else(|_| worktree.to_path_buf());
    let default_branch =
        crate::engine::git::get_default_branch(&main_repo).unwrap_or_else(|_| "main".to_string());

    let current = crate::engine::git::current_branch(worktree)
        .ok()
        .flatten()
        .unwrap_or_default();

    // Candidates: default branch + remote branches sharing this wave's name.
    let wave_slug = crate::engine::naming::sanitize_for_branch(wave_name);
    let mut candidates: Vec<String> = vec![default_branch.clone()];
    if let Some(sibling_branches) = wave_remote_branches(worktree, &wave_slug) {
        for branch in sibling_branches {
            if !is_current_or_tracking_branch(&branch, &current) && !candidates.contains(&branch) {
                candidates.push(branch);
            }
        }
    }

    // For each candidate, find merge-base with HEAD and count commits from there to HEAD.
    // Pick the one closest to HEAD.
    let mut best_ref = default_branch;
    let mut best_distance = u64::MAX;

    for candidate in &candidates {
        let mb = match crate::engine::git::merge_base(worktree, "HEAD", candidate) {
            Ok(sha) => sha,
            Err(_) => continue,
        };
        let distance = commit_count(worktree, &mb);
        if distance < best_distance {
            best_distance = distance;
            best_ref = mb;
        }
    }

    best_ref
}

fn is_current_or_tracking_branch(candidate: &str, current: &str) -> bool {
    if candidate == current {
        return true;
    }
    candidate
        .split_once('/')
        .is_some_and(|(_, branch)| branch == current)
}

/// List remote branches whose name contains the wave slug (e.g. ".conviction.").
fn wave_remote_branches(worktree: &std::path::Path, wave_slug: &str) -> Option<Vec<String>> {
    let pattern = format!(".{}.", wave_slug);
    let output = std::process::Command::new("git")
        .args(["branch", "-r", "--format=%(refname:short)"])
        .current_dir(worktree)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branches = String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| line.contains(&pattern))
        .map(|line| line.trim().to_string())
        .collect();
    Some(branches)
}

fn commit_count(worktree: &std::path::Path, from_ref: &str) -> u64 {
    std::process::Command::new("git")
        .args(["rev-list", "--count", &format!("{from_ref}..HEAD")])
        .current_dir(worktree)
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8_lossy(&o.stdout).trim().parse().ok()
            } else {
                None
            }
        })
        .unwrap_or(u64::MAX)
}

fn git_commit_log(worktree: &std::path::Path, diff_ref: &str) -> Vec<CommitEntryDto> {
    let output = std::process::Command::new("git")
        .args(["log", "--oneline", &format!("{diff_ref}..HEAD")])
        .current_dir(worktree)
        .output();
    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Vec::new(),
    };
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| {
            let (sha, message) = line.split_once(' ')?;
            Some(CommitEntryDto {
                sha: sha.to_string(),
                message: message.to_string(),
            })
        })
        .collect()
}

fn git_diff_stat(worktree: &std::path::Path, diff_ref: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["diff", "--stat", diff_ref])
        .current_dir(worktree)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let stat = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if stat.is_empty() {
        None
    } else {
        Some(stat)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::id::LfdId;
    use crate::lfd::types::{
        LivePrState, LivePullRequestState, PullRequest, WaveRun, WaveRunKind, WaveRunSnapshot,
        WaveRunStackStatus, WaveRunStatus,
    };
    use std::collections::{HashMap, HashSet};
    use time::OffsetDateTime;

    fn wave_run_with_pr(pr_number: Option<u32>, pr_state: Option<&str>) -> WaveRun {
        WaveRun {
            id: LfdId::new(),
            wave_id: LfdId::new(),
            snapshot: WaveRunSnapshot {
                repo: ".".to_string(),
                flow: "ship".to_string(),
                direction: Vec::new(),
                area: Vec::new(),
                pr: Some(PullRequest {
                    url: "https://example.test/pr/1".to_string(),
                    number: pr_number,
                    state: pr_state.map(ToString::to_string),
                    title: Some("test".to_string()),
                    branch: Some("feature".to_string()),
                }),
            },
            iteration: 0,
            step_index: 0,
            status: WaveRunStatus::Running,
            worktree: "/tmp/worktree".to_string(),
            branch: "feature".to_string(),
            started_at: None,
            ended_at: None,
            error: None,
            flow_parents: Vec::new(),
            run_kind: WaveRunKind::Main,
            sidecar_kind: None,
            parent_run_id: None,
            parent_pr_number: None,
            stack_position: 0,
            stack_group_id: "wave-group".to_string(),
            stack_status: WaveRunStackStatus::Active,
            lineage_inferred: false,
        }
    }

    #[test]
    fn current_tracking_branch_matches_local_branch_name() {
        assert!(is_current_or_tracking_branch(
            "origin/jack.wave.20260209_1000",
            "jack.wave.20260209_1000"
        ));
    }

    #[test]
    fn sibling_branch_does_not_match_current_branch_name() {
        assert!(!is_current_or_tracking_branch(
            "origin/jack.wave.20260209_1001",
            "jack.wave.20260209_1000"
        ));
    }

    #[test]
    fn unknown_pr_state_is_not_open() {
        assert!(!is_open_pr_state(None));
        assert!(!is_open_pr_state(Some("closed")));
        assert!(!is_open_pr_state(Some("merged")));
        assert!(is_open_pr_state(Some("open")));
        assert!(is_open_pr_state(Some("draft")));
    }

    #[test]
    fn live_pr_for_run_uses_projection_values() {
        let run = wave_run_with_pr(Some(101), Some("open"));
        let key = run_live_pr_key(&run).expect("key");
        let projection = WaveLivePrProjection {
            live_states: HashMap::from([(
                key.clone(),
                LivePullRequestState {
                    repo_id: ".".to_string(),
                    pr_number: 101,
                    state: LivePrState::Open,
                    is_draft: false,
                    head_ref: "feature".to_string(),
                    head_sha: "abc123".to_string(),
                    base_ref: "main".to_string(),
                    updated_at: OffsetDateTime::now_utc(),
                    merged_at: None,
                    synced_at: OffsetDateTime::now_utc(),
                },
            )]),
            stale_keys: HashSet::from([key]),
            open_pr_count: 1,
            has_stale_pr_state: true,
        };

        let (live_pr_state, stale) = live_pr_for_run(&projection, &run);
        assert_eq!(live_pr_state.map(|state| state.pr_number), Some(101));
        assert!(stale);
    }
}
