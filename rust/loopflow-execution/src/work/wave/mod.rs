//! Durable Wave identity, authored context, and placement.

pub mod bootstrap;
pub mod config;
pub mod context;
pub mod relocate;
pub mod setup;

pub use loopflow_storage::work::wave::{Wave, WaveLocator, WaveLocatorError};
pub fn discover(repo: &std::path::Path, slug: &str) -> Result<WaveLocator, WaveLocatorError> {
    WaveLocator::new(crate::repository::discover(repo)?, slug)
}

pub async fn ensure_wave_row(
    store: &crate::store::Store,
    repo: &std::path::Path,
    name: &str,
) -> crate::store::StoreResult<Wave> {
    let locator = crate::work::wave::discover(repo, name)
        .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))?;
    let id = crate::work::wave::bootstrap::ensure_wave(
        &store.sqlite,
        &repo.to_string_lossy(),
        locator.slug(),
    )?;
    store
        .get_wave(&id)
        .await?
        .ok_or(crate::store::StoreError::NotFound)
}

pub mod lifecycle;
