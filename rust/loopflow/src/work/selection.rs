//! Typed selection and portable arrangement intent. No provider or surface ownership.
use crate::id::WaveId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum WorkWorkspaceKind {
    Repository,
    Wave,
    Task,
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct WorkWorkspaceKey {
    /// Durable repository plan ID, not a filesystem opening or execution locator.
    pub repository: String,
    pub kind: WorkWorkspaceKind,
    pub id: String,
}

/// Stable authored node identity, independent of its position in a composition.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SelectionId(pub String);

impl SelectionId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}
impl Default for SelectionId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct QueryId(pub String);

impl QueryId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}
impl Default for QueryId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Selection {
    pub id: SelectionId,
    pub expression: SelectionExpression,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum SelectionExpression {
    Repository,
    Wave { wave: WaveId },
    Work { work: WorkWorkspaceKey },
    Compose { items: Vec<Selection> },
    Query { query: QueryId },
}

/// The source path contains authored IDs, never array indexes or current focus.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct MemberRef {
    pub source: Vec<SelectionId>,
    pub work: WorkWorkspaceKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectionResolution {
    pub members: Vec<MemberRef>,
    /// Nonempty means departures cannot be inferred from this reading.
    pub unavailable: Vec<String>,
}

/// Facts from the shared Work reader. Unknown Task state remains unknown.
#[derive(Debug, Clone)]
pub struct SelectionWork {
    pub key: WorkWorkspaceKey,
    pub parent_wave: Option<WaveId>,
    pub open: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SelectionError {
    #[error("selection node IDs must be nonempty and unique: {0}")]
    NodeIdentity(String),
    #[error("named query cycle at {0}")]
    QueryCycle(String),
    #[error("Work belongs to another repository: {0}")]
    Repository(String),
    #[error(
        "arrangement must contain every selected member exactly once, including hidden members"
    )]
    Membership,
    #[error("split ratio must be finite and between 0.1 and 0.9")]
    Ratio,
}

impl Selection {
    pub fn validate(&self) -> Result<(), SelectionError> {
        fn visit(node: &Selection, ids: &mut BTreeSet<SelectionId>) -> Result<(), SelectionError> {
            if node.id.0.is_empty() || !ids.insert(node.id.clone()) {
                return Err(SelectionError::NodeIdentity(node.id.0.clone()));
            }
            if let SelectionExpression::Compose { items } = &node.expression {
                for child in items {
                    visit(child, ids)?;
                }
            }
            Ok(())
        }
        visit(self, &mut BTreeSet::new())
    }

    pub fn resolve(
        &self,
        repository: &str,
        work: &[SelectionWork],
        queries: &BTreeMap<QueryId, Selection>,
    ) -> Result<SelectionResolution, SelectionError> {
        self.validate()?;
        let mut result = SelectionResolution {
            members: Vec::new(),
            unavailable: Vec::new(),
        };
        self.expand(
            repository,
            work,
            queries,
            &mut Vec::new(),
            &mut BTreeSet::new(),
            &mut result,
        )?;
        Ok(result)
    }

