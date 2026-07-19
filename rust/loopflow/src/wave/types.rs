//! Durable Wave registry identity and placement.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::id::WaveId;

/// The one-time typed wake derived from a child Wave's durable parent link.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PromotionWake {
    pub parent_wave_id: WaveId,
    pub parent: String,
}

impl PromotionWake {
    pub fn inbox_id(&self) -> String {
        format!("promotion:{}", self.parent_wave_id)
    }

    pub fn prompt(&self) -> String {
        format!(
            "Promotion from parent Wave '{}' is complete. Begin the first child-Wave pass and report what this Wave now owns.",
            self.parent
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wave {
    pub id: WaveId,
    pub name: String,
    /// The single repo this wave targets. A wave = exactly one repo.
    pub repo: String,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    /// Parent wave in the chord tree. `None` for a root wave. A chord is simply
    /// a wave that has children (`children_of(id)` non-empty) — there is no
    /// `wave_type` discriminator.
    pub parent_wave_id: Option<WaveId>,
}

impl Wave {
    pub fn new(id: WaveId, name: String, repo: String) -> Self {
        Self {
            id,
            name,
            repo,
            created_at: Some(OffsetDateTime::now_utc()),
            parent_wave_id: None,
        }
    }

    /// Attach this wave to a parent, making it a child in the chord tree.
    pub fn with_parent(mut self, parent: WaveId) -> Self {
        self.parent_wave_id = Some(parent);
        self
    }

    pub fn id(&self) -> &WaveId {
        &self.id
    }

    /// Parent wave in the chord tree, `None` for a root wave.
    pub fn parent_wave_id(&self) -> Option<&WaveId> {
        self.parent_wave_id.as_ref()
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
