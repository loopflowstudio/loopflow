use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use time::OffsetDateTime;

use crate::http::dto::{
    LandWaveResponse, ListWavesResponse, RunWaveResponse, StopWaveResponse, WaveResponse,
};
use crate::http::routes::build_wave_view;
use crate::http::state::HttpState;
use crate::http::{api_error, map_store_error, parse_id, run_store, ApiResult};
use crate::id::LfdId;
use crate::store::{SharedStore, StoreError};
use crate::types::{Event, Wave, WaveRun, WaveRunStatus};

#[derive(Deserialize)]
pub(crate) struct ListWavesQuery {
    repo: Option<String>,
}

#[derive(Deserialize)]
pub(crate) struct CreateWaveRequest {
    repo: String,
    name: Option<String>,
    flow: Option<String>,
    direction: Option<Vec<String>>,
    area: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
pub(crate) struct UpdateWaveRequest {
    flow: Option<String>,
    direction: Option<Vec<String>>,
    area: Option<Vec<String>>,
    paused: Option<bool>,
}

#[derive(Deserialize, Default)]
pub(crate) struct RunWaveRequest {
    area: Option<Vec<String>>,
    direction: Option<Vec<String>>,
    flow: Option<String>,
}

#[derive(Deserialize, Default)]
pub(crate) struct LandWaveRequest {
    strict: Option<bool>,
    local: Option<bool>,
    create_pr: Option<bool>,
    worktree: Option<String>,
    lint: Option<bool>,
}

pub async fn list_waves_handler(
    State(state): State<HttpState>,
    Query(query): Query<ListWavesQuery>,
) -> ApiResult<ListWavesResponse> {
    let waves = run_store(&state.store, move |store| {
        store.list_waves(query.repo.as_deref())
    })
    .await
    .map_err(map_store_error)?;
    let views = crate::http::routes::build_wave_views(&state.store, waves)
        .await
        .map_err(map_store_error)?;
    Ok(Json(ListWavesResponse { waves: views }))
}

pub async fn create_wave_handler(
    State(state): State<HttpState>,
    Json(payload): Json<CreateWaveRequest>,
) -> ApiResult<WaveResponse> {
    let id = LfdId::new();
    let name = payload.name.unwrap_or_else(|| format!("wave-{}", id));
    let flow = payload.flow.unwrap_or_else(|| "ship".to_string());
    let wave = Wave {
        id: id.clone(),
        name,
        repo: payload.repo,
        flow,
        direction: payload.direction.unwrap_or_default(),
        area: payload.area.unwrap_or_default(),
        paused: false,
        created_at: Some(OffsetDateTime::now_utc()),
    };
    let wave_clone = wave.clone();
    run_store(&state.store, move |store| store.create_wave(&wave_clone))
        .await
        .map_err(map_store_error)?;

    state
        .event_hub
        .send(Event::wave_created(wave.id.clone(), wave.name.clone()));

    let view = build_wave_view(&state.store, wave)
        .await
        .map_err(map_store_error)?;
    Ok(Json(WaveResponse { wave: view }))
}

pub async fn get_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<WaveResponse> {
    let wave_id = parse_id(&wave_id)?;
    let wave = run_store(&state.store, move |store| store.get_wave(&wave_id))
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;
    let view = build_wave_view(&state.store, wave)
        .await
        .map_err(map_store_error)?;
    Ok(Json(WaveResponse { wave: view }))
}

pub async fn update_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Json(payload): Json<UpdateWaveRequest>,
) -> ApiResult<WaveResponse> {
    let wave_id = parse_id(&wave_id)?;
    let mut wave = run_store(&state.store, {
        let wave_id = wave_id.clone();
        move |store| store.get_wave(&wave_id)
    })
    .await
    .map_err(map_store_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    if let Some(flow) = payload.flow {
        wave.flow = flow;
    }
    if let Some(direction) = payload.direction {
        if !direction.is_empty() {
            wave.direction = direction;
        }
    }
    if let Some(area) = payload.area {
        if !area.is_empty() {
            wave.area = area;
        }
    }
    if let Some(paused) = payload.paused {
        wave.paused = paused;
    }

    let wave_clone = wave.clone();
    run_store(&state.store, move |store| store.update_wave(&wave_clone))
        .await
        .map_err(map_store_error)?;

    state.event_hub.send(Event::wave_updated(wave.id.clone()));

    let view = build_wave_view(&state.store, wave)
        .await
        .map_err(map_store_error)?;
    Ok(Json(WaveResponse { wave: view }))
}

pub async fn delete_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let wave_id = parse_id(&wave_id)?;
    let wave_id_for_delete = wave_id.clone();
    run_store(&state.store, move |store| {
        store.delete_wave(&wave_id_for_delete)
    })
    .await
    .map_err(map_store_error)?;

    state.event_hub.send(Event::wave_deleted(wave_id));

    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub async fn run_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    payload: Option<Json<RunWaveRequest>>,
) -> ApiResult<RunWaveResponse> {
    let wave_id = parse_id(&wave_id)?;
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

    wave.paused = false;
    if let Some(flow) = payload.flow {
        wave.flow = flow;
    }
    if let Some(direction) = payload.direction {
        if !direction.is_empty() {
            wave.direction = direction;
        }
    }
    if let Some(area) = payload.area {
        if !area.is_empty() {
            wave.area = area;
        }
    }

    let wave_clone = wave.clone();
    run_store(&state.store, move |store| store.update_wave(&wave_clone))
        .await
        .map_err(map_store_error)?;

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
            return Err(map_store_error(err));
        }
        Err(err) => {
            state.scheduler.release(run_id.as_str());
            return Err(api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            ));
        }
    };

    let exec = state.executor.clone();
    let store = state.store.clone();
    let scheduler = state.scheduler.clone();
    let run_id_for_release = run.id.clone();
    tokio::spawn(async move {
        if let Err(err) = exec.execute(&run_id_for_release).await {
            tracing::error!(run_id = %run_id_for_release, error = %err, "run execution failed");
            if let Ok(Some(mut run)) = store.get_wave_run(&run_id_for_release) {
                run.status = WaveRunStatus::Failed;
                run.error = Some(err.to_string());
                run.ended_at = Some(OffsetDateTime::now_utc());
                let _ = store.update_wave_run(&run);
            }
        }
        scheduler.release(run_id_for_release.as_str());
    });

    state
        .event_hub
        .send(Event::wave_started(wave.id.clone(), run.id.clone()));

    Ok(Json(RunWaveResponse {
        started: true,
        wave_id: wave.id.to_string(),
        wave_run_id: Some(run.id.to_string()),
    }))
}

