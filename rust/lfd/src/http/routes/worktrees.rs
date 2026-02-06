use std::path::PathBuf;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::http::dto::{ListResponse, WorktreeDto};
use crate::http::state::HttpState;
use crate::http::{api_error_with_param, ApiResult};

#[derive(Deserialize)]
pub(crate) struct ListWorktreesQuery {
    repo: Option<String>,
}

pub async fn list_worktrees_handler(
    State(_state): State<HttpState>,
    Query(query): Query<ListWorktreesQuery>,
) -> ApiResult<ListResponse<WorktreeDto>> {
    let repo = query
        .repo
        .ok_or_else(|| api_error_with_param(StatusCode::BAD_REQUEST, "repo required", "repo"))?;

    let repo_path = PathBuf::from(repo);
    let main_repo = tokio::task::spawn_blocking({
        let repo_path = repo_path.clone();
        move || loopflow_engine::worktrees::main_repo_root(&repo_path)
    })
    .await
    .map_err(|err| {
        api_error_with_param(StatusCode::INTERNAL_SERVER_ERROR, err.to_string(), "repo")
    })?
    .map_err(|err| api_error_with_param(StatusCode::BAD_REQUEST, err.to_string(), "repo"))?;

    let worktrees =
        tokio::task::spawn_blocking(move || loopflow_engine::worktrees::list_worktrees(&main_repo))
            .await
            .map_err(|err| {
                api_error_with_param(StatusCode::INTERNAL_SERVER_ERROR, err.to_string(), "repo")
            })?
            .map_err(|err| {
                api_error_with_param(StatusCode::BAD_REQUEST, err.to_string(), "repo")
            })?;

    let data = worktrees
        .into_iter()
        .map(|wt| WorktreeDto {
            branch: wt.branch.unwrap_or_default(),
            path: wt.path.to_string_lossy().to_string(),
            kind: None,
            base_branch: wt.base_branch,
            prunable: Some(wt.prunable),
        })
        .collect::<Vec<_>>();

    Ok(Json(ListResponse::new(data, false)))
}
