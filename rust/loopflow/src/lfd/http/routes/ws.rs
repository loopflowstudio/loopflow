use axum::extract::connect_info::ConnectInfo;
use axum::extract::ws::{CloseFrame, Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use std::net::SocketAddr;
use tokio::sync::mpsc;
use tokio::time::{interval, Duration};
use tokio_stream::wrappers::BroadcastStream;

use crate::lfd::http::dto::ErrorResponse;
use crate::lfd::http::routes::{build_wave_dto, build_wave_dtos};
use crate::lfd::http::state::HttpState;
use crate::lfd::id::LfdId;
use crate::lfd::store::SharedStore;
use crate::lfd::types::Event;

pub async fn ws_handler(
    State(state): State<HttpState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
) -> Result<impl IntoResponse, (StatusCode, axum::Json<ErrorResponse>)> {
    let bearer_token = crate::lfd::auth::bearer_token(&headers);
    let source_ip = peer.ip();
    let ws = ws
        .max_frame_size(state.http_security.max_ws_frame_bytes)
        .max_message_size(state.http_security.max_ws_message_bytes);
    Ok(ws.on_upgrade(move |socket| handle_ws(socket, state, bearer_token, source_ip)))
}

async fn handle_ws(
    mut socket: WebSocket,
    state: HttpState,
    bearer_token: Option<String>,
    source_ip: std::net::IpAddr,
) {
    let connected = match current_snapshot(&state).await {
        Ok(snapshot) => snapshot,
        Err(err) => {
            tracing::warn!(error = %err, "failed to build websocket snapshot");
            let _ = socket
                .send(text_message(
                    serde_json::json!({
                        "type": "error",
                        "error": "failed to build initial snapshot",
                    })
                    .to_string(),
                ))
                .await;
            return;
        }
    };

    let _ = socket
        .send(text_message(
            serde_json::json!({
                "type": "connected",
                "timestamp": time::OffsetDateTime::now_utc()
                    .format(&time::format_description::well_known::Rfc3339)
                    .unwrap_or_default(),
                "waves": connected,
            })
            .to_string(),
        ))
        .await;

    let (mut ws_sender, mut ws_receiver) = socket.split();
    let (outbound_tx, mut outbound_rx) = mpsc::channel::<Message>(state.http_security.max_ws_queue);
    let writer = tokio::spawn(async move {
        while let Some(message) = outbound_rx.recv().await {
            if ws_sender.send(message).await.is_err() {
                break;
            }
        }
    });

    let mut events = BroadcastStream::new(state.event_hub.subscribe());
    let mut output = BroadcastStream::new(state.output_hub.subscribe());
    let mut ticker = interval(Duration::from_secs(30));
    let mut revalidate_ticker = interval(Duration::from_secs(60));
    revalidate_ticker.tick().await;
    let mut malformed_messages = 0_u32;
    let malformed_limit = state.http_security.max_ws_malformed;

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let ping = serde_json::json!({ "type": "ping" }).to_string();
                if !enqueue_message(&outbound_tx, text_message(ping)) {
                    break;
                }
            }
            _ = revalidate_ticker.tick() => {
                if state
                    .auth
                    .validate(bearer_token.as_deref(), source_ip)
                    .await
                    .is_err()
                {
                    let _ = enqueue_message(&outbound_tx, unauthorized_close_message());
                    break;
                }
            }
            maybe_event = events.next() => {
                let Some(event) = maybe_event else { break };
                if let Ok(event) = event {
                    let json = match enrich_event(&event, &state.store, &state.github).await {
                        Some(enriched) => enriched,
                        None => serde_json::to_string(&event).unwrap_or_default(),
                    };
                    if !enqueue_message(&outbound_tx, text_message(json)) {
                        break;
                    }
                }
            }
            maybe_output = output.next() => {
                if let Some(Ok(output_event)) = maybe_output {
                    let event = Event::OutputLine {
                        wave_id: LfdId::from_raw(output_event.wave_id),
                        agent_id: LfdId::from_raw(output_event.agent_id),
                        text: output_event.text,
                        timestamp: Event::now(),
                    };
                    let json = serde_json::to_string(&event).unwrap_or_default();
                    if !enqueue_message(&outbound_tx, text_message(json)) {
                        break;
                    }
                }
            }
            message = ws_receiver.next() => {
                let Some(message) = message else {
                    break;
                };
                match message {
                    Ok(Message::Text(text)) => {
                        if !is_valid_client_envelope(text.as_str())
                            && record_malformed(&mut malformed_messages, malformed_limit)
                        {
                            break;
                        }
                    }
                    Ok(Message::Pong(_)) => {}
                    Ok(Message::Close(_)) => break,
                    Ok(_) => {
                        if record_malformed(&mut malformed_messages, malformed_limit) {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        }
    }

    drop(outbound_tx);
    let _ = writer.await;
}

fn enqueue_message(sender: &mpsc::Sender<Message>, message: Message) -> bool {
    sender.try_send(message).is_ok()
}

fn text_message(text: String) -> Message {
    Message::Text(text.into())
}

fn unauthorized_close_message() -> Message {
    Message::Close(Some(CloseFrame {
        code: 4401,
        reason: "token revoked".into(),
    }))
}

fn is_valid_client_envelope(text: &str) -> bool {
    let Ok(payload) = serde_json::from_str::<serde_json::Value>(text) else {
        return false;
    };
    payload
        .as_object()
        .and_then(|value| value.get("type"))
        .and_then(serde_json::Value::as_str)
        .is_some()
}

fn record_malformed(counter: &mut u32, malformed_limit: u32) -> bool {
    *counter = counter.saturating_add(1);
    *counter >= malformed_limit
}

async fn current_snapshot(
    state: &HttpState,
) -> Result<Vec<crate::lfd::http::dto::WaveDto>, String> {
    let store = state.store.clone();
    let waves = store
        .list_waves(None)
        .await
        .map_err(|err| err.to_string())?;
    build_wave_dtos(&state.store, &state.github, waves, true)
        .await
        .map_err(|err| err.to_string())
}

/// Enrich wave lifecycle events with the full WaveDto payload.
/// Returns `None` for non-wave events, letting the caller fall back to plain serialization.
async fn enrich_event(
    event: &Event,
    store: &SharedStore,
    github_config: &crate::lfd::config::GitHubConfig,
) -> Option<String> {
    let wave_id = match event {
        Event::WaveCreated { wave_id, .. }
        | Event::WaveUpdated { wave_id, .. }
        | Event::WaveStarted { wave_id, .. }
        | Event::WaveStopped { wave_id, .. }
        | Event::WaveWaiting { wave_id, .. }
        | Event::CiFailure { wave_id, .. }
        | Event::ActivationQueued { wave_id, .. }
        | Event::ActivationCoalesced { wave_id, .. }
        | Event::ActivationDropped { wave_id, .. } => wave_id.clone(),
        _ => return None,
    };

    let wave = store.get_wave(&wave_id).await.ok()??;
    let dto = build_wave_dto(store, github_config, wave, true)
        .await
        .ok()?;

    let mut base = serde_json::to_value(event).ok()?;
    if let serde_json::Value::Object(ref mut map) = base {
        map.insert("wave".to_string(), serde_json::to_value(&dto).ok()?);
    }
    serde_json::to_string(&base).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_envelope_requires_type_field() {
        assert!(is_valid_client_envelope(r#"{"type":"pong"}"#));
        assert!(!is_valid_client_envelope(r#"{"message":"pong"}"#));
        assert!(!is_valid_client_envelope(r#"["type","pong"]"#));
        assert!(!is_valid_client_envelope("not-json"));
    }

    #[test]
    fn malformed_counter_disconnects_at_limit() {
        let mut counter = 0;
        assert!(!record_malformed(&mut counter, 3));
        assert!(!record_malformed(&mut counter, 3));
        assert!(record_malformed(&mut counter, 3));
    }

    #[test]
    fn unauthorized_close_message_uses_private_code() {
        let message = unauthorized_close_message();
        match message {
            Message::Close(Some(frame)) => {
                assert_eq!(frame.code, 4401);
                assert_eq!(frame.reason.as_str(), "token revoked");
            }
            _ => panic!("expected close frame"),
        }
    }

    #[tokio::test]
    async fn outbound_queue_overflow_disconnects_sender() {
        let (tx, mut rx) = mpsc::channel(1);
        assert!(enqueue_message(&tx, text_message("first".to_string())));
        assert!(!enqueue_message(&tx, text_message("second".to_string())));
        assert!(rx.recv().await.is_some());
    }
}
