//! Durable administrative receipts. Recording a receipt never creates or revokes a key.
use serde::{Deserialize, Serialize};

use crate::durable::HomeId;
use crate::spend::{AccessRequirement, Credential, EnvironmentId};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RotationId(pub String);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RotationState {
    Candidate,
    RetirementPending,
    Retired,
    Cancelled,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RotationOperation {
    CandidateRead,
    ConsumerCutover,
    Retirement,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RotationReceipt {
    pub operation: RotationOperation,
    pub environment: Option<EnvironmentId>,
    pub executed_home: Option<HomeId>,
    pub candidate_version: String,
    pub success: bool,
    pub observed_at: i64,
    /// Non-secret provider/operator evidence, never a response body or credential value.
    pub evidence: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotationConsumer {
    pub environment: EnvironmentId,
    pub home_id: Option<HomeId>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rotation {
    pub id: RotationId,
    pub old: Credential,
    pub candidate: Credential,
    pub requirements: Vec<AccessRequirement>,
    pub consumers: Vec<RotationConsumer>,
    /// Explicit administrative attestation, not inferred from the discovered inventory.
    pub consumer_inventory_evidence: Option<String>,
    pub state: RotationState,
    pub receipts: Vec<RotationReceipt>,
    pub activated_at: Option<i64>,
}
impl Rotation {
    pub fn consumers_ready(&self, operation: RotationOperation) -> bool {
        !self.consumers.is_empty()
            && self.consumers.iter().all(|consumer| {
                self.receipts
                    .iter()
                    .filter(|receipt| {
                        receipt.operation == operation
                            && receipt.environment.as_ref() == Some(&consumer.environment)
                            && receipt.executed_home == consumer.home_id
                            && consumer.home_id.is_some()
                            && Some(&receipt.candidate_version) == self.candidate.version.as_ref()
                    })
                    .max_by_key(|receipt| receipt.observed_at)
                    .is_some_and(|receipt| receipt.success)
            })
    }
}
