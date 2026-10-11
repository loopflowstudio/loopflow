use serde::{Deserialize, Serialize};

use super::{TaskEvent, TaskEventKind};

/// Exact filing input, persisted before contacting Linear.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowThroughIntent {
    pub key: String,
    pub issue_id: String,
    pub relation_id: String,
    pub project_id: String,
    pub team_id: String,
    pub state_id: Option<String>,
    pub wave: String,
    pub title: String,
    pub notes: String,
    pub due: Option<String>,
    pub existing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowThroughLink {
    pub key: String,
    pub issue_id: String,
    pub identifier: String,
    pub url: Option<String>,
    pub due: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowThrough {
    pub intents: Vec<FollowThroughIntent>,
    pub links: Vec<FollowThroughLink>,
    pub reason: Option<String>,
    pub needs_conversion: bool,
    pub scope_notes: Vec<String>,
}

impl FollowThrough {
    pub fn from_events(events: &[TaskEvent]) -> Self {
        let mut value = Self {
            intents: Vec::new(),
            links: Vec::new(),
            reason: None,
            needs_conversion: false,
            scope_notes: Vec::new(),
        };
        for event in events {
            match &event.kind {
                TaskEventKind::FollowUp { remaining, reason } => {
                    value.needs_conversion = remaining.is_some();
                    value.scope_notes.clear();
                    if let Some(remaining) = remaining {
                        let check_at =
                            time::OffsetDateTime::from_unix_timestamp(remaining.check_at)
                                .map(|date| date.to_string())
                                .unwrap_or_else(|_| remaining.check_at.to_string());
                        value.scope_notes.push(format!(
                            "{}\nEvidence: {}\nCheck at: {}\n{}",
                            remaining.outcome, remaining.evidence, check_at, reason
                        ));
                    }
                }
                TaskEventKind::FollowThroughConversion { reason } => {
                    value.needs_conversion = true;
                    value.scope_notes.push(reason.clone());
                }
                TaskEventKind::FollowThroughIntent { intent } => {
                    if !value.intents.iter().any(|prior| prior.key == intent.key) {
                        value.intents.push(intent.clone());
                    }
                    value.reason = None;
                }
                TaskEventKind::FollowThroughLinked { link } => {
                    value.links.retain(|prior| prior.key != link.key);
                    value.links.push(link.clone());
                }
                TaskEventKind::FollowThroughDisposition { reason } => {
                    value.reason = Some(reason.clone());
                    value.needs_conversion = false;
                    value.scope_notes.clear();
                }
                _ => {}
            }
        }
        value
    }

    pub fn resolved(&self) -> bool {
        self.reason.is_some() && !self.needs_conversion && self.links_confirmed()
    }

    pub(crate) fn links_confirmed(&self) -> bool {
        self.intents.iter().all(|intent| {
            self.links
                .iter()
                .any(|link| link.key == intent.key && link.issue_id == intent.issue_id)
        })
    }
}

/// Direction comes from a confirmed source filing, not a generic issue relation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FollowThroughSource {
    pub issue_id: String,
    pub identifier: String,
}
