//! Versioned, immutable inputs for replaying one recorded provider invocation.

use std::collections::BTreeMap;

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::trace::ContextAsset;

pub const REPLAY_CONTRACT_SCHEMA_VERSION: u32 = 1;
pub const EXECUTION_CONTRACT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArtifactReferenceV1 {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContextManifestIdentityV1 {
    pub sha256: String,
    pub asset_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReplayTurnV1 {
    pub turn_id: String,
    pub ordinal: u32,
    pub input_op: String,
    pub timing: String,
    pub system_prompt: Option<ArtifactReferenceV1>,
    pub task_prompt: ArtifactReferenceV1,
    pub context_manifest: ContextManifestIdentityV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConversationReferenceV1 {
    pub path: String,
    pub sha256: String,
    pub trace_schema_version: u32,
    pub event_count: u64,
    pub bytes: u64,
}

/// Producer-finalized contract for one complete AgentInvocation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReplayContractV1 {
    pub schema_version: u32,
    pub invocation_id: String,
    pub home_id: String,
    pub wave: Option<String>,
    pub project: Option<String>,
    pub task: Option<String>,
    pub flow: Option<String>,
    pub skill: Option<String>,
    pub execution_contract: ArtifactReferenceV1,
    pub turns: Vec<ReplayTurnV1>,
    pub conversation: ConversationReferenceV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RepositoryExecutionV1 {
    pub root: String,
    pub commit: String,
    pub clean: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LocalFileIdentityV1 {
    pub path: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProviderExecutionV1 {
    pub provider: String,
    pub model: String,
    pub account_id: String,
    pub binary: LocalFileIdentityV1,
    pub config_files: Vec<LocalFileIdentityV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StructuredReplyV1 {
    pub name: String,
    pub description: String,
    pub guidance: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AgentConfigV1 {
    pub agent: String,
    pub max_turns: Option<u32>,
    pub cwd: String,
    pub run_context: String,
    pub permission_policy: String,
    pub write_scope: String,
    pub writable_roots: Vec<String>,
    pub network_access: bool,
    pub skip_permissions: bool,
    pub structured_replies: Vec<StructuredReplyV1>,
    pub directive_relay: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProcessConfigV1 {
    pub surface: String,
    pub unattended: bool,
    pub stream: bool,
    pub stream_format: String,
    pub timeout_ms: Option<u64>,
}

/// Exact pre-launch value from which a provider process was constructed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ExecutionContractV1 {
    pub schema_version: u32,
    pub invocation_id: String,
    pub home_id: String,
    pub repository: RepositoryExecutionV1,
    pub provider: ProviderExecutionV1,
    pub agent: AgentConfigV1,
    pub process: ProcessConfigV1,
    pub sanitized_argv: Vec<String>,
    pub environment_selectors: BTreeMap<String, String>,
    pub initial_turn: ReplayTurnV1,
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ReplayArtifactError {
    #[error("unsupported schema version: {0}")]
    UnsupportedSchema(u64),
    #[error("invalid replay artifact: {0}")]
    Invalid(String),
}

pub fn read_replay_contract(bytes: &[u8]) -> Result<ReplayContractV1, ReplayArtifactError> {
    _read_versioned(bytes, REPLAY_CONTRACT_SCHEMA_VERSION)
}

pub fn read_execution_contract(bytes: &[u8]) -> Result<ExecutionContractV1, ReplayArtifactError> {
    _read_versioned(bytes, EXECUTION_CONTRACT_SCHEMA_VERSION)
}

pub fn sha256_bytes(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

pub fn context_manifest_sha256(assets: &[ContextAsset]) -> String {
    let bytes = serde_json::to_vec(assets).expect("ContextAsset always serializes");
    sha256_bytes(&bytes)
}

pub fn is_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn _read_versioned<T: DeserializeOwned>(
    bytes: &[u8],
    supported: u32,
) -> Result<T, ReplayArtifactError> {
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|error| ReplayArtifactError::Invalid(error.to_string()))?;
    let version = value
        .get("schema_version")
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            ReplayArtifactError::Invalid("schema_version must be an unsigned integer".to_string())
        })?;
    if version != u64::from(supported) {
        return Err(ReplayArtifactError::UnsupportedSchema(version));
    }
    serde_json::from_value(value).map_err(|error| ReplayArtifactError::Invalid(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versioned_reader_distinguishes_unknown_from_malformed_contracts() {
        let unsupported = read_replay_contract(br#"{"schema_version":2}"#).unwrap_err();
        assert!(matches!(
            unsupported,
            ReplayArtifactError::UnsupportedSchema(2)
        ));

        let malformed = read_replay_contract(br#"{"schema_version":1}"#).unwrap_err();
        assert!(matches!(malformed, ReplayArtifactError::Invalid(_)));
    }
}
