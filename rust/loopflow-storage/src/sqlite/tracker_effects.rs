//! The outbound tracker queue: one row per change a bridge carries, in the
//! store's words. Writers enqueue inside their own transaction; the drain reads
//! unsettled rows in `seq` order and records what the provider answered.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde_json::Value;

use super::SqliteStore;
use crate::planning_sync::PlanningBaseline;
use crate::provider::Provider;
use crate::rows::now_unix;
use crate::work::edit::WorkValue;
use crate::{StoreError, StoreResult};
use loopflow_model::durable::{ProjectId, TaskId};
use loopflow_model::id::WaveId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EffectKind {
    Create,
    Field,
    Order,
    State,
    Comment,
    Delete,
    Link,
}

impl EffectKind {
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

    fn parse(value: &str) -> StoreResult<Self> {
        Ok(match value {
            "create" => Self::Create,
            "field" => Self::Field,
            "order" => Self::Order,
            "state" => Self::State,
            "comment" => Self::Comment,
            "delete" => Self::Delete,
            "link" => Self::Link,
            other => {
                return Err(StoreError::InvalidData(format!(
                    "unknown tracker effect kind {other}"
                )))
            }
        })
    }

    /// Only the newest unsent value of these is worth sending.
    fn coalesces(self) -> bool {
        matches!(self, Self::Field | Self::Order | Self::State)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EffectPhase {
    Pending,
    Attempted,
    Acknowledged,
    Retired,
}

impl EffectPhase {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Attempted => "attempted",
            Self::Acknowledged => "acknowledged",
            Self::Retired => "retired",
        }
    }

    fn parse(value: &str) -> StoreResult<Self> {
        Ok(match value {
            "pending" => Self::Pending,
            "attempted" => Self::Attempted,
            "acknowledged" => Self::Acknowledged,
            "retired" => Self::Retired,
            other => {
                return Err(StoreError::InvalidData(format!(
                    "unknown tracker effect phase {other}"
                )))
            }
        })
    }
}

/// The tracker object a request addresses.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EffectOwner {
    Wave(WaveId),
    Project(ProjectId),
    Task(TaskId),
}

