use std::net::SocketAddr;
use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use tokio::time::{interval, Duration};
use tokio_stream::wrappers::BroadcastStream;

use crate::auth::AuthContext;
use crate::events::EventHub;
use crate::executor::WaveExecutor;
use crate::output::OutputHub;
use crate::registration::{RegistrationClient, RegistrationState};
use crate::scheduler::Scheduler;
use crate::store::{SharedStore, StoreError};
use crate::{id::LfdId, proto::control};

#[derive(Clone)]
pub struct HttpState {
    pub store: SharedStore,
    pub scheduler: Arc<Scheduler>,
    pub executor: Arc<WaveExecutor>,
    pub event_hub: EventHub,
    #[allow(dead_code)] // Reserved for output streaming endpoints.
    pub output_hub: OutputHub,
    pub auth: AuthContext,
    pub started_at: OffsetDateTime,
    pub registration: Option<RegistrationClient>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    uptime_seconds: i64,
    database: bool,
    waves_running: u32,
    agents_active: u32,
    registration: Option<RegistrationState>,
}

#[derive(Serialize)]
struct StatusResponse {
    pid: u32,
    waves_defined: u32,
    waves_running: u32,
    agents_active: u32,
    slots_used: u32,
    slots_total: u32,
    registration: Option<RegistrationState>,
}

#[derive(Serialize)]
struct MetricsResponse {
    waves_total: u32,
    waves_running: u32,
    agents_active: u32,
    slots_used: u32,
    slots_total: u32,
}

pub fn router(state: HttpState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/status", get(status_handler))
        .route("/metrics", get(metrics_handler))
        .route(
            "/api/waves",
            get(list_waves_handler).post(create_wave_handler),
        )
        .route(
            "/api/waves/:wave_id",
            get(get_wave_handler)
                .patch(update_wave_handler)
                .delete(delete_wave_handler),
        )
        .route("/api/waves/:wave_id/run", post(run_wave_handler))
        .route("/api/waves/:wave_id/stop", post(stop_wave_handler))
        .route("/api/waves/:wave_id/land", post(land_wave_handler))
        .route("/ws", get(ws_handler))
        .route("/hooks/git", post(git_hook_handler))
        .with_state(state)
}

async fn health_handler(State(state): State<HttpState>) -> Json<HealthResponse> {
    let counts = counts(&state).await;
    let registration = match &state.registration {
        Some(client) => Some(client.status().await),
        None => None,
    };
    Json(HealthResponse {
        status: if counts.database_ok { "ok" } else { "degraded" }.to_string(),
        uptime_seconds: (OffsetDateTime::now_utc() - state.started_at).whole_seconds(),
        database: counts.database_ok,
        waves_running: counts.waves_running,
        agents_active: counts.agents_active,
        registration,
    })
}

async fn status_handler(State(state): State<HttpState>) -> Json<StatusResponse> {
    let counts = counts(&state).await;
    let registration = match &state.registration {
        Some(client) => Some(client.status().await),
        None => None,
    };
    Json(StatusResponse {
        pid: std::process::id(),
        waves_defined: counts.waves_defined,
        waves_running: counts.waves_running,
        agents_active: counts.agents_active,
        slots_used: state.scheduler.slots_used(),
        slots_total: state.scheduler.max_slots() as u32,
        registration,
    })
}

