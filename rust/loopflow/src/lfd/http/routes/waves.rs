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
use crate::lfd::executor::ensure_wave_worktree;
use crate::lfd::executor::helpers::{auto_commit_if_dirty, resolve_current_step_name};
use crate::lfd::http::dto::{
    activation_log_dto, signal_str, stimulus_dto, ActivationLogDto, CombineResponse,
    CombineResponseResult, ContinueWaveResponse, DeletedResourceResponse, ErrorResponse,
    LandWaveResponse, ListResponse, NextWaveResponse, RestartStepResponse, RunWaveResponse,
    StopWaveResponse, WaveDto,
};
use crate::lfd::http::routes::wave_config::{
    read_wave_config, update_wave_agent_config, StimulusDef,
};
use crate::lfd::http::routes::{build_wave_dto, hooks, resolve_wave_id, ApiError};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiMessage, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::triggers::{
    dispatch_wave_if_ready, enqueue_pending_activation, spawn_immediate_activation,
    spawn_run_task_with_slot, ActivationEnvelope, EnqueueOutcome,
};
use crate::lfd::types::{
    ActivationSource, AgentStatus, Event, Signal, Stimulus, Wave, WaveRun, WaveRunStatus,
    WaveStatus,
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
    #[serde(alias = "schema")]
    name: Option<String>,
    flow: Option<String>,
    direction: Option<Vec<String>>,
    area: Option<Vec<String>>,
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
}

#[derive(Debug, Deserialize)]
pub struct AddStimulusRequest {
    #[serde(alias = "kind")]
    signal: Signal,
    flow: Option<String>,
    cron: Option<String>,
    source_wave_id: Option<String>,
    max_iterations: Option<u32>,
}

#[derive(Debug, Clone)]
struct ParsedStimulus {
    signal: Signal,
    flow: Option<String>,
    cron: Option<String>,
    source: Option<String>,
    source_repo: Option<String>,
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
        run,
        serialized,
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
        .or_else(|| wave_config.as_ref().and_then(|c| c.flow.clone()))
        .unwrap_or_else(|| "build".to_string());
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
    let listen_source_wave_id =
        resolve_listen_source_wave_id(&state.store, &repo, &name, config_stimulus.as_ref()).await?;

    let mut wave = Wave {
        id: id.clone(),
        name,
        repo,
        flow,
        direction,
        area,
        status: WaveStatus::Idle,
        iteration: 0,
        created_at: Some(OffsetDateTime::now_utc()),
        serialized,
    };
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

    // Collect stimuli: config-provided first, then defaults.
    let mut stimuli: Vec<Stimulus> = Vec::new();

    if let Some(parsed) = config_stimulus {
        let mut s = Stimulus::new(LfdId::new(), wave.id().clone(), parsed.signal);
        s.source_wave_id = listen_source_wave_id.clone();
        s.flow = parsed.flow;
        s.cron = parsed.cron;
        stimuli.push(s);
    }

    // Every wave gets watch (rebase on main) and ci-fix unless config already covers them.
    for (signal, flow) in [(Signal::Watch, "integrate"), (Signal::CiFailure, "ci-fix")] {
        if stimuli.iter().any(|s| s.signal == signal) {
            continue;
        }
        let mut s = Stimulus::new(LfdId::new(), wave.id().clone(), signal);
        s.flow = Some(flow.to_string());
        stimuli.push(s);
    }

