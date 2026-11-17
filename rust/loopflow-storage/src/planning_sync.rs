//! What an owner's tracker has yet to confirm and what it won: the open
//! tracker actions `lf repo plan`, `lf task show` and Desktop read.

use serde::{Deserialize, Serialize};

use crate::provider::Provider;
use crate::sqlite::tracker_actions::{ActionKind, ActionPhase};

/// A pending field edit and its provider baseline, independent of connection state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningChange {
    pub id: String,
    pub value: crate::work::edit::WorkValue,
    pub base: Option<PlanningBaseline>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningBaseline {
    pub revision: Option<String>,
    /// None retains an explicitly null membership claim; no baseline means omission.
    pub value: Option<crate::work::edit::WorkValue>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TaskDeletion {
    pub id: String,
    pub base_revision: Option<String>,
}

/// An owner's open tracker actions read with its repository, before observing
/// whether that repository still names a tracker.
#[derive(Debug, Clone)]
pub struct TrackerActions {
    pub repo: String,
    pub actions: Vec<TrackerActionStatus>,
}

impl TrackerActions {
    /// With no tracker configured, nothing queued will be sent, so only what
    /// was sent or lost stays.
    pub fn status(self, connected: bool) -> TrackerSync {
        TrackerSync {
            actions: self
                .actions
                .into_iter()
                .filter(|action| connected || action.phase != ActionPhase::Queued)
                .collect(),
        }
    }
}

/// A Task's or Project's slice of `lf repo plan`'s actions: what its tracker
/// has yet to confirm, and each value the tracker won until it is saved again.
/// Read-only; these rows never confer execution authority.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TrackerSync {
    pub actions: Vec<TrackerActionStatus>,
}

/// One tracker action as a person reads it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TrackerActionStatus {
    pub id: String,
    /// Send order across the repository; none for Git, which keeps no queue.
    pub seq: Option<i64>,
    pub owner_kind: ActionOwnerKind,
    pub owner_id: String,
    pub kind: ActionKind,
    /// What changes, as a person names it: `title`, `complete`, `comment`.
    pub change: String,
    /// The field or comment the change addresses.
    pub subject: Option<String>,
    /// `queued`, `sent` (reply unknown) or `conflicted` (the tracker's value won).
    pub phase: ActionPhase,
    pub tracker: TrackerKind,
    /// Why the last request did not confirm it.
    pub error: Option<String>,
    /// The value saved here; when conflicted, the one that lost.
    pub local_value: serde_json::Value,
    /// The tracker's winning value, when conflicted.
    pub tracker_value: Option<serde_json::Value>,
}

/// What a tracker action addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActionOwnerKind {
    Task,
    Project,
    Wave,
}

impl ActionOwnerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Project => "project",
            Self::Wave => "wave",
        }
    }
}

/// Where a planning change goes: Git's journal, or a queued tracker.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TrackerKind {
    Git,
    Linear,
    Asana,
}

impl TrackerKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Linear => "linear",
            Self::Asana => "asana",
        }
    }

    /// The queued tracker a provider names; `None` for a provider that is not one.
    pub fn of(provider: Provider) -> Option<Self> {
        match provider {
            Provider::Linear => Some(Self::Linear),
            Provider::Asana => Some(Self::Asana),
            _ => None,
        }
    }
}

impl TrackerActionStatus {
    /// One line: `title · queued`, `title · conflicted · lost: "A" · linear: "B"`.
    pub fn line(&self) -> String {
        let mut line = format!("{} · {}", self.change, self.phase.as_str());
        if self.phase == ActionPhase::Sent {
            line.push_str(&format!(
                ", waiting for {} to confirm",
                self.tracker.as_str()
            ));
        }
        if self.phase == ActionPhase::Conflicted {
            let won = self
                .tracker_value
                .as_ref()
                .unwrap_or(&serde_json::Value::Null);
            line.push_str(&format!(
                " · lost: {} · {}: {won}",
                self.local_value,
                self.tracker.as_str()
            ));
        }
        if let Some(error) = &self.error {
            line.push_str(&format!(" · {error}"));
        }
        line
    }
}

impl TrackerSync {
    pub fn lines(&self) -> Vec<String> {
        self.actions.iter().map(TrackerActionStatus::line).collect()
    }
}