impl EffectOwner {
    fn column(&self) -> &'static str {
        match self {
            Self::Wave(_) => "wave_id",
            Self::Project(_) => "project_id",
            Self::Task(_) => "task_id",
        }
    }

    fn id(&self) -> &str {
        match self {
            Self::Wave(id) => id.as_str(),
            Self::Project(id) => id.as_str(),
            Self::Task(id) => id.as_str(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TrackerEffect {
    pub seq: i64,
    pub id: String,
    pub owner: EffectOwner,
    pub subject: Option<String>,
    pub kind: EffectKind,
    pub value: Value,
    pub base: Option<PlanningBaseline>,
    pub conflict: Option<Value>,
    pub phase: EffectPhase,
    pub provider: Provider,
    pub remote_id: Option<String>,
    pub acknowledged_revision: Option<String>,
    pub error: Option<String>,
    pub attempted_at: Option<i64>,
    pub looked_at: Option<i64>,
}

/// What a pass decided for one effect. `Send` and `Hold` leave it unsettled.
#[derive(Debug, Clone, PartialEq)]
pub enum Settlement {
    Acknowledged {
        revision: Option<String>,
        remote_id: Option<String>,
    },
    /// The provider's value won; the local value it replaced is kept.
    Adopt {
        losing: Value,
    },
    Retire {
        reason: String,
    },
}

const COLUMNS: &str = "e.seq,e.id,e.wave_id,e.project_id,e.task_id,e.subject,e.kind,e.value_json,
    e.base_json,e.conflict_json,e.phase,e.provider,e.remote_id,e.acknowledged_revision,e.error,
    e.attempted_at,e.looked_at";

// Every effect owner reaches its repository through one of these joins.
const IN_REPO: &str = "(e.task_id IN (SELECT t.id FROM tasks t JOIN repository_plans r ON r.id=t.repository_id WHERE r.repo=?1)
    OR e.project_id IN (SELECT p.id FROM projects p JOIN waves w ON w.id=p.wave_id WHERE w.repo=?1)
    OR e.wave_id IN (SELECT id FROM waves WHERE repo=?1))";

fn read_effect(row: &Row<'_>) -> StoreResult<TrackerEffect> {
    let wave: Option<String> = row.get(2)?;
    let project: Option<String> = row.get(3)?;
    let task: Option<String> = row.get(4)?;
    let owner = match (wave, project, task) {
        (Some(id), None, None) => EffectOwner::Wave(
            WaveId::parse(&id)
                .map_err(|_| StoreError::InvalidData(format!("invalid Wave id {id}")))?,
        ),
        (None, Some(id), None) => EffectOwner::Project(ProjectId::from_raw(id)),
        (None, None, Some(id)) => EffectOwner::Task(TaskId::from_raw(id)),
        _ => {
            return Err(StoreError::InvalidData(
                "tracker effect has no single owner".into(),
            ))
        }
    };
    let base: Option<String> = row.get(8)?;
    let conflict: Option<String> = row.get(9)?;
    let provider: String = row.get(11)?;
    Ok(TrackerEffect {
        seq: row.get(0)?,
        id: row.get(1)?,
        owner,
        subject: row.get(5)?,
        kind: EffectKind::parse(&row.get::<_, String>(6)?)?,
        value: serde_json::from_str(&row.get::<_, String>(7)?)?,
        base: base.map(|base| serde_json::from_str(&base)).transpose()?,
        conflict: conflict
            .map(|conflict| serde_json::from_str(&conflict))
            .transpose()?,
        phase: EffectPhase::parse(&row.get::<_, String>(10)?)?,
        provider: provider
            .parse()
            .map_err(|_| StoreError::InvalidData(format!("unknown tracker {provider}")))?,
        remote_id: row.get(12)?,
        acknowledged_revision: row.get(13)?,
        error: row.get(14)?,
        attempted_at: row.get(15)?,
        looked_at: row.get(16)?,
    })
}

/// The bridge an owner's effects go to: its binding's, else its pending
/// creation's. An owner with neither enqueues nothing.
pub(crate) fn owner_tracker(
    conn: &Connection,
    owner: &EffectOwner,
) -> StoreResult<Option<Provider>> {
    let bound: Option<Option<String>> = match owner {
        EffectOwner::Task(id) => conn
            .query_row(
                "SELECT (SELECT o.provider FROM pm_items o JOIN repository_plans r ON r.repo=o.repo
                    WHERE r.id=t.repository_id AND o.id=t.external_issue_id LIMIT 1)
                 FROM tasks t WHERE t.id=?1 AND t.external_issue_id IS NOT NULL",
                [id.as_str()],
                |row| row.get(0),
            )
            .optional()?,
        EffectOwner::Project(id) => conn
            .query_row(
                "SELECT (SELECT o.provider FROM pm_projects o JOIN waves w ON w.repo=o.repo
                    WHERE w.id=p.wave_id AND o.id=p.external_project_id LIMIT 1)
                 FROM projects p WHERE p.id=?1 AND p.external_project_id IS NOT NULL",
                [id.as_str()],
                |row| row.get(0),
            )
            .optional()?,
        EffectOwner::Wave(_) => None,
    };
    let provider = match bound {
        // A binding acquired before observations recorded their bridge.
        Some(provider) => Some(provider.unwrap_or_else(|| "linear".into())),
        None => conn
            .query_row(
                &format!(
                    "SELECT provider FROM tracker_effects WHERE {}=?1 AND kind='create'
                     AND phase!='retired' ORDER BY seq DESC LIMIT 1",
                    owner.column()
                ),
                [owner.id()],
                |row| row.get(0),
            )
            .optional()?,
    };
    provider
        .map(|provider: String| {
            provider
                .parse()
                .map_err(|_| StoreError::InvalidData(format!("unknown tracker {provider}")))
        })
        .transpose()
}

/// Add one effect for a bound or creating owner, retiring older unsent values
/// of the same subject. Returns the effect id, or `None` when no bridge carries
/// this owner.
pub(crate) fn enqueue(
    conn: &Connection,
    owner: &EffectOwner,
    kind: EffectKind,
    subject: Option<&str>,
    value: &Value,
    base: Option<&PlanningBaseline>,
) -> StoreResult<Option<String>> {
    let Some(provider) = owner_tracker(conn, owner)? else {
        return Ok(None);
    };
    insert(conn, owner, kind, subject, value, base, provider).map(Some)
}

