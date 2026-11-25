//! One saved Task thread and its tracker actions, with or without a tracker.

use super::planning_write;
use crate::work::edit::WorkValue as Edit;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::{StoreError, StoreResult};
use loopflow_model::comment::TaskComments;
use loopflow_model::comment::{TaskComment, TaskCommentAuthor};
use loopflow_model::durable::TaskId;

use super::SqliteStore;

pub(super) fn ingest_task_comment(
    conn: &Connection,
    task: &TaskId,
    comment: &crate::planning::TrackerComment,
    machine_names: &std::collections::HashMap<loopflow_model::durable::MachineId, String>,
) -> StoreResult<bool> {
    // A historical filing keeps its original local UUID. Only its Linear
    // representation changes version bits; the local comment proves the link.
    let mut comment = comment.clone();
    if let Ok(uuid) = uuid::Uuid::parse_str(&comment.id) {
        if uuid.get_version_num() == 4 {
            let mut bytes = *uuid.as_bytes();
            bytes[6] = (bytes[6] & 0x0f) | 0x50;
            let local = uuid::Uuid::from_bytes(bytes).to_string();
            let retained: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM task_comments WHERE id=?1)",
                [&local],
                |row| row.get(0),
            )?;
            if retained {
                comment.id = local;
            }
        }
    }
    let comment = &comment;
    // Provider order for ownership entries is the comment's Linear createdAt.
    super::task_ownership::ingest_marker_in(
        conn,
        task,
        &comment.body,
        super::task_ownership::provider_order(comment.created_at.as_deref()),
        machine_names,
    )?;
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
    let unconfirmed: Option<(String, String)> = conn
        .query_row(
            "SELECT id,json_extract(value_json,'$.value.body') FROM tracker_actions
             WHERE task_id=?1 AND kind='comment' AND subject=?2 AND phase IN ('queued','sent')
             ORDER BY seq DESC LIMIT 1",
            params![task.as_str(), comment.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((action, body)) = unconfirmed {
        // The tracker holds this comment: ours arrived, or someone edited it
        // there first, and then theirs wins and the action keeps ours.
        let resolution = if body == comment.body {
            super::tracker_actions::Resolution::Confirmed {
                revision: comment.revision.clone(),
                remote_id: Some(comment.id.clone()),
            }
        } else {
            super::tracker_actions::Resolution::Conflicted {
                remote: serde_json::json!({"revision": comment.revision, "value": comment}),
            }
        };
        super::tracker_actions::resolve_in(conn, &action, &resolution)?;
    }
    write_comment(
        conn,
        task,
        &TaskComment::from(comment),
        planning_write::WriteOrigin::Tracker,
    )?;
    conn.execute(
        "UPDATE task_comments SET provider_revision=?2 WHERE id=?1",
        params![comment.id, comment.revision],
    )?;
    Ok(true)
}

pub(super) fn read_comment(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskComment> {
    let author: String = row.get(2)?;
    let body: String = row.get(1)?;
    let author = serde_json::from_str::<TaskCommentAuthor>(&author).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(error))
    })?;
    Ok(TaskComment {
        id: row.get(0)?,
        author: author.read(&body),
        body,
        created_at: row.get(3)?,
    })
}

fn write_comment(
    conn: &Connection,
    task: &TaskId,
    comment: &TaskComment,
    origin: planning_write::WriteOrigin,
) -> StoreResult<()> {
    planning_write::create(
        conn,
        "",
        &crate::work::edit::WorkEdit::for_target(
            &comment.id,
            &[
                Edit::CommentTask(task.to_string()),
                Edit::CommentContent(crate::work::edit::CommentContent {
                    body: comment.body.clone(),
                    author: comment.author.stored(),
                    created_at: comment.created_at.clone(),
                }),
            ],
        ),
        origin,
    )
}

// Local and acquired comments share the field writer; only a local save
// queues the comment for the tracker.
pub(super) fn insert_authored_comment(
    conn: &Connection,
    task: &TaskId,
    comment: &TaskComment,
) -> StoreResult<()> {
    write_comment(conn, task, comment, planning_write::WriteOrigin::Local)
}

/// The saved thread and its repository, read together before provider presentation.
#[derive(Debug)]
pub struct TaskCommentThread {
    pub repo: String,
    pub thread: TaskComments,
}

