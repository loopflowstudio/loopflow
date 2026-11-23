//! The outbound tracker queue: one row per change a tracker carries, in the
//! store's words. Writers enqueue inside their own transaction; the drain reads
//! unconfirmed rows in `seq` order and records what the tracker answered.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::Value;

use super::SqliteStore;
use crate::planning_sync::{ActionOwnerKind, PlanningBaseline, TrackerActionStatus, TrackerKind};
use crate::rows::now_unix;
use crate::work::edit::WorkValue;
use crate::{StoreError, StoreResult};
use loopflow_model::durable::{ProjectId, TaskId};
use loopflow_model::id::WaveId;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActionKind {
    Create,
    Field,
    Order,
    State,
    Comment,
    Delete,
    Link,
}

impl ActionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Field => "field",
            Self::Order => "order",
            Self::State => "state",
            Self::Comment => "comment",
            Self::Delete => "delete",
            Self::Link => "link",
        }
    }

    /// `None` for a kind a later lf writes.
    fn parse(value: &str) -> Option<Self> {
        Some(match value {
            "create" => Self::Create,
            "field" => Self::Field,
            "order" => Self::Order,
            "state" => Self::State,
            "comment" => Self::Comment,
            "delete" => Self::Delete,
            "link" => Self::Link,
            _ => return None,
        })
    }

    /// The action a local change to `value` makes; `None` for values no tracker
    /// carries (bindings, ranks, execution facts). Order and creation are
    /// enqueued by their writers: neither is one field's value.
    pub fn of(value: &WorkValue) -> Option<Self> {
        match value {
            WorkValue::TaskTitle(_)
            | WorkValue::TaskDescription(_)
            | WorkValue::TaskAssignee(_)
            | WorkValue::TaskProject(_)
            | WorkValue::TaskParent(_)
            | WorkValue::ProjectName(_)
            | WorkValue::ProjectSummary(_)
            | WorkValue::ProjectDescription(_)
            | WorkValue::ProjectStatus(_) => Some(Self::Field),
            WorkValue::Disposition(_) => Some(Self::State),
            WorkValue::CommentContent(_) => Some(Self::Comment),
            WorkValue::TaskDeletedAt(Some(_)) => Some(Self::Delete),
            _ => None,
        }
    }

    /// Only the newest unsent value of these is worth sending. A link's
    /// subject names what it links, so a newer one replaces an unsent one.
    fn coalesces(self) -> bool {
        matches!(self, Self::Field | Self::Order | Self::State | Self::Link)
    }
}

/// Where an action stands. A person sees `queued`, `sent` and `conflicted`;
/// `confirmed` and `retired` actions have left the queue.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ActionPhase {
    Queued,
    /// Sent with no answer yet; looked for before anything else on its owner.
    Sent,
    Confirmed,
    /// The tracker's value won; `conflict` holds it and `value` the local loser.
    Conflicted,
    Retired,
}

impl ActionPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Sent => "sent",
            Self::Confirmed => "confirmed",
            Self::Conflicted => "conflicted",
            Self::Retired => "retired",
        }
    }

    fn parse(value: &str) -> StoreResult<Self> {
        Ok(match value {
            "queued" => Self::Queued,
            "sent" => Self::Sent,
            "confirmed" => Self::Confirmed,
            "conflicted" => Self::Conflicted,
            "retired" => Self::Retired,
            other => {
                return Err(StoreError::InvalidData(format!(
                    "unknown tracker action phase {other}"
                )))
            }
        })
    }
}

/// The tracker object a request addresses.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ActionOwner {
    Wave(WaveId),
    Project(ProjectId),
    Task(TaskId),
}

impl ActionOwner {
    pub fn kind(&self) -> ActionOwnerKind {
        match self {
            Self::Wave(_) => ActionOwnerKind::Wave,
            Self::Project(_) => ActionOwnerKind::Project,
            Self::Task(_) => ActionOwnerKind::Task,
        }
    }

    fn column(&self) -> &'static str {
        match self {
            Self::Wave(_) => "wave_id",
            Self::Project(_) => "project_id",
            Self::Task(_) => "task_id",
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Wave(id) => id.as_str(),
            Self::Project(id) => id.as_str(),
            Self::Task(id) => id.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackerAction {
    pub seq: i64,
    pub id: String,
    pub owner: ActionOwner,
    pub subject: Option<String>,
    pub kind: ActionKind,
    pub value: Value,
    /// `{revision, value}` as the tracker held it at enqueue.
    pub base: Option<Value>,
    pub conflict: Option<Value>,
    pub phase: ActionPhase,
    pub tracker: TrackerKind,
    pub remote_id: Option<String>,
    pub confirmed_revision: Option<String>,
    pub error: Option<String>,
    pub sent_at: Option<i64>,
    pub looked_at: Option<i64>,
}

/// What a Task's `link` action connects it to, named by its subject.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskLink {
    /// A follow-up relation to another Task, sent once both have tracker objects.
    Relation { id: String, target: TaskId },
    /// The Task's pull request, shown on its tracker item.
    Pr { url: String },
}

const RELATION: &str = "relation:";
const PR: &str = "pr:";

impl TrackerAction {
    /// The link a Task `link` action carries; `None` for every other action.
    pub fn task_link(&self) -> Option<TaskLink> {
        if self.kind != ActionKind::Link || !matches!(self.owner, ActionOwner::Task(_)) {
            return None;
        }
        let subject = self.subject.as_deref()?;
        if let Some(id) = subject.strip_prefix(RELATION) {
            let target = self.value["target"].as_str()?;
            return Some(TaskLink::Relation {
                id: id.to_owned(),
                target: TaskId::from_raw(target),
            });
        }
        subject.strip_prefix(PR).map(|url| TaskLink::Pr {
            url: url.to_owned(),
        })
    }

    /// The text a `comment` action posts.
    pub fn comment_body(&self) -> Option<&str> {
        self.value["value"]["body"].as_str()
    }

    /// The action as a person reads it in `lf repo tracker` and on its Task.
    pub fn status(&self) -> TrackerActionStatus {
        // A field or state action holds a `WorkValue` (`{field, value}`); a
        // conflict holds `{revision, value}` around one.
        let shown = |value: &Value| match self.kind {
            ActionKind::Field | ActionKind::Comment => value["value"].clone(),
            ActionKind::State => value["value"]["planning_state"].clone(),
            ActionKind::Delete => Value::Bool(true),
            _ => value.clone(),
        };
        TrackerActionStatus {
            id: self.id.clone(),
            seq: Some(self.seq),
            owner_kind: self.owner.kind(),
            owner_id: self.owner.id().to_owned(),
            kind: self.kind,
            change: change_word(
                self.kind,
                self.subject.as_deref(),
                self.value["value"]["planning_state"].as_str(),
            ),
            subject: self.subject.clone(),
            phase: self.phase,
            tracker: self.tracker,
            error: self.error.clone(),
            local_value: shown(&self.value),
            tracker_value: self
                .conflict
                .as_ref()
                .map(|conflict| shown(&conflict["value"])),
        }
    }
}

/// What a change does, as a person names it: `title`, `complete`, `comment`.
pub(crate) fn change_word(
    kind: ActionKind,
    subject: Option<&str>,
    planning_state: Option<&str>,
) -> String {
    match (kind, subject) {
        (ActionKind::Field, Some(field)) => field
            .trim_start_matches("task_")
            .trim_start_matches("project_")
            .to_owned(),
        (ActionKind::State, _) => match planning_state {
            Some("completed") => "complete",
            Some("canceled") => "cancel",
            _ => "reopen",
        }
        .to_owned(),
        (kind, _) => kind.as_str().to_owned(),
    }
}

/// How an action leaves the queue.
#[derive(Debug, Clone, PartialEq)]
pub enum Resolution {
    Confirmed {
        revision: Option<String>,
        remote_id: Option<String>,
    },
    /// The tracker's value won: `remote` is `{revision, value}` as it held
    /// it; the action keeps the local value that lost.
    Conflicted {
        remote: Value,
    },
    Retire {
        reason: String,
    },
}

pub(super) const COLUMNS: &str =
    "e.seq,e.id,e.wave_id,e.project_id,e.task_id,e.subject,e.kind,e.value_json,
    e.base_json,e.conflict_json,e.phase,e.tracker,e.remote_id,e.confirmed_revision,e.error,
    e.sent_at,e.looked_at";

// Every action owner reaches its repository through one of these joins. Each
// is a lookup per unconfirmed row, so a pass costs its queue, not the repository's
// Task count.
pub(super) const IN_REPO: &str = "(EXISTS(SELECT 1 FROM tasks t JOIN repository_plans r ON r.id=t.repository_id WHERE t.id=e.task_id AND r.repo=?1)
    OR EXISTS(SELECT 1 FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=e.project_id AND w.repo=?1)
    OR EXISTS(SELECT 1 FROM waves w WHERE w.id=e.wave_id AND w.repo=?1))";

