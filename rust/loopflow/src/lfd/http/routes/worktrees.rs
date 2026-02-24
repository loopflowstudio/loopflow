use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::engine::worktrees::{list_worktrees, main_repo_root, worktree_short_name_for_main_repo};
use crate::lfd::http::dto::{ListResponse, WorktreeDto};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiMessage, ApiResult};

#[derive(Debug, Deserialize)]
pub struct ListWorktreesQuery {
    repo: String,
}

fn internal_error(
    err: impl std::fmt::Display,
) -> (StatusCode, Json<crate::lfd::http::dto::ErrorResponse>) {
    api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        ApiMessage::Untrusted(err.to_string()),
    )
}

pub async fn list_worktrees_handler(
    State(state): State<HttpState>,
    Query(query): Query<ListWorktreesQuery>,
) -> ApiResult<ListResponse<WorktreeDto>> {
    let repo_path = std::path::PathBuf::from(&query.repo);
    let main_repo = main_repo_root(&repo_path).unwrap_or_else(|_| repo_path.clone());
    let repo_str = main_repo.to_string_lossy().to_string();
    let waves = state
        .store
        .list_waves(Some(&repo_str))
        .await
        .map_err(map_store_error)?;

    let wave_name_to_id: std::collections::HashMap<String, String> = waves
        .into_iter()
        .map(|w| (w.name().clone(), w.id().to_string()))
        .collect();

    let worktrees = tokio::task::spawn_blocking({
        let repo = main_repo.clone();
        move || list_worktrees(&repo)
    })
    .await
    .map_err(internal_error)?
    .map_err(internal_error)?;

    let dtos: Vec<WorktreeDto> = worktrees
        .into_iter()
        .filter(|wt| wt.path != main_repo)
        .map(|wt| {
            let wave_id = worktree_short_name_for_main_repo(&wt.path, &main_repo)
                .and_then(|name| wave_name_to_id.get(&name).cloned());

            WorktreeDto {
                object: "worktree".to_string(),
                path: wt.path.to_string_lossy().to_string(),
                branch: wt.branch,
                merged: wt.merged,
                prunable: wt.prunable,
                wave_id,
            }
        })
        .collect();

    Ok(Json(ListResponse::new(dtos, false)))
}
