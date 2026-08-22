//! Durable Wave registry identity and placement.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::id::WaveId;
use crate::repository::{CanonicalRepo, CanonicalRepoError};

/// A Wave's mutable human address inside one canonical repository.
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

/// The one-time typed wake derived from a child Wave's durable promotion occurrence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct PromotionWake {
    pub(crate) parent_wave_id: WaveId,
    pub(crate) parent: String,
}

impl PromotionWake {
    pub(crate) fn inbox_id(&self) -> String {
        format!("promotion:{}", self.parent_wave_id)
    }

    pub(crate) fn prompt(&self) -> String {
        format!(
            "Promotion from parent Wave '{}' is complete. Begin the first child-Wave pass and report what this Wave now owns.",
            self.parent
        )
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Wave {
    id: WaveId,
    name: String,
    /// Current canonical repository half of the mutable locator.
    repo: String,
    #[serde(with = "time::serde::rfc3339::option")]
    created_at: Option<OffsetDateTime>,
    /// Parent wave in the chord tree. `None` for a root wave. A chord is simply
    /// a wave that has children (`children_of(id)` non-empty) — there is no
    /// `wave_type` discriminator.
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

    /// Establish initial chord ancestry without recording a promotion occurrence.
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

    /// Record the first promotion occurrence and its validated ancestry.
    pub(crate) fn record_promotion(
        &mut self,
        parent: &WaveId,
        at: OffsetDateTime,
    ) -> Result<(), String> {
        if self
            .parent_wave_id
            .as_ref()
            .is_some_and(|current| current != parent)
        {
            return Err(format!(
                "Wave '{}' already belongs to another parent",
                self.name
            ));
        }
        self.parent_wave_id.get_or_insert_with(|| parent.clone());
        self.promoted_at.get_or_insert(at);
        Ok(())
    }

    pub fn id(&self) -> &WaveId {
        &self.id
    }

    /// Parent wave in the chord tree, `None` for a root wave.
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reconstructing_identity_cannot_copy_or_reparent_a_promotion() {
        let parent = WaveId::new();
        let mut promoted = Wave::new(WaveId::new(), "ship".to_string(), "/repo".to_string());
        promoted
            .record_promotion(&parent, OffsetDateTime::now_utc())
            .expect("record promotion");

        let rebuilt = Wave::new(
            promoted.id().clone(),
            promoted.name().to_string(),
            promoted.repo().to_string(),
        );
        assert_eq!(rebuilt.parent_wave_id(), None);
        assert_eq!(rebuilt.promoted_at(), None);

        let other_parent = WaveId::new();
        let unchanged = promoted.with_parent(other_parent);
        assert_eq!(unchanged.parent_wave_id(), Some(&parent));
        assert!(unchanged.promoted_at().is_some());
    }

    #[test]
    fn locator_slugs_allow_safe_nesting_but_never_path_traversal() {
        let repo = crate::repository::CanonicalRepo::discover(
            &std::env::current_dir().expect("current directory"),
        )
        .unwrap();
        assert_eq!(
            WaveLocator::new(repo.clone(), "goals/release")
                .unwrap()
                .slug(),
            "goals/release"
        );
        for unsafe_slug in [
            "../escape",
            "goals/../escape",
            "goals//release",
            "goals\\release",
        ] {
            assert!(WaveLocator::new(repo.clone(), unsafe_slug).is_err());
        }
    }
}
