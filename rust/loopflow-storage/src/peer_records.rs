//! Retained planning import status and journals.

use crate::planning_exchange::PlanningMutation;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct PeerProjectionConflict {
    pub target: String,
    pub reason: String,
}

/// Local routing and import evidence. Endpoints are deliberately not displayed:
/// Git URLs can contain credentials. A checkpoint is not publication evidence.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct PeerPlanningStatus {
    pub id: String,
    pub reference: String,
    pub active: bool,
    pub selected_records: u64,
    pub imported_revision: Option<String>,
    pub fetched_revision: Option<String>,
    pub acquisition_error: Option<String>,
    pub publication_revision: Option<String>,
    pub publication_state: Option<String>,
    pub publication_error: Option<String>,
    pub pending_local: bool,
    pub conflicts: Vec<PeerProjectionConflict>,
}

/// `lf repo tracker journal`: one destination's retained journal. It grows with
/// every edit, so it is a one-shot read, not part of the status.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlanningJournal {
    pub destination: String,
    /// Retained journal entries, including losing values and rejected projections.
    pub changes: BTreeMap<String, PlanningMutation>,
    /// Ranked mutations, not evidence of successful projection or publication.
    pub candidates: BTreeSet<String>,
}
