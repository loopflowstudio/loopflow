use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::Json;
use futures_util::StreamExt;
use serde::Deserialize;
use std::convert::Infallible;
use std::path::{Path as FsPath, PathBuf};
use std::str::FromStr;
use std::time::Duration;
use time::OffsetDateTime;
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};
use tracing::warn;

use crate::agent::{tools, turn};
use crate::chat::{validate_turn_completion, AgentEvent, CompletionError, ContextSnapshot};
use crate::engine::git::{branch_rename, current_branch, delete_remote_branch, push_with_upstream};
use crate::engine::naming::sanitize_for_branch;
use crate::engine::platform::kill_process;
use crate::engine::worktree::remove_worktree;
use crate::engine::worktrees::{branch_exists, worktree_path};
use crate::lfd::config::ExecutorType;
use crate::lfd::executor::{create_wave_run_with_id, ensure_wave_worktree};
use crate::lfd::http::dto::{
    chat_memory_block_dto, stimulus_dto, stimulus_kind_str, ChatMemoryBlockDto, ChatTurnDto,
    CombineResponse, CombineResponseResult, ContinueWaveResponse, DeletedResourceResponse,
    ErrorResponse, LandWaveResponse, ListResponse, NextWaveResponse, RestartStepResponse,
    RunWaveResponse, StopWaveResponse, WaveDto,
};
use crate::lfd::http::routes::wave_schemas::{resolve_wave_schema, StimulusDef};
use crate::lfd::http::routes::{build_wave_dto, hooks, resolve_wave_id};
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::triggers::spawn_run_task_with_slot;
use crate::lfd::types::{
    AgentStatus, ChatMemoryBlock, Event, Stimulus, StimulusKind, Wave, WaveRun, WaveRunStatus,
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
    schema: Option<String>,
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
}

#[derive(Debug, Deserialize)]
pub struct UpsertMemoryBlockRequest {
    content: String,
    position: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct CreateChatTurnRequest {
    message: String,
    #[serde(default)]
    memory_blocks: Vec<ChatTurnMemoryBlockInput>,
}

#[derive(Debug, Deserialize)]
pub struct ChatTurnMemoryBlockInput {
    name: String,
    content: String,
    position: Option<u32>,
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
        |w| &w.id,
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
        name,
        flow,
        direction,
        area,
        schema,
    } = payload;
    let repo_path = PathBuf::from(&repo);
    let resolved_schema = if let Some(schema_input) = schema.as_deref() {
        Some(
            resolve_wave_schema(&repo_path, schema_input)
                .map_err(|err| api_error(err.status_code(), err.message()))?,
        )
    } else {
        None
    };
    let schema_stimulus = resolved_schema
        .as_ref()
        .and_then(|schema| schema.schema.stimulus.as_ref())
        .map(parse_schema_stimulus)
        .transpose()?;
    let schema_defaults = resolved_schema.as_ref().map(|resolved| &resolved.schema);

    let id = LfdId::new();
    let name = name
        .or_else(|| schema_defaults.map(|schema| schema.name.clone()))
        .unwrap_or_else(|| format!("wave-{}", id));
    let flow = flow
        .or_else(|| schema_defaults.map(|schema| schema.flow.clone()))
        .unwrap_or_else(|| "ship".to_string());
    let direction = direction
        .or_else(|| schema_defaults.and_then(|schema| schema.direction.clone()))
        .unwrap_or_default();
    let area = area
        .or_else(|| schema_defaults.map(|schema| schema.area.clone()))
        .unwrap_or_default();
    let schema_ref = resolved_schema
        .as_ref()
        .map(|schema| schema.schema_ref.clone());
    let schema_name = resolved_schema
        .as_ref()
        .map(|schema| schema.schema.name.clone());

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
        schema_ref,
        schema_name,
        created_at: Some(OffsetDateTime::now_utc()),
    };
    state
        .store
        .create_wave(&wave)
        .await
        .map_err(map_store_error)?;

    if state.executor.executor_type() == ExecutorType::Local {
        // Create worktree eagerly so it exists immediately for local execution.
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
            let _ = state.store.delete_wave(&wave_id).await;
            return Err(api_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to create worktree: {err}"),
            ));
        }
    }

    if let Some((kind, cron)) = schema_stimulus {
        let stimulus = Stimulus {
            id: LfdId::new(),
            wave_id: wave.id.clone(),
            kind,
            cron,
            last_main_sha: None,
            last_triggered_at: None,
            created_at: Some(OffsetDateTime::now_utc()),
            enabled: true,
        };
        if let Err(err) = state.store.create_stimulus(&stimulus).await {
            let wave_id = wave.id.clone();
            let _ = state.store.delete_wave(&wave_id).await;
            return Err(map_store_error(err));
        }
    }

    state
        .event_hub
        .send(Event::wave_created(wave.id.clone(), wave.name.clone()));

    let view = build_wave_dto(&state.store, &state.github, wave, false)
        .await
        .map_err(map_store_error)?;
    Ok(Json(view))
}

