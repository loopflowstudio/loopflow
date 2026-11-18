//! Convert retained filing events into ordinary Tasks and comments. No new filing
//! lifecycle writes here: historical provider effects retain their original IDs.

use rusqlite::{Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};

use super::SqliteStore;
use crate::work::task::{TaskEventKind, TaskId};
use crate::{StoreError, StoreResult};
use loopflow_model::comment::{TaskComment, TaskCommentAuthor};

// Private decoding of immutable old events, never part of the Work model or DTOs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetainedFiling {
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

impl SqliteStore {
    pub fn recover_task_filings(&self, repo: &str) -> StoreResult<()> {
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
                        super::tracker_actions::retain_relation_in(
                            &tx,
                            &TaskId::from_raw(source.clone()),
                            &intent.relation_id,
                            &intent.issue_id,
                        )?;
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
}

fn recover_filing_in(conn: &Connection, intent: &RetainedFiling) -> StoreResult<String> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM tasks WHERE id=?1 OR external_issue_id=?1
         OR id='task_'||replace(lower(?1),'-','')",
            [&intent.issue_id],
            |row| row.get(0),
        )
        .optional()?;
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
            repository_id: super::repositories::repository_id_in(
                conn,
                &conn.query_row(
                    "SELECT w.repo FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
                    [&project_id],
                    |row| row.get::<_, String>(0),
                )?,
            )?
            .ok_or(StoreError::NotFound)?,
            project_id: Some(loopflow_model::durable::ProjectId::from_raw(project_id)),
            title: intent.title.clone(),
            description: intent.notes.clone(),
            due_date: intent.due.clone(),
        },
    )?;
    if !local {
        let project = conn
            .query_row(
                "SELECT id FROM projects WHERE id=?1 OR external_project_id=?1",
                [&intent.project_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        super::tracker_actions::retain_filing_create_in(
            conn,
            &task.id,
            &[
                crate::work::edit::WorkValue::TaskTitle(Some(intent.title.clone())),
                crate::work::edit::WorkValue::TaskDescription(Some(intent.notes.clone())),
                crate::work::edit::WorkValue::TaskDueDate(intent.due.clone()),
                crate::work::edit::WorkValue::TaskProject(Some(
                    project.unwrap_or_else(|| intent.project_id.clone()),
                )),
            ],
        )?;
    }
    Ok(task.identifier)
}

#[cfg(test)]
mod tests {
    use super::{RetainedFiling, SqliteStore};
    use crate::work::task::TaskEventKind;
    use crate::work::wave::Wave;
    use loopflow_model::durable::{ProjectId, TaskId};
    use loopflow_model::id::WaveId;
    use rusqlite::params;
    use serde_json::json;

    fn fixture() -> (tempfile::TempDir, SqliteStore, TaskId, RetainedFiling) {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        let wave = Wave::new(WaveId::new(), "proof".into(), "/repo".into());
        crate::test_fixture::wave(&store, &wave);
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
        let thread = store.task_comment_thread(&source).unwrap().thread;
        assert_eq!(thread.comments.len(), 1);
        assert!(thread.comments[0].body.contains(&intent.notes));
        assert_eq!(store.task_events_after(&source, 0).unwrap(), events);
        assert!(store.task_graph_row(&child).unwrap().is_none());
    }

    #[test]
    fn provider_filing_preserves_unknown_creation_and_queues_its_relation_once() {
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
        // The creation may have reached Linear: it waits for a look, with the
        // intent's values, never today's Task.
        let actions = store
            .tracker_actions_for(&crate::sqlite::tracker_actions::ActionOwner::Task(
                child.id.clone(),
            ))
            .unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(
            actions[0].kind,
            crate::sqlite::tracker_actions::ActionKind::Create
        );
        assert_eq!(
            actions[0].phase,
            crate::sqlite::tracker_actions::ActionPhase::Sent
        );
        assert_eq!(actions[0].value[0]["value"], json!(intent.title));
        assert!(actions[0].error.as_deref().unwrap().contains("unknown"));
        // The relation goes out on its source Task once, under the relation's
        // own id, aimed at the Task the filing made.
        let links = store
            .tracker_actions_for(&crate::sqlite::tracker_actions::ActionOwner::Task(
                source.clone(),
            ))
            .unwrap()
            .into_iter()
            .filter(|action| action.kind == crate::sqlite::tracker_actions::ActionKind::Link)
            .collect::<Vec<_>>();
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].id, intent.relation_id);
        assert_eq!(
            links[0].task_link(),
            Some(crate::sqlite::tracker_actions::TaskLink::Relation {
                id: intent.relation_id.clone(),
                target: child.id.clone(),
            })
        );
        assert_eq!(store.task_events_after(&source, 0).unwrap().len(), 1);
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
        assert_eq!(
            store
                .task_comment_thread(&source)
                .unwrap()
                .thread
                .comments
                .len(),
            1
        );
        assert!(
            store.task_comment_thread(&source).unwrap().thread.comments[0]
                .body
                .contains(&intent.issue_id)
        );
    }
}