async fn metrics_handler(State(state): State<HttpState>) -> Json<MetricsResponse> {
    let counts = counts(&state).await;
    Json(MetricsResponse {
        waves_total: counts.waves_defined,
        waves_running: counts.waves_running,
        agents_active: counts.agents_active,
        slots_used: state.scheduler.slots_used(),
        slots_total: state.scheduler.max_slots() as u32,
    })
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

type ApiResult<T> = Result<Json<T>, (StatusCode, Json<ErrorResponse>)>;

#[derive(Deserialize)]
struct ListWavesQuery {
    repo: Option<String>,
}

#[derive(Deserialize)]
struct CreateWaveRequest {
    repo: String,
    name: Option<String>,
    flow: Option<String>,
    direction: Option<Vec<String>>,
    area: Option<Vec<String>>,
}

#[derive(Deserialize, Default)]
struct UpdateWaveRequest {
    flow: Option<String>,
    direction: Option<Vec<String>>,
    area: Option<Vec<String>>,
    paused: Option<bool>,
}

#[derive(Deserialize, Default)]
struct RunWaveRequest {
    area: Option<Vec<String>>,
    direction: Option<Vec<String>>,
    flow: Option<String>,
}

#[derive(Deserialize, Default)]
struct LandWaveRequest {
    strict: Option<bool>,
    local: Option<bool>,
    create_pr: Option<bool>,
    worktree: Option<String>,
    lint: Option<bool>,
}

#[derive(Deserialize)]
struct GitHookRequest {
    hook: String,
    repo: String,
    branch: Option<String>,
}

#[derive(Serialize)]
struct WaveSummary {
    id: String,
    name: String,
    repo: String,
    flow: String,
    direction: Vec<String>,
    area: Vec<String>,
    paused: bool,
    created_at: Option<String>,
}

#[derive(Serialize)]
struct WaveRunSummary {
    id: String,
    wave_id: String,
    iteration: u32,
    step_index: u32,
    status: String,
    worktree: String,
    branch: String,
    started_at: Option<String>,
    ended_at: Option<String>,
    error: Option<String>,
    flow_parents: Vec<String>,
}

#[derive(Serialize)]
struct WaveView {
    wave: WaveSummary,
    status: String,
    active_run: Option<WaveRunSummary>,
}

#[derive(Serialize)]
struct ListWavesResponse {
    waves: Vec<WaveView>,
}

#[derive(Serialize)]
struct WaveResponse {
    wave: WaveView,
}

#[derive(Serialize)]
struct RunWaveResponse {
    started: bool,
    wave_id: String,
    wave_run_id: Option<String>,
}

#[derive(Serialize)]
struct StopWaveResponse {
    stopped: bool,
}

#[derive(Serialize)]
struct LandWaveResponse {
    merged: bool,
}

#[derive(Deserialize)]
struct WsQuery {
    token: Option<String>,
}

async fn list_waves_handler(
    State(state): State<HttpState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(query): Query<ListWavesQuery>,
) -> ApiResult<ListWavesResponse> {
    ensure_auth(&state, &headers, &peer, None).await?;
    let waves = run_store(&state.store, move |store| {
        store.list_waves(query.repo.as_deref())
    })
    .await
    .map_err(map_store_error)?;
    let views = build_wave_views(&state.store, waves)
        .await
        .map_err(map_store_error)?;
    Ok(Json(ListWavesResponse { waves: views }))
}

async fn create_wave_handler(
    State(state): State<HttpState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(payload): Json<CreateWaveRequest>,
) -> ApiResult<WaveResponse> {
    ensure_auth(&state, &headers, &peer, None).await?;
    let id = LfdId::new().to_string();
    let name = payload.name.unwrap_or_else(|| format!("wave-{id}"));
    let flow = payload.flow.unwrap_or_else(|| "ship".to_string());
    let wave = control::Wave {
        id,
        name,
        repo: payload.repo,
        flow,
        direction: payload.direction.unwrap_or_default(),
        area: payload.area.unwrap_or_default(),
        paused: false,
        created_at: Some(now_timestamp()),
    };
    let wave_clone = wave.clone();
    run_store(&state.store, move |store| store.create_wave(&wave_clone))
        .await
        .map_err(map_store_error)?;

    emit_event(
        &state.event_hub,
        "wave.created",
        control::event::Payload::WaveCreated(control::WaveCreatedEvent {
            wave_id: wave.id.clone(),
            name: wave.name.clone(),
        }),
    );

    let view = build_wave_view(&state.store, wave)
        .await
        .map_err(map_store_error)?;
    Ok(Json(WaveResponse { wave: view }))
}

async fn get_wave_handler(
    State(state): State<HttpState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(wave_id): Path<String>,
) -> ApiResult<WaveResponse> {
    ensure_auth(&state, &headers, &peer, None).await?;
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

async fn update_wave_handler(
    State(state): State<HttpState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(wave_id): Path<String>,
    Json(payload): Json<UpdateWaveRequest>,
) -> ApiResult<WaveResponse> {
    ensure_auth(&state, &headers, &peer, None).await?;
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

    emit_event(
        &state.event_hub,
        "wave.updated",
        control::event::Payload::WaveUpdated(control::WaveUpdatedEvent {
            wave_id: wave.id.clone(),
        }),
    );

    let view = build_wave_view(&state.store, wave)
        .await
        .map_err(map_store_error)?;
    Ok(Json(WaveResponse { wave: view }))
}

async fn delete_wave_handler(
    State(state): State<HttpState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(wave_id): Path<String>,
) -> ApiResult<serde_json::Value> {
    ensure_auth(&state, &headers, &peer, None).await?;
    let wave_id = parse_id(&wave_id)?;
    let wave_id_for_delete = wave_id.clone();
    run_store(&state.store, move |store| {
        store.delete_wave(&wave_id_for_delete)
    })
    .await
    .map_err(map_store_error)?;

    emit_event(
        &state.event_hub,
        "wave.deleted",
        control::event::Payload::WaveDeleted(control::WaveDeletedEvent {
            wave_id: wave_id.to_string(),
        }),
    );

    Ok(Json(serde_json::json!({ "deleted": true })))
}

async fn run_wave_handler(
    State(state): State<HttpState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(wave_id): Path<String>,
    payload: Option<Json<RunWaveRequest>>,
) -> ApiResult<RunWaveResponse> {
    ensure_auth(&state, &headers, &peer, None).await?;
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
            wave_id: wave.id,
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
        let run_id = LfdId::parse(&run_id_for_release)
            .unwrap_or_else(|_| LfdId::from_raw(run_id_for_release.clone()));
        if let Err(err) = exec.execute(&run_id).await {
            tracing::error!(run_id = %run_id_for_release, error = %err, "run execution failed");
            if let Ok(Some(mut run)) = store.get_wave_run(&run_id) {
                run.status = control::WaveRunStatus::WaveRunFailed as i32;
                run.error = Some(err.to_string());
                run.ended_at = Some(now_timestamp());
                let _ = store.update_wave_run(&run);
            }
        }
        scheduler.release(&run_id_for_release);
    });

    emit_event(
        &state.event_hub,
        "wave.started",
        control::event::Payload::WaveStarted(control::WaveStartedEvent {
            wave_id: wave.id.clone(),
        }),
    );

    Ok(Json(RunWaveResponse {
        started: true,
        wave_id: wave.id,
        wave_run_id: Some(run.id),
    }))
}

