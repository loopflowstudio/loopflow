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
                "workflow",
                "krs",
                "metric_targets",
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

/// The provider fact that justified a mutation, including its original acquisition
/// time. Peer receipt time and Git ancestry cannot establish provider freshness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LinearObservation {
    pub body: Value,
    pub observed_at: i64,
}

impl LinearObservation {
    pub fn revision(&self) -> Option<&str> {
        self.body["revision"].as_str()
    }

    pub(crate) fn revision_time(&self) -> Option<i128> {
        self.revision().and_then(|revision| {
            time::OffsetDateTime::parse(revision, &time::format_description::well_known::Rfc3339)
                .ok()
                .map(|time| time.unix_timestamp_nanos())
        })
    }

    /// Fields with entity-revision ordering. Membership's local IDs are resolved
    /// by the Store; Project relationships and list rank have separate frontiers.
    pub(crate) fn fields(&self, kind: PlanningKind) -> Result<Value, serde_json::Error> {
        Ok(match kind {
            PlanningKind::Task => {
                let item: crate::pm::PmItem = serde_json::from_value(self.body.clone())?;
                serde_json::json!({
                    "external_issue_id":item.id,"issue_identifier":item.identifier,
                    "issue_title":item.name,"issue_description":item.description,
                    "planning_assignee":item.assignee,
                    "disposition":{"planning_state":item.state,"planning_completed":i32::from(item.completed),"planning_completed_at":item.completed_at},
                    "planning_url":item.url,"planning_branch_name":item.branch_name,"planning_team_id":item.team_id,
                })
            }
            PlanningKind::Project => {
                let project: crate::pm::PmProject = serde_json::from_value(self.body.clone())?;
                serde_json::json!({
                    "external_project_id":project.id,"project_slug":project.slug,"project_name":project.name,
                    "project_summary":project.summary,"workflow":project.workflow,"status":project.status.as_str(),
                    "krs":project.krs,"metric_targets":project.metric_targets,
                })
            }
            PlanningKind::Comment => {
                let observed: crate::pm::IssueComment = serde_json::from_value(self.body.clone())?;
                let comment = crate::ops::pm::TaskComment::from(&observed);
                serde_json::json!({"content":{
                    "body":comment.body,"author":serde_json::to_string(&comment.author)?,
                    "created_at":comment.created_at,
                }})
            }
            PlanningKind::Wave => serde_json::json!({}),
        })
    }

