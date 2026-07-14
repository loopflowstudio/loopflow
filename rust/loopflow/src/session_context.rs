//! Linear context captured when a Project or Task Session starts.
//!
//! These snapshots make a child resumable without another provider read. They
//! are launch facts, not a second mutable PM model; current Project/KR/Task
//! truth remains in the Wave's atomic PM snapshot.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SessionContextError {
    #[error("invalid Linear id: {0}")]
    InvalidId(String),
}

macro_rules! validated_string_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, SessionContextError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(SessionContextError::InvalidId(format!(
                        "{} cannot be empty",
                        $label
                    )));
                }
                Ok(Self(value))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub(crate) fn from_raw(value: impl Into<String>) -> Self {
                Self(value.into())
            }
        }
    };
}

validated_string_id!(LinearIssueId, "Linear issue id");
validated_string_id!(LinearProjectId, "Linear project id");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinearIssueSnapshot {
    pub id: LinearIssueId,
    pub identifier: String,
    pub title: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinearProjectSnapshot {
    pub id: LinearProjectId,
    pub slug: String,
    pub name: String,
    /// Definition and proof-shaped KRs captured when the child Session starts.
    pub context: String,
}