async fn stop_wave_handler(
    State(state): State<HttpState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(wave_id): Path<String>,
) -> ApiResult<StopWaveResponse> {
    ensure_auth(&state, &headers, &peer, None).await?;
    let wave_id = parse_id(&wave_id)?;
    let run = run_store(&state.store, {
        let wave_id = wave_id.clone();
        move |store| store.get_active_wave_run(&wave_id)
    })
    .await
    .map_err(map_store_error)?;

    if let Some(mut run) = run {
        run.status = control::WaveRunStatus::WaveRunFailed as i32;
        run.error = Some("stopped".to_string());
        run.ended_at = Some(now_timestamp());
        let run_clone = run.clone();
        run_store(&state.store, move |store| store.update_wave_run(&run_clone))
            .await
            .map_err(map_store_error)?;
    }

    emit_event(
        &state.event_hub,
        "wave.stopped",
        control::event::Payload::WaveStopped(control::WaveStoppedEvent {
            wave_id: wave_id.to_string(),
        }),
    );

    Ok(Json(StopWaveResponse { stopped: true }))
}

async fn land_wave_handler(
    State(state): State<HttpState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Path(wave_id): Path<String>,
    payload: Option<Json<LandWaveRequest>>,
) -> ApiResult<LandWaveResponse> {
    ensure_auth(&state, &headers, &peer, None).await?;
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

async fn git_hook_handler(
    State(state): State<HttpState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    Json(payload): Json<GitHookRequest>,
) -> ApiResult<serde_json::Value> {
    if !peer.ip().is_loopback() {
        return Err(api_error(StatusCode::FORBIDDEN, "hooks require localhost"));
    }

    let reason = match payload.hook.as_str() {
        "post-commit" => control::WorktreeChangeReason::WorktreeCommit,
        "post-checkout" => control::WorktreeChangeReason::WorktreeCheckout,
        "post-merge" => control::WorktreeChangeReason::WorktreeMerged,
        "post-rewrite" => control::WorktreeChangeReason::WorktreeChanged,
        _ => control::WorktreeChangeReason::Unspecified,
    };

    let event = control::WorktreeUpdatedEvent {
        branch: payload.branch.unwrap_or_default(),
        reason: reason as i32,
        repo: payload.repo.clone(),
        worktree: None,
        changes: None,
    };

    emit_event(
        &state.event_hub,
        "worktree.updated",
        control::event::Payload::WorktreeUpdated(event),
    );

    Ok(Json(serde_json::json!({ "ok": true })))
}

async fn ws_handler(
    State(state): State<HttpState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(query): Query<WsQuery>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    ensure_auth(&state, &headers, &peer, query.token).await?;
    Ok(ws.on_upgrade(move |socket| handle_ws(socket, state)))
}

async fn handle_ws(mut socket: WebSocket, state: HttpState) {
    let connected = match current_snapshot(&state.store).await {
        Ok(snapshot) => snapshot,
        Err(err) => {
            let _ = socket
                .send(Message::Text(
                    serde_json::json!({
                        "type": "error",
                        "error": err,
                    })
                    .to_string(),
                ))
                .await;
            return;
        }
    };

    let _ = socket
        .send(Message::Text(
            serde_json::json!({
                "type": "connected",
                "timestamp": OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap_or_default(),
                "waves": connected,
            })
            .to_string(),
        ))
        .await;

    let (mut sender, mut receiver) = socket.split();
    let mut events = BroadcastStream::new(state.event_hub.subscribe());
    let mut ticker = interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let ping = serde_json::json!({ "type": "ping" }).to_string();
                if sender.send(Message::Text(ping)).await.is_err() {
                    break;
                }
            }
            maybe_event = events.next() => {
                let Some(event) = maybe_event else { break };
                if let Ok(event) = event {
                    let payload = event_to_json(&event);
                    if sender.send(Message::Text(payload.to_string())).await.is_err() {
                        break;
                    }
                }
            }
            message = receiver.next() => {
                match message {
                    Some(Ok(Message::Text(_))) => {}
                    Some(Ok(Message::Pong(_))) => {}
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) | None => break,
                }
            }
        }
    }
}

