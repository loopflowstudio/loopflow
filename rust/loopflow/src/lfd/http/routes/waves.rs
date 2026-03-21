use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use secrecy::ExposeSecret;
use serde::Deserialize;
use std::path::{Path as FsPath, PathBuf};
use std::str::FromStr;
use time::OffsetDateTime;
use tokio::process::Command as TokioCommand;
use tracing::warn;

use crate::engine::git::{branch_rename, current_branch, delete_remote_branch, push_with_upstream};
use crate::engine::naming::sanitize_for_branch;
use crate::engine::platform::kill_process;
use crate::engine::worktree::remove_worktree;
use crate::engine::worktrees::{branch_exists, worktree_path};
use crate::lfd::executor::ensure_wave_worktree;
use crate::lfd::http::dto::{
    activation_log_dto, trigger_dto, ActivationLogDto, CombineResponse, CombineResponseResult,
    DeletedResourceResponse, ErrorResponse, LandWaveResponse, ListResponse, NextWaveResponse,
    RestartStepResponse, RunWaveResponse, StopWaveResponse, WaveDto,
};
use crate::lfd::http::routes::wave_config::{
    read_wave_config, update_wave_agent_config, TriggerDef, WaveConfig,
};
use crate::lfd::http::routes::{build_wave_dto, hooks, resolve_wave_id, ApiError};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiMessage, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::triggers::{
    spawn_immediate_activation, spawn_run_task_with_slot, ActivationEnvelope, ImmediateActivation,
};
use crate::lfd::types::{
    AgentStatus, Event, Signal, Trigger, Wave, WaveMode, WaveRun, WaveRunStatus, WaveStatus,
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

#[derive(Debug, Deserialize, Default)]
pub struct ListActivationsQuery {
    limit: Option<u32>,
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
    name: Option<String>,
    flow: Option<String>,
    direction: Option<Vec<String>>,
    area: Option<Vec<String>>,
    workers: Option<u32>,
    status: Option<String>,
    #[serde(default)]
    run: bool,
    #[serde(default)]
    serialized: bool,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateWaveRequest {
    name: Option<String>,
    flow: Option<String>,
    direction: Option<Vec<String>>,
    area: Option<Vec<String>>,
    workers: Option<u32>,
    status: Option<String>,
    agent: Option<String>,
    step_agents: Option<std::collections::HashMap<String, String>>,
    serialized: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct RunWaveRequest {
    area: Option<Vec<String>>,
    direction: Option<Vec<String>>,
    flow: Option<String>,
    roadmap_item: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddTriggerRequest {
    signal: Signal,
    flow: Option<String>,
    source_wave_id: Option<String>,
    max_iterations: Option<u32>,
}

#[derive(Debug, Clone)]
struct ParsedTrigger {
    signal: Signal,
    flow: Option<String>,
    source: Option<String>,
    source_repo: Option<String>,
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
        workers: requested_workers,
        status,
        run,
        serialized,
    } = payload;
    let repo_path = PathBuf::from(&repo);

    let id = LfdId::new();
    let name = requested_name.unwrap_or_else(|| format!("wave-{}", id));
    let wave_config = read_wave_config(&repo_path, &name);
    let config_trigger = wave_config
        .as_ref()
        .and_then(|config| config.triggers.as_ref())
        .map(parse_trigger)
        .transpose()?;
    let direction = direction
        .or_else(|| wave_config.as_ref().and_then(|c| c.direction.clone()))
        .unwrap_or_default();
    let area = area
        .or_else(|| wave_config.as_ref().and_then(|c| c.area.clone()))
        .unwrap_or_default();

    // Check for duplicate wave name in the same repo.
    let existing = wave_name_exists(&state, &repo, &name)
        .await
        .map_err(map_store_error)?;
    if existing {
        return Err(wave_name_exists_error(&name));
    }
    let wave_source_wave_id =
        resolve_wave_source_wave_id(&state.store, &repo, &name, config_trigger.as_ref()).await?;

    let mode = wave_config
        .as_ref()
        .and_then(|c| c.mode.as_deref())
        .and_then(|m| m.parse::<WaveMode>().ok())
        .unwrap_or_default();
    let primary_flow = flow
        .or_else(|| wave_config.as_ref().and_then(|c| c.primary_flow.clone()))
        .or_else(|| wave_config.as_ref().and_then(|c| c.flow.clone()))
        .unwrap_or_else(|| "ship-roadmap".to_string());
    let cron_field = wave_config.as_ref().and_then(|c| c.cron.clone());
    let workers = create_wave_workers(requested_workers, serialized, wave_config.as_ref());

    let mut wave = Wave {
        id: id.clone(),
        name,
        repo,
        mode,
        primary_flow,
        cron: cron_field,
        direction,
        area,
        status: WaveStatus::Idle,
        iteration: 0,
        cycle_start_iteration: 0,
        created_at: Some(OffsetDateTime::now_utc()),
        workers,
    };
    if let Some(status) = status {
        wave.status = WaveStatus::from_str(&status)
            .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid status"))?;
    }
    state
        .store
        .create_wave(&wave)
        .await
        .map_err(map_store_error)?;

    if let Err(err) = ensure_wave_workspace(&state, &wave).await {
        let wave_id = wave.id().clone();
        let _ = state.store.delete_wave(&wave_id).await;
        return Err(err);
    }

    // Collect triggers: config-provided first, then defaults.
    let mut triggers: Vec<Trigger> = Vec::new();

    if let Some(parsed) = config_trigger {
        let mut s = Trigger::new(LfdId::new(), wave.id().clone(), parsed.signal);
        s.source_wave_id = wave_source_wave_id.clone();
        s.flow = parsed.flow;
        triggers.push(s);
    }

    // Every wave gets repo (rebase on main) and ci-fix unless config already covers them.
    for (signal, flow) in [(Signal::Repo, "integrate"), (Signal::CiFailure, "ci-fix")] {
        if triggers.iter().any(|s| s.signal == signal) {
            continue;
        }
        let mut s = Trigger::new(LfdId::new(), wave.id().clone(), signal);
        s.flow = Some(flow.to_string());
        triggers.push(s);
    }

    for trigger in triggers {
        if let Err(err) = state.store.create_trigger(&trigger).await {
            let wave_id = wave.id().clone();
            let _ = state.store.delete_wave(&wave_id).await;
            return Err(map_store_error(err));
        }
    }

    state
        .event_hub
        .send(Event::wave_created(wave.id().clone(), wave.name().clone()));

    if run {
        start_wave_run(&state, &mut wave, None).await?;
    }

    let response_wave = if run {
        wait_for_wave_start_settle(&state, wave).await
    } else {
        wave
    };

    let view = build_wave_dto(&state.store, &state.github, response_wave, run)
        .await
        .map_err(map_store_error)?;
    Ok(Json(view))
}

async fn wait_for_wave_start_settle(state: &HttpState, mut wave: Wave) -> Wave {
    // Give the executor a moment to settle (e.g., hit WaitInteractive)
    // so the response reflects the actual state instead of transient Running.
    let wave_id = wave.id().clone();
    for _ in 0..10 {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        let Ok(Some(updated)) = state.store.get_wave(&wave_id).await else {
            continue;
        };
        if updated.status != WaveStatus::Running {
            return updated;
        }
        wave = updated;
    }
    wave
}

async fn ensure_wave_workspace(
    state: &HttpState,
    wave: &Wave,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    state
        .executor
        .ensure_wave_workspace(wave)
        .await
        .map_err(|err| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiMessage::Untrusted(format!("failed to create worktree: {err}")),
            )
        })
}

fn parse_trigger(trigger: &TriggerDef) -> Result<ParsedTrigger, (StatusCode, Json<ErrorResponse>)> {
    let signal = match trigger.signal.as_str() {
        "repo" => Signal::Repo,
        "wave" => Signal::Wave,
        "ci_failure" => Signal::CiFailure,
        value => {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                ApiMessage::Safe(format!("invalid wave config trigger signal '{value}'")),
            ));
        }
    };

    let flow = trimmed_non_empty(trigger.flow.as_deref());
    let source = trimmed_non_empty(trigger.source.as_deref());
    let source_repo = trimmed_non_empty(trigger.source_repo.as_deref());

    if signal != Signal::Wave {
        if source.is_some() {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "wave config source is only valid for wave trigger",
            ));
        }
        if source_repo.is_some() {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                "wave config source_repo is only valid for wave trigger",
            ));
        }
    }

    let source = if signal == Signal::Wave {
        Some(source.ok_or_else(|| {
            api_error(
                StatusCode::BAD_REQUEST,
                "wave config wave trigger requires source",
            )
        })?)
    } else {
        None
    };
    let source_repo = if signal == Signal::Wave {
        source_repo
    } else {
        None
    };

    Ok(ParsedTrigger {
        signal,
        flow,
        source,
        source_repo,
    })
}

