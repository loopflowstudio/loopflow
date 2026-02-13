use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use std::path::Path as FsPath;
use std::str::FromStr;
use time::OffsetDateTime;
use tracing::warn;

use crate::engine::git::{branch_rename, current_branch, delete_remote_branch, push_with_upstream};
use crate::engine::naming::sanitize_for_branch;
use crate::engine::platform::kill_process;
use crate::engine::worktree::remove_worktree;
use crate::engine::worktrees::{branch_exists, worktree_path};
use crate::lfd::executor::{create_wave_run_with_id, ensure_wave_worktree};
use crate::lfd::http::dto::{
    stimulus_dto, stimulus_kind_str, CombineResponse, CombineResponseResult, ContinueWaveResponse,
    DeletedResourceResponse, ErrorResponse, LandWaveResponse, ListResponse, NextWaveResponse,
    RunWaveResponse, StopWaveResponse, WaveDto,
};
use crate::lfd::http::routes::{build_wave_dto, resolve_wave_id};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, run_store, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::triggers::spawn_run_task_with_slot;
use crate::lfd::types::{
    AgentStatus, Event, Stimulus, StimulusKind, Wave, WaveRun, WaveRunStatus, WaveStatus,
};

#[derive(Deserialize)]
pub struct ListWavesQuery {
    repo: Option<String>,
    limit: Option<u32>,
    starting_after: Option<String>,
    ending_before: Option<String>,
    #[serde(default, rename = "expand[]")]
    expand: ExpandParam,
}

#[derive(Deserialize, Default)]
pub struct ExpandQuery {
    #[serde(default, rename = "expand[]")]
    expand: ExpandParam,
}

/// Accept `expand[]=value` as either a single string or repeated params.
#[derive(Default, Clone)]
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