fn insert(
    conn: &Connection,
    owner: &EffectOwner,
    kind: EffectKind,
    subject: Option<&str>,
    value: &Value,
    base: Option<&PlanningBaseline>,
    provider: Provider,
) -> StoreResult<String> {
    let column = owner.column();
    if kind.coalesces() {
        conn.execute(
            &format!(
                "UPDATE tracker_effects SET phase='retired',error='superseded by a later save'
                 WHERE {column}=?1 AND kind=?2 AND subject IS ?3 AND phase='pending'"
            ),
            params![owner.id(), kind.as_str(), subject],
        )?;
    }
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute(
        &format!(
            "INSERT INTO tracker_effects(id,{column},subject,kind,value_json,base_json,phase,provider)
             VALUES(?1,?2,?3,?4,?5,?6,'pending',?7)"
        ),
        params![
            id,
            owner.id(),
            subject,
            kind.as_str(),
            value.to_string(),
            base.map(serde_json::to_string).transpose()?,
            provider.as_str()
        ],
    )?;
    Ok(id)
}

fn creation_baseline(conn: &Connection, owner: &EffectOwner) -> StoreResult<Vec<WorkValue>> {
    Ok(match owner {
        EffectOwner::Task(id) => {
            let task = super::children::task_on(conn, id)?.ok_or(StoreError::NotFound)?;
            vec![
                WorkValue::TaskTitle(Some(task.title)),
                WorkValue::TaskDescription(Some(task.description)),
                WorkValue::TaskAssignee(task.assignee),
                WorkValue::TaskDueDate(task.due_date),
                WorkValue::TaskProject(task.project_id.as_ref().map(ToString::to_string)),
            ]
        }
        EffectOwner::Project(id) => {
            let project = super::children::project_in(conn, id)?;
            vec![
                WorkValue::ProjectName(Some(project.name)),
                WorkValue::ProjectSummary(project.summary),
                WorkValue::ProjectDescription(Some(project.description)),
                WorkValue::ProjectStatus(project.status),
                WorkValue::Krs(project.krs),
                WorkValue::MetricTargets(project.metric_targets),
            ]
        }
        EffectOwner::Wave(_) => {
            return Err(StoreError::InvalidData(
                "a Wave is connected, never created, by a bridge".into(),
            ))
        }
    })
}

