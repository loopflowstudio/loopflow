use crate::id::WaveId;
use crate::work::wave::{Wave, WaveLocator};

pub async fn ensure_wave_row(
    store: &crate::store::Store,
    main_repo: &std::path::Path,
    name: &str,
) -> crate::store::StoreResult<Wave> {
    let locator = WaveLocator::discover(main_repo, name)
        .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))?;
    let existing = store.get_wave_at(&locator).await?;
    let is_new = existing.is_none();
    let wave = existing.unwrap_or_else(|| {
        Wave::new(
            WaveId::new(),
            locator.slug().to_string(),
            locator.repo().to_string(),
        )
    });
    store.create_wave(&wave).await?;
    if is_new {
        tracing::info!(
            wave = name,
            wave_id = %wave.id(),
            "wave was not in the registry; created its row"
        );
    }
    Ok(wave)
}
