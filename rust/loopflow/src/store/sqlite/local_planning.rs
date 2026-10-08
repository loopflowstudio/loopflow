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
            "SELECT body,acknowledged,conflicting_body IS NOT NULL FROM task_comment_deliveries WHERE comment_id=?1",
            [&comment.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if let Some((body, acknowledged, conflict)) = delivery {
        if body == comment.body {
            conn.execute(
                "UPDATE task_comment_deliveries SET acknowledged=1,error=NULL,conflicting_body=NULL WHERE comment_id=?1",
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
                "UPDATE task_comment_deliveries SET conflicting_body=?2 WHERE comment_id=?1",
                params![comment.id, comment.body],
            )?;
            conn.execute(
                "UPDATE task_comments SET provider_revision=?2 WHERE id=?1",
                params![comment.id, comment.revision],
            )?;
            return Ok(());
        }
    }
    let author =
        if comment.author_id.is_some() || crate::ops::linear_observe::is_steer(&comment.body) {
            crate::ops::pm::TaskCommentAuthor::Person {
                name: crate::ops::linear_observe::comment_requester(
                    &comment.body,
                    comment.author_name.as_deref(),
                ),
            }
        } else {
            crate::ops::pm::TaskCommentAuthor::Integration
        };
    conn.execute(
        "INSERT INTO task_comments(id,task_id,body,author,created_at,provider_revision)
         VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET body=excluded.body,
         author=excluded.author,created_at=excluded.created_at,provider_revision=excluded.provider_revision",
        params![comment.id, task.as_str(), comment.body, serde_json::to_string(&author)?,
            comment.created_at, comment.revision],
    )?;
    Ok(())
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
                 AND d.acknowledged=0 AND d.conflicting_body IS NULL))
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
            "INSERT INTO task_comment_deliveries(comment_id,body) VALUES(?1,?2)",
            params![comment.id, comment.body],
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
            "SELECT c.id,d.conflicting_body FROM task_comments c
            JOIN task_comment_deliveries d ON d.comment_id=c.id
            WHERE c.task_id=?1 AND d.conflicting_body IS NOT NULL",
        )?;
        let rows = query.query_map([task.as_str()], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<_, _>>().map_err(StoreError::from)
    }

    pub fn record_comment_delivery(&self, id: &str, error: Option<&str>) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // A late failed attempt cannot undo an acknowledgement from another reader.
        conn.execute(
            "UPDATE task_comment_deliveries SET acknowledged=?2,error=?3
             WHERE comment_id=?1 AND acknowledged=0",
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