    fn expand(
        &self,
        repository: &str,
        work: &[SelectionWork],
        queries: &BTreeMap<QueryId, Selection>,
        path: &mut Vec<SelectionId>,
        visiting: &mut BTreeSet<QueryId>,
        result: &mut SelectionResolution,
    ) -> Result<(), SelectionError> {
        path.push(self.id.clone());
        match &self.expression {
            SelectionExpression::Compose { items } => {
                for item in items {
                    item.expand(repository, work, queries, path, visiting, result)?;
                }
            }
            SelectionExpression::Query { query } => {
                if !visiting.insert(query.clone()) {
                    return Err(SelectionError::QueryCycle(query.0.clone()));
                }
                if let Some(selection) = queries.get(query) {
                    selection.validate()?;
                    selection.expand(repository, work, queries, path, visiting, result)?;
                } else {
                    result
                        .unavailable
                        .push(format!("Named query {} is unavailable", query.0));
                }
                visiting.remove(query);
            }
            SelectionExpression::Work { work: key } => {
                if key.repository != repository {
                    return Err(SelectionError::Repository(key.repository.clone()));
                }
                if work.iter().any(|item| &item.key == key) {
                    result.members.push(MemberRef {
                        source: path.clone(),
                        work: key.clone(),
                    });
                } else {
                    result
                        .unavailable
                        .push(format!("Work {} is unavailable", key.id));
                }
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
                        result
                            .unavailable
                            .push(format!("Wave {wave} is unavailable"));
                    }
                }
                let mut descendants = BTreeSet::new();
                if let Some(wave) = wave {
                    descendants.insert(wave.to_string());
                }
                loop {
                    let before = descendants.len();
                    for item in work.iter().filter(|item| {
                        item.key.repository == repository
                            && item.key.kind == WorkWorkspaceKind::Wave
                    }) {
                        if item
                            .parent_wave
                            .as_ref()
                            .is_some_and(|parent| descendants.contains(parent.as_str()))
                        {
                            descendants.insert(item.key.id.clone());
                        }
                    }
                    if before == descendants.len() {
                        break;
                    }
                }
                for item in work.iter().filter(|item| item.key.repository == repository) {
                    let included = match item.key.kind {
                        WorkWorkspaceKind::Repository => false,
                        WorkWorkspaceKind::Wave => item.parent_wave.as_ref() == wave,
                        WorkWorkspaceKind::Task => {
                            let in_scope = wave.is_none()
                                || item
                                    .parent_wave
                                    .as_ref()
                                    .is_some_and(|parent| descendants.contains(parent.as_str()));
                            if in_scope && item.open.is_none() {
                                result
                                    .unavailable
                                    .push(format!("Task {} state is unavailable", item.key.id));
                            }
                            in_scope && item.open == Some(true)
                        }
                    };
                    if included {
                        result.members.push(MemberRef {
                            source: path.clone(),
                            work: item.key.clone(),
                        });
                    }
                }
            }
        }
        path.pop();
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArrangementAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ArrangementTree {
    Member {
        member: MemberRef,
    },
    Split {
        axis: ArrangementAxis,
        ratio: f64,
        first: Box<Self>,
        second: Box<Self>,
    },
}

impl ArrangementTree {
    pub fn members(&self) -> Vec<MemberRef> {
        match self {
            Self::Member { member } => vec![member.clone()],
            Self::Split { first, second, .. } => [first.members(), second.members()].concat(),
        }
    }

    fn validate(&self) -> Result<(), SelectionError> {
        if let Self::Split {
            ratio,
            first,
            second,
            ..
        } = self
        {
            if !ratio.is_finite() || !(0.1..=0.9).contains(ratio) {
                return Err(SelectionError::Ratio);
            }
            first.validate()?;
            second.validate()?;
        }
        Ok(())
    }

    fn retaining(&self, keep: &mut impl FnMut(&MemberRef) -> bool) -> Option<Self> {
        match self {
            Self::Member { member } => keep(member).then(|| self.clone()),
            Self::Split {
                axis,
                ratio,
                first,
                second,
            } => match (first.retaining(keep), second.retaining(keep)) {
                (Some(first), Some(second)) => Some(Self::Split {
                    axis: *axis,
                    ratio: *ratio,
                    first: Box::new(first),
                    second: Box::new(second),
                }),
                (first, second) => first.or(second),
            },
        }
    }

