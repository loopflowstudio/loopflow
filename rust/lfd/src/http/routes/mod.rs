pub mod hooks;
pub mod system;
pub mod wave_runs;
pub mod waves;
pub mod worktrees;
pub mod ws;

use crate::http::dto::{wave_dto, wave_run_dto, WaveDto};
use crate::http::run_store;
use crate::store::{SharedStore, StoreError};
use crate::types::Wave;

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

    let status = active
        .as_ref()
        .map(|run| wave_run_dto(run.clone(), Some(&wave)).status.clone())
        .unwrap_or_else(|| "idle".to_string());

    let active_run = if include_active_run {
        active.map(|run| wave_run_dto(run, Some(&wave)))
    } else {
        None
    };

    Ok(wave_dto(&wave, status, active_run))
}

// Note: wave_run_dto is used directly in wave run routes.
