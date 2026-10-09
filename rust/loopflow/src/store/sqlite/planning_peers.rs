//! Peer receipts and projection commit together on the existing planning tables.
//! Export reads retained mutation identities; it never creates a new edit.

use std::collections::{BTreeMap, BTreeSet};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::Value;

use crate::engine::planning_exchange::{
    PlanningKind, PlanningMutation, PlanningObject, PlanningSnapshot,
};
use crate::store::{PeerProjectionConflict, StoreError, StoreResult};

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

    /// A revision acknowledges the retained journal, allowed projections and
    /// explicit conflicts, not complete projection. No execution writer is called.
    pub fn import_peer_planning(
        &self,
        repo: &str,
        destination: &str,
        revision: &str,
        incoming: &PlanningSnapshot,
    ) -> StoreResult<PlanningSnapshot> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let saved = export_in(&tx, repo)?;
        let merged = saved.merge(incoming).map_err(invalid)?;
        retain_mutations(&tx, &saved, incoming)?;
        tx.execute("UPDATE planning_peer_context SET importing=1", [])?;
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
            // Pending projections retain ownership even without a live row.
            require_repository(&tx, object, repo)?;
        }
        let mut conflicts = BTreeSet::new();
        let mut pending: Vec<_> = objects.iter().collect();
        loop {
            let count = pending.len();
            let mut retry = Vec::new();
            for (object, fields) in pending {
                tx.execute_batch("SAVEPOINT peer_projection")?;
                match insert_and_project(&tx, object, fields, repo) {
                    Ok(()) => {
                        tx.execute_batch("RELEASE peer_projection")?;
                        conflicts.retain(|(candidate, _)| candidate != object);
                    }
                    Err(error) if projection_conflict(&error) => {
                        tx.execute_batch("ROLLBACK TO peer_projection; RELEASE peer_projection")?;
                        conflicts.insert((object.clone(), error.to_string()));
                        retry.push((object, fields));
                    }
                    Err(error) => return Err(error),
                }
            }
            // Parent ordering may need another pass, but a conflict never blocks
            // an independent object or causes an unbounded retry.
            if retry.len() == count || retry.is_empty() {
                break;
            }
            pending = retry;
        }
        // Wave selection points back at Projects. Set it only after identity and
        // ownership projection, rather than deferring all foreign-key checks.
        for (object, fields) in &objects {
            if object.kind != PlanningKind::Wave || !exists(&tx, object)? {
                continue;
            }
            match project_fields(
                &tx,
                object,
                std::iter::once(("current_project_id", &fields["current_project_id"])),
            ) {
                Ok(()) => {}
                Err(error) if projection_conflict(&error) => {
                    conflicts.insert((object.clone(), error.to_string()));
                }
                Err(error) => return Err(error),
            }
        }
        reconcile_conflicts(&tx, repo, &conflicts)?;
        tx.execute("UPDATE planning_peer_context SET importing=0", [])?;
        tx.execute(
            "INSERT INTO planning_peer_imports(repo,destination,revision) VALUES(?1,?2,?3)
            ON CONFLICT(repo,destination) DO UPDATE SET revision=excluded.revision",
            params![repo, destination, revision],
        )?;
        tx.commit()?;
        Ok(merged)
    }

    pub fn peer_projection_conflicts(
        &self,
        repo: &str,
    ) -> StoreResult<Vec<PeerProjectionConflict>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        projection_conflicts_in(&conn, repo)
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

