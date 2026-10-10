//! Durable Wave identity, authored context, and placement.

pub mod config;
pub mod context;
pub mod metrics;
pub mod relocate;

use serde::Serialize;
use time::OffsetDateTime;

use crate::id::WaveId;
use crate::repository::{CanonicalRepo, CanonicalRepoError};

/// A Wave's mutable readable address inside one canonical repository.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WaveLocator {
    repo: CanonicalRepo,
    slug: String,
}

#[derive(Debug, thiserror::Error)]
pub enum WaveLocatorError {
    #[error(transparent)]
    Repository(#[from] CanonicalRepoError),
    #[error("invalid Wave slug {0:?}")]
    InvalidSlug(String),
}

impl WaveLocator {
    pub fn discover(repo: &std::path::Path, slug: &str) -> Result<Self, WaveLocatorError> {
        Self::new(CanonicalRepo::discover(repo)?, slug)
    }

    pub fn new(repo: CanonicalRepo, slug: &str) -> Result<Self, WaveLocatorError> {
        let slug = crate::ops::util::normalize_wave_name(slug)
            .ok_or_else(|| WaveLocatorError::InvalidSlug(slug.to_string()))?;
        if slug.contains('\\')
            || slug
                .split('/')
                .any(|component| component.is_empty() || matches!(component, "." | ".."))
        {
            return Err(WaveLocatorError::InvalidSlug(slug));
        }
        Ok(Self { repo, slug })
    }

    pub fn repo(&self) -> &CanonicalRepo {
        &self.repo
    }

    pub fn slug(&self) -> &str {
        &self.slug
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Wave {
    id: WaveId,
    name: String,
    /// Derived on read from this Wave and its ancestors; never persisted.
    #[serde(skip)]
    slug: String,
    /// Current canonical repository half of the mutable locator.
    repo: String,
    #[serde(with = "time::serde::rfc3339::option")]
    created_at: Option<OffsetDateTime>,
    /// Directory parent; absent for a top-level Wave.
    parent_wave_id: Option<WaveId>,
    /// The first completed promotion occurrence. Ancestry alone leaves this
    /// absent so an older parent link cannot manufacture a new wake.
    #[serde(with = "time::serde::rfc3339::option")]
    promoted_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    retired_at: Option<OffsetDateTime>,
    superseded_by_wave_id: Option<WaveId>,
    retirement_reason: Option<String>,
}

impl Wave {
    pub fn new(id: WaveId, name: String, repo: String) -> Self {
        Self {
            id,
            slug: name.clone(),
            name,
            repo,
            created_at: Some(OffsetDateTime::now_utc()),
            parent_wave_id: None,
            promoted_at: None,
            retired_at: None,
            superseded_by_wave_id: None,
            retirement_reason: None,
        }
    }

    /// Establish an initial directory parent without recording a promotion occurrence.
    /// An existing parent always wins.
    pub fn with_parent(mut self, parent: WaveId) -> Self {
        self.parent_wave_id.get_or_insert(parent);
        self
    }

    #[allow(clippy::too_many_arguments)] // Exact Wave row shape; named accessors expose the domain API.
    pub(crate) fn from_stored_parts(
        id: WaveId,
        name: String,
        repo: String,
        created_at: OffsetDateTime,
        parent_wave_id: Option<WaveId>,
        promoted_at: Option<OffsetDateTime>,
        retired_at: Option<OffsetDateTime>,
        superseded_by_wave_id: Option<WaveId>,
        retirement_reason: Option<String>,
    ) -> Self {
        Self {
            id,
            slug: name.clone(),
            name,
            repo,
            created_at: Some(created_at),
            parent_wave_id,
            promoted_at,
            retired_at,
            superseded_by_wave_id,
            retirement_reason,
        }
    }

    pub fn id(&self) -> &WaveId {
        &self.id
    }

    /// Directory parent, `None` for a root Wave.
    pub fn parent_wave_id(&self) -> Option<&WaveId> {
        self.parent_wave_id.as_ref()
    }

    /// The first completed promotion occurrence, distinct from ancestry.
    pub fn promoted_at(&self) -> Option<OffsetDateTime> {
        self.promoted_at
    }

    pub fn retired_at(&self) -> Option<OffsetDateTime> {
        self.retired_at
    }

    pub fn superseded_by_wave_id(&self) -> Option<&WaveId> {
        self.superseded_by_wave_id.as_ref()
    }

    pub fn retirement_reason(&self) -> Option<&str> {
        self.retirement_reason.as_deref()
    }

    pub fn is_retired(&self) -> bool {
        self.retired_at.is_some()
    }

    pub fn slug(&self) -> &str {
        &self.slug
    }

    pub(crate) fn with_slug(mut self, slug: String) -> Self {
        self.slug = slug;
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// The repo this wave targets.
    pub fn repo(&self) -> &str {
        &self.repo
    }

    pub fn created_at(&self) -> Option<OffsetDateTime> {
        self.created_at
    }
}

pub async fn ensure_wave_row(
    store: &crate::store::Store,
    repo: &std::path::Path,
    name: &str,
) -> crate::store::StoreResult<Wave> {
    let locator = WaveLocator::discover(repo, name)
        .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))?;
    let id = store
        .sqlite
        .ensure_wave(&repo.to_string_lossy(), locator.slug())?;
    store
        .get_wave(&id)
        .await?
        .ok_or(crate::store::StoreError::NotFound)
}