pub async fn stop_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<StopWaveResponse> {
    let wave_id = parse_id(&wave_id)?;
    let run = run_store(&state.store, {
        let wave_id = wave_id.clone();
        move |store| store.get_active_wave_run(&wave_id)
    })
    .await
    .map_err(map_store_error)?;

    if let Some(mut run) = run {
        run.status = WaveRunStatus::Failed;
        run.error = Some("stopped".to_string());
        run.ended_at = Some(OffsetDateTime::now_utc());
        let run_clone = run.clone();
        run_store(&state.store, move |store| store.update_wave_run(&run_clone))
            .await
            .map_err(map_store_error)?;
    }

    state.event_hub.send(Event::wave_stopped(wave_id));

    Ok(Json(StopWaveResponse { stopped: true }))
}

pub async fn land_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    payload: Option<Json<LandWaveRequest>>,
) -> ApiResult<LandWaveResponse> {
    let wave_id = parse_id(&wave_id)?;
    let payload = payload.map(|Json(value)| value).unwrap_or_default();
    let wave = run_store(&state.store, move |store| store.get_wave(&wave_id))
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    let active_run = run_store(&state.store, move |store| {
        let wave_id = LfdId::from_raw(wave.id.clone());
        store.get_active_wave_run(&wave_id)
    })
    .await
    .map_err(map_store_error)?;

    let worktree = payload
        .worktree
        .or_else(|| active_run.as_ref().map(|run| run.worktree.clone()))
        .filter(|value| !value.is_empty());

    let strict = payload.strict.unwrap_or(false);
    let local = payload.local.unwrap_or(false);
    let create_pr = payload.create_pr.unwrap_or(true);
    let lint = payload.lint.unwrap_or(true);

    let repo_path = wave.repo.clone();
    tokio::task::spawn_blocking(move || {
        let progress = loopflow_ops::NullProgress;
        loopflow_ops::land(
            std::path::Path::new(&repo_path),
            &loopflow_ops::LandOptions {
                strict,
                local,
                create_pr,
                worktree,
                lint,
            },
            &progress,
        )
    })
    .await
    .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
    .map_err(|err| api_error(StatusCode::BAD_REQUEST, err.to_string()))?;

    Ok(Json(LandWaveResponse { merged: true }))
}

fn create_wave_run_with_id(
    store: &SharedStore,
    wave: &Wave,
    run_id: &LfdId,
) -> Result<WaveRun, StoreError> {
    let last_run = store
        .list_wave_runs(Some(&wave.id), Some(1))?
        .into_iter()
        .next();
    let iteration = last_run.map(|run| run.iteration + 1).unwrap_or(0);

    let run = WaveRun {
        id: run_id.clone(),
        wave_id: wave.id.clone(),
        iteration,
        step_index: 0,
        status: WaveRunStatus::Running,
        worktree: wave.repo.clone(),
        branch: String::new(),
        started_at: Some(OffsetDateTime::now_utc()),
        ended_at: None,
        error: None,
        flow_parents: Vec::new(),
    };
    store.create_wave_run(&run)?;
    Ok(run)
}
