use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use std::path::Path as FsPath;
use std::str::FromStr;
use time::OffsetDateTime;
use tokio::process::Command as TokioCommand;
use tracing::warn;

use super::session_controls::connection_host;
use crate::engine::git::{branch_rename, current_branch, delete_remote_branch, push_with_upstream};
use crate::engine::naming::sanitize_for_branch;
use crate::engine::wave_config::update_wave_agent_config;
use crate::engine::worktree::remove_worktree;
use crate::engine::worktrees::{branch_exists, ensure_wave_worktree, worktree_path};
use crate::lfd::http::dto::{
    session_connection_info_dto, session_dto, CombineResponse, CombineResponseResult,
    DeletedResourceResponse, LandWaveResponse, ListResponse, NextWaveResponse, StopWaveResponse,
    WaveAgentTreeDto, WaveAgentTreeSessionDto, WaveDto,
};
use crate::lfd::http::routes::{build_wave_dto, resolve_wave_id, ApiError};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiMessage, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::types::{Run, RunStatus, Wave, WaveStatus, LIVE_SESSION_STATUSES};

// TODO(M1): convert the mutating wave routes in this file to exec lf argv or
// remove them. lfd is the local face, not a hand: no direct git, worktree, tmux,
// or ops calls from route handlers.
#[derive(Debug, Deserialize)]
pub struct ListWavesQuery {
    repo: Option<String>,
    limit: Option<u32>,
    starting_after: Option<String>,
    ending_before: Option<String>,
    #[serde(default, rename = "expand[]")]
    expand: ExpandParam,
}

#[derive(Debug, Deserialize, Default)]
pub struct ExpandQuery {
    #[serde(default, rename = "expand[]")]
    expand: ExpandParam,
}

/// Accept `expand[]=value` as either a single string or repeated params.
#[derive(Debug, Default, Clone)]
pub struct ExpandParam(Vec<String>);

impl ExpandParam {
    fn contains(&self, value: &str) -> bool {
        self.0.iter().any(|v| v == value)
    }
}

impl<'de> serde::Deserialize<'de> for ExpandParam {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        struct ExpandVisitor;
        impl<'de> de::Visitor<'de> for ExpandVisitor {
            type Value = ExpandParam;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a string or list of strings")
            }
            fn visit_str<E: de::Error>(self, v: &str) -> Result<ExpandParam, E> {
                Ok(ExpandParam(vec![v.to_string()]))
            }
            fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<ExpandParam, A::Error> {
                let mut values = Vec::new();
                while let Some(v) = seq.next_element::<String>()? {
                    values.push(v);
                }
                Ok(ExpandParam(values))
            }
        }

        deserializer.deserialize_any(ExpandVisitor)
    }
}

#[derive(Debug, Deserialize, Default)]
pub struct GetWaveAgentTreeQuery {
    active_only: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateWaveRequest {
    name: Option<String>,
    goal: Option<String>,
    direction: Option<Vec<String>>,
    area: Option<Vec<String>>,
    workers: Option<u32>,
    status: Option<String>,
    agent: Option<String>,
    step_agents: Option<std::collections::HashMap<String, String>>,
    serialized: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct LandWaveRequest {
    strict: Option<bool>,
    local: Option<bool>,
    create_pr: Option<bool>,
    worktree: Option<String>,
}

pub async fn list_waves_handler(
    State(state): State<HttpState>,
    Query(query): Query<ListWavesQuery>,
) -> ApiResult<ListResponse<WaveDto>> {
    let waves = state
        .store
        .list_waves(query.repo.as_deref())
        .await
        .map_err(map_store_error)?;
    let include_active_run = query.expand.contains("active_run");
    let (waves, has_more) = super::paginate(
        waves,
        query.limit,
        query.starting_after.as_deref(),
        query.ending_before.as_deref(),
        |w| w.id(),
    );
    let views = crate::lfd::http::routes::build_wave_dtos(
        &state.store,
        &state.github,
        waves,
        include_active_run,
    )
    .await
    .map_err(map_store_error)?;
    Ok(Json(ListResponse::new(views, has_more)))
}

fn trimmed_non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn update_wave_workers(
    current_workers: u32,
    requested_workers: Option<u32>,
    serialized: Option<bool>,
) -> u32 {
    requested_workers
        .or_else(|| serialized.map(|_| 1))
        .unwrap_or(current_workers)
}

async fn wave_name_exists(state: &HttpState, name: &str) -> Result<bool, crate::lfdb::StoreError> {
    let waves = state.store.list_waves(None).await?;
    Ok(waves.into_iter().any(|wave| wave.name() == name))
}

pub async fn get_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Query(query): Query<ExpandQuery>,
) -> ApiResult<WaveDto> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;
    let include_active_run = query.expand.contains("active_run");
    let view = build_wave_dto(&state.store, &state.github, wave, include_active_run)
        .await
        .map_err(map_store_error)?;
    Ok(Json(view))
}

