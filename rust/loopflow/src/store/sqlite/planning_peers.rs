//! Peer receipts and projection commit together on the existing planning tables.
//! Export reads retained mutation identities; it never creates a new edit.

use std::collections::BTreeMap;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::Value;

use crate::engine::planning_exchange::{
    PlanningKind, PlanningMutation, PlanningObject, PlanningSnapshot,
};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

impl SqliteStore {
    pub fn export_peer_planning(&self, repo: &str) -> StoreResult<PlanningSnapshot> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.unchecked_transaction()?;
        let snapshot = export_in(&tx, repo)?;
        tx.commit()?;
        Ok(snapshot)
    }

    /// A fetched revision is acknowledged only after all retained mutations and
    /// their planning projection commit. No Process/Workflow/PR writer is called.
    pub fn import_peer_planning(
        &self,
        repo: &str,
        destination: &str,
        revision: &str,
        incoming: &PlanningSnapshot,
    ) -> StoreResult<PlanningSnapshot> {
        incoming.validate().map_err(invalid)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let saved = export_in(&tx, repo)?;
        let merged = saved.merge(incoming).map_err(invalid)?;
        // Check identities against the whole store, not only this repository.
        for (id, change) in &incoming.changes {
            let retained: Option<(String, String, String, String, i64, bool, String)> = tx.query_row(
                "SELECT kind,object_id,field,value,clock,linear,parents FROM planning_peer_changes WHERE id=?1",
                [id], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?,r.get(6)?)),
            ).optional()?;
            if let Some((kind, object, field, value, clock, linear, parents)) = retained {
                if kind != change.object.kind.as_str()
                    || object != change.object.id
                    || field != change.field
                    || serde_json::from_str::<Value>(&value)? != change.value
                    || clock != change.clock
                    || linear != change.linear
                    || serde_json::from_str::<std::collections::BTreeSet<String>>(&parents)?
                        != change.parents
                {
                    return Err(invalid(format!(
                        "planning change {id} has conflicting contents"
                    )));
                }
            }
        }
        let mut added: Vec<_> = incoming
            .changes
            .iter()
            .filter(|(id, _)| !saved.changes.contains_key(*id))
            .collect();
        added.sort_by_key(|(id, change)| (change.clock, *id));
        for (id, change) in added {
            tx.execute(
                "INSERT OR IGNORE INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
                 VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
                params![id,change.object.kind.as_str(),change.object.id,change.field,
                    change.value.to_string(),change.clock,change.linear,serde_json::to_string(&change.parents)?],
            )?;
        }
        tx.execute("UPDATE planning_peer_context SET importing=1", [])?;
        tx.execute_batch("PRAGMA defer_foreign_keys=ON")?;
        let objects = merged.resolved();
        for (object, fields) in &objects {
            require_complete(object, fields)?;
            match object.kind {
                PlanningKind::Wave => {
                    crate::id::WaveId::parse(&object.id).map_err(invalid)?;
                }
                PlanningKind::Project => {
                    crate::durable::ProjectId::parse(&object.id).map_err(invalid)?;
                }
                PlanningKind::Task => {
                    crate::durable::TaskId::parse(&object.id).map_err(invalid)?;
                }
                PlanningKind::Comment => {}
            }
        }

        // Insert parent identities before their children, then project fields.
        for (object, fields) in &objects {
            let table = table(object.kind);
            let exists: bool = tx.query_row(
                &format!("SELECT EXISTS(SELECT 1 FROM {table} WHERE id=?1)"),
                [&object.id],
                |r| r.get(0),
            )?;
            if exists {
                require_repository(&tx, object, repo)?;
                continue;
            }
            let required = |key: &str| {
                fields
                    .get(key)
                    .and_then(Value::as_str)
                    .ok_or_else(|| invalid(format!("missing {key} for {}", object.id)))
            };
            match object.kind {
                PlanningKind::Wave => {
                    tx.execute(
                        "INSERT INTO waves(id,name,repo,created_at,parent_wave_id) VALUES(?1,?2,?3,unixepoch(),?4)",
                        params![object.id, required("name")?, repo, fields["parent_wave_id"].as_str()],
                    )?;
                }
                PlanningKind::Project => {
                    tx.execute(
                        "INSERT INTO projects(id,wave_id,created_at) VALUES(?1,?2,unixepoch())",
                        params![object.id, required("wave_id")?],
                    )?;
                }
                PlanningKind::Task => {
                    tx.execute("INSERT INTO tasks(id,project_id,issue_identifier,created_at) VALUES(?1,?2,?3,unixepoch())", params![object.id,required("project_id")?,required("issue_identifier")?])?;
                }
                PlanningKind::Comment => {
                    let content = fields
                        .get("content")
                        .ok_or_else(|| invalid("missing comment content"))?;
                    tx.execute("INSERT INTO task_comments(id,task_id,body,author,created_at) VALUES(?1,?2,?3,?4,?5)",
                        params![object.id,required("task_id")?,content["body"].as_str(),content["author"].as_str(),content["created_at"].as_str()])?;
                }
            }
        }
        for (object, fields) in &objects {
            require_complete(object, fields)?;
            project_in(&tx, object, fields)?;
            require_repository(&tx, object, repo)?;
        }
        tx.execute("UPDATE planning_peer_context SET importing=0", [])?;
        tx.execute(
            "INSERT INTO planning_peer_imports(repo,destination,revision) VALUES(?1,?2,?3)
            ON CONFLICT(repo,destination) DO UPDATE SET revision=excluded.revision",
            params![repo, destination, revision],
        )?;
        tx.commit()?;
        Ok(merged)
    }

    pub fn peer_import_revision(
        &self,
        repo: &str,
        destination: &str,
    ) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT revision FROM planning_peer_imports WHERE repo=?1 AND destination=?2",
            params![repo, destination],
            |r| r.get(0),
        )
        .optional()
        .map_err(Into::into)
    }
}

