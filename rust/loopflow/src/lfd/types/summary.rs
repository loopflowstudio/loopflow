//! Summary type for cached wave area summaries.

use time::OffsetDateTime;

use crate::lfd::id::LfdId;

#[derive(Debug, Clone)]
pub struct Summary {
    pub id: LfdId,
    pub wave_id: LfdId,
    pub content: String,
    pub source_hash: String,
    pub token_budget: u32,
    pub agent: String,
    pub created_at: Option<OffsetDateTime>,
}