pub async fn update_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Json(payload): Json<UpdateWaveRequest>,
) -> ApiResult<WaveDto> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let mut wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    // Handle rename: move worktree + rename branch before updating DB.
    if let Some(ref name) = payload.name {
        if !name.is_empty() && *name != *wave.name() {
            let new_name = name.clone();

            // Check for duplicate wave name (globally unique).
            let existing = wave_name_exists(&state, &new_name)
                .await
                .map_err(map_store_error)?;
            if existing {
                return Err(wave_name_exists_error(&new_name));
            }

            // Reject rename while wave is running or waiting.
            let active_run = state
                .store
                .get_active_run(wave.id())
                .await
                .map_err(map_store_error)?;
            if let Some(run) = active_run {
                if matches!(run.status, RunStatus::Running | RunStatus::Waiting) {
                    return Err(api_error(
                        StatusCode::PRECONDITION_FAILED,
                        "cannot rename wave while running",
                    ));
                }
            }

            // Move worktree + rename branch on disk.
            let old_name = wave.name().clone();
            let repo = wave.repo().to_string();
            let new_name_for_wt = new_name.clone();
            run_blocking_result(
                move || {
                    rename_wave_worktree(std::path::Path::new(&repo), &old_name, &new_name_for_wt)
                },
                StatusCode::CONFLICT,
            )
            .await?;

            wave.name = new_name;
        }
    }

    if let Some(goal) = trimmed_non_empty(payload.goal.as_deref()) {
        wave.goal = goal;
    }
    if let Some(direction) = payload.direction {
        wave.direction = direction;
    }
    if let Some(area) = payload.area {
        wave.area = area;
    }
    wave.workers = update_wave_workers(wave.workers, payload.workers, payload.serialized);
    if let Some(status) = payload.status {
        let parsed = WaveStatus::from_str(&status)
            .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid status"))?;
        wave.set_status(parsed);
    }

    if payload.agent.is_some() || payload.step_agents.is_some() {
        let repo = wave.repo().to_string();
        let wave_name = wave.name().clone();
        let agent = payload.agent.clone();
        let step_agents = payload.step_agents.clone();
        run_blocking_result(
            move || update_wave_agent_config(FsPath::new(&repo), &wave_name, agent, step_agents),
            StatusCode::BAD_REQUEST,
        )
        .await?;
    }

    state
        .store
        .update_wave(&wave)
        .await
        .map_err(map_store_error)?;

    let view = build_wave_dto(&state.store, &state.github, wave, false)
        .await
        .map_err(map_store_error)?;
    Ok(Json(view))
}

pub async fn delete_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<DeletedResourceResponse> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    // Get wave info before deleting (for worktree cleanup).
    let wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    // Delete from the store (cascades to runs, sessions, etc.).
    state
        .store
        .delete_wave(&wave_id)
        .await
        .map_err(map_store_error)?;

    crate::lfd::queue::remove_reconcile_lock(&wave_id).await;

    // Clean up the worktree on disk.
    let repo = wave.repo().to_string();
    let wave_name = wave.name().clone();
    tokio::task::spawn_blocking(move || {
        let wt = worktree_path(std::path::Path::new(&repo), &wave_name);
        if wt.exists() {
            if let Err(err) = remove_worktree(&wt, false) {
                tracing::warn!(worktree = %wt.display(), error = %err, "failed to remove worktree");
            }
        }
    })
    .await
    .map_err(|err| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiMessage::Untrusted(err.to_string()),
        )
    })?;

    Ok(Json(DeletedResourceResponse {
        id: wave_id.to_string(),
        object: "wave".to_string(),
        deleted: true,
    }))
}

