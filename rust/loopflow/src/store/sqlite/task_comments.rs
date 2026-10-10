//! One saved Task thread and its delivery evidence, with or without Linear.

use super::planning_write::{self, PlanningEdit as Edit};
use crate::engine::planning_exchange::PlanningKind;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::durable::TaskId;
use crate::ops::pm::{TaskComment, TaskCommentAuthor, TaskComments};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

pub(super) fn ingest_task_comment(
    conn: &Connection,
    task: &TaskId,
    comment: &crate::pm::IssueComment,
) -> StoreResult<bool> {
    let existing: Option<(String, Option<String>)> = conn
        .query_row(
            "SELECT task_id,provider_revision FROM task_comments WHERE id=?1",
            [&comment.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((owner, revision)) = existing {
        if owner != task.as_str() {
            return Err(StoreError::InvalidData(
                "comment belongs to another Task".into(),
            ));
        }
        if revision.as_deref().is_some_and(|revision| {
            comment
                .revision
                .as_deref()
                .is_none_or(|incoming| incoming <= revision)
        }) {
            return Ok(false);
        }
    }
    let delivery: Option<(String, bool)> = conn
        .query_row(
            "SELECT json_extract(comment_json,'$.body'),acknowledged FROM task_comment_deliveries
             WHERE comment_id=?1 AND conflicting_comment_json IS NULL",
            [&comment.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((body, acknowledged)) = delivery {
        if body == comment.body {
            conn.execute(
                "UPDATE task_comment_deliveries SET acknowledged=1,error=NULL WHERE comment_id=?1",
                [&comment.id],
            )?;
        } else if !acknowledged {
            // Adopt Linear and retain the complete losing comment and observation.
            conn.execute(
                "UPDATE task_comment_deliveries SET conflicting_comment_json=?2,error=NULL WHERE comment_id=?1",
                params![comment.id, serde_json::to_string(comment)?],
            )?;
        }
    }
    let author = comment_author(comment);
    planning_write::create(
        conn,
        "",
        PlanningKind::Comment,
        &comment.id,
        &[
            Edit::CommentTask(task.to_string()),
            Edit::CommentContent(super::planning_write::CommentContent {
                body: comment.body.clone(),
                author: serde_json::to_string(&author)?,
                created_at: comment.created_at.clone(),
            }),
        ],
    )?;
    conn.execute(
        "UPDATE task_comments SET provider_revision=?2 WHERE id=?1",
        params![comment.id, comment.revision],
    )?;
    Ok(true)
}

fn comment_author(comment: &crate::pm::IssueComment) -> TaskCommentAuthor {
    if comment.author_id.is_some() || crate::ops::linear_observe::is_steer(&comment.body) {
        TaskCommentAuthor::Person {
            name: crate::ops::linear_observe::comment_requester(
                &comment.body,
                comment.author_name.as_deref(),
            ),
        }
    } else {
        TaskCommentAuthor::Integration
    }
}

fn read_comment(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskComment> {
    let author: String = row.get(2)?;
    Ok(TaskComment {
        id: row.get(0)?,
        body: row.get(1)?,
        author: serde_json::from_str(&author).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                2,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        created_at: row.get(3)?,
    })
}

// Every locally authored comment retains its delivery identity in the same
// transaction. Acquired provider comments enter through ingest_task_comment.
fn insert_authored_comment(
    conn: &Connection,
    task: &TaskId,
    comment: &TaskComment,
) -> StoreResult<()> {
    planning_write::create(
        conn,
        "",
        PlanningKind::Comment,
        &comment.id,
        &[
            Edit::CommentTask(task.to_string()),
            Edit::CommentContent(super::planning_write::CommentContent {
                body: comment.body.clone(),
                author: serde_json::to_string(&comment.author)?,
                created_at: comment.created_at.clone(),
            }),
        ],
    )?;
    conn.execute(
        "INSERT INTO task_comment_deliveries(comment_id,comment_json) VALUES(?1,?2)",
        params![comment.id, serde_json::to_string(comment)?],
    )?;
    Ok(())
}

impl SqliteStore {
    /// Comments and delivery badges come from one SQLite snapshot, even when
    /// another foreground connection acknowledges or resolves a delivery.
    pub fn task_comments(&self, task: &TaskId) -> StoreResult<TaskComments> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let task = super::children::task_on(&tx, task)?.ok_or(StoreError::NotFound)?;
        let repo: String = tx.query_row(
            "SELECT w.repo FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
            [task.project_id.as_str()],
            |row| row.get(0),
        )?;
        let connected = crate::ops::linear_observe::connected(&repo);
        let mut thread = TaskComments {
            identifier: task.plan.identifier,
            comments: Vec::new(),
            pending_sync: Vec::new(),
            conflicts: Default::default(),
            refresh_error: None,
        };
        {
            let mut query = tx.prepare(
                "SELECT c.id,c.body,c.author,c.created_at,
                    d.acknowledged=0 AND d.conflicting_comment_json IS NULL,
                    CASE WHEN d.conflicting_comment_json IS NOT NULL THEN json_extract(d.comment_json,'$.body') END
                 FROM task_comments c LEFT JOIN task_comment_deliveries d
                     ON d.comment_id=c.id
                 WHERE c.task_id=?1 ORDER BY c.created_at,c.id",
            )?;
            let mut rows = query.query([task.id.as_str()])?;
            while let Some(row) = rows.next()? {
                let comment = read_comment(row)?;
                if connected && row.get::<_, Option<bool>>(4)?.unwrap_or(false) {
                    thread.pending_sync.push(comment.id.clone());
                }
                if let Some(body) = row.get::<_, Option<String>>(5)? {
                    thread.conflicts.insert(comment.id.clone(), body);
                }
                thread.comments.push(comment);
            }
        }
        tx.commit()?;
        Ok(thread)
    }

    pub fn append_task_comment(&self, task: &TaskId, comment: &TaskComment) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task = super::children::task_on(&tx, task)?.ok_or(StoreError::NotFound)?;
        super::children::require_task_not_deleted(&tx, &task)?;
        let created_at = comment.created_at.as_deref().ok_or_else(|| {
            StoreError::InvalidData("local comments require a creation time".into())
        })?;
        if comment.body.trim().is_empty() {
            return Err(StoreError::InvalidData(
                "Task comment cannot be empty".into(),
            ));
        }
        let author = serde_json::to_string(&comment.author)?;
        let previous: Option<(String, String, String, String)> = tx
            .query_row(
                "SELECT task_id,body,author,created_at FROM task_comments WHERE id=?1",
                [&comment.id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?;
        if let Some(previous) = previous {
            if previous
                != (
                    task.id.to_string(),
                    comment.body.clone(),
                    author,
                    created_at.to_string(),
                )
            {
                return Err(StoreError::InvalidData(
                    "comment identity already belongs to another comment".into(),
                ));
            }
            return Ok(());
        }
        insert_authored_comment(&tx, &task.id, comment)?;
        if let TaskCommentAuthor::Person { name } = &comment.author {
            if crate::ops::linear_observe::is_direction_comment(&comment.body, Some("local")) {
                let text = match name {
                    Some(name) => format!("Comment {} by {name}:\n\n{}", comment.id, comment.body),
                    None => format!(
                        "Comment {} (attribution unresolved):\n\n{}",
                        comment.id, comment.body
                    ),
                };
                Self::append_task_steer_in(&tx, &task.id, &crate::durable::Author::User, &text)?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub fn pending_task_comments(&self, task: &TaskId) -> StoreResult<Vec<TaskComment>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT c.id,c.body,c.author,c.created_at FROM task_comments c
             JOIN task_comment_deliveries d ON d.comment_id=c.id
             WHERE c.task_id=?1 AND d.acknowledged=0
                 AND d.conflicting_comment_json IS NULL
             ORDER BY c.created_at,c.id",
        )?;
        let rows = query.query_map([task.as_str()], read_comment)?;
        rows.collect::<Result<_, _>>().map_err(StoreError::from)
    }

    pub fn record_comment_delivery(&self, id: &str, error: Option<&str>) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // A late failed attempt cannot undo an acknowledgement from another reader.
        conn.execute(
            "UPDATE task_comment_deliveries SET acknowledged=?2,error=?3
             WHERE comment_id=?1 AND acknowledged=0 AND conflicting_comment_json IS NULL",
            params![id, error.is_none(), error],
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use crate::durable::TaskId;
    use crate::id::WaveId;
    use crate::ops::pm::{TaskComment, TaskCommentAuthor};
    use crate::store::sqlite::SqliteStore;
    use crate::work::wave::Wave;

    #[test]
    fn linear_comment_adoption_preserves_local_history_and_execution() {
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&database).unwrap();
        let wave = Wave::new(WaveId::new(), "planning".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        let project = crate::durable::ProjectId::new();
        let task = TaskId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO projects(id,wave_id,created_at,updated_at,project_slug,project_name,project_prompt_context)
                VALUES(?1,?2,1,1,'planning','Planning','')", rusqlite::params![project.as_str(), wave.id()]).unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,issue_title,issue_description,created_at,updated_at,workspace_slug)
                VALUES(?1,?2,'linear-task','FIX-1','Keep identity','',1,1,'keep-identity')", rusqlite::params![task.as_str(), project.as_str()]).unwrap();
        }
        let original = TaskComment {
            id: uuid::Uuid::new_v4().to_string(),
            body: "Keep the local body".into(),
            author: TaskCommentAuthor::Person {
                name: Some("Maya".into()),
            },
            created_at: Some("2026-10-08T01:00:00Z".into()),
        };
        let initial_state = store.task_state(&task).unwrap();
        store.append_task_comment(&task, &original).unwrap();
        let steers = store.task_steers(&task).unwrap();
        let observed = crate::pm::IssueComment {
            id: original.id.clone(),
            body: "Provider body".into(),
            author_id: Some("person-2".into()),
            author_name: Some("Quinn".into()),
            created_at: Some("2026-10-08T01:01:00Z".into()),
            revision: Some("2026-10-08T01:02:00Z".into()),
        };
        {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            super::ingest_task_comment(&tx, &task, &observed).unwrap();
            tx.rollback().unwrap();
        }
        assert_eq!(
            store.task_comments(&task).unwrap().comments,
            vec![original.clone()]
        );
        assert_eq!(
            store.pending_task_comments(&task).unwrap(),
            vec![original.clone()]
        );
        super::ingest_task_comment(&store.conn.lock().unwrap(), &task, &observed).unwrap();
        drop(store);
        let store = SqliteStore::open_ephemeral(&database).unwrap();
        store.record_comment_delivery(&original.id, None).unwrap();
        store
            .record_comment_delivery(&original.id, Some("late failure"))
            .unwrap();
        let thread = store.task_comments(&task).unwrap();
        assert_eq!(thread.comments.len(), 1);
        assert_eq!(thread.comments[0].body, observed.body);
        assert_eq!(
            thread.comments[0].author,
            TaskCommentAuthor::Person {
                name: Some("Quinn".into())
            }
        );
        assert_eq!(thread.conflicts[&original.id], original.body);
        assert!(thread.pending_sync.is_empty());
        assert!(store.pending_task_comments(&task).unwrap().is_empty());
        let receipt: (String, String, bool) = store.conn.lock().unwrap().query_row(
            "SELECT comment_json,conflicting_comment_json,acknowledged FROM task_comment_deliveries WHERE comment_id=?1",
            [&original.id], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        ).unwrap();
        assert_eq!(
            serde_json::from_str::<TaskComment>(&receipt.0).unwrap(),
            original
        );
        assert_eq!(
            serde_json::from_str::<crate::pm::IssueComment>(&receipt.1).unwrap(),
            observed
        );
        assert!(!receipt.2);
        let stale = crate::pm::IssueComment {
            body: original.body.clone(),
            revision: Some("2026-10-08T01:00:00Z".into()),
            ..observed.clone()
        };
        super::ingest_task_comment(&store.conn.lock().unwrap(), &task, &stale).unwrap();
        assert_eq!(
            store.task_comments(&task).unwrap().comments,
            thread.comments
        );
        let amended = crate::pm::IssueComment {
            body: "Later provider correction".into(),
            revision: Some("2026-10-08T01:04:00Z".into()),
            ..observed
        };
        super::ingest_task_comment(&store.conn.lock().unwrap(), &task, &amended).unwrap();
        assert_eq!(
            store.task_comments(&task).unwrap().comments[0].body,
            amended.body
        );
        assert_eq!(store.task_steers(&task).unwrap(), steers);
        assert_eq!(store.task_state(&task).unwrap(), initial_state);
    }
}