fn export_in(conn: &Connection, repo: &str) -> StoreResult<PlanningSnapshot> {
    let mut query = conn.prepare("WITH objects(kind,id) AS (
        SELECT 'wave',id FROM waves WHERE repo=?1
        UNION ALL SELECT 'project',p.id FROM projects p JOIN waves w ON w.id=p.wave_id WHERE w.repo=?1
        UNION ALL SELECT 'task',t.id FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id WHERE w.repo=?1
        UNION ALL SELECT 'comment',c.id FROM task_comments c JOIN tasks t ON t.id=c.task_id JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id WHERE w.repo=?1)
        SELECT c.id,c.kind,c.object_id,c.field,c.value,c.clock,c.linear,c.parents
        FROM planning_peer_changes c JOIN objects o ON o.kind=c.kind AND o.id=c.object_id ORDER BY c.clock,c.id")?;
    let mut snapshot = PlanningSnapshot::default();
    let mut rows = query.query([repo])?;
    while let Some(row) = rows.next()? {
        let kind: String = row.get(1)?;
        let value: String = row.get(4)?;
        let parents: String = row.get(7)?;
        snapshot.changes.insert(
            row.get(0)?,
            PlanningMutation {
                object: PlanningObject {
                    kind: serde_json::from_value(Value::String(kind))?,
                    id: row.get(2)?,
                },
                field: row.get(3)?,
                value: serde_json::from_str(&value)?,
                clock: row.get(5)?,
                linear: row.get(6)?,
                parents: serde_json::from_str(&parents)?,
            },
        );
    }
    snapshot.validate().map_err(invalid)?;
    let expected: std::collections::BTreeSet<_> =
        snapshot.heads().into_values().flatten().collect();
    let mut query = conn.prepare("SELECT id FROM planning_peer_heads")?;
    let retained = query
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let actual: std::collections::BTreeSet<_> = retained
        .into_iter()
        .filter(|id| snapshot.changes.contains_key(id))
        .collect();
    if expected != actual {
        return Err(invalid(
            "planning causal heads disagree with retained mutations",
        ));
    }
    Ok(snapshot)
}

fn table(kind: PlanningKind) -> &'static str {
    match kind {
        PlanningKind::Wave => "waves",
        PlanningKind::Project => "projects",
        PlanningKind::Task => "tasks",
        PlanningKind::Comment => "task_comments",
    }
}