impl SqliteStore {
    /// Give every unbound local Project and Task in `repo` a `create` for
    /// `provider`, including those made before the tracker was connected, then
    /// queue their comments behind it. Sends nothing.
    pub fn enqueue_tracker_creations(&self, repo: &str, provider: Provider) -> StoreResult<usize> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let mut owners = Vec::new();
        {
            let mut query = tx.prepare(
                "SELECT 'project',p.id FROM projects p JOIN waves w ON w.id=p.wave_id
                 WHERE w.repo=?1 AND p.external_project_id IS NULL
                 AND EXISTS(SELECT 1 FROM project_transitions c WHERE c.successor_id=p.id AND c.local_plan_json IS NOT NULL)
                 AND NOT EXISTS(SELECT 1 FROM tracker_effects e WHERE e.project_id=p.id AND e.kind='create' AND e.phase!='retired')
                 UNION ALL
                 SELECT 'task',t.id FROM tasks t JOIN repository_plans r ON r.id=t.repository_id
                 WHERE r.repo=?1 AND t.external_issue_id IS NULL AND t.planning_deleted_at IS NULL
                 AND EXISTS(SELECT 1 FROM task_creation_intents c WHERE c.task_id=t.id)
                 AND NOT EXISTS(SELECT 1 FROM tracker_effects e WHERE e.task_id=t.id AND e.kind='create' AND e.phase!='retired')",
            )?;
            let rows = query.query_map([repo], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            for row in rows {
                let (kind, id) = row?;
                owners.push(if kind == "project" {
                    EffectOwner::Project(ProjectId::from_raw(id))
                } else {
                    EffectOwner::Task(TaskId::from_raw(id))
                });
            }
        }
        for owner in &owners {
            let baseline = serde_json::to_value(creation_baseline(&tx, owner)?)?;
            insert(
                &tx,
                owner,
                EffectKind::Create,
                None,
                &baseline,
                None,
                provider,
            )?;
            if let EffectOwner::Task(task) = owner {
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
                        EffectKind::Comment,
                        Some(&comment),
                        &serde_json::to_value(content)?,
                        None,
                        provider,
                    )?;
                }
            }
        }
        tx.commit()?;
        Ok(owners.len())
    }

    /// Unsettled effects for `repo` in send order.
    pub fn unsettled_tracker_effects(&self, repo: &str) -> StoreResult<Vec<TrackerEffect>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM tracker_effects e
             WHERE e.phase IN ('pending','attempted') AND {IN_REPO} ORDER BY e.seq"
        ))?;
        let effects = query.query_and_then([repo], read_effect)?;
        effects.collect()
    }

    /// Every effect for one owner, oldest first, settled ones included.
    pub fn tracker_effects_for(&self, owner: &EffectOwner) -> StoreResult<Vec<TrackerEffect>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM tracker_effects e WHERE e.{}=?1 ORDER BY e.seq",
            owner.column()
        ))?;
        let effects = query.query_and_then([owner.id()], read_effect)?;
        effects.collect()
    }

    /// Mark an unsettled effect sent before its request leaves. Returns false
    /// when it settled meanwhile.
    pub fn attempt_tracker_effect(&self, id: &str) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE tracker_effects SET phase='attempted',attempted_at=?2,error=NULL
             WHERE id=?1 AND phase IN ('pending','attempted')",
            params![id, now_unix()],
        )? == 1)
    }

    /// A request the provider rejected before acting leaves the effect unsent.
    pub fn reject_tracker_effect(&self, id: &str, error: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE tracker_effects SET phase='pending',error=?2 WHERE id=?1 AND phase='attempted'",
            params![id, error],
        )?;
        Ok(())
    }

    /// Record why the last request did not settle, keeping the phase.
    pub fn tracker_effect_error(&self, id: &str, error: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE tracker_effects SET error=?2 WHERE id=?1
             AND phase IN ('pending','attempted') AND error IS NOT ?2",
            params![id, error],
        )?;
        Ok(())
    }

    /// Record a look in the provider for an attempted effect's lost reply.
    pub fn look_tracker_effect(&self, id: &str, at: i64) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE tracker_effects SET looked_at=?2 WHERE id=?1 AND phase='attempted'",
            params![id, at],
        )?;
        Ok(())
    }

    pub fn settle_tracker_effect(&self, id: &str, settlement: &Settlement) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = match settlement {
            Settlement::Acknowledged {
                revision,
                remote_id,
            } => conn.execute(
                "UPDATE tracker_effects SET phase='acknowledged',acknowledged_revision=?2,
                 remote_id=COALESCE(?3,remote_id),error=NULL
                 WHERE id=?1 AND phase IN ('pending','attempted')",
                params![id, revision, remote_id],
            )?,
            Settlement::Adopt { losing } => conn.execute(
                "UPDATE tracker_effects SET phase='acknowledged',conflict_json=?2,error=NULL
                 WHERE id=?1 AND phase IN ('pending','attempted')",
                params![id, losing.to_string()],
            )?,
            Settlement::Retire { reason } => conn.execute(
                "UPDATE tracker_effects SET phase='retired',error=?2
                 WHERE id=?1 AND phase IN ('pending','attempted')",
                params![id, reason],
            )?,
        };
        Ok(changed == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::{EffectKind, EffectOwner, EffectPhase, Settlement};
    use crate::provider::Provider;
    use crate::sqlite::SqliteStore;
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
        kind: EffectKind,
        subject: &str,
    ) -> Option<String> {
        let conn = store.conn.lock().unwrap();
        super::enqueue(
            &conn,
            &EffectOwner::Task(task.clone()),
            kind,
            Some(subject),
            &json!({"field":"task_title","value":subject}),
            None,
        )
        .unwrap()
    }

    #[test]
    fn an_owner_no_bridge_carries_enqueues_nothing() {
        let (_dir, store) = store();
        let task = task(&store, false);
        assert_eq!(
            enqueue(&store, &task, EffectKind::Field, "task_title"),
            None
        );
        assert!(store.unsettled_tracker_effects("/repo").unwrap().is_empty());
    }

    #[test]
    fn a_later_save_retires_the_unsent_value_and_keeps_the_attempted_one() {
        let (_dir, store) = store();
        let task = task(&store, true);
        let sent = enqueue(&store, &task, EffectKind::Field, "task_title").unwrap();
        assert!(store.attempt_tracker_effect(&sent).unwrap());
        let unsent = enqueue(&store, &task, EffectKind::Field, "task_title").unwrap();
        let latest = enqueue(&store, &task, EffectKind::Field, "task_title").unwrap();
        let comment = enqueue(&store, &task, EffectKind::Comment, "c1").unwrap();
        let again = enqueue(&store, &task, EffectKind::Comment, "c1").unwrap();
        let phases: Vec<_> = store
            .tracker_effects_for(&EffectOwner::Task(task))
            .unwrap()
            .into_iter()
            .map(|effect| (effect.id, effect.phase))
            .collect();
        assert_eq!(
            phases,
            vec![
                (sent, EffectPhase::Attempted),
                (unsent, EffectPhase::Retired),
                (latest, EffectPhase::Pending),
                (comment, EffectPhase::Pending),
                (again, EffectPhase::Pending),
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
                .enqueue_tracker_creations("/repo", Provider::Asana)
                .unwrap(),
            1
        );
        assert_eq!(
            store
                .enqueue_tracker_creations("/repo", Provider::Asana)
                .unwrap(),
            0,
            "a second pass queues nothing new"
        );
        let effects = store.unsettled_tracker_effects("/repo").unwrap();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].owner, EffectOwner::Task(earlier.clone()));
        assert_eq!(effects[0].kind, EffectKind::Create);
        assert_eq!(effects[0].provider, Provider::Asana);
        assert_eq!(effects[0].value[0]["field"], "task_title");
        // The creating owner now carries later edits to the same bridge.
        assert!(enqueue(&store, &earlier, EffectKind::Field, "task_title").is_some());
        assert!(store
            .tracker_effects_for(&EffectOwner::Task(bound))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn a_settled_effect_leaves_the_queue_and_keeps_its_answer() {
        let (_dir, store) = store();
        let task = task(&store, true);
        let id = enqueue(&store, &task, EffectKind::Field, "task_title").unwrap();
        assert!(store
            .settle_tracker_effect(
                &id,
                &Settlement::Adopt {
                    losing: json!("mine")
                }
            )
            .unwrap());
        assert!(!store
            .settle_tracker_effect(
                &id,
                &Settlement::Retire {
                    reason: "late".into()
                }
            )
            .unwrap());
        assert!(store.unsettled_tracker_effects("/repo").unwrap().is_empty());
        let effect = &store.tracker_effects_for(&EffectOwner::Task(task)).unwrap()[0];
        assert_eq!(effect.phase, EffectPhase::Acknowledged);
        assert_eq!(effect.conflict, Some(json!("mine")));
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
    fn tracker_effects_upgrade_moves_every_unsettled_receipt_once() {
        let conn = Connection::open_in_memory().unwrap();
        let observers = crate::test_fixture::observers();
        let build = crate::test_fixture::build();
        apply_before_migration(&observers, &build, &conn, "tracker_effects");
        conn.execute_batch(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/migrations/tracker_effects.sql"
        )))
        .unwrap();
        upgrade_from_migration(&observers, &build, &conn, "tracker_effects");
        assert_eq!(
            rows(
                &conn,
                "SELECT coalesce(project_id,task_id)||' '||kind||' '||coalesce(subject,'-')||' '||phase||' '||provider
                 FROM tracker_effects ORDER BY seq"
            ),
            [
                "project-new create - attempted linear",
                "project-new link initiative pending linear",
                "task-new create - attempted linear",
                "task-bound field task_title attempted linear",
                "task-bound field task_title pending linear",
                "task-bound delete - pending linear",
                "project-bound field project_name pending linear",
                "project-bound order - attempted linear",
                "task-bound state - pending linear",
                "task-bound comment comment-proof attempted linear",
            ]
        );
        assert_eq!(
            rows(
                &conn,
                "SELECT value_json FROM tracker_effects WHERE kind IN ('create','state','comment') ORDER BY seq"
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
