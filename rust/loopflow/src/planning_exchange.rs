//! Immutable planning mutations, not execution records. Causality precedes clocks;
//! concurrent values use the latest clock/identity. Linear uses its own transport.
//! Losing values stay in the document and are available for recovery.

use std::collections::{BTreeMap, BTreeSet};

use crate::work::edit::WorkEdit;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanningMutation {
    pub edit: WorkEdit,
    /// Hybrid logical milliseconds, advanced beyond every observed mutation.
    pub clock: i64,
    /// Observed heads of this field, not a global revision or Git ancestry.
    pub parents: BTreeSet<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct PlanningSnapshot {
    pub changes: BTreeMap<String, PlanningMutation>,
}

impl<'de> Deserialize<'de> for PlanningSnapshot {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Document {
            changes: BTreeMap<String, PlanningMutation>,
        }
        let document = Document::deserialize(deserializer)?;
        Self {
            changes: document.changes,
        }
        .canonicalized()
        .map_err(serde::de::Error::custom)
    }
}

impl PlanningSnapshot {
    pub(crate) fn canonicalized(&self) -> Result<Self, PlanningExchangeError> {
        let mut changes = BTreeMap::new();
        for (id, change) in &self.changes {
            let canonical = crate::id::canonical_uuid(id);
            let mut parents = BTreeSet::new();
            for parent in &change.parents {
                if !parents.insert(crate::id::canonical_uuid(parent)) {
                    return Err(PlanningExchangeError::Invalid(
                        "canonical identity collision: planning predecessors",
                    ));
                }
            }
            let edit = change.edit.canonicalized().map_err(|_| {
                PlanningExchangeError::Invalid("canonical identity collision: Work value")
            })?;
            if changes
                .insert(
                    canonical,
                    PlanningMutation {
                        edit,
                        clock: change.clock,
                        parents,
                    },
                )
                .is_some()
            {
                return Err(PlanningExchangeError::Invalid(
                    "canonical identity collision: planning mutation keys",
                ));
            }
        }
        let canonical = Self { changes };
        let winners: BTreeMap<_, _> = canonical
            .winners()
            .map(|(id, change)| (change.edit.key(), id))
            .collect();
        for (id, change) in self.winners() {
            let edit = change
                .edit
                .canonicalized()
                .map_err(|_| PlanningExchangeError::Invalid("invalid Work value"))?;
            if winners.get(&edit.key()).copied() != Some(crate::id::canonical_uuid(id).as_str()) {
                return Err(PlanningExchangeError::Invalid(
                    "canonical identity collision: planning winner would change",
                ));
            }
        }
        Ok(canonical)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, PlanningExchangeError> {
        let snapshot: Self = serde_json::from_slice(bytes)?;
        snapshot.validate()?;
        Ok(snapshot)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, PlanningExchangeError> {
        let canonical = self.canonicalized()?;
        canonical.validate()?;
        Ok(serde_json::to_vec(&canonical)?)
    }

    /// Omission is never deletion. Reused identities reject the document without
    /// replacing either original; independent writes need distinct identities.
    pub fn merge(&self, incoming: &Self) -> Result<Self, PlanningExchangeError> {
        let mut merged = self.canonicalized()?;
        let incoming = incoming.canonicalized()?;
        merged.validate()?;
        incoming.validate()?;
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
                        .is_some_and(|parent| parent.edit.key() == change.edit.key())
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
            if id.is_empty() || change.edit.target.is_empty() || change.clock < 0 {
                return Err(PlanningExchangeError::Invalid("invalid planning mutation"));
            }
            change
                .edit
                .change
                .validate()
                .map_err(|_| PlanningExchangeError::Invalid("invalid Work value"))?;
            for parent in &change.parents {
                let previous = self
                    .changes
                    .get(parent)
                    .ok_or(PlanningExchangeError::Invalid(
                        "missing planning predecessor",
                    ))?;
                if previous.edit.key() != change.edit.key() || previous.clock >= change.clock {
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
            .entry(change.edit.key())
            .or_insert((priority, (id, change)));
        if priority > winner.0 {
            *winner = (priority, (id, change));
        }
    }
    fields.into_values().map(|(_, winner)| winner)
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

#[cfg(test)]
mod canonical_tests {
    use super::PlanningSnapshot;
    use serde_json::json;

    #[test]
    fn canonical_ids_peer_reimport_keeps_original_mutations_and_opaque_content() {
        let original = json!({"changes":{
            "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA":{"edit":{"target":"TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","change":{"field":"task_title","value":"Case-Sensitive Title"}},"clock":1,"parents":[]},
            "BBBBBBBB-BBBB-4BBB-8BBB-BBBBBBBBBBBB":{"edit":{"target":"TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","change":{"field":"task_title","value":"Next Title"}},"clock":2,"parents":["AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA"]}
        }});
        let snapshot =
            PlanningSnapshot::from_bytes(&serde_json::to_vec(&original).unwrap()).unwrap();
        assert_eq!(snapshot.changes.len(), 2);
        let saved = PlanningSnapshot::from_bytes(&snapshot.to_bytes().unwrap()).unwrap();
        assert_eq!(
            saved.merge(&snapshot).unwrap().merge(&snapshot).unwrap(),
            saved
        );
        assert_eq!(
            saved.changes["aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"]
                .edit
                .target,
            "task_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
        );
        assert_eq!(
            saved.changes["aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"]
                .edit
                .change
                .value()
                .unwrap(),
            "Case-Sensitive Title"
        );
    }

    #[test]
    fn canonical_ids_peer_rejects_colliding_keys_predecessors_and_changed_winners() {
        let a = "AAAAAAAA-AAAA-4AAA-8AAA-AAAAAAAAAAAA";
        let mut value = json!({"changes":{a:{"edit":{"target":"TASK_AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA","change":{"field":"task_title","value":"Title"}},"clock":1,"parents":[]}}});
        value["changes"][a.to_ascii_lowercase()] = value["changes"][a].clone();
        assert!(
            PlanningSnapshot::from_bytes(&serde_json::to_vec(&value).unwrap())
                .unwrap_err()
                .to_string()
                .contains("collision")
        );
        value["changes"]
            .as_object_mut()
            .unwrap()
            .remove(&a.to_ascii_lowercase());
        let b = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        value["changes"][b] = value["changes"][a].clone();
        // Uppercase C sorts before lowercase b; normalization would reverse that winner.
        let c = "CCCCCCCC-CCCC-4CCC-8CCC-CCCCCCCCCCCC";
        value["changes"][c] = value["changes"][a].clone();
        assert!(
            PlanningSnapshot::from_bytes(&serde_json::to_vec(&value).unwrap())
                .unwrap_err()
                .to_string()
                .contains("winner")
        );
        value["changes"].as_object_mut().unwrap().remove(c);
        value["changes"][b]["clock"] = 2.into();
        value["changes"][b]["parents"] = json!([a, a.to_ascii_lowercase()]);
        assert!(
            PlanningSnapshot::from_bytes(&serde_json::to_vec(&value).unwrap())
                .unwrap_err()
                .to_string()
                .contains("predecessors")
        );
    }
}
