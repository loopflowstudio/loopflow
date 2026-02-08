use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use std::str::FromStr;
use time::OffsetDateTime;

use crate::lfd::http::dto::{
    ContinueWaveResponse, DeletedResourceResponse, LandWaveResponse, ListResponse, RunWaveResponse,
    StopWaveResponse, WaveDto,
};
use crate::lfd::http::routes::{build_wave_dto, resolve_wave_id};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, run_store, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::loops::common::create_wave_run_with_id;
use crate::lfd::types::{Event, Wave, WaveRun, WaveRunStatus, WaveStatus};

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
    let (waves, has_more) = paginate_waves(
        waves,
        query.limit,
        query.starting_after.as_deref(),
        query.ending_before.as_deref(),
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

    if let Some(name) = payload.name {
        if !name.is_empty() {
            wave.name = name;
        }
    }
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
    let wave_id_for_delete = wave_id.clone();
    run_store(&state.store, move |store| {
        store.delete_wave(&wave_id_for_delete)
    })
    .await
    .map_err(map_store_error)?;

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
            return Err(api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()));
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
                if let Ok(Some(mut wave)) = store.get_wave(&run.wave_id) {
                    wave.status = WaveStatus::Failed;
                    let _ = store.update_wave(&wave);
                }
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
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
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
        let wave_id = run.wave_id.clone();
        run_store(&state.store, move |store| {
            if let Some(mut wave) = store.get_wave(&wave_id)? {
                wave.status = WaveStatus::Failed;
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
    let worktree = if run.worktree.is_empty() {
        run.snapshot.repo.clone()
    } else {
        run.worktree.clone()
    };

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

    // Re-acquire scheduler slot (idempotent for same run_id).
    let (acquired, _) = state.scheduler.acquire(run.id.as_str()).await;
    if !acquired {
        return Err(api_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "no scheduler slots available",
        ));
    }

    // Spawn executor to continue the flow.
    let exec = state.executor.clone();
    let store = state.store.clone();
    let scheduler = state.scheduler.clone();
    let run_id = run.id.clone();
    tokio::spawn(async move {
        if let Err(err) = exec.execute(&run_id).await {
            tracing::error!(run_id = %run_id, error = %err, "run execution failed");
            if let Ok(Some(mut run)) = store.get_wave_run(&run_id) {
                run.status = WaveRunStatus::Failed;
                run.error = Some(err.to_string());
                run.ended_at = Some(OffsetDateTime::now_utc());
                let _ = store.update_wave_run(&run);
            }
        }
        scheduler.release(run_id.as_str());
    });

    state
        .event_hub
        .send(Event::wave_started(wave_id.clone(), run.id.clone()));

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
        let progress = crate::ops::NullProgress;
        crate::ops::land(
            std::path::Path::new(&repo_path),
            &crate::ops::LandOptions {
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

fn paginate_waves(
    waves: Vec<Wave>,
    limit: Option<u32>,
    starting_after: Option<&str>,
    ending_before: Option<&str>,
) -> (Vec<Wave>, bool) {
    let mut items = waves;
    if let Some(starting_after) = starting_after {
        if let Some(pos) = items
            .iter()
            .position(|wave| wave.id.to_string() == starting_after)
        {
            items = items.split_off(pos + 1);
        }
    }
    if let Some(ending_before) = ending_before {
        if let Some(pos) = items
            .iter()
            .position(|wave| wave.id.to_string() == ending_before)
        {
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
