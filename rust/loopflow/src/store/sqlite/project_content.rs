//! Project content and stored Wave workflows share one transaction in every repository.

use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::durable::ProjectId;
use crate::id::WaveId;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

impl SqliteStore {
    pub(crate) fn wave_workflows(&self, wave: &WaveId) -> StoreResult<Vec<(String, String)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query =
            conn.prepare("SELECT name,content FROM wave_workflows WHERE wave_id=?1 ORDER BY name")?;
        let rows = query.query_map([wave], |row| Ok((row.get(0)?, row.get(1)?)))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
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
}

#[cfg(test)]
mod tests {
    use crate::id::WaveId;
    use crate::store::sqlite::SqliteStore;
    use crate::work::wave::Wave;

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
}