/// A row and its owner; the action is `Err` for a kind a later lf wrote.
pub(super) fn read_action(row: &Row<'_>) -> StoreResult<Result<TrackerAction, ActionOwner>> {
    let wave: Option<String> = row.get(2)?;
    let project: Option<String> = row.get(3)?;
    let task: Option<String> = row.get(4)?;
    let owner = match (wave, project, task) {
        (Some(id), None, None) => ActionOwner::Wave(
            WaveId::parse(&id)
                .map_err(|_| StoreError::InvalidData(format!("invalid Wave id {id}")))?,
        ),
        (None, Some(id), None) => ActionOwner::Project(ProjectId::from_raw(id)),
        (None, None, Some(id)) => ActionOwner::Task(TaskId::from_raw(id)),
        _ => {
            return Err(StoreError::InvalidData(
                "tracker action has no single owner".into(),
            ))
        }
    };
    let kind: String = row.get(6)?;
    let Some(kind) = ActionKind::parse(&kind) else {
        tracing::debug!(%kind, ?owner, "tracker action of an unknown kind holds its owner");
        return Ok(Err(owner));
    };
    let base: Option<String> = row.get(8)?;
    let conflict: Option<String> = row.get(9)?;
    let tracker: String = row.get(11)?;
    Ok(Ok(TrackerAction {
        seq: row.get(0)?,
        id: row.get(1)?,
        owner,
        subject: row.get(5)?,
        kind,
        value: serde_json::from_str(&row.get::<_, String>(7)?)?,
        base: base.map(|base| serde_json::from_str(&base)).transpose()?,
        conflict: conflict
            .map(|conflict| serde_json::from_str(&conflict))
            .transpose()?,
        phase: ActionPhase::parse(&row.get::<_, String>(10)?)?,
        tracker: TrackerKind::parse(&tracker)?,
        remote_id: row.get(12)?,
        confirmed_revision: row.get(13)?,
        error: row.get(14)?,
        sent_at: row.get(15)?,
        looked_at: row.get(16)?,
    }))
}

/// The tracker an owner's actions go to: its binding's, else its pending
/// creation's. An owner with neither enqueues nothing.
pub(crate) fn owner_tracker(
    conn: &Connection,
    owner: &ActionOwner,
) -> StoreResult<Option<TrackerKind>> {
    let bound: Option<Option<String>> = match owner {
        ActionOwner::Task(id) => conn
            .query_row(
                "SELECT (SELECT o.tracker FROM tracker_items o JOIN repository_plans r ON r.repo=o.repo
                    WHERE r.id=t.repository_id AND o.id=t.external_issue_id LIMIT 1)
                 FROM tasks t WHERE t.id=?1 AND t.external_issue_id IS NOT NULL",
                [id.as_str()],
                |row| row.get(0),
            )
            .optional()?,
        ActionOwner::Project(id) => conn
            .query_row(
                "SELECT (SELECT o.tracker FROM tracker_projects o JOIN waves w ON w.repo=o.repo
                    WHERE w.id=p.wave_id AND o.id=p.external_project_id LIMIT 1)
                 FROM projects p WHERE p.id=?1 AND p.external_project_id IS NOT NULL",
                [id.as_str()],
                |row| row.get(0),
            )
            .optional()?,
        ActionOwner::Wave(_) => None,
    };
    let tracker = match bound {
        // A binding acquired before observations recorded their tracker.
        Some(tracker) => Some(tracker.unwrap_or_else(|| "linear".into())),
        None => conn
            .query_row(
                &format!(
                    "SELECT tracker FROM tracker_actions WHERE {}=?1 AND kind='create'
                     AND phase!='retired' ORDER BY seq DESC LIMIT 1",
                    owner.column()
                ),
                [owner.id()],
                |row| row.get(0),
            )
            .optional()?,
    };
    tracker
        .map(|tracker: String| TrackerKind::parse(&tracker))
        .transpose()
}

/// Add one action for a bound or creating owner, retiring older unsent values
/// of the same subject. Returns the action id, or `None` when no tracker carries
/// this owner.
pub(crate) fn enqueue(
    conn: &Connection,
    owner: &ActionOwner,
    kind: ActionKind,
    subject: Option<&str>,
    value: &Value,
    base: Option<&Value>,
) -> StoreResult<Option<String>> {
    let Some(tracker) = owner_tracker(conn, owner)? else {
        return Ok(None);
    };
    insert(conn, owner, kind, subject, value, base, tracker).map(Some)
}

/// Queue what a local save of `value` on `table`/`id` asks a tracker to do.
/// The planning writer calls this for every value a `Local` write changed.
pub(crate) fn enqueue_local(
    conn: &Connection,
    table: &str,
    id: &str,
    value: &WorkValue,
) -> StoreResult<()> {
    let Some(kind) = ActionKind::of(value) else {
        return Ok(());
    };
    let (owner, subject) = match table {
        "tasks" => (ActionOwner::Task(TaskId::from_raw(id)), None),
        "projects" => (ActionOwner::Project(ProjectId::from_raw(id)), None),
        // A comment goes out on its Task, behind the Task's creation.
        "task_comments" => {
            let task: String = conn.query_row(
                "SELECT task_id FROM task_comments WHERE id=?1",
                [id],
                |row| row.get(0),
            )?;
            (ActionOwner::Task(TaskId::from_raw(task)), Some(id))
        }
        _ => return Ok(()),
    };
    if kind == ActionKind::Delete && withdraw_unsent(conn, &owner)? {
        return Ok(());
    }
    let subject = match kind {
        ActionKind::Field => Some(value.field()),
        _ => subject,
    };
    let base = match kind {
        ActionKind::Field => field_baseline(conn, &owner, value)?,
        ActionKind::State => state_baseline(conn, &owner)?,
        ActionKind::Delete => observed_revision(conn, &owner)?.map(|revision| PlanningBaseline {
            revision,
            value: Some(WorkValue::TaskDeletedAt(None)),
        }),
        _ => None,
    };
    enqueue(
        conn,
        &owner,
        kind,
        subject,
        &serde_json::to_value(value)?,
        base.map(serde_json::to_value).transpose()?.as_ref(),
    )?;
    Ok(())
}