fn require_complete(object: &PlanningObject, fields: &BTreeMap<String, Value>) -> StoreResult<()> {
    if object
        .kind
        .fields()
        .iter()
        .any(|field| !fields.contains_key(*field))
    {
        return Err(invalid(format!("incomplete planning record {}", object.id)));
    }
    Ok(())
}

fn require_repository(conn: &Connection, object: &PlanningObject, repo: &str) -> StoreResult<()> {
    let sql = match object.kind {
        PlanningKind::Wave => "SELECT repo FROM waves WHERE id=?1",
        PlanningKind::Project => "SELECT w.repo FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
        PlanningKind::Task => "SELECT w.repo FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id WHERE t.id=?1",
        PlanningKind::Comment => "SELECT w.repo FROM task_comments c JOIN tasks t ON t.id=c.task_id JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id WHERE c.id=?1",
    };
    let owner: String = conn.query_row(sql, [&object.id], |r| r.get(0))?;
    if owner != repo {
        return Err(invalid(format!(
            "planning {} belongs to another repository",
            object.id
        )));
    }
    Ok(())
}

fn project_in(
    conn: &Connection,
    object: &PlanningObject,
    fields: &BTreeMap<String, Value>,
) -> StoreResult<()> {
    if object.kind == PlanningKind::Wave {
        let id = crate::id::WaveId::parse(&object.id).map_err(invalid)?;
        let name = fields["name"]
            .as_str()
            .ok_or_else(|| invalid("Wave name must be text"))?;
        let parent = fields["parent_wave_id"]
            .as_str()
            .map(crate::id::WaveId::parse)
            .transpose()
            .map_err(invalid)?;
        let repo: String =
            conn.query_row("SELECT repo FROM waves WHERE id=?1", [&object.id], |r| {
                r.get(0)
            })?;
        super::validate_wave_parent(conn, &id, name, &repo, parent.as_ref())?;
    }
    let mut columns = BTreeMap::new();
    for (field, value) in fields {
        if matches!(field.as_str(), "disposition" | "content") {
            let allowed = if field == "disposition" {
                &[
                    "planning_state",
                    "planning_completed",
                    "planning_completed_at",
                ][..]
            } else {
                &["body", "author", "created_at"][..]
            };
            let group = value
                .as_object()
                .ok_or_else(|| invalid("invalid planning field group"))?;
            if group.len() != allowed.len() || allowed.iter().any(|key| !group.contains_key(*key)) {
                return Err(invalid("invalid planning field group"));
            }
            columns.extend(
                group
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone())),
            );
        } else {
            columns.insert(field.clone(), value.clone());
        }
    }
    let assignments = columns
        .keys()
        .map(|key| format!("{key}=json_extract(?2,'$.{key}')"))
        .collect::<Vec<_>>()
        .join(",");
    let changed = columns
        .keys()
        .map(|key| format!("{key} IS NOT json_extract(?2,'$.{key}')"))
        .collect::<Vec<_>>()
        .join(" OR ");
    let revision = if object.kind == PlanningKind::Task {
        ",planning_revision=planning_revision+1"
    } else {
        ""
    };
    // Names came from the portable allowlist, never arbitrary incoming SQL.
    // Advance the common optimistic revision, but not on repeated delivery.
    conn.execute(
        &format!(
            "UPDATE {} SET {assignments}{revision} WHERE id=?1 AND ({changed})",
            table(object.kind)
        ),
        params![object.id, serde_json::to_string(&columns)?],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::params;
    use serde_json::json;

    use super::SqliteStore;
    use crate::durable::{ProjectId, TaskId};
    use crate::id::{ProcessLfid, TraceId, WaveId};
    use crate::ops::pm::{TaskComment, TaskCommentAuthor};
    use crate::work::wave::Wave;

    fn store() -> (tempfile::TempDir, SqliteStore) {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        (home, store)
    }

    fn seed(store: &SqliteStore) -> TaskId {
        let wave = Wave::new(WaveId::new(), "planning".into(), "/source".into());
        store.create_wave(&wave).unwrap();
        let project = ProjectId::new();
        let task = TaskId::new();
        let conn = store.conn.lock().unwrap();
        conn.execute("INSERT INTO projects(id,wave_id,created_at,project_slug,project_name,project_prompt_context)
            VALUES(?1,?2,1,'chapter','Chapter','')", params![project.as_str(),wave.id()]).unwrap();
        conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,issue_title,issue_description,created_at)
            VALUES(?1,?2,'FIX-1','Original','Brief',1)",params![task.as_str(),project.as_str()]).unwrap();
        task
    }

    #[test]
    fn peer_migration_preserves_the_populated_predecessor() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE schema_migrations(version TEXT PRIMARY KEY,applied_at INTEGER NOT NULL);",
        )
        .unwrap();
        let mut upgrade = None;
        for migration in crate::store::migration_catalog::MIGRATIONS {
            if let Some((before, after)) = migration.sql.split_once("-- draft: planning_peers") {
                conn.execute_batch(before).unwrap();
                upgrade = Some(after.to_string());
                break;
            }
            conn.execute_batch(migration.sql).unwrap();
        }
        if upgrade.is_none() {
            for draft in crate::build_info::migration_draft_manifest() {
                if draft.name == "planning_peers" {
                    upgrade = Some(draft.sql.to_string());
                    break;
                }
                conn.execute_batch(draft.sql).unwrap();
            }
        }
        conn.execute_batch(
            "INSERT INTO waves(id,name,repo,created_at) VALUES('wave','planning','/fixture',1);
            INSERT INTO projects(id,wave_id,created_at) VALUES('project','wave',1);
            INSERT INTO tasks(id,project_id,issue_identifier,issue_title,created_at,worktree,agent)
            VALUES('task','project','FIX-1','Retain identity',1,'/retained','codex');",
        )
        .unwrap();
        conn.execute_batch(&upgrade.expect("peer draft or materialized migration"))
            .unwrap();
        let before = super::export_in(&conn, "/fixture").unwrap();
        assert!(!before.changes.is_empty());
        conn.execute("UPDATE tasks SET issue_title='Changed' WHERE id='task'", [])
            .unwrap();
        let after = super::export_in(&conn, "/fixture").unwrap();
        assert_eq!(after.changes.len(), before.changes.len() + 1);
        let retained: (String, String) = conn
            .query_row(
                "SELECT worktree,agent FROM tasks WHERE id='task'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(retained, ("/retained".into(), "codex".into()));
    }

    #[test]
    fn peer_import_keeps_identity_execution_and_concurrent_local_saves() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        let task = seed(&left);
        let base = left.export_peer_planning("/source").unwrap();
        assert_eq!(base, left.export_peer_planning("/source").unwrap());
        right
            .import_peer_planning("/target", "synthetic", "first", &base)
            .unwrap();
        assert_eq!(right.export_peer_planning("/target").unwrap(), base);
        let driver = ProcessLfid::new();
        {
            let conn = right.conn.lock().unwrap();
            conn.execute(
                "UPDATE tasks SET worktree='/retained/checkout',agent='codex' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
            conn.execute("INSERT INTO processes(lfid,trace_id,started_at,completed_at,outcome) VALUES(?1,?2,1,2,'succeeded')",params![driver,TraceId::new()]).unwrap();
        }
        let definition = crate::engine::workflow::WorkflowDefinition {
            name: "review".into(),
            nodes: vec![crate::engine::workflow::WorkflowNode {
                name: "review".into(),
                skill: "review".into(),
                description: None,
            }],
            edges: Vec::new(),
        };
        right
            .take_up_workflow(&task, &definition, &driver, None)
            .unwrap();
        right
            .set_workflow_node(&task, "review", &driver, None)
            .unwrap();
        let workflow = right.workflow(&task).unwrap();
        {
            let conn = left.conn.lock().unwrap();
            conn.execute("UPDATE tasks SET issue_title='New title',planning_completed=1,planning_state='completed' WHERE id=?1",[task.as_str()]).unwrap();
        }
        {
            let conn = right.conn.lock().unwrap();
            conn.execute(
                "UPDATE tasks SET issue_description='Concurrent brief' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        }
        let comment = TaskComment {
            id: "comment-1".into(),
            body: "Retain the direction".into(),
            author: TaskCommentAuthor::Person {
                name: Some("Maya".into()),
            },
            created_at: Some("2026-10-08T12:00:00Z".into()),
        };
        left.append_task_comment(&task, &comment).unwrap();
        let incoming = left.export_peer_planning("/source").unwrap();
        let merged = right
            .import_peer_planning("/target", "synthetic", "second", &incoming)
            .unwrap();
        let revisions = right.revisions().unwrap();
        assert_eq!(
            right
                .import_peer_planning("/target", "synthetic", "second", &incoming)
                .unwrap(),
            merged
        );
        assert_eq!(right.revisions().unwrap(), revisions);
        assert_eq!(
            right
                .peer_import_revision("/target", "synthetic")
                .unwrap()
                .as_deref(),
            Some("second")
        );
        let record = right.planning_task(&task).unwrap().record.unwrap();
        assert_eq!(record.item.name, "New title");
        assert_eq!(record.item.description, "Concurrent brief");
        assert!(record.item.completed);
        assert_eq!(right.task_comments(&task).unwrap().comments, vec![comment]);
        assert_eq!(right.workflow(&task).unwrap(), workflow);
        let conn = right.conn.lock().unwrap();
        let placement: (String, String) = conn
            .query_row(
                "SELECT worktree,agent FROM tasks WHERE id=?1",
                [task.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(placement, ("/retained/checkout".into(), "codex".into()));
        drop(conn);
        left.import_peer_planning("/source", "synthetic", "third", &merged)
            .unwrap();
        assert_eq!(
            left.export_peer_planning("/source").unwrap(),
            right.export_peer_planning("/target").unwrap()
        );
    }

    #[test]
    fn failed_projection_rolls_back_import_receipts_and_allows_exact_retry() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        let task = seed(&left);
        let base = left.export_peer_planning("/source").unwrap();
        right.conn.lock().unwrap().execute_batch("CREATE TEMP TRIGGER interrupt_peer BEFORE INSERT ON tasks BEGIN SELECT RAISE(ABORT,'simulated interruption'); END;").unwrap();
        assert!(right
            .import_peer_planning("/target", "synthetic", "candidate", &base)
            .is_err());
        assert!(right
            .export_peer_planning("/target")
            .unwrap()
            .changes
            .is_empty());
        assert!(right
            .peer_import_revision("/target", "synthetic")
            .unwrap()
            .is_none());
        right
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER interrupt_peer;")
            .unwrap();
        right
            .import_peer_planning("/target", "synthetic", "candidate", &base)
            .unwrap();
        assert!(right.planning_task(&task).unwrap().record.is_some());
        let mut conflicting = base.clone();
        conflicting
            .changes
            .values_mut()
            .find(|c| c.field == "issue_title")
            .unwrap()
            .value = json!("Reused identity");
        assert!(right
            .import_peer_planning("/target", "synthetic", "invalid", &conflicting)
            .is_err());
        assert_eq!(
            right
                .peer_import_revision("/target", "synthetic")
                .unwrap()
                .as_deref(),
            Some("candidate")
        );
        assert_eq!(right.export_peer_planning("/target").unwrap(), base);
    }

    #[test]
    fn common_writers_capture_causal_reopening_and_explicit_deletion_without_echo() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        let task = seed(&left);
        left.conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET planning_completed=1,planning_state='completed' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        let completed = left.export_peer_planning("/source").unwrap();
        right
            .import_peer_planning("/target", "synthetic", "done", &completed)
            .unwrap();
        right
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET planning_completed=0,planning_state='unstarted' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        right
            .import_peer_planning("/target", "synthetic", "delayed", &completed)
            .unwrap();
        assert!(
            !right
                .planning_task(&task)
                .unwrap()
                .record
                .unwrap()
                .item
                .completed
        );
        left.conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET planning_deleted_at=42 WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        right
            .import_peer_planning(
                "/target",
                "synthetic",
                "deleted",
                &left.export_peer_planning("/source").unwrap(),
            )
            .unwrap();
        assert_eq!(
            right.planning_task(&task).unwrap().state,
            crate::store::PlanningState::Removed
        );
        let retained = right.export_peer_planning("/target").unwrap();
        right
            .import_peer_planning("/target", "synthetic", "omission", &Default::default())
            .unwrap();
        assert_eq!(right.export_peer_planning("/target").unwrap(), retained);
        assert!(right.planning_task(&task).unwrap().record.is_some());
    }
}