#[derive(Deserialize)]
pub struct CreateWaveRequest {
    repo: String,
    name: Option<String>,
    flow: Option<String>,
    direction: Option<Vec<String>>,
    area: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
pub struct UpdateWaveRequest {
    name: Option<String>,
    flow: Option<String>,
    direction: Option<Vec<String>>,
    area: Option<Vec<String>>,
    status: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct RunWaveRequest {
    area: Option<Vec<String>>,
    direction: Option<Vec<String>>,
    flow: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddStimulusRequest {
    kind: StimulusKind,
    cron: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct LandWaveRequest {
    strict: Option<bool>,
    local: Option<bool>,
    create_pr: Option<bool>,
    worktree: Option<String>,
    lint: Option<bool>,
}

pub async fn list_waves_handler(
    State(state): State<HttpState>,
    Query(query): Query<ListWavesQuery>,
) -> ApiResult<ListResponse<WaveDto>> {
    let waves = run_store(&state.store, move |store| {
        store.list_waves(query.repo.as_deref())
    })
    .await
    .map_err(map_store_error)?;
    let include_active_run = query.expand.contains("active_run");
    let (waves, has_more) = super::paginate(
        waves,
        query.limit,
        query.starting_after.as_deref(),
        query.ending_before.as_deref(),
        |w| &w.id,
    );
    let views = crate::lfd::http::routes::build_wave_dtos(&state.store, waves, include_active_run)
        .await
        .map_err(map_store_error)?;
    Ok(Json(ListResponse::new(views, has_more)))
}

pub async fn create_wave_handler(
    State(state): State<HttpState>,
    Json(payload): Json<CreateWaveRequest>,
) -> ApiResult<WaveDto> {
    let id = LfdId::new();
    let name = payload.name.unwrap_or_else(|| format!("wave-{}", id));
    let flow = payload.flow.unwrap_or_else(|| "ship".to_string());

    // Check for duplicate wave name in the same repo.
    let repo_for_check = payload.repo.clone();
    let name_for_check = name.clone();
    let existing = run_store(&state.store, move |store| {
        let waves = store.list_waves(Some(&repo_for_check))?;
        Ok(waves.into_iter().any(|w| w.name == name_for_check))
    })
    .await
    .map_err(map_store_error)?;
    if existing {
        return Err(api_error(
            StatusCode::CONFLICT,
            format!("wave '{}' already exists in this repo", name),
        ));
    }

    let wave = Wave {
        id: id.clone(),
        name,
        repo: payload.repo,
        flow,
        direction: payload.direction.unwrap_or_default(),
        area: payload.area.unwrap_or_default(),
        status: WaveStatus::Idle,
        iteration: 0,
        created_at: Some(OffsetDateTime::now_utc()),
    };
    let wave_clone = wave.clone();
    run_store(&state.store, move |store| store.create_wave(&wave_clone))
        .await
        .map_err(map_store_error)?;

    // Create worktree eagerly so it exists immediately.
    let repo_for_wt = wave.repo.clone();
    let name_for_wt = wave.name.clone();
    let wt_result = tokio::task::spawn_blocking(move || {
        ensure_wave_worktree(std::path::Path::new(&repo_for_wt), &name_for_wt)
    })
    .await
    .map_err(|err| err.to_string())
    .and_then(|r| r.map_err(|err| err.to_string()));

    if let Err(err) = wt_result {
        let wave_id = wave.id.clone();
        let _ = run_store(&state.store, move |store| store.delete_wave(&wave_id)).await;
        return Err(api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("failed to create worktree: {err}"),
        ));
    }

    state
        .event_hub
        .send(Event::wave_created(wave.id.clone(), wave.name.clone()));

    let view = build_wave_dto(&state.store, wave, false)
        .await
        .map_err(map_store_error)?;
    Ok(Json(view))
}

pub async fn get_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Query(query): Query<ExpandQuery>,
) -> ApiResult<WaveDto> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let wave = run_store(&state.store, move |store| store.get_wave(&wave_id))
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;
    let include_active_run = query.expand.contains("active_run");
    let view = build_wave_dto(&state.store, wave, include_active_run)
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
    let mut wave = run_store(&state.store, {
        let wave_id = wave_id.clone();
        move |store| store.get_wave(&wave_id)
    })
    .await
    .map_err(map_store_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    // Handle rename: move worktree + rename branch before updating DB.
    if let Some(ref name) = payload.name {
        if !name.is_empty() && *name != wave.name {
            let new_name = name.clone();

            // Check for duplicate name in same repo.
            let repo_for_check = wave.repo.clone();
            let name_for_check = new_name.clone();
            let existing = run_store(&state.store, move |store| {
                let waves = store.list_waves(Some(&repo_for_check))?;
                Ok(waves.into_iter().any(|w| w.name == name_for_check))
            })
            .await
            .map_err(map_store_error)?;
            if existing {
                return Err(api_error(
                    StatusCode::CONFLICT,
                    format!("wave '{}' already exists in this repo", new_name),
                ));
            }

            // Reject rename while wave is running or waiting.
            let active_run = run_store(&state.store, {
                let wave_id = wave.id.clone();
                move |store| store.get_active_wave_run(&wave_id)
            })
            .await
            .map_err(map_store_error)?;
            if let Some(run) = active_run {
                if matches!(run.status, WaveRunStatus::Running | WaveRunStatus::Waiting) {
                    return Err(api_error(
                        StatusCode::PRECONDITION_FAILED,
                        "cannot rename wave while running",
                    ));
                }
            }

            // Move worktree + rename branch on disk.
            let old_name = wave.name.clone();
            let repo = wave.repo.clone();
            let new_name_for_wt = new_name.clone();
            tokio::task::spawn_blocking(move || {
                rename_wave_worktree(std::path::Path::new(&repo), &old_name, &new_name_for_wt)
            })
            .await
            .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
            .map_err(|err| api_error(StatusCode::CONFLICT, err))?;

            wave.name = new_name;
        }
    }

    if let Some(flow) = payload.flow {
        wave.flow = flow;
    }
    if let Some(direction) = payload.direction {
        wave.direction = direction;
    }
    if let Some(area) = payload.area {
        wave.area = area;
    }
    if let Some(status) = payload.status {
        wave.status = WaveStatus::from_str(&status)
            .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid status"))?;
    }

    let wave_clone = wave.clone();
    run_store(&state.store, move |store| store.update_wave(&wave_clone))
        .await
        .map_err(map_store_error)?;

    state.event_hub.send(Event::wave_updated(wave.id.clone()));

    let view = build_wave_dto(&state.store, wave, false)
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
    let wave = run_store(&state.store, {
        let wave_id = wave_id.clone();
        move |store| store.get_wave(&wave_id)
    })
    .await
    .map_err(map_store_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    terminate_active_agents(&state, &wave_id).await?;

    // Delete from the store (cascades to wave_runs, agents, etc.).
    let wave_id_for_delete = wave_id.clone();
    run_store(&state.store, move |store| {
        store.delete_wave(&wave_id_for_delete)
    })
    .await
    .map_err(map_store_error)?;

    // Clean up the worktree on disk.
    let repo = wave.repo.clone();
    let wave_name = wave.name.clone();
    tokio::task::spawn_blocking(move || {
        let wt = worktree_path(std::path::Path::new(&repo), &wave_name);
        if wt.exists() {
            if let Err(err) = remove_worktree(&wt, false) {
                tracing::warn!(worktree = %wt.display(), error = %err, "failed to remove worktree");
            }
        }
    })
    .await
    .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    state.event_hub.send(Event::wave_deleted(wave_id.clone()));

    Ok(Json(DeletedResourceResponse {
        id: wave_id.to_string(),
        object: "wave".to_string(),
        deleted: true,
    }))
}

pub async fn run_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    payload: Option<Json<RunWaveRequest>>,
) -> ApiResult<RunWaveResponse> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let payload = payload.map(|Json(value)| value).unwrap_or_default();
    let mut wave = run_store(&state.store, {
        let wave_id = wave_id.clone();
        move |store| store.get_wave(&wave_id)
    })
    .await
    .map_err(map_store_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    let active_run = run_store(&state.store, {
        let wave_id = wave_id.clone();
        move |store| store.get_active_wave_run(&wave_id)
    })
    .await
    .map_err(map_store_error)?;
    if active_run.is_some() {
        return Err(api_error(
            StatusCode::PRECONDITION_FAILED,
            "wave already running",
        ));
    }

    if let Some(flow) = payload.flow {
        wave.flow = flow;
    }
    if let Some(direction) = payload.direction {
        wave.direction = direction;
    }
    if let Some(area) = payload.area {
        wave.area = area;
    }

    let wave_clone = wave.clone();
    run_store(&state.store, move |store| store.update_wave(&wave_clone))
        .await
        .map_err(map_store_error)?;

    // Re-enable all stimuli (they may have been disabled by a previous stop).
    let _ = set_wave_stimuli_enabled(&state, &wave.id, true, false).await;

    let run_id = LfdId::new();
    let (acquired, _) = state.scheduler.acquire(run_id.as_str()).await;
    if !acquired {
        return Ok(Json(RunWaveResponse {
            started: false,
            wave_id: wave.id.to_string(),
            wave_run_id: None,
        }));
    }

    let run = match tokio::task::spawn_blocking({
        let store = state.store.clone();
        let wave = wave.clone();
        let run_id = run_id.clone();
        move || create_wave_run_with_id(&store, &wave, &run_id)
    })
    .await
    {
        Ok(Ok(run)) => run,
        Ok(Err(err)) => {
            state.scheduler.release(run_id.as_str());
            return Err(api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            ));
        }
        Err(err) => {
            state.scheduler.release(run_id.as_str());
            return Err(api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            ));
        }
    };