/// Deleting an owner whose creation never left: the tracker has nothing to
/// trash, so its unsent actions retire instead of creating it to delete it.
/// A creation already sent may exist there, so its deletion still goes.
fn withdraw_unsent(conn: &Connection, owner: &ActionOwner) -> StoreResult<bool> {
    let unbound = match owner {
        ActionOwner::Task(_) => {
            "NOT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND external_issue_id IS NOT NULL)"
        }
        ActionOwner::Project(_) => {
            "NOT EXISTS(SELECT 1 FROM projects WHERE id=?1 AND external_project_id IS NOT NULL)"
        }
        ActionOwner::Wave(_) => return Ok(false),
    };
    let column = owner.column();
    let unsent: bool = conn.query_row(
        &format!(
            "SELECT {unbound}
                AND EXISTS(SELECT 1 FROM tracker_actions WHERE {column}=?1 AND kind='create' AND phase='queued')
                AND NOT EXISTS(SELECT 1 FROM tracker_actions WHERE {column}=?1 AND kind='create' AND phase<>'queued' AND phase<>'retired')"
        ),
        [owner.id()],
        |row| row.get(0),
    )?;
    if unsent {
        conn.execute(
            &format!(
                "UPDATE tracker_actions SET phase='retired',error='deleted before it was sent'
                 WHERE {column}=?1 AND phase='queued'"
            ),
            [owner.id()],
        )?;
    }
    Ok(unsent)
}

/// Removing `wave`: its Project's unsent actions retire, and an unsent Task
/// creation naming that Project names the Project its Task has now, which the
/// tracker will have. Removal moves a Wave's Tasks out before it retires.
pub(crate) fn retire_wave_in(conn: &Connection, wave: &WaveId) -> StoreResult<()> {
    conn.execute(
        "UPDATE tracker_actions SET phase='retired', error='its Wave was removed'
         WHERE phase='queued' AND project_id IN (SELECT id FROM projects WHERE wave_id=?1)",
        [wave],
    )?;
    let creations: Vec<(String, String, Option<String>, String)> = conn
        .prepare(
            "SELECT e.id,p.id,t.project_id,e.value_json FROM tracker_actions e
             JOIN tasks t ON t.id=e.task_id JOIN projects p ON p.wave_id=?1
             WHERE e.kind='create' AND e.phase='queued'
               AND EXISTS(SELECT 1 FROM json_each(e.value_json) v
                   WHERE json_extract(v.value,'$.field')='task_project'
                     AND json_extract(v.value,'$.value')=p.id)",
        )?
        .query_map([wave], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })?
        .collect::<Result<_, _>>()?;
    for (action, removed, current, values) in creations {
        let values: Vec<WorkValue> =
            crate::work::edit::known_values(serde_json::from_str(&values)?)?
                .into_iter()
                .map(|value| match value {
                    WorkValue::TaskProject(Some(project)) if project == removed => {
                        WorkValue::TaskProject(current.clone())
                    }
                    value => value,
                })
                .collect();
        conn.execute(
            "UPDATE tracker_actions SET value_json=?2 WHERE id=?1",
            params![action, serde_json::to_string(&values)?],
        )?;
    }
    Ok(())
}

/// The owner's tracker observation, joined from its binding.
fn observation_source(owner: &ActionOwner) -> Option<&'static str> {
    match owner {
        ActionOwner::Task(_) => Some(
            "tasks t JOIN repository_plans w ON w.id=t.repository_id
             JOIN tracker_items o ON o.repo=w.repo AND o.tracker IN ('linear','asana') AND o.id=t.external_issue_id
             WHERE t.id=?1",
        ),
        ActionOwner::Project(_) => Some(
            "projects p JOIN waves w ON w.id=p.wave_id
             JOIN tracker_projects o ON o.repo=w.repo AND o.tracker IN ('linear','asana') AND o.id=p.external_project_id
             WHERE p.id=?1",
        ),
        ActionOwner::Wave(_) => None,
    }
}

/// `Some(revision)` when the owner has an observation at all.
fn observed_revision(
    conn: &Connection,
    owner: &ActionOwner,
) -> StoreResult<Option<Option<String>>> {
    let Some(source) = observation_source(owner) else {
        return Ok(None);
    };
    Ok(conn
        .query_row(
            &format!("SELECT o.revision FROM {source}"),
            [owner.id()],
            |row| row.get(0),
        )
        .optional()?)
}

fn state_baseline(conn: &Connection, owner: &ActionOwner) -> StoreResult<Option<PlanningBaseline>> {
    let ActionOwner::Task(id) = owner else {
        return Ok(None);
    };
    Ok(conn
        .query_row(
            &format!(
                "SELECT o.revision,o.state,o.completed,o.completed_at FROM {}",
                observation_source(owner).expect("a Task has an observation source")
            ),
            [id.as_str()],
            |row| {
                Ok(PlanningBaseline {
                    revision: row.get(0)?,
                    value: Some(WorkValue::Disposition(crate::work::edit::Disposition {
                        planning_state: row.get(1)?,
                        planning_completed: i64::from(row.get::<_, bool>(2)?),
                        planning_completed_at: row.get(3)?,
                    })),
                })
            },
        )
        .optional()?)
}

// Read only the edited field's observation. Prose keeps its own revision;
// an explicitly null membership is distinct from an omitted observation.
fn field_baseline(
    conn: &Connection,
    owner: &ActionOwner,
    edit: &WorkValue,
) -> StoreResult<Option<PlanningBaseline>> {
    let Some(source) = observation_source(owner) else {
        return Ok(None);
    };
    let (column, presence) = match edit {
        WorkValue::TaskTitle(_) | WorkValue::ProjectName(_) => ("o.name", "o.name IS NOT NULL"),
        WorkValue::TaskDescription(_) => ("o.description", "o.description IS NOT NULL"),
        WorkValue::ProjectDescription(_) => ("o.description", "o.description_observed_at IS NOT NULL"),
        WorkValue::ProjectSummary(_) => ("o.summary", "o.summary IS NOT NULL"),
        WorkValue::ProjectStatus(_) => ("o.status", "o.status IS NOT NULL"),
        WorkValue::TaskAssignee(_) => ("o.assignee", "o.assignee_observed"),
        WorkValue::TaskProject(_) => ("COALESCE((SELECT id FROM projects WHERE id=o.claimed_project_id OR external_project_id=o.claimed_project_id),o.claimed_project_id)", "o.project_id_observed"),
        WorkValue::TaskParent(_) => ("(SELECT id FROM tasks WHERE external_issue_id=o.parent_id)", "o.parent_id_observed"),
        _ => return Ok(None),
    };
    let revision = if matches!(edit, WorkValue::ProjectDescription(_)) {
        "o.description_revision"
    } else {
        "o.revision"
    };
    let mut query = conn.prepare(&format!(
        "SELECT {revision},{column} FROM {source} AND {presence}"
    ))?;
    let baseline = query
        .query_and_then([owner.id()], |row| -> StoreResult<PlanningBaseline> {
            let value = Value::from(row.get::<_, Option<String>>(1)?);
            Ok(PlanningBaseline {
                revision: row.get(0)?,
                value: Some(serde_json::from_value(
                    serde_json::json!({"field": edit.field(), "value": value}),
                )?),
            })
        })?
        .next()
        .transpose();
    baseline
}

fn insert(
    conn: &Connection,
    owner: &ActionOwner,
    kind: ActionKind,
    subject: Option<&str>,
    value: &Value,
    base: Option<&Value>,
    tracker: TrackerKind,
) -> StoreResult<String> {
    let column = owner.column();
    // A later save answers a value the tracker won.
    conn.execute(
        &format!(
            "UPDATE tracker_actions SET phase='retired',error='answered by a later save'
             WHERE {column}=?1 AND kind=?2 AND subject IS ?3 AND phase='conflicted'"
        ),
        params![owner.id(), kind.as_str(), subject],
    )?;
    if kind.coalesces() {
        conn.execute(
            &format!(
                "UPDATE tracker_actions SET phase='retired',error='superseded by a later save'
                 WHERE {column}=?1 AND kind=?2 AND subject IS ?3 AND phase='queued'"
            ),
            params![owner.id(), kind.as_str(), subject],
        )?;
    }
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        &format!(
            "INSERT INTO tracker_actions(id,{column},subject,kind,value_json,base_json,phase,tracker)
             VALUES(?1,?2,?3,?4,?5,?6,'queued',?7)"
        ),
        params![
            id,
            owner.id(),
            subject,
            kind.as_str(),
            value.to_string(),
            base.map(Value::to_string),
            tracker.as_str()
        ],
    )?;
    Ok(id)
}

