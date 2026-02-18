use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
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
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, (StatusCode, axum::Json<ErrorResponse>)> {
    Ok(ws.on_upgrade(move |socket| handle_ws(socket, state)))
}

async fn handle_ws(mut socket: WebSocket, state: HttpState) {
    let connected = match current_snapshot(&state).await {
        Ok(snapshot) => snapshot,
        Err(err) => {
            let _ = socket
                .send(text_message(
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

    let (mut sender, mut receiver) = socket.split();
    let mut events = BroadcastStream::new(state.event_hub.subscribe());
    let mut output = BroadcastStream::new(state.output_hub.subscribe());
    let mut ticker = interval(Duration::from_secs(30));

    loop {
        tokio::select! {
            _ = ticker.tick() => {
                let ping = serde_json::json!({ "type": "ping" }).to_string();
                if sender.send(text_message(ping)).await.is_err() {
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
                    if sender.send(text_message(json)).await.is_err() {
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
                    if sender.send(text_message(json)).await.is_err() {
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

fn text_message(text: String) -> Message {
    Message::Text(text.into())
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
        | Event::CiFailure { wave_id, .. } => wave_id.clone(),
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