    spawn_run_task_with_slot(
        state.store.clone(),
        (*state.executor).clone(),
        state.scheduler.clone(),
        state.event_hub.clone(),
        run.clone(),
    );

    Ok(Json(RunWaveResponse {
        started: true,
        wave_id: wave.id.to_string(),
        wave_run_id: Some(run.id.to_string()),
    }))
}

// Stimulus CRUD handlers

pub async fn add_stimulus_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Json(payload): Json<AddStimulusRequest>,
) -> ApiResult<serde_json::Value> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    // Verify wave exists.
    run_store(&state.store, {
        let wave_id = wave_id.clone();
        move |store| store.get_wave(&wave_id)
    })
    .await
    .map_err(map_store_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    let stimulus = Stimulus {
        id: LfdId::new(),
        wave_id,
        kind: payload.kind,
        cron: payload.cron.unwrap_or_default(),
        last_main_sha: None,
        last_triggered_at: None,
        created_at: Some(OffsetDateTime::now_utc()),
        enabled: true,
    };

    let stimulus_clone = stimulus.clone();
    run_store(&state.store, move |store| {
        store.create_stimulus(&stimulus_clone)
    })
    .await
    .map_err(map_store_error)?;

    Ok(Json(serde_json::json!({
        "id": stimulus.id.to_string(),
        "kind": stimulus_kind_str(stimulus.kind),
        "cron": if stimulus.cron.is_empty() { None } else { Some(&stimulus.cron) },
    })))
}

