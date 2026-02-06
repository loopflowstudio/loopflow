pub mod hooks;
pub mod system;
pub mod waves;
pub mod ws;

use crate::http::dto::{wave_run_summary, wave_summary, WaveViewDto};
use crate::http::run_store;
use crate::store::{SharedStore, StoreError};
use crate::types::Wave;

pub async fn build_wave_views(
    store: &SharedStore,
    waves: Vec<Wave>,
) -> Result<Vec<WaveViewDto>, StoreError> {
    let mut views = Vec::with_capacity(waves.len());
    for wave in waves {
        views.push(build_wave_view(store, wave).await?);
    }
    Ok(views)
}

pub async fn build_wave_view(store: &SharedStore, wave: Wave) -> Result<WaveViewDto, StoreError> {
    let wave_id = wave.id.clone();
    let active = run_store(store, move |store| store.get_active_wave_run(&wave_id)).await?;

    let summary = wave_summary(&wave);
    let active_run = active.map(wave_run_summary);
    let status = active_run
        .as_ref()
        .map(|run| run.status.clone())
        .unwrap_or_else(|| "idle".to_string());

    Ok(WaveViewDto {
        wave: summary,
        status,
        active_run,
    })
}
