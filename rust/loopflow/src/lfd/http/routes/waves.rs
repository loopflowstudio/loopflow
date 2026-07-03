use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::Json;
use secrecy::ExposeSecret;
use serde::Deserialize;
use std::path::{Path as FsPath, PathBuf};
use std::str::FromStr;
use time::OffsetDateTime;
use tokio::process::Command as TokioCommand;
use tracing::warn;

use super::session_controls::connection_host;
use crate::engine::git::{branch_rename, current_branch, delete_remote_branch, push_with_upstream};
use crate::engine::naming::sanitize_for_branch;
use crate::engine::platform::kill_process;
use crate::engine::worktree::remove_worktree;
use crate::engine::worktrees::{branch_exists, worktree_path};
use crate::lfd::executor::ensure_wave_worktree;
use crate::lfd::http::dto::{
    activation_log_dto, session_connection_info_dto, session_dto, trigger_dto, wave_cron_dto,
    ActivationLogDto, CombineResponse, CombineResponseResult, DeletedResourceResponse,
    ErrorResponse, LandWaveResponse, ListResponse, NextWaveResponse, RestartStepResponse,
    SessionDto, StopWaveResponse, WaveAgentTreeDto, WaveAgentTreeSessionDto, WaveCronDto, WaveDto,
};
#[cfg(test)]
use crate::lfd::http::routes::wave_config::TriggerDef;
use crate::lfd::http::routes::wave_config::{
    read_wave_config, update_wave_agent_config, WaveConfig, WaveCronDef,
};
use crate::lfd::http::routes::{build_wave_dto, hooks, resolve_wave_id, ApiError};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiMessage, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::triggers::activation::create_run;
use crate::lfd::triggers::{
    spawn_immediate_activation, spawn_run_task_with_session, spawn_run_task_with_slot,
    ActivationEnvelope, ImmediateActivation,
};
use crate::lfd::types::{
    Event, ExecutionProcessStatus, Run, RunStatus, Session, SessionUse, Signal, Trigger, Wave,
    WaveCron, WaveMode, WaveStatus, LIVE_SESSION_STATUSES,
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
    goal: Option<String>,
    crons: Option<Vec<WaveCronDef>>,
    direction: Option<Vec<String>>,
    area: Option<Vec<String>>,
    workers: Option<u32>,
    status: Option<String>,
    #[serde(default)]
    run: bool,
    #[serde(default)]
    serialized: bool,
}

#[derive(Debug, Deserialize)]
pub struct RunWorkerRequest {
    flow: String,
    task: String,
    parent_session_id: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct GetWaveAgentTreeQuery {
    active_only: Option<bool>,
}

#[derive(Debug, Deserialize, Default)]
pub struct UpdateWaveRequest {
    name: Option<String>,
    flow: Option<String>,
    goal: Option<String>,
    crons: Option<Vec<WaveCronDef>>,
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
    goal: Option<String>,
    roadmap_item: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddTriggerRequest {
    signal: Signal,
    flow: Option<String>,
    source_wave_id: Option<String>,
    max_iterations: Option<u32>,
}

#[cfg(test)]
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
        goal,
        crons: requested_crons,
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

    let mode = wave_config
        .as_ref()
        .and_then(|c| c.mode.as_deref())
        .and_then(|m| m.parse::<WaveMode>().ok())
        .unwrap_or_default();
    let primary_flow = flow
        .or_else(|| wave_config.as_ref().and_then(|c| c.primary_flow.clone()))
        .or_else(|| wave_config.as_ref().and_then(|c| c.flow.clone()))
        .unwrap_or_else(|| "ship-roadmap".to_string());
    let config_goal = wave_config
        .as_ref()
        .and_then(|config| config.goal.as_deref());
    let goal = trimmed_non_empty(goal.as_deref())
        .or_else(|| trimmed_non_empty(config_goal))
        .unwrap_or_else(|| "ship-roadmap".to_string());
    let workers = create_wave_workers(requested_workers, serialized, wave_config.as_ref());
    let metrics = wave_config
        .as_ref()
        .and_then(|config| config.metrics.clone())
        .unwrap_or_default();
    let cron_defs = requested_crons.unwrap_or_default();
    let crons = build_wave_crons(&id, &cron_defs)?;

