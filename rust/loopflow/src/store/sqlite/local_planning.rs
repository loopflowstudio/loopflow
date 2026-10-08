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
            for part in name.split('/') {
                let existing: Option<(WaveId, Option<String>)> = tx
                    .query_row(
                        "SELECT id,current_project_id FROM waves WHERE personal_plan_id=?1
                     AND name=?2 AND parent_wave_id IS ?3 AND retired_at IS NULL",
                        params![plan, part, parent],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?;
                let (wave, _) = match existing {
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
            }
            let wave = parent.expect("validated personal Wave has at least one component");
            tx.commit()?;
            drop(conn);
            self.ensure_project(&wave, name)?
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
            .create_task(&NewTask {
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