fn retain_mutations(
    conn: &Connection,
    saved: &PlanningSnapshot,
    incoming: &PlanningSnapshot,
) -> StoreResult<()> {
    let mut added: Vec<_> = incoming
        .changes
        .iter()
        .filter(|(id, _)| !saved.changes.contains_key(*id))
        .collect();
    added.sort_by_key(|(id, change)| (change.clock, *id));
    // Merge checked this repository's identities. New receipts must also
    // agree with any identity retained elsewhere in the same store.
    let mut query = conn.prepare_cached(
        "SELECT kind,object_id,field,value,clock,linear,parents FROM planning_peer_changes WHERE id=?1",
    )?;
    for (id, change) in added {
        let mut rows = query.query([id])?;
        if let Some(row) = rows.next()? {
            if read_mutation(row)? != *change {
                return Err(invalid(format!(
                    "planning change {id} has conflicting contents"
                )));
            }
            continue;
        }
        conn.execute(
            "INSERT INTO planning_peer_changes(id,kind,object_id,field,value,clock,linear,parents)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",
            params![
                id,
                change.object.kind.as_str(),
                change.object.id,
                change.field,
                change.value.to_string(),
                change.clock,
                change.linear,
                serde_json::to_string(&change.parents)?
            ],
        )?;
    }
    Ok(())
}

fn export_in(conn: &Connection, repo: &str) -> StoreResult<PlanningSnapshot> {
    let mut query = conn.prepare("WITH objects(kind,id) AS (
        SELECT 'wave',id FROM waves WHERE repo=?1
        UNION ALL SELECT 'project',p.id FROM projects p JOIN waves w ON w.id=p.wave_id WHERE w.repo=?1
        UNION ALL SELECT 'task',t.id FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id WHERE w.repo=?1
        UNION ALL SELECT 'comment',c.id FROM task_comments c JOIN tasks t ON t.id=c.task_id JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id WHERE w.repo=?1
        UNION SELECT kind,object_id FROM planning_peer_conflicts WHERE repo=?1 AND active=1)
        SELECT c.kind,c.object_id,c.field,c.value,c.clock,c.linear,c.parents,c.id
        FROM planning_peer_changes c JOIN objects o ON o.kind=c.kind AND o.id=c.object_id ORDER BY c.clock,c.id")?;
    let mut snapshot = PlanningSnapshot::default();
    let mut rows = query.query([repo])?;
    while let Some(row) = rows.next()? {
        snapshot.changes.insert(row.get(7)?, read_mutation(row)?);
    }
    snapshot.validate().map_err(invalid)?;
    let expected: BTreeSet<_> = snapshot.heads().into_values().flatten().collect();
    let mut query = conn.prepare("SELECT id FROM planning_peer_heads")?;
    let retained = query
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    let actual: BTreeSet<_> = retained
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

/// SELECT kind,object_id,field,value,clock,linear,parents
fn read_mutation(row: &rusqlite::Row<'_>) -> StoreResult<PlanningMutation> {
    Ok(PlanningMutation {
        object: PlanningObject {
            kind: serde_json::from_value(Value::String(row.get(0)?))?,
            id: row.get(1)?,
        },
        field: row.get(2)?,
        value: serde_json::from_str(&row.get::<_, String>(3)?)?,
        clock: row.get(4)?,
        linear: row.get(5)?,
        parents: serde_json::from_str(&row.get::<_, String>(6)?)?,
    })
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
    let owner: Option<String> = conn.query_row(sql, [&object.id], |r| r.get(0)).optional()?;
    let retained: Option<String> = conn
        .query_row(
            "SELECT repo FROM planning_peer_conflicts WHERE kind=?1 AND object_id=?2 LIMIT 1",
            params![object.kind.as_str(), object.id],
            |r| r.get(0),
        )
        .optional()?;
    if owner.as_deref().is_some_and(|owner| owner != repo)
        || retained.as_deref().is_some_and(|owner| owner != repo)
    {
        return Err(invalid(format!(
            "planning {} belongs to another repository",
            object.id
        )));
    }
    Ok(())
}

fn exists(conn: &Connection, object: &PlanningObject) -> StoreResult<bool> {
    Ok(conn.query_row(
        &format!(
            "SELECT EXISTS(SELECT 1 FROM {} WHERE id=?1)",
            table(object.kind)
        ),
        [&object.id],
        |r| r.get(0),
    )?)
}

fn insert_and_project(
    conn: &Connection,
    object: &PlanningObject,
    fields: &BTreeMap<String, Value>,
    repo: &str,
) -> StoreResult<()> {
    if !exists(conn, object)? {
        let required = |key: &str| {
            fields
                .get(key)
                .and_then(Value::as_str)
                .ok_or_else(|| invalid(format!("missing {key} for {}", object.id)))
        };
        match object.kind {
            PlanningKind::Wave => {
                conn.execute(
                    "INSERT INTO waves(id,name,repo,created_at,parent_wave_id) VALUES(?1,?2,?3,unixepoch(),?4)",
                    params![object.id, required("name")?, repo, fields["parent_wave_id"].as_str()],
                )?;
            }
            PlanningKind::Project => {
                conn.execute(
                    "INSERT INTO projects(id,wave_id,created_at) VALUES(?1,?2,unixepoch())",
                    params![object.id, required("wave_id")?],
                )?;
            }
            PlanningKind::Task => {
                conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,created_at) VALUES(?1,?2,?3,unixepoch())",
                    params![object.id,required("project_id")?,required("issue_identifier")?])?;
            }
            PlanningKind::Comment => {
                let content = &fields["content"];
                conn.execute("INSERT INTO task_comments(id,task_id,body,author,created_at) VALUES(?1,?2,?3,?4,?5)",
                    params![object.id,required("task_id")?,content["body"].as_str(),content["author"].as_str(),content["created_at"].as_str()])?;
            }
        }
    }
    if object.kind == PlanningKind::Wave {
        validate_wave(conn, object, fields, repo)?;
    }
    project_fields(
        conn,
        object,
        fields
            .iter()
            .filter(|(field, _)| {
                object.kind != PlanningKind::Wave || *field != "current_project_id"
            })
            .map(|(field, value)| (field.as_str(), value)),
    )?;
    require_repository(conn, object, repo)
}

