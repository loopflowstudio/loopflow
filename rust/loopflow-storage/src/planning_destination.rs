use sha2::{Digest, Sha256};
use std::path::Path;

#[derive(Debug, thiserror::Error)]
pub enum PlanningGitError {
    #[error("planning Git {operation} failed (exit {code:?}); local planning is retained")]
    Command {
        operation: &'static str,
        code: Option<i32>,
    },
    #[error("planning Git {0} timed out; publication may need readback")]
    Timeout(&'static str),
    #[error("planning Git I/O: {0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("local planning changed while saving; reconcile with its retained revision")]
    ConcurrentWrite,
}

type Result<T> = std::result::Result<T, PlanningGitError>;

/// A pinned endpoint and ref. Persist this value, not the remote alias used to
/// select it. Ref separation is not access control.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct PlanningDestination {
    endpoint: String,
    reference: String,
}

impl PlanningDestination {
    pub fn new(endpoint: &str, reference: &str) -> Result<Self> {
        if endpoint.is_empty() || endpoint.starts_with('-') || endpoint.contains(['\n', '\r']) {
            return Err(PlanningGitError::Invalid("select a planning Git endpoint"));
        }
        let suffix =
            reference
                .strip_prefix("refs/loopflow/planning/")
                .ok_or(PlanningGitError::Invalid(
                    "select a user-keyed or shared planning ref",
                ))?;
        let valid = if let Some(key) = suffix.strip_prefix("users/") {
            uuid::Uuid::parse_str(key).is_ok_and(|id| id.to_string() == key)
        } else if let Some(key) = suffix.strip_prefix("shared/") {
            !key.is_empty()
                && key
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'-' | b'_'))
        } else {
            false
        };
        if !valid {
            return Err(PlanningGitError::Invalid(
                "invalid user-keyed or shared planning ref",
            ));
        }

        // Local destinations must not change meaning when another checkout opens.
        if !endpoint.contains(':') && !Path::new(endpoint).is_absolute() {
            return Err(PlanningGitError::Invalid(
                "planning local endpoint must be absolute",
            ));
        }
        Ok(Self {
            endpoint: endpoint.into(),
            reference: reference.into(),
        })
    }

    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }
    pub fn reference(&self) -> &str {
        &self.reference
    }
    pub fn id(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(format!("{}\0{}", self.endpoint, self.reference))
        )
    }
}