fn trimmed_non_empty(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

fn create_wave_workers(
    requested_workers: Option<u32>,
    serialized: bool,
    wave_config: Option<&WaveConfig>,
) -> u32 {
    requested_workers
        .or_else(|| wave_config.and_then(|config| config.workers))
        .or_else(|| serialized.then_some(1))
        .or_else(|| wave_config.and_then(|config| config.serialized).map(|_| 1))
        .unwrap_or(1)
        .max(1)
}

fn update_wave_workers(
    current_workers: u32,
    requested_workers: Option<u32>,
    serialized: Option<bool>,
) -> u32 {
    requested_workers
        .map(|workers| workers.max(1))
        .or_else(|| serialized.map(|_| 1))
        .unwrap_or(current_workers)
}

async fn resolve_wave_source_wave_id(
    store: &crate::lfd::store::SharedStore,
    repo: &str,
    wave_name: &str,
    parsed: Option<&ParsedTrigger>,
) -> Result<Option<LfdId>, (StatusCode, Json<ErrorResponse>)> {
    let Some(parsed) = parsed else {
        return Ok(None);
    };
    if parsed.signal != Signal::Wave {
        return Ok(None);
    }
    let source = parsed
        .source
        .as_deref()
        .expect("wave trigger parsing requires source");
    let source_repo = parsed.source_repo.as_deref().unwrap_or(repo);
    if source_repo == repo && source == wave_name {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "wave trigger cannot target the same wave",
        ));
    }
    resolve_wave_id_in_repo(store, source_repo, source)
        .await
        .map(Some)
}

