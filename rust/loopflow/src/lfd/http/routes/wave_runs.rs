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
use crate::lfd::types::WaveRun;

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
    let output_rx = state.output_hub.subscribe();
    let (tx, rx) = tokio::sync::mpsc::channel::<Result<Bytes, std::io::Error>>(128);
    let store = state.store.clone();

    tokio::spawn(async move {
        let mut stream = BroadcastStream::new(output_rx);
        let mut cache: std::collections::HashMap<String, bool> = std::collections::HashMap::new();
        while let Some(event) = stream.next().await {
            let Ok(OutputEvent {
                wave_run_id,
                agent_id: _,
                text,
            }) = event
            else {
                continue;
            };

            let include = if let Some(hit) = cache.get(&wave_run_id) {
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

            if include && tx.send(Ok(Bytes::from(format!("{text}\n")))).await.is_err() {
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

    let (runs, has_more) = paginate_wave_runs(
        filtered,
        query.limit,
        query.starting_after.as_deref(),
        query.ending_before.as_deref(),
    );

    let mut data = Vec::with_capacity(runs.len());
    for run in runs {
        data.push(wave_run_dto(run));
    }

    Ok(Json(ListResponse::new(data, has_more)))
}

fn paginate_wave_runs(
    runs: Vec<WaveRun>,
    limit: Option<u32>,
    starting_after: Option<&str>,
    ending_before: Option<&str>,
) -> (Vec<WaveRun>, bool) {
    let mut items = runs;
    if let Some(starting_after) = starting_after {
        if let Some(pos) = items
            .iter()
            .position(|run| run.id.to_string() == starting_after)
        {
            items = items.split_off(pos + 1);
        }
    }
    if let Some(ending_before) = ending_before {
        if let Some(pos) = items
            .iter()
            .position(|run| run.id.to_string() == ending_before)
        {
            items.truncate(pos);
        }
    }

    let mut has_more = false;
    if let Some(limit) = limit {
        let limit = limit as usize;
        if items.len() > limit {
            items.truncate(limit);
            has_more = true;
        }
    }

    (items, has_more)
}