    fn validate(&self, kind: PlanningKind) -> Result<(), PlanningExchangeError> {
        if self.observed_at < 0
            || self.body["id"].as_str().is_none_or(str::is_empty)
            || self
                .body
                .get("revision")
                .is_none_or(|value| !value.is_null() && !value.is_string())
            || (self.revision().is_some() && self.revision_time().is_none())
        {
            return Err(PlanningExchangeError::Invalid("invalid Linear observation"));
        }
        // Round trips reject extra payload, including local execution or paths.
        let body = match kind {
            PlanningKind::Task => serde_json::to_value(
                serde_json::from_value::<crate::pm::PmItem>(self.body.clone())?,
            )?,
            PlanningKind::Project => serde_json::to_value(serde_json::from_value::<
                crate::pm::PmProject,
            >(self.body.clone())?)?,
            PlanningKind::Comment => serde_json::to_value(serde_json::from_value::<
                crate::pm::IssueComment,
            >(self.body.clone())?)?,
            PlanningKind::Wave => {
                return Err(PlanningExchangeError::Invalid(
                    "Wave has no Linear observation",
                ))
            }
        };
        if body != self.body {
            return Err(PlanningExchangeError::Invalid(
                "unexpected Linear observation fields",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningMutation {
    pub object: PlanningObject,
    pub field: String,
    pub value: Value,
    /// Hybrid logical milliseconds, advanced beyond every observed mutation.
    pub clock: i64,
    pub linear: Option<LinearObservation>,
    /// Observed heads of this field, not a global revision or Git ancestry.
    pub parents: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PlanningSnapshot {
    pub changes: BTreeMap<String, PlanningMutation>,
}

impl PlanningMutation {
    pub(crate) fn provider_evidence(&self) -> bool {
        matches!(
            (self.object.kind, self.field.as_str()),
            (
                PlanningKind::Task,
                "provider_removal" | "provider_invalidation"
            ) | (PlanningKind::Project, "provider_archive" | "provider_teams")
        )
    }

    pub(crate) fn order_receipt(&self) -> Option<&str> {
        (self.object.kind == PlanningKind::Project)
            .then(|| {
                self.field
                    .strip_prefix("order:")
                    .filter(|id| !id.is_empty())
            })
            .flatten()
    }

    pub(crate) fn deletion_receipt(&self) -> Option<&str> {
        if self.object.kind != PlanningKind::Task {
            return None;
        }
        self.field
            .strip_prefix("deletion:")
            .filter(|id| !id.is_empty())
    }
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
        Ok(merged)
    }

    /// Head mutations in change-ID order, borrowed from the retained journal.
    pub fn heads(&self) -> impl Iterator<Item = (&str, &PlanningMutation)> {
        let retired: BTreeSet<_> = self.changes.values().flat_map(|c| &c.parents).collect();
        self.changes
            .iter()
            .filter(move |(id, _)| !retired.contains(id))
            .map(|(id, change)| (id.as_str(), change))
    }

    pub(crate) fn objects(&self) -> BTreeSet<&PlanningObject> {
        self.changes.values().map(|change| &change.object).collect()
    }

    /// Keep winner identity and origin available to delivery projection. Values
    /// alone cannot distinguish a local intention from an observed Linear fact.
    pub fn winners(&self) -> impl Iterator<Item = (&str, &PlanningMutation)> {
        let mut fields = BTreeMap::new();
        for (id, change) in self.heads() {
            let priority = (
                change.linear.is_some(),
                change
                    .linear
                    .as_ref()
                    .and_then(LinearObservation::revision_time),
                change.clock,
                id,
            );
            let winner = fields
                .entry((&change.object, change.field.as_str()))
                .or_insert((priority, (id, change)));
            if priority > winner.0 {
                *winner = (priority, (id, change));
            }
        }
        fields.into_values().map(|(_, winner)| winner)
    }

    pub fn validate(&self) -> Result<(), PlanningExchangeError> {
        for (id, change) in &self.changes {
            if id.is_empty()
                || change.object.id.is_empty()
                || change.clock < 0
                || !(change.object.kind.fields().contains(&change.field.as_str())
                    || (matches!(
                        change.object.kind,
                        PlanningKind::Task | PlanningKind::Project
                    ) && change.field == "creation")
                    || change.deletion_receipt().is_some()
                    || change.order_receipt().is_some()
                    || change.provider_evidence())
            {
                return Err(PlanningExchangeError::Invalid("invalid planning mutation"));
            }
            validate_value(change)?;
            if let Some(observation) = &change.linear {
                observation.validate(change.object.kind)?;
                if change.object.kind == PlanningKind::Comment
                    && observation.body["id"].as_str() != Some(change.object.id.as_str())
                {
                    return Err(PlanningExchangeError::Invalid(
                        "comment observation identity mismatch",
                    ));
                }
                let fields = observation.fields(change.object.kind)?;
                if let Some(value) = fields.get(&change.field) {
                    if value != &change.value {
                        return Err(PlanningExchangeError::Invalid(
                            "mutation disagrees with its Linear observation",
                        ));
                    }
                } else if !matches!(
                    (change.object.kind, change.field.as_str()),
                    (PlanningKind::Task, "project_id") | (PlanningKind::Comment, "task_id")
                ) {
                    return Err(PlanningExchangeError::Invalid(
                        "field has no Linear entity frontier",
                    ));
                }
            }
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
    if change.provider_evidence() {
        return crate::store::sqlite::planning::ProviderEvidence::validate(
            change.object.kind,
            &change.field,
            value,
        )
        .map(|_| ())
        .map_err(|_| PlanningExchangeError::Invalid("invalid provider evidence"));
    }
    if change.deletion_receipt().is_some() {
        return crate::store::sqlite::planning_changes::validate_peer_deletion(value)
            .map_err(|_| PlanningExchangeError::Invalid("invalid planning deletion receipt"));
    }
    if change.order_receipt().is_some() {
        return crate::store::sqlite::planning_order::validate_peer_receipt(value)
            .map_err(|_| PlanningExchangeError::Invalid("invalid planning order receipt"));
    }
    let text = |value: &Value| value.is_null() || value.is_string();
    let valid = match change.field.as_str() {
        "workflow" | "krs" | "metric_targets" if change.object.kind == PlanningKind::Project => {
            let mut content = serde_json::json!({"workflow":"","krs":[],"metric_targets":[]});
            content[&change.field] = value.clone();
            serde_json::from_value::<crate::pm::ProjectContent>(content).is_ok_and(|content| {
                content.validate().is_ok()
                    && serde_json::to_value(&content)
                        .is_ok_and(|roundtrip| roundtrip[&change.field] == *value)
            })
        }
        "creation" => {
            crate::store::sqlite::planning_export::validate_peer_receipt(&change.object, value)
                .is_ok()
        }
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
                    .is_some_and(|a| {
                        serde_json::from_str::<crate::ops::pm::TaskCommentAuthor>(a).is_ok()
                    })
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
