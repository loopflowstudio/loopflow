use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use std::path::{Path as FsPath, PathBuf};
use std::str::FromStr;
use time::OffsetDateTime;
use tracing::warn;

use crate::engine::git::{branch_rename, current_branch, delete_remote_branch, push_with_upstream};
use crate::engine::naming::sanitize_for_branch;
use crate::engine::platform::kill_process;
use crate::engine::worktree::remove_worktree;
use crate::engine::worktrees::{branch_exists, worktree_path};
use crate::lfd::config::ExecutorType;
use crate::lfd::executor::{create_wave_run_with_id, ensure_wave_worktree};
use crate::lfd::http::dto::{
    stimulus_dto, stimulus_kind_str, CombineResponse, CombineResponseResult, ContinueWaveResponse,
    DeletedResourceResponse, ErrorResponse, LandWaveResponse, ListResponse, NextWaveResponse,
    RestartStepResponse, RunWaveResponse, StopWaveResponse, WaveDto,
};
use crate::lfd::http::routes::wave_config::{read_wave_config, StimulusDef};
use crate::lfd::http::routes::{build_wave_dto, hooks, resolve_wave_id, ApiError};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::triggers::spawn_run_task_with_slot;
use crate::lfd::types::{
    AgentStatus, Event, Stimulus, StimulusKind, Wave, WaveRun, WaveRunStatus, WaveStatus,
};

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