impl SqliteStore {
    /// The Task's comments that are not also steers. A direction comment
    /// reaches Sessions once, as the steer saved with source `comment:<id>`.
    pub fn task_undirected_comments(&self, task: &TaskId) -> StoreResult<Vec<TaskComment>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT id,body,author,created_at FROM task_comments c
             WHERE task_id=?1
               AND NOT EXISTS(SELECT 1 FROM steers s WHERE s.source='comment:'||c.id)
             ORDER BY created_at,id",
        )?;
        let comments = query
            .query_map([task.as_str()], read_comment)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(comments)
    }

    /// Comments and delivery badges come from one SQLite snapshot, even when
    /// another foreground connection acknowledges or resolves a delivery.
    pub fn task_comment_thread(&self, task: &TaskId) -> StoreResult<TaskCommentThread> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let task = super::children::task_on(&tx, task)?.ok_or(StoreError::NotFound)?;
        let repo: String = tx.query_row(
            "SELECT repo FROM repository_plans WHERE id=?1",
            [task.repository_id.as_str()],
            |row| row.get(0),
        )?;
        let mut thread = TaskComments {
            identifier: task.identifier,
            comments: Vec::new(),
            pending_sync: Vec::new(),
            conflicts: Default::default(),
            refresh_error: None,
        };
        {
            let mut query = tx.prepare(
                "SELECT c.id,c.body,c.author,c.created_at,
                    EXISTS(SELECT 1 FROM tracker_actions e WHERE e.task_id=c.task_id AND e.kind='comment'
                        AND e.subject=c.id AND e.phase IN ('queued','sent')),
                    (SELECT json_extract(e.value_json,'$.value.body') FROM tracker_actions e
                        WHERE e.task_id=c.task_id AND e.kind='comment' AND e.subject=c.id
                        AND e.conflict_json IS NOT NULL ORDER BY e.seq DESC LIMIT 1)
                 FROM task_comments c WHERE c.task_id=?1 ORDER BY c.created_at,c.id",
            )?;
            let mut rows = query.query([task.id.as_str()])?;
            while let Some(row) = rows.next()? {
                let comment = read_comment(row)?;
                if row.get::<_, Option<bool>>(4)?.unwrap_or(false) {
                    thread.pending_sync.push(comment.id.clone());
                }
                if let Some(body) = row.get::<_, Option<String>>(5)? {
                    thread.conflicts.insert(comment.id.clone(), body);
                }
                thread.comments.push(comment);
            }
        }
        tx.commit()?;
        Ok(TaskCommentThread { repo, thread })
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
        let author = serde_json::to_string(&comment.author.stored())?;
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
        comment_steer_in(&tx, &task.id, comment)?;
        tx.commit()?;
        Ok(())
    }

    /// Which transport has acknowledged this exact local comment, if any.
    pub fn comment_carrier(&self, task: &TaskId, comment: &str) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT tracker FROM tracker_actions
             WHERE task_id=?1 AND subject=?2 AND kind='comment' AND phase='confirmed'
             UNION ALL
             SELECT c.carrier FROM plan_changes p JOIN plan_carriers c ON c.repo=p.repo
             WHERE p.owner_table='task_comments' AND p.owner_id=?2 AND c.sent_seq>=p.seq
             LIMIT 1",
                params![task.as_str(), comment],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn pending_task_comments(&self, task: &TaskId) -> StoreResult<Vec<TaskComment>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT c.id,c.body,c.author,c.created_at FROM task_comments c
             WHERE c.task_id=?1 AND EXISTS(SELECT 1 FROM tracker_actions e WHERE e.task_id=c.task_id
                 AND e.kind='comment' AND e.subject=c.id AND e.phase IN ('queued','sent'))
             ORDER BY c.created_at,c.id",
        )?;
        let rows = query.query_map([task.as_str()], read_comment)?;
        rows.collect::<Result<_, _>>().map_err(StoreError::from)
    }
}