    let mut wave = Wave {
        id: id.clone(),
        name,
        repo,
        mode,
        primary_flow,
        goal,
        metrics,
        crons: Vec::new(),
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

    if let Err(err) = state.store.replace_wave_crons(wave.id(), &crons).await {
        let wave_id = wave.id().clone();
        let _ = state.store.delete_wave(&wave_id).await;
        return Err(map_store_error(err));
    }

    if let Err(err) = ensure_wave_workspace(&state, &wave).await {
        let wave_id = wave.id().clone();
        let _ = state.store.delete_wave(&wave_id).await;
        return Err(err);
    }

    let mut triggers: Vec<Trigger> = Vec::new();

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
        start_run(&state, &mut wave, None).await?;
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

#[cfg(test)]
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

fn required_trimmed(
    value: &str,
    message: &'static str,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    trimmed_non_empty(Some(value)).ok_or_else(|| api_error(StatusCode::BAD_REQUEST, message))
}

fn build_wave_crons(
    wave_id: &LfdId,
    cron_defs: &[WaveCronDef],
) -> Result<Vec<WaveCron>, (StatusCode, Json<ErrorResponse>)> {
    cron_defs
        .iter()
        .map(|cron| {
            Ok(WaveCron {
                id: LfdId::new(),
                wave_id: wave_id.clone(),
                flow: required_trimmed(&cron.flow, "wave cron flow is required")?,
                schedule: required_trimmed(&cron.schedule, "wave cron schedule is required")?,
                last_triggered_at: None,
                created_at: Some(OffsetDateTime::now_utc()),
            })
        })
        .collect()
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

#[cfg(test)]
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

#[cfg(test)]
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

    if let Some(cron_defs) = payload.crons {
        let crons = build_wave_crons(wave.id(), &cron_defs)?;
        state
            .store
            .replace_wave_crons(wave.id(), &crons)
            .await
            .map_err(map_store_error)?;
    }

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

    // Delete from the store (cascades to runs, agents, etc.).
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
    _headers: HeaderMap,
    Path(wave_id): Path<String>,
    payload: Option<Json<RunWaveRequest>>,
) -> ApiResult<SessionDto> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    if let Some(session) = active_wave_agent_session(&state, &wave_id).await? {
        return Ok(Json(session_dto(session)));
    }

    let mut wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;
    apply_run_wave_overrides(&state, &mut wave, payload.map(|Json(value)| value)).await?;
    let session = state
        .executor
        .launch_wave_agent_session(wave.id())
        .await
        .map_err(|err| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiMessage::Untrusted(err.to_string()),
            )
        })?;

    Ok(Json(session_dto(session)))
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
        child_waves: Vec::new(),
        sessions,
    }))
}

pub async fn run_worker_handler(
    State(state): State<HttpState>,
    _headers: HeaderMap,
    Path(wave_id): Path<String>,
    Json(payload): Json<RunWorkerRequest>,
) -> ApiResult<SessionDto> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    assert_parent_can_launch_worker(&state, payload.parent_session_id.as_deref()).await?;
    let wave = state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;
    let (_run, session) = launch_worker_run(
        &state,
        &wave,
        payload.flow,
        payload.task,
        payload.parent_session_id.as_deref(),
    )
    .await?;
    Ok(Json(session_dto(session)))
}