async fn resolve_wave_id_in_repo(
    store: &crate::lfd::store::SharedStore,
    repo: &str,
    name: &str,
) -> Result<LfdId, (StatusCode, Json<ErrorResponse>)> {
    if let Ok(id) = name.parse::<LfdId>() {
        let wave = store
            .get_wave(&id)
            .await
            .map_err(map_store_error)?
            .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;
        if wave.repo() != repo {
            return Err(api_error(StatusCode::NOT_FOUND, "wave not found"));
        }
        return Ok(id);
    }

    let waves = store
        .list_waves(Some(repo))
        .await
        .map_err(map_store_error)?;
    waves
        .into_iter()
        .find(|wave| wave.name() == name)
        .map(|wave| wave.id().clone())
        .ok_or_else(|| {
            api_error(
                StatusCode::NOT_FOUND,
                ApiMessage::Safe(format!("source wave '{name}' not found in repo '{repo}'")),
            )
        })
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
                return Err(wave_name_exists_error(&new_name));
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
        wave.primary_flow = flow;
    }
    if let Some(direction) = payload.direction {
        wave.direction = direction;
    }
    if let Some(area) = payload.area {
        wave.area = area;
    }
    wave.workers = update_wave_workers(wave.workers, payload.workers, payload.serialized);
    if let Some(status) = payload.status {
        wave.status = WaveStatus::from_str(&status)
            .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid status"))?;
    }

    if payload.agent.is_some() || payload.step_agents.is_some() {
        let repo = wave.repo().clone();
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

    crate::lfd::queue::remove_reconcile_lock(&wave_id).await;

    if let Err(err) = state.executor.cleanup_wave_workspace(&wave).await {
        warn!(
            wave_id = %wave.id(),
            error = %err,
            "failed to remove executor wave workspace"
        );
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
    .map_err(|err| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiMessage::Untrusted(err.to_string()),
        )
    })?;

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
    let mut wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;
    let payload = payload.map(|Json(value)| value);
    let run = start_wave_run(&state, &mut wave, payload).await?;

    Ok(Json(RunWaveResponse {
        started: run.is_some(),
        wave_id: wave.id().to_string(),
        wave_run_id: run.as_ref().map(|value| value.id.to_string()),
    }))
}

