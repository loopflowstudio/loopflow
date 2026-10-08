//! Durable planning facts for Project and Task Work.

use serde::{Deserialize, Serialize};

/// A pending field edit and its provider baseline, independent of connection state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningChange {
    pub id: String,
    pub field: String,
    pub value: serde_json::Value,
    pub base: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PlanningError {
    #[error("invalid Linear id: {0}")]
    InvalidId(String),
    #[error("planning record has no Linear mapping")]
    NoLinearMapping,
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
    pub summary: String,
    /// Optional provider mapping; the owning Project carries durable identity.
    pub linear_id: Option<LinearProjectId>,
    pub slug: String,
    pub name: String,
    /// Definition and proof-shaped KRs from the latest PM snapshot.
    pub prompt_context: String,
    pub pm_snapshot_synced_at: Option<i64>,
    pub workflow: String,
    pub status: crate::pm::ProjectStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskPlan {
    /// Local optimistic-write revision, independent of provider observation age.
    pub revision: u64,
    /// Optional provider mapping; the owning Task carries durable identity.
    pub linear_id: Option<LinearIssueId>,
    pub identifier: String,
    pub title: String,
    pub description: String,
    pub pm_snapshot_synced_at: Option<i64>,
}

impl TaskPlan {
    pub fn linear_id(&self) -> Result<&LinearIssueId, PlanningError> {
        self.linear_id
            .as_ref()
            .ok_or(PlanningError::NoLinearMapping)
    }
}

impl ProjectPlan {
    pub fn linear_id(&self) -> Result<&LinearProjectId, PlanningError> {
        self.linear_id
            .as_ref()
            .ok_or(PlanningError::NoLinearMapping)
    }
}

/// The caller retains this identity across a retry; a separate create mints another.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewTask {
    pub id: crate::durable::TaskId,
    pub project_id: crate::durable::ProjectId,
    pub title: String,
    pub description: String,
}
