//! Linear delivery evidence for saved Work.

use serde::{Deserialize, Serialize};

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

/// Read-only delivery evidence. These rows never confer execution authority.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningSyncStatus {
    pub connected: bool,
    pub changes: Vec<PlanningSyncChange>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningSyncChange {
    pub id: String,
    pub field: String,
    pub state: PlanningSyncState,
    pub local_value: serde_json::Value,
    pub linear_value: Option<serde_json::Value>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PlanningSyncState {
    Pending,
    Uncertain,
    AdoptedLinear,
}

impl PlanningSyncStatus {
    pub fn lines(&self) -> Vec<String> {
        self.changes
            .iter()
            .map(|change| {
                let label = match change.state {
                    PlanningSyncState::Pending => "Saved locally; pending Linear sync",
                    PlanningSyncState::Uncertain => "Awaiting Linear confirmation",
                    PlanningSyncState::AdoptedLinear => "Adopted Linear change",
                };
                let mut line = format!("{label} · {}", change.field);
                if change.state == PlanningSyncState::AdoptedLinear {
                    let value = change
                        .linear_value
                        .as_ref()
                        .unwrap_or(&serde_json::Value::Null);
                    line.push_str(&format!(
                        " · saved: {} · Linear: {value}",
                        change.local_value
                    ));
                }
                if let Some(error) = &change.error {
                    line.push_str(&format!(" · {error}"));
                }
                line
            })
            .collect()
    }
}
