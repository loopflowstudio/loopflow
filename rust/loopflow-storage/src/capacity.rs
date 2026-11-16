//! Saved account capacity and a revision-checked replacement.
use crate::{AccountLimitRow, AccountLimitWindow};

#[derive(Debug)]
pub struct ProviderCapacityReading {
    pub revision: i64,
    pub cooldown_reason: Option<String>,
    pub cooldown_until: Option<i64>,
    pub limits: Vec<AccountLimitRow>,
}

#[derive(Debug)]
pub struct ProviderCapacityUpdate {
    pub windows: Vec<AccountLimitWindow>,
    pub superseded_windows: Vec<String>,
    pub recovered: bool,
}