#[derive(Debug, Deserialize)]
pub struct CreateWaveRequest {
    repo: String,
    #[serde(alias = "schema")]
    name: Option<String>,
    flow: Option<String>,
    direction: Option<Vec<String>>,
    area: Option<Vec<String>>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateWaveRequest {
    name: Option<String>,
    flow: Option<String>,
    direction: Option<Vec<String>>,
    area: Option<Vec<String>>,
    status: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct RunWaveRequest {
    area: Option<Vec<String>>,
    direction: Option<Vec<String>>,
    flow: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddStimulusRequest {
    kind: StimulusKind,
    cron: Option<String>,
    source_wave_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
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

pub async fn create_wave_handler(
    State(state): State<HttpState>,
    Json(payload): Json<CreateWaveRequest>,
) -> ApiResult<WaveDto> {
    let CreateWaveRequest {
        repo,
        name: requested_name,
        flow,
        direction,
        area,
    } = payload;
    let repo_path = PathBuf::from(&repo);

    let id = LfdId::new();
    let name = requested_name.unwrap_or_else(|| format!("wave-{}", id));
    let wave_config = read_wave_config(&repo_path, &name);
    let config_stimulus = wave_config
        .as_ref()
        .and_then(|config| config.stimulus.as_ref())
        .map(parse_stimulus)
        .transpose()?;
    let flow = flow
        .or_else(|| wave_config.as_ref().map(|c| c.flow.clone()))
        .unwrap_or_else(|| "ship".to_string());
    let direction = direction
        .or_else(|| wave_config.as_ref().and_then(|c| c.direction.clone()))
        .unwrap_or_default();
    let area = area
        .or_else(|| wave_config.as_ref().map(|c| c.area.clone()))
        .unwrap_or_default();

    // Check for duplicate wave name in the same repo.
    let existing = wave_name_exists(&state, &repo, &name)
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
        repo,
        flow,
        direction,
        area,
        status: WaveStatus::Idle,
        iteration: 0,
        created_at: Some(OffsetDateTime::now_utc()),
    };
    state
        .store
        .create_wave(&wave)
        .await
        .map_err(map_store_error)?;

    if let Err(err) = ensure_local_wave_worktree(&state, &wave).await {
        let wave_id = wave.id().clone();
        let _ = state.store.delete_wave(&wave_id).await;
        return Err(err);
    }

    if let Some((kind, cron)) = config_stimulus {
        let stimulus = Stimulus {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            source_wave_id: None,
            kind,
            cron,
            last_main_sha: None,
            last_triggered_at: None,
            created_at: Some(OffsetDateTime::now_utc()),
            enabled: true,
        };
        if let Err(err) = state.store.create_stimulus(&stimulus).await {
            let wave_id = wave.id().clone();
            let _ = state.store.delete_wave(&wave_id).await;
            return Err(map_store_error(err));
        }
    }

    state
        .event_hub
        .send(Event::wave_created(wave.id().clone(), wave.name().clone()));

    let view = build_wave_dto(&state.store, &state.github, wave, false)
        .await
        .map_err(map_store_error)?;
    Ok(Json(view))
}

async fn ensure_local_wave_worktree(
    state: &HttpState,
    wave: &Wave,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if state.executor.executor_type() != ExecutorType::Local {
        return Ok(());
    }

    let repo_for_wt = wave.repo().clone();
    let name_for_wt = wave.name().clone();
    run_blocking_result(
        move || {
            ensure_wave_worktree(std::path::Path::new(&repo_for_wt), &name_for_wt)
                .map(|_| ())
                .map_err(|err| format!("failed to create worktree: {err}"))
        },
        StatusCode::INTERNAL_SERVER_ERROR,
    )
    .await
}

fn parse_stimulus(
    stimulus: &StimulusDef,
) -> Result<(StimulusKind, String), (StatusCode, Json<ErrorResponse>)> {
    let kind = match stimulus.kind.as_str() {
        "cron" => StimulusKind::Cron,
        "watch" => StimulusKind::Watch,
        "loop" => StimulusKind::Loop,
        "once" => StimulusKind::Once,
        value => {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                format!("invalid wave config stimulus kind '{value}'"),
            ));
        }
    };

    let cron = match kind {
        StimulusKind::Cron => stimulus
            .cron
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .ok_or_else(|| {
                api_error(
                    StatusCode::BAD_REQUEST,
                    "wave config stimulus kind 'cron' requires a cron expression",
                )
            })?,
        _ => String::new(),
    };

    Ok((kind, cron))
}

async fn wave_name_exists(
    state: &HttpState,
    repo: &str,
    name: &str,
) -> Result<bool, crate::lfd::store::StoreError> {
    let waves = state.store.list_waves(Some(repo)).await?;
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

            // Check for duplicate name in same repo.
            let existing = wave_name_exists(&state, wave.repo(), &new_name)
                .await
                .map_err(map_store_error)?;
            if existing {
                return Err(api_error(
                    StatusCode::CONFLICT,
                    format!("wave '{}' already exists in this repo", new_name),
                ));
            }

            // Reject rename while wave is running or waiting.
            let active_run = state
                .store
                .get_active_wave_run(wave.id())
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
            let old_name = wave.name().clone();
            let repo = wave.repo().clone();
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

    state
        .store
        .update_wave(&wave)
        .await
        .map_err(map_store_error)?;

    state.event_hub.send(Event::wave_updated(wave.id().clone()));

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

    terminate_active_agents(&state, &wave_id).await?;

    // Delete from the store (cascades to wave_runs, agents, etc.).
    state
        .store
        .delete_wave(&wave_id)
        .await
        .map_err(map_store_error)?;

    if state.executor.executor_type() == ExecutorType::Docker {
        if let Err(err) = state.executor.cleanup_wave_workspace(&wave).await {
            warn!(
                wave_id = %wave.id(),
                error = %err,
                "failed to remove docker-backed wave workspace"
            );
        }
    }

    // Clean up the worktree on disk.
    let repo = wave.repo().clone();
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
    let mut wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    let active_run = state
        .store
        .get_active_wave_run(&wave_id)
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

    state
        .store
        .update_wave(&wave)
        .await
        .map_err(map_store_error)?;

    // Re-enable all stimuli (they may have been disabled by a previous stop).
    let _ = set_wave_stimuli_enabled(&state, wave.id(), true, false).await;

    let run_id = LfdId::new();
    let slot_guard = match state.scheduler.acquire_guard(run_id.as_str()).await {
        Ok(guard) => guard,
        Err(_) => {
            return Ok(Json(RunWaveResponse {
                started: false,
                wave_id: wave.id().to_string(),
                wave_run_id: None,
            }))
        }
    };

    let run = match create_wave_run_with_id(&state.store, &wave, &run_id).await {
        Ok(run) => run,
        Err(err) => {
            return Err(api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                err.to_string(),
            ))
        }
    };

    spawn_run_task_with_slot(
        state.store.clone(),
        (*state.executor).clone(),
        state.event_hub.clone(),
        run.clone(),
        slot_guard,
    );

    Ok(Json(RunWaveResponse {
        started: true,
        wave_id: wave.id().to_string(),
        wave_run_id: Some(run.id.to_string()),
    }))
}

pub async fn check_wave_ci_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let token = state
        .github
        .token
        .clone()
        .ok_or_else(|| api_error(StatusCode::BAD_REQUEST, "github token is not configured"))?;

    let emitted = hooks::poll_wave_ci(
        &state.store,
        &state.event_hub,
        &state.ci_failure_cache,
        &wave_id,
        &token,
    )
    .await
    .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err))?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "wave_id": wave_id.to_string(),
        "emitted": emitted
    })))
}

// Stimulus CRUD handlers