async fn start_wave_run(
    state: &HttpState,
    wave: &mut Wave,
    overrides: Option<RunWaveRequest>,
) -> Result<Option<WaveRun>, ApiError> {
    let active_runs = state
        .store
        .count_active_wave_runs(wave.id())
        .await
        .map_err(map_store_error)?;
    if active_runs >= wave.workers() {
        return Err(api_error(
            StatusCode::PRECONDITION_FAILED,
            "wave already at worker capacity",
        ));
    }

    let mut flow_override = None;
    let mut roadmap_item = None;
    if let Some(overrides) = overrides {
        flow_override = overrides.flow;
        roadmap_item = overrides.roadmap_item;
        if let Some(direction) = overrides.direction {
            wave.direction = direction;
        }
        if let Some(area) = overrides.area {
            wave.area = area;
        }
    }

    if wave.status == WaveStatus::Paused {
        wave.status = WaveStatus::Idle;
    }

    state
        .store
        .update_wave(wave)
        .await
        .map_err(map_store_error)?;

    // Re-enable all triggers (they may have been disabled by a previous stop).
    let _ = set_wave_triggers_enabled(state, wave.id(), true).await;

    let envelope = ActivationEnvelope::new(
        wave.id(),
        None,
        "manual run requested via API",
        "",
        "",
        "main",
    );

    spawn_immediate_activation(
        &state.store,
        &state.executor,
        &state.scheduler,
        &state.event_hub,
        ImmediateActivation {
            wave,
            flow_override,
            roadmap_item,
            envelope,
        },
    )
    .await
    .map_err(|err| {
        let message = err.to_string();
        let status = if message.contains("roadmap item not found")
            || message.contains("wave directory not found")
            || message.contains("no roadmap items")
        {
            StatusCode::BAD_REQUEST
        } else {
            StatusCode::INTERNAL_SERVER_ERROR
        };
        api_error(status, ApiMessage::Untrusted(message))
    })
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
        token.expose_secret(),
    )
    .await
    .map_err(|err| {
        api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiMessage::Untrusted(err),
        )
    })?;

    Ok(Json(serde_json::json!({
        "ok": true,
        "wave_id": wave_id.to_string(),
        "emitted": emitted
    })))
}

// Trigger CRUD handlers

pub async fn add_trigger_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Json(payload): Json<AddTriggerRequest>,
) -> ApiResult<serde_json::Value> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    // Verify wave exists.
    state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    let source_wave_id = match payload.signal {
        Signal::Wave => {
            let source = payload.source_wave_id.as_deref().ok_or_else(|| {
                api_error(
                    StatusCode::BAD_REQUEST,
                    "wave trigger requires source_wave_id",
                )
            })?;
            let resolved = resolve_wave_id(&state, source).await?;
            if resolved == wave_id {
                return Err(api_error(
                    StatusCode::BAD_REQUEST,
                    "wave trigger cannot target the same wave",
                ));
            }
            Some(resolved)
        }
        _ => {
            if payload.source_wave_id.is_some() {
                return Err(api_error(
                    StatusCode::BAD_REQUEST,
                    "source_wave_id is only valid for wave trigger",
                ));
            }
            None
        }
    };

    let trigger = Trigger {
        id: LfdId::new(),
        wave_id,
        source_wave_id,
        signal: payload.signal,
        flow: payload.flow,
        last_main_sha: None,
        last_triggered_at: None,
        created_at: Some(OffsetDateTime::now_utc()),
        enabled: true,
        max_iterations: payload.max_iterations,
    };

    state
        .store
        .create_trigger(&trigger)
        .await
        .map_err(map_store_error)?;

    Ok(Json(serde_json::json!({
        "id": trigger.id.to_string(),
        "signal": trigger.signal.as_str(),
        "flow": trigger.flow,
        "source_wave_id": trigger.source_wave_id.as_ref().map(ToString::to_string),
    })))
}

pub async fn remove_trigger_handler(
    State(state): State<HttpState>,
    Path((wave_id, trigger_id)): Path<(String, String)>,
) -> ApiResult<serde_json::Value> {
    let _wave_id = resolve_wave_id(&state, &wave_id).await?;
    let trigger_id = LfdId::from_str(&trigger_id)
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid trigger id"))?;

    state
        .store
        .delete_trigger(&trigger_id)
        .await
        .map_err(map_store_error)?;

    Ok(Json(serde_json::json!({ "deleted": true })))
}

pub async fn list_triggers_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    let triggers = state
        .store
        .list_triggers(Some(&wave_id))
        .await
        .map_err(map_store_error)?;

    let dtos: Vec<_> = triggers.into_iter().map(trigger_dto).collect();

    Ok(Json(serde_json::json!({ "data": dtos })))
}

