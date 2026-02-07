use axum::extract::State;
use axum::Json;
use serde::Deserialize;

use crate::lfd::http::state::HttpState;
use crate::lfd::http::ApiResult;
use crate::lfd::types::Event;

#[derive(Deserialize)]
pub struct GitHookRequest {
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
