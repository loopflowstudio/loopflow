use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event as SseEvent, KeepAlive, KeepAliveStream, Sse};
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::ReceiverStream;

use crate::lfd::http::dto::{format_datetime, ErrorResponse};
use crate::lfd::http::routes::ApiError;
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiMessage, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::sessions::types::{
    CreateSessionParams, PersistedSessionEvent, Session, SessionConfig,
};
use crate::lfd::sessions::SessionManagerError;

#[derive(Debug, Deserialize)]
pub struct CreateSessionRequest {
    pub harness: String,
    #[serde(default)]
    pub wave_run_id: Option<String>,
    #[serde(flatten)]
    pub config: SessionConfig,
}

#[derive(Debug, Deserialize)]
pub struct SessionInputRequest {
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub struct SessionEventsQuery {
    pub after_seq: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct SessionDto {
    pub id: String,
    pub object: String,
    pub harness: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wave_run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_session_id: Option<String>,
    pub config: SessionConfig,
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
}

pub async fn create_session_handler(
    State(state): State<HttpState>,
    Json(payload): Json<CreateSessionRequest>,
) -> ApiResult<SessionDto> {
    let session = state
        .sessions
        .create_session(CreateSessionParams {
            harness: payload.harness,
            wave_run_id: payload.wave_run_id,
            config: payload.config,
        })
        .await
        .map_err(map_session_error)?;
    Ok(Json(session_dto(session)))
}

pub async fn get_session_handler(
    State(state): State<HttpState>,
    Path(session_id): Path<String>,
) -> ApiResult<SessionDto> {
    let session_id = parse_session_id(&session_id)?;
    let session = state
        .sessions
        .get_session(&session_id)
        .await
        .map_err(map_session_error)?;
    Ok(Json(session_dto(session)))
}

pub async fn send_session_input_handler(
    State(state): State<HttpState>,
    Path(session_id): Path<String>,
    Json(payload): Json<SessionInputRequest>,
) -> ApiResult<SessionDto> {
    let session_id = parse_session_id(&session_id)?;
    if payload.content.trim().is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "content cannot be empty",
        ));
    }

    state
        .sessions
        .send_input(&session_id, &payload.content)
        .await
        .map_err(map_session_error)?;

    let session = state
        .sessions
        .get_session(&session_id)
        .await
        .map_err(map_session_error)?;
    Ok(Json(session_dto(session)))
}

pub async fn stream_session_events_handler(
    State(state): State<HttpState>,
    Path(session_id): Path<String>,
    Query(query): Query<SessionEventsQuery>,
) -> Result<Sse<KeepAliveStream<ReceiverStream<Result<SseEvent, Infallible>>>>, ApiError> {
    let session_id = parse_session_id(&session_id)?;
    let live_rx = state
        .sessions
        .subscribe(&session_id)
        .await
        .map_err(map_session_error)?;
    let replay = state
        .sessions
        .list_events(&session_id, query.after_seq)
        .await
        .map_err(map_session_error)?;

    let mut last_seq = query.after_seq.unwrap_or(-1);
    if let Some(last) = replay.last() {
        last_seq = last.seq;
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<SseEvent, Infallible>>(256);
    tokio::spawn(async move {
        for event in replay {
            if tx.send(Ok(session_event_sse(&event))).await.is_err() {
                return;
            }
        }

        let sentinel = SseEvent::default()
            .event("session.replay_completed")
            .data(serde_json::json!({ "last_seq": last_seq }).to_string());
        if tx.send(Ok(sentinel)).await.is_err() {
            return;
        }

        let Some(mut live_rx) = live_rx else {
            return;
        };

        loop {
            match live_rx.recv().await {
                Ok(event) => {
                    if event.seq <= last_seq {
                        continue;
                    }
                    last_seq = event.seq;
                    if tx.send(Ok(session_event_sse(&event))).await.is_err() {
                        return;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => return,
            }
        }
    });

    Ok(Sse::new(ReceiverStream::new(rx)).keep_alive(
        KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("keep-alive"),
    ))
}

pub async fn delete_session_handler(
    State(state): State<HttpState>,
    Path(session_id): Path<String>,
) -> ApiResult<SessionDto> {
    let session_id = parse_session_id(&session_id)?;
    let session = state
        .sessions
        .stop_session(&session_id)
        .await
        .map_err(map_session_error)?;
    Ok(Json(session_dto(session)))
}

fn session_dto(session: Session) -> SessionDto {
    SessionDto {
        id: session.id.to_string(),
        object: "session".to_string(),
        harness: session.harness,
        status: session.status.as_str().to_string(),
        wave_run_id: session.wave_run_id,
        provider_session_id: session.provider_session_id,
        config: session.config,
        created_at: format_datetime(Some(session.created_at)),
        ended_at: format_datetime(session.ended_at),
    }
}

fn session_event_sse(event: &PersistedSessionEvent) -> SseEvent {
    let data = serde_json::to_string(&event.event).unwrap_or_else(|err| {
        serde_json::json!({
            "type": "error",
            "code": "serialization_error",
            "message": err.to_string(),
        })
        .to_string()
    });

    SseEvent::default()
        .id(event.seq.to_string())
        .event("session.event")
        .data(data)
}

fn parse_session_id(value: &str) -> Result<LfdId, ApiError> {
    value
        .parse::<LfdId>()
        .map_err(|_| api_error(StatusCode::BAD_REQUEST, "invalid session id"))
}

fn map_session_error(err: SessionManagerError) -> (StatusCode, Json<ErrorResponse>) {
    match err {
        SessionManagerError::Store(err) => map_store_error(err),
        SessionManagerError::NotFound => api_error(StatusCode::NOT_FOUND, "session not found"),
        SessionManagerError::InvalidState { expected, actual } => api_error(
            StatusCode::CONFLICT,
            ApiMessage::Safe(format!(
                "invalid session state: expected {expected}, got {actual:?}"
            )),
        ),
        SessionManagerError::UnsupportedHarness(name) => api_error(
            StatusCode::BAD_REQUEST,
            ApiMessage::Safe(format!("unsupported harness: {name}")),
        ),
        SessionManagerError::HarnessNotImplemented(name) => api_error(
            StatusCode::NOT_IMPLEMENTED,
            ApiMessage::Safe(format!("harness not implemented yet: {name}")),
        ),
        SessionManagerError::WaveRunSessionConflict(wave_run_id) => api_error(
            StatusCode::CONFLICT,
            ApiMessage::Safe(format!(
                "wave run already has an active session: {wave_run_id}"
            )),
        ),
        SessionManagerError::InvalidConfig(message) => {
            api_error(StatusCode::BAD_REQUEST, ApiMessage::Safe(message))
        }
        SessionManagerError::InvalidRepoRoot(message) => api_error(
            StatusCode::BAD_REQUEST,
            ApiMessage::Safe(format!("invalid repo_root: {message}")),
        ),
        SessionManagerError::TurnAlreadyInProgress => {
            api_error(StatusCode::CONFLICT, "turn already in progress")
        }
        SessionManagerError::Harness(message) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiMessage::Untrusted(message),
        ),
    }
}
