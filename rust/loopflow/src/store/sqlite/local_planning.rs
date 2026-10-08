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