/// A person's direction comment steers the Task's Sessions at their next
/// boundary, wherever the Task runs. Local comments and comments arriving
/// from the hub render identically, so every Machine agrees on the
/// steer's `(source, revision)` and text.
pub(super) fn comment_steer_in(
    tx: &rusqlite::Transaction<'_>,
    task: &TaskId,
    comment: &TaskComment,
) -> StoreResult<()> {
    let name = match &comment.author {
        TaskCommentAuthor::Person { name } => name,
        TaskCommentAuthor::Session { requester, .. } => requester,
        TaskCommentAuthor::Integration => return Ok(()),
    };
    if !loopflow_model::comment::is_direction_comment(&comment.body, Some("local")) {
        return Ok(());
    }
    let text = match &comment.author {
        TaskCommentAuthor::Person { name: None } => format!(
            "Comment {} (attribution unresolved):\n\n{}",
            comment.id, comment.body
        ),
        author => format!("Comment {} by {author}:\n\n{}", comment.id, comment.body),
    };
    SqliteStore::save_task_steer_in(
        tx,
        task,
        &name
            .as_ref()
            .map(|name| loopflow_model::durable::Author::Named(name.clone()))
            .unwrap_or(loopflow_model::durable::Author::User),
        &text,
        &format!("comment:{}", comment.id),
        "local",
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {

    use crate::work::wave::Wave;
    use loopflow_model::comment::{TaskComment, TaskCommentAuthor};
    use loopflow_model::durable::TaskId;
    use loopflow_model::id::WaveId;

    #[test]
    fn a_sessions_comment_is_stored_as_its_requesters_and_read_as_the_sessions() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        let wave = Wave::new(WaveId::new(), "planning".into(), "/repo".into());
        crate::test_fixture::wave(&store, &wave);
        let project = loopflow_model::durable::ProjectId::new();
        let task = TaskId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT OR REPLACE INTO projects(id,wave_id,created_at,updated_at,project_slug,project_name,description)
                VALUES(?1,?2,1,1,'planning','Planning','')", rusqlite::params![project.as_str(), wave.id()]).unwrap();
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,issue_identifier,issue_title,issue_description,created_at,updated_at,workspace_slug)
                VALUES((SELECT id FROM repository_plans WHERE selected=1),?1,?2,'FIX-2','Attribute','',1,1,'attribute')", rusqlite::params![task.as_str(), project.as_str()]).unwrap();
        }
        let comment = TaskComment {
            id: uuid::Uuid::new_v4().to_string(),
            body: "Restart after a crash\n\n<!-- loopflow-session:session-a -->".into(),
            author: TaskCommentAuthor::Session {
                session_id: "session-a".into(),
                requester: Some("Jack Heart".into()),
            },
            created_at: Some("2026-10-29T01:00:00Z".into()),
        };
        store.append_task_comment(&task, &comment).unwrap();
        // Released readers parse every stored author.
        let stored: Vec<String> = store
            .conn
            .lock()
            .unwrap()
            .prepare("SELECT author FROM task_comments")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(stored, vec![r#"{"kind":"person","name":"Jack Heart"}"#]);
        assert_eq!(
            store.task_comment_thread(&task).unwrap().thread.comments,
            vec![comment.clone()]
        );
        // Appending it again is the same comment, not another one.
        store.append_task_comment(&task, &comment).unwrap();
    }

    #[test]
    fn linear_comment_adoption_preserves_local_history_and_execution() {
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("store.db");
        let store = crate::test_fixture::open(&database).unwrap();
        let wave = Wave::new(WaveId::new(), "planning".into(), "/repo".into());
        crate::test_fixture::wave(&store, &wave);
        let project = loopflow_model::durable::ProjectId::new();
        let task = TaskId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT OR REPLACE INTO projects(id,wave_id,created_at,updated_at,project_slug,project_name,description)
                VALUES(?1,?2,1,1,'planning','Planning','')", rusqlite::params![project.as_str(), wave.id()]).unwrap();
            conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,issue_title,issue_description,created_at,updated_at,workspace_slug)
                VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'linear-task','FIX-1','Keep identity','',1,1,'keep-identity')", rusqlite::params![task.as_str(), project.as_str()]).unwrap();
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
        let observed = crate::planning::TrackerComment {
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
            super::ingest_task_comment(&tx, &task, &observed, &Default::default()).unwrap();
            tx.rollback().unwrap();
        }
        assert_eq!(
            store.task_comment_thread(&task).unwrap().thread.comments,
            vec![original.clone()]
        );
        assert_eq!(
            store.pending_task_comments(&task).unwrap(),
            vec![original.clone()]
        );
        super::ingest_task_comment(
            &store.conn.lock().unwrap(),
            &task,
            &observed,
            &Default::default(),
        )
        .unwrap();
        drop(store);
        let store = crate::test_fixture::open(&database).unwrap();
        let thread = store.task_comment_thread(&task).unwrap().thread;
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
        let action = store
            .tracker_actions_for(&crate::sqlite::tracker_actions::ActionOwner::Task(
                task.clone(),
            ))
            .unwrap()
            .remove(0);
        assert_eq!(action.value["value"]["body"], original.body);
        assert_eq!(
            serde_json::from_value::<crate::planning::TrackerComment>(
                action.conflict.unwrap()["value"].clone()
            )
            .unwrap(),
            observed
        );
        let stale = crate::planning::TrackerComment {
            body: original.body.clone(),
            revision: Some("2026-10-08T01:00:00Z".into()),
            ..observed.clone()
        };
        super::ingest_task_comment(
            &store.conn.lock().unwrap(),
            &task,
            &stale,
            &Default::default(),
        )
        .unwrap();
        assert_eq!(
            store.task_comment_thread(&task).unwrap().thread.comments,
            thread.comments
        );
        let amended = crate::planning::TrackerComment {
            body: "Later provider correction".into(),
            revision: Some("2026-10-08T01:04:00Z".into()),
            ..observed
        };
        super::ingest_task_comment(
            &store.conn.lock().unwrap(),
            &task,
            &amended,
            &Default::default(),
        )
        .unwrap();
        assert_eq!(
            store.task_comment_thread(&task).unwrap().thread.comments[0].body,
            amended.body
        );
        assert_eq!(store.task_steers(&task).unwrap(), steers);
        assert_eq!(store.task_state(&task).unwrap(), initial_state);
    }
}