pub async fn remove_stimulus_handler(
    State(state): State<HttpState>,
    Path((wave_id, stimulus_id)): Path<(String, String)>,
) -> ApiResult<serde_json::Value> {
    let _wave_id = resolve_wave_id(&state, &wave_id).await?;
    let stimulus_id = LfdId::from_str(&stimulus_id)
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid stimulus id"))?;

    run_store(&state.store, move |store| {
        store.delete_stimulus(&stimulus_id)
    })
    .await
    .map_err(map_store_error)?;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub async fn list_stimuli_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    let stimuli = run_store(&state.store, move |store| {
        store.list_stimuli(Some(&wave_id))
    })
    .await
    .map_err(map_store_error)?;

    let dtos: Vec<_> = stimuli.into_iter().map(stimulus_dto).collect();

    Ok(Json(serde_json::json!({ "data": dtos })))
}

pub async fn stop_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<StopWaveResponse> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    terminate_active_agents(&state, &wave_id).await?;

    let run = run_store(&state.store, {
        let wave_id = wave_id.clone();
        move |store| store.get_active_wave_run(&wave_id)
    })
    .await
    .map_err(map_store_error)?;

    // Disable all auto stimuli so tickers won't restart the wave.
    let has_auto_stimulus = set_wave_stimuli_enabled(&state, &wave_id, false, true).await;

    if let Some(mut run) = run {
        run.status = WaveRunStatus::Failed;
        run.error = Some("stopped".to_string());
        run.ended_at = Some(OffsetDateTime::now_utc());
        let run_clone = run.clone();
        run_store(&state.store, move |store| store.update_wave_run(&run_clone))
            .await
            .map_err(map_store_error)?;
        let wave_id = run.wave_id.clone();
        run_store(&state.store, move |store| {
            if let Some(mut wave) = store.get_wave(&wave_id)? {
                wave.status = if has_auto_stimulus {
                    WaveStatus::Paused
                } else {
                    WaveStatus::Failed
                };
                store.update_wave(&wave)?;
            }
            // Mark agents as ended in the store.
            let _ = store.end_active_agent_for_wave(
                &wave_id,
                AgentStatus::Failed.as_i32(),
                OffsetDateTime::now_utc().unix_timestamp(),
            );
            Ok(())
        })
        .await
        .map_err(map_store_error)?;
    } else if has_auto_stimulus {
        // No active run, but still pause the wave so the auto stimulus doesn't restart it.
        let wid = wave_id.clone();
        run_store(&state.store, move |store| {
            if let Some(mut wave) = store.get_wave(&wid)? {
                wave.status = WaveStatus::Paused;
                store.update_wave(&wave)?;
            }
            Ok(())
        })
        .await
        .map_err(map_store_error)?;
    }

    state.event_hub.send(Event::wave_stopped(wave_id));

    Ok(Json(StopWaveResponse { stopped: true }))
}

async fn terminate_active_agents(
    state: &HttpState,
    wave_id: &LfdId,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let agents = run_store(&state.store, {
        let wave_id = wave_id.clone();
        move |store| store.get_active_agents_for_wave(&wave_id)
    })
    .await
    .map_err(map_store_error)?;

    for agent in agents {
        if let Err(err) = state.executor.terminate_agent(&agent.id).await {
            warn!(agent_id = %agent.id, error = %err, "failed to terminate agent executor handle");
        }
        if let Some(pid) = agent.pid {
            kill_process(pid);
        }
    }

    Ok(())
}