fn creation_baseline(conn: &Connection, owner: &ActionOwner) -> StoreResult<Vec<WorkValue>> {
    Ok(match owner {
        ActionOwner::Task(id) => {
            let task = super::children::task_on(conn, id)?.ok_or(StoreError::NotFound)?;
            vec![
                WorkValue::TaskTitle(Some(task.title)),
                WorkValue::TaskDescription(Some(task.description)),
                WorkValue::TaskAssignee(task.assignee),
                WorkValue::TaskDueDate(task.due_date),
                WorkValue::TaskProject(task.project_id.as_ref().map(ToString::to_string)),
                WorkValue::TaskParent(task.parent_task_id.as_ref().map(ToString::to_string)),
            ]
        }
        ActionOwner::Project(id) => {
            let project = super::children::project_in(conn, id)?;
            vec![
                WorkValue::ProjectName(Some(project.name)),
                WorkValue::ProjectSummary(project.summary),
                WorkValue::ProjectDescription(Some(project.description)),
                WorkValue::ProjectStatus(project.status),
            ]
        }
        ActionOwner::Wave(_) => {
            return Err(StoreError::InvalidData(
                "a Wave is connected, never created, by a tracker".into(),
            ))
        }
    })
}

/// Resolve an unconfirmed action; false when it resolved already.
pub(crate) fn resolve_in(
    conn: &Connection,
    id: &str,
    resolution: &Resolution,
) -> StoreResult<bool> {
    let changed = match resolution {
        Resolution::Confirmed {
            revision,
            remote_id,
        } => conn.execute(
            "UPDATE tracker_actions SET phase='confirmed',confirmed_revision=?2,
             remote_id=COALESCE(?3,remote_id),error=NULL
             WHERE id=?1 AND phase IN ('queued','sent')",
            params![id, revision, remote_id],
        )?,
        Resolution::Conflicted { remote } => conn.execute(
            "UPDATE tracker_actions SET phase='conflicted',conflict_json=?2,error=NULL
             WHERE id=?1 AND phase IN ('queued','sent')",
            params![id, remote.to_string()],
        )?,
        Resolution::Retire { reason } => conn.execute(
            "UPDATE tracker_actions SET phase='retired',error=?2
             WHERE id=?1 AND phase IN ('queued','sent')",
            params![id, reason],
        )?,
    };
    Ok(changed == 1)
}

/// An owner's unconfirmed actions in send order, inside the caller's transaction.
pub(crate) fn unconfirmed_for_in(
    conn: &Connection,
    owner: &ActionOwner,
) -> StoreResult<Vec<TrackerAction>> {
    let mut query = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM tracker_actions e WHERE e.{}=?1
         AND e.phase IN ('queued','sent') ORDER BY e.seq",
        owner.column()
    ))?;
    let rows = query.query_and_then([owner.id()], read_action)?;
    rows.filter_map(|row| row.map(Result::ok).transpose())
        .collect()
}

/// The newest unconfirmed value of each field an owner has queued.
pub(crate) fn queued_changes_in(
    conn: &Connection,
    owner: &ActionOwner,
) -> StoreResult<Vec<crate::planning_sync::PlanningChange>> {
    let mut changes: Vec<crate::planning_sync::PlanningChange> = Vec::new();
    for action in unconfirmed_for_in(conn, owner)? {
        if action.kind != ActionKind::Field {
            continue;
        }
        let change = crate::planning_sync::PlanningChange {
            id: action.id,
            value: serde_json::from_value(action.value)?,
            base: action.base.map(serde_json::from_value).transpose()?,
        };
        changes.retain(|older| older.value.field() != change.value.field());
        changes.push(change);
    }
    Ok(changes)
}

/// Acceptance of the tracker's copy of `owner` at `revision`, before it is
/// written: sent actions the copy answers resolve (the rule the drain
/// uses), unsent values rebase onto it, and every unconfirmed value replaces the
/// observed one in `edits`, so the write keeps local intent until it resolves.
/// Returns the resolutions made, so the caller can tell what was adopted.
pub(crate) fn reconcile_in(
    conn: &Connection,
    owner: &ActionOwner,
    revision: Option<&str>,
    edits: &mut [WorkValue],
) -> StoreResult<Vec<(TrackerAction, Resolution)>> {
    let actions: Vec<TrackerAction> = unconfirmed_for_in(conn, owner)?
        .into_iter()
        .filter(|action| {
            matches!(
                action.kind,
                ActionKind::Field | ActionKind::State | ActionKind::Delete
            )
        })
        .collect();
    let observed = crate::planning::rules::Observed {
        revision: revision.map(str::to_owned),
        values: edits.to_vec(),
        ..Default::default()
    };
    let seen = crate::planning::revision_nanos(revision)?;
    retire_changed_again(conn, owner, seen, edits)?;
    let mut resolved = Vec::new();
    for action in &actions {
        if action.phase != ActionPhase::Sent {
            continue;
        }
        // An older copy than the one the action was based on says nothing.
        let base = action
            .base
            .as_ref()
            .and_then(|base| base["revision"].as_str());
        if seen < crate::planning::revision_nanos(base)? {
            continue;
        }
        if let crate::planning::rules::Decision::Resolve(resolution) =
            crate::planning::rules::decide(action, Some(&observed), false)
        {
            if resolve_in(conn, &action.id, &resolution)? {
                resolved.push((action.clone(), resolution));
            }
        }
    }
    // The newest unconfirmed value of each field is the one local intent holds.
    let mut overlaid = std::collections::HashSet::new();
    for action in actions.iter().rev() {
        if resolved.iter().any(|(done, _)| done.id == action.id)
            || action.kind == ActionKind::Delete
        {
            continue;
        }
        let Ok(value) = serde_json::from_value::<WorkValue>(action.value.clone()) else {
            continue;
        };
        if !overlaid.insert(value.field()) {
            continue;
        }
        let Some(edit) = edits.iter_mut().find(|edit| edit.field() == value.field()) else {
            continue;
        };
        if *edit == value {
            continue;
        }
        let base = action
            .base
            .as_ref()
            .and_then(|base| serde_json::from_value::<WorkValue>(base["value"].clone()).ok());
        if action.phase == ActionPhase::Queued && base.as_ref() != Some(edit) {
            // The unsent value now replaces what the tracker holds.
            conn.execute(
                "UPDATE tracker_actions SET base_json=?2 WHERE id=?1",
                params![
                    action.id,
                    serde_json::to_string(&PlanningBaseline {
                        revision: revision.map(str::to_owned),
                        value: Some(edit.clone()),
                    })?
                ],
            )?;
        }
        *edit = value;
    }
    Ok(resolved)
}

/// A value the tracker won stops being lost once the tracker changes that field
/// again: the person it lost to moved on, so there is nothing left to answer.
fn retire_changed_again(
    conn: &Connection,
    owner: &ActionOwner,
    seen: Option<i64>,
    edits: &[WorkValue],
) -> StoreResult<()> {
    let mut query = conn.prepare(&format!(
        "SELECT {COLUMNS} FROM tracker_actions e WHERE e.{}=?1 AND e.phase='conflicted'",
        owner.column()
    ))?;
    let lost = query
        .query_and_then([owner.id()], read_action)?
        .filter_map(|row| row.map(Result::ok).transpose())
        .collect::<StoreResult<Vec<_>>>()?;
    for action in lost {
        let Some(conflict) = &action.conflict else {
            continue;
        };
        let Ok(won) = serde_json::from_value::<WorkValue>(conflict["value"].clone()) else {
            continue;
        };
        let newer = seen > crate::planning::revision_nanos(conflict["revision"].as_str())?;
        let changed = edits
            .iter()
            .any(|edit| edit.field() == won.field() && *edit != won);
        if newer && changed {
            conn.execute(
                "UPDATE tracker_actions SET phase='retired',error=?2 WHERE id=?1 AND phase='conflicted'",
                params![
                    action.id,
                    format!("changed again in {}", action.tracker.display_name())
                ],
            )?;
        }
    }
    Ok(())
}