fn projection_conflict(error: &StoreError) -> bool {
    match error {
        StoreError::Sqlite(rusqlite::Error::SqliteFailure(code, message)) => {
            matches!(
                code.extended_code,
                rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
                    | rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY
            ) || (code.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_TRIGGER
                && matches!(
                    message.as_deref(),
                    Some(
                        "Project would change AgentSession ancestry"
                            | "Task would change AgentSession ancestry"
                            | "selected Project cannot change Wave ownership"
                            | "selected Project belongs to another Wave"
                    )
                ))
        }
        StoreError::InvalidData(reason) => matches!(
            reason.as_str(),
            "Wave parent would create a cycle" | "Wave parent is unavailable"
        ),
        _ => false,
    }
}

fn projection_conflicts_in(
    conn: &Connection,
    repo: &str,
) -> StoreResult<Vec<PeerProjectionConflict>> {
    let mut query = conn.prepare(
        "SELECT kind,object_id,reason FROM planning_peer_conflicts
         WHERE repo=?1 AND active=1 ORDER BY kind,object_id,reason",
    )?;
    let mut rows = query.query([repo])?;
    let mut conflicts = Vec::new();
    while let Some(row) = rows.next()? {
        conflicts.push(PeerProjectionConflict {
            object: PlanningObject {
                kind: serde_json::from_value(Value::String(row.get(0)?))?,
                id: row.get(1)?,
            },
            reason: row.get(2)?,
        });
    }
    Ok(conflicts)
}

fn reconcile_conflicts(
    conn: &Connection,
    repo: &str,
    conflicts: &BTreeSet<(PlanningObject, String)>,
) -> StoreResult<()> {
    for PeerProjectionConflict { object, reason } in projection_conflicts_in(conn, repo)? {
        if !conflicts.contains(&(object.clone(), reason.clone())) {
            conn.execute(
                "UPDATE planning_peer_conflicts SET active=0 WHERE kind=?1 AND object_id=?2 AND reason=?3",
                params![object.kind.as_str(), object.id, reason],
            )?;
        }
    }
    for (object, reason) in conflicts {
        conn.execute(
            "INSERT INTO planning_peer_conflicts(repo,kind,object_id,reason,active) VALUES(?1,?2,?3,?4,1)
             ON CONFLICT(kind,object_id,reason) DO UPDATE SET active=1 WHERE active=0",
            params![repo,object.kind.as_str(),object.id,reason],
        )?;
    }
    Ok(())
}

