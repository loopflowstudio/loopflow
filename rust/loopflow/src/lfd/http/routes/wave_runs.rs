use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::Response;
use axum::Json;
use bytes::Bytes;
use futures_util::StreamExt;
use serde::Deserialize;
use tokio_stream::wrappers::{BroadcastStream, ReceiverStream};

use crate::lfd::http::dto::{wave_run_dto, ListResponse, WaveRunDto};
use crate::lfd::http::routes::resolve_wave_id;
use crate::lfd::http::state::HttpState;
use crate::lfd::http::{map_store_error, run_store, ApiResult};
use crate::lfd::id::LfdId;
use crate::lfd::output::OutputEvent;

#[derive(Deserialize, Default)]
pub struct ListWaveRunsQuery {
    wave_id: Option<String>,
    repo: Option<String>,
    limit: Option<u32>,
    starting_after: Option<String>,
    ending_before: Option<String>,
}

pub async fn list_wave_runs_handler(
    State(state): State<HttpState>,
    Query(query): Query<ListWaveRunsQuery>,
) -> ApiResult<ListResponse<WaveRunDto>> {
    list_wave_runs(&state, None, query).await
}

pub async fn list_wave_runs_for_wave_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
    Query(query): Query<ListWaveRunsQuery>,
) -> ApiResult<ListResponse<WaveRunDto>> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;
    list_wave_runs(&state, Some(wave_id), query).await
}

/// Replay + follow: sends persisted output from the log file, then
/// continues with live broadcast events. Subscribe before read ensures
/// no lines are lost at the boundary.
pub async fn wave_logs_handler(
    State(state): State<HttpState>,
    Path(wave_id): Path<String>,
) -> Result<
    Response,
    (
        axum::http::StatusCode,
        Json<crate::lfd::http::dto::ErrorResponse>,
    ),
> {
    let wave_id = resolve_wave_id(&state, &wave_id).await?;

    // Find the most recent run for this wave (active or completed).
    let store = state.store.clone();
    let wid = wave_id.clone();
    let latest_run = run_store(&store, move |store| {
        store.list_wave_runs(Some(&wid), Some(1))
    })
    .await
    .map_err(map_store_error)?
    .into_iter()
    .next();

    // Step 1: Subscribe to broadcast BEFORE reading the file.
    let output_rx = state.output_hub.subscribe();
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(128);

    let output_hub = state.output_hub.clone();

    tokio::spawn(async move {
        // Step 2: Replay from log file.
        let mut replayed_lines: usize = 0;
        if let Some(ref run) = latest_run {
            let run_id_str = run.id.to_string();
            if let Some((lines, _offset)) = output_hub.read_log(&run_id_str) {
                for line in &lines {
                    if tx.send(Ok(Bytes::from(format!("{line}\n")))).await.is_err() {
                        return;
                    }
                }
                replayed_lines = lines.len();
            }
        }

        // Step 3: Forward live broadcast, skipping lines already replayed.
        // Since we subscribed before reading, some broadcast events may
        // duplicate what we just sent from the file. Track count to skip.
        let mut skip_remaining = replayed_lines;
        let mut stream = BroadcastStream::new(output_rx);
        let mut cache: std::collections::HashMap<String, bool> = std::collections::HashMap::new();

        while let Some(event) = stream.next().await {
            let Ok(OutputEvent {
                wave_run_id,
                agent_id: _,
                wave_id: event_wave_id,
                text,
            }) = event
            else {
                continue;
            };

            // Filter: only include output for runs belonging to this wave.
            let include = if event_wave_id == wave_id.to_string() {
                true
            } else if let Some(hit) = cache.get(&wave_run_id) {
                *hit
            } else {
                let run_id = LfdId::from_raw(wave_run_id.clone());
                let target = wave_id.clone();
                let result = run_store(&store, move |store| store.get_wave_run(&run_id)).await;
                let matches = match result {
                    Ok(Some(run)) => run.wave_id == target,
                    _ => false,
                };
                cache.insert(wave_run_id.clone(), matches);
                matches
            };

            if !include {
                continue;
            }

            // Skip lines that were already sent from the file replay.
            if skip_remaining > 0 {
                skip_remaining -= 1;
                continue;
            }

            if tx.send(Ok(Bytes::from(format!("{text}\n")))).await.is_err() {
                break;
            }
        }
    });

    let stream = ReceiverStream::new(rx);
    let body = Body::from_stream(stream);
    let mut response = Response::new(body);
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        header::HeaderValue::from_static("text/plain"),
    );
    Ok(response)
}

async fn list_wave_runs(
    state: &HttpState,
    path_wave_id: Option<LfdId>,
    query: ListWaveRunsQuery,
) -> ApiResult<ListResponse<WaveRunDto>> {
    let query_wave_id = match query.wave_id.as_deref() {
        Some(id) => Some(resolve_wave_id(state, id).await?),
        None => None,
    };
    let wave_id = path_wave_id.or(query_wave_id);

    let runs = run_store(&state.store, {
        let wave_id = wave_id.clone();
        move |store| store.list_wave_runs(wave_id.as_ref(), None)
    })
    .await
    .map_err(map_store_error)?;

    let mut filtered = runs;

    if let Some(repo) = query.repo.as_deref() {
        filtered.retain(|run| run.snapshot.repo == repo);
    }

    let (runs, has_more) = super::paginate(
        filtered,
        query.limit,
        query.starting_after.as_deref(),
        query.ending_before.as_deref(),
        |r| &r.id,
    );

    let mut data = Vec::with_capacity(runs.len());
    for run in runs {
        data.push(wave_run_dto(run));
    }

    Ok(Json(ListResponse::new(data, has_more)))
}