fn parse_schema_stimulus(
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
                format!("invalid schema stimulus kind '{value}'"),
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
                    "schema stimulus kind 'cron' requires a cron expression",
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
    Ok(waves.into_iter().any(|wave| wave.name == name))
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
        if !name.is_empty() && *name != wave.name {
            let new_name = name.clone();

            // Check for duplicate name in same repo.
            let existing = wave_name_exists(&state, &wave.repo, &new_name)
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
                .get_active_wave_run(&wave.id)
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

    state
        .store
        .update_wave(&wave)
        .await
        .map_err(map_store_error)?;

    state.event_hub.send(Event::wave_updated(wave.id.clone()));

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
                wave_id = %wave.id,
                error = %err,
                "failed to remove docker-backed wave workspace"
            );
        }
    }

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

    let run = match create_wave_run_with_id(&state.store, &wave, &run_id).await {
        Ok(run) => run,
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

    state
        .store
        .create_stimulus(&stimulus)
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

pub async fn list_memory_blocks_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<ListResponse<ChatMemoryBlockDto>> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    let blocks = state
        .store
        .list_chat_memory_blocks(&wave_id)
        .await
        .map_err(map_store_error)?;

    let dtos = blocks.into_iter().map(chat_memory_block_dto).collect();
    Ok(Json(ListResponse::new(dtos, false)))
}

pub async fn upsert_memory_block_handler(
    State(state): State<HttpState>,
    Path((wave_id, name)): Path<(String, String)>,
    Json(payload): Json<UpsertMemoryBlockRequest>,
) -> ApiResult<ChatMemoryBlockDto> {
    let name = normalized_memory_block_name(name)?;
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let content = payload.content;

    let position = match payload.position {
        Some(position) => position,
        None => {
            let existing = state
                .store
                .list_chat_memory_blocks(&wave_id)
                .await
                .map_err(map_store_error)?;

            default_memory_block_position(&existing, &name)
        }
    };

    let block = ChatMemoryBlock {
        wave_id: wave_id.clone(),
        name: name.clone(),
        content,
        position,
        updated_at: Some(OffsetDateTime::now_utc()),
    };

    state
        .store
        .upsert_chat_memory_block(&block)
        .await
        .map_err(map_store_error)?;

    Ok(Json(chat_memory_block_dto(block)))
}

pub async fn delete_memory_block_handler(
    State(state): State<HttpState>,
    Path((wave_id, name)): Path<(String, String)>,
) -> ApiResult<DeletedResourceResponse> {
    let name = normalized_memory_block_name(name)?;
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    state
        .store
        .delete_chat_memory_block(&wave_id, &name)
        .await
        .map_err(map_store_error)?;

    Ok(Json(DeletedResourceResponse {
        id: name,
        object: "memory_block".to_string(),
        deleted: true,
    }))
}

pub async fn create_chat_turn_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Json(payload): Json<CreateChatTurnRequest>,
) -> ApiResult<ChatTurnDto> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let message = payload.message.trim().to_string();
    if message.is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "message cannot be empty",
        ));
    }

    run_store(&state.store, {
        let wave_id = wave_id.clone();
        move |store| store.get_wave(&wave_id)
    })
    .await
    .map_err(map_store_error)?
    .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    let memory_blocks = if payload.memory_blocks.is_empty() {
        run_store(&state.store, {
            let wave_id = wave_id.clone();
            move |store| store.list_chat_memory_blocks(&wave_id)
        })
        .await
        .map_err(map_store_error)?
        .into_iter()
        .map(|block| PromptMemoryBlock {
            name: block.name,
            content: block.content,
            position: block.position,
        })
        .collect::<Vec<_>>()
    } else {
        prompt_memory_blocks_from_request(payload.memory_blocks)?
    };

    let system_prompt = build_chat_system_prompt(&memory_blocks);
    let (turn_id, turn_stream) = state.chat_turns.create(wave_id.clone());

    tokio::spawn(async move {
        let registry = tools::default_registry();
        let config = turn::TurnConfig {
            system: Some(system_prompt),
            ..Default::default()
        };

        let run_result = turn::run_with_event_handler(&message, &config, &registry, |event| {
            turn_stream.publish(event.clone());
        })
        .await;

        match run_result {
            Ok(result) => {
                let completion = completion_event_from_result(&result);
                turn_stream.publish(completion);
            }
            Err(err) => {
                turn_stream.publish(AgentEvent::Failed {
                    code: "turn_failed".to_string(),
                    message: err.to_string(),
                });
            }
        }
        turn_stream.mark_completed();
    });

    Ok(Json(ChatTurnDto {
        id: turn_id.to_string(),
        object: "chat_turn".to_string(),
        wave_id: wave_id.to_string(),
        status: "running".to_string(),
    }))
}