pub async fn list_activations_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Query(query): Query<ListActivationsQuery>,
) -> ApiResult<ListResponse<ActivationLogDto>> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let limit = query.limit.unwrap_or(50).min(200);
    let logs = state
        .store
        .list_activation_log(&wave_id, limit)
        .await
        .map_err(map_store_error)?;
    let data = logs.into_iter().map(activation_log_dto).collect();
    Ok(Json(ListResponse::new(data, false)))
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

    // Disable all auto triggers so tickers won't restart the wave.
    let has_auto_trigger = set_wave_triggers_enabled(&state, &wave_id, false).await;

    if let Some(mut run) = run {
        run.status = WaveRunStatus::Failed;
        run.error = Some("stopped".to_string());
        run.ended_at = Some(OffsetDateTime::now_utc());
        state
            .store
            .update_wave_run(&run)
            .await
            .map_err(map_store_error)?;
        cancel_active_terminal_session(&state, &run).await?;
        let wave_id_for_update = run.wave_id.clone();
        if let Some(mut wave) = state
            .store
            .get_wave(&wave_id_for_update)
            .await
            .map_err(map_store_error)?
        {
            wave.status = if has_auto_trigger {
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
    } else if has_auto_trigger {
        // No active run, but still pause the wave so the auto trigger doesn't restart it.
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

async fn cancel_active_terminal_session(state: &HttpState, run: &WaveRun) -> Result<(), ApiError> {
    let Some(mut session) = state
        .store
        .get_active_terminal_session_for_wave_run(&run.id)
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
        .update_terminal_session(&session)
        .await
        .map_err(map_store_error)?;
    state
        .event_hub
        .send(Event::terminal_session_updated(session.clone()));

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

    let repo_path = wave.repo().clone();
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
                },
                &progress,
            )
        },
        StatusCode::BAD_REQUEST,
    )
    .await?;

    // Sync PR info back to the run so downstream code (CI webhooks) can find it.
    if let (Some(mut run), Some(pr_info)) = (latest_run, land_result.pr) {
        run.pr = Some(crate::lfd::types::PullRequest {
            url: pr_info.url,
            number: Some(pr_info.number as u32),
            state: Some(pr_info.state),
            title: None,
            branch: Some(pr_info.branch),
        });
        if let Err(err) = state.store.update_wave_run(&run).await {
            tracing::warn!(run_id = %run.id, error = %err, "failed to sync PR to run after land");
        }
    }

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

    let repo = wave.repo().clone();
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

    let (worktree, _) =
        ensure_wave_worktree(FsPath::new(repo_path), wave_name).map_err(|err| err.to_string())?;
    Ok(worktree)
}

