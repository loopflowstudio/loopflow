//! Project content and stored Wave workflows share one transaction in every repository.

use super::planning_write::{self, PlanningEdit as Edit};
use crate::engine::planning_exchange::PlanningKind;
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::Value;

use crate::durable::ProjectId;
use crate::id::WaveId;
use crate::planning::PlanningChange;
use crate::pm::{PmProject, ProjectContent};
use crate::store::{StoreError, StoreResult};

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
        let current = super::plan_read::project_in(&tx, project)?;
        let mut changed = false;
        for (field, previous, value) in [
            ("name", current.name, name),
            ("summary", current.summary, summary),
        ] {
            if let Some(value) = value {
                changed |= PlanningChanges::Project(project).record(
                    &tx,
                    field,
                    Value::String(previous),
                    Value::String(value.into()),
                )?;
            }
        }
        if changed {
            let mut edits = Vec::new();
            if let Some(name) = name {
                edits.push(Edit::ProjectName(Some(name.into())));
            }
            if let Some(summary) = summary {
                edits.push(Edit::ProjectSummary(summary.into()));
            }
            planning_write::local(&tx, PlanningKind::Project, project.as_str(), &edits)?;
        }
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
        let current = super::plan_read::project_in(&tx, project)?;
        let repo: String = tx.query_row(
            "SELECT w.repo FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
            [project.as_str()],
            |row| row.get(0),
        )?;
        crate::engine::workflow::parse_workflow(name, definition, std::path::Path::new(&repo))
            .map_err(StoreError::InvalidData)?;
        write_content(
            &tx,
            project,
            &current,
            &ProjectContent {
                workflow: name.into(),
                krs: current.krs.clone(),
                metric_targets: current.metric_targets.clone(),
            },
        )?;
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
        content: &ProjectContent,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = super::plan_read::project_in(&tx, project)?;
        write_content(&tx, project, &current, content)?;
        tx.commit()?;
        Ok(())
    }
}

pub(super) fn write_content(
    conn: &Connection,
    project: &ProjectId,
    current: &PmProject,
    content: &ProjectContent,
) -> StoreResult<()> {
    for (field, previous, value) in [
        (
            "workflow",
            serde_json::to_value(&current.workflow)?,
            serde_json::to_value(&content.workflow)?,
        ),
        (
            "krs",
            serde_json::to_value(&current.krs)?,
            serde_json::to_value(&content.krs)?,
        ),
        (
            "metric_targets",
            serde_json::to_value(&current.metric_targets)?,
            serde_json::to_value(&content.metric_targets)?,
        ),
    ] {
        PlanningChanges::Project(project).record(conn, field, previous, value)?;
    }
    save_content(conn, project, content)
}

/// Persist the combined semantic winners without minting another delivery receipt.
/// Local edits and peer projection use the same content representation.
pub(super) fn save_content(
    conn: &Connection,
    project: &ProjectId,
    content: &ProjectContent,
) -> StoreResult<()> {
    planning_write::local(
        conn,
        PlanningKind::Project,
        project.as_str(),
        &[
            Edit::Workflow(content.workflow.clone()),
            Edit::Krs(content.krs.clone()),
            Edit::MetricTargets(content.metric_targets.clone()),
        ],
    )?;
    Ok(())
}

pub(super) fn read_content(conn: &Connection, project: &ProjectId) -> StoreResult<ProjectContent> {
    let (body, workflow): (String, String) = conn.query_row(
        "SELECT COALESCE(project_prompt_context,''),workflow FROM projects WHERE id=?1",
        [project.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let mut content = crate::pm::parse_project_content(&body)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    // Match the common reader: the saved selection owns workflow, not old prose.
    content.workflow = workflow;
    Ok(content)
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
        conn.execute("INSERT INTO projects(id,wave_id,created_at,updated_at,project_slug,project_name,project_prompt_context)
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
        let read = store.planning_project(&project).unwrap();
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
                .select_project_workflow(&project, &content.workflow, definition)
                .unwrap();
            let before = store.revisions().unwrap();
            let changes = store.pending_project_changes(&project).unwrap();
            store
                .select_project_workflow(&project, &content.workflow, definition)
                .unwrap();
            store.update_project_content(&project, &content).unwrap();
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
            assert_eq!(
                store.project(&project).unwrap().unwrap().plan.workflow,
                "review"
            );
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
            assert_eq!(
                store.project(&project).unwrap().unwrap().plan.workflow,
                "review"
            );
        }
        assert!(store
            .update_project_content(&crate::durable::ProjectId::new(), &content)
            .is_err());
    }
}
