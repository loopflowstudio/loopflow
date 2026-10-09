//! Immutable planning mutations, not execution records. Causality precedes clocks;
//! concurrent Linear observations win, otherwise the latest clock/identity wins.
//! Losing values stay in the document and are available for recovery.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PlanningKind {
    Wave,
    Project,
    Task,
    Comment,
}

impl PlanningKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wave => "wave",
            Self::Project => "project",
            Self::Task => "task",
            Self::Comment => "comment",
        }
    }

    /// The portable allowlist deliberately excludes execution, paths and controls.
    pub fn fields(self) -> &'static [&'static str] {
        match self {
            Self::Wave => &["name", "parent_wave_id", "current_project_id"],
            Self::Project => &[
                "wave_id",
                "external_project_id",
                "project_slug",
                "project_name",
                "project_summary",
                "project_prompt_context",
                "workflow",
                "status",
                "planning_rank",
                "planning_initiatives",
                "planning_teams",
            ],
            Self::Task => &[
                "project_id",
                "external_issue_id",
                "issue_identifier",
                "issue_title",
                "issue_description",
                "planning_rank",
                "planning_assignee",
                "disposition",
                "planning_deleted_at",
                "planning_url",
                "planning_branch_name",
                "planning_team_id",
            ],
            Self::Comment => &["task_id", "content"],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningObject {
    pub kind: PlanningKind,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningMutation {
    pub object: PlanningObject,
    pub field: String,
    pub value: Value,
    /// Hybrid logical milliseconds, advanced beyond every observed mutation.
    pub clock: i64,
    pub linear: bool,
    /// Observed heads of this field, not a global revision or Git ancestry.
    pub parents: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningSnapshot {
    pub changes: BTreeMap<String, PlanningMutation>,
}

impl PlanningSnapshot {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PlanningExchangeError> {
        let snapshot: Self = serde_json::from_slice(bytes)?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, PlanningExchangeError> {
        self.validate()?;
        Ok(serde_json::to_vec(self)?)
    }

    /// Omission is never deletion. Reused identities reject the document without
    /// replacing either original; independent writes need distinct identities.
    pub fn merge(&self, incoming: &Self) -> Result<Self, PlanningExchangeError> {
        self.validate()?;
        incoming.validate()?;
        let mut merged = self.clone();
        for (id, change) in &incoming.changes {
            if merged.changes.get(id).is_some_and(|saved| saved != change) {
                return Err(PlanningExchangeError::ReusedChange(id.clone()));
            }
            merged.changes.insert(id.clone(), change.clone());
        }
        Ok(merged)
    }

    pub fn heads(&self) -> BTreeMap<(PlanningObject, String), BTreeSet<String>> {
        let retired: BTreeSet<_> = self.changes.values().flat_map(|c| &c.parents).collect();
        let mut heads: BTreeMap<_, BTreeSet<_>> = BTreeMap::new();
        for (id, change) in &self.changes {
            if !retired.contains(id) {
                heads
                    .entry((change.object.clone(), change.field.clone()))
                    .or_default()
                    .insert(id.clone());
            }
        }
        heads
    }

    pub(crate) fn objects(&self) -> BTreeSet<&PlanningObject> {
        self.changes.values().map(|change| &change.object).collect()
    }

    pub fn resolved(&self) -> BTreeMap<PlanningObject, BTreeMap<String, Value>> {
        let mut objects: BTreeMap<_, BTreeMap<_, _>> = BTreeMap::new();
        for ((object, field), heads) in self.heads() {
            // Causal successors retire predecessors regardless of origin/clock.
            let winner = heads
                .iter()
                .max_by_key(|id| {
                    let change = &self.changes[*id];
                    (change.linear, change.clock, *id)
                })
                .expect("a field has at least one head");
            objects
                .entry(object)
                .or_default()
                .insert(field, self.changes[winner].value.clone());
        }
        objects
    }

    pub fn validate(&self) -> Result<(), PlanningExchangeError> {
        for (id, change) in &self.changes {
            if id.is_empty()
                || change.object.id.is_empty()
                || change.clock < 0
                || !change.object.kind.fields().contains(&change.field.as_str())
            {
                return Err(PlanningExchangeError::Invalid("invalid planning mutation"));
            }
            validate_value(change)?;
            for parent in &change.parents {
                let previous = self
                    .changes
                    .get(parent)
                    .ok_or(PlanningExchangeError::Invalid(
                        "missing planning predecessor",
                    ))?;
                if previous.object != change.object
                    || previous.field != change.field
                    || previous.clock >= change.clock
                {
                    return Err(PlanningExchangeError::Invalid("invalid planning causality"));
                }
            }
        }
        Ok(())
    }
}

fn validate_value(change: &PlanningMutation) -> Result<(), PlanningExchangeError> {
    let value = &change.value;
    let text = |value: &Value| value.is_null() || value.is_string();
    let valid = match change.field.as_str() {
        "planning_rank" => value.as_u64().is_some_and(|rank| rank <= u32::MAX as u64),
        "planning_deleted_at" => value.is_null() || value.as_i64().is_some_and(|at| at >= 0),
        "planning_initiatives" | "planning_teams" => value
            .as_str()
            .is_some_and(|text| serde_json::from_str::<Vec<String>>(text).is_ok()),
        "disposition" => value.as_object().is_some_and(|group| {
            group.len() == 3
                && group.get("planning_state").is_some_and(text)
                && group.get("planning_completed_at").is_some_and(text)
                && group
                    .get("planning_completed")
                    .is_some_and(|v| matches!(v.as_i64(), Some(0 | 1)))
        }),
        "content" => value.as_object().is_some_and(|group| {
            group.len() == 3
                && group.get("body").is_some_and(Value::is_string)
                && group
                    .get("author")
                    .and_then(Value::as_str)
                    .is_some_and(|a| serde_json::from_str::<Value>(a).is_ok())
                && group.get("created_at").is_some_and(text)
        }),
        _ => text(value),
    };
    if valid {
        Ok(())
    } else {
        Err(PlanningExchangeError::Invalid(
            "invalid planning field value",
        ))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PlanningExchangeError {
    #[error("invalid planning document: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("planning change {0} has conflicting contents; retain the original write")]
    ReusedChange(String),
}