async fn set_wave_triggers_enabled(state: &HttpState, wave_id: &LfdId, enabled: bool) -> bool {
    // All remaining signals (Repo, Wave, CiFailure) are reactive/auto.
    let triggers = match state.store.list_triggers(Some(wave_id)).await {
        Ok(triggers) => triggers,
        Err(_) => return false,
    };
    let mut matched = false;
    for mut trigger in triggers {
        matched = true;
        if trigger.enabled != enabled {
            trigger.enabled = enabled;
            if state.store.update_trigger(&trigger).await.is_err() {
                return false;
            }
        }
    }
    matched
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
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{
        Signal, TerminalSession, TerminalSessionStatus, Wave, WaveMode, WaveStatus,
    };
    use std::path::Path;
    use std::process::Command;
    use tempfile::tempdir;
    use time::OffsetDateTime;

    fn make_wave(repo: &str, name: &str) -> Wave {
        Wave {
            id: LfdId::new(),
            name: name.to_string(),
            repo: repo.to_string(),
            mode: WaveMode::Loop,
            primary_flow: "ship-roadmap".to_string(),
            cron: None,
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
        }
    }

    fn init_git_repo_with_origin(path: &Path) {
        let root = path.parent().expect("repo parent");
        let origin = root.join("origin.git");

        let run = |cwd: &Path, args: &[&str]| {
            let output = Command::new("git")
                .args(args)
                .current_dir(cwd)
                .output()
                .expect("git should run");
            assert!(
                output.status.success(),
                "git {:?} failed: {}",
                args,
                String::from_utf8_lossy(&output.stderr)
            );
        };

        std::fs::create_dir_all(path).expect("create repo directory");
        run(
            root,
            &[
                "init",
                "--bare",
                "-b",
                "main",
                origin.to_str().expect("origin path"),
            ],
        );
        run(path, &["init", "-b", "main"]);
        run(path, &["config", "user.email", "test@example.com"]);
        run(path, &["config", "user.name", "Test User"]);
        std::fs::write(path.join("README.md"), "seed").expect("write seed file");
        run(path, &["add", "."]);
        run(path, &["commit", "-m", "init"]);
        run(
            path,
            &[
                "remote",
                "add",
                "origin",
                origin.to_str().expect("origin path"),
            ],
        );
        run(path, &["push", "-u", "origin", "main"]);
        run(
            path,
            &[
                "symbolic-ref",
                "refs/remotes/origin/HEAD",
                "refs/remotes/origin/main",
            ],
        );
    }

    #[test]
    fn wave_trigger_schema_requires_source() {
        let trigger = TriggerDef {
            signal: "wave".to_string(),
            flow: None,
            source: None,
            source_repo: None,
        };

        let result = parse_trigger(&trigger);
        assert!(result.is_err());
    }

    #[test]
    fn wave_trigger_schema_parses_source_and_source_repo() {
        let trigger = TriggerDef {
            signal: "wave".to_string(),
            flow: None,
            source: Some("infra".to_string()),
            source_repo: Some("/tmp/source".to_string()),
        };

        let parsed = parse_trigger(&trigger).expect("parse trigger");
        assert_eq!(parsed.signal, Signal::Wave);
        assert_eq!(parsed.source.as_deref(), Some("infra"));
        assert_eq!(parsed.source_repo.as_deref(), Some("/tmp/source"));
    }

    #[tokio::test]
    async fn resolve_wave_id_in_repo_matches_repo_scope() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("test.db");
        let store = std::sync::Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );

        let repo_a = "/tmp/repo-a";
        let repo_b = "/tmp/repo-b";
        let wave_a = make_wave(repo_a, "infra");
        let wave_b = make_wave(repo_b, "infra");

        store.create_wave(&wave_a).await.expect("create wave_a");
        store.create_wave(&wave_b).await.expect("create wave_b");

        let resolved = resolve_wave_id_in_repo(&store, repo_b, "infra")
            .await
            .expect("wave should resolve");
        assert_eq!(resolved, *wave_b.id());
    }

    #[tokio::test]
    async fn resolve_wave_id_in_repo_errors_when_missing() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("test.db");
        let store = std::sync::Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );

        let err = resolve_wave_id_in_repo(&store, "/tmp/repo-missing", "infra")
            .await
            .expect_err("missing source wave should error");
        assert_eq!(err.0, StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn resolve_wave_source_wave_id_rejects_self_target() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("test.db");
        let store = std::sync::Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );

        let parsed = ParsedTrigger {
            signal: Signal::Wave,
            flow: None,
            source: Some("designer".to_string()),
            source_repo: None,
        };
        let err = resolve_wave_source_wave_id(&store, "/tmp/repo", "designer", Some(&parsed))
            .await
            .expect_err("self target should be rejected");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn resolve_wave_source_wave_id_resolves_cross_repo_source() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("test.db");
        let store = std::sync::Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );

        let source_wave = make_wave("/tmp/source-repo", "infra");
        store
            .create_wave(&source_wave)
            .await
            .expect("create source wave");

        let parsed = ParsedTrigger {
            signal: Signal::Wave,
            flow: None,
            source: Some("infra".to_string()),
            source_repo: Some("/tmp/source-repo".to_string()),
        };
        let resolved =
            resolve_wave_source_wave_id(&store, "/tmp/listener-repo", "designer", Some(&parsed))
                .await
                .expect("source should resolve");
        assert_eq!(resolved, Some(source_wave.id().clone()));
    }

    #[tokio::test]
    async fn create_and_get_handlers() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        let repo_path = repo_tmp.path().join("repo");
        init_git_repo_with_origin(&repo_path);
        let repo = repo_path.to_string_lossy().to_string();

        let Json(created) = create_wave_handler(
            State(state.clone()),
            Json(CreateWaveRequest {
                repo: repo.clone(),
                name: Some("designer".to_string()),
                flow: Some("build".to_string()),
                direction: Some(vec!["clarity".to_string()]),
                area: Some(vec!["src/".to_string()]),
                workers: None,
                status: None,
                run: false,
                serialized: false,
            }),
        )
        .await
        .expect("create wave");

        let Json(found) = get_wave_handler(
            State(state),
            Path(created.id.clone()),
            Query(ExpandQuery::default()),
        )
        .await
        .expect("get wave");

        assert_eq!(found.id, created.id);
        assert_eq!(found.name, "designer");
        assert_eq!(found.repo, repo);
        assert_eq!(found.primary_flow, "build");
        assert_eq!(found.direction, vec!["clarity".to_string()]);
        assert_eq!(found.area, vec!["src/".to_string()]);
    }

    #[tokio::test]
    async fn serialized_manual_run_applies_flow_override() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_tmp.path());
        let repo = repo_tmp.path().to_string_lossy().to_string();

        let Json(created) = create_wave_handler(
            State(state.clone()),
            Json(CreateWaveRequest {
                repo: repo.clone(),
                name: Some("designer".to_string()),
                flow: Some("ship-roadmap".to_string()),
                direction: None,
                area: None,
                workers: None,
                status: Some("paused".to_string()),
                run: false,
                serialized: true,
            }),
        )
        .await
        .expect("create wave");

        let mut wave = state
            .store
            .get_wave(&created.id.parse().expect("wave id"))
            .await
            .expect("load wave")
            .expect("wave exists");

        let run = start_wave_run(
            &state,
            &mut wave,
            Some(RunWaveRequest {
                area: None,
                direction: None,
                flow: Some("design".to_string()),
                roadmap_item: None,
            }),
        )
        .await
        .expect("run wave");

        assert_eq!(wave.status, WaveStatus::Idle);
        if let Some(run) = run {
            assert_eq!(run.snapshot.flow, "design");
        }
    }

    #[tokio::test]
    async fn create_wave_uses_mode_from_wave_config() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_tmp.path());
        let repo = repo_tmp.path().to_string_lossy().to_string();
        let wave_dir = repo_tmp.path().join("wave").join("designer");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(
            wave_dir.join("designer.yaml"),
            "flow: build\nmode: manual\ndirection: [clarity]\narea: [src/]\n",
        )
        .expect("write wave config");

        let Json(created) = create_wave_handler(
            State(state.clone()),
            Json(CreateWaveRequest {
                repo,
                name: Some("designer".to_string()),
                flow: None,
                direction: None,
                area: None,
                workers: None,
                status: None,
                run: false,
                serialized: false,
            }),
        )
        .await
        .expect("create wave");

        assert_eq!(created.mode, "manual");
        assert_eq!(created.primary_flow, "build");
        assert_eq!(created.direction, vec!["clarity".to_string()]);
        assert_eq!(created.area, vec!["src/".to_string()]);
    }

    #[tokio::test]
    async fn create_wave_accepts_initial_paused_status() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_tmp.path());
        let repo = repo_tmp.path().to_string_lossy().to_string();

        let Json(created) = create_wave_handler(
            State(state),
            Json(CreateWaveRequest {
                repo,
                name: Some("designer".to_string()),
                flow: None,
                direction: None,
                area: None,
                workers: None,
                status: Some("paused".to_string()),
                run: false,
                serialized: false,
            }),
        )
        .await
        .expect("create wave");

        assert_eq!(created.status, "paused");
    }

    #[tokio::test]
    async fn stop_wave_cancels_active_terminal_session_for_waiting_run() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_tmp.path());
        let repo = repo_tmp.path().to_string_lossy().to_string();

        let mut wave = make_wave(&repo, "designer");
        wave.status = WaveStatus::Running;
        state
            .store
            .create_wave(&wave)
            .await
            .expect("wave should be created");

        let mut run = WaveRun::new(LfdId::new(), wave.id().clone());
        run.snapshot.repo = repo.clone();
        run.snapshot.flow = "build".to_string();
        run.status = WaveRunStatus::Waiting;
        run.worktree = repo.clone();
        run.branch = "main".to_string();
        state
            .store
            .create_wave_run(&run)
            .await
            .expect("run should be created");

        let session = TerminalSession {
            id: LfdId::new(),
            wave_id: wave.id().clone(),
            wave_run_id: Some(run.id.clone()),
            step: "design".to_string(),
            agent: "lf".to_string(),
            cwd: repo.clone(),
            argv: vec!["lf".to_string(), "design".to_string()],
            env: Default::default(),
            source: "wave_step".to_string(),
            tmux_name: "lf-test-branch".to_string(),
            status: TerminalSessionStatus::Running,
            attached_at: Some(OffsetDateTime::now_utc()),
            started_at: Some(OffsetDateTime::now_utc()),
            completed_at: None,
            created_at: OffsetDateTime::now_utc(),
            completion_token: Some("token".to_string()),
        };
        state
            .store
            .create_terminal_session(&session)
            .await
            .expect("terminal session should be created");

        let Json(response) = stop_wave_handler(State(state.clone()), Path(wave.id().to_string()))
            .await
            .expect("stop wave");
        assert!(response.stopped);

        let updated_run = state
            .store
            .get_wave_run(&run.id)
            .await
            .expect("run lookup should succeed")
            .expect("run should exist");
        assert_eq!(updated_run.status, WaveRunStatus::Failed);
        assert_eq!(updated_run.error.as_deref(), Some("stopped"));

        let updated_wave = state
            .store
            .get_wave(wave.id())
            .await
            .expect("wave lookup should succeed")
            .expect("wave should exist");
        assert_eq!(updated_wave.status, WaveStatus::Failed);

        let updated_session = state
            .store
            .get_terminal_session(&session.id)
            .await
            .expect("session lookup should succeed")
            .expect("session should exist");
        assert_eq!(updated_session.status, TerminalSessionStatus::Canceled);
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

        let _ = create_wave_handler(
            State(state.clone()),
            Json(CreateWaveRequest {
                repo: repo_a.clone(),
                name: Some("wave-a".to_string()),
                flow: None,
                direction: None,
                area: None,
                workers: None,
                status: None,
                run: false,
                serialized: false,
            }),
        )
        .await
        .expect("create wave in repo a");
        let _ = create_wave_handler(
            State(state.clone()),
            Json(CreateWaveRequest {
                repo: repo_b.clone(),
                name: Some("wave-b".to_string()),
                flow: None,
                direction: None,
                area: None,
                workers: None,
                status: None,
                run: false,
                serialized: false,
            }),
        )
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

        let Json(created) = create_wave_handler(
            State(state.clone()),
            Json(CreateWaveRequest {
                repo,
                name: Some("before".to_string()),
                flow: Some("ship-roadmap".to_string()),
                direction: Some(vec!["infra".to_string()]),
                area: Some(vec!["src/".to_string()]),
                workers: None,
                status: None,
                run: false,
                serialized: false,
            }),
        )
        .await
        .expect("create wave");

        let Json(updated) = update_wave_handler(
            State(state.clone()),
            Path(created.id.clone()),
            Json(UpdateWaveRequest {
                name: Some("after".to_string()),
                flow: Some("build".to_string()),
                direction: Some(vec!["clarity".to_string()]),
                area: Some(vec!["docs/".to_string()]),
                ..Default::default()
            }),
        )
        .await
        .expect("update wave");

        assert_eq!(updated.name, "after");
        assert_eq!(updated.primary_flow, "build");
        assert_eq!(updated.direction, vec!["clarity".to_string()]);
        assert_eq!(updated.area, vec!["docs/".to_string()]);

        let Json(found) = get_wave_handler(
            State(state),
            Path(created.id),
            Query(ExpandQuery::default()),
        )
        .await
        .expect("get updated wave");
        assert_eq!(found.name, "after");
        assert_eq!(found.primary_flow, "build");
        assert_eq!(found.direction, vec!["clarity".to_string()]);
        assert_eq!(found.area, vec!["docs/".to_string()]);
    }

    #[tokio::test]
    async fn delete_then_get_returns_not_found() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_tmp.path());
        let repo = repo_tmp.path().to_string_lossy().to_string();

        let Json(created) = create_wave_handler(
            State(state.clone()),
            Json(CreateWaveRequest {
                repo,
                name: Some("delete-me".to_string()),
                flow: None,
                direction: None,
                area: None,
                workers: None,
                status: None,
                run: false,
                serialized: false,
            }),
        )
        .await
        .expect("create wave");

        let Json(deleted) = delete_wave_handler(State(state.clone()), Path(created.id.clone()))
            .await
            .expect("delete wave");
        assert!(deleted.deleted);

        let missing = get_wave_handler(
            State(state),
            Path(created.id),
            Query(ExpandQuery::default()),
        )
        .await;
        assert!(matches!(missing, Err((StatusCode::NOT_FOUND, _))));
    }

    #[tokio::test]
    async fn trigger_add_remove_handlers() {
        let state = test_http_state().await;
        let wave = make_wave("/tmp/repo", "trigger-wave");
        state.store.create_wave(&wave).await.expect("seed wave");

        let Json(initial) =
            list_triggers_handler(State(state.clone()), Path(wave.id().to_string()))
                .await
                .expect("list initial triggers");
        let initial_data = initial["data"].as_array().expect("triggers data array");
        assert!(initial_data.is_empty());

        let Json(added) = add_trigger_handler(
            State(state.clone()),
            Path(wave.id().to_string()),
            Json(AddTriggerRequest {
                signal: Signal::Repo,
                flow: Some("integrate".to_string()),
                source_wave_id: None,
                max_iterations: None,
            }),
        )
        .await
        .expect("add trigger");
        let trigger_id = added["id"]
            .as_str()
            .expect("trigger id in add response")
            .to_string();

        let Json(after_add) =
            list_triggers_handler(State(state.clone()), Path(wave.id().to_string()))
                .await
                .expect("list added triggers");
        let after_add_data = after_add["data"].as_array().expect("triggers data array");
        assert_eq!(after_add_data.len(), 1);
        assert_eq!(
            after_add_data[0]["id"].as_str().expect("trigger id"),
            trigger_id
        );

        let _ = remove_trigger_handler(
            State(state.clone()),
            Path((wave.id().to_string(), trigger_id)),
        )
        .await
        .expect("remove trigger");

        let Json(after_remove) = list_triggers_handler(State(state), Path(wave.id().to_string()))
            .await
            .expect("list triggers after remove");
        let after_remove_data = after_remove["data"]
            .as_array()
            .expect("triggers data array");
        assert!(after_remove_data.is_empty());
    }
}
