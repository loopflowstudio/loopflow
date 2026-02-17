//! Persisted chat memory blocks keyed by wave + block name.

use time::OffsetDateTime;

use crate::lfd::id::LfdId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMemoryBlock {
    pub wave_id: LfdId,
    pub name: String,
    pub content: String,
    pub position: u32,
    pub updated_at: Option<OffsetDateTime>,
}
