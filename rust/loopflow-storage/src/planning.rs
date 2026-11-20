//! Retained tracker planning records and their pure syntax.
use super::{StoreError, StoreResult};
use crate::work::project::ProjectStatus;
use loopflow_model::comment::{TaskComment, TaskCommentAuthor};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LinearBindingError {
    #[error("invalid Linear id: {0}")]
    InvalidId(String),
    #[error("planning record has no Linear mapping")]
    NoLinearMapping,
}

macro_rules! validated_string_id {
    ($name:ident, $label:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, schemars::JsonSchema)]
        #[serde(transparent)]
        pub struct $name(String);

        impl<'de> serde::Deserialize<'de> for $name {
            fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let value = <String as serde::Deserialize>::deserialize(deserializer)?;
                Ok(Self::from_raw(value))
            }
        }

        impl $name {
            pub fn new(value: impl Into<String>) -> Result<Self, LinearBindingError> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(LinearBindingError::InvalidId(format!(
                        "{} cannot be empty",
                        $label
                    )));
                }
                Ok(Self(loopflow_model::id::canonical_uuid(&value)))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            pub fn from_raw(value: impl Into<String>) -> Self {
                Self(loopflow_model::id::canonical_uuid(&value.into()))
            }
        }
    };
}

validated_string_id!(LinearIssueId, "Linear issue id");
validated_string_id!(LinearProjectId, "Linear project id");

pub fn terminal_reason(state: Option<&str>) -> Option<&'static str> {
    match state {
        Some("canceled") => Some("Linear Task is canceled"),
        Some("duplicate") => Some("Linear Task is duplicate"),
        Some("completed") => Some("Linear Task is complete"),
        _ => None,
    }
}

pub fn project_slug(name: &str) -> String {
    let mut slug = String::new();
    let mut separator = false;
    for character in name.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            if separator && !slug.is_empty() {
                slug.push('-');
            }
            slug.push(character);
            separator = false;
        } else if !slug.is_empty() {
            separator = true;
        }
    }
    slug
}

/// Provider evidence about an issue's state since a delivery's base revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StateHistory {
    /// No evidence was read; an uncertain write stays uncertain.
    Unknown,
    /// Only unrelated edits followed the base: an earlier write had no effect.
    Unchanged,
    /// The state left its base: a write landed, or someone moved the issue.
    Changed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackerItem {
    /// Provider revision; absent only in historical planning.
    pub revision: Option<String>,
    #[serde(deserialize_with = "loopflow_model::id::deserialize_uuid")]
    pub id: String,
    #[serde(deserialize_with = "loopflow_model::id::deserialize_task_identifier")]
    pub identifier: String,
    /// Provider branch name used to find existing Git work.
    pub branch_name: Option<String>,
    /// Provider-owned issue URL captured during PM sync. `None` stays explicit
    /// when the tracker did not return one; status and roadmap reads never
    /// fetch it on demand.
    pub url: Option<String>,
    pub name: String,
    pub description: String,
    pub rank: u32,
    pub completed: bool,
    pub completed_at: Option<String>,
    pub due_date: Option<String>,
    /// Provider workflow category; absent in historical snapshots.
    pub state: Option<String>,
    /// Stable owning Project id. Task-to-Wave resolution follows this edge.
    pub project_id: Option<String>,
    /// Stable owning repository Team id.
    pub team_id: Option<String>,
    /// Provider user ID of the assignee, if any.
    pub assignee: Option<String>,
}

/// Asana has no short identifiers; their Tasks keep the local
/// `lf-` identifier Loopflow assigned.
pub fn local_identifier(id: &str) -> String {
    uuid::Uuid::parse_str(id)
        .map(|uuid| format!("lf-{}", uuid.simple()))
        .unwrap_or_else(|_| id.to_string())
}

impl TrackerItem {
    /// Linear calls the item complete, not canceled or duplicate.
    pub fn is_complete(&self) -> bool {
        match self.state.as_deref() {
            Some(state) => state == "completed",
            None => self.completed,
        }
    }
}

/// A read of one issue's editable content plus its comments, taken to
/// stream Linear edits into a Task. `revision` is the tracker's
/// last-updated marker (Linear `updatedAt`), monotonic per issue, and is
/// compared — not trusted as identity — so out-of-order responses never move
/// the cursor backward.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackerEvent {
    pub revision: String,
    pub title: String,
    pub description: String,
    pub comments: Vec<TrackerComment>,
}

/// Linear's comment and attribution metadata, retained unchanged in conflict receipts.
/// A tracker user or an explicit requester steer supplies person attribution;
/// an integration's display name alone does not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrackerComment {
    #[serde(deserialize_with = "loopflow_model::id::deserialize_uuid")]
    pub id: String,
    /// Provider creation time; display order. `revision` orders direction.
    pub created_at: Option<String>,
    pub revision: Option<String>,
    pub body: String,
    pub author_id: Option<String>,
    /// Tracker display name for attribution, independent of the tracker user ID.
    pub author_name: Option<String>,
}

impl From<&TrackerComment> for TaskComment {
    fn from(comment: &TrackerComment) -> Self {
        let author =
            if comment.author_id.is_some() || loopflow_model::comment::is_steer(&comment.body) {
                TaskCommentAuthor::Person {
                    name: loopflow_model::comment::comment_requester(
                        &comment.body,
                        comment.author_name.as_deref(),
                    ),
                }
                .read(&comment.body)
            } else {
                TaskCommentAuthor::Integration
            };
        Self {
            id: comment.id.clone(),
            body: comment.body.clone(),
            author,
            created_at: comment.created_at.clone(),
        }
    }
}

/// Retained Linear facts, including foreign and not-yet-associated Projects.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackerProject {
    #[serde(deserialize_with = "loopflow_model::id::deserialize_uuid")]
    pub id: String,
    pub revision: Option<String>,
    pub slug: String,
    pub name: String,
    pub summary: String,
    /// None means historical/unobserved prose, never an observed empty value.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: ProjectStatus,
    #[serde(deserialize_with = "loopflow_model::id::deserialize_uuid_vec")]
    pub initiative_ids: Vec<String>,
    /// Stable ids of the Linear teams this Project belongs to. A managed Project
    /// must carry exactly the repository Team.
    #[serde(deserialize_with = "loopflow_model::id::deserialize_uuid_vec")]
    pub team_ids: Vec<String>,
}
pub fn revision_nanos(revision: Option<&str>) -> StoreResult<Option<i64>> {
    revision
        .map(|revision| {
            time::OffsetDateTime::parse(revision, &time::format_description::well_known::Rfc3339)
                .map_err(|error| {
                    StoreError::InvalidData(format!("invalid planning revision: {error}"))
                })
                .and_then(|time| {
                    i64::try_from(time.unix_timestamp_nanos()).map_err(|error| {
                        StoreError::InvalidData(format!("planning revision out of range: {error}"))
                    })
                })
        })
        .transpose()
}
pub mod project_document;
pub mod rules;

#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub struct ProjectDocumentError(pub String);