fn event_to_json(event: &control::Event) -> serde_json::Value {
    let timestamp = format_timestamp(event.timestamp.as_ref());
    let payload = event.payload.as_ref().map(payload_to_json);
    let mut root = serde_json::Map::new();
    root.insert(
        "type".to_string(),
        serde_json::Value::String(event.event.clone()),
    );
    if let Some(timestamp) = timestamp {
        root.insert(
            "timestamp".to_string(),
            serde_json::Value::String(timestamp),
        );
    }
    if let Some(payload) = payload {
        root.insert("data".to_string(), payload);
    }
    serde_json::Value::Object(root)
}

fn payload_to_json(payload: &control::event::Payload) -> serde_json::Value {
    match payload {
        control::event::Payload::WaveCreated(event) => {
            serde_json::json!({ "wave_id": event.wave_id, "name": event.name })
        }
        control::event::Payload::WaveUpdated(event) => {
            serde_json::json!({ "wave_id": event.wave_id })
        }
        control::event::Payload::WaveDeleted(event) => {
            serde_json::json!({ "wave_id": event.wave_id })
        }
        control::event::Payload::WaveStarted(event) => {
            serde_json::json!({ "wave_id": event.wave_id })
        }
        control::event::Payload::WaveStopped(event) => {
            serde_json::json!({ "wave_id": event.wave_id })
        }
        control::event::Payload::WaveWaiting(event) => {
            serde_json::json!({
                "wave_id": event.wave_id,
                "step": event.step,
                "agent_id": event.agent_id,
                "wave_run_id": event.wave_run_id,
                "step_index": event.step_index,
                "reason": control::WaitingReason::try_from(event.reason)
                    .ok()
                    .map(|reason| reason.as_str_name().to_string()),
                "flow_parents": event.flow_parents,
            })
        }
        control::event::Payload::WorktreeUpdated(event) => {
            serde_json::json!({
                "branch": event.branch,
                "reason": control::WorktreeChangeReason::try_from(event.reason)
                    .ok()
                    .map(|reason| reason.as_str_name().to_string()),
                "repo": event.repo,
            })
        }
        control::event::Payload::WorktreePruned(event) => {
            serde_json::json!({ "branch": event.branch, "repo": event.repo })
        }
        control::event::Payload::AgentStarted(event) => {
            serde_json::json!({
                "id": event.id,
                "step": event.step,
                "worktree": event.worktree,
                "wave_id": event.wave_id,
                "wave_run_id": event.wave_run_id,
                "step_index": event.step_index,
            })
        }
        control::event::Payload::AgentEnded(event) => {
            serde_json::json!({
                "id": event.id,
                "status": event.status,
                "wave_id": event.wave_id,
                "wave_run_id": event.wave_run_id,
                "flow_will_continue": event.flow_will_continue,
            })
        }
        control::event::Payload::OutputLine(event) => {
            serde_json::json!({ "agent_id": event.agent_id, "text": event.text })
        }
        control::event::Payload::WaveActivated(event) => {
            serde_json::json!({
                "wave_id": event.wave_id,
                "triggering_stimuli": event.triggering_stimuli,
                "from_sha": event.from_sha,
                "to_sha": event.to_sha,
            })
        }
        control::event::Payload::SchedulerSlotAcquired(event) => {
            serde_json::json!({ "run_id": event.run_id, "slots_used": event.slots_used })
        }
        control::event::Payload::SchedulerSlotReleased(event) => {
            serde_json::json!({ "run_id": event.run_id, "slots_used": event.slots_used })
        }
    }
}

