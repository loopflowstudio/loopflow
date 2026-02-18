//! Persisted chat messages keyed by wave.

use time::OffsetDateTime;

use crate::lfd::id::LfdId;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatMessage {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub role: String,
    pub content: String,
    pub created_at: OffsetDateTime,
}
