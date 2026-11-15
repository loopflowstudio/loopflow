//! Rust resolves identity, scope and order; clients evaluate this finite program
//! over selection states without reconstructing Task lifecycle rules.
use super::{
    DashboardId, MemberRef, Selection, SelectionError, SelectionExpression, SelectionId,
    SelectionResolution, SelectionState, SelectionWork, WorkWorkspaceKey, WorkWorkspaceKind,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Support belongs to this typed value, independently of query completeness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct FieldSupport {
    pub machine: String,
    pub epoch: String,
    pub schema: u32,
    pub derivation: String,
    pub revisions: BTreeMap<String, i64>,
    pub observations: FieldObservations,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum FieldObservations {
    Sqlite,
    Captured {
        evidence: crate::observe::ObservationEvidence,
    },
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ParentFact {
    #[serde(deserialize_with = "Option::deserialize")]
    pub wave: Option<crate::id::WaveId>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub support: Option<FieldSupport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct StateFact {
    pub states: BTreeSet<SelectionState>,
    pub known: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    pub support: Option<FieldSupport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MembershipFacts {
    pub work: WorkWorkspaceKey,
    pub name: String,
    pub parent: ParentFact,
    pub lifecycle: StateFact,
    pub activity: StateFact,
}

impl MembershipFacts {
    fn contains(&self, state: &SelectionState) -> bool {
        self.lifecycle.states.contains(state) || self.activity.states.contains(state)
    }

    pub fn states(&self) -> BTreeSet<SelectionState> {
        self.lifecycle
            .states
            .union(&self.activity.states)
            .copied()
            .collect()
    }
}

/// Support is optional only for pure selection inputs without a store snapshot.
#[derive(Debug, Clone)]
pub struct MembershipSupport {
    pub parent: FieldSupport,
    pub lifecycle: FieldSupport,
    pub activity: FieldSupport,
}

/// Authored membership predicates over the captured repository manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MembershipPredicate {
    Default {
        #[serde(deserialize_with = "Option::deserialize")]
        wave: Option<crate::id::WaveId>,
    },
    Descendants {
        wave: String,
    },
    Kind {
        work_kind: WorkWorkspaceKind,
    },
    State {
        state: SelectionState,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MembershipNode {
    Members {
        members: Vec<MemberRef>,
    },
    Select {
        source: Vec<SelectionId>,
        predicate: MembershipPredicate,
    },
    Compose {
        items: Vec<MembershipNode>,
    },
    Intersect {
        items: Vec<MembershipNode>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MembershipPlan {
    pub domains: BTreeSet<String>,
    pub root: MembershipNode,
    pub facts: Vec<MembershipFacts>,
    pub unavailable: Vec<String>,
    pub effects: BTreeMap<String, MembershipStateEffect>,
}

/// Predicted display-state changes. These never establish execution readiness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MembershipStateEffect {
    pub stage: String,
    pub domains: BTreeSet<String>,
    pub remove: BTreeSet<SelectionState>,
    pub insert: BTreeSet<SelectionState>,
}

impl MembershipPlan {
    pub fn resolve(&self) -> SelectionResolution {
        let mut unavailable = self.unavailable.clone();
        let parents = self
            .facts
            .iter()
            .filter(|fact| fact.work.kind == WorkWorkspaceKind::Wave)
            .map(|fact| {
                (
                    fact.work.id.as_str(),
                    fact.parent.wave.as_ref().map(crate::id::WaveId::as_str),
                )
            })
            .collect();
        let members = self.root.evaluate(&self.facts, &parents, &mut unavailable);
        let selected: BTreeSet<_> = members.iter().map(|member| &member.work).collect();
        let attention = self
            .facts
            .iter()
            .filter(|fact| {
                selected.contains(&fact.work)
                    && (fact.contains(&SelectionState::Available)
                        || fact.contains(&SelectionState::Blocked))
            })
            .map(|fact| fact.work.clone())
            .collect();
        SelectionResolution {
            attention,
            members,
            unavailable,
        }
    }
}

impl MembershipNode {
    fn keys(&self, keys: &mut BTreeSet<WorkWorkspaceKey>) -> bool {
        match self {
            Self::Members { members } => {
                keys.extend(members.iter().map(|member| member.work.clone()));
                false
            }
            Self::Select { .. } => true,
            Self::Compose { items } | Self::Intersect { items } => {
                let mut dynamic = false;
                for item in items {
                    dynamic |= item.keys(keys);
                }
                dynamic
            }
        }
    }

    fn evaluate(
        &self,
        facts: &[MembershipFacts],
        parents: &BTreeMap<&str, Option<&str>>,
        unavailable: &mut Vec<String>,
    ) -> Vec<MemberRef> {
        match self {
            Self::Members { members } => members.clone(),
            Self::Select { source, predicate } => facts
                .iter()
                .filter(|fact| {
                    let state = match predicate {
                        MembershipPredicate::State { state } => Some(*state),
                        MembershipPredicate::Default { .. }
                            if fact.work.kind == WorkWorkspaceKind::Task =>
                        {
                            Some(SelectionState::Working)
                        }
                        _ => None,
                    };
                    let included = match predicate {
                        MembershipPredicate::State { .. } => {
                            fact.work.kind == WorkWorkspaceKind::Task
                        }
                        MembershipPredicate::Kind { work_kind } => fact.work.kind == *work_kind,
                        MembershipPredicate::Descendants { wave } => {
                            is_descendant(fact, wave.as_str(), parents, unavailable)
                        }
                        MembershipPredicate::Default { wave } => match fact.work.kind {
                            WorkWorkspaceKind::Repository => false,
                            WorkWorkspaceKind::Wave => fact.parent.wave == *wave,
                            WorkWorkspaceKind::Task => wave.as_ref().is_none_or(|wave| {
                                is_descendant(fact, wave.as_str(), parents, unavailable)
                            }),
                        },
                    };
                    if !included {
                        return false;
                    }
                    let Some(state) = state else {
                        return true;
                    };
                    if fact.contains(&state) {
                        return true;
                    }
                    if state.is_activity() && !fact.activity.known {
                        unavailable.push(format!("Task {} activity is unavailable", fact.name));
                    }
                    false
                })
                .map(|fact| MemberRef {
                    source: source.clone(),
                    work: fact.work.clone(),
                })
                .collect(),
            Self::Compose { items } => items
                .iter()
                .flat_map(|item| item.evaluate(facts, parents, unavailable))
                .collect(),
            Self::Intersect { items } => {
                let mut result = None::<Vec<MemberRef>>;
                for item in items {
                    let members = item.evaluate(facts, parents, unavailable);
                    if let Some(result) = &mut result {
                        let keys: BTreeSet<_> = members.iter().map(|member| &member.work).collect();
                        result.retain(|member| keys.contains(&member.work));
                    } else {
                        result = Some(members);
                    }
                }
                result.unwrap_or_default()
            }
        }
    }
}

fn is_descendant(
    fact: &MembershipFacts,
    wave: &str,
    parents: &BTreeMap<&str, Option<&str>>,
    unavailable: &mut Vec<String>,
) -> bool {
    let mut parent = fact.parent.wave.as_ref().map(crate::id::WaveId::as_str);
    let mut seen = BTreeSet::new();
    while let Some(id) = parent {
        if id == wave {
            return true;
        }
        if !seen.insert(id) {
            unavailable.push(format!("Wave {id} ancestry is cyclic"));
            return false;
        }
        let Some(next) = parents.get(id) else {
            unavailable.push(format!("Wave {id} ancestry is unavailable"));
            return false;
        };
        parent = *next;
    }
    false
}

impl Selection {
    pub fn membership_plan(
        &self,
        repository: &str,
        work: &[SelectionWork],
        queries: &BTreeMap<DashboardId, Selection>,
    ) -> Result<MembershipPlan, SelectionError> {
        self.validate()?;
        let mut unavailable = Vec::new();
        let root = self.compile_membership(
            repository,
            work,
            queries,
            &mut Vec::new(),
            &mut BTreeSet::new(),
            &mut unavailable,
        )?;
        let mut keys = BTreeSet::new();
        let dynamic = root.keys(&mut keys);
        Ok(MembershipPlan {
            // Work display facts have no usage dependency. External/native
            // readiness remains outside the SQLite revision contract.
            domains: ["planning", "sessions", "graphs", "processes"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            root,
            facts: work
                .iter()
                .filter(|item| {
                    item.key.repository == repository && (dynamic || keys.contains(&item.key))
                })
                .map(|item| MembershipFacts {
                    work: item.key.clone(),
                    name: item.name.clone(),
                    parent: ParentFact {
                        wave: item.parent_wave.clone(),
                        support: item.support.as_ref().map(|support| support.parent.clone()),
                    },
                    lifecycle: StateFact {
                        states: item
                            .states
                            .iter()
                            .copied()
                            .filter(|state| !state.is_activity())
                            .collect(),
                        known: true,
                        support: item
                            .support
                            .as_ref()
                            .map(|support| support.lifecycle.clone()),
                    },
                    activity: StateFact {
                        states: item
                            .states
                            .iter()
                            .copied()
                            .filter(SelectionState::is_activity)
                            .collect(),
                        known: item.activity_known,
                        support: item
                            .support
                            .as_ref()
                            .map(|support| support.activity.clone()),
                    },
                })
                .collect(),
            unavailable,
            effects: BTreeMap::from([
                (
                    "task.move".into(),
                    MembershipStateEffect {
                        stage: "task.membership_changed".into(),
                        domains: BTreeSet::from(["planning".into()]),
                        remove: BTreeSet::new(),
                        insert: BTreeSet::new(),
                    },
                ),
                (
                    "task.complete".into(),
                    MembershipStateEffect {
                        stage: "task.completed".into(),
                        domains: BTreeSet::from(["planning".into()]),
                        remove: BTreeSet::from([
                            SelectionState::Open,
                            SelectionState::Available,
                            SelectionState::Busy,
                            SelectionState::Blocked,
                            SelectionState::Unstarted,
                            SelectionState::Idle,
                            SelectionState::Active,
                            SelectionState::Moving,
                            SelectionState::Shipping,
                            SelectionState::Backlog,
                            SelectionState::Working,
                            SelectionState::Autonomous,
                        ]),
                        insert: BTreeSet::from([SelectionState::Done]),
                    },
                ),
                (
                    "task.abandon".into(),
                    MembershipStateEffect {
                        stage: "task.abandoned".into(),
                        domains: BTreeSet::from(["planning".into()]),
                        remove: BTreeSet::from([
                            SelectionState::Open,
                            SelectionState::Available,
                            SelectionState::Busy,
                            SelectionState::Blocked,
                            SelectionState::Unstarted,
                            SelectionState::Idle,
                            SelectionState::Active,
                            SelectionState::Moving,
                            SelectionState::Shipping,
                            SelectionState::Backlog,
                            SelectionState::Working,
                            SelectionState::Autonomous,
                            SelectionState::Done,
                        ]),
                        insert: BTreeSet::new(),
                    },
                ),
            ]),
        })
    }

    fn compile_membership(
        &self,
        repository: &str,
        work: &[SelectionWork],
        queries: &BTreeMap<DashboardId, Selection>,
        path: &mut Vec<SelectionId>,
        visiting: &mut BTreeSet<DashboardId>,
        unavailable: &mut Vec<String>,
    ) -> Result<MembershipNode, SelectionError> {
        path.push(self.id.clone());
        let node = match &self.expression {
            SelectionExpression::Compose { items } | SelectionExpression::Intersect { items } => {
                let nodes = items
                    .iter()
                    .map(|item| {
                        item.compile_membership(
                            repository,
                            work,
                            queries,
                            path,
                            visiting,
                            unavailable,
                        )
                    })
                    .collect::<Result<_, _>>()?;
                if matches!(self.expression, SelectionExpression::Compose { .. }) {
                    MembershipNode::Compose { items: nodes }
                } else {
                    MembershipNode::Intersect { items: nodes }
                }
            }
            SelectionExpression::Query { query } => {
                if !visiting.insert(query.clone()) {
                    return Err(SelectionError::QueryCycle(query.0.clone()));
                }
                let node = if let Some(selection) = queries.get(query) {
                    selection.validate()?;
                    selection.compile_membership(
                        repository,
                        work,
                        queries,
                        path,
                        visiting,
                        unavailable,
                    )?
                } else {
                    unavailable.push(format!("Named query {} is unavailable", query.0));
                    MembershipNode::Members {
                        members: Vec::new(),
                    }
                };
                visiting.remove(query);
                node
            }
            SelectionExpression::Filter { filter } => {
                Self::compile_filter(filter, repository, work, path, unavailable)?
            }
            SelectionExpression::Work { work: key } => {
                if key.repository != repository {
                    return Err(SelectionError::Repository(key.repository.clone()));
                }
                let members = if work.iter().any(|item| &item.key == key) {
                    vec![MemberRef {
                        source: path.clone(),
                        work: key.clone(),
                    }]
                } else {
                    unavailable.push(format!("Work {} is unavailable", key.id));
                    Vec::new()
                };
                MembershipNode::Members { members }
            }
            scope => {
                let wave = match scope {
                    SelectionExpression::Wave { wave } => Some(wave),
                    _ => None,
                };
                if let Some(wave) = wave {
                    if !work.iter().any(|item| {
                        item.key.repository == repository
                            && item.key.kind == WorkWorkspaceKind::Wave
                            && item.key.id == wave.as_str()
                    }) {
                        unavailable.push(format!("Wave {wave} is unavailable"));
                    }
                }
                MembershipNode::Select {
                    source: path.clone(),
                    predicate: MembershipPredicate::Default {
                        wave: wave.cloned(),
                    },
                }
            }
        };
        path.pop();
        Ok(node)
    }
}
