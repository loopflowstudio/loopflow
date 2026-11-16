//! Rust resolves identity, scope and order; clients evaluate this finite program
//! over independently supported Task inputs.
use super::{
    DashboardId, MemberRef, Selection, SelectionError, SelectionExpression, SelectionId,
    SelectionResolution, SelectionState, SelectionWork, WorkWorkspaceKey, WorkWorkspaceKind,
};
use crate::durable::{TaskSection, TaskState};
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

/// Independently supported SQL input. Unknown is distinct from a known null.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(bound(deserialize = "T: Deserialize<'de>"))]
pub struct ValueFact<T> {
    #[serde(deserialize_with = "T::deserialize")]
    pub value: T,
    pub known: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    pub support: Option<FieldSupport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct TaskDisposition {
    pub completed: bool,
    pub abandoned: bool,
    #[serde(deserialize_with = "Option::deserialize")]
    pub completed_at: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub planning_state: Option<String>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub completion_request: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct TaskMembershipInputs {
    pub disposition: TaskDisposition,
    pub ownership: Option<crate::durable::TaskOwner>,
    pub publication: bool,
    pub execution: crate::durable::TaskExecution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MembershipFacts {
    pub work: WorkWorkspaceKey,
    pub name: String,
    pub parent: ParentFact,
    pub disposition: ValueFact<TaskDisposition>,
    pub ownership: ValueFact<Option<crate::durable::TaskOwner>>,
    pub publication: ValueFact<bool>,
    pub execution: ValueFact<crate::durable::TaskExecution>,
    pub activity: StateFact,
}

impl MembershipFacts {
    fn contains(&self, state: &SelectionState) -> bool {
        self.states().contains(state)
    }

    pub fn states(&self) -> BTreeSet<SelectionState> {
        if self.work.kind != WorkWorkspaceKind::Task {
            return BTreeSet::new();
        }
        let d = &self.disposition.value;
        let runtime = if d.abandoned {
            crate::durable::TaskState::Abandoned
        } else if d.completed {
            crate::durable::TaskState::Done
        } else {
            self.execution.value.state()
        };
        task_selection_states(
            runtime,
            self.ownership.value.is_some(),
            self.publication.value,
            d.planning_state.as_deref(),
            &self.activity.states,
        )
    }
}

/// Support is optional only for pure selection inputs without a store snapshot.
#[derive(Debug, Clone)]
pub struct MembershipSupport {
    pub parent: FieldSupport,
    pub disposition: FieldSupport,
    pub ownership: FieldSupport,
    pub publication: FieldSupport,
    pub execution: FieldSupport,
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

/// Semantic stages and reload domains; clients project the typed input change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MembershipStateEffect {
    pub stage: String,
    pub domains: BTreeSet<String>,
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
                    disposition: ValueFact {
                        value: item
                            .inputs
                            .as_ref()
                            .map(|i| i.disposition.clone())
                            .unwrap_or(TaskDisposition {
                                completed: false,
                                abandoned: false,
                                completed_at: None,
                                planning_state: None,
                                completion_request: None,
                            }),
                        known: item.inputs.is_some(),
                        support: item.support.as_ref().map(|s| s.disposition.clone()),
                    },
                    ownership: ValueFact {
                        value: item.inputs.as_ref().and_then(|i| i.ownership.clone()),
                        known: item.inputs.is_some(),
                        support: item.support.as_ref().map(|s| s.ownership.clone()),
                    },
                    publication: ValueFact {
                        value: item.inputs.as_ref().is_some_and(|i| i.publication),
                        known: item.inputs.is_some(),
                        support: item.support.as_ref().map(|s| s.publication.clone()),
                    },
                    execution: ValueFact {
                        value: item.inputs.as_ref().map(|i| i.execution).unwrap_or(
                            crate::durable::TaskExecution {
                                checkout: false,
                                graph_running: false,
                                carrier_running: false,
                                held: false,
                                approved: false,
                            },
                        ),
                        known: item.inputs.is_some(),
                        support: item.support.as_ref().map(|s| s.execution.clone()),
                    },
                    activity: StateFact {
                        states: item.activity_states.clone(),
                        known: item.activity_known,
                        support: item
                            .support
                            .as_ref()
                            .map(|support| support.activity.clone()),
                    },
                })
                .collect(),
            unavailable,
            effects: [
                ("task.move", "task.membership_changed", vec!["planning"]),
                ("task.complete", "task.completed", vec!["planning"]),
                ("task.abandon", "task.abandoned", vec!["planning"]),
                ("task.claim", "task.claimed", vec!["planning"]),
                ("task.unassign", "task.unassigned", vec!["planning"]),
                ("task.reopen", "task.reopened", vec!["planning", "graphs"]),
            ]
            .into_iter()
            .map(|(kind, stage, domains)| {
                (
                    kind.into(),
                    MembershipStateEffect {
                        stage: stage.into(),
                        domains: domains.into_iter().map(str::to_owned).collect(),
                    },
                )
            })
            .collect(),
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

/// The same lifecycle, section and activity set for roadmap rows and selections.
pub(crate) fn task_selection_states(
    runtime: TaskState,
    owned: bool,
    published: bool,
    plan_state: Option<&str>,
    activity: &BTreeSet<SelectionState>,
) -> BTreeSet<SelectionState> {
    let state = runtime.lifecycle(plan_state);
    // Completion changes actionable membership, not the retained conversation's
    // execution facts. A completed Task cannot re-enter Ready/Working from an
    // old Session report after its removal effect is incorporated.
    let mut states = if state.is_terminal() {
        BTreeSet::new()
    } else {
        activity.clone()
    };
    if (owned || published) && !state.is_terminal() {
        states.insert(SelectionState::Open);
    }
    states.insert(match state {
        TaskState::Unstarted => SelectionState::Unstarted,
        TaskState::Idle => SelectionState::Idle,
        TaskState::Active => SelectionState::Active,
        TaskState::Moving => SelectionState::Moving,
        TaskState::Shipping => SelectionState::Shipping,
        TaskState::Done | TaskState::Abandoned => SelectionState::Done,
    });
    states.insert(match TaskSection::of(runtime, owned, plan_state) {
        TaskSection::Backlog => SelectionState::Backlog,
        TaskSection::Working => SelectionState::Working,
        TaskSection::Autonomous => SelectionState::Autonomous,
        TaskSection::Done | TaskSection::Abandoned => SelectionState::Done,
    });
    states
}
