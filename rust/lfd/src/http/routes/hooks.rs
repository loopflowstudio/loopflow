use axum::extract::State;
use axum::Json;
use serde::Deserialize;

use crate::http::state::HttpState;
use crate::http::ApiResult;
use crate::types::Event;

#[derive(Deserialize)]
pub(crate) struct GitHookRequest {
    #[allow(dead_code)]
    hook: String,
    repo: String,
    branch: Option<String>,
}

pub async fn git_hook_handler(
    State(state): State<HttpState>,
    Json(payload): Json<GitHookRequest>,
) -> ApiResult<serde_json::Value> {
    state.event_hub.send(Event::worktree_updated(
        payload.repo.clone(),
        payload.repo,
        payload.branch,
    ));

    Ok(Json(serde_json::json!({ "ok": true })))
}
