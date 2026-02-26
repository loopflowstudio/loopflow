//! Chord type.

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::lfd::id::LfdId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Chord {
    pub id: LfdId,
    pub name: String,
    pub is_default: bool,
    #[serde(with = "time::serde::rfc3339::option")]
    pub created_at: Option<OffsetDateTime>,
}

impl Chord {
    pub fn id(&self) -> &LfdId {
        &self.id
    }

    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn is_default(&self) -> bool {
        self.is_default
    }

    pub fn created_at(&self) -> Option<OffsetDateTime> {
        self.created_at
    }
}