pub async fn stream_chat_turn_events_handler(
    State(state): State<HttpState>,
    Path((wave_id, turn_id)): Path<(String, String)>,
) -> Result<Sse<ReceiverStream<Result<SseEvent, Infallible>>>, ApiError> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let turn_id = LfdId::from_str(&turn_id)
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid chat turn id"))?;

    let turn_stream = state
        .chat_turns
        .get(&wave_id, &turn_id)
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "chat turn not found"))?;

    let history = turn_stream.history();
    let live_rx = turn_stream.subscribe();
    let completed = turn_stream.is_completed();

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<SseEvent, Infallible>>(256);
    tokio::spawn(async move {
        for event in history {
            let terminal = is_terminal_agent_event(&event);
            if tx.send(Ok(agent_event_sse(&event))).await.is_err() {
                return;
            }
            if terminal {
                return;
            }
        }

        if completed {
            return;
        }

        let mut stream = BroadcastStream::new(live_rx);
        while let Some(next) = stream.next().await {
            let Ok(event) = next else {
                continue;
            };
            let terminal = is_terminal_agent_event(&event);
            if tx.send(Ok(agent_event_sse(&event))).await.is_err() {
                break;
            }
            if terminal {
                break;
            }
        }
    });

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

fn normalized_memory_block_name(name: String) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    let trimmed = name.trim().to_string();
    if trimmed.is_empty() {
        Err(api_error(
            StatusCode::BAD_REQUEST,
            "memory block name cannot be empty",
        ))
    } else {
        Ok(trimmed)
    }
}

