//! Portable planning only: no checkout, Workflow, Session or process fields.
//!
//! Each field retains its current alternatives and the identities of superseded
//! writes. Merging unions that causal context, so a delayed completion cannot
//! replace a reopening that observed it. Concurrent writes remain alternatives;
//! resolving them is a new write after observing all alternatives. No clock or
//! Git merge-base selects a winner. Superseded values remain in Git history.
//!
//! The common local writer must persist these change identities and causal context
//! atomically with mutations/imports. Export must never mint a new identity for
//! a retry. This module performs no storage, execution or provider effects.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PlanningChangeId(String);

impl PlanningChangeId {
    pub fn new(id: String) -> Result<Self, PlanningExchangeError> {
        if id.trim().is_empty() {
            return Err(PlanningExchangeError::Invalid(
                "empty planning change identity",
            ));
        }
        Ok(Self(id))
    }
}

/// A causal field. Multiple distinct values are an explicit unresolved conflict.
/// Retired identities are retained indefinitely; omission is not retirement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningField<T: Ord> {
    current: BTreeMap<PlanningChangeId, BTreeSet<T>>,
    retired: BTreeSet<PlanningChangeId>,
}

impl<T: Clone + Ord> PlanningField<T> {
    pub fn new(change: PlanningChangeId, value: T) -> Self {
        Self {
            current: BTreeMap::from([(change, BTreeSet::from([value]))]),
            retired: BTreeSet::new(),
        }
    }

    pub fn values(&self) -> BTreeSet<&T> {
        self.current.values().flatten().collect()
    }

    /// None means conflicting values, never an absent/default planning value.
    pub fn resolved(&self) -> Option<&T> {
        let mut values = self.current.values().flatten();
        let first = values.next()?;
        values.all(|value| value == first).then_some(first)
    }

    /// A writer can replay its current write, but cannot reuse a past identity.
    pub fn write(
        &mut self,
        change: PlanningChangeId,
        value: T,
    ) -> Result<(), PlanningExchangeError> {
        let current = BTreeMap::from([(change.clone(), BTreeSet::from([value]))]);
        if self.current == current {
            return Ok(());
        }
        if self.retired.contains(&change) || self.current.contains_key(&change) {
            return Err(PlanningExchangeError::ReusedChange);
        }
        let previous = std::mem::replace(&mut self.current, current);
        self.retired.extend(previous.into_keys());
        Ok(())
    }

    pub fn merge(&self, incoming: &Self) -> Self {
        let retired: BTreeSet<_> = self.retired.union(&incoming.retired).cloned().collect();
        let mut current = self.current.clone();
        for (change, values) in &incoming.current {
            current
                .entry(change.clone())
                .or_default()
                .extend(values.iter().cloned());
        }
        current.retain(|change, _| !retired.contains(change));
        Self { current, retired }
    }

    fn validate(&self) -> Result<(), PlanningExchangeError> {
        if self.current.is_empty()
            || self.current.values().any(BTreeSet::is_empty)
            || self.current.keys().any(|id| self.retired.contains(id))
            || self
                .current
                .keys()
                .chain(&self.retired)
                .any(|id| id.0.trim().is_empty())
        {
            return Err(PlanningExchangeError::Invalid(
                "invalid planning field causal context",
            ));
        }
        Ok(())
    }
}

/// Wave and Project move together so reconciliation cannot invent a pairing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningMembership {
    pub wave_id: String,
    pub project_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningIssue {
    pub id: String,
    pub identifier: String,
}

/// Shared planning disposition; importing it must not move a local Workflow.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case", deny_unknown_fields)]
#[non_exhaustive]
pub enum PlanningDisposition {
    Open,
    Completed { summary: String },
    Abandoned { reason: String },
    Deleted,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningComment {
    /// None preserves unresolved attribution, without inferring a machine owner.
    pub author: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PortableTask {
    pub title: PlanningField<String>,
    pub brief: PlanningField<String>,
    pub membership: PlanningField<PlanningMembership>,
    pub issue: PlanningField<Option<PlanningIssue>>,
    pub disposition: PlanningField<PlanningDisposition>,
    /// Immutable comment IDs deduplicate delivery; conflicting bytes stay explicit.
    pub comments: BTreeMap<String, BTreeSet<PlanningComment>>,
}

impl PortableTask {
    pub fn merge(&self, incoming: &Self) -> Self {
        let mut comments = self.comments.clone();
        for (id, values) in &incoming.comments {
            comments
                .entry(id.clone())
                .or_default()
                .extend(values.iter().cloned());
        }
        Self {
            title: self.title.merge(&incoming.title),
            brief: self.brief.merge(&incoming.brief),
            membership: self.membership.merge(&incoming.membership),
            issue: self.issue.merge(&incoming.issue),
            disposition: self.disposition.merge(&incoming.disposition),
            comments,
        }
    }

    fn validate(&self) -> Result<(), PlanningExchangeError> {
        self.title.validate()?;
        self.brief.validate()?;
        self.membership.validate()?;
        self.issue.validate()?;
        self.disposition.validate()?;
        if self
            .comments
            .iter()
            .any(|(id, values)| id.trim().is_empty() || values.is_empty())
            || self.membership.values().iter().any(|membership| {
                membership.wave_id.trim().is_empty()
                    || membership
                        .project_id
                        .as_ref()
                        .is_some_and(|id| id.trim().is_empty())
            })
            || self.issue.values().iter().any(|issue| {
                issue.as_ref().is_some_and(|issue| {
                    issue.id.trim().is_empty() || issue.identifier.trim().is_empty()
                })
            })
        {
            return Err(PlanningExchangeError::Invalid(
                "invalid portable planning identity",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningSnapshot {
    /// Existing durable Task IDs, never IDs inferred from issue names at import.
    pub tasks: BTreeMap<String, PortableTask>,
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

    /// Pure reconciliation. Unknown/omitted Tasks and comments always survive.
    pub fn merge(&self, incoming: &Self) -> Self {
        let mut tasks = self.tasks.clone();
        for (id, task) in &incoming.tasks {
            tasks
                .entry(id.clone())
                .and_modify(|existing| *existing = existing.merge(task))
                .or_insert_with(|| task.clone());
        }
        Self { tasks }
    }

    fn validate(&self) -> Result<(), PlanningExchangeError> {
        for (id, task) in &self.tasks {
            if id.trim().is_empty() {
                return Err(PlanningExchangeError::Invalid(
                    "empty portable Task identity",
                ));
            }
            task.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PlanningExchangeError {
    #[error("invalid planning document: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(&'static str),
    #[error("planning change identity was already used; retain the original write")]
    ReusedChange,
}
