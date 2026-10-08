//! Stored planning and definitions live beside durable Work.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::durable::{PlanId, ProjectId};
use crate::id::WaveId;
use crate::planning::{PersonalWaveDefinition, PlanningAuthority};
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::work::project::Project;

use super::{durable, SqliteStore};

pub(super) fn project_authority_on(
    conn: &Connection,
    project: &ProjectId,
) -> StoreResult<PlanningAuthority> {
    let personal: bool = conn.query_row(
        "SELECT w.personal_plan_id IS NOT NULL FROM projects p
         JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
        [project.as_str()],
        |row| row.get(0),
    )?;
    Ok(if personal {
        PlanningAuthority::Local
    } else {
        PlanningAuthority::Linear
    })
}

pub(super) fn ingest_task_comment(
    conn: &Connection,
    task: &crate::durable::TaskId,
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

fn comment_author(comment: &crate::pm::IssueComment) -> crate::ops::pm::TaskCommentAuthor {
    if comment.author_id.is_some() || crate::ops::linear_observe::is_steer(&comment.body) {
        crate::ops::pm::TaskCommentAuthor::Person {
            name: crate::ops::linear_observe::comment_requester(
                &comment.body,
                comment.author_name.as_deref(),
            ),
        }
    } else {
        crate::ops::pm::TaskCommentAuthor::Integration
    }
}

impl SqliteStore {
    pub(crate) fn delete_local_task(&self, id: &crate::durable::TaskId) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let task = super::children::task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
        if project_authority_on(&tx, &task.project_id)? != PlanningAuthority::Local {
            return Err(StoreError::InvalidAuthority(
                "Task belongs to Linear".into(),
            ));
        }
        tx.execute(
            "UPDATE tasks SET planning_deleted_at=COALESCE(planning_deleted_at,?2) WHERE id=?1",
            params![id.as_str(), now_unix()],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn wave_workflows(&self, wave: &WaveId) -> StoreResult<Vec<(String, String)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query =
            conn.prepare("SELECT name,content FROM wave_workflows WHERE wave_id=?1 ORDER BY name")?;
        let rows = query.query_map([wave], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub(crate) fn edit_local_project(
        &self,
        id: &ProjectId,
        name: Option<&str>,
        summary: Option<&str>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if project_authority_on(&tx, id)? != PlanningAuthority::Local {
            return Err(StoreError::InvalidAuthority(
                "Project belongs to Linear".into(),
            ));
        }
        tx.execute(
            "UPDATE projects SET project_name=COALESCE(?2,project_name),
            project_summary=COALESCE(?3,project_summary),updated_at=?4 WHERE id=?1",
            params![id.as_str(), name, summary, now_unix()],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn refile_unplaced_task(
        &self,
        task: &crate::durable::TaskId,
        destination: &ProjectId,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = super::children::task_on(&tx, task)?.ok_or(StoreError::NotFound)?;
        super::children::require_task_not_deleted(&tx, &current)?;
        if current.project_id == *destination {
            return Ok(());
        }
        if project_authority_on(&tx, &current.project_id)?
            != project_authority_on(&tx, destination)?
        {
            return Err(StoreError::InvalidAuthority(
                "refiling cannot transfer planning authority".into(),
            ));
        }
        let changed = tx.execute(
            "UPDATE tasks SET project_id=?2,planning_revision=planning_revision+1,updated_at=?3
            WHERE id=?1 AND worktree IS NULL AND started_at IS NULL AND abandon_requested_at IS NULL
            AND NOT EXISTS(SELECT 1 FROM agent_sessions WHERE task_id=?1)
            AND NOT EXISTS(SELECT 1 FROM task_workflows WHERE task_id=?1)",
            params![task.as_str(), destination.as_str(), now_unix()],
        )?;
        if changed != 1 {
            return Err(StoreError::InvalidAuthority(
                "a Task with recorded work retains its owning Wave".into(),
            ));
        }
        super::durable::inherit_task_placement(
            &tx,
            &super::children::task_on(&tx, task)?.ok_or(StoreError::NotFound)?,
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn wave_workflow(&self, wave: &WaveId, name: &str) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT content FROM wave_workflows WHERE wave_id=?1 AND name=?2",
            params![wave, name],
            |row| row.get(0),
        )
        .optional()
        .map_err(StoreError::from)
    }

    pub(crate) fn rotate_local_projects(
        &self,
        name: &str,
        entries: &[(
            crate::ops::chapter::WaveChapterPlan,
            crate::ops::chapter::WaveRotation,
        )],
    ) -> StoreResult<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for (input, rotation) in entries {
            let selected = super::project_selection::read_in(&tx, &input.wave_id)?;
            let original = serde_json::to_string(input)?;
            let settled: Option<(Option<String>, bool, Option<String>)> = tx
                .query_row(
                    "SELECT reset_name, settled_at IS NOT NULL, local_plan_json
                     FROM project_transitions WHERE wave_id=?1 AND successor_id=?2",
                    params![input.wave_id, input.successor_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            if let Some((reset, true, intent)) = settled {
                if reset.as_deref() != Some(name)
                    || selected.as_deref() != Some(&input.successor_id)
                    || intent.as_deref() != Some(&original)
                {
                    return Err(StoreError::InvalidAuthority(
                        "Project selection or original rotation request changed".into(),
                    ));
                }
                continue;
            }
            let predecessor = rotation.predecessor.as_ref().map(|p| p.id.as_str());
            if selected.as_deref() != predecessor {
                return Err(StoreError::InvalidAuthority(
                    "Project selection changed before rotation".into(),
                ));
            }
            let now = now_unix();
            let content = crate::pm::render_project_content(&input.content);
            let successor = ProjectId::parse(&input.successor_id)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            if input.create {
                tx.execute(
                    "INSERT INTO projects(id,wave_id,created_at,updated_at,project_slug,project_name,
                         project_prompt_context,status,workflow)
                     VALUES(?1,?2,?3,?3,?4,?4,?5,'started',?6)",
                    params![
                        successor.as_str(),
                        input.wave_id,
                        now,
                        input.project_name,
                        content,
                        input.content.workflow
                    ],
                )?;
                durable::inherit_project_placement(&tx, &successor)?;
            } else {
                if project_authority_on(&tx, &successor)? != PlanningAuthority::Local {
                    return Err(StoreError::InvalidAuthority(
                        "destination belongs to Linear".into(),
                    ));
                }
                let changed = tx.execute(
                    "UPDATE projects SET project_name=?3,project_prompt_context=?4,workflow=?5,
                         status='started',updated_at=?6
                     WHERE id=?1 AND wave_id=?2 AND status NOT IN ('completed','canceled')",
                    params![
                        successor.as_str(),
                        input.wave_id,
                        input.project_name,
                        content,
                        input.content.workflow,
                        now
                    ],
                )?;
                if changed != 1 {
                    return Err(StoreError::InvalidAuthority(
                        "destination is no longer available".into(),
                    ));
                }
            }
            tx.execute(
                "INSERT INTO project_transitions(wave_id,successor_id,predecessor_id,reset_name,
                     create_successor,created_at,settled_at,local_plan_json)
                 VALUES(?1,?2,?3,?4,?5,?6,?6,?7)",
                params![
                    input.wave_id,
                    successor.as_str(),
                    predecessor,
                    name,
                    input.create,
                    now,
                    original
                ],
            )?;
            for task in rotation
                .tasks
                .iter()
                .filter(|t| t.disposition == crate::ops::chapter::TaskDisposition::Move)
            {
                let changed = tx.execute(
                    "UPDATE tasks SET project_id=?2,planning_revision=planning_revision+1,updated_at=?3
                     WHERE id=?1 AND project_id=?4",
                    params![task.task.id, successor.as_str(), now, predecessor],
                )?;
                if changed != 1 {
                    return Err(StoreError::InvalidAuthority(
                        "Task membership changed before rotation".into(),
                    ));
                }
                tx.execute(
                    "INSERT INTO project_transition_items(wave_id,successor_id,issue_id)
                     VALUES(?1,?2,?3)",
                    params![input.wave_id, successor.as_str(), task.task.id],
                )?;
            }
            tx.execute(
                "UPDATE waves SET current_project_id=?2 WHERE id=?1",
                params![input.wave_id, successor.as_str()],
            )?;
            if let Some(predecessor) = predecessor {
                tx.execute(
                    "UPDATE projects SET status='completed',updated_at=?2 WHERE id=?1",
                    params![predecessor, now],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn local_task_creation_project(
        &self,
        task: &crate::durable::TaskId,
    ) -> StoreResult<Option<ProjectId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let project: Option<String> = conn
            .query_row(
                "SELECT project_id FROM task_creation_intents WHERE task_id=?1",
                [task.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        project
            .map(|id| {
                ProjectId::parse(&id).map_err(|error| StoreError::InvalidData(error.to_string()))
            })
            .transpose()
    }

    pub(crate) fn update_personal_wave_document(
        &self,
        wave: &WaveId,
        document: &str,
        content: &str,
    ) -> StoreResult<()> {
        let column = match document {
            "GOAL.md" => "goal",
            "MEMORY.md" => "memory",
            _ => return Err(StoreError::InvalidData("unknown Wave document".into())),
        };
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute(
            &format!("UPDATE personal_wave_definitions SET {column}=?2 WHERE wave_id=?1"),
            params![wave, content],
        )? == 0
        {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    pub fn update_project_content(
        &self,
        project: &ProjectId,
        content: &crate::pm::ProjectContent,
        workflow_definition: Option<&str>,
    ) -> StoreResult<()> {
        content
            .validate()
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(definition) = workflow_definition {
            let name = &content.workflow;
            if name.trim().is_empty() || name.contains([':', '/', '\\']) {
                return Err(StoreError::InvalidData("invalid Workflow name".into()));
            }
            tx.execute(
                "INSERT INTO wave_workflows(wave_id,name,content)
                SELECT wave_id,?2,?3 FROM projects WHERE id=?1
                ON CONFLICT(wave_id,name) DO UPDATE SET content=excluded.content",
                params![project.as_str(), name, definition],
            )?;
        }
        let changed = tx.execute(
            "UPDATE projects SET project_prompt_context=?2,workflow=?3,updated_at=?4 WHERE id=?1",
            params![
                project.as_str(),
                crate::pm::render_project_content(content),
                content.workflow,
                now_unix()
            ],
        )?;
        if changed != 1 {
            return Err(StoreError::NotFound);
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn local_task_fields(
        &self,
        task: &crate::durable::TaskId,
    ) -> StoreResult<(u32, Option<i64>, Option<String>)> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT (SELECT count(*) FROM tasks earlier WHERE earlier.project_id=t.project_id
                AND earlier.planning_deleted_at IS NULL
                AND (earlier.planning_rank,earlier.created_at,earlier.id)<(t.planning_rank,t.created_at,t.id)),
                    (SELECT max(created_at) FROM task_events e
                     WHERE e.task_id=t.id AND json_extract(e.kind_json,'$.kind')='completed'),planning_assignee
             FROM tasks t WHERE id=?1",
            [task.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?)
    }

    pub fn task_comments(
        &self,
        task: &crate::durable::TaskId,
    ) -> StoreResult<Vec<crate::ops::pm::TaskComment>> {
        self.read_task_comments(task, false)
    }

    fn read_task_comments(
        &self,
        task: &crate::durable::TaskId,
        pending_only: bool,
    ) -> StoreResult<Vec<crate::ops::pm::TaskComment>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT c.id,c.body,c.author,c.created_at FROM task_comments c
             WHERE c.task_id=?1 AND (NOT ?2 OR EXISTS (
                 SELECT 1 FROM task_comment_deliveries d WHERE d.comment_id=c.id
                 AND d.acknowledged=0 AND d.conflicting_comment_json IS NULL AND d.resolution IS NULL))
             ORDER BY c.created_at,c.id",
        )?;
        let rows = query.query_map(params![task.as_str(), pending_only], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (id, body, author, created_at) = row?;
            Ok(crate::ops::pm::TaskComment {
                id,
                body,
                author: serde_json::from_str(&author)?,
                created_at,
            })
        })
        .collect()
    }

    pub fn append_task_comment(
        &self,
        task: &crate::durable::TaskId,
        comment: &crate::ops::pm::TaskComment,
    ) -> StoreResult<()> {
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
        tx.execute(
            "INSERT INTO task_comments(id,task_id,body,author,created_at) VALUES(?1,?2,?3,?4,?5)",
            params![
                comment.id,
                task.id.as_str(),
                comment.body,
                author,
                created_at
            ],
        )?;
        tx.execute(
            "INSERT INTO task_comment_deliveries(comment_id,comment_json) VALUES(?1,?2)",
            params![comment.id, serde_json::to_string(comment)?],
        )?;
        if let crate::ops::pm::TaskCommentAuthor::Person { name } = &comment.author {
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

    pub fn pending_task_comments(
        &self,
        task: &crate::durable::TaskId,
    ) -> StoreResult<Vec<crate::ops::pm::TaskComment>> {
        self.read_task_comments(task, true)
    }

    pub fn task_comment_conflicts(
        &self,
        task: &crate::durable::TaskId,
    ) -> StoreResult<std::collections::BTreeMap<String, String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT c.id,json_extract(d.conflicting_comment_json,'$.body') FROM task_comments c
            JOIN task_comment_deliveries d ON d.comment_id=c.id
            WHERE c.task_id=?1 AND d.conflicting_comment_json IS NOT NULL AND d.resolution IS NULL",
        )?;
        let rows = query.query_map([task.as_str()], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<_, _>>().map_err(StoreError::from)
    }

    /// Resolve the retained collision without overwriting a provider comment.
    /// Keeping the local body publishes a new comment with its own durable ID.
    pub fn resolve_task_comment(
        &self,
        task: &crate::durable::TaskId,
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
        let original: crate::ops::pm::TaskComment = serde_json::from_str(&original)?;
        let replacement = if keep_local {
            let id = uuid::Uuid::new_v4().to_string();
            let created = time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            let replacement = crate::ops::pm::TaskComment {
                id: id.clone(),
                created_at: Some(created),
                ..original
            };
            tx.execute(
                "INSERT INTO task_comments(id,task_id,body,author,created_at) VALUES(?1,?2,?3,?4,?5)",
                params![id, task.as_str(), replacement.body, serde_json::to_string(&replacement.author)?, replacement.created_at],
            )?;
            tx.execute(
                "INSERT INTO task_comment_deliveries(comment_id,comment_json) VALUES(?1,?2)",
                params![id, serde_json::to_string(&replacement)?],
            )?;
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

    pub fn project_planning_authority(
        &self,
        project: &ProjectId,
    ) -> StoreResult<PlanningAuthority> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        project_authority_on(&conn, project)
    }

    /// Explicit creation is the only provisioning boundary; reads never call this.
    pub fn ensure_personal_project(&self, repo: &str, name: &str) -> StoreResult<Project> {
        if name.split('/').any(|part| {
            part.trim().is_empty() || part.contains([':', '\\']) || matches!(part, "." | "..")
        }) {
            return Err(StoreError::InvalidData("invalid personal Wave name".into()));
        }
        let project = {
            let mut conn = self.conn.lock().expect("store mutex poisoned");
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            tx.execute(
                "INSERT INTO personal_plans(id,repo) VALUES(?1,?2)
                ON CONFLICT(repo) DO NOTHING",
                params![PlanId::new().as_str(), repo],
            )?;
            let plan: String = tx.query_row(
                "SELECT id FROM personal_plans WHERE repo=?1",
                [repo],
                |row| row.get(0),
            )?;
            let now = now_unix();
            let mut parent: Option<WaveId> = None;
            let mut selected = None;
            for part in name.split('/') {
                let existing: Option<(WaveId, Option<String>)> = tx
                    .query_row(
                        "SELECT id,current_project_id FROM waves WHERE personal_plan_id=?1
                     AND name=?2 AND parent_wave_id IS ?3 AND retired_at IS NULL",
                        params![plan, part, parent],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?;
                let (wave, project) = match existing {
                    Some(existing) => existing,
                    None => {
                        let wave = WaveId::new();
                        tx.execute(
                            "INSERT INTO waves(id,name,repo,created_at,personal_plan_id,parent_wave_id)
                             VALUES(?1,?2,?3,?4,?5,?6)",
                            params![wave, part, repo, now, plan, parent],
                        )?;
                        tx.execute("INSERT INTO personal_wave_definitions(wave_id,goal,memory) VALUES(?1,'','')", [&wave])?;
                        durable::create_wave_work(&tx, &wave, now)?;
                        (wave, None)
                    }
                };
                parent = Some(wave);
                selected = project;
            }
            let wave = parent.expect("validated personal Wave has at least one component");
            let project = if let Some(selected) = selected {
                ProjectId::parse(&selected)
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?
            } else {
                let project = ProjectId::new();
                tx.execute(
                    "INSERT INTO projects(id,wave_id,created_at,updated_at,
                    project_slug,project_name,project_prompt_context,status,workflow)
                    VALUES(?1,?2,?3,?3,?4,?4,'','started','')",
                    params![project.as_str(), wave, now, name],
                )?;
                durable::inherit_project_placement(&tx, &project)?;
                tx.execute(
                    "UPDATE waves SET current_project_id=?2 WHERE id=?1",
                    params![wave, project.as_str()],
                )?;
                project
            };
            tx.commit()?;
            project
        };
        self.project(&project)?.ok_or(StoreError::NotFound)
    }

    pub fn personal_wave_definition(
        &self,
        wave: &WaveId,
    ) -> StoreResult<Option<PersonalWaveDefinition>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT goal,memory FROM personal_wave_definitions WHERE wave_id=?1",
            [wave],
            |row| {
                Ok(PersonalWaveDefinition {
                    goal: row.get(0)?,
                    memory: row.get(1)?,
                })
            },
        )
        .optional()
        .map_err(StoreError::from)
    }
}

#[cfg(test)]
mod tests {
    use crate::durable::TaskId;
    use crate::id::WaveId;
    use crate::planning::{NewTask, PersonalWaveDefinition, PlanningAuthority};
    use crate::store::sqlite::SqliteStore;
    use crate::work::wave::{Wave, WaveLocator};

    #[test]
    fn comment_resolution_retains_both_values_and_retries_without_repeating_direction() {
        for keep_local in [false, true] {
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
                conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,issue_title,issue_description,created_at,updated_at,workspace_slug)
                    VALUES(?1,?2,'FIX-1','Keep identity','',1,1,'keep-identity')", rusqlite::params![task.as_str(), project.as_str()]).unwrap();
            }
            let original = crate::ops::pm::TaskComment {
                id: uuid::Uuid::new_v4().to_string(),
                body: "Keep the local body".into(),
                author: crate::ops::pm::TaskCommentAuthor::Person {
                    name: Some("Maya".into()),
                },
                created_at: Some("2026-10-08T01:00:00Z".into()),
            };
            let initial_state = store.task_state(&task).unwrap();
            store.append_task_comment(&task, &original).unwrap();
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
            assert_eq!(store.task_comments(&task).unwrap(), vec![original.clone()]);
            assert_eq!(
                store.task_comment_conflicts(&task).unwrap()[&original.id],
                observed.body
            );
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
            assert!(store.task_comment_conflicts(&task).unwrap().is_empty());
            let comments = store.task_comments(&task).unwrap();
            let provider = comments.iter().find(|c| c.id == original.id).unwrap();
            assert_eq!(provider.body, observed.body);
            assert_eq!(
                provider.author,
                crate::ops::pm::TaskCommentAuthor::Person {
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
                serde_json::from_str::<crate::ops::pm::TaskComment>(&retained.0).unwrap(),
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
            assert_eq!(store.task_comments(&task).unwrap(), comments);
            assert_eq!(store.task_steers(&task).unwrap(), steers);
            let amended = crate::pm::IssueComment {
                body: "Later provider correction".into(),
                revision: Some("2026-10-08T01:04:00Z".into()),
                ..observed
            };
            super::ingest_task_comment(&store.conn.lock().unwrap(), &task, &amended).unwrap();
            assert!(store.task_comment_conflicts(&task).unwrap().is_empty());
            assert_eq!(
                store
                    .task_comments(&task)
                    .unwrap()
                    .iter()
                    .find(|c| c.id == original.id)
                    .unwrap()
                    .body,
                amended.body
            );
            assert_eq!(store.task_state(&task).unwrap(), initial_state);
        }
    }

    #[test]
    fn stored_workflows_use_the_same_owner_with_and_without_linear_mapping() {
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&database).unwrap();
        let content = crate::pm::ProjectContent {
            workflow: "review".into(),
            krs: Vec::new(),
            metric_targets: Vec::new(),
        };
        let definition = "nodes: {}\nedges: [{from: start, to: end}]\n";
        let mut owners = Vec::new();
        for (name, mapping) in [("offline", None), ("connected", Some("linear-project"))] {
            let wave = Wave::new(WaveId::new(), name.into(), "/repo".into());
            store.create_wave(&wave).unwrap();
            let project = crate::durable::ProjectId::new();
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "INSERT INTO projects(id,wave_id,external_project_id,created_at,updated_at,
                 project_slug,project_name,project_prompt_context) VALUES(?1,?2,?3,1,1,?4,?4,'')",
                    rusqlite::params![project.as_str(), wave.id(), mapping, name],
                )
                .unwrap();
            store
                .update_project_content(&project, &content, Some(definition))
                .unwrap();
            owners.push((wave.id().clone(), project));
        }
        drop(store);
        let store = SqliteStore::open_ephemeral(&database).unwrap();
        for (wave, project) in owners {
            assert_eq!(
                store.wave_workflow(&wave, "review").unwrap().as_deref(),
                Some(definition)
            );
            assert_eq!(
                store.wave_workflows(&wave).unwrap(),
                vec![("review".into(), definition.into())]
            );
            assert_eq!(
                store.project(&project).unwrap().unwrap().plan.workflow,
                "review"
            );
            let invalid = crate::pm::ProjectContent {
                workflow: "personal:review".into(),
                ..content.clone()
            };
            assert!(store
                .update_project_content(&project, &invalid, Some("changed"))
                .is_err());
            assert_eq!(
                store.wave_workflow(&wave, "review").unwrap().as_deref(),
                Some(definition)
            );
            assert_eq!(
                store.project(&project).unwrap().unwrap().plan.workflow,
                "review"
            );
        }
        assert!(store
            .update_project_content(
                &crate::durable::ProjectId::new(),
                &content,
                Some(definition)
            )
            .is_err());
    }

    #[test]
    fn local_planning_personal_names_do_not_shadow_shared_definitions() {
        let repo = loopflow_test_support::TestRepo::new();
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let canonical = crate::repository::CanonicalRepo::discover(repo.path()).unwrap();
        let shared = Wave::new(WaveId::new(), "inbox".into(), canonical.to_string());
        store.create_wave(&shared).unwrap();
        assert!(store
            .personal_wave_definition(shared.id())
            .unwrap()
            .is_none());
        let project = store
            .ensure_personal_project(&canonical.to_string(), "inbox")
            .unwrap();
        let definition = PersonalWaveDefinition {
            goal: "Private objective".into(),
            memory: "Private memory".into(),
        };
        for (document, content) in [
            ("GOAL.md", &definition.goal),
            ("MEMORY.md", &definition.memory),
        ] {
            store
                .update_personal_wave_document(&project.wave_id, document, content)
                .unwrap();
        }
        let task = store
            .create_local_task(&NewTask {
                id: TaskId::new(),
                project_id: project.id.clone(),
                title: "Local work".into(),
                description: "".into(),
            })
            .unwrap();
        assert!(task.worktree.is_none());
        assert_eq!(
            store.project_planning_authority(&project.id).unwrap(),
            PlanningAuthority::Local
        );
        assert!(store.task_prs(&task.id).unwrap().is_empty());
        assert_eq!(
            store
                .get_wave_at(&WaveLocator::new(canonical.clone(), "inbox").unwrap())
                .unwrap()
                .unwrap()
                .id(),
            shared.id()
        );
        assert_eq!(
            store
                .get_wave_at(&WaveLocator::new(canonical.clone(), "personal:inbox").unwrap())
                .unwrap()
                .unwrap()
                .id(),
            &project.wave_id
        );
        let plan_id = |store: &SqliteStore| {
            store
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT personal_plan_id FROM waves WHERE id=?1",
                    [&project.wave_id],
                    |row| row.get::<_, String>(0),
                )
                .unwrap()
        };
        let original_plan = plan_id(&store);
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        assert_eq!(
            store
                .ensure_personal_project(&canonical.to_string(), "inbox")
                .unwrap(),
            project
        );
        assert_eq!(plan_id(&store), original_plan);
        assert_eq!(
            store.personal_wave_definition(&project.wave_id).unwrap(),
            Some(definition)
        );
        assert!(!repo.path().join("wave").exists());
        assert!(!repo.path().join(".lf").exists());
    }
}