async fn ensure_auth(
    state: &HttpState,
    headers: &HeaderMap,
    peer: &SocketAddr,
    query_token: Option<String>,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    if peer.ip().is_loopback() {
        return Ok(());
    }

    let enabled = *state.auth.enabled.read().await;
    let registered = *state.auth.registered.read().await;
    if !enabled || !registered {
        return Err(api_error(
            StatusCode::FORBIDDEN,
            "remote access requires loopflow.studio registration",
        ));
    }

    let token = query_token
        .or_else(|| extract_token(headers))
        .ok_or_else(|| api_error(StatusCode::UNAUTHORIZED, "missing connection token"))?;

    let Some(validator) = &state.auth.validator else {
        return Ok(());
    };

    if validator.validate(&token).await {
        Ok(())
    } else {
        Err(api_error(
            StatusCode::UNAUTHORIZED,
            "invalid connection token",
        ))
    }
}

fn extract_token(headers: &HeaderMap) -> Option<String> {
    if let Some(value) = headers.get("x-loopflow-connection-token") {
        if let Ok(token) = value.to_str() {
            return Some(token.to_string());
        }
    }
    if let Some(value) = headers.get("connection-token") {
        if let Ok(token) = value.to_str() {
            return Some(token.to_string());
        }
    }
    if let Some(value) = headers.get("authorization") {
        if let Ok(auth) = value.to_str() {
            if let Some(token) = auth.strip_prefix("Bearer ") {
                return Some(token.trim().to_string());
            }
            if let Some(token) = auth.strip_prefix("bearer ") {
                return Some(token.trim().to_string());
            }
        }
    }
    None
}

