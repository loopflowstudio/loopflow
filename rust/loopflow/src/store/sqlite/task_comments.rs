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
) -> StoreResult<()> {
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
            return Ok(());
        }
    }
    let delivery: Option<(String, bool, bool)> = conn
        .query_row(
            "SELECT json_extract(comment_json,'$.body'),acknowledged,conflicting_comment_json IS NOT NULL FROM task_comment_deliveries WHERE comment_id=?1 AND resolution IS NULL",
            [&comment.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if let Some((body, acknowledged, conflict)) = delivery {
        if body == comment.body {
            if conflict {
                return Ok(());
            }
            conn.execute(
                "UPDATE task_comment_deliveries SET acknowledged=1,error=NULL,conflicting_comment_json=NULL WHERE comment_id=?1",
                [&comment.id],
            )?;
            conn.execute(
                "UPDATE task_comments SET provider_revision=?2 WHERE id=?1",
                params![comment.id, comment.revision],
            )?;
            return Ok(());
        }
        if !acknowledged || conflict {
            // Preserve both values; an ID collision is not an acknowledgement.
            conn.execute(
                "UPDATE task_comment_deliveries SET conflicting_comment_json=?2 WHERE comment_id=?1",
                params![comment.id, serde_json::to_string(comment)?],
            )?;
            conn.execute(
                "UPDATE task_comments SET provider_revision=?2 WHERE id=?1",
                params![comment.id, comment.revision],
            )?;
            return Ok(());
        }
    }
    let author = comment_author(comment);
    conn.execute(
        "INSERT INTO task_comments(id,task_id,body,author,created_at,provider_revision)
         VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET body=excluded.body,
         author=excluded.author,created_at=excluded.created_at,provider_revision=excluded.provider_revision",
        params![comment.id, task.as_str(), comment.body, serde_json::to_string(&author)?,
            comment.created_at, comment.revision],
    )?;
    Ok(())
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
    Ok(())
}

impl SqliteStore {
    /// Comments and delivery badges come from one SQLite snapshot, even when
    /// another foreground connection acknowledges or resolves a delivery.
    pub fn task_comments(&self, task: &TaskId) -> StoreResult<TaskComments> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let task = super::children::task_on(&tx, task)?.ok_or(StoreError::NotFound)?;
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
                    json_extract(d.conflicting_comment_json,'$.body')
                 FROM task_comments c LEFT JOIN task_comment_deliveries d
                     ON d.comment_id=c.id AND d.resolution IS NULL
                 WHERE c.task_id=?1 ORDER BY c.created_at,c.id",
            )?;
            let mut rows = query.query([task.id.as_str()])?;
            while let Some(row) = rows.next()? {
                let comment = read_comment(row)?;
                if task.plan.linear_id.is_some() && row.get::<_, Option<bool>>(4)?.unwrap_or(false)
                {
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
                 AND d.conflicting_comment_json IS NULL AND d.resolution IS NULL
             ORDER BY c.created_at,c.id",
        )?;
        let rows = query.query_map([task.as_str()], read_comment)?;
        rows.collect::<Result<_, _>>().map_err(StoreError::from)
    }

    /// Resolve the retained collision without overwriting a provider comment.
    /// Keeping the local body publishes a new comment with its own durable ID.
    pub fn resolve_task_comment(
        &self,
        task: &TaskId,
        comment: &str,
        keep_local: bool,
    ) -> StoreResult<Option<String>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let saved = super::children::task_on(&tx, task)?.ok_or(StoreError::NotFound)?;
        super::children::require_task_not_deleted(&tx, &saved)?;
        let (original, conflict, resolved, replacement): (
            String, Option<String>, Option<String>, Option<String>,
        ) = tx.query_row(
            "SELECT d.comment_json,d.conflicting_comment_json,d.resolution,d.replacement_comment_id
             FROM task_comments c JOIN task_comment_deliveries d ON d.comment_id=c.id
             WHERE c.id=?1 AND c.task_id=?2",
            params![comment, task.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        ).optional()?.ok_or(StoreError::NotFound)?;
        let choice = if keep_local { "local" } else { "linear" };
        if let Some(resolved) = resolved {
            if resolved != choice {
                return Err(StoreError::InvalidAuthority(
                    "comment conflict is already resolved; inspect its retained decision".into(),
                ));
            }
            return Ok(replacement);
        }
        let observed: crate::pm::IssueComment =
            serde_json::from_str(&conflict.ok_or_else(|| {
                StoreError::InvalidData("comment has no retained conflict".into())
            })?)?;
        let original: TaskComment = serde_json::from_str(&original)?;
        let replacement = if keep_local {
            let id = uuid::Uuid::new_v4().to_string();
            let created = time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            let replacement = TaskComment {
                id: id.clone(),
                created_at: Some(created),
                ..original
            };
            insert_authored_comment(&tx, task, &replacement)?;
            // The original local comment already supplied its Steer. Republishing
            // it must not repeat direction or start work.
            Some(id)
        } else {
            None
        };
        tx.execute(
            "UPDATE task_comments SET body=?2,author=?3,created_at=?4,provider_revision=?5 WHERE id=?1",
            params![comment, observed.body, serde_json::to_string(&comment_author(&observed))?, observed.created_at, observed.revision],
        )?;
        tx.execute(
            "UPDATE task_comment_deliveries SET resolution=?2,replacement_comment_id=?3 WHERE comment_id=?1",
            params![comment, choice, replacement],
        )?;
        super::children::insert_task_event_in(&tx, task, &crate::work::task::TaskEventKind::Progress {
            summary: format!("Resolved comment {comment}: retained Linear's comment{}; both conflict values remain in delivery history.",
                replacement.as_ref().map(|id| format!(" and saved the local body as comment {id}")).unwrap_or_default()),
        })?;
        tx.commit()?;
        Ok(replacement)
    }

    pub fn record_comment_delivery(&self, id: &str, error: Option<&str>) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // A late failed attempt cannot undo an acknowledgement from another reader.
        conn.execute(
            "UPDATE task_comment_deliveries SET acknowledged=?2,error=?3
             WHERE comment_id=?1 AND acknowledged=0 AND resolution IS NULL",
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
    fn comment_resolution_retains_both_values_and_retries_without_repeating_direction() {
        for (keep_local, mapping) in [
            (false, None),
            (true, None),
            (false, Some("linear-task")),
            (true, Some("linear-task")),
        ] {
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
                    VALUES(?1,?2,?3,?4,'Keep identity','',1,1,'keep-identity')", rusqlite::params![task.as_str(), project.as_str(), mapping,
                        if mapping.is_some() { String::from("FIX-1") } else { format!("lf-{}", &task.as_str()[5..]) }]).unwrap();
            }
            let empty = store.task_comments(&task).unwrap();
            assert_eq!(
                empty.identifier,
                if mapping.is_some() {
                    String::from("FIX-1")
                } else {
                    format!("lf-{}", &task.as_str()[5..12])
                }
            );
            assert!(empty.comments.is_empty());
            assert!(empty.pending_sync.is_empty());
            assert!(empty.conflicts.is_empty());
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
            let saved = store.task_comments(&task).unwrap();
            assert_eq!(saved.comments, vec![original.clone()]);
            assert_eq!(
                saved.pending_sync,
                if mapping.is_some() {
                    vec![original.id.clone()]
                } else {
                    Vec::new()
                }
            );
            let steers = store.task_steers(&task).unwrap();
            let observed = crate::pm::IssueComment {
                id: original.id.clone(),
                body: "Keep the provider body".into(),
                author_id: Some("person-2".into()),
                author_name: Some("Quinn".into()),
                created_at: Some("2026-10-08T01:01:00Z".into()),
                revision: Some("2026-10-08T01:02:00Z".into()),
            };
            {
                let conn = store.conn.lock().unwrap();
                super::ingest_task_comment(&conn, &task, &observed).unwrap();
                // A later echo cannot implicitly resolve an observed collision.
                let echo = crate::pm::IssueComment {
                    body: original.body.clone(),
                    revision: Some("2026-10-08T01:03:00Z".into()),
                    ..observed.clone()
                };
                super::ingest_task_comment(&conn, &task, &echo).unwrap();
                conn.execute_batch(
                    "CREATE TRIGGER fail_resolution BEFORE INSERT ON task_events
                    BEGIN SELECT RAISE(ABORT,'fixture rollback'); END;",
                )
                .unwrap();
            }
            assert!(store
                .resolve_task_comment(&task, &original.id, keep_local)
                .is_err());
            let conflicted = store.task_comments(&task).unwrap();
            assert_eq!(conflicted.comments, vec![original.clone()]);
            assert_eq!(conflicted.conflicts[&original.id], observed.body);
            assert!(conflicted.pending_sync.is_empty());
            store
                .conn
                .lock()
                .unwrap()
                .execute_batch("DROP TRIGGER fail_resolution")
                .unwrap();
            assert!(store
                .resolve_task_comment(&TaskId::new(), &original.id, keep_local)
                .is_err());
            let replacement = store
                .resolve_task_comment(&task, &original.id, keep_local)
                .unwrap();
            assert_eq!(replacement.is_some(), keep_local);
            let resolved = store.task_comments(&task).unwrap();
            assert!(resolved.conflicts.is_empty());
            assert_eq!(
                resolved.pending_sync,
                replacement
                    .iter()
                    .filter(|_| mapping.is_some())
                    .cloned()
                    .collect::<Vec<_>>()
            );
            let comments = resolved.comments;
            let provider = comments.iter().find(|c| c.id == original.id).unwrap();
            assert_eq!(provider.body, observed.body);
            assert_eq!(
                provider.author,
                TaskCommentAuthor::Person {
                    name: Some("Quinn".into())
                }
            );
            assert_eq!(provider.created_at, observed.created_at);
            let pending = store.pending_task_comments(&task).unwrap();
            if let Some(id) = &replacement {
                assert_eq!(pending.len(), 1);
                assert_eq!(&pending[0].id, id);
                assert_ne!(id, &original.id);
                assert_eq!(pending[0].body, original.body);
                assert_eq!(pending[0].author, original.author);
            } else {
                assert!(pending.is_empty());
            }
            assert_eq!(store.task_steers(&task).unwrap(), steers);
            let retained: (String, String) = store.conn.lock().unwrap().query_row(
                "SELECT comment_json,conflicting_comment_json FROM task_comment_deliveries WHERE comment_id=?1",
                [&original.id], |row| Ok((row.get(0)?, row.get(1)?)),
            ).unwrap();
            assert_eq!(
                serde_json::from_str::<TaskComment>(&retained.0).unwrap(),
                original
            );
            assert_eq!(
                serde_json::from_str::<crate::pm::IssueComment>(&retained.1).unwrap(),
                observed
            );
            drop(store);
            let store = SqliteStore::open_ephemeral(&database).unwrap();
            assert_eq!(
                store
                    .resolve_task_comment(&task, &original.id, keep_local)
                    .unwrap(),
                replacement
            );
            assert!(store
                .resolve_task_comment(&task, &original.id, !keep_local)
                .is_err());
            store.record_comment_delivery(&original.id, None).unwrap();
            assert_eq!(store.pending_task_comments(&task).unwrap(), pending);
            assert_eq!(store.task_comments(&task).unwrap().comments, comments);
            assert_eq!(store.task_steers(&task).unwrap(), steers);
            let amended = crate::pm::IssueComment {
                body: "Later provider correction".into(),
                revision: Some("2026-10-08T01:04:00Z".into()),
                ..observed
            };
            super::ingest_task_comment(&store.conn.lock().unwrap(), &task, &amended).unwrap();
            assert!(store.task_comments(&task).unwrap().conflicts.is_empty());
            assert_eq!(
                store
                    .task_comments(&task)
                    .unwrap()
                    .comments
                    .iter()
                    .find(|c| c.id == original.id)
                    .unwrap()
                    .body,
                amended.body
            );
            assert_eq!(store.task_state(&task).unwrap(), initial_state);
        }
    }
}
