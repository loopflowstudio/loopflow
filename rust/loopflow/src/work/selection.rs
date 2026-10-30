//! Typed selection and portable arrangement intent. No provider or surface ownership.
use crate::id::WaveId;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

mod panes;
mod query;
pub use panes::{
    BrowserTab, FileDraft, FileTab, PaneIntent, PaneKind, PaneTree, ShellTab, WorkPaneLayout,
};

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
    /// Session axis: a Task Session's turn ended and it is ready for the person.
    Available,
    /// Session axis: a Task Session is mid-turn.
    Busy,
    Blocked,
    /// Task axis: the Task's own state, as `TaskState` derives it.
    Unstarted,
    Idle,
    Active,
    Moving,
    Shipping,
    /// Board axis: the Task's `TaskSection`.
    Backlog,
    Working,
    Autonomous,
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

impl SelectionResolution {
    /// Member Task IDs in member order, each once.
    pub fn task_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = Vec::new();
        for member in &self.members {
            if member.work.kind == WorkWorkspaceKind::Task && !ids.contains(&member.work.id) {
                ids.push(member.work.id.clone());
            }
        }
        ids
    }
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
    #[error("slot layout: {0}")]
    Slots(String),
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
                                    .push(format!("Task {} state is unavailable", item.name));
                            }
                            // The repository and a Wave list the board's Working
                            // section; the others are `state:backlog`, `state:autonomous`
                            // and `state:done`.
                            in_scope && item.states.contains(&SelectionState::Working)
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

/// `vertical` places children side by side; `horizontal` stacks them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SplitAxis {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ArrangementMode {
    Auto,
    Touched,
}

/// The one reorder gesture: the item at `from` takes `to`'s place.
pub fn move_into_place<T>(order: &mut Vec<T>, from: usize, to: usize) {
    let moved = order.remove(from);
    order.insert(to, moved);
}

/// Occurrence order. Geometry belongs to the slot layout; members fill it in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct Arrangement {
    pub order: Vec<MemberRef>,
    pub hidden: BTreeSet<MemberRef>,
    pub mode: ArrangementMode,
}

/// An authored arrangement. The writer decides `mode`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArrangementEdit {
    pub order: Vec<MemberRef>,
    pub hidden: BTreeSet<MemberRef>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct ArrangementProjection {
    pub visible: Vec<MemberRef>,
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
            order: members.to_vec(),
            hidden: BTreeSet::new(),
            mode: ArrangementMode::Auto,
        }
    }

    pub fn to_edit(&self) -> ArrangementEdit {
        ArrangementEdit {
            order: self.order.clone(),
            hidden: self.hidden.clone(),
        }
    }

    /// The order is a permutation of `members`; hidden members are among them.
    pub fn validate(&self, members: &[MemberRef]) -> Result<(), SelectionError> {
        let set: BTreeSet<_> = self.order.iter().collect();
        let expected: BTreeSet<_> = members.iter().collect();
        if self.order.len() != set.len()
            || members.len() != expected.len()
            || set != expected
            || !self.hidden.iter().all(|member| set.contains(member))
        {
            return Err(SelectionError::Membership);
        }
        Ok(())
    }

    /// The first visible occurrence of `work`: the one its tile shows.
    pub fn winner(&self, work: &WorkWorkspaceKey) -> Option<usize> {
        self.order
            .iter()
            .position(|member| &member.work == work && !self.hidden.contains(member))
    }

    /// Hiding keeps the member's position, so restoring returns it there.
    pub fn hide(&mut self, member: &MemberRef, hidden: bool) -> Result<(), SelectionError> {
        if !self.order.contains(member) {
            return Err(SelectionError::Membership);
        }
        if hidden {
            self.hidden.insert(member.clone());
        } else {
            self.hidden.remove(member);
        }
        Ok(())
    }

    /// The first visible occurrence of each Work wins; later ones are suppressed.
    pub fn project(&self) -> ArrangementProjection {
        let mut winners: BTreeMap<&WorkWorkspaceKey, &MemberRef> = BTreeMap::new();
        let mut projection = ArrangementProjection {
            visible: Vec::new(),
            suppressed: Vec::new(),
        };
        for member in self.order.iter().filter(|m| !self.hidden.contains(m)) {
            if let Some(winner) = winners.get(&member.work) {
                projection.suppressed.push(SuppressedMember {
                    member: member.clone(),
                    winner: (*winner).clone(),
                });
            } else {
                winners.insert(&member.work, member);
                projection.visible.push(member.clone());
            }
        }
        projection
    }

    /// Incomplete reads can add evidence, but can never erase retained intent.
    pub fn reconcile(&mut self, resolution: &SelectionResolution) {
        if resolution.unavailable.is_empty() {
            if self.mode == ArrangementMode::Auto {
                self.order = resolution.members.clone();
            } else {
                let selected: BTreeSet<_> = resolution.members.iter().collect();
                self.order.retain(|member| selected.contains(member));
            }
        }
        let mut present: BTreeSet<_> = self.order.iter().cloned().collect();
        for member in &resolution.members {
            if present.insert(member.clone()) {
                self.order.push(member.clone());
            }
        }
        self.hidden.retain(|member| present.contains(member));
    }
}

