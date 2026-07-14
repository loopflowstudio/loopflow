//! Durable Wave identity and authored launch policy.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

fn default_task_capacity() -> u32 {
    1
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wave {
    pub id: LfdId,
    pub name: String,
    /// The single repo this wave targets. A wave = exactly one repo.
    pub repo: String,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
    /// Maximum number of active Task Sessions this Wave can have at once.
    pub task_capacity: u32,
    /// Parent wave in the chord tree. `None` for a root wave. A chord is simply
    /// a wave that has children (`children_of(id)` non-empty) — there is no
    /// `wave_type` discriminator.
    pub parent_wave_id: Option<LfdId>,
}

impl Wave {
    pub fn new(id: LfdId, name: String, repo: String) -> Self {
        Self {
            id,
            name,
            repo,
            created_at: Some(OffsetDateTime::now_utc()),
            task_capacity: default_task_capacity(),
            parent_wave_id: None,
        }
    }

    /// Attach this wave to a parent, making it a child in the chord tree.
    pub fn with_parent(mut self, parent: LfdId) -> Self {
        self.parent_wave_id = Some(parent);
        self
    }

    pub fn id(&self) -> &LfdId {
        &self.id
    }

    /// Parent wave in the chord tree, `None` for a root wave.
    pub fn parent_wave_id(&self) -> Option<&LfdId> {
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

    pub fn task_capacity(&self) -> u32 {
        self.task_capacity
    }
}
