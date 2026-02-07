pub mod flows;
pub mod hooks;
pub mod system;
pub mod wave_runs;
pub mod waves;
pub mod ws;

use crate::lfd::http::dto::{wave_dto, wave_run_dto, ErrorResponse, WaveDto};
use crate::lfd::http::run_store;
use crate::lfd::store::{SharedStore, StoreError};
use crate::lfd::types::Wave;
use axum::http::StatusCode;
use axum::Json;

pub async fn resolve_wave_id(
    state: &crate::lfd::http::HttpState,
    value: &str,
) -> Result<crate::lfd::id::LfdId, (StatusCode, Json<ErrorResponse>)> {
    if let Ok(id) = value.parse::<crate::lfd::id::LfdId>() {
        return Ok(id);
    }

    let name = value.to_string();
    let wave = run_store(&state.store, move |store| store.get_wave_by_name(&name))
        .await
        .map_err(crate::lfd::http::map_store_error)?;
    wave.map(|wave| wave.id)
        .ok_or_else(|| crate::lfd::http::api_error(StatusCode::NOT_FOUND, "wave not found"))
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

    let active_run = if include_active_run {
        active.map(wave_run_dto)
    } else {
        None
    };

    Ok(wave_dto(&wave, active_run))
}