    for stimulus in stimuli {
        if let Err(err) = state.store.create_stimulus(&stimulus).await {
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

fn parse_stimulus(
    stimulus: &StimulusDef,
) -> Result<ParsedStimulus, (StatusCode, Json<ErrorResponse>)> {
    let signal = match stimulus.kind.as_str() {
        "cron" => Signal::Cron,
        "watch" => Signal::Watch,
        "loop" => Signal::Loop,
        "once" => Signal::Once,
        "listen" => Signal::Listen,
        "ci_failure" => Signal::CiFailure,
        value => {
            return Err(api_error(
                StatusCode::BAD_REQUEST,
                ApiMessage::Safe(format!("invalid wave config stimulus kind '{value}'")),
            ));
        }
    };

    let cron_value = trimmed_non_empty(stimulus.cron.as_deref());
    let flow = trimmed_non_empty(stimulus.flow.as_deref());
    let source = trimmed_non_empty(stimulus.source.as_deref());
    let source_repo = trimmed_non_empty(stimulus.source_repo.as_deref());

    let cron = match signal {
        Signal::Cron => Some(cron_value.ok_or_else(|| {
            api_error(
                StatusCode::BAD_REQUEST,
                "wave config stimulus kind 'cron' requires a cron expression",
            )
        })?),
        Signal::Listen => {
            if cron_value.is_some() {
                return Err(api_error(
                    StatusCode::BAD_REQUEST,
                    "wave config listen stimulus does not accept cron",
                ));
            }
            None
        }
        _ => {
            if source.is_some() {
                return Err(api_error(
                    StatusCode::BAD_REQUEST,
                    "wave config source is only valid for listen stimulus",
                ));
            }
            if source_repo.is_some() {
                return Err(api_error(
                    StatusCode::BAD_REQUEST,
                    "wave config source_repo is only valid for listen stimulus",
                ));
            }
            None
        }
    };

    let source = if signal == Signal::Listen {
        Some(source.ok_or_else(|| {
            api_error(
                StatusCode::BAD_REQUEST,
                "wave config listen stimulus requires source",
            )
        })?)
    } else {
        None
    };
    let source_repo = if signal == Signal::Listen {
        source_repo
    } else {
        None
    };

    Ok(ParsedStimulus {
        signal,
        flow,
        cron,
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

async fn resolve_listen_source_wave_id(
    store: &crate::lfd::store::SharedStore,
    repo: &str,
    wave_name: &str,
    parsed: Option<&ParsedStimulus>,
) -> Result<Option<LfdId>, (StatusCode, Json<ErrorResponse>)> {
    let Some(parsed) = parsed else {
        return Ok(None);
    };
    if parsed.signal != Signal::Listen {
        return Ok(None);
    }
    let source = parsed
        .source
        .as_deref()
        .expect("listen stimulus parsing requires source");
    let source_repo = parsed.source_repo.as_deref().unwrap_or(repo);
    if source_repo == repo && source == wave_name {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "listen stimulus cannot target the same wave",
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
    if let Some(serialized) = payload.serialized {
        wave.serialized = serialized;
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
    if wave.serialized {
        let active_run = state
            .store
            .get_active_wave_run(wave.id())
            .await
            .map_err(map_store_error)?;
        if active_run.is_some() {
            return Err(api_error(
                StatusCode::PRECONDITION_FAILED,
                "wave already running",
            ));
        }
    }

    if let Some(overrides) = overrides {
        if let Some(flow) = overrides.flow {
            wave.flow = flow;
        }
        if let Some(direction) = overrides.direction {
            wave.direction = direction;
        }
        if let Some(area) = overrides.area {
            wave.area = area;
        }
    }

    state
        .store
        .update_wave(wave)
        .await
        .map_err(map_store_error)?;

    // Re-enable all stimuli (they may have been disabled by a previous stop).
    let _ = set_wave_stimuli_enabled(state, wave.id(), true, false).await;

    let stimulus_id = ensure_manual_stimulus(state, wave.id()).await?;
    let envelope = ActivationEnvelope::new(
        wave.id(),
        &stimulus_id,
        ActivationSource::Manual,
        "manual run requested via API",
        "",
        "",
        "main",
    );

    if wave.serialized {
        let outcome = enqueue_pending_activation(&state.store, &state.event_hub, envelope).await;
        match outcome {
            Some(EnqueueOutcome::Dropped) => {
                warn!(
                    wave_id = %wave.id(),
                    "manual activation dropped because activation queue is full"
                );
                return Ok(None);
            }
            Some(_) => {}
            None => {
                return Err(api_error(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "failed to queue manual activation",
                ));
            }
        }
        Ok(dispatch_wave_if_ready(
            &state.store,
            &state.executor,
            &state.scheduler,
            &state.event_hub,
            wave,
        )
        .await)
    } else {
        Ok(spawn_immediate_activation(
            &state.store,
            &state.executor,
            &state.scheduler,
            &state.event_hub,
            wave,
            None,
            envelope,
        )
        .await)
    }
}

async fn ensure_manual_stimulus(state: &HttpState, wave_id: &LfdId) -> Result<LfdId, ApiError> {
    let stimuli = state
        .store
        .list_stimuli(Some(wave_id))
        .await
        .map_err(map_store_error)?;
    if let Some(existing) = stimuli
        .into_iter()
        .find(|stimulus| stimulus.signal == Signal::Once)
    {
        return Ok(existing.id);
    }

    let stimulus = Stimulus {
        id: LfdId::new(),
        wave_id: wave_id.clone(),
        source_wave_id: None,
        signal: Signal::Once,
        flow: None,
        cron: None,
        last_main_sha: None,
        last_triggered_at: None,
        created_at: Some(OffsetDateTime::now_utc()),
        enabled: true,
        max_iterations: None,
    };
    state
        .store
        .create_stimulus(&stimulus)
        .await
        .map_err(map_store_error)?;
    Ok(stimulus.id)
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

    let source_wave_id = match payload.signal {
        Signal::Listen => {
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
        signal: payload.signal,
        flow: payload.flow,
        cron: payload.cron,
        last_main_sha: None,
        last_triggered_at: None,
        created_at: Some(OffsetDateTime::now_utc()),
        enabled: true,
        max_iterations: payload.max_iterations,
    };

    state
        .store
        .create_stimulus(&stimulus)
        .await
        .map_err(map_store_error)?;

    Ok(Json(serde_json::json!({
        "id": stimulus.id.to_string(),
        "kind": signal_str(stimulus.signal),
        "flow": stimulus.flow,
        "source_wave_id": stimulus.source_wave_id.as_ref().map(ToString::to_string),
        "cron": stimulus.cron,
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

fn is_auto_stimulus(signal: Signal) -> bool {
    matches!(
        signal,
        Signal::Loop | Signal::Watch | Signal::Cron | Signal::Listen | Signal::CiFailure
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
        if auto_only && !is_auto_stimulus(stimulus.signal) {
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
    use crate::lfd::store::{open_store, StorageConfig};
    use crate::lfd::types::{Wave, WaveStatus};
    use tempfile::tempdir;
    use time::OffsetDateTime;

    #[test]
    fn parse_stimulus_requires_cron_expression_for_cron_kind() {
        let stimulus = StimulusDef {
            kind: "cron".to_string(),
            flow: None,
            cron: None,
            source: None,
            source_repo: None,
        };

        let result = parse_stimulus(&stimulus);
        assert!(result.is_err());
    }

    #[test]
    fn parse_stimulus_accepts_valid_cron_expression() {
        let stimulus = StimulusDef {
            kind: "cron".to_string(),
            flow: None,
            cron: Some("0 8 * * *".to_string()),
            source: None,
            source_repo: None,
        };

        let parsed = parse_stimulus(&stimulus).expect("parse stimulus");
        assert_eq!(parsed.signal, Signal::Cron);
        assert_eq!(parsed.cron.as_deref(), Some("0 8 * * *"));
        assert!(parsed.source.is_none());
    }

    #[test]
    fn listen_stimulus_schema_requires_source() {
        let stimulus = StimulusDef {
            kind: "listen".to_string(),
            flow: None,
            cron: None,
            source: None,
            source_repo: None,
        };

        let result = parse_stimulus(&stimulus);
        assert!(result.is_err());
    }

    #[test]
    fn listen_stimulus_schema_parses_source_and_source_repo() {
        let stimulus = StimulusDef {
            kind: "listen".to_string(),
            flow: None,
            cron: None,
            source: Some("infra".to_string()),
            source_repo: Some("/tmp/source".to_string()),
        };

        let parsed = parse_stimulus(&stimulus).expect("parse stimulus");
        assert_eq!(parsed.signal, Signal::Listen);
        assert_eq!(parsed.source.as_deref(), Some("infra"));
        assert_eq!(parsed.source_repo.as_deref(), Some("/tmp/source"));
        assert!(parsed.cron.is_none());
    }

    #[test]
    fn listen_stimulus_is_auto_stimulus() {
        assert!(is_auto_stimulus(Signal::Listen));
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
        let wave_a = Wave {
            id: LfdId::new(),
            name: "infra".to_string(),
            repo: repo_a.to_string(),
            flow: "build".to_string(),
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            serialized: false,
        };
        let wave_b = Wave {
            id: LfdId::new(),
            name: "infra".to_string(),
            repo: repo_b.to_string(),
            flow: "build".to_string(),
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            serialized: false,
        };

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
    async fn resolve_listen_source_wave_id_rejects_self_target() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("test.db");
        let store = std::sync::Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );

        let parsed = ParsedStimulus {
            signal: Signal::Listen,
            flow: None,
            cron: None,
            source: Some("designer".to_string()),
            source_repo: None,
        };
        let err = resolve_listen_source_wave_id(&store, "/tmp/repo", "designer", Some(&parsed))
            .await
            .expect_err("self target should be rejected");
        assert_eq!(err.0, StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn resolve_listen_source_wave_id_resolves_cross_repo_source() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("test.db");
        let store = std::sync::Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("store should open"),
        );

        let source_wave = Wave {
            id: LfdId::new(),
            name: "infra".to_string(),
            repo: "/tmp/source-repo".to_string(),
            flow: "build".to_string(),
            direction: Vec::new(),
            area: Vec::new(),
            status: WaveStatus::Idle,
            iteration: 0,
            created_at: Some(OffsetDateTime::now_utc()),
            serialized: false,
        };
        store
            .create_wave(&source_wave)
            .await
            .expect("create source wave");

        let parsed = ParsedStimulus {
            signal: Signal::Listen,
            flow: None,
            cron: None,
            source: Some("infra".to_string()),
            source_repo: Some("/tmp/source-repo".to_string()),
        };
        let resolved =
            resolve_listen_source_wave_id(&store, "/tmp/listener-repo", "designer", Some(&parsed))
                .await
                .expect("source should resolve");
        assert_eq!(resolved, Some(source_wave.id().clone()));
    }
}