    fn tiled(members: &[MemberRef]) -> Option<Self> {
        match members {
            [] => None,
            [member] => Some(Self::Member {
                member: member.clone(),
            }),
            _ => {
                let mid = members.len() / 2;
                Some(Self::Split {
                    axis: ArrangementAxis::Vertical,
                    ratio: 0.5,
                    first: Box::new(Self::tiled(&members[..mid]).expect("nonempty first half")),
                    second: Box::new(Self::tiled(&members[mid..]).expect("nonempty second half")),
                })
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArrangementMode {
    Auto,
    Touched,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Arrangement {
    pub tree: Option<ArrangementTree>,
    pub hidden: BTreeSet<MemberRef>,
    pub mode: ArrangementMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArrangementProjection {
    pub visible: Option<ArrangementTree>,
    pub suppressed: Vec<SuppressedMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SuppressedMember {
    pub member: MemberRef,
    pub winner: MemberRef,
}

impl Arrangement {
    pub fn automatic(members: &[MemberRef]) -> Self {
        Self {
            tree: ArrangementTree::tiled(members),
            hidden: BTreeSet::new(),
            mode: ArrangementMode::Auto,
        }
    }

    pub fn members(&self) -> Vec<MemberRef> {
        self.tree
            .as_ref()
            .map_or_else(Vec::new, ArrangementTree::members)
    }

    pub fn validate(&self, members: &[MemberRef]) -> Result<(), SelectionError> {
        if let Some(tree) = &self.tree {
            tree.validate()?;
        }
        let actual = self.members();
        let set: BTreeSet<_> = actual.iter().collect();
        let expected: BTreeSet<_> = members.iter().collect();
        if actual.len() != set.len()
            || members.len() != expected.len()
            || set != expected
            || !self.hidden.iter().all(|member| set.contains(member))
        {
            return Err(SelectionError::Membership);
        }
        Ok(())
    }

    pub fn replace(&mut self, tree: Option<ArrangementTree>) -> Result<(), SelectionError> {
        let replacement = Self {
            tree,
            hidden: self.hidden.clone(),
            mode: ArrangementMode::Touched,
        };
        replacement.validate(&self.members())?;
        *self = replacement;
        Ok(())
    }

    pub fn hide(&mut self, member: &MemberRef, hidden: bool) -> Result<(), SelectionError> {
        if !self.members().contains(member) {
            return Err(SelectionError::Membership);
        }
        if hidden {
            self.hidden.insert(member.clone());
        } else {
            self.hidden.remove(member);
        }
        Ok(())
    }

    pub fn project(&self) -> ArrangementProjection {
        let mut winners: BTreeMap<WorkWorkspaceKey, MemberRef> = BTreeMap::new();
        let mut suppressed = Vec::new();
        let visible = self.tree.as_ref().and_then(|tree| {
            tree.retaining(&mut |member| {
                if self.hidden.contains(member) {
                    return false;
                }
                if let Some(winner) = winners.get(&member.work) {
                    suppressed.push(SuppressedMember {
                        member: member.clone(),
                        winner: winner.clone(),
                    });
                    false
                } else {
                    winners.insert(member.work.clone(), member.clone());
                    true
                }
            })
        });
        ArrangementProjection {
            visible,
            suppressed,
        }
    }

    /// Incomplete reads can add evidence, but can never erase retained intent.
    pub fn reconcile(&mut self, resolution: &SelectionResolution) {
        let mut members = if resolution.unavailable.is_empty() {
            resolution.members.clone()
        } else {
            self.members()
        };
        let mut selected: BTreeSet<_> = members.iter().cloned().collect();
        if !resolution.unavailable.is_empty() {
            for member in &resolution.members {
                if selected.insert(member.clone()) {
                    members.push(member.clone());
                }
            }
        }
        self.hidden.retain(|member| selected.contains(member));
        if self.mode == ArrangementMode::Auto {
            self.tree = ArrangementTree::tiled(&members);
        } else {
            self.tree = self
                .tree
                .as_ref()
                .and_then(|tree| tree.retaining(&mut |member| selected.contains(member)));
            let retained: BTreeSet<_> = self.members().into_iter().collect();
            let arrivals: Vec<_> = members
                .into_iter()
                .filter(|member| !retained.contains(member))
                .collect();
            if let Some(new) = ArrangementTree::tiled(&arrivals) {
                self.tree = Some(match self.tree.take() {
                    Some(old) => ArrangementTree::Split {
                        axis: ArrangementAxis::Vertical,
                        ratio: 0.5,
                        first: Box::new(old),
                        second: Box::new(new),
                    },
                    None => new,
                });
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SavedSelection {
    pub repository: String,
    pub id: QueryId,
    pub revision: i64,
    pub selection: Selection,
    pub arrangement: Arrangement,
}

/// A coherent read; reconciliation changes this value only, never saved intent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SelectionReading {
    pub saved: SavedSelection,
    pub resolution: SelectionResolution,
    pub arrangement: Arrangement,
}
