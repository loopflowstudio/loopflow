//! Durable planning facts for Project and Task Work.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanningError {
    #[error("invalid Linear id: {0}")]
    InvalidId(String),
}

macro_rules! validated_string_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, PlanningError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(PlanningError::InvalidId(format!(
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
pub struct ProjectPlan {
    pub id: LinearProjectId,
    pub slug: String,
    pub name: String,
    /// Definition and proof-shaped KRs from the latest PM snapshot.
    pub prompt_context: String,
    pub pm_snapshot_synced_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPlan {
    pub id: LinearIssueId,
    pub identifier: String,
    pub title: String,
    pub description: String,
    pub pm_snapshot_synced_at: i64,
}