fn api_error(status: StatusCode, message: impl Into<String>) -> (StatusCode, Json<ErrorResponse>) {
    (
        status,
        Json(ErrorResponse {
            error: message.into(),
        }),
    )
}

fn map_store_error(err: StoreError) -> (StatusCode, Json<ErrorResponse>) {
    match err {
        StoreError::NotFound => api_error(StatusCode::NOT_FOUND, "not found"),
        StoreError::InvalidData(message) => api_error(StatusCode::BAD_REQUEST, message),
        StoreError::Serde(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        StoreError::Sqlite(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        StoreError::Postgres(err) => api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        StoreError::PostgresPool(err) => {
            api_error(StatusCode::INTERNAL_SERVER_ERROR, err.to_string())
        }
    }
}

fn parse_id(value: &str) -> Result<LfdId, (StatusCode, Json<ErrorResponse>)> {
    value
        .parse::<LfdId>()
        .map_err(|err| api_error(StatusCode::BAD_REQUEST, err.to_string()))
}

async fn run_store<T, F>(store: &SharedStore, f: F) -> Result<T, StoreError>
where
    F: FnOnce(&dyn crate::store::RunStore) -> Result<T, StoreError> + Send + 'static,
    T: Send + 'static,
{
    let store = store.clone();
    tokio::task::spawn_blocking(move || f(store.as_ref()))
        .await
        .map_err(|err| StoreError::InvalidData(err.to_string()))?
}

async fn build_wave_views(
    store: &SharedStore,
    waves: Vec<control::Wave>,
) -> Result<Vec<WaveView>, StoreError> {
    let mut views = Vec::with_capacity(waves.len());
    for wave in waves {
        views.push(build_wave_view(store, wave).await?);
    }
    Ok(views)
}

async fn build_wave_view(store: &SharedStore, wave: control::Wave) -> Result<WaveView, StoreError> {
    let wave_id = LfdId::from_raw(wave.id.clone());
    let active = run_store(store, move |store| store.get_active_wave_run(&wave_id)).await?;

    let summary = WaveSummary {
        id: wave.id.clone(),
        name: wave.name.clone(),
        repo: wave.repo.clone(),
        flow: wave.flow.clone(),
        direction: wave.direction.clone(),
        area: wave.area.clone(),
        paused: wave.paused,
        created_at: format_timestamp(wave.created_at.as_ref()),
    };

    let active_run = active.map(wave_run_summary);
    let status = active_run
        .as_ref()
        .map(|run| run.status.clone())
        .unwrap_or_else(|| "idle".to_string());

    Ok(WaveView {
        wave: summary,
        status,
        active_run,
    })
}

fn wave_run_summary(run: control::WaveRun) -> WaveRunSummary {
    WaveRunSummary {
        id: run.id,
        wave_id: run.wave_id,
        iteration: run.iteration,
        step_index: run.step_index,
        status: wave_run_status(run.status),
        worktree: run.worktree,
        branch: run.branch,
        started_at: format_timestamp(run.started_at.as_ref()),
        ended_at: format_timestamp(run.ended_at.as_ref()),
        error: run.error,
        flow_parents: run.flow_parents,
    }
}

fn wave_run_status(status: i32) -> String {
    match control::WaveRunStatus::try_from(status).ok() {
        Some(control::WaveRunStatus::WaveRunPending) => "pending",
        Some(control::WaveRunStatus::WaveRunRunning) => "running",
        Some(control::WaveRunStatus::WaveRunWaiting) => "waiting",
        Some(control::WaveRunStatus::WaveRunCompleted) => "completed",
        Some(control::WaveRunStatus::WaveRunFailed) => "failed",
        _ => "unknown",
    }
    .to_string()
}

async fn current_snapshot(store: &SharedStore) -> Result<Vec<WaveView>, String> {
    let waves = run_store(store, move |store| store.list_waves(None))
        .await
        .map_err(|err| err.to_string())?;
    build_wave_views(store, waves)
        .await
        .map_err(|err| err.to_string())
}

fn format_timestamp(timestamp: Option<&prost_types::Timestamp>) -> Option<String> {
    let timestamp = timestamp?;
    let seconds = timestamp.seconds;
    let nanos = timestamp.nanos;
    let datetime = OffsetDateTime::from_unix_timestamp(seconds).ok()?;
    let datetime = datetime + time::Duration::nanoseconds(nanos as i64);
    datetime
        .format(&time::format_description::well_known::Rfc3339)
        .ok()
}

fn emit_event(event_hub: &EventHub, name: &str, payload: control::event::Payload) {
    let event = control::Event {
        event: name.to_string(),
        timestamp: Some(now_timestamp()),
        payload: Some(payload),
    };
    event_hub.send(event);
}

fn now_timestamp() -> prost_types::Timestamp {
    let now = OffsetDateTime::now_utc();
    prost_types::Timestamp {
        seconds: now.unix_timestamp(),
        nanos: 0,
    }
}

fn create_wave_run_with_id(
    store: &SharedStore,
    wave: &control::Wave,
    run_id: &LfdId,
) -> Result<control::WaveRun, StoreError> {
    let wave_id = LfdId::parse(&wave.id).map_err(|err| StoreError::InvalidData(err.to_string()))?;
    let last_run = store
        .list_wave_runs(Some(&wave_id), Some(1))?
        .into_iter()
        .next();
    let iteration = last_run.map(|run| run.iteration + 1).unwrap_or(0);

    let run = control::WaveRun {
        id: run_id.to_string(),
        wave_id: wave.id.clone(),
        iteration,
        step_index: 0,
        status: control::WaveRunStatus::WaveRunRunning as i32,
        worktree: wave.repo.clone(),
        branch: String::new(),
        started_at: Some(now_timestamp()),
        ended_at: None,
        error: None,
        flow_parents: Vec::new(),
    };
    store.create_wave_run(&run)?;
    Ok(run)
}

struct Counts {
    waves_defined: u32,
    waves_running: u32,
    agents_active: u32,
    database_ok: bool,
}

async fn counts(state: &HttpState) -> Counts {
    let store = state.store.clone();
    let waves = tokio::task::spawn_blocking(move || store.list_waves(None))
        .await
        .ok()
        .and_then(|result| result.ok())
        .unwrap_or_default();

    let store = state.store.clone();
    let agents = tokio::task::spawn_blocking(move || store.list_agents())
        .await
        .ok()
        .and_then(|result| result.ok())
        .unwrap_or_default();

    let store = state.store.clone();
    let wave_runs = tokio::task::spawn_blocking(move || store.list_wave_runs(None, None))
        .await
        .ok()
        .and_then(|result| result.ok())
        .unwrap_or_default();

    let store = state.store.clone();
    let database_ok = tokio::task::spawn_blocking(move || store.health_check())
        .await
        .ok()
        .and_then(|result| result.ok())
        .is_some();

    let waves_defined = waves.len() as u32;
    let waves_running = wave_runs
        .iter()
        .filter(|run| {
            run.status == crate::proto::control::WaveRunStatus::WaveRunRunning as i32
                || run.status == crate::proto::control::WaveRunStatus::WaveRunWaiting as i32
                || run.status == crate::proto::control::WaveRunStatus::WaveRunPending as i32
        })
        .count() as u32;
    let agents_active = agents
        .iter()
        .filter(|run| {
            run.status == crate::proto::control::AgentStatus::AgentRunning as i32
                || run.status == crate::proto::control::AgentStatus::AgentWaiting as i32
        })
        .count() as u32;

    Counts {
        waves_defined,
        waves_running,
        agents_active,
        database_ok,
    }
}
