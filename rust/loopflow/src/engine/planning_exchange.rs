//! Immutable planning mutations, not execution records. Causality precedes clocks;
//! concurrent values use the latest clock/identity. Linear uses its own transport.
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
    pub fn fields(self) -> Vec<&'static str> {
        crate::store::sqlite::planning_write::fields(self)
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
            let saved = merged
                .changes
                .entry(id.clone())
                .or_insert_with(|| change.clone());
            if saved != change {
                return Err(PlanningExchangeError::ReusedChange(id.clone()));
            }
        }
        merged.validate()?;
        Ok(merged)
    }

    /// Head mutations in change-ID order, borrowed from the retained journal.
    pub fn heads(&self) -> impl Iterator<Item = (&str, &PlanningMutation)> {
        let retired: BTreeSet<_> = self
            .changes
            .values()
            .flat_map(|change| {
                change.parents.iter().filter(|id| {
                    self.changes
                        .get(*id)
                        .is_some_and(|parent| parent.object == change.object)
                })
            })
            .collect();
        self.changes
            .iter()
            .filter(move |(id, _)| !retired.contains(id))
            .map(|(id, change)| (id.as_str(), change))
    }

    /// Select current values while retaining every competing mutation.
    pub fn winners(&self) -> impl Iterator<Item = (&str, &PlanningMutation)> {
        winning_heads(self.heads())
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

/// Select within an already evaluated frontier. Import and accepted local
/// observations share this policy without reconstructing a partial journal.
pub(crate) fn winning_heads<'a>(
    heads: impl IntoIterator<Item = (&'a str, &'a PlanningMutation)>,
) -> impl Iterator<Item = (&'a str, &'a PlanningMutation)> {
    let mut fields = BTreeMap::new();
    for (id, change) in heads {
        let priority = (change.clock, id);
        let winner = fields
            .entry((&change.object, change.field.as_str()))
            .or_insert((priority, (id, change)));
        if priority > winner.0 {
            *winner = (priority, (id, change));
        }
    }
    fields.into_values().map(|(_, winner)| winner)
}

fn validate_value(change: &PlanningMutation) -> Result<(), PlanningExchangeError> {
    crate::store::sqlite::planning_write::PlanningEdit::from_value(
        change.object.kind,
        &change.field,
        change.value.clone(),
    )
    .map(|_| ())
    .map_err(|_| PlanningExchangeError::Invalid("invalid planning field value"))
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