fn default_memory_block_position(existing: &[ChatMemoryBlock], name: &str) -> u32 {
    if let Some(block) = existing.iter().find(|block| block.name == name) {
        return block.position;
    }

    existing
        .iter()
        .map(|block| block.position)
        .max()
        .map(|max_position| max_position.saturating_add(1))
        .unwrap_or(0)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PromptMemoryBlock {
    name: String,
    content: String,
    position: u32,
}

fn prompt_memory_blocks_from_request(
    raw_blocks: Vec<ChatTurnMemoryBlockInput>,
) -> Result<Vec<PromptMemoryBlock>, ApiError> {
    let mut blocks = Vec::with_capacity(raw_blocks.len());
    for (index, block) in raw_blocks.into_iter().enumerate() {
        let name = normalized_memory_block_name(block.name)?;
        blocks.push(PromptMemoryBlock {
            name,
            content: block.content,
            position: block.position.unwrap_or(index as u32),
        });
    }
    blocks.sort_by(|left, right| {
        left.position
            .cmp(&right.position)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(blocks)
}

fn build_chat_system_prompt(memory_blocks: &[PromptMemoryBlock]) -> String {
    let mut lines = vec![
        "You are the Loopflow chat assistant.".to_string(),
        "All user-visible output must be sent with the send_message tool.".to_string(),
        "Use send_message phase=\"progress\" for intermediate updates.".to_string(),
        "End successful turns with exactly one send_message phase=\"final\".".to_string(),
        "Use memory_edit when durable memory should change.".to_string(),
    ];
    if !memory_blocks.is_empty() {
        lines.push("<memory>".to_string());
        for block in memory_blocks {
            lines.push(format!("<block name=\"{}\">", xml_escape(&block.name)));
            lines.push(xml_escape(&block.content));
            lines.push("</block>".to_string());
        }
        lines.push("</memory>".to_string());
    }
    lines.join("\n")
}

fn xml_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn completion_event_from_result(result: &turn::TurnResult) -> AgentEvent {
    match validate_turn_completion(&result.events) {
        Ok(()) => AgentEvent::Done {
            context: ContextSnapshot {
                memory_tokens: 0,
                history_tokens: result.input_tokens,
                total_tokens: result.input_tokens.saturating_add(result.output_tokens),
            },
        },
        Err(err) => AgentEvent::Failed {
            code: completion_error_code(&err).to_string(),
            message: err.to_string(),
        },
    }
}

fn completion_error_code(error: &CompletionError) -> &'static str {
    match error {
        CompletionError::MissingFinalMessage => "missing_final_message",
        CompletionError::MultipleFinalMessages => "multiple_final_messages",
        CompletionError::FinalMessageOnFailedTurn => "final_message_on_failed_turn",
    }
}

fn is_terminal_agent_event(event: &AgentEvent) -> bool {
    matches!(event, AgentEvent::Done { .. } | AgentEvent::Failed { .. })
}

fn agent_event_sse(event: &AgentEvent) -> SseEvent {
    let payload = serde_json::to_string(event).expect("AgentEvent should always serialize for SSE");
    SseEvent::default().event("agent_event").data(payload)
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
        run,
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
    tokio::task::spawn_blocking(move || {
        auto_commit_if_dirty(std::path::Path::new(&worktree), &step_name)
    })
    .await
    .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?
    .map_err(|err| api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()))?;

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
        .get_latest_wave_run(&wave.id)
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
    use crate::lfd::id::LfdId;

    #[test]
    fn parse_schema_stimulus_requires_cron_expression_for_cron_kind() {
        let stimulus = StimulusDef {
            kind: "cron".to_string(),
            cron: None,
        };

        let result = parse_schema_stimulus(&stimulus);
        assert!(result.is_err());
    }

    #[test]
    fn parse_schema_stimulus_accepts_valid_cron_expression() {
        let stimulus = StimulusDef {
            kind: "cron".to_string(),
            cron: Some("0 8 * * *".to_string()),
        };

        let parsed = parse_schema_stimulus(&stimulus).expect("parse stimulus");
        assert_eq!(parsed.0, StimulusKind::Cron);
        assert_eq!(parsed.1, "0 8 * * *");
    }

    #[test]
    fn normalized_memory_block_name_rejects_whitespace() {
        let result = normalized_memory_block_name("   ".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn normalized_memory_block_name_trims_surrounding_whitespace() {
        let result = normalized_memory_block_name("  project-context  ".to_string())
            .expect("name should normalize");
        assert_eq!(result, "project-context");
    }

    #[test]
    fn default_memory_block_position_keeps_existing_position() {
        let existing = vec![ChatMemoryBlock {
            wave_id: LfdId::from_raw("wave-1"),
            name: "project-context".to_string(),
            content: "repo context".to_string(),
            position: 4,
            updated_at: None,
        }];

        let position = default_memory_block_position(&existing, "project-context");
        assert_eq!(position, 4);
    }

    #[test]
    fn default_memory_block_position_appends_after_highest_position() {
        let existing = vec![
            ChatMemoryBlock {
                wave_id: LfdId::from_raw("wave-1"),
                name: "first".to_string(),
                content: "a".to_string(),
                position: 0,
                updated_at: None,
            },
            ChatMemoryBlock {
                wave_id: LfdId::from_raw("wave-1"),
                name: "second".to_string(),
                content: "b".to_string(),
                position: 3,
                updated_at: None,
            },
        ];

        let position = default_memory_block_position(&existing, "third");
        assert_eq!(position, 4);
    }

    #[test]
    fn prompt_memory_blocks_from_request_normalizes_order() {
        let blocks = vec![
            ChatTurnMemoryBlockInput {
                name: "second".to_string(),
                content: "B".to_string(),
                position: Some(2),
            },
            ChatTurnMemoryBlockInput {
                name: " first ".to_string(),
                content: "A".to_string(),
                position: Some(1),
            },
        ];

        let normalized = prompt_memory_blocks_from_request(blocks).expect("normalize blocks");
        assert_eq!(normalized[0].name, "first");
        assert_eq!(normalized[1].name, "second");
    }

    #[test]
    fn build_chat_system_prompt_embeds_memory_xml() {
        let prompt = build_chat_system_prompt(&[PromptMemoryBlock {
            name: "prefs".to_string(),
            content: "Use <short> answers & bullets".to_string(),
            position: 0,
        }]);

        assert!(prompt.contains("send_message"));
        assert!(prompt.contains("<memory>"));
        assert!(prompt.contains("&lt;short&gt; answers &amp; bullets"));
    }

    #[test]
    fn completion_event_from_result_reports_missing_final_message() {
        let result = turn::TurnResult {
            response: "assistant text".to_string(),
            iterations: 1,
            input_tokens: 10,
            output_tokens: 5,
            events: vec![AgentEvent::Message {
                content: "working".to_string(),
                phase: crate::chat::UserMessagePhase::Progress,
            }],
        };

        let event = completion_event_from_result(&result);
        assert!(matches!(
            event,
            AgentEvent::Failed { code, .. } if code == "missing_final_message"
        ));
    }
}