pub async fn add_stimulus_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Json(payload): Json<AddStimulusRequest>,
) -> ApiResult<serde_json::Value> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    // Verify wave exists.
    state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    let source_wave_id = match payload.kind {
        StimulusKind::Listen => {
            let source = payload.source_wave_id.as_deref().ok_or_else(|| {
                api_error(
                    StatusCode::BAD_REQUEST,
                    "listen stimulus requires source_wave_id",
                )
            })?;
            let resolved = resolve_wave_id(&state, source).await?;
            if resolved == wave_id {
                return Err(api_error(
                    StatusCode::BAD_REQUEST,
                    "listen stimulus cannot target the same wave",
                ));
            }
            Some(resolved)
        }
        _ => {
            if payload.source_wave_id.is_some() {
                return Err(api_error(
                    StatusCode::BAD_REQUEST,
                    "source_wave_id is only valid for listen stimulus",
                ));
            }
            None
        }
    };

    let stimulus = Stimulus {
        id: LfdId::new(),
        wave_id,
        source_wave_id,
        kind: payload.kind,
        cron: payload.cron.unwrap_or_default(),
        last_main_sha: None,
        last_triggered_at: None,
        created_at: Some(OffsetDateTime::now_utc()),
        enabled: true,
    };

    state
        .store
        .create_stimulus(&stimulus)
        .await
        .map_err(map_store_error)?;

    Ok(Json(serde_json::json!({
        "id": stimulus.id.to_string(),
        "kind": stimulus_kind_str(stimulus.kind),
        "source_wave_id": stimulus.source_wave_id.as_ref().map(ToString::to_string),
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

    state
        .store
        .delete_stimulus(&stimulus_id)
        .await
        .map_err(map_store_error)?;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub async fn list_stimuli_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    let stimuli = state
        .store
        .list_stimuli(Some(&wave_id))
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

    let run = state
        .store
        .get_active_wave_run(&wave_id)
        .await
        .map_err(map_store_error)?;

    // Disable all auto stimuli so tickers won't restart the wave.
    let has_auto_stimulus = set_wave_stimuli_enabled(&state, &wave_id, false, true).await;

    if let Some(mut run) = run {
        run.status = WaveRunStatus::Failed;
        run.error = Some("stopped".to_string());
        run.ended_at = Some(OffsetDateTime::now_utc());
        state
            .store
            .update_wave_run(&run)
            .await
            .map_err(map_store_error)?;
        let wave_id_for_update = run.wave_id.clone();
        if let Some(mut wave) = state
            .store
            .get_wave(&wave_id_for_update)
            .await
            .map_err(map_store_error)?
        {
            wave.status = if has_auto_stimulus {
                WaveStatus::Paused
            } else {
                WaveStatus::Failed
            };
            state
                .store
                .update_wave(&wave)
                .await
                .map_err(map_store_error)?;
        }
        mark_active_agents_failed(&state, &wave_id).await;
    } else if has_auto_stimulus {
        // No active run, but still pause the wave so the auto stimulus doesn't restart it.
        if let Some(mut wave) = state
            .store
            .get_wave(&wave_id)
            .await
            .map_err(map_store_error)?
        {
            wave.status = WaveStatus::Paused;
            state
                .store
                .update_wave(&wave)
                .await
                .map_err(map_store_error)?;
        }
    }

    state.event_hub.send(Event::wave_stopped(wave_id));

    Ok(Json(StopWaveResponse { stopped: true }))
}

pub async fn restart_step_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<RestartStepResponse> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    let run = state
        .store
        .get_active_wave_run(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "no active run for wave"))?;

    if run.status != WaveRunStatus::Running {
        return Err(api_error(
            StatusCode::PRECONDITION_FAILED,
            "wave run is not running",
        ));
    }

    // Kill running agents.
    terminate_active_agents(&state, &wave_id).await?;
    mark_active_agents_failed(&state, &wave_id).await;

    // Re-acquire scheduler slot and relaunch.
    let wave_run_id = run.id.to_string();
    let step_index = run.step_index;
    respawn_run_task(&state, run).await?;

    state.event_hub.send(Event::wave_updated(wave_id.clone()));

    Ok(Json(RestartStepResponse {
        restarted: true,
        wave_id: wave_id.to_string(),
        wave_run_id,
        step_index,
    }))
}

async fn terminate_active_agents(
    state: &HttpState,
    wave_id: &LfdId,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let agents = state
        .store
        .get_active_agents_for_wave(wave_id)
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

async fn mark_active_agents_failed(state: &HttpState, wave_id: &LfdId) {
    let _ = state
        .store
        .end_active_agent_for_wave(
            wave_id,
            AgentStatus::Failed.as_i32(),
            OffsetDateTime::now_utc().unix_timestamp(),
        )
        .await;
}

async fn respawn_run_task(
    state: &HttpState,
    run: WaveRun,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let slot_guard = state
        .scheduler
        .acquire_guard(run.id.as_str())
        .await
        .map_err(|_| {
            api_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "no scheduler slots available",
            )
        })?;

    spawn_run_task_with_slot(
        state.store.clone(),
        (*state.executor).clone(),
        state.event_hub.clone(),
        run,
        slot_guard,
    );

    Ok(())
}

pub async fn continue_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<ContinueWaveResponse> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let mut run = state
        .store
        .get_active_wave_run(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "no active run for wave"))?;

    if run.status != WaveRunStatus::Waiting {
        return Err(api_error(
            StatusCode::PRECONDITION_FAILED,
            "wave run is not waiting for interactive input",
        ));
    }

    let mut wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    // Resolve worktree and check for uncommitted changes.
    let worktree = run.worktree.clone();

    // Resolve the current step name for the commit message.
    let step_name = resolve_current_step_name(&run, run.step_index);
    run_blocking_result(
        move || auto_commit_if_dirty(std::path::Path::new(&worktree), &step_name),
        StatusCode::INTERNAL_SERVER_ERROR,
    )
    .await?;

    // Advance to the next step.
    run.step_index += 1;
    run.status = WaveRunStatus::Running;
    state
        .store
        .update_wave_run(&run)
        .await
        .map_err(map_store_error)?;
    wave.status = WaveStatus::Running;
    state
        .store
        .update_wave(&wave)
        .await
        .map_err(map_store_error)?;

    state.event_hub.send(Event::wave_updated(wave_id.clone()));

    // Re-acquire scheduler slot (idempotent for same run_id).
    let wave_run_id = run.id.to_string();
    respawn_run_task(&state, run).await?;

    Ok(Json(ContinueWaveResponse {
        continued: true,
        wave_id: wave_id.to_string(),
        wave_run_id,
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
    let wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    let latest_run = state
        .store
        .get_latest_wave_run(wave.id())
        .await
        .map_err(map_store_error)?;

    let latest_worktree = payload
        .worktree
        .or_else(|| latest_run.as_ref().map(|run| run.worktree.clone()))
        .filter(|value| !value.is_empty());
    let worktree =
        resolve_wave_work_dir_for_api(wave.repo().clone(), wave.name().clone(), latest_worktree)
            .await?;

    let strict = payload.strict.unwrap_or(false);
    let local = payload.local.unwrap_or(false);
    let create_pr = payload.create_pr.unwrap_or(true);
    let lint = payload.lint.unwrap_or(true);

    let repo_path = wave.repo().clone();
    run_blocking_result(
        move || {
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
        },
        StatusCode::BAD_REQUEST,
    )
    .await?;

    state.event_hub.send(Event::wave_updated(wave_id_for_event));

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
    let wave = state
        .store
        .get_wave(wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    let latest_run = state
        .store
        .get_latest_wave_run(wave_id)
        .await
        .map_err(map_store_error)?;

    let latest_worktree = latest_run
        .map(|run| run.worktree)
        .filter(|value| !value.is_empty());
    let work_dir =
        resolve_wave_work_dir_for_api(wave.repo().clone(), wave.name().clone(), latest_worktree)
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
    api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
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
        .map_err(|err| api_error(failure_status, err.to_string()))
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
        StimulusKind::Loop | StimulusKind::Watch | StimulusKind::Cron | StimulusKind::Listen
    )
}

async fn set_wave_stimuli_enabled(
    state: &HttpState,
    wave_id: &LfdId,
    enabled: bool,
    auto_only: bool,
) -> bool {
    let stimuli = match state.store.list_stimuli(Some(wave_id)).await {
        Ok(stimuli) => stimuli,
        Err(_) => return false,
    };
    let mut matched = false;
    for mut stimulus in stimuli {
        if auto_only && !is_auto_stimulus(stimulus.kind) {
            continue;
        }
        matched = true;
        if stimulus.enabled != enabled {
            stimulus.enabled = enabled;
            if state.store.update_stimulus(&stimulus).await.is_err() {
                return false;
            }
        }
    }
    matched
}

fn resolve_current_step_name(run: &WaveRun, step_index: u32) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_stimulus_requires_cron_expression_for_cron_kind() {
        let stimulus = StimulusDef {
            kind: "cron".to_string(),
            cron: None,
        };

        let result = parse_stimulus(&stimulus);
        assert!(result.is_err());
    }

    #[test]
    fn parse_stimulus_accepts_valid_cron_expression() {
        let stimulus = StimulusDef {
            kind: "cron".to_string(),
            cron: Some("0 8 * * *".to_string()),
        };

        let parsed = parse_stimulus(&stimulus).expect("parse stimulus");
        assert_eq!(parsed.0, StimulusKind::Cron);
        assert_eq!(parsed.1, "0 8 * * *");
    }

    #[test]
    fn listen_stimulus_is_auto_stimulus() {
        assert!(is_auto_stimulus(StimulusKind::Listen));
    }
}
