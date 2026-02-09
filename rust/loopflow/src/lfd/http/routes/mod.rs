pub mod flows;
pub mod hooks;
pub mod system;
pub mod wave_runs;
pub mod waves;
pub mod ws;

use crate::lfd::http::dto::{wave_dto, wave_run_dto, CommitEntryDto, ErrorResponse, WaveDto};
use crate::lfd::http::run_store;
use crate::lfd::id::LfdId;
use crate::lfd::store::{SharedStore, StoreError};
use crate::lfd::types::Wave;
use axum::http::StatusCode;
use axum::Json;

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
    waves: Vec<Wave>,
    include_active_run: bool,
) -> Result<Vec<WaveDto>, StoreError> {
    let mut views = Vec::with_capacity(waves.len());
    for wave in waves {
        views.push(build_wave_dto(store, wave, include_active_run).await?);
    }
    Ok(views)
}

pub async fn build_wave_dto(
    store: &SharedStore,
    wave: Wave,
    include_active_run: bool,
) -> Result<WaveDto, StoreError> {
    let wave_id = wave.id.clone();
    let latest = run_store(store, move |store| store.get_latest_wave_run(&wave_id)).await?;
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

    let active_run = if include_active_run {
        latest.map(wave_run_dto)
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

    Ok(wave_dto(
        &wave,
        active_run,
        local_worktree,
        remote_branch,
        commits,
        diff_stat,
        flow_steps,
    ))
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

    let main_repo =
        crate::engine::worktrees::main_repo_root(&worktree).unwrap_or_else(|_| worktree.clone());
    let base_branch = crate::engine::git::get_default_branch(&main_repo).unwrap_or_default();
    let diff_ref = format!("origin/{base_branch}");

    let commits = git_commit_log(&worktree, &diff_ref);
    let diff_stat = git_diff_stat(&worktree, &diff_ref);

    Some(WaveGitState {
        worktree: worktree.to_string_lossy().to_string(),
        branch,
        commits,
        diff_stat,
    })
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