pub async fn continue_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<ContinueWaveResponse> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let mut run = run_store(&state.store, {
        let wave_id = wave_id.clone();
        move |store| store.get_active_wave_run(&wave_id)
    })
    .await
    .map_err(map_store_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "no active run for wave"))?;

    if run.status != WaveRunStatus::Waiting {
        return Err(api_error(
            StatusCode::PRECONDITION_FAILED,
            "wave run is not waiting for interactive input",
        ));
    }

    let mut wave = run_store(&state.store, {
        let wave_id = wave_id.clone();
        move |store| store.get_wave(&wave_id)
    })
    .await
    .map_err(map_store_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    // Resolve worktree and check for uncommitted changes.
    let worktree = run.worktree.clone();

    // Resolve the current step name for the commit message.
    let step_name = resolve_current_step_name(&run, &wave, run.step_index);

    let worktree_path = worktree.clone();
    let step_name_for_commit = step_name.clone();
    tokio::task::spawn_blocking(move || {
        auto_commit_if_dirty(std::path::Path::new(&worktree_path), &step_name_for_commit)
    })
    .await
    .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
    .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

    // Advance to the next step.
    run.step_index += 1;
    run.status = WaveRunStatus::Running;
    let run_clone = run.clone();
    run_store(&state.store, move |store| store.update_wave_run(&run_clone))
        .await
        .map_err(map_store_error)?;
    wave.status = WaveStatus::Running;
    let wave_clone = wave.clone();
    run_store(&state.store, move |store| store.update_wave(&wave_clone))
        .await
        .map_err(map_store_error)?;

    state.event_hub.send(Event::wave_updated(wave_id.clone()));

    // Re-acquire scheduler slot (idempotent for same run_id).
    let (acquired, _) = state.scheduler.acquire(run.id.as_str()).await;
    if !acquired {
        return Err(api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "no scheduler slots available",
        ));
    }

    spawn_run_task_with_slot(
        state.store.clone(),
        (*state.executor).clone(),
        state.scheduler.clone(),
        state.event_hub.clone(),
        run.clone(),
    );

    Ok(Json(ContinueWaveResponse {
        continued: true,
        wave_id: wave_id.to_string(),
        wave_run_id: run.id.to_string(),
    }))
}

pub async fn land_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    payload: Option<Json<LandWaveRequest>>,
) -> ApiResult<LandWaveResponse> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let payload = payload.map(|Json(value)| value).unwrap_or_default();
    let wave_id_for_event = wave_id.clone();
    let wave = run_store(&state.store, move |store| store.get_wave(&wave_id))
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    let latest_run = run_store(&state.store, move |store| {
        let wave_id = LfdId::from_raw(wave.id.clone());
        store.get_latest_wave_run(&wave_id)
    })
    .await
    .map_err(map_store_error)?;

    let latest_worktree = payload
        .worktree
        .or_else(|| latest_run.as_ref().map(|run| run.worktree.clone()))
        .filter(|value| !value.is_empty());
    let worktree =
        resolve_wave_work_dir_for_api(wave.repo.clone(), wave.name.clone(), latest_worktree)
            .await?;

    let strict = payload.strict.unwrap_or(false);
    let local = payload.local.unwrap_or(false);
    let create_pr = payload.create_pr.unwrap_or(true);
    let lint = payload.lint.unwrap_or(true);

    let repo_path = wave.repo.clone();
    tokio::task::spawn_blocking(move || {
        let progress = crate::ops::NullProgress;
        crate::ops::land(
            std::path::Path::new(&repo_path),
            &crate::ops::LandOptions {
                strict,
                local,
                create_pr,
                worktree: Some(worktree),
                lint,
            },
            &progress,
        )
    })
    .await
    .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
    .map_err(|err| api_error(StatusCode::BAD_REQUEST, err.to_string()))?;

    state.event_hub.send(Event::wave_updated(wave_id_for_event));

    Ok(Json(LandWaveResponse { merged: true }))
}

