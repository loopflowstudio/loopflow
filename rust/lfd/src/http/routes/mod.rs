pub mod hooks;
pub mod system;
pub mod wave_runs;
pub mod waves;
pub mod worktrees;
pub mod ws;

use axum::http::StatusCode;
use axum::Json;

use crate::http::dto::{wave_dto, wave_run_dto, ErrorResponse, WaveDto};
use crate::http::run_store;
use crate::store::{SharedStore, StoreError};
use crate::types::Wave;

pub async fn resolve_wave_id(
    state: &crate::http::HttpState,
    value: &str,
) -> Result<crate::id::LfdId, (StatusCode, Json<ErrorResponse>)> {
    if let Ok(id) = value.parse::<crate::id::LfdId>() {
        return Ok(id);
    }

    let name = value.to_string();
    let waves = run_store(&state.store, move |store| store.list_waves(None))
        .await
        .map_err(crate::http::map_store_error)?;

    waves
        .into_iter()
        .find(|wave| wave.name == name)
        .map(|wave| wave.id)
        .ok_or_else(|| crate::http::api_error(StatusCode::NOT_FOUND, "wave not found"))
}

pub async fn build_wave_dtos(
    store: &SharedStore,
    waves: Vec<Wave>,
    include_active_run: bool,
) -> Result<Vec<WaveDto>, StoreError> {
    let mut views = Vec::with_capacity(waves.len());
    for wave in waves {
        views.push(build_wave_dto(store, wave, include_active_run).await?);
    }
    Ok(views)
}

pub async fn build_wave_dto(
    store: &SharedStore,
    wave: Wave,
    include_active_run: bool,
) -> Result<WaveDto, StoreError> {
    let wave_id = wave.id.clone();
    let active = run_store(store, move |store| store.get_active_wave_run(&wave_id)).await?;

    let status = if wave.paused {
        "paused".to_string()
    } else {
        active
            .as_ref()
            .map(|run| wave_run_dto(run.clone(), Some(&wave)).status.clone())
            .unwrap_or_else(|| "idle".to_string())
    };

    let iteration = if let Some(run) = active.as_ref() {
        run.iteration
    } else {
        let wave_id = wave.id.clone();
        let last_run = run_store(store, move |store| {
            store.list_wave_runs(Some(&wave_id), Some(1))
        })
        .await?
        .into_iter()
        .next();
        last_run.map(|run| run.iteration).unwrap_or(0)
    };

    let active_run = if include_active_run {
        active.map(|run| wave_run_dto(run, Some(&wave)))
    } else {
        None
    };

    Ok(wave_dto(&wave, status, iteration, active_run))
}

// Note: wave_run_dto is used directly in wave run routes.