fn validate_wave(
    conn: &Connection,
    object: &PlanningObject,
    fields: &BTreeMap<String, Value>,
    repo: &str,
) -> StoreResult<()> {
    let id = crate::id::WaveId::parse(&object.id).map_err(invalid)?;
    let name = fields["name"]
        .as_str()
        .ok_or_else(|| invalid("Wave name must be text"))?;
    let parent = fields["parent_wave_id"]
        .as_str()
        .map(crate::id::WaveId::parse)
        .transpose()
        .map_err(invalid)?;
    super::validate_wave_parent(conn, &id, name, repo, parent.as_ref()).map_err(|error| {
        // Validation reads only live parents. A missing or retired parent is
        // an unresolved relationship, not an operational failure of the import.
        if matches!(
            error,
            StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows)
        ) {
            invalid("Wave parent is unavailable")
        } else {
            error
        }
    })
}

fn project_fields<'a>(
    conn: &Connection,
    object: &PlanningObject,
    fields: impl IntoIterator<Item = (&'a str, &'a Value)>,
) -> StoreResult<()> {
    let mut columns = BTreeMap::new();
    for (field, value) in fields {
        if matches!(field, "disposition" | "content") {
            // The merged snapshot has already validated names and grouped values.
            columns.extend(
                value
                    .as_object()
                    .expect("validated planning field group")
                    .iter()
                    .map(|(key, value)| (key.as_str(), value)),
            );
        } else {
            columns.insert(field, value);
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
    fn duplicate_provider_identity_retains_pending_objects_without_blocking_other_tasks() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        let task = seed(&left);
        let independent = TaskId::new();
        let legacy = TaskId::new();
        let project: String = left
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT project_id FROM tasks WHERE id=?1",
                [task.as_str()],
                |r| r.get(0),
            )
            .unwrap();
        {
            let conn = left.conn.lock().unwrap();
            conn.execute(
                "UPDATE tasks SET external_issue_id='provider-1' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,issue_title,created_at) VALUES(?1,?2,'FIX-2','Independent',1)", params![independent, project]).unwrap();
        }
        left.append_task_comment(
            &task,
            &TaskComment {
                id: "pending-comment".into(),
                body: "Retain even without a projected Task".into(),
                author: TaskCommentAuthor::Person {
                    name: Some("Maya".into()),
                },
                created_at: None,
            },
        )
        .unwrap();
        let incoming = left.export_peer_planning("/source").unwrap();
        let mut parents = incoming.clone();
        parents.changes.retain(|_, change| {
            matches!(
                change.object.kind,
                crate::engine::planning_exchange::PlanningKind::Wave
                    | crate::engine::planning_exchange::PlanningKind::Project
            )
        });
        right
            .import_peer_planning("/target", "synthetic", "parents", &parents)
            .unwrap();
        right.conn.lock().unwrap().execute(
            "INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,issue_title,created_at,worktree)
             VALUES(?1,?2,'provider-1','FIX-1','Legacy identity',1,'/legacy/checkout')",
            params![legacy, project],
        ).unwrap();
        let merged = right
            .import_peer_planning("/target", "synthetic", "conflict", &incoming)
            .unwrap();
        assert!(right.planning_task(&task).unwrap().record.is_none());
        assert_eq!(
            right
                .planning_task(&independent)
                .unwrap()
                .record
                .unwrap()
                .item
                .name,
            "Independent"
        );
        assert_eq!(
            right
                .planning_task(&legacy)
                .unwrap()
                .record
                .unwrap()
                .item
                .name,
            "Legacy identity"
        );
        let conflicts = right.peer_projection_conflicts("/target").unwrap();
        assert_eq!(conflicts.len(), 2);
        assert!(conflicts
            .iter()
            .any(|c| c.object.id == task.as_str() && c.reason.contains("external_issue_id")));
        assert!(conflicts.iter().any(|c| c.object.id == "pending-comment"));
        assert_eq!(right.export_peer_planning("/target").unwrap(), merged);
        let revisions = right.revisions().unwrap();
        right
            .import_peer_planning("/target", "synthetic", "conflict", &incoming)
            .unwrap();
        assert_eq!(right.revisions().unwrap(), revisions);
        assert_eq!(
            right.peer_projection_conflicts("/target").unwrap(),
            conflicts
        );
        // Pending identities cannot be claimed by another repository.
        assert!(right
            .import_peer_planning("/other", "synthetic", "cross-repo", &incoming)
            .is_err());

        // An explicit mapping repair allows a later acquisition to project the
        // retained journal even if that acquisition supplies no new mutations.
        right
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET external_issue_id=NULL WHERE id=?1",
                [legacy.as_str()],
            )
            .unwrap();
        right
            .import_peer_planning("/target", "synthetic", "repaired", &Default::default())
            .unwrap();
        assert!(right
            .peer_projection_conflicts("/target")
            .unwrap()
            .is_empty());
        assert_eq!(right.task_comments(&task).unwrap().comments.len(), 1);
        let conn = right.conn.lock().unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT worktree FROM tasks WHERE id=?1",
                [legacy.as_str()],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "/legacy/checkout"
        );
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM planning_peer_conflicts WHERE active=0",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            2
        );
    }

    #[test]
    fn protected_session_ancestry_does_not_block_independent_planning() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        let task = seed(&left);
        let base = left.export_peer_planning("/source").unwrap();
        right
            .import_peer_planning("/target", "synthetic", "base", &base)
            .unwrap();
        let (project, wave): (String, String) = right.conn.lock().unwrap().query_row(
            "SELECT t.project_id,p.wave_id FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
            [task.as_str()], |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        right.conn.lock().unwrap().execute(
            "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id)
             VALUES('session-retained','Retained','human',1,0,'/target/task',?1,?2)",
            params![task, wave],
        ).unwrap();
        let destination = Wave::new(WaveId::new(), "destination".into(), "/source".into());
        left.create_wave(&destination).unwrap();
        let moved_project = ProjectId::new();
        let independent = TaskId::new();
        {
            let conn = left.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO projects(id,wave_id,created_at) VALUES(?1,?2,1)",
                params![moved_project, destination.id()],
            )
            .unwrap();
            conn.execute(
                "UPDATE tasks SET project_id=?2 WHERE id=?1",
                params![task, moved_project],
            )
            .unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,issue_title,created_at) VALUES(?1,?2,'FIX-2','Still progresses',1)", params![independent,project]).unwrap();
        }
        let incoming = left.export_peer_planning("/source").unwrap();
        let merged = right
            .import_peer_planning("/target", "synthetic", "move", &incoming)
            .unwrap();
        assert_eq!(
            right
                .planning_task(&independent)
                .unwrap()
                .record
                .unwrap()
                .item
                .name,
            "Still progresses"
        );
        let conflicts = right.peer_projection_conflicts("/target").unwrap();
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts[0].reason.contains("AgentSession ancestry"));
        assert_eq!(right.export_peer_planning("/target").unwrap(), merged);
        let conn = right.conn.lock().unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT project_id FROM tasks WHERE id=?1",
                [task.as_str()],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            project
        );
        assert_eq!(
            conn.query_row(
                "SELECT wave_id FROM agent_sessions WHERE id='session-retained'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            wave
        );
    }

    #[test]
    fn wave_selection_waits_for_projects_and_reports_conflicts_without_revision_churn() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        let task = seed(&left);
        let (project, wave): (String, String) = left.conn.lock().unwrap().query_row(
            "SELECT t.project_id,p.wave_id FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
            [task.as_str()], |r| Ok((r.get(0)?, r.get(1)?)),
        ).unwrap();
        left.conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE waves SET current_project_id=?2 WHERE id=?1",
                params![wave, project],
            )
            .unwrap();
        let incoming = left.export_peer_planning("/source").unwrap();
        right
            .import_peer_planning("/target", "synthetic", "selected", &incoming)
            .unwrap();
        assert!(right
            .peer_projection_conflicts("/target")
            .unwrap()
            .is_empty());
        assert_eq!(
            right
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT current_project_id FROM waves WHERE id=?1",
                    [&wave],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            project
        );

        // A concurrent selection can name an unavailable Project. The selection
        // stays pending; an existing selected Project is never erased to fit it.
        let mut missing = incoming.clone();
        let (id, parent) = missing
            .changes
            .iter()
            .find(|(_, change)| {
                change.field == "current_project_id" && change.value == json!(project)
            })
            .unwrap();
        let mut selection = parent.clone();
        selection.parents = [id.clone()].into();
        selection.clock += 1;
        selection.value = json!(ProjectId::new().as_str());
        missing
            .changes
            .insert("missing-selection".into(), selection);
        right
            .import_peer_planning("/target", "synthetic", "pending-selection", &missing)
            .unwrap();
        assert_eq!(right.peer_projection_conflicts("/target").unwrap().len(), 1);
        let revisions = right.revisions().unwrap();
        right
            .import_peer_planning("/target", "synthetic", "pending-selection", &missing)
            .unwrap();
        assert_eq!(right.revisions().unwrap(), revisions);
        assert_eq!(
            right
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT current_project_id FROM waves WHERE id=?1",
                    [&wave],
                    |r| r.get::<_, String>(0)
                )
                .unwrap(),
            project
        );
    }

    #[test]
    fn missing_wave_parent_retains_the_existing_wave_while_tasks_progress() {
        let (_left_home, left) = store();
        let (_right_home, right) = store();
        let task = seed(&left);
        let base = left.export_peer_planning("/source").unwrap();
        right
            .import_peer_planning("/target", "synthetic", "base", &base)
            .unwrap();
        left.conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET issue_title='Independent edit' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        let mut incoming = left.export_peer_planning("/source").unwrap();
        let (id, previous) = incoming
            .changes
            .iter()
            .find(|(_, change)| change.field == "parent_wave_id")
            .unwrap();
        let wave = previous.object.id.clone();
        let mut parent = previous.clone();
        parent.parents = [id.clone()].into();
        parent.clock += 1;
        parent.value = json!(WaveId::new().as_str());
        incoming.changes.insert("missing-parent".into(), parent);
        let merged = right
            .import_peer_planning("/target", "synthetic", "missing-parent", &incoming)
            .unwrap();
        assert_eq!(
            right
                .planning_task(&task)
                .unwrap()
                .record
                .unwrap()
                .item
                .name,
            "Independent edit"
        );
        let conflicts = right.peer_projection_conflicts("/target").unwrap();
        assert_eq!(conflicts.len(), 1);
        assert!(conflicts[0].reason.contains("parent is unavailable"));
        assert_eq!(right.export_peer_planning("/target").unwrap(), merged);
        assert_eq!(
            right
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT parent_wave_id FROM waves WHERE id=?1",
                    [&wave],
                    |r| r.get::<_, Option<String>>(0)
                )
                .unwrap(),
            None
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
    fn reused_changes_in_another_repository_preserve_the_original_and_import_checkpoint() {
        let (_home, store) = store();
        seed(&store);
        let original = store.export_peer_planning("/source").unwrap();
        let mut reused = original.clone();
        reused
            .changes
            .values_mut()
            .find(|change| change.field == "issue_title")
            .unwrap()
            .value = json!("Conflicting identity from another plan");
        let error = store
            .import_peer_planning("/other", "synthetic", "reused", &reused)
            .unwrap_err();
        assert!(
            error.to_string().contains("conflicting contents"),
            "{error}"
        );
        assert_eq!(store.export_peer_planning("/source").unwrap(), original);
        assert!(store
            .export_peer_planning("/other")
            .unwrap()
            .changes
            .is_empty());
        assert!(store
            .peer_import_revision("/other", "synthetic")
            .unwrap()
            .is_none());
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