async fn apply_run_wave_overrides(
    state: &HttpState,
    wave: &mut Wave,
    overrides: Option<RunWaveRequest>,
) -> Result<(), ApiError> {
    if let Some(overrides) = overrides {
        if let Some(flow) = trimmed_non_empty(overrides.flow.as_deref()) {
            wave.primary_flow = flow;
        }
        if let Some(roadmap_item) = trimmed_non_empty(overrides.roadmap_item.as_deref()) {
            let roadmap_path = FsPath::new(wave.repo())
                .join("wave")
                .join(wave.name())
                .join(&roadmap_item);
            if !roadmap_path.is_file() {
                return Err(api_error(StatusCode::BAD_REQUEST, "roadmap item not found"));
            }
        }
        if let Some(goal) = trimmed_non_empty(overrides.goal.as_deref()) {
            wave.goal = goal;
        }
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
    if wave.status == WaveStatus::Idle {
        wave.status = WaveStatus::Running;
    }

    state
        .store
        .update_wave(wave)
        .await
        .map_err(map_store_error)?;
    let _ = set_wave_triggers_enabled(state, wave.id(), true).await;
    Ok(())
}

async fn launch_worker_run(
    state: &HttpState,
    wave: &Wave,
    flow: String,
    task: String,
    parent_session_id: Option<&str>,
) -> Result<(Run, Session), ApiError> {
    let flow = flow.trim();
    if flow.is_empty() {
        return Err(api_error(StatusCode::BAD_REQUEST, "flow is required"));
    }
    let task = task.trim();
    if task.is_empty() {
        return Err(api_error(StatusCode::BAD_REQUEST, "task is required"));
    }

    let active_runs = state
        .store
        .count_active_runs(wave.id())
        .await
        .map_err(map_store_error)?;
    if wave.workers() > 0 && active_runs >= wave.workers() {
        return Err(api_error(
            StatusCode::PRECONDITION_FAILED,
            "wave already at worker capacity",
        ));
    }

    let run_id = LfdId::new();
    let slot_guard = state
        .scheduler
        .acquire_guard(run_id.as_str())
        .await
        .map_err(|_| {
            api_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "no scheduler slots available",
            )
        })?;

    // Select the worktree strategy the same way activation does: a shared
    // per-wave worktree only for the strictly-serial workers==1 case, and an
    // isolated per-run worktree otherwise — so concurrent dispatches (workers>=2
    // or workers==0) never collide in the shared worktree.
    let mut run = create_run(&state.store, wave, &run_id, false, None)
        .await
        .map_err(|err| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiMessage::Untrusted(err.to_string()),
            )
        })?;
    run.flow = flow.to_string();
    run.task = Some(task.to_string());
    state
        .store
        .update_run(&run)
        .await
        .map_err(map_store_error)?;

    let session = state
        .executor
        .prepare_session_for_run_with_parent(
            &run.id,
            parent_session_id
                .map(|value| {
                    crate::lfd::http::routes::parse_lfd_id(value, "invalid parent session id")
                })
                .transpose()?,
        )
        .await
        .map_err(|err| {
            api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                ApiMessage::Untrusted(err.to_string()),
            )
        })?;

    spawn_run_task_with_session(
        state.store.clone(),
        (*state.executor).clone(),
        state.event_hub.clone(),
        run.clone(),
        slot_guard,
        session.id.clone(),
    );

    Ok((run, session))
}

async fn assert_parent_can_launch_worker(
    state: &HttpState,
    parent_session_id: Option<&str>,
) -> Result<(), ApiError> {
    let Some(parent_session_id) = parent_session_id else {
        return Ok(());
    };
    let parent_session_id =
        crate::lfd::http::routes::parse_lfd_id(parent_session_id, "invalid parent session id")?;
    let Some(session) = state
        .store
        .get_control_session(&parent_session_id)
        .await
        .map_err(map_store_error)?
    else {
        return Err(api_error(StatusCode::NOT_FOUND, "parent session not found"));
    };
    if session.session_use == SessionUse::Worker {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "worker sessions cannot launch worker sessions",
        ));
    }
    Ok(())
}

async fn active_wave_agent_session(
    state: &HttpState,
    wave_id: &LfdId,
) -> Result<Option<Session>, ApiError> {
    let sessions = state
        .store
        .list_control_sessions(Some(wave_id), Some(LIVE_SESSION_STATUSES))
        .await
        .map_err(map_store_error)?;
    Ok(sessions
        .into_iter()
        .find(|session| session.session_use == SessionUse::WaveAgent))
}