pub async fn get_wave_agent_tree_handler(
    State(state): State<HttpState>,
    headers: HeaderMap,
    Path(wave_id): Path<String>,
    Query(query): Query<GetWaveAgentTreeQuery>,
) -> ApiResult<WaveAgentTreeDto> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;
    let root_wave = build_wave_dto(&state.store, &state.github, wave, false)
        .await
        .map_err(map_store_error)?;
    let children = state
        .store
        .list_child_waves(&wave_id)
        .await
        .map_err(map_store_error)?;
    let child_waves =
        crate::lfd::http::routes::build_wave_dtos(&state.store, &state.github, children, false)
            .await
            .map_err(map_store_error)?;
    let statuses = query
        .active_only
        .unwrap_or(true)
        .then_some(LIVE_SESSION_STATUSES);
    let sessions = state
        .store
        .list_control_sessions(Some(&wave_id), statuses)
        .await
        .map_err(map_store_error)?
        .into_iter()
        .map(|session| {
            let connection = session
                .is_tmux_backed()
                .then(|| session_connection_info_dto(&session, connection_host(&headers)));
            WaveAgentTreeSessionDto {
                session: session_dto(session),
                connection,
            }
        })
        .collect();

    Ok(Json(WaveAgentTreeDto {
        object: "wave_agent_tree".to_string(),
        id: format!("tree-{wave_id}"),
        wave: root_wave,
        child_waves,
        sessions,
    }))
}

pub async fn stop_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<StopWaveResponse> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    let run = state
        .store
        .get_active_run(&wave_id)
        .await
        .map_err(map_store_error)?;

    if let Some(mut run) = run {
        run.status = RunStatus::Failed;
        run.error = Some("stopped".to_string());
        run.ended_at = Some(OffsetDateTime::now_utc());
        state
            .store
            .update_run(&run)
            .await
            .map_err(map_store_error)?;
        cancel_active_session(&state, &run).await?;
        let wave_id_for_update = run.wave_id.clone();
        if let Some(mut wave) = state
            .store
            .get_wave(&wave_id_for_update)
            .await
            .map_err(map_store_error)?
        {
            wave.set_status(WaveStatus::Failed);
            state
                .store
                .update_wave(&wave)
                .await
                .map_err(map_store_error)?;
        }
    }

    Ok(Json(StopWaveResponse { stopped: true }))
}

async fn cancel_active_session(state: &HttpState, run: &Run) -> Result<(), ApiError> {
    let Some(mut session) = state
        .store
        .get_active_control_session_for_run(&run.id)
        .await
        .map_err(map_store_error)?
    else {
        return Ok(());
    };

    if !session.cancel() {
        return Ok(());
    }

    state
        .store
        .update_control_session(&session)
        .await
        .map_err(map_store_error)?;
    if session.is_tmux_backed() {
        match TokioCommand::new("tmux")
            .args(["kill-session", "-t", &session.tmux_name])
            .status()
            .await
        {
            Ok(status) if status.success() => {}
            Ok(status) => warn!(
                session_id = %session.id,
                exit_code = ?status.code(),
                "failed to kill tmux terminal session while stopping wave"
            ),
            Err(err) => warn!(
                session_id = %session.id,
                error = %err,
                "failed to kill tmux terminal session while stopping wave"
            ),
        }
    }

    Ok(())
}

pub async fn land_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    payload: Option<Json<LandWaveRequest>>,
) -> ApiResult<LandWaveResponse> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let payload = payload.map(|Json(value)| value).unwrap_or_default();
    let wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    let latest_run = state
        .store
        .get_latest_run(wave.id())
        .await
        .map_err(map_store_error)?;

    let latest_worktree = payload
        .worktree
        .or_else(|| latest_run.as_ref().map(|run| run.worktree.clone()))
        .filter(|value| !value.is_empty());
    let worktree = resolve_wave_work_dir_for_api(
        wave.repo().to_string(),
        wave.name().clone(),
        latest_worktree,
    )
    .await?;

    let strict = payload.strict.unwrap_or(false);
    let local = payload.local.unwrap_or(false);
    let create_pr = payload.create_pr.unwrap_or(true);

    let repo_path = wave.repo().to_string();
    let land_result = run_blocking_result(
        move || {
            let progress = crate::ops::NullProgress;
            crate::ops::land(
                std::path::Path::new(&repo_path),
                &crate::ops::LandOptions {
                    strict,
                    local,
                    create_pr,
                    worktree: Some(worktree),
                    commit_message: None,
                    pr_title: None,
                    pr_body: None,
                    agent: None,
                },
                &progress,
            )
        },
        StatusCode::BAD_REQUEST,
    )
    .await?;

    // Sync PR info back to the run so downstream code (CI webhooks) can find it.
    if let (Some(mut run), Some(pr_info)) = (latest_run, land_result) {
        run.pr = Some(crate::lfd::types::PullRequest {
            url: pr_info.url,
            number: Some(pr_info.number as u32),
            state: Some(pr_info.state),
            title: None,
            branch: Some(pr_info.branch),
        });
        if let Err(err) = state.store.update_run(&run).await {
            tracing::warn!(run_id = %run.id, error = %err, "failed to sync PR to run after land");
        }
    }

    Ok(Json(LandWaveResponse { merged: true }))
}

