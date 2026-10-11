//! Project content and stored Wave workflows share one transaction in every repository.

use super::planning_write::{self, PlanningEdit as Edit};
use crate::planning_exchange::{PlanningKind, PlanningObject};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::durable::ProjectId;
use crate::id::WaveId;
use crate::planning::PlanningChange;
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
            .collect();
        write_edits(&tx, project, &edits)?;
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
        write_edits(&tx, project, &[Edit::Workflow(name.into())])?;
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
        write_edits(
            &tx,
            project,
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

/// Save the same typed values to local Work, Git history and Linear receipts.
pub(super) fn write_edits(
    conn: &Connection,
    project: &ProjectId,
    edits: &[Edit],
) -> StoreResult<()> {
    let object = PlanningObject {
        kind: PlanningKind::Project,
        id: project.to_string(),
    };
    let current = planning_write::read(conn, &object)?;
    for edit in edits {
        let field = super::planning_changes::linear_field(edit)
            .ok_or_else(|| StoreError::InvalidData("Project edit has no Linear field".into()))?;
        let previous = current
            .iter()
            .find(|previous| previous.field() == edit.field())
            .ok_or_else(|| {
                StoreError::InvalidData("Project edit belongs to another owner".into())
            })?;
        PlanningChanges::Project(project).record(conn, field, previous.value()?, edit.value()?)?;
    }
    planning_write::local(conn, PlanningKind::Project, project.as_str(), edits)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::id::WaveId;
    use crate::store::sqlite::SqliteStore;
    use crate::work::wave::Wave;

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
            .edit_project(&project, Some("Changed"), Some("New summary"))
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
            store.edit_project(&project, Some(name), Some("")).unwrap();
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