pub async fn next_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<NextWaveResponse> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let (wave, work_dir) = wave_and_work_dir(&state, &wave_id).await?;

    let wave_name = wave.name.clone();
    let result = tokio::task::spawn_blocking(move || {
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
    })
    .await
    .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
    .map_err(|err| api_error(StatusCode::BAD_REQUEST, err.to_string()))?;

    state.event_hub.send(Event::wave_updated(wave_id));

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
    let wave_name = wave.name.clone();

    let result = tokio::task::spawn_blocking(move || {
        crate::ops::combine_prs(
            std::path::Path::new(&work_dir),
            &crate::ops::CombineOptions {
                wave_name: Some(wave_name),
            },
            &crate::ops::NullProgress,
        )
    })
    .await
    .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
    .map_err(|err| api_error(StatusCode::BAD_REQUEST, err.to_string()))?;

    state.event_hub.send(Event::wave_updated(wave_id));

    Ok(Json(CombineResponse {
        ok: true,
        result: CombineResponseResult {
            new_pr_url: result.new_pr_url,
            closed_prs: result.closed_prs,
        },
    }))
}

async fn wave_and_work_dir(state: &HttpState, wave_id: &LfdId) -> Result<(Wave, String), ApiError> {
    let wave = run_store(&state.store, {
        let wave_id = wave_id.clone();
        move |store| store.get_wave(&wave_id)
    })
    .await
    .map_err(map_store_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    let latest_run = run_store(&state.store, {
        let wave_id = wave_id.clone();
        move |store| store.get_latest_wave_run(&wave_id)
    })
    .await
    .map_err(map_store_error)?;

    let latest_worktree = latest_run
        .map(|run| run.worktree)
        .filter(|value| !value.is_empty());
    let work_dir =
        resolve_wave_work_dir_for_api(wave.repo.clone(), wave.name.clone(), latest_worktree)
            .await?;

    Ok((wave, work_dir))
}

type ApiError = (StatusCode, Json<ErrorResponse>);

async fn resolve_wave_work_dir_for_api(
    repo_path: String,
    wave_name: String,
    latest_worktree: Option<String>,
) -> Result<String, ApiError> {
    tokio::task::spawn_blocking(move || {
        resolve_wave_work_dir(&repo_path, &wave_name, latest_worktree)
    })
    .await
    .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
    .map_err(|err| api_error(StatusCode::BAD_REQUEST, err))
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

    let (worktree, _) =
        ensure_wave_worktree(FsPath::new(repo_path), wave_name).map_err(|err| err.to_string())?;
    Ok(worktree)
}

fn is_auto_stimulus(kind: StimulusKind) -> bool {
    matches!(
        kind,
        StimulusKind::Loop | StimulusKind::Watch | StimulusKind::Cron
    )
}

async fn set_wave_stimuli_enabled(
    state: &HttpState,
    wave_id: &LfdId,
    enabled: bool,
    auto_only: bool,
) -> bool {
    let wave_id = wave_id.clone();
    run_store(&state.store, move |store| {
        let stimuli = store.list_stimuli(Some(&wave_id))?;
        let mut matched = false;
        for mut stimulus in stimuli {
            if auto_only && !is_auto_stimulus(stimulus.kind) {
                continue;
            }
            matched = true;
            if stimulus.enabled != enabled {
                stimulus.enabled = enabled;
                store.update_stimulus(&stimulus)?;
            }
        }
        Ok(matched)
    })
    .await
    .unwrap_or(false)
}

fn resolve_current_step_name(run: &WaveRun, _wave: &Wave, step_index: u32) -> String {
    use crate::engine::flow::{expand_flow, load_flow, next_action, FlowAction};
    let repo = std::path::Path::new(&run.snapshot.repo);
    let name = load_flow(&run.snapshot.flow, repo)
        .ok()
        .and_then(|flow| expand_flow(&flow, repo).ok())
        .and_then(|plan| match next_action(&plan, step_index as usize) {
            FlowAction::WaitInteractive { step } => Some(step.step.name),
            FlowAction::RunStep { step } => Some(step.step.name),
            _ => None,
        });
    name.unwrap_or_else(|| format!("step-{step_index}"))
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

fn auto_commit_if_dirty(worktree: &std::path::Path, step_name: &str) -> Result<(), String> {
    use crate::engine::git::{commit, is_clean, stage_all};
    if is_clean(worktree).map_err(|e| e.to_string())? {
        return Ok(());
    }
    stage_all(worktree).map_err(|e| e.to_string())?;
    let message = format!("lfd: auto-commit after interactive step '{step_name}'");
    commit(worktree, &message).map_err(|e| e.to_string())?;
    Ok(())
}
