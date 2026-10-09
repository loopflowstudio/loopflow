//! One saved Task thread and its delivery evidence, with or without Linear.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::durable::TaskId;
use crate::ops::pm::{TaskComment, TaskCommentAuthor, TaskComments};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

pub(super) fn ingest_task_comment(
    conn: &Connection,
    task: &TaskId,
    comment: &crate::pm::IssueComment,
    observed_at: i64,
) -> StoreResult<bool> {
    let incoming = TaskComment::from(comment);
    let incoming_revision = super::planning::revision_nanos(comment.revision.as_deref())?;
    let existing = conn
        .query_row(
            "SELECT id,body,author,created_at,task_id,provider_revision,
            NOT EXISTS(SELECT 1 FROM task_comment_deliveries WHERE comment_id=?1
                AND acknowledged=0 AND conflicting_comment_json IS NULL)
         FROM task_comments WHERE id=?1",
            [&comment.id],
            |row| {
                Ok((
                    read_comment(row)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, bool>(6)?,
                ))
            },
        )
        .optional()?;
    if let Some((saved, owner, revision, acquired)) = existing {
        if owner != task.as_str() {
            return Err(StoreError::InvalidData(
                "comment belongs to another Task".into(),
            ));
        }
        let previous = super::planning::revision_nanos(revision.as_deref())?;
        if previous.is_some() && incoming_revision < previous {
            return Ok(false);
        }
        if (previous.is_some() || acquired) && incoming_revision == previous && saved != incoming {
            return Err(StoreError::ProviderObservationConflict {
                entity: "comment",
                id: comment.id.clone(),
            });
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
                "UPDATE task_comment_deliveries SET acknowledged=1,error=NULL WHERE comment_id=?1 AND (acknowledged=0 OR error IS NOT NULL)",
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
    super::planning_peers::observe_comment(conn, task, comment, observed_at)?;
    conn.execute(
        "INSERT INTO task_comments(id,task_id,body,author,created_at,provider_revision)
         VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET body=excluded.body,
         author=excluded.author,created_at=excluded.created_at,provider_revision=excluded.provider_revision
         WHERE body IS NOT excluded.body OR author IS NOT excluded.author
            OR created_at IS NOT excluded.created_at OR provider_revision IS NOT excluded.provider_revision
            OR ((SELECT importing FROM planning_peer_context)=0 AND NOT EXISTS(
                SELECT 1 FROM planning_peer_heads h JOIN planning_peer_changes c ON c.id=h.id
                WHERE h.kind='comment' AND h.object_id=excluded.id AND h.field='content'
                    AND c.linear IS NOT NULL
                    AND json_extract(c.linear,'$.body.revision') IS excluded.provider_revision))",
        params![comment.id, task.as_str(), comment.body, serde_json::to_string(&incoming.author)?,
            comment.created_at, comment.revision],
    )?;
    super::planning_peers::clear_observation(conn)?;
    Ok(true)
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
pub(super) fn insert_authored_comment(
    conn: &Connection,
    task: &TaskId,
    comment: &TaskComment,
) -> StoreResult<bool> {
    let existing = conn
        .query_row(
            "SELECT id,body,author,created_at,task_id FROM task_comments WHERE id=?1",
            [&comment.id],
            |row| Ok((read_comment(row)?, row.get::<_, String>(4)?)),
        )
        .optional()?;
    if let Some((saved, owner)) = existing {
        if owner == task.as_str() && saved == *comment {
            // In particular, never reset a lost reply or recreate an acquired delivery.
            return Ok(false);
        }
        return Err(StoreError::InvalidData(
            "comment identity already belongs to another comment".into(),
        ));
    }
    conn.execute(
        "INSERT INTO task_comments(id,task_id,body,author,created_at) VALUES(?1,?2,?3,?4,?5)",
        params![
            comment.id,
            task.as_str(),
            comment.body,
            serde_json::to_string(&comment.author)?,
            comment.created_at
        ],
    )?;
    conn.execute(
        "INSERT INTO task_comment_deliveries(comment_id,comment_json) VALUES(?1,?2)",
        params![comment.id, serde_json::to_string(comment)?],
    )?;
    Ok(true)
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
        comment.created_at.as_deref().ok_or_else(|| {
            StoreError::InvalidData("local comments require a creation time".into())
        })?;
        if comment.body.trim().is_empty() {
            return Err(StoreError::InvalidData(
                "Task comment cannot be empty".into(),
            ));
        }
        if !insert_authored_comment(&tx, &task.id, comment)? {
            return Ok(());
        }
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
            super::ingest_task_comment(&tx, &task, &observed, 42).unwrap();
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
        super::ingest_task_comment(&store.conn.lock().unwrap(), &task, &observed, 42).unwrap();
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
        super::ingest_task_comment(&store.conn.lock().unwrap(), &task, &stale, 42).unwrap();
        assert_eq!(
            store.task_comments(&task).unwrap().comments,
            thread.comments
        );
        let amended = crate::pm::IssueComment {
            body: "Later provider correction".into(),
            revision: Some("2026-10-08T01:04:00Z".into()),
            ..observed
        };
        super::ingest_task_comment(&store.conn.lock().unwrap(), &task, &amended, 42).unwrap();
        assert_eq!(
            store.task_comments(&task).unwrap().comments[0].body,
            amended.body
        );
        assert_eq!(store.task_steers(&task).unwrap(), steers);
        assert_eq!(store.task_state(&task).unwrap(), initial_state);
    }
}
