use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::Json;
use futures_util::StreamExt;
use serde::Deserialize;
use std::convert::Infallible;
use std::time::Duration;
use time::OffsetDateTime;
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};

use crate::agent::{tools, turn};
use crate::chat::{validate_turn_completion, AgentEvent, CompletionError, ContextSnapshot};
use crate::lfd::http::dto::{
    chat_memory_block_dto, chat_message_dto, ChatMemoryBlockDto, ChatMessageDto, ChatStartedDto,
    DeletedResourceResponse, ListResponse,
};
use crate::lfd::http::routes::{resolve_wave_id, ApiError};
use crate::lfd::http::state::{ChatTurnStartError, HttpState};
use crate::lfd::http::{api_error, map_store_error, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::types::{ChatMemoryBlock, ChatMessage};

#[derive(Debug, Deserialize)]
pub struct UpsertMemoryBlockRequest {
    content: String,
    position: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct StartChatRequest {
    message: String,
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

pub async fn start_chat_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Json(payload): Json<StartChatRequest>,
) -> ApiResult<ChatStartedDto> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    let message = payload.message.trim().to_string();
    if message.is_empty() {
        return Err(api_error(
            StatusCode::BAD_REQUEST,
            "message cannot be empty",
        ));
    }

    state
        .store
        .get_wave(&wave_id)
        .await
        .map_err(map_store_error)?
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "wave not found"))?;

    let memory_blocks = load_sorted_memory_blocks(&state, &wave_id).await?;
    let system_prompt = build_chat_system_prompt(&memory_blocks);
    let turn_stream = state
        .chat_turns
        .start_for_wave(wave_id.clone())
        .map_err(map_chat_turn_start_error)?;

    let user_msg = ChatMessage {
        id: LfdId::new(),
        wave_id: wave_id.clone(),
        role: "user".to_string(),
        content: message.clone(),
        created_at: OffsetDateTime::now_utc(),
    };
    if let Err(err) = state.store.create_chat_message(&user_msg).await {
        turn_stream.mark_completed();
        return Err(map_store_error(err));
    }

    let store = state.store.clone();
    let spawn_wave_id = wave_id.clone();
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
                persist_turn_events(&store, &spawn_wave_id, &result.events).await;
                publish_terminal_event(
                    &store,
                    &spawn_wave_id,
                    &turn_stream,
                    completion_event_from_result(&result),
                )
                .await;
            }
            Err(err) => {
                publish_terminal_event(
                    &store,
                    &spawn_wave_id,
                    &turn_stream,
                    AgentEvent::Failed {
                        code: "turn_failed".to_string(),
                        message: err.to_string(),
                    },
                )
                .await;
            }
        }
        turn_stream.mark_completed();
    });

    Ok(Json(ChatStartedDto {
        object: "chat".to_string(),
        wave_id: wave_id.to_string(),
        status: "running".to_string(),
    }))
}

pub async fn stream_chat_events_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> Result<Sse<ReceiverStream<Result<SseEvent, Infallible>>>, ApiError> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    let turn_stream = state
        .chat_turns
        .get(&wave_id)
        .ok_or_else(|| api_error(StatusCode::NOT_FOUND, "no active chat for wave"))?;

    if turn_stream.is_completed() {
        return Err(api_error(StatusCode::GONE, "chat turn already completed"));
    }

    let live_rx = turn_stream.subscribe();

    let (tx, rx) = tokio::sync::mpsc::channel::<Result<SseEvent, Infallible>>(256);
    tokio::spawn(async move {
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

pub async fn list_chat_messages_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> ApiResult<ListResponse<ChatMessageDto>> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    let messages = state
        .store
        .list_chat_messages(&wave_id)
        .await
        .map_err(map_store_error)?;

    let dtos = messages.into_iter().map(chat_message_dto).collect();
    Ok(Json(ListResponse::new(dtos, false)))
}

async fn persist_turn_events(
    store: &crate::lfd::store::SharedStore,
    wave_id: &LfdId,
    events: &[AgentEvent],
) {
    for event in events {
        let Some(message) = chat_message_from_agent_event(wave_id, event) else {
            continue;
        };
        let _ = store.create_chat_message(&message).await;
    }
}

async fn publish_terminal_event(
    store: &crate::lfd::store::SharedStore,
    wave_id: &LfdId,
    turn_stream: &std::sync::Arc<crate::lfd::http::state::ChatTurnStream>,
    event: AgentEvent,
) {
    if let Some(message) = chat_message_from_agent_event(wave_id, &event) {
        let _ = store.create_chat_message(&message).await;
    }
    turn_stream.publish(event);
}

async fn load_sorted_memory_blocks(
    state: &HttpState,
    wave_id: &LfdId,
) -> Result<Vec<ChatMemoryBlock>, ApiError> {
    let mut memory_blocks = state
        .store
        .list_chat_memory_blocks(wave_id)
        .await
        .map_err(map_store_error)?;
    memory_blocks.sort_by(|left, right| {
        left.position
            .cmp(&right.position)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(memory_blocks)
}

fn chat_message_from_agent_event(wave_id: &LfdId, event: &AgentEvent) -> Option<ChatMessage> {
    let (role, content) = match event {
        AgentEvent::Message { content, .. } => {
            let trimmed = content.trim();
            if trimmed.is_empty() {
                return None;
            }
            ("assistant", trimmed.to_string())
        }
        AgentEvent::MemoryEdit { op, block, .. } => ("memory", memory_edit_badge(op, block)),
        AgentEvent::Failed { message, .. } => ("error", message.clone()),
        _ => return None,
    };

    Some(ChatMessage {
        id: LfdId::new(),
        wave_id: wave_id.clone(),
        role: role.to_string(),
        content,
        created_at: OffsetDateTime::now_utc(),
    })
}

fn memory_edit_badge(op: &str, block: &str) -> String {
    if op.eq_ignore_ascii_case("delete") {
        format!("Agent updated memory: deleted {block}")
    } else {
        format!("Agent updated memory: {block}")
    }
}

fn normalized_memory_block_name(name: String) -> Result<String, ApiError> {
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

fn build_chat_system_prompt(memory_blocks: &[ChatMemoryBlock]) -> String {
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

fn map_chat_turn_start_error(error: ChatTurnStartError) -> ApiError {
    match error {
        ChatTurnStartError::AlreadyRunning => {
            api_error(StatusCode::CONFLICT, "chat turn already running")
        }
        ChatTurnStartError::Unavailable => api_error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "chat turn registry unavailable",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::id::LfdId;

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
    fn build_chat_system_prompt_embeds_memory_xml() {
        let prompt = build_chat_system_prompt(&[ChatMemoryBlock {
            wave_id: LfdId::from_raw("wave-1"),
            name: "prefs".to_string(),
            content: "Use <short> answers & bullets".to_string(),
            position: 0,
            updated_at: None,
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

    #[test]
    fn chat_message_from_memory_delete_event_uses_delete_badge() {
        let wave_id = LfdId::from_raw("wave-1");
        let event = AgentEvent::MemoryEdit {
            op: "delete".to_string(),
            block: "prefs".to_string(),
            detail: "".to_string(),
        };

        let message = chat_message_from_agent_event(&wave_id, &event)
            .expect("memory edit should map to chat message");
        assert_eq!(message.role, "memory");
        assert_eq!(message.content, "Agent updated memory: deleted prefs");
    }
}
