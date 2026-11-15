//! Rust resolves identity, scope and order; clients evaluate this finite program
//! over selection states without reconstructing Task lifecycle rules.
use super::{
    wave_descendants, DashboardId, MemberRef, Selection, SelectionError, SelectionExpression,
    SelectionId, SelectionResolution, SelectionState, SelectionWork, WorkWorkspaceKey,
    WorkWorkspaceKind,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct MembershipFacts {
    pub work: WorkWorkspaceKey,
    pub name: String,
    pub states: BTreeSet<SelectionState>,
    pub activity_known: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MembershipNode {
    Members {
        members: Vec<MemberRef>,
    },
    State {
        state: SelectionState,
        members: Vec<MemberRef>,
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
        let facts = self.facts.iter().map(|fact| (&fact.work, fact)).collect();
        let mut unavailable = self.unavailable.clone();
        let members = self.root.evaluate(&facts, &mut unavailable);
        let selected: BTreeSet<_> = members.iter().map(|member| &member.work).collect();
        let attention = self
            .facts
            .iter()
            .filter(|fact| {
                selected.contains(&fact.work)
                    && (fact.states.contains(&SelectionState::Available)
                        || fact.states.contains(&SelectionState::Blocked))
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
    fn keys(&self, keys: &mut BTreeSet<WorkWorkspaceKey>) {
        match self {
            Self::Members { members } | Self::State { members, .. } => {
                keys.extend(members.iter().map(|member| member.work.clone()));
            }
            Self::Compose { items } | Self::Intersect { items } => {
                for item in items {
                    item.keys(keys);
                }
            }
        }
    }

    fn evaluate(
        &self,
        facts: &BTreeMap<&WorkWorkspaceKey, &MembershipFacts>,
        unavailable: &mut Vec<String>,
    ) -> Vec<MemberRef> {
        match self {
            Self::Members { members } => members.clone(),
            Self::State { state, members } => members
                .iter()
                .filter(|member| {
                    let Some(fact) = facts.get(&member.work) else {
                        unavailable.push(format!("Work {} facts are unavailable", member.work.id));
                        return false;
                    };
                    if fact.states.contains(state) {
                        return true;
                    }
                    if !fact.activity_known
                        && matches!(
                            state,
                            SelectionState::Available
                                | SelectionState::Busy
                                | SelectionState::Blocked
                        )
                    {
                        unavailable.push(format!("Task {} activity is unavailable", fact.name));
                    }
                    false
                })
                .cloned()
                .collect(),
            Self::Compose { items } => items
                .iter()
                .flat_map(|item| item.evaluate(facts, unavailable))
                .collect(),
            Self::Intersect { items } => {
                let mut result = None::<Vec<MemberRef>>;
                for item in items {
                    let members = item.evaluate(facts, unavailable);
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
        root.keys(&mut keys);
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
                .filter(|item| keys.contains(&item.key))
                .map(|item| MembershipFacts {
                    work: item.key.clone(),
                    name: item.name.clone(),
                    states: item.states.clone(),
                    activity_known: item.activity_known,
                })
                .collect(),
            unavailable,
            effects: BTreeMap::from([
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
        let member = |item: &SelectionWork| MemberRef {
            source: path.clone(),
            work: item.key.clone(),
        };
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
                let descendants = wave.map_or_else(BTreeSet::new, |wave| {
                    wave_descendants(repository, wave.as_str(), work)
                });
                // Preserve the server's interleaved Wave/Task order.
                let items = work
                    .iter()
                    .filter(|item| item.key.repository == repository)
                    .filter_map(|item| match item.key.kind {
                        WorkWorkspaceKind::Repository => None,
                        WorkWorkspaceKind::Wave if item.parent_wave.as_ref() == wave => {
                            Some(MembershipNode::Members {
                                members: vec![member(item)],
                            })
                        }
                        WorkWorkspaceKind::Task
                            if wave.is_none()
                                || item.parent_wave.as_ref().is_some_and(|parent| {
                                    descendants.contains(parent.as_str())
                                }) =>
                        {
                            Some(MembershipNode::State {
                                state: SelectionState::Working,
                                members: vec![member(item)],
                            })
                        }
                        _ => None,
                    })
                    .collect();
                MembershipNode::Compose { items }
            }
        };
        path.pop();
        Ok(node)
    }
}