/// Called by every ingestion path before it can allocate a new local identity.
/// An observation of an id this store created claims the owner whose `create`
/// was sent with it (both trackers carry the owner's uuid): the creation
/// resolves, actions queued behind it take its values as their base, and the
/// owner binds. Ordinary title matches do nothing.
pub(crate) fn attach_in(
    conn: &Connection,
    repo: &str,
    project: bool,
    external: &str,
    revision: Option<&str>,
    observed: &[WorkValue],
) -> StoreResult<()> {
    let Ok(uuid) = uuid::Uuid::parse_str(external) else {
        return Ok(());
    };
    let sql = if project {
        "SELECT o.id,e.id,e.value_json FROM projects o JOIN waves w ON w.id=o.wave_id
         JOIN tracker_actions e ON e.project_id=o.id AND e.kind='create' AND e.phase='sent'
         WHERE w.repo=?1 AND o.external_project_id IS NULL AND substr(o.id,6)=?2
         ORDER BY e.seq DESC LIMIT 1"
    } else {
        "SELECT o.id,e.id,e.value_json FROM tasks o JOIN repository_plans w ON w.id=o.repository_id
         JOIN tracker_actions e ON e.task_id=o.id AND e.kind='create' AND e.phase='sent'
         WHERE w.repo=?1 AND o.external_issue_id IS NULL AND substr(o.id,6)=?2
         ORDER BY e.seq DESC LIMIT 1"
    };
    let created: Option<(String, String, String)> = conn
        .query_row(sql, params![repo, uuid.simple().to_string()], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?))
        })
        .optional()?;
    let Some((id, action, baseline)) = created else {
        return Ok(());
    };
    let owner = if project {
        ActionOwner::Project(ProjectId::from_raw(id.clone()))
    } else {
        ActionOwner::Task(TaskId::from_raw(id.clone()))
    };
    resolve_in(
        conn,
        &action,
        &Resolution::Confirmed {
            revision: revision.map(str::to_owned),
            remote_id: Some(external.to_owned()),
        },
    )?;
    let column = owner.column();
    let mut bases: Vec<(ActionKind, Option<&str>, WorkValue)> = Vec::new();
    let baseline = crate::work::edit::known_values(serde_json::from_str(&baseline)?)?;
    for value in &baseline {
        bases.push((ActionKind::Field, Some(value.field()), value.clone()));
    }
    if let Some(state) = observed
        .iter()
        .find(|value| matches!(value, WorkValue::Disposition(_)))
    {
        bases.push((ActionKind::State, None, state.clone()));
    }
    if !project {
        bases.push((ActionKind::Delete, None, WorkValue::TaskDeletedAt(None)));
    }
    for (kind, subject, value) in bases {
        let base = PlanningBaseline {
            revision: revision.map(str::to_owned),
            value: Some(value),
        };
        conn.execute(
            &format!(
                "UPDATE tracker_actions SET base_json=?4 WHERE {column}=?1 AND kind=?2 AND subject IS ?3
                 AND base_json IS NULL AND phase IN ('queued','sent')"
            ),
            params![id, kind.as_str(), subject, serde_json::to_string(&base)?],
        )?;
    }
    let bind = if project {
        WorkValue::ProjectLinearId(Some(external.to_owned()))
    } else {
        WorkValue::TaskLinearId(Some(external.to_owned()))
    };
    super::planning_write::tracker(conn, &crate::work::edit::WorkEdit::for_target(&id, &[bind]))?;
    Ok(())
}

