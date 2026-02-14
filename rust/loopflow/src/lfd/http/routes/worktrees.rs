use axum::extract::{Query, State};
use axum::Json;
use serde::Deserialize;

use crate::engine::worktrees::{list_worktrees, worktree_short_name_for_main_repo};
use crate::lfd::http::dto::{ListResponse, WorktreeDto};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, run_store, ApiResult};

#[derive(Deserialize)]
pub struct ListWorktreesQuery {
    repo: String,
}

pub async fn list_worktrees_handler(
    State(state): State<HttpState>,
    Query(query): Query<ListWorktreesQuery>,
) -> ApiResult<ListResponse<WorktreeDto>> {
    let repo_path = std::path::PathBuf::from(&query.repo);

    // Resolve main repo root (worktree paths may be passed as repo)
    let main_repo =
        crate::engine::worktrees::main_repo_root(&repo_path).unwrap_or_else(|_| repo_path.clone());

    // Get all waves for this repo to cross-reference wave IDs
    let repo_str = main_repo.to_string_lossy().to_string();
    let waves = run_store(&state.store, move |store| store.list_waves(Some(&repo_str)))
        .await
        .map_err(map_store_error)?;

    // Build a map: wave name → wave ID
    let wave_name_to_id: std::collections::HashMap<String, String> = waves
        .into_iter()
        .map(|w| (w.name.clone(), w.id.to_string()))
        .collect();

    // List worktrees (blocking — spawns threads for merge checks)
    let worktrees = tokio::task::spawn_blocking({
        let repo = main_repo.clone();
        move || list_worktrees(&repo)
    })
    .await
    .map_err(|e| api_error(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .map_err(|e| api_error(axum::http::StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let dtos: Vec<WorktreeDto> = worktrees
        .into_iter()
        .filter_map(|wt| {
            // Filter out the main repo worktree
            if wt.path == main_repo {
                return None;
            }

            let wave_id = worktree_short_name_for_main_repo(&wt.path, &main_repo)
                .as_deref()
                .and_then(|name| wave_name_to_id.get(name))
                .cloned();

            Some(WorktreeDto {
                object: "worktree".to_string(),
                path: wt.path.to_string_lossy().to_string(),
                branch: wt.branch,
                merged: wt.merged,
                prunable: wt.prunable,
                wave_id,
            })
        })
        .collect();

    Ok(Json(ListResponse::new(dtos, false)))
}