/// A Dashboard page's fixed slots. Members fill slots by number, in order, and
/// overflow to further pages; they never add, remove or resize slots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct SlotLayout {
    pub root: SlotNode,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SlotNode {
    Slot {
        slot: usize,
    },
    Split {
        axis: SplitAxis,
        weights: Vec<f64>,
        children: Vec<SlotNode>,
    },
}

impl SlotLayout {
    /// Jack Heart's default, October 20: (1 | (2 | 3)) over (1 | 2 | 3 | 4), golden
    /// proportions. The top band takes 61.8% of the height, slot 1 61.8% of its width.
    pub fn golden() -> Self {
        const GOLDEN: f64 = 0.618;
        let slot = |slot| SlotNode::Slot { slot };
        Self {
            root: SlotNode::Split {
                axis: SplitAxis::Horizontal,
                weights: vec![GOLDEN, 1.0 - GOLDEN],
                children: vec![
                    SlotNode::Split {
                        axis: SplitAxis::Vertical,
                        weights: vec![GOLDEN, 1.0 - GOLDEN],
                        children: vec![
                            slot(0),
                            SlotNode::Split {
                                axis: SplitAxis::Vertical,
                                weights: vec![1.0, 1.0],
                                children: vec![slot(1), slot(2)],
                            },
                        ],
                    },
                    SlotNode::Split {
                        axis: SplitAxis::Vertical,
                        weights: vec![1.0; 4],
                        children: (3..7).map(slot).collect(),
                    },
                ],
            },
        }
    }

    /// `count` equal slots side by side: the Wave view's footer of Tasks.
    pub fn strip(count: usize) -> Self {
        Self {
            root: SlotNode::Split {
                axis: SplitAxis::Vertical,
                weights: vec![1.0; count],
                children: (0..count).map(|slot| SlotNode::Slot { slot }).collect(),
            },
        }
    }

    pub fn slots(&self) -> Vec<usize> {
        fn collect(node: &SlotNode, slots: &mut Vec<usize>) {
            match node {
                SlotNode::Slot { slot } => slots.push(*slot),
                SlotNode::Split { children, .. } => {
                    children.iter().for_each(|child| collect(child, slots))
                }
            }
        }
        let mut slots = Vec::new();
        collect(&self.root, &mut slots);
        slots
    }

    /// `slot` takes `share` of the space it divides with `toward` at the innermost
    /// split separating them; the split's other children keep their weights.
    /// None when the two are the same slot or either is absent.
    pub fn resized(&self, slot: usize, toward: usize, share: f64) -> Option<Self> {
        fn contains(node: &SlotNode, slot: usize) -> bool {
            match node {
                SlotNode::Slot { slot: own } => *own == slot,
                SlotNode::Split { children, .. } => {
                    children.iter().any(|child| contains(child, slot))
                }
            }
        }
        fn resize(node: &mut SlotNode, slot: usize, toward: usize, share: f64) -> bool {
            let SlotNode::Split {
                weights, children, ..
            } = node
            else {
                return false;
            };
            let a = children.iter().position(|child| contains(child, slot));
            let b = children.iter().position(|child| contains(child, toward));
            match (a, b) {
                (Some(a), Some(b)) if a == b => resize(&mut children[a], slot, toward, share),
                (Some(a), Some(b)) => {
                    let total = weights[a] + weights[b];
                    weights[a] = total * share;
                    weights[b] = total * (1.0 - share);
                    true
                }
                _ => false,
            }
        }
        let mut layout = self.clone();
        (slot != toward && resize(&mut layout.root, slot, toward, share)).then_some(layout)
    }

    /// Slots are numbered 0..n exactly once; every split has two or more children
    /// with one finite, positive weight each.
    pub fn validate(&self) -> Result<(), SelectionError> {
        fn check(node: &SlotNode) -> Result<(), SelectionError> {
            let SlotNode::Split {
                weights, children, ..
            } = node
            else {
                return Ok(());
            };
            if children.len() < 2 || weights.len() != children.len() {
                return Err(SelectionError::Slots(
                    "each split needs two or more children and one weight per child".into(),
                ));
            }
            if weights
                .iter()
                .any(|weight| !weight.is_finite() || *weight <= 0.0)
            {
                return Err(SelectionError::Slots(
                    "weights must be finite and positive".into(),
                ));
            }
            children.iter().try_for_each(check)
        }
        check(&self.root)?;
        let mut slots = self.slots();
        slots.sort_unstable();
        if slots.iter().enumerate().any(|(index, slot)| index != *slot) {
            return Err(SelectionError::Slots(
                "slots must be numbered 0..n, each exactly once".into(),
            ));
        }
        Ok(())
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
    /// None keeps the default slot layout.
    pub slots: Option<SlotLayout>,
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
    pub selection: Selection,
    pub saved_revision: Option<i64>,
    pub resolution: SelectionResolution,
    pub arrangement: Arrangement,
    pub projection: ArrangementProjection,
    /// The page geometry visible members fill, in projection order.
    pub slots: SlotLayout,
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
