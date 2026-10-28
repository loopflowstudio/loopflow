//! Convert retained filing events into ordinary Tasks and comments. No new filing
//! lifecycle writes here: historical provider effects retain their original IDs.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use super::children::{insert_task_event_in, task_events_after_in};
use super::SqliteStore;
use crate::store::{StoreError, StoreResult};
use crate::work::task::comment::{TaskComment, TaskCommentAuthor};
use crate::work::task::{TaskEventKind, TaskId};

// Private decoding of immutable old events, never part of the Work model or DTOs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct RetainedFiling {
    pub key: String,
    pub issue_id: String,
    pub relation_id: String,
    pub project_id: String,
    pub team_id: String,
    pub state_id: Option<String>,
    pub wave: String,
    pub title: String,
    pub notes: String,
    pub due: Option<String>,
    pub existing: bool,
}

#[derive(Debug)]
pub(crate) struct RetainedRelation {
    pub task_id: TaskId,
    pub relation_id: String,
    pub source_issue_id: String,
    pub target_issue_id: String,
}

impl SqliteStore {
    pub(crate) fn recover_task_filings(&self, repo: &str) -> StoreResult<()> {
        let events = {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let mut query = conn.prepare(
                "SELECT e.id,e.task_id,e.kind_json,e.created_at FROM task_events e
                 JOIN tasks t ON t.id=e.task_id JOIN repository_plans w ON w.id=t.repository_id WHERE w.repo=?1 AND t.planning_deleted_at IS NULL
                 AND json_extract(e.kind_json,'$.kind') IN
                    ('follow_through_intent','follow_through_linked','follow_up','follow_through_conversion')
                 ORDER BY e.id",
            )?;
            let rows = query
                .query_map([repo], |row| {
                    Ok((
                        row.get::<_, i64>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        for (id, source, json, created_at) in events {
            let comment_id = uuid::Uuid::new_v5(
                &uuid::Uuid::NAMESPACE_URL,
                format!("loopflow/task/{source}/event/{id}").as_bytes(),
            )
            .to_string();
            {
                let conn = self.conn.lock().expect("store mutex poisoned");
                if conn.query_row(
                    "SELECT EXISTS(SELECT 1 FROM task_comments WHERE id=?1)",
                    [&comment_id],
                    |row| row.get::<_, bool>(0),
                )? {
                    continue;
                }
            }
            let event: TaskEventKind = serde_json::from_str(&json)?;
            let body = match event {
                TaskEventKind::FollowThroughIntent { intent } => {
                    let intent: RetainedFiling = serde_json::from_value(intent)?;
                    // Commit the reserved Task before its comment. A failed comment
                    // cannot discard a Task or authorize another creation identity.
                    let target = {
                        let mut conn = self.conn.lock().expect("store mutex poisoned");
                        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
                        let target = recover_filing_in(&tx, &intent)?;
                        tx.commit()?;
                        target
                    };
                    format!(
                        "Follow-up: {target} — {}\n{}{}",
                        intent.title,
                        intent.notes,
                        intent
                            .due
                            .as_ref()
                            .map(|due| format!("\nDue: {due}"))
                            .unwrap_or_default()
                    )
                }
                TaskEventKind::FollowThroughLinked { link } => {
                    let target = link
                        .get("identifier")
                        .and_then(|value| value.as_str())
                        .or_else(|| link.get("issue_id").and_then(|value| value.as_str()))
                        .ok_or_else(|| {
                            StoreError::InvalidData(
                                "Retained follow-up link has no identity".into(),
                            )
                        })?;
                    format!(
                        "Follow-up: {target}{}",
                        link.get("url")
                            .and_then(|value| value.as_str())
                            .map(|url| format!("\n{url}"))
                            .unwrap_or_default()
                    )
                }
                TaskEventKind::FollowUp {
                    remaining: Some(scope),
                    reason,
                } => format!(
                    "{}\nEvidence: {}\nCheck at: {}\n{}",
                    scope.outcome, scope.evidence, scope.check_at, reason
                ),
                TaskEventKind::FollowThroughConversion { reason } => reason,
                _ => continue,
            };
            let created_at = time::OffsetDateTime::from_unix_timestamp(created_at)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            self.append_task_comment(
                &TaskId::from_raw(source),
                &TaskComment {
                    id: comment_id,
                    body,
                    author: TaskCommentAuthor::Integration,
                    created_at: Some(created_at),
                },
            )?;
        }
        Ok(())
    }

    pub(crate) fn pending_retained_relations(
        &self,
        repo: &str,
    ) -> StoreResult<Vec<RetainedRelation>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT DISTINCT source.id, json_extract(e.kind_json, '$.intent.relation_id'),
                source.external_issue_id, target.external_issue_id
             FROM task_events e JOIN tasks source ON source.id=e.task_id
             JOIN repository_plans w ON w.id=source.repository_id
             JOIN tasks target ON target.id=json_extract(e.kind_json, '$.intent.issue_id')
                OR target.external_issue_id=json_extract(e.kind_json, '$.intent.issue_id')
                OR target.id=(SELECT task_id FROM task_creation_intents WHERE json_extract(export_json,'$.id')=json_extract(e.kind_json,'$.intent.issue_id'))
             WHERE w.repo=?1 AND json_extract(e.kind_json, '$.kind')='follow_through_intent'
                AND source.external_issue_id IS NOT NULL AND target.external_issue_id IS NOT NULL
                AND source.planning_deleted_at IS NULL AND target.planning_deleted_at IS NULL
                AND NOT EXISTS(SELECT 1 FROM task_events confirmed WHERE confirmed.task_id=source.id
                    AND json_extract(confirmed.kind_json, '$.kind')='follow_through_relation_confirmed'
                    AND json_extract(confirmed.kind_json, '$.relation_id')=json_extract(e.kind_json, '$.intent.relation_id')
                    AND json_extract(confirmed.kind_json, '$.source_issue_id')=source.external_issue_id
                    AND json_extract(confirmed.kind_json, '$.target_issue_id')=target.external_issue_id)",
        )?;
        let rows = query.query_map([repo], |row| {
            Ok(RetainedRelation {
                task_id: TaskId::from_raw(row.get::<_, String>(0)?),
                relation_id: row.get(1)?,
                source_issue_id: row.get(2)?,
                target_issue_id: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub(crate) fn confirm_retained_relation(&self, relation: &RetainedRelation) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let kind = TaskEventKind::FollowThroughRelationConfirmed {
            relation_id: relation.relation_id.clone(),
            source_issue_id: relation.source_issue_id.clone(),
            target_issue_id: relation.target_issue_id.clone(),
        };
        if !task_events_after_in(&tx, &relation.task_id, 0)?
            .iter()
            .any(|event| event.kind == kind)
        {
            insert_task_event_in(&tx, &relation.task_id, &kind)?;
        }
        tx.commit()?;
        Ok(())
    }
}

fn recover_filing_in(conn: &Connection, intent: &RetainedFiling) -> StoreResult<String> {
    let existing: Option<String> = conn.query_row(
        "SELECT id FROM tasks WHERE id=?1 OR external_issue_id=?1
         OR id=(SELECT task_id FROM task_creation_intents WHERE json_extract(export_json,'$.id')=?1)",
        [&intent.issue_id], |row| row.get(0),
    ).optional()?;
    if let Some(id) = existing {
        return super::children::short_task_id(conn, &TaskId::from_raw(id))?
            .ok_or(StoreError::NotFound);
    }
    if intent.existing {
        return Ok(intent.issue_id.clone());
    }
    let local = intent.issue_id.starts_with("task_");
    let id = if local {
        TaskId::parse(&intent.issue_id)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?
    } else {
        let uuid = uuid::Uuid::parse_str(&intent.issue_id)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        TaskId::from_raw(format!("task_{}", uuid.simple()))
    };
    let project_id = conn.query_row(
        "SELECT id FROM projects WHERE id=?1 OR external_project_id=?1",
        [&intent.project_id],
        |row| row.get::<_, String>(0),
    )?;
    let task = super::children::create_task_in(
        conn,
        &crate::work::task::NewTask {
            id,
            repository_id: super::repositories::ensure_repository_in(
                conn,
                &conn.query_row(
                    "SELECT w.repo FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
                    [&project_id],
                    |row| row.get::<_, String>(0),
                )?,
            )?,
            project_id: Some(crate::durable::ProjectId::from_raw(project_id)),
            title: intent.title.clone(),
            description: intent.notes.clone(),
            due_date: intent.due.clone(),
        },
    )?;
    if !local {
        super::planning_export::retain_filing_export_in(conn, &task.id, intent)?;
    }
    Ok(task.identifier)
}

#[cfg(test)]
mod tests {
    use super::{RetainedFiling, SqliteStore};
    use crate::durable::{ProjectId, TaskId};
    use crate::id::WaveId;
    use crate::store::sqlite::planning_changes::PlanningChanges;
    use crate::work::task::TaskEventKind;
    use crate::work::wave::Wave;
    use rusqlite::params;
    use serde_json::json;

    fn fixture() -> (tempfile::TempDir, SqliteStore, TaskId, RetainedFiling) {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let wave = Wave::new(WaveId::new(), "proof".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        let project = ProjectId::new();
        let source = TaskId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO projects(id,wave_id,created_at,updated_at,project_slug,project_name,description,external_project_id)
                VALUES(?1,?2,1,1,'proof','Proof','','original-project')", params![project.as_str(),wave.id()]).unwrap();
            conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,issue_title,issue_description,created_at,updated_at,workspace_slug)
                VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'source-linear','FIX-1','Source','',1,1,'source')", params![source.as_str(),project.as_str()]).unwrap();
        }
        let intent = RetainedFiling {
            key: "installed".into(),
            issue_id: TaskId::new().to_string(),
            relation_id: uuid::Uuid::new_v4().to_string(),
            project_id: project.to_string(),
            team_id: "original-team".into(),
            state_id: Some("original-state".into()),
            wave: "proof".into(),
            title: "Verify installed behavior".into(),
            notes: "Run the accepted scenario after release".into(),
            due: Some("2026-10-20".into()),
            existing: false,
        };
        (home, store, source, intent)
    }

    fn save(store: &SqliteStore, source: &TaskId, intent: &RetainedFiling) {
        store
            .append_task_event(
                source,
                &TaskEventKind::FollowThroughIntent {
                    intent: serde_json::to_value(intent).unwrap(),
                },
            )
            .unwrap();
    }

    #[test]
    fn retained_task_survives_failed_comment_and_retry_preserves_edits_and_events() {
        let (_home, store, source, intent) = fixture();
        save(&store, &source, &intent);
        let events = store.task_events_after(&source, 0).unwrap();
        store.conn.lock().unwrap().execute_batch(
            "CREATE TRIGGER reject_comment BEFORE INSERT ON task_comments BEGIN SELECT RAISE(ABORT,'comment unavailable'); END"
        ).unwrap();
        assert!(store
            .recover_task_filings("/repo")
            .unwrap_err()
            .to_string()
            .contains("comment unavailable"));
        let child = TaskId::parse(&intent.issue_id).unwrap();
        assert_eq!(store.task(&child).unwrap().unwrap().title, intent.title);
        {
            let conn = store.conn.lock().unwrap();
            conn.execute_batch("DROP TRIGGER reject_comment").unwrap();
            conn.execute(
                "UPDATE tasks SET issue_title='Edited after save' WHERE id=?1",
                [child.as_str()],
            )
            .unwrap();
        }
        for _ in 0..2 {
            store.recover_task_filings("/repo").unwrap();
        }
        assert_eq!(store.list_tasks(None).unwrap().len(), 2);
        assert_eq!(
            store.task(&child).unwrap().unwrap().title,
            "Edited after save"
        );
        let thread = store.task_comments(&source).unwrap();
        assert_eq!(thread.comments.len(), 1);
        assert!(thread.comments[0].body.contains(&intent.notes));
        assert_eq!(store.task_events_after(&source, 0).unwrap(), events);
        assert!(store.task_graph(&child).unwrap().is_none());
    }

    #[test]
    fn provider_filing_preserves_unknown_creation_and_original_relation_without_link_event() {
        let (_home, store, source, mut intent) = fixture();
        intent.issue_id = uuid::Uuid::new_v4().to_string();
        intent.project_id = "original-project".into();
        save(&store, &source, &intent);
        for _ in 0..2 {
            store.recover_task_filings("/repo").unwrap();
        }
        let child = store
            .list_tasks(None)
            .unwrap()
            .into_iter()
            .find(|task| task.id != source)
            .unwrap();
        let export = store
            .prepare_planning_export(
                PlanningChanges::Task(&child.id),
                "changed-team",
                Some("changed-initiative"),
            )
            .unwrap();
        assert_eq!(export.id, intent.issue_id);
        assert_eq!(
            export.input,
            json!({"id":intent.issue_id,"teamId":"original-team","projectId":"original-project",
            "stateId":"original-state","title":intent.title,"description":intent.notes,"dueDate":intent.due})
        );
        let (attempted, _) = store
            .planning_export_attempts(PlanningChanges::Task(&child.id))
            .unwrap();
        assert!(attempted);
        let error: String = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT export_error FROM task_creation_intents WHERE task_id=?1",
                [child.id.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert!(error.contains("unknown"));
        assert!(store
            .pending_retained_relations("/repo")
            .unwrap()
            .is_empty());
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET external_issue_id=?2 WHERE id=?1",
                params![child.id.as_str(), intent.issue_id],
            )
            .unwrap();
        let pending = store.pending_retained_relations("/repo").unwrap();
        assert_eq!(pending.len(), 1);
        assert_eq!(pending[0].relation_id, intent.relation_id);
        assert_eq!(pending[0].target_issue_id, intent.issue_id);
        assert_eq!(pending[0].source_issue_id, "source-linear");
        for _ in 0..2 {
            store.confirm_retained_relation(&pending[0]).unwrap();
        }
        assert!(store
            .pending_retained_relations("/repo")
            .unwrap()
            .is_empty());
        assert_eq!(store.task_events_after(&source, 0).unwrap().len(), 2);
    }

    #[test]
    fn linking_existing_work_never_creates_another_task() {
        let (_home, store, source, mut intent) = fixture();
        intent.existing = true;
        intent.issue_id = uuid::Uuid::new_v4().to_string();
        save(&store, &source, &intent);
        for _ in 0..2 {
            store.recover_task_filings("/repo").unwrap();
        }
        assert_eq!(store.list_tasks(None).unwrap().len(), 1);
        assert_eq!(store.task_comments(&source).unwrap().comments.len(), 1);
        assert!(store.task_comments(&source).unwrap().comments[0]
            .body
            .contains(&intent.issue_id));
    }
}
