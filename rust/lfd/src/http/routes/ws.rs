use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use futures_util::{SinkExt, StreamExt};
use tokio::time::{interval, Duration};
use tokio_stream::wrappers::BroadcastStream;

use crate::http::dto::ErrorResponse;
use crate::http::routes::build_wave_dtos;
use crate::http::run_store;
use crate::http::state::HttpState;

pub async fn ws_handler(
    State(state): State<HttpState>,
    ws: WebSocketUpgrade,
) -> Result<impl IntoResponse, (StatusCode, axum::Json<ErrorResponse>)> {
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
                    let json = serde_json::to_string(&event).unwrap_or_default();
                    if sender.send(Message::Text(json)).await.is_err() {
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

async fn current_snapshot(
    store: &crate::store::SharedStore,
) -> Result<Vec<crate::http::dto::WaveDto>, String> {
    let waves = run_store(store, move |store| store.list_waves(None))
        .await
        .map_err(|err| err.to_string())?;
    build_wave_dtos(store, waves, true)
        .await
        .map_err(|err| err.to_string())
}
