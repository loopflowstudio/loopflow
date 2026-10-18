//! Typed selection and portable arrangement intent. No provider or surface ownership.
use crate::id::WaveId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

mod panes;
mod query;
pub use panes::{FileDraft, FileTab, PaneIntent, PaneTree, ShellTab, WorkPaneLayout};

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
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
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

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct DashboardId(pub String);

impl DashboardId {
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }
}
impl Default for DashboardId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Selection {
    pub id: SelectionId,
    pub expression: SelectionExpression,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[non_exhaustive]
pub enum SelectionExpression {
    Repository,
    Wave {
        #[schemars(with = "String")]
        wave: WaveId,
    },
    Work {
        work: WorkWorkspaceKey,
    },
    Compose {
        items: Vec<Selection>,
    },
    Query {
        query: DashboardId,
    },
    Filter {
        filter: SelectionFilter,
    },
    Intersect {
        items: Vec<Selection>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "field", content = "value", rename_all = "snake_case")]
pub enum SelectionFilter {
    Wave(String),
    Task(String),
    Kind(WorkWorkspaceKind),
    State(SelectionState),
}

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum SelectionState {
    Open,
    Available,
    Running,
    Blocked,
    Paused,
    Idle,
    Backlog,
    Done,
}

/// The source path contains authored IDs, never array indexes or current focus.
#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, schemars::JsonSchema,
)]
pub struct MemberRef {
    pub source: Vec<SelectionId>,
    pub work: WorkWorkspaceKey,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SelectionResolution {
    pub attention: BTreeSet<WorkWorkspaceKey>,
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
    pub name: String,
    pub states: BTreeSet<SelectionState>,
    /// An unobservable process is different from a complete read with no current reports.
    pub activity_known: bool,
}

fn wave_descendants<'a>(
    repository: &str,
    wave: &'a str,
    work: &'a [SelectionWork],
) -> BTreeSet<&'a str> {
    let mut descendants = BTreeSet::from([wave]);
    loop {
        let before = descendants.len();
        for item in work {
            if item.key.repository == repository
                && item.key.kind == WorkWorkspaceKind::Wave
                && item
                    .parent_wave
                    .as_ref()
                    .is_some_and(|parent| descendants.contains(parent.as_str()))
            {
                descendants.insert(item.key.id.as_str());
            }
        }
        if descendants.len() == before {
            return descendants;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SelectionError {
    #[error("ambiguous Work reference: {0}; use its canonical ID")]
    Ambiguous(String),
    #[error("invalid query: {0}")]
    Query(String),
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
    pub fn referenced_query(&self) -> Option<&DashboardId> {
        match &self.expression {
            SelectionExpression::Query { query } => Some(query),
            SelectionExpression::Compose { items } if items.len() == 1 => {
                items[0].referenced_query()
            }
            _ => None,
        }
    }

    pub fn validate(&self) -> Result<(), SelectionError> {
        fn visit(node: &Selection, ids: &mut BTreeSet<SelectionId>) -> Result<(), SelectionError> {
            if node.id.0.is_empty() || !ids.insert(node.id.clone()) {
                return Err(SelectionError::NodeIdentity(node.id.0.clone()));
            }
            if let SelectionExpression::Compose { items }
            | SelectionExpression::Intersect { items } = &node.expression
            {
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
        queries: &BTreeMap<DashboardId, Selection>,
    ) -> Result<SelectionResolution, SelectionError> {
        self.validate()?;
        let mut result = SelectionResolution {
            attention: BTreeSet::new(),
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
        let selected: BTreeSet<_> = result.members.iter().map(|member| &member.work).collect();
        result.attention = work
            .iter()
            .filter(|item| {
                selected.contains(&item.key)
                    && (item.states.contains(&SelectionState::Available)
                        || item.states.contains(&SelectionState::Blocked))
            })
            .map(|item| item.key.clone())
            .collect();
        Ok(result)
    }

    fn expand(
        &self,
        repository: &str,
        work: &[SelectionWork],
        queries: &BTreeMap<DashboardId, Selection>,
        path: &mut Vec<SelectionId>,
        visiting: &mut BTreeSet<DashboardId>,
        result: &mut SelectionResolution,
    ) -> Result<(), SelectionError> {
        path.push(self.id.clone());
        match &self.expression {
            SelectionExpression::Intersect { items } => {
                let mut members = None::<Vec<MemberRef>>;
                for item in items {
                    let mut reading = SelectionResolution {
                        attention: BTreeSet::new(),
                        members: Vec::new(),
                        unavailable: Vec::new(),
                    };
                    item.expand(repository, work, queries, path, visiting, &mut reading)?;
                    if let Some(members) = &mut members {
                        let keys: BTreeSet<_> =
                            reading.members.iter().map(|member| &member.work).collect();
                        members.retain(|member| keys.contains(&member.work));
                    } else {
                        members = Some(reading.members);
                    }
                    // Even an empty intersection must report unavailable evidence.
                    result.unavailable.extend(reading.unavailable);
                }
                result.members.extend(members.unwrap_or_default());
            }
            SelectionExpression::Filter { filter } => {
                Self::expand_filter(filter, repository, work, path, result)?;
            }
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
                let descendants = wave.map_or_else(BTreeSet::new, |wave| {
                    wave_descendants(repository, wave.as_str(), work)
                });
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArrangementAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
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
        fn collect(tree: &ArrangementTree, members: &mut Vec<MemberRef>) {
            match tree {
                ArrangementTree::Member { member } => members.push(member.clone()),
                ArrangementTree::Split { first, second, .. } => {
                    collect(first, members);
                    collect(second, members);
                }
            }
        }
        let mut members = Vec::new();
        collect(self, &mut members);
        members
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

    fn tiled(members: &[MemberRef], attention: &BTreeSet<WorkWorkspaceKey>) -> Option<Self> {
        fn build(members: &[MemberRef], weights: &[f64]) -> Option<ArrangementTree> {
            match members {
                [] => None,
                [member] => Some(ArrangementTree::Member {
                    member: member.clone(),
                }),
                _ => {
                    let mid = members.len() / 2;
                    let first = weights[..mid].iter().sum::<f64>();
                    let second = weights[mid..].iter().sum::<f64>();
                    // A suppressed subtree keeps a restorable slot, but gets no
                    // share in the ratios of ancestors with two visible children.
                    let ratio = if first > 0.0 && second > 0.0 {
                        (first / (first + second)).clamp(0.1, 0.9)
                    } else {
                        0.5
                    };
                    Some(ArrangementTree::Split {
                        axis: ArrangementAxis::Vertical,
                        ratio,
                        first: Box::new(
                            build(&members[..mid], &weights[..mid]).expect("nonempty first half"),
                        ),
                        second: Box::new(
                            build(&members[mid..], &weights[mid..]).expect("nonempty second half"),
                        ),
                    })
                }
            }
        }
        let mut seen = BTreeSet::new();
        let weights: Vec<_> = members
            .iter()
            .map(|member| {
                if !seen.insert(&member.work) {
                    0.0
                } else if attention.contains(&member.work) {
                    2.0
                } else {
                    1.0
                }
            })
            .collect();
        build(members, &weights)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArrangementMode {
    Auto,
    Touched,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Arrangement {
    pub tree: Option<ArrangementTree>,
    pub hidden: BTreeSet<MemberRef>,
    pub mode: ArrangementMode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ArrangementProjection {
    pub visible: Option<ArrangementTree>,
    pub suppressed: Vec<SuppressedMember>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SuppressedMember {
    pub member: MemberRef,
    pub winner: MemberRef,
}

impl Arrangement {
    pub fn automatic(members: &[MemberRef]) -> Self {
        Self {
            tree: ArrangementTree::tiled(members, &BTreeSet::new()),
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
        if self.mode == ArrangementMode::Auto && resolution.unavailable.is_empty() {
            self.tree = ArrangementTree::tiled(&members, &resolution.attention);
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
            if let Some(new) = ArrangementTree::tiled(&arrivals, &resolution.attention) {
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

/// A custom view with its own identity; members reference existing Work workspaces.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Dashboard {
    pub repository: String,
    pub id: DashboardId,
    pub revision: i64,
    pub name: Option<String>,
    pub pinned: bool,
    pub selection: Selection,
    pub arrangement: Arrangement,
}

/// A coherent read; reconciliation changes this value only, never saved intent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct DashboardReading {
    pub saved: Dashboard,
    pub resolution: SelectionResolution,
    pub arrangement: Arrangement,
}

/// A passive host plan. It describes saved intent and observed Work, not native readiness.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ViewPlan {
    pub name: Option<String>,
    pub pinned: bool,
    pub dashboard_id: Option<DashboardId>,
    pub repository: String,
    pub observed_at: i64,
    pub selection: Selection,
    pub saved_revision: Option<i64>,
    pub resolution: SelectionResolution,
    pub arrangement: Arrangement,
    pub projection: ArrangementProjection,
    pub contents: Vec<ViewContent>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ViewContent {
    pub pane_revision: Option<i64>,
    pub face: Option<PaneIntent>,
    pub work: WorkWorkspaceKey,
    pub title: String,
    pub states: BTreeSet<SelectionState>,
    pub primary_session: Option<String>,
    pub location: Option<ViewLocation>,
    /// An explicit connect command, never a fallback shell or provider launch argv.
    pub argv: Option<Vec<String>>,
    pub unavailable: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ViewLocation {
    pub machine_id: String,
    pub cwd: String,
}