pub async fn next_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<NextWaveResponse> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let (wave, work_dir) = wave_and_work_dir(&state, &wave_id).await?;

    let wave_name = wave.name().clone();
    let result = run_blocking_result(
        move || {
            let progress = crate::ops::NullProgress;
            crate::ops::next_branch(
                std::path::Path::new(&work_dir),
                &crate::ops::NextOptions {
                    wave_name: Some(wave_name),
                    create_pr: true,
                    ..Default::default()
                },
                &progress,
            )
        },
        StatusCode::BAD_REQUEST,
    )
    .await?;

    Ok(Json(NextWaveResponse {
        new_branch: result.new_branch,
    }))
}

pub async fn combine_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<CombineResponse> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let (wave, work_dir) = wave_and_work_dir(&state, &wave_id).await?;
    let wave_name = wave.name().clone();

    let result = run_blocking_result(
        move || {
            crate::ops::combine_prs(
                std::path::Path::new(&work_dir),
                &crate::ops::CombineOptions {
                    wave_name: Some(wave_name),
                },
                &crate::ops::NullProgress,
            )
        },
        StatusCode::BAD_REQUEST,
    )
    .await?;

    Ok(Json(CombineResponse {
        ok: true,
        result: CombineResponseResult {
            new_pr_url: result.new_pr_url,
            closed_prs: result.closed_prs,
        },
    }))
}

#[derive(Debug, Deserialize)]
pub struct WaveDiffQuery {
    path: String,
}

pub async fn get_wave_file_diff_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Query(query): Query<WaveDiffQuery>,
) -> ApiResult<serde_json::Value> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    let file_path = query.path;
    if file_path.contains("..") {
        return Err(api_error(StatusCode::BAD_REQUEST, "invalid file path"));
    }

    let repo = wave.repo().to_string();
    let wave_name = wave.name().clone();
    let diff = tokio::task::spawn_blocking(move || {
        let repo_path = std::path::Path::new(&repo);
        let worktree = crate::engine::worktrees::worktree_path(repo_path, &wave_name);
        if !worktree.exists() {
            return String::new();
        }
        let diff_ref = super::nearest_base_ref(&worktree, &wave_name);
        super::git_file_diff(&worktree, &diff_ref, &file_path)
    })
    .await
    .map_err(|err| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiMessage::Untrusted(err.to_string()),
        )
    })?;

    Ok(Json(serde_json::json!({ "diff": diff })))
}

fn wave_name_exists_error(name: &str) -> ApiError {
    api_error(
        StatusCode::CONFLICT,
        ApiMessage::Safe(format!("wave '{name}' already exists in this repo")),
    )
}