/// Unbound Projects of live Waves, then Tasks, made on this Machine in `repo`
/// with no live `create`. A Project is made here unless another Machine
/// stamped its Wave.
fn uncreated(conn: &Connection, repo: &str) -> StoreResult<Vec<ActionOwner>> {
    let mut query = conn.prepare_cached(
        "SELECT 'project',p.id FROM projects p JOIN waves w ON w.id=p.wave_id
         WHERE w.repo=?1 AND w.retired_at IS NULL AND p.external_project_id IS NULL
         AND NOT EXISTS(SELECT 1 FROM plan_stamps s WHERE s.owner_id=p.id AND s.field='project_wave'
             AND s.machine_id!=(SELECT id FROM machines WHERE is_local=1))
         AND NOT EXISTS(SELECT 1 FROM tracker_actions e WHERE e.project_id=p.id AND e.kind='create' AND e.phase!='retired')
         UNION ALL
         SELECT 'task',t.id FROM tasks t JOIN repository_plans r ON r.id=t.repository_id
         WHERE r.repo=?1 AND t.external_issue_id IS NULL AND t.planning_deleted_at IS NULL
         AND EXISTS(SELECT 1 FROM task_creation_intents c WHERE c.task_id=t.id)
         AND NOT EXISTS(SELECT 1 FROM tracker_actions e WHERE e.task_id=t.id AND e.kind='create' AND e.phase!='retired')",
    )?;
    let rows = query.query_map([repo], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.map(|row| {
        let (kind, id) = row?;
        Ok(if kind == "project" {
            ActionOwner::Project(ProjectId::from_raw(id))
        } else {
            ActionOwner::Task(TaskId::from_raw(id))
        })
    })
    .collect()
}

impl SqliteStore {
    /// Give every unbound local Project and Task in `repo` a `create` for
    /// `tracker`, including those made before the tracker was connected, then
    /// queue their comments behind it. Sends nothing.
    pub fn enqueue_tracker_creations(
        &self,
        repo: &str,
        tracker: TrackerKind,
    ) -> StoreResult<usize> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        // Every pass asks; almost every pass finds nothing, so it reads first
        // and takes the shared write lock only when there is something to queue.
        if uncreated(&conn, repo)?.is_empty() {
            return Ok(0);
        }
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let owners = uncreated(&tx, repo)?;
        for owner in &owners {
            let baseline = serde_json::to_value(creation_baseline(&tx, owner)?)?;
            insert(
                &tx,
                owner,
                ActionKind::Create,
                None,
                &baseline,
                None,
                tracker,
            )?;
            if let ActionOwner::Task(task) = owner {
                // A creation opens the item; a Task already started or done
                // before the tracker was connected says so next.
                let moved = super::planning_write::read(&tx, "tasks", task.as_str())?
                    .into_iter()
                    .find(|value| match value {
                        WorkValue::Disposition(disposition) => {
                            disposition.planning_completed != 0
                                || disposition
                                    .planning_state
                                    .as_deref()
                                    .is_some_and(|state| state != "unstarted")
                        }
                        _ => false,
                    });
                if let Some(disposition) = moved {
                    insert(
                        &tx,
                        owner,
                        ActionKind::State,
                        None,
                        &serde_json::to_value(disposition)?,
                        None,
                        tracker,
                    )?;
                }
                let mut comments = tx.prepare(
                    "SELECT id FROM task_comments WHERE task_id=?1 ORDER BY created_at,id",
                )?;
                let ids = comments
                    .query_map([task.as_str()], |row| row.get::<_, String>(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                for comment in ids {
                    let values = super::planning_write::read(&tx, "task_comments", &comment)?;
                    let Some(content) = values
                        .into_iter()
                        .find(|value| matches!(value, WorkValue::CommentContent(_)))
                    else {
                        continue;
                    };
                    insert(
                        &tx,
                        owner,
                        ActionKind::Comment,
                        Some(&comment),
                        &serde_json::to_value(content)?,
                        None,
                        tracker,
                    )?;
                }
            }
        }
        tx.commit()?;
        Ok(owners.len())
    }

    /// Show the Task's pull request in its tracker: `state` is what the PR
    /// line says (Open, Merged, Abandoned). Returns the action id, or `None`
    /// when no tracker carries the Task. A later state replaces an unsent one;
    /// the same state again is the action that already carries it, sent or
    /// not, so a push or refresh with nothing new to say queues nothing.
    pub fn enqueue_pr_link(
        &self,
        task: &TaskId,
        url: &str,
        number: u32,
        state: &str,
        comment_id: Option<&str>,
    ) -> StoreResult<Option<String>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let subject = format!("{PR}{url}");
        let latest: Option<(String, String)> = tx
            .query_row(
                "SELECT id, value_json FROM tracker_actions
                 WHERE task_id=?1 AND subject=?2 AND kind='link'
                   AND phase IN ('queued','sent','confirmed')
                 ORDER BY seq DESC LIMIT 1",
                params![task.as_str(), subject],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        if let Some((id, carried)) = latest {
            let carried: Value = serde_json::from_str(&carried)?;
            if carried["number"] == number && carried["state"] == state {
                return Ok(Some(id));
            }
        }
        let value = serde_json::json!({
            "number": number,
            "state": state,
            "comment_id": comment_id,
        });
        let id = enqueue(
            &tx,
            &ActionOwner::Task(task.clone()),
            ActionKind::Link,
            Some(&subject),
            &value,
            None,
        )?;
        tx.commit()?;
        Ok(id)
    }

    /// One action by id, resolved or not.
    pub fn tracker_action(&self, id: &str) -> StoreResult<Option<TrackerAction>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let action = conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM tracker_actions e WHERE e.id=?1"),
                [id],
                |row| Ok(read_action(row)),
            )
            .optional()?
            .transpose()?;
        Ok(action.and_then(Result::ok))
    }

    /// Unconfirmed actions for `repo` in send order.
    pub fn unconfirmed_tracker_actions(&self, repo: &str) -> StoreResult<Vec<TrackerAction>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM tracker_actions e
             WHERE e.phase IN ('queued','sent') AND {IN_REPO} ORDER BY e.seq"
        ))?;
        // An owner with a row this lf cannot read waits from that row on, for
        // the lf that wrote it; other owners drain.
        let mut held = std::collections::HashSet::new();
        let mut actions = Vec::new();
        for row in query.query_and_then([repo], read_action)? {
            match row? {
                Ok(action) if !held.contains(&action.owner) => actions.push(action),
                Ok(_) => {}
                Err(owner) => {
                    held.insert(owner);
                }
            }
        }
        Ok(actions)
    }

    /// Every action for one owner, oldest first, resolved ones included.
    pub fn tracker_actions_for(&self, owner: &ActionOwner) -> StoreResult<Vec<TrackerAction>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM tracker_actions e WHERE e.{}=?1 ORDER BY e.seq",
            owner.column()
        ))?;
        let actions = query.query_and_then([owner.id()], read_action)?;
        actions
            .filter_map(|row| row.map(Result::ok).transpose())
            .collect()
    }

    /// Mark an unconfirmed action sent before its request leaves. Returns false
    /// when it resolved meanwhile.
    pub fn attempt_tracker_action(&self, id: &str) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE tracker_actions SET phase='sent',sent_at=?2,error=NULL
             WHERE id=?1 AND phase IN ('queued','sent')",
            params![id, now_unix()],
        )? == 1)
    }

    /// A request the tracker rejected before acting leaves the action unsent.
    pub fn reject_tracker_action(&self, id: &str, error: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE tracker_actions SET phase='queued',error=?2 WHERE id=?1 AND phase='sent'",
            params![id, error],
        )?;
        Ok(())
    }

    /// Record why the last request did not confirm it, keeping the phase.
    pub fn tracker_action_error(&self, id: &str, error: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE tracker_actions SET error=?2 WHERE id=?1
             AND phase IN ('queued','sent') AND error IS NOT ?2",
            params![id, error],
        )?;
        Ok(())
    }

    /// Part of a multi-request action landed: what the tracker now holds is
    /// the base the rest of it, or a lost reply, is judged against.
    pub fn rebase_tracker_action(&self, id: &str, base: &Value) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE tracker_actions SET base_json=?2 WHERE id=?1 AND phase IN ('queued','sent')",
            params![id, base.to_string()],
        )?;
        Ok(())
    }

    /// Record a look in the tracker for a sent action's lost reply.
    pub fn look_tracker_action(&self, id: &str, at: i64) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE tracker_actions SET looked_at=?2 WHERE id=?1 AND phase='sent'",
            params![id, at],
        )?;
        Ok(())
    }

    pub fn resolve_tracker_action(&self, id: &str, resolution: &Resolution) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        resolve_in(&conn, id, resolution)
    }
}

#[cfg(test)]
mod tests {
    use super::{ActionKind, ActionOwner, ActionPhase, Resolution};
    use crate::planning_sync::TrackerKind;
    use crate::sqlite::SqliteStore;
    use crate::work::edit::WorkValue;
    use loopflow_model::durable::TaskId;
    use serde_json::json;

