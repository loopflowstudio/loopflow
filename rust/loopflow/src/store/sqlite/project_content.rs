//! Project content and stored Wave workflows share one transaction in every repository.

use super::planning_write::PlanningEdit as Edit;
use rusqlite::{params, OptionalExtension, TransactionBehavior};

use crate::durable::ProjectId;
use crate::id::WaveId;
use crate::store::planning_sync::PlanningChange;
use crate::store::{StoreError, StoreResult};
use crate::work::project::ProjectKR;
use crate::work::wave::metrics::MetricTarget;

use super::planning_changes::PlanningChanges;
use super::SqliteStore;

impl SqliteStore {
    pub fn pending_project_changes(&self, project: &ProjectId) -> StoreResult<Vec<PlanningChange>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        PlanningChanges::Project(project).pending(&conn)
    }

    pub fn edit_project(
        &self,
        project: &ProjectId,
        name: Option<&str>,
        summary: Option<&str>,
        description: Option<&str>,
    ) -> StoreResult<()> {
        if name.is_some_and(|name| name.trim().is_empty()) {
            return Err(StoreError::InvalidData(
                "Project name cannot be empty".into(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let edits: Vec<_> = name
            .map(|name| Edit::ProjectName(Some(name.into())))
            .into_iter()
            .chain(summary.map(|summary| Edit::ProjectSummary(summary.into())))
            .chain(
                description
                    .map(|description| Edit::ProjectDescription(Some(description.trim().into()))),
            )
            .collect();
        PlanningChanges::Project(project).save(&tx, &edits)?;
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

    pub fn select_project_workflow(
        &self,
        project: &ProjectId,
        name: &str,
        definition: &str,
    ) -> StoreResult<()> {
        if name.trim().is_empty() || name.contains([':', '/', '\\']) {
            return Err(StoreError::InvalidData("invalid Workflow name".into()));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let repo: String = tx.query_row(
            "SELECT w.repo FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
            [project.as_str()],
            |row| row.get(0),
        )?;
        crate::workflow::parse_workflow(name, definition, std::path::Path::new(&repo))
            .map_err(StoreError::InvalidData)?;
        PlanningChanges::Project(project).save(&tx, &[Edit::Workflow(name.into())])?;
        tx.execute(
            "INSERT INTO wave_workflows(wave_id,name,content)
                SELECT wave_id,?2,?3 FROM projects WHERE id=?1
                ON CONFLICT(wave_id,name) DO UPDATE SET content=excluded.content
                WHERE content IS NOT excluded.content",
            params![project.as_str(), name, definition],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn update_project_content(
        &self,
        project: &ProjectId,
        workflow: &str,
        krs: &[ProjectKR],
        metric_targets: &[MetricTarget],
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        PlanningChanges::Project(project).save(
            &tx,
            &[
                Edit::Workflow(workflow.into()),
                Edit::Krs(krs.to_vec()),
                Edit::MetricTargets(metric_targets.to_vec()),
            ],
        )?;
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
    fn project_prose_unobserved_baseline_stays_unknown_at_the_same_revision() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let wave = Wave::new(WaveId::new(), "product".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        let mut observed: crate::pm::linear::LinearProject = serde_json::from_value(serde_json::json!({
            "id":"linear-project", "revision":"2026-10-10T12:00:00Z", "slug":"product", "name":"Product", "summary":"",
            "workflow":"", "krs":[], "metric_targets":[], "status":"started", "initiative_ids":["initiative"], "team_ids":["team"]
        })).unwrap();
        store
            .put_pm_project(wave.id(), "linear", "initiative", &observed, 1)
            .unwrap();
        let project = store.list_projects(Some(wave.id())).unwrap().remove(0);
        store
            .edit_project(&project.id, None, None, Some("Saved without a baseline"))
            .unwrap();
        let change = store
            .pending_project_changes(&project.id)
            .unwrap()
            .remove(0);
        assert_eq!(change.base, None);
        observed.description = Some(String::new());
        store
            .put_pm_project(wave.id(), "linear", "initiative", &observed, 2)
            .unwrap();
        assert_eq!(store.project(&project.id).unwrap().unwrap().description, "");
        let conn = store.conn.lock().unwrap();
        let (value, conflict): (String, String) = conn
            .query_row(
                "SELECT value_json,conflict_json FROM project_changes WHERE id=?1",
                [&change.id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<String>(&value).unwrap(),
            "Saved without a baseline"
        );
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&conflict).unwrap()["value"],
            ""
        );
    }

    #[test]
    fn project_prose_partial_observation_retains_its_original_baseline_and_age() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let wave = Wave::new(WaveId::new(), "product".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        let mut observed: crate::pm::linear::LinearProject = serde_json::from_value(serde_json::json!({
            "id":"linear-project", "revision":"2026-10-10T12:00:00Z", "slug":"product", "name":"Product", "summary":"", "description":"Baseline",
            "workflow":"", "krs":[], "metric_targets":[], "status":"started", "initiative_ids":["initiative"], "team_ids":["team"]
        })).unwrap();
        store
            .put_pm_project(wave.id(), "linear", "initiative", &observed, 17)
            .unwrap();
        let project = store.list_projects(Some(wave.id())).unwrap().remove(0);
        observed.description = None;
        observed.revision = Some("2026-10-10T12:01:00Z".into());
        observed.summary = "New summary".into();
        store
            .put_pm_project(wave.id(), "linear", "initiative", &observed, 29)
            .unwrap();
        let saved = store.project(&project.id).unwrap().unwrap();
        assert_eq!(saved.description, "Baseline");
        assert_eq!(saved.summary, "New summary");
        store
            .edit_project(&project.id, None, None, Some("Next save"))
            .unwrap();
        let change = store
            .pending_project_changes(&project.id)
            .unwrap()
            .remove(0);
        assert_eq!(
            change.base,
            Some(serde_json::json!({"value":"Baseline", "revision":"2026-10-10T12:00:00Z"}))
        );
        let prose: String = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT prose_json FROM pm_projects WHERE id='linear-project'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&prose).unwrap(),
            serde_json::json!({"value":"Baseline", "revision":"2026-10-10T12:00:00Z", "observed_at":17})
        );
        observed.description = Some("Baseline".into());
        store
            .put_pm_project(wave.id(), "linear", "initiative", &observed, 30)
            .unwrap();
        assert_eq!(
            store.project(&project.id).unwrap().unwrap().description,
            "Next save"
        );
        assert_eq!(
            store.pending_project_changes(&project.id).unwrap(),
            vec![change]
        );
    }

    #[test]
    fn project_edit_failure_rolls_back_fields_definitions_and_receipts() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let wave = Wave::new(WaveId::new(), "product".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        let project = crate::durable::ProjectId::new();
        let conn = store.conn.lock().unwrap();
        conn.execute("INSERT INTO projects(id,wave_id,created_at,updated_at,project_slug,project_name,description)
            VALUES(?1,?2,1,1,'product','Original','')",rusqlite::params![project.as_str(),wave.id()]).unwrap();
        conn.execute_batch(
            "CREATE TRIGGER fail_project_save BEFORE UPDATE ON projects
            BEGIN SELECT RAISE(ABORT,'injected write failure'); END;",
        )
        .unwrap();
        drop(conn);
        let before = store.revisions().unwrap();
        assert!(store
            .edit_project(&project, Some("Changed"), Some("New summary"), None)
            .is_err());
        assert!(store
            .select_project_workflow(
                &project,
                "review",
                "nodes: {}\nedges: [{from: start, to: end}]\n"
            )
            .is_err());
        let read = store.project(&project).unwrap().unwrap();
        assert_eq!(read.name, "Original");
        assert_eq!(read.summary, "");
        assert_eq!(read.workflow, "");
        assert!(store.pending_project_changes(&project).unwrap().is_empty());
        assert!(store.wave_workflow(wave.id(), "review").unwrap().is_none());
        assert_eq!(store.revisions().unwrap(), before);
    }

    #[test]
    fn stored_workflows_use_the_same_owner_with_and_without_linear_mapping() {
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&database).unwrap();
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
                 project_slug,project_name,description) VALUES(?1,?2,?3,1,1,?4,?4,'')",
                    rusqlite::params![project.as_str(), wave.id(), mapping, name],
                )
                .unwrap();
            store
                .select_project_workflow(&project, "review", definition)
                .unwrap();
            let before = store.revisions().unwrap();
            let changes = store.pending_project_changes(&project).unwrap();
            store
                .select_project_workflow(&project, "review", definition)
                .unwrap();
            store
                .update_project_content(&project, "review", &[], &[])
                .unwrap();
            store
                .edit_project(&project, Some(name), Some(""), None)
                .unwrap();
            assert_eq!(store.revisions().unwrap(), before);
            assert_eq!(store.pending_project_changes(&project).unwrap(), changes);
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
            assert_eq!(store.project(&project).unwrap().unwrap().workflow, "review");
            for (name, source) in [("invalid/review", definition), ("review", "changed")] {
                assert!(store
                    .select_project_workflow(&project, name, source)
                    .is_err());
            }
            let changed = "nodes: {}\nedges: [{from: start, to: end, flow: debug}]\n";
            store
                .select_project_workflow(&project, "review", changed)
                .unwrap();
            assert_eq!(
                store.wave_workflow(&wave, "review").unwrap().as_deref(),
                Some(changed)
            );
            assert_eq!(store.project(&project).unwrap().unwrap().workflow, "review");
        }
        assert!(store
            .update_project_content(&crate::durable::ProjectId::new(), "review", &[], &[])
            .is_err());
    }
}