async fn start_run(
    state: &HttpState,
    wave: &mut Wave,
    overrides: Option<RunWaveRequest>,
) -> Result<Option<Run>, ApiError> {
    let active_runs = state
        .store
        .count_active_runs(wave.id())
        .await
        .map_err(map_store_error)?;
    if wave.workers() > 0 && active_runs >= wave.workers() {
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
        if let Some(goal) = trimmed_non_empty(overrides.goal.as_deref()) {
            wave.goal = goal;
        }
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
            force_parallel: false,
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

pub async fn list_wave_crons_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<ListResponse<WaveCronDto>> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let crons = state
        .store
        .list_wave_crons(&wave_id)
        .await
        .map_err(map_store_error)?;
    let data = crons.into_iter().map(wave_cron_dto).collect();
    Ok(Json(ListResponse::new(data, false)))
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
        .get_active_run(&wave_id)
        .await
        .map_err(map_store_error)?;

    // Disable all auto triggers so tickers won't restart the wave.
    let has_auto_trigger = set_wave_triggers_enabled(&state, &wave_id, false).await;

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
    state
        .event_hub
        .send(Event::session_updated(session.clone()));

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
        .get_active_run(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "no active run for wave"))?;

    if run.status != RunStatus::Running {
        return Err(api_error(
            StatusCode::PRECONDITION_FAILED,
            "wave run is not running",
        ));
    }

    // Kill running agents.
    terminate_active_agents(&state, &wave_id).await?;
    mark_active_agents_failed(&state, &wave_id).await;

    // Re-acquire scheduler slot and relaunch.
    let run_id = run.id.to_string();
    let step_index = run.step_index;
    respawn_run_task(&state, run).await?;

    state.event_hub.send(Event::wave_updated(wave_id.clone()));

    Ok(Json(RestartStepResponse {
        restarted: true,
        wave_id: wave_id.to_string(),
        run_id,
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
            ExecutionProcessStatus::Failed.as_i32(),
            OffsetDateTime::now_utc().unix_timestamp(),
        )
        .await;
}

async fn respawn_run_task(
    state: &HttpState,
    run: Run,
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
        .get_latest_run(wave.id())
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
                    agent: None,
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
        if let Err(err) = state.store.update_run(&run).await {
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
        .get_latest_run(wave_id)
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
    use crate::lfd::auth::{AuthFailureThrottle, AuthProvider};
    use crate::lfd::config::{GitHubConfig, HttpSecurityConfig};
    use crate::lfd::conversations::ConversationManager;
    use crate::lfd::events::EventHub;
    use crate::lfd::executor::{AgentExecutor, ExecutionContext, WaveExecutor};
    use crate::lfd::http::routes::test_helpers::{init_git_repo, test_http_state};
    use crate::lfd::output::OutputHub;
    use crate::lfd::provider_auth::ProviderAuthService;
    use crate::lfd::scheduler::Scheduler;
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{
        Session, SessionStatus, SessionUse, Signal, Wave, WaveMode, WaveStatus,
    };
    use anyhow::Result;
    use async_trait::async_trait;
    use std::path::Path;
    use std::process::Command;
    use std::sync::Arc;
    use tempfile::tempdir;
    use time::OffsetDateTime;
    use tokio::sync::Mutex;

    struct FailingRunner;

    #[async_trait]
    impl AgentExecutor for FailingRunner {
        async fn run(
            &self,
            _cmd: Vec<String>,
            _cwd: &Path,
            _context: ExecutionContext,
        ) -> Result<i32> {
            Ok(1)
        }

        async fn terminate(&self, _agent_id: &str) -> Result<()> {
            Ok(())
        }
    }

    async fn test_http_state_with_runner() -> HttpState {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store: crate::lfd::store::SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );
        let scheduler = Arc::new(Scheduler::new(1));
        let output_hub = OutputHub::new(128, tmp.path().join("output"));
        let event_hub = EventHub::new(128);
        let sessions = ConversationManager::new(store.clone());
        let executor = Arc::new(WaveExecutor::with_runner(
            store.clone(),
            scheduler.clone(),
            output_hub.clone(),
            event_hub.clone(),
            Arc::new(FailingRunner),
        ));

        HttpState {
            store: store.clone(),
            scheduler,
            executor,
            event_hub,
            output_hub,
            provider_auth: ProviderAuthService::new(store),
            auth: AuthProvider::Bearer {
                session_token: secrecy::SecretString::from("test-token".to_string()),
            },
            started_at: OffsetDateTime::now_utc(),
            github: GitHubConfig::default(),
            http_security: HttpSecurityConfig::default(),
            auth_failure_throttle: AuthFailureThrottle::new(),
            ci_failure_cache: Arc::new(Mutex::new(std::collections::HashSet::new())),
            conversations: sessions,
        }
    }

    fn make_wave(repo: &str, name: &str) -> Wave {
        Wave {
            id: LfdId::new(),
            name: name.to_string(),
            repo: repo.to_string(),
            mode: WaveMode::Loop,
            primary_flow: "ship-roadmap".to_string(),
            goal: "ship-roadmap".to_string(),
            metrics: Vec::new(),
            crons: Vec::new(),
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            cycle_start_iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            workers: 1,
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
            flow: Some("build".to_string()),
            source: Some("infra".to_string()),
            source_repo: Some("/tmp/source".to_string()),
        };

        let parsed = parse_trigger(&trigger).expect("parse trigger");
        assert_eq!(parsed.signal, Signal::Wave);
        assert_eq!(parsed.flow.as_deref(), Some("build"));
        assert_eq!(parsed.source.as_deref(), Some("infra"));
        assert_eq!(parsed.source_repo.as_deref(), Some("/tmp/source"));
    }

    #[test]
    fn create_wave_workers_allows_zero() {
        assert_eq!(create_wave_workers(Some(0), false, None), 0);
    }

    #[test]
    fn build_wave_crons_requires_flow_and_schedule() {
        let wave_id = LfdId::new();
        assert!(build_wave_crons(
            &wave_id,
            &[WaveCronDef {
                flow: "".to_string(),
                schedule: "0 0 * * *".to_string(),
            }]
        )
        .is_err());
        assert!(build_wave_crons(
            &wave_id,
            &[WaveCronDef {
                flow: "wave-polish".to_string(),
                schedule: "".to_string(),
            }]
        )
        .is_err());
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
                goal: None,
                crons: None,
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
                goal: None,
                crons: None,
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

        let run = start_run(
            &state,
            &mut wave,
            Some(RunWaveRequest {
                area: None,
                direction: None,
                flow: Some("design".to_string()),
                goal: None,
                roadmap_item: None,
            }),
        )
        .await
        .expect("run wave");

        assert_eq!(wave.status, WaveStatus::Idle);
        if let Some(run) = run {
            assert_eq!(run.flow, "design");
        }
    }

    #[tokio::test]
    async fn run_worker_handler_returns_launch_session() {
        let state = test_http_state_with_runner().await;
        let repo = tempdir().expect("tempdir");
        init_git_repo(repo.path());
        let wave = make_wave(&repo.path().to_string_lossy(), "worker-wave");
        state.store.create_wave(&wave).await.expect("seed wave");
        let caller = make_session(&wave, SessionUse::WaveAgent, repo.path());
        state
            .store
            .create_control_session(&caller)
            .await
            .expect("seed caller session");

        let Json(response) = run_worker_handler(
            State(state.clone()),
            HeaderMap::new(),
            Path(wave.id().to_string()),
            Json(RunWorkerRequest {
                flow: "implement".to_string(),
                task: "Add the worker endpoint.".to_string(),
                parent_session_id: Some(caller.id.to_string()),
            }),
        )
        .await
        .expect("run worker");

        assert_eq!(response.object, "session");
        assert_eq!(response.session_use, "worker");
        assert!(response.run_id.is_some());
        assert_eq!(
            response.parent_session_id.as_deref(),
            Some(caller.id.as_str())
        );
        assert_eq!(response.session_use, "worker");
        assert!(!response.tmux_name.is_empty());

        let response_session_id = LfdId::from_raw(response.id.clone());
        let mut stored_session = state
            .store
            .get_control_session(&response_session_id)
            .await
            .expect("load terminal session")
            .expect("session exists");
        for _ in 0..20 {
            if stored_session.status == SessionStatus::Failed {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            stored_session = state
                .store
                .get_control_session(&response_session_id)
                .await
                .expect("load terminal session")
                .expect("session exists");
        }
        assert_eq!(stored_session.status, SessionStatus::Failed);

        let sessions = state
            .store
            .list_control_sessions(Some(wave.id()), None)
            .await
            .expect("list terminal sessions");
        let child = sessions
            .iter()
            .find(|session| session.id == response_session_id)
            .expect("launched session should be stored");
        assert_eq!(child.parent_session_id.as_ref(), Some(&caller.id));
    }

    #[tokio::test]
    async fn run_worker_handler_rejects_worker_caller() {
        let state = test_http_state_with_runner().await;
        let repo = tempdir().expect("tempdir");
        init_git_repo(repo.path());
        let wave = make_wave(&repo.path().to_string_lossy(), "blocked-worker-wave");
        state.store.create_wave(&wave).await.expect("seed wave");
        let caller = make_session(&wave, SessionUse::Worker, repo.path());
        state
            .store
            .create_control_session(&caller)
            .await
            .expect("seed caller session");

        let result = run_worker_handler(
            State(state.clone()),
            HeaderMap::new(),
            Path(wave.id().to_string()),
            Json(RunWorkerRequest {
                flow: "implement".to_string(),
                task: "Add the worker endpoint.".to_string(),
                parent_session_id: Some(caller.id.to_string()),
            }),
        )
        .await;

        let err = result.expect_err("worker caller should be rejected");
        assert_eq!(err.0, StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn get_wave_agent_tree_returns_sessions_with_connection() {
        let state = test_http_state_with_runner().await;
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
    // Reproduces the "Ingest & build" empty-reply bug: when a wave has PM
    // configured, POST /waves/{id}/run with a roadmap_item aborts the HTTP
    // connection without a response, because the ingest code path calls
    // `block_on_pm` (which constructs a new Tokio runtime) from inside the
    // axum handler's runtime context — Tokio panics with "Cannot start a
    // runtime from within a runtime" and axum's panic handler can't always
    // recover a clean 500. The handler should respond with a structured error.
    #[tokio::test]
    async fn run_wave_with_roadmap_item_and_pm_config_does_not_panic() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_tmp.path());
        let repo = repo_tmp.path().to_string_lossy().to_string();

        let lf_dir = repo_tmp.path().join(".lf");
        std::fs::create_dir_all(&lf_dir).expect("create .lf");
        std::fs::write(lf_dir.join("config.yaml"), "pm:\n  provider: asana\n")
            .expect("write config");

        let wave_dir = repo_tmp.path().join("wave").join("designer");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(
            wave_dir.join("GOAL.md"),
            "---\npm:\n  provider: asana\n  asana_project: '123'\n---\nDrive the work.\n",
        )
        .expect("write wave goal");
        std::fs::write(wave_dir.join("1-something.md"), "# Something\n")
            .expect("write roadmap item");

        let Json(created) = create_wave_handler(
            State(state.clone()),
            Json(CreateWaveRequest {
                repo: repo.clone(),
                name: Some("designer".to_string()),
                flow: Some("ship-roadmap".to_string()),
                goal: None,
                crons: None,
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

        // The handler must return a Result — not panic and not hang — even when
        // the PM-backed ingest code path runs inside the async handler context.
        let result = run_wave_handler(
            State(state.clone()),
            HeaderMap::new(),
            Path(created.id.clone()),
            Some(Json(RunWaveRequest {
                area: None,
                direction: None,
                flow: Some("build".to_string()),
                goal: None,
                roadmap_item: Some("1-something.md".to_string()),
            })),
        )
        .await;

        // We accept either success or a structured error — the failure mode we
        // are guarding against is a panic from `block_on` inside async that
        // aborts the HTTP response entirely.
        match result {
            Ok(_) => {}
            Err((status, _)) => assert!(
                status.is_client_error() || status.is_server_error(),
                "unexpected status: {status}"
            ),
        }
    }

    // Reproduces the "Ingest & build" failure in Concerto: POST /waves/{id}/run
    // with a `roadmap_item` that doesn't exist under wave/<name>/ returns an
    // empty reply to the client. The handler should respond with a structured
    // 400 error, not close the connection.
    #[tokio::test]
    async fn run_wave_with_missing_roadmap_item_returns_bad_request() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_tmp.path());
        let repo = repo_tmp.path().to_string_lossy().to_string();

        let wave_dir = repo_tmp.path().join("wave").join("designer");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(
            wave_dir.join("1-existing-item.md"),
            "# Existing roadmap item\n",
        )
        .expect("write existing roadmap item");

        let Json(created) = create_wave_handler(
            State(state.clone()),
            Json(CreateWaveRequest {
                repo: repo.clone(),
                name: Some("designer".to_string()),
                flow: Some("ship-roadmap".to_string()),
                goal: None,
                crons: None,
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

        let result = run_wave_handler(
            State(state.clone()),
            HeaderMap::new(),
            Path(created.id.clone()),
            Some(Json(RunWaveRequest {
                area: None,
                direction: None,
                flow: Some("build".to_string()),
                goal: None,
                roadmap_item: Some("does-not-exist.md".to_string()),
            })),
        )
        .await;

        let err = result.expect_err("expected handler to return an error, not panic or hang");
        assert_eq!(
            err.0,
            StatusCode::BAD_REQUEST,
            "expected 400 for missing roadmap item, got {err:?}"
        );
    }

    #[tokio::test]
    async fn create_wave_uses_intent_from_goal_md() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_tmp.path());
        let repo = repo_tmp.path().to_string_lossy().to_string();
        let wave_dir = repo_tmp.path().join("wave").join("designer");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(
            wave_dir.join("GOAL.md"),
            "---\nprimary_flow: build\nmode: manual\ndirection: [clarity]\narea: [src/]\nmetrics:\n  - tests pass\n---\nDrive the work.\n",
        )
        .expect("write wave goal");

        let Json(created) = create_wave_handler(
            State(state.clone()),
            Json(CreateWaveRequest {
                repo,
                name: Some("designer".to_string()),
                flow: None,
                goal: None,
                crons: None,
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
        assert_eq!(created.goal, "designer");
        assert_eq!(created.metrics, vec!["tests pass".to_string()]);
        assert_eq!(created.direction, Vec::<String>::new());
        assert_eq!(created.area, Vec::<String>::new());
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
                goal: None,
                crons: None,
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
    async fn create_wave_ignores_crons_from_goal_md() {
        let state = test_http_state().await;
        let repo_tmp = tempdir().expect("tempdir");
        init_git_repo(repo_tmp.path());
        let repo = repo_tmp.path().to_string_lossy().to_string();
        let wave_dir = repo_tmp.path().join("wave").join("designer");
        std::fs::create_dir_all(&wave_dir).expect("create wave dir");
        std::fs::write(
            wave_dir.join("GOAL.md"),
            "---\nprimary_flow: build\nworkers: 0\ncrons:\n  - flow: wave-polish\n    schedule: '0 0 * * 1'\n---\nDrive the work.\n",
        )
        .expect("write wave goal");

        let Json(created) = create_wave_handler(
            State(state.clone()),
            Json(CreateWaveRequest {
                repo,
                name: Some("designer".to_string()),
                flow: None,
                goal: None,
                crons: None,
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

        assert_eq!(created.workers, 0);
        assert!(created.crons.is_empty());

        let Json(listed) = list_wave_crons_handler(State(state), Path(created.id.clone()))
            .await
            .expect("list crons");
        assert!(listed.data.is_empty());
    }

    #[tokio::test]
    async fn stop_wave_cancels_active_session_for_waiting_run() {
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
        assert_eq!(updated_wave.status, WaveStatus::Failed);

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

        let _ = create_wave_handler(
            State(state.clone()),
            Json(CreateWaveRequest {
                repo: repo_a.clone(),
                name: Some("wave-a".to_string()),
                flow: None,
                goal: None,
                crons: None,
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
                goal: None,
                crons: None,
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
                goal: None,
                crons: None,
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
                goal: None,
                crons: None,
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
