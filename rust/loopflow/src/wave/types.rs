//! Durable Wave registry identity and placement.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::id::WaveId;

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
    /// The single repo this wave targets. A wave = exactly one repo.
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
        }
    }

    /// Establish initial chord ancestry without recording a promotion occurrence.
    /// An existing parent always wins.
    pub fn with_parent(mut self, parent: WaveId) -> Self {
        self.parent_wave_id.get_or_insert(parent);
        self
    }

    pub(crate) fn from_stored_parts(
        id: WaveId,
        name: String,
        repo: String,
        created_at: OffsetDateTime,
        parent_wave_id: Option<WaveId>,
        promoted_at: Option<OffsetDateTime>,
    ) -> Self {
        Self {
            id,
            name,
            repo,
            created_at: Some(created_at),
            parent_wave_id,
            promoted_at,
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
}