    fn store() -> (tempfile::TempDir, SqliteStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("db")).unwrap();
        (dir, store)
    }

    fn task(store: &SqliteStore, bound: bool) -> TaskId {
        let id = TaskId::new();
        store
            .create_task(&crate::work::task::NewTask {
                parent_task_id: None,
                repository_id: crate::test_fixture::repository(store, "/repo"),
                id: id.clone(),
                project_id: None,
                title: "Title".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        if bound {
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE tasks SET external_issue_id=?2 WHERE id=?1",
                    rusqlite::params![id.as_str(), uuid::Uuid::new_v4().to_string()],
                )
                .unwrap();
        }
        id
    }

    fn enqueue(
        store: &SqliteStore,
        task: &TaskId,
        kind: ActionKind,
        subject: &str,
    ) -> Option<String> {
        let conn = store.conn.lock().unwrap();
        super::enqueue(
            &conn,
            &ActionOwner::Task(task.clone()),
            kind,
            Some(subject),
            &json!({"field":"task_title","value":subject}),
            None,
        )
        .unwrap()
    }

    #[test]
    fn a_local_save_queues_what_a_tracker_carries_and_a_tracker_fact_queues_nothing() {
        let (_dir, store) = store();
        let task = task(&store, true);
        let save = |origin, values: &[WorkValue]| {
            let conn = store.conn.lock().unwrap();
            crate::sqlite::planning_write::write(
                &conn,
                &crate::work::edit::WorkEdit::for_target(task.as_str(), values),
                origin,
            )
            .unwrap();
        };
        save(
            crate::sqlite::planning_write::WriteOrigin::Local,
            &[
                WorkValue::TaskTitle(Some("Renamed".into())),
                WorkValue::TaskRank(4),
                WorkValue::TaskDeletedAt(Some(1)),
            ],
        );
        save(
            crate::sqlite::planning_write::WriteOrigin::Tracker,
            &[WorkValue::TaskDescription(Some("Theirs".into()))],
        );
        let actions: Vec<_> = store
            .tracker_actions_for(&ActionOwner::Task(task))
            .unwrap()
            .into_iter()
            .map(|action| (action.kind, action.subject))
            .collect();
        assert_eq!(
            actions,
            [
                (ActionKind::Field, Some("task_title".into())),
                (ActionKind::Delete, None)
            ]
        );
    }

    #[test]
    fn an_owner_no_tracker_carries_enqueues_nothing() {
        let (_dir, store) = store();
        let task = task(&store, false);
        assert_eq!(
            enqueue(&store, &task, ActionKind::Field, "task_title"),
            None
        );
        assert!(store
            .unconfirmed_tracker_actions("/repo")
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_later_save_retires_the_unsent_value_and_keeps_the_sent_one() {
        let (_dir, store) = store();
        let task = task(&store, true);
        let sent = enqueue(&store, &task, ActionKind::Field, "task_title").unwrap();
        assert!(store.attempt_tracker_action(&sent).unwrap());
        let unsent = enqueue(&store, &task, ActionKind::Field, "task_title").unwrap();
        let latest = enqueue(&store, &task, ActionKind::Field, "task_title").unwrap();
        let comment = enqueue(&store, &task, ActionKind::Comment, "c1").unwrap();
        let again = enqueue(&store, &task, ActionKind::Comment, "c1").unwrap();
        let phases: Vec<_> = store
            .tracker_actions_for(&ActionOwner::Task(task))
            .unwrap()
            .into_iter()
            .map(|action| (action.id, action.phase))
            .collect();
        assert_eq!(
            phases,
            vec![
                (sent, ActionPhase::Sent),
                (unsent, ActionPhase::Retired),
                (latest, ActionPhase::Queued),
                (comment, ActionPhase::Queued),
                (again, ActionPhase::Queued),
            ]
        );
    }

    #[test]
    fn connecting_a_tracker_queues_a_creation_for_tasks_made_before() {
        let (_dir, store) = store();
        let earlier = task(&store, false);
        let bound = task(&store, true);
        assert_eq!(
            store
                .enqueue_tracker_creations("/repo", TrackerKind::Asana)
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .enqueue_tracker_creations("/repo", TrackerKind::Asana)
                .unwrap(),
            0,
            "a second pass queues nothing new"
        );
        let actions = store.unconfirmed_tracker_actions("/repo").unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].owner, ActionOwner::Task(earlier.clone()));
        assert_eq!(actions[0].kind, ActionKind::Create);
        assert_eq!(actions[0].tracker, TrackerKind::Asana);
        assert_eq!(actions[0].value[0]["field"], "task_title");
        // The creating owner now carries later edits to the same tracker.
        assert!(enqueue(&store, &earlier, ActionKind::Field, "task_title").is_some());
        assert!(store
            .tracker_actions_for(&ActionOwner::Task(bound))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_project_made_here_is_created_in_the_tracker_and_one_from_another_machine_is_not() {
        let (_dir, store) = store();
        let wave = crate::work::wave::Wave::new(
            loopflow_model::id::WaveId::new(),
            "billing".into(),
            "/repo".into(),
        );
        crate::test_fixture::wave(&store, &wave);
        let here = store.seed_project(wave.id()).unwrap().id;
        // The released lf made Projects before fields were stamped, and
        // another Machine's Project arrives with its own stamps.
        let (older, theirs, removed) = (
            loopflow_model::id::WaveId::new(),
            loopflow_model::id::WaveId::new(),
            loopflow_model::id::WaveId::new(),
        );
        let unstamped = loopflow_model::durable::ProjectId::new();
        // A removed Wave's Project is never made in a tracker connected later.
        let retired = loopflow_model::durable::ProjectId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(&format!(
                "INSERT INTO waves(id,name,repo,created_at,retired_at) VALUES('{older}','older','/repo',1,NULL),('{theirs}','theirs','/repo',1,NULL),('{removed}','removed','/repo',1,2);
                 INSERT INTO projects(id,wave_id,created_at,updated_at,description,project_name,project_slug)
                 VALUES('{unstamped}','{older}',1,1,'','Earlier','earlier'),('{retired}','{removed}',1,1,'','Removed','removed')"
            ))
            .unwrap();
        let elsewhere = loopflow_model::durable::ProjectId::new();
        crate::test_fixture::arrive(
            &store,
            "/repo",
            &crate::work::edit::WorkEdit::for_target(
                elsewhere.as_str(),
                &[
                    WorkValue::ProjectWave(theirs.to_string()),
                    WorkValue::ProjectName(Some("Theirs".into())),
                ],
            ),
        )
        .unwrap();
        assert_eq!(
            store
                .enqueue_tracker_creations("/repo", TrackerKind::Linear)
                .unwrap(),
            2
        );
        let mut created: Vec<_> = store
            .unconfirmed_tracker_actions("/repo")
            .unwrap()
            .into_iter()
            .map(|action| action.owner)
            .collect();
        created.sort_by_key(|owner| format!("{owner:?}"));
        let mut expected = vec![ActionOwner::Project(here), ActionOwner::Project(unstamped)];
        expected.sort_by_key(|owner| format!("{owner:?}"));
        assert_eq!(created, expected);
    }

    #[test]
    fn a_removed_waves_project_is_not_created_in_the_tracker() {
        let (_dir, store) = store();
        let wave = crate::work::wave::Wave::new(
            loopflow_model::id::WaveId::new(),
            "gone".into(),
            "/repo".into(),
        );
        crate::test_fixture::wave(&store, &wave);
        let project = store.seed_project(wave.id()).unwrap().id;
        store
            .enqueue_tracker_creations("/repo", TrackerKind::Linear)
            .unwrap();
        assert_eq!(store.unconfirmed_tracker_actions("/repo").unwrap().len(), 1);
        store
            .retire_waves(std::slice::from_ref(wave.id()), "removed")
            .unwrap();
        assert!(store
            .unconfirmed_tracker_actions("/repo")
            .unwrap()
            .is_empty());
        let actions = store
            .tracker_actions_for(&ActionOwner::Project(project))
            .unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].phase, ActionPhase::Retired);
    }

    #[test]
    fn a_task_moved_out_of_a_removed_wave_is_created_in_its_new_project() {
        let (_dir, store) = store();
        let wave = |slug: &str| {
            let wave = crate::work::wave::Wave::new(
                loopflow_model::id::WaveId::new(),
                slug.into(),
                "/repo".into(),
            );
            crate::test_fixture::wave(&store, &wave);
            wave
        };
        let (gone, kept) = (wave("gone"), wave("kept"));
        let gone_project = store.seed_project(gone.id()).unwrap().id;
        let kept_project = store.seed_project(kept.id()).unwrap().id;
        let moved = task(&store, false);
        store.move_task(&moved, None, Some(gone.id())).unwrap();
        store
            .enqueue_tracker_creations("/repo", TrackerKind::Linear)
            .unwrap();
        // `lf wave remove` moves the Wave's Tasks out, then retires it.
        store
            .move_task(&moved, Some(&gone_project), Some(kept.id()))
            .unwrap();
        store
            .retire_waves(std::slice::from_ref(gone.id()), "removed")
            .unwrap();
        let creation = store
            .tracker_actions_for(&ActionOwner::Task(moved))
            .unwrap()
            .into_iter()
            .find(|action| action.kind == ActionKind::Create)
            .unwrap();
        assert_eq!(creation.phase, ActionPhase::Queued);
        let project = creation
            .value
            .as_array()
            .unwrap()
            .iter()
            .find(|value| value["field"] == "task_project")
            .unwrap()["value"]
            .clone();
        assert_eq!(project, json!(kept_project.as_str()));
    }

    #[test]
    fn renaming_a_wave_renames_its_project_in_the_tracker_too() {
        let (_dir, store) = store();
        let rename = |wave: &crate::work::wave::Wave, to: &str| {
            store
                .relocate_waves(&[crate::WaveLocatorUpdate {
                    wave_id: wave.id().clone(),
                    expected_repo: "/repo".into(),
                    expected_slug: wave.slug().into(),
                    target: crate::work::wave::WaveLocator::new(
                        crate::address::CanonicalRepo::from_canonical_path("/repo".into()),
                        to,
                    )
                    .unwrap(),
                }])
                .unwrap();
        };
        let local = crate::work::wave::Wave::new(
            loopflow_model::id::WaveId::new(),
            "scratchpad".into(),
            "/repo".into(),
        );
        let linked = crate::work::wave::Wave::new(
            loopflow_model::id::WaveId::new(),
            "billing".into(),
            "/repo".into(),
        );
        crate::test_fixture::wave(&store, &local);
        crate::test_fixture::wave(&store, &linked);
        let local_project = store.seed_project(local.id()).unwrap().id;
        let linked_project = store.seed_project(linked.id()).unwrap().id;
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE projects SET external_project_id='lin-1',project_name='Billing 2026' WHERE id=?1",
                [linked_project.as_str()],
            )
            .unwrap();
        rename(&local, "sandbox");
        rename(&linked, "payments");
        let name = |id| store.project(id).unwrap().unwrap().name;
        assert_eq!(name(&local_project), "sandbox");
        assert_eq!(name(&linked_project), "payments");
        let renamed = store
            .tracker_actions_for(&ActionOwner::Project(linked_project))
            .unwrap()
            .into_iter()
            .find(|action| action.kind == ActionKind::Field)
            .unwrap();
        assert_eq!(renamed.phase, ActionPhase::Queued);
        assert_eq!(renamed.subject.as_deref(), Some("project_name"));
    }

    #[test]
    fn a_task_done_before_the_tracker_was_connected_arrives_done() {
        let (_dir, store) = store();
        let done = task(&store, false);
        let open = task(&store, false);
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET planning_state='completed',planning_completed=1 WHERE id=?1",
                [done.as_str()],
            )
            .unwrap();
        store
            .enqueue_tracker_creations("/repo", TrackerKind::Linear)
            .unwrap();
        let kinds = |task: &TaskId| -> Vec<ActionKind> {
            store
                .tracker_actions_for(&ActionOwner::Task(task.clone()))
                .unwrap()
                .into_iter()
                .map(|action| action.kind)
                .collect()
        };
        assert_eq!(kinds(&done), [ActionKind::Create, ActionKind::State]);
        assert_eq!(kinds(&open), [ActionKind::Create]);
    }

    #[test]
    fn deleting_a_task_before_its_creation_is_sent_sends_nothing() {
        let (_dir, store) = store();
        let task = task(&store, false);
        store
            .enqueue_tracker_creations("/repo", TrackerKind::Linear)
            .unwrap();
        {
            let conn = store.conn.lock().unwrap();
            super::enqueue_local(
                &conn,
                "tasks",
                task.as_str(),
                &WorkValue::TaskDeletedAt(Some(1_794_800_000)),
            )
            .unwrap();
        }
        let phases: Vec<_> = store
            .tracker_actions_for(&ActionOwner::Task(task))
            .unwrap()
            .into_iter()
            .map(|action| (action.kind, action.phase))
            .collect();
        assert_eq!(phases, [(ActionKind::Create, ActionPhase::Retired)]);
    }

    #[test]
    fn a_resolved_action_leaves_the_queue_and_keeps_its_answer() {
        let (_dir, store) = store();
        let task = task(&store, true);
        let id = enqueue(&store, &task, ActionKind::Field, "task_title").unwrap();
        assert!(store
            .resolve_tracker_action(
                &id,
                &Resolution::Conflicted {
                    remote: json!("theirs")
                }
            )
            .unwrap());
        assert!(!store
            .resolve_tracker_action(
                &id,
                &Resolution::Retire {
                    reason: "late".into()
                }
            )
            .unwrap());
        assert!(store
            .unconfirmed_tracker_actions("/repo")
            .unwrap()
            .is_empty());
        let action = &store.tracker_actions_for(&ActionOwner::Task(task)).unwrap()[0];
        assert_eq!(action.phase, ActionPhase::Conflicted);
        assert_eq!(action.conflict, Some(json!("theirs")));
    }

    #[test]
    fn a_pr_link_is_queued_once_per_state() {
        const URL: &str = "https://github.com/acme/repo/pull/7";
        let (_dir, store) = store();
        let task = task(&store, true);
        let link = |state: &str| {
            store
                .enqueue_pr_link(&task, URL, 7, state, None)
                .unwrap()
                .unwrap()
        };

        // Waiting to be sent, then shown: each refresh is the same action.
        let open = link("Open");
        assert_eq!(link("Open"), open);
        store
            .resolve_tracker_action(
                &open,
                &Resolution::Confirmed {
                    revision: None,
                    remote_id: Some("att-1".into()),
                },
            )
            .unwrap();
        assert_eq!(link("Open"), open);
        assert!(store
            .unconfirmed_tracker_actions("/repo")
            .unwrap()
            .is_empty());

        let merged = link("Merged");
        assert_ne!(merged, open);
        assert_eq!(link("Merged"), merged);
        let actions = store.tracker_actions_for(&ActionOwner::Task(task)).unwrap();
        assert_eq!(actions.len(), 2);
    }
}