async fn wave_and_work_dir(state: &HttpState, wave_id: &LfdId) -> Result<(Wave, String), ApiError> {
    let wave = state
        .store
        .get_wave(wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    let latest_run = state
        .store
        .get_latest_run(wave_id)
        .await
        .map_err(map_store_error)?;

    let latest_worktree = latest_run
        .map(|run| run.worktree)
        .filter(|value| !value.is_empty());
    let work_dir = resolve_wave_work_dir_for_api(
        wave.repo().to_string(),
        wave.name().clone(),
        latest_worktree,
    )
    .await?;

    Ok((wave, work_dir))
}

async fn resolve_wave_work_dir_for_api(
    repo_path: String,
    wave_name: String,
    latest_worktree: Option<String>,
) -> Result<String, ApiError> {
    run_blocking_result(
        move || resolve_wave_work_dir(&repo_path, &wave_name, latest_worktree),
        StatusCode::BAD_REQUEST,
    )
    .await
}

fn map_join_error(err: tokio::task::JoinError) -> ApiError {
    api_error(
        StatusCode::INTERNAL_SERVER_ERROR,
        ApiMessage::Untrusted(err.to_string()),
    )
}

async fn run_blocking_result<T, E, F>(func: F, failure_status: StatusCode) -> Result<T, ApiError>
where
    T: Send + 'static,
    E: std::fmt::Display + Send + 'static,
    F: FnOnce() -> Result<T, E> + Send + 'static,
{
    tokio::task::spawn_blocking(func)
        .await
        .map_err(map_join_error)?
        .map_err(|err| api_error(failure_status, ApiMessage::Untrusted(err.to_string())))
}

fn resolve_wave_work_dir(
    repo_path: &str,
    wave_name: &str,
    latest_worktree: Option<String>,
) -> Result<String, String> {
    if let Some(worktree) = latest_worktree {
        let path = FsPath::new(&worktree);
        if path.exists() && path.join(".git").exists() {
            return Ok(worktree);
        }
        warn!(
            worktree = %path.display(),
            wave_name,
            "stored worktree missing; recreating wave worktree"
        );
    }

    let lease =
        ensure_wave_worktree(FsPath::new(repo_path), wave_name).map_err(|err| err.to_string())?;
    Ok(lease.path.to_string_lossy().to_string())
}

/// Move a wave's worktree and rename its branch to match the new name.
/// Returns Ok(()) if no worktree exists (legacy wave). Returns Err with a
/// user-facing message if the rename is not possible.
fn rename_wave_worktree(
    repo: &std::path::Path,
    old_name: &str,
    new_name: &str,
) -> Result<(), String> {
    use crate::engine::git::worktree_move;

    let old_wt = worktree_path(repo, old_name);
    if !old_wt.exists() {
        return Ok(());
    }

    let new_wt = worktree_path(repo, new_name);
    if new_wt.exists() {
        return Err(format!(
            "destination worktree already exists: {}",
            new_wt.display()
        ));
    }

    // Compute new branch name by substituting the sanitized wave name.
    let old_branch = current_branch(&old_wt)
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "worktree is in detached HEAD state".to_string())?;

    let old_sanitized = sanitize_for_branch(old_name);
    let new_sanitized = sanitize_for_branch(new_name);
    let new_branch = old_branch.replacen(&old_sanitized, &new_sanitized, 1);

    if new_branch != old_branch && branch_exists(repo, &new_branch).map_err(|e| e.to_string())? {
        return Err(format!("branch already exists: {new_branch}"));
    }

    // Move worktree directory.
    worktree_move(repo, &old_wt, &new_wt).map_err(|e| e.to_string())?;

    // Rename branch.
    if new_branch != old_branch {
        branch_rename(repo, &old_branch, &new_branch).map_err(|e| e.to_string())?;

        // Update remote (best-effort).
        let _ = push_with_upstream(repo, "origin", &new_branch);
        let _ = delete_remote_branch(repo, "origin", &old_branch);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::http::routes::test_helpers::{init_git_repo, test_http_state};
    use crate::lfd::types::{Session, SessionStatus, SessionUse, Wave};
    use std::path::Path;
    use tempfile::tempdir;
    use time::OffsetDateTime;

    fn make_wave(repo: &str, name: &str) -> Wave {
        Wave {
            id: LfdId::new(),
            name: name.to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            repo: repo.to_string(),
            worktree: String::new(),
            branch: String::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            direction: Vec::new(),
            area: Vec::new(),
            paused: false,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
            parent_wave_id: None,
        }
    }

    fn make_session(wave: &Wave, session_use: SessionUse, cwd: &Path) -> Session {
        Session {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: None,
            parent_session_id: None,
            session_use,
            step: "implement".to_string(),
            agent: "lf".to_string(),
            cwd: cwd.to_string_lossy().to_string(),
            argv: Vec::new(),
            env: Default::default(),
            source: "wave_step_tmux".to_string(),
            tmux_name: "lf-worker-caller".to_string(),
            status: SessionStatus::Running,
            attached_at: None,
            started_at: Some(OffsetDateTime::now_utc()),
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: None,
        }
    }

    #[tokio::test]
    async fn get_wave_agent_tree_returns_sessions_with_connection() {
        let state = test_http_state().await;
        let repo = tempdir().expect("tempdir");
        init_git_repo(repo.path());
        let wave = make_wave(&repo.path().to_string_lossy(), "tree-wave");
        state.store.create_wave(&wave).await.expect("seed wave");
        let parent = make_session(&wave, SessionUse::WaveAgent, repo.path());
        let mut child = make_session(&wave, SessionUse::Worker, repo.path());
        child.parent_session_id = Some(parent.id.clone());
        child.tmux_name = "lf-worker".to_string();
        state
            .store
            .create_control_session(&parent)
            .await
            .expect("seed parent session");
        state
            .store
            .create_control_session(&child)
            .await
            .expect("seed child session");

        let Json(response) = get_wave_agent_tree_handler(
            State(state.clone()),
            HeaderMap::new(),
            Path(wave.id().to_string()),
            Query(GetWaveAgentTreeQuery {
                active_only: Some(true),
            }),
        )
        .await
        .expect("get wave agent tree");

        assert_eq!(response.object, "wave_agent_tree");
        assert_eq!(response.wave.id, wave.id().to_string());
        // This wave is a leaf (no child waves), so the chord tree is empty here.
        assert_eq!(response.child_waves.len(), 0);
        let child_node = response
            .sessions
            .iter()
            .find(|node| node.session.id == child.id.to_string())
            .expect("child session in tree");
        assert_eq!(
            child_node.session.parent_session_id.as_deref(),
            Some(parent.id.as_str())
        );
        assert_eq!(
            child_node
                .connection
                .as_ref()
                .map(|connection| connection.session_name.as_str()),
            Some("lf-worker")
        );
    }

    // A chord's WaveAgentTree exposes its children: one leaf child wave per
    // repo, each with its own repo. This is the two-repo chord shape — the
    // ancestry query drives which children a chord would loop.
    #[tokio::test]
    async fn get_wave_agent_tree_returns_child_waves_for_chord() {
        let state = test_http_state().await;
        let repo_a = tempdir().expect("tempdir a");
        let repo_b = tempdir().expect("tempdir b");
        init_git_repo(repo_a.path());
        init_git_repo(repo_b.path());

        let chord = make_wave(&repo_a.path().to_string_lossy(), "chord-root");
        state.store.create_wave(&chord).await.expect("seed chord");

        let child_a = make_wave(&repo_a.path().to_string_lossy(), "chord-child-a")
            .with_parent(chord.id().clone());
        let child_b = make_wave(&repo_b.path().to_string_lossy(), "chord-child-b")
            .with_parent(chord.id().clone());
        state
            .store
            .create_wave(&child_a)
            .await
            .expect("seed child a");
        state
            .store
            .create_wave(&child_b)
            .await
            .expect("seed child b");

        let Json(response) = get_wave_agent_tree_handler(
            State(state.clone()),
            HeaderMap::new(),
            Path(chord.id().to_string()),
            Query(GetWaveAgentTreeQuery {
                active_only: Some(true),
            }),
        )
        .await
        .expect("get wave agent tree");

        assert_eq!(response.child_waves.len(), 2);
        let repos: Vec<&str> = response
            .child_waves
            .iter()
            .map(|w| w.repo.as_str())
            .collect();
        assert!(repos.contains(&repo_a.path().to_string_lossy().as_ref()));
        assert!(repos.contains(&repo_b.path().to_string_lossy().as_ref()));
    }

    #[tokio::test]
    async fn stop_wave_cancels_active_session_for_waiting_run() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_tmp.path());
        let repo = repo_tmp.path().to_string_lossy().to_string();

        let mut wave = make_wave(&repo, "designer");
        wave.set_status(WaveStatus::Running);
        state
            .store
            .create_wave(&wave)
            .await
            .expect("wave should be created");

        let mut run = Run::new(LfdId::new(), wave.id().clone());
        run.repo = repo.clone();
        run.flow = "build".to_string();
        run.status = RunStatus::Waiting;
        run.worktree = repo.clone();
        run.branch = "main".to_string();
        state
            .store
            .create_run(&run)
            .await
            .expect("run should be created");

        let session = Session {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            run_id: Some(run.id.clone()),
            parent_session_id: None,
            session_use: SessionUse::Worker,
            step: "design".to_string(),
            agent: "lf".to_string(),
            cwd: repo.clone(),
            argv: vec!["lf".to_string(), "design".to_string()],
            env: Default::default(),
            source: "wave_step".to_string(),
            tmux_name: "lf-test-branch".to_string(),
            status: SessionStatus::Running,
            attached_at: Some(OffsetDateTime::now_utc()),
            started_at: Some(OffsetDateTime::now_utc()),
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: Some("token".to_string()),
        };
        state
            .store
            .create_control_session(&session)
            .await
            .expect("terminal session should be created");

        let Json(response) = stop_wave_handler(State(state.clone()), Path(wave.id().to_string()))
            .await
            .expect("stop wave");
        assert!(response.stopped);

        let updated_run = state
            .store
            .get_run(&run.id)
            .await
            .expect("run lookup should succeed")
            .expect("run should exist");
        assert_eq!(updated_run.status, RunStatus::Failed);
        assert_eq!(updated_run.error.as_deref(), Some("stopped"));

        let updated_wave = state
            .store
            .get_wave(wave.id())
            .await
            .expect("wave lookup should succeed")
            .expect("wave should exist");
        assert_eq!(updated_wave.status(), WaveStatus::Failed);

        let updated_session = state
            .store
            .get_control_session(&session.id)
            .await
            .expect("session lookup should succeed")
            .expect("session should exist");
        assert_eq!(updated_session.status, SessionStatus::Canceled);
        assert!(updated_session.completed_at.is_some());
    }

    #[tokio::test]
    async fn list_with_repo_filter() {
        let state = test_http_state().await;
        let repo_a_tmp = tempdir().expect("tempdir");
        let repo_b_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_a_tmp.path());
        init_git_repo(repo_b_tmp.path());
        let repo_a = repo_a_tmp.path().to_string_lossy().to_string();
        let repo_b = repo_b_tmp.path().to_string_lossy().to_string();

        state
            .store
            .create_wave(&make_wave(&repo_a, "wave-a"))
            .await
            .expect("create wave in repo a");
        state
            .store
            .create_wave(&make_wave(&repo_b, "wave-b"))
            .await
            .expect("create wave in repo b");

        let Json(listed) = list_waves_handler(
            State(state),
            Query(ListWavesQuery {
                repo: Some(repo_a.clone()),
                limit: None,
                starting_after: None,
                ending_before: None,
                expand: ExpandParam::default(),
            }),
        )
        .await
        .expect("list waves");

        assert_eq!(listed.data.len(), 1);
        assert_eq!(listed.data[0].repo, repo_a);
        assert_eq!(listed.data[0].name, "wave-a");
    }

    #[tokio::test]
    async fn update_fields_handler() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_tmp.path());
        let repo = repo_tmp.path().to_string_lossy().to_string();

        let mut wave = make_wave(&repo, "before");
        wave.direction = vec!["infra".to_string()];
        wave.area = vec!["src/".to_string()];
        state.store.create_wave(&wave).await.expect("create wave");
        let created_id = wave.id().to_string();

        let Json(updated) = update_wave_handler(
            State(state.clone()),
            Path(created_id.clone()),
            Json(UpdateWaveRequest {
                name: Some("after".to_string()),
                direction: Some(vec!["clarity".to_string()]),
                area: Some(vec!["docs/".to_string()]),
                ..Default::default()
            }),
        )
        .await
        .expect("update wave");

        assert_eq!(updated.name, "after");
        assert_eq!(updated.direction, vec!["clarity".to_string()]);
        assert_eq!(updated.area, vec!["docs/".to_string()]);

        let Json(found) = get_wave_handler(
            State(state),
            Path(created_id),
            Query(ExpandQuery::default()),
        )
        .await
        .expect("get updated wave");
        assert_eq!(found.name, "after");
        assert_eq!(found.direction, vec!["clarity".to_string()]);
        assert_eq!(found.area, vec!["docs/".to_string()]);
    }

    #[tokio::test]
    async fn delete_then_get_returns_not_found() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_tmp.path());
        let repo = repo_tmp.path().to_string_lossy().to_string();

        let wave = make_wave(&repo, "delete-me");
        state.store.create_wave(&wave).await.expect("create wave");
        let created_id = wave.id().to_string();

        let Json(deleted) = delete_wave_handler(State(state.clone()), Path(created_id.clone()))
            .await
            .expect("delete wave");
        assert!(deleted.deleted);

        let missing = get_wave_handler(
            State(state),
            Path(created_id),
            Query(ExpandQuery::default()),
        )
        .await;
        assert!(matches!(missing, Err((StatusCode::NOT_FOUND, _))));
    }
}
