use std::convert::Infallible;
use std::time::Duration;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::sse::{Event as SseEvent, KeepAlive, KeepAliveStream, Sse};
use axum::Json;
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::ReceiverStream;

use crate::lfd::conversations::harness::HarnessKind;
use crate::lfd::conversations::types::{
    Conversation, ConversationConfig, PersistedConversationEvent,
};
use crate::lfd::conversations::{ConversationManager, ConversationManagerError};
use crate::lfd::http::dto::{format_datetime, ErrorResponse};
use crate::lfd::http::routes::ApiError;
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{api_error, map_store_error, ApiMessage, ApiResult};
use crate::lfd::id::LfdId;

#[derive(Debug, Deserialize)]
pub struct ConversationInputRequest {
    pub text: String,
}

#[derive(Debug, Deserialize)]
pub struct ConversationEventsQuery {
    pub after_seq: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct ConversationDto {
    pub id: String,
    pub object: String,
    pub harness: String,
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_session_id: Option<String>,
    pub config: ConversationConfig,
    pub input_supported: bool,
    pub created_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<String>,
}

pub async fn send_conversation_input_handler(
    State(state): State<HttpState>,
    Path(conversation_id): Path<String>,
    Json(payload): Json<ConversationInputRequest>,
) -> ApiResult<ConversationDto> {
    let conversation_id = parse_conversation_id(&conversation_id)?;
    if payload.text.trim().is_empty() {
        return Err(api_error(StatusCode::BAD_REQUEST, "text cannot be empty"));
    }

    state
        .conversations
        .send_input(&conversation_id, &payload.text)
        .await
        .map_err(map_conversation_error)?;

    let conversation = state
        .conversations
        .get_conversation(&conversation_id)
        .await
        .map_err(map_conversation_error)?;
    Ok(Json(conversation_dto(conversation)))
}

pub async fn stream_conversation_events_handler(
    State(state): State<HttpState>,
    Path(conversation_id): Path<String>,
    Query(query): Query<ConversationEventsQuery>,
) -> Result<Sse<KeepAliveStream<ReceiverStream<Result<SseEvent, Infallible>>>>, ApiError> {
    let conversation_id = parse_conversation_id(&conversation_id)?;
    let live_rx = state
        .conversations
        .subscribe(&conversation_id)
        .await
        .map_err(map_conversation_error)?;
    let replay = state
        .conversations
        .list_events(&conversation_id, query.after_seq)
        .await
        .map_err(map_conversation_error)?;
    let conversations = state.conversations.clone();

    let mut last_seq = query.after_seq.unwrap_or(-1);
    if let Some(last) = replay.last() {
        last_seq = last.seq;
    }

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<SseEvent, Infallible>>(256);
    tokio::spawn(async move {
        for event in replay {
            if tx.send(Ok(conversation_event_sse(&event))).await.is_err() {
                return;
            }
        }

        let sentinel = SseEvent::default()
            .event("conversation.replay_completed")
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
                    if tx.send(Ok(conversation_event_sse(&event))).await.is_err() {
                        return;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                    if !backfill_lagged_events(&conversations, &conversation_id, &mut last_seq, &tx)
                        .await
                    {
                        return;
                    }
                }
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

fn conversation_dto(conversation: Conversation) -> ConversationDto {
    let input_supported = HarnessKind::parse(&conversation.harness)
        .map(HarnessKind::input_supported)
        .unwrap_or(false);
    ConversationDto {
        id: conversation.id.to_string(),
        object: "conversation".to_string(),
        harness: conversation.harness,
        status: conversation.status.as_str().to_string(),
        run_id: conversation.run_id,
        provider_session_id: conversation.provider_session_id,
        input_supported,
        config: conversation.config,
        created_at: format_datetime(Some(conversation.created_at)),
        ended_at: format_datetime(conversation.ended_at),
    }
}

fn conversation_event_sse(event: &PersistedConversationEvent) -> SseEvent {
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
        .event("conversation.event")
        .data(data)
}

async fn backfill_lagged_events(
    conversations: &ConversationManager,
    conversation_id: &LfdId,
    last_seq: &mut i64,
    tx: &tokio::sync::mpsc::Sender<Result<SseEvent, Infallible>>,
) -> bool {
    let missed = match conversations
        .list_events(conversation_id, Some(*last_seq))
        .await
    {
        Ok(events) => events,
        Err(err) => {
            tracing::warn!(
                conversation_id = %conversation_id,
                error = %err,
                "failed to backfill lagged conversation events"
            );
            return true;
        }
    };

    for event in missed {
        if event.seq <= *last_seq {
            continue;
        }
        *last_seq = event.seq;
        if tx.send(Ok(conversation_event_sse(&event))).await.is_err() {
            return false;
        }
    }

    true
}

fn parse_conversation_id(value: &str) -> Result<LfdId, ApiError> {
    super::parse_lfd_id(value, "invalid conversation id")
}

fn map_conversation_error(err: ConversationManagerError) -> (StatusCode, Json<ErrorResponse>) {
    match err {
        ConversationManagerError::Store(err) => map_store_error(err),
        ConversationManagerError::NotFound => {
            api_error(StatusCode::NOT_FOUND, "conversation not found")
        }
        ConversationManagerError::InvalidState { expected, actual } => api_error(
            StatusCode::CONFLICT,
            ApiMessage::Safe(format!(
                "invalid conversation state: expected {expected}, got {actual:?}"
            )),
        ),
        ConversationManagerError::UnsupportedHarness(name) => api_error(
            StatusCode::BAD_REQUEST,
            ApiMessage::Safe(format!("unsupported harness: {name}")),
        ),
        ConversationManagerError::HarnessNotImplemented(name) => api_error(
            StatusCode::NOT_IMPLEMENTED,
            ApiMessage::Safe(format!("harness not implemented yet: {name}")),
        ),
        ConversationManagerError::RunSessionConflict(run_id) => api_error(
            StatusCode::CONFLICT,
            ApiMessage::Safe(format!("run already has an active session: {run_id}")),
        ),
        ConversationManagerError::InvalidConfig(message) => {
            api_error(StatusCode::BAD_REQUEST, ApiMessage::Safe(message))
        }
        ConversationManagerError::InvalidRepoRoot(message) => api_error(
            StatusCode::BAD_REQUEST,
            ApiMessage::Safe(format!("invalid repo_root: {message}")),
        ),
        ConversationManagerError::TurnAlreadyInProgress => {
            api_error(StatusCode::CONFLICT, "turn already in progress")
        }
        ConversationManagerError::InputNotSupported(harness) => api_error(
            StatusCode::BAD_REQUEST,
            ApiMessage::Safe(format!("input not supported for this harness: {harness}")),
        ),
        ConversationManagerError::Harness(message) => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiMessage::Untrusted(message),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::conversations::types::ConversationStatus;
    use crate::lfd::id::LfdId;
    use crate::lfd::store::{open_store, StorageConfig};
    use tempfile::tempdir;

    #[tokio::test]
    async fn codex_conversation_dto_supports_input() {
        let conversation_id = LfdId::new();
        let session = Conversation {
            id: conversation_id.clone(),
            harness: "codex".to_string(),
            status: ConversationStatus::Active,
            run_id: None,
            provider_session_id: None,
            config: ConversationConfig {
                step: "design".to_string(),
                repo_root: "/tmp/repo".to_string(),
                ..Default::default()
            },
            created_at: time::OffsetDateTime::now_utc(),
            ended_at: None,
        };
        let dto = conversation_dto(session);

        assert!(dto.input_supported);
    }

    #[tokio::test]
    async fn send_conversation_input_rejects_unsupported_harness() {
        let state = crate::lfd::http::routes::test_helpers::test_http_state().await;
        let conversation_id = LfdId::new();
        let session = Conversation {
            id: conversation_id.clone(),
            harness: "claude".to_string(),
            status: ConversationStatus::Active,
            run_id: None,
            provider_session_id: None,
            config: ConversationConfig {
                step: "design".to_string(),
                repo_root: "/tmp/repo".to_string(),
                ..Default::default()
            },
            created_at: time::OffsetDateTime::now_utc(),
            ended_at: None,
        };
        state
            .store
            .create_conversation(&session)
            .await
            .expect("seed session");

        let result = send_conversation_input_handler(
            State(state),
            Path(conversation_id.to_string()),
            Json(ConversationInputRequest {
                text: "hello".to_string(),
            }),
        )
        .await;

        let Err((status, Json(error))) = result else {
            panic!("unsupported input should fail");
        };
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(error.error.message.contains("input not supported"));
    }

    #[tokio::test]
    async fn backfill_lagged_events_replays_from_store_after_last_seq() {
        let tmp = tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store = std::sync::Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );

        let conversation_id = LfdId::new();
        let session = Conversation {
            id: conversation_id.clone(),
            harness: "claude".to_string(),
            status: crate::lfd::conversations::types::ConversationStatus::Active,
            run_id: None,
            provider_session_id: None,
            config: ConversationConfig {
                step: "design".to_string(),
                repo_root: tmp.path().to_string_lossy().to_string(),
                ..Default::default()
            },
            created_at: time::OffsetDateTime::now_utc(),
            ended_at: None,
        };
        store
            .create_conversation(&session)
            .await
            .expect("create session");

        store
            .append_conversation_event(
                &conversation_id,
                0,
                &crate::lfd::conversations::types::ConversationEvent::StatusChanged {
                    status: crate::lfd::conversations::types::ConversationStatus::Active,
                },
                time::OffsetDateTime::now_utc().unix_timestamp(),
            )
            .await
            .expect("append status event");
        store
            .append_conversation_event(
                &conversation_id,
                1,
                &crate::lfd::conversations::types::ConversationEvent::TextDelta {
                    turn_id: "turn_1".to_string(),
                    content: "hello".to_string(),
                },
                time::OffsetDateTime::now_utc().unix_timestamp(),
            )
            .await
            .expect("append delta event");
        store
            .append_conversation_event(
                &conversation_id,
                2,
                &crate::lfd::conversations::types::ConversationEvent::TurnCompleted {
                    turn_id: "turn_1".to_string(),
                    status: crate::lfd::conversations::types::TurnStatus::Completed,
                },
                time::OffsetDateTime::now_utc().unix_timestamp(),
            )
            .await
            .expect("append completion event");

        let conversations = ConversationManager::new(store);
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let mut last_seq = 1;

        let keep_streaming =
            backfill_lagged_events(&conversations, &conversation_id, &mut last_seq, &tx).await;
        assert!(keep_streaming);
        assert_eq!(last_seq, 2);

        let first = rx.recv().await.expect("first backfilled event");
        assert!(first.is_ok());
        assert!(rx.try_recv().is_err());
    }
}