#[cfg(test)]
mod upgrade_tests {
    use crate::migration_runner::{apply_before_migration, upgrade_from_migration};
    use rusqlite::Connection;

    fn rows(conn: &Connection, sql: &str) -> Vec<String> {
        let mut query = conn.prepare(sql).unwrap();
        let rows = query.query_map([], |row| row.get(0)).unwrap();
        rows.collect::<Result<_, _>>().unwrap()
    }

    #[test]
    fn tracker_actions_upgrade_moves_every_unconfirmed_receipt_once() {
        let conn = Connection::open_in_memory().unwrap();
        let observers = crate::test_fixture::observers();
        let build = crate::test_fixture::build();
        apply_before_migration(&observers, &build, &conn, "tracker_actions");
        conn.execute_batch(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/migrations/tracker_actions.sql"
        )))
        .unwrap();
        upgrade_from_migration(&observers, &build, &conn, "tracker_actions");
        assert_eq!(
            rows(
                &conn,
                "SELECT coalesce(project_id,task_id)||' '||kind||' '||coalesce(subject,'-')||' '||phase||' '||tracker
                 FROM tracker_actions ORDER BY seq"
            ),
            [
                "project-new create - sent linear",
                "project-new link initiative retired linear",
                "task-new create - sent linear",
                "task-bound field task_title sent linear",
                "task-bound field task_title queued linear",
                "task-bound delete - queued linear",
                "project-bound field project_name queued linear",
                "project-bound order - sent linear",
                "task-bound state - queued linear",
                "task-bound comment comment-proof sent linear",
                "task-bound link relation:0b7c8f0e-1d2a-4e3b-9c4d-5e6f7a8b9c0d queued linear",
            ]
        );
        assert_eq!(
            rows(
                &conn,
                "SELECT value_json FROM tracker_actions WHERE kind IN ('create','state','comment') ORDER BY seq"
            )[1..],
            [
                r#"[{"field":"task_title","value":"New"}]"#,
                r#"{"field":"disposition","value":{"planning_state":"completed","planning_completed":true,"planning_completed_at":"2026-11-01T00:00:00Z"}}"#,
                r#"{"field":"comment_content","value":{"body":"first comment","author":{"kind":"person","name":"Proof"},"created_at":2}}"#,
            ]
        );
        // An older lf on the same store finds nothing left to send.
        assert_eq!(
            rows(
                &conn,
                "SELECT id FROM task_changes WHERE acknowledged=0 AND conflict_json IS NULL
                 UNION ALL SELECT id FROM project_changes WHERE acknowledged=0 AND conflict_json IS NULL
                 UNION ALL SELECT id FROM task_state_deliveries WHERE settled=0
                 UNION ALL SELECT comment_id FROM task_comment_deliveries WHERE acknowledged=0
                 UNION ALL SELECT task_id FROM task_creation_intents
                     WHERE export_json IS NOT NULL AND export_observed_at IS NOT 9007199254740991"
            ),
            Vec::<String>::new()
        );
    }
}
