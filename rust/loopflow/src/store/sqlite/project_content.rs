//! Project content and stored Wave workflows share one transaction in every repository.

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde_json::Value;

use crate::durable::ProjectId;
use crate::id::WaveId;
use crate::planning::PlanningChange;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

fn record_change(
    conn: &Connection,
    project: &ProjectId,
    field: &str,
    previous: Value,
    value: Value,
) -> StoreResult<()> {
    if previous == value {
        return Ok(());
    }
    let body: Option<String> = conn
        .query_row(
            "SELECT o.body FROM projects p JOIN waves w ON w.id=p.wave_id
         JOIN pm_projects o ON o.id=p.external_project_id AND o.repo=w.repo AND o.provider='linear'
         WHERE p.id=?1",
            [project.as_str()],
            |row| row.get(0),
        )
        .optional()?;
    let base = body
        .map(|body| -> StoreResult<Value> {
            let body: Value = serde_json::from_str(&body)?;
            Ok(serde_json::json!({"revision": body["revision"], "value": body[field]}))
        })
        .transpose()?;
    conn.execute(
        "INSERT INTO project_changes(id,project_id,field,value_json,base_json)
         VALUES(?1,?2,?3,?4,?5)",
        params![
            uuid::Uuid::new_v4().to_string(),
            project.as_str(),
            field,
            value.to_string(),
            base.map(|v| v.to_string())
        ],
    )?;
    Ok(())
}

/// Provider reads retain conflicts; neither matching values nor clocks acknowledge writes.
pub(super) fn retain_edits(
    conn: &Connection,
    id: &ProjectId,
    observed: &crate::pm::PmProject,
) -> StoreResult<crate::pm::PmProject> {
    let mut saved = serde_json::to_value(observed)?;
    for change in pending_in(conn, id)? {
        let remote = saved[&change.field].clone();
        if remote != change.value
            && change
                .base
                .as_ref()
                .is_none_or(|base| base["value"] != remote)
        {
            conn.execute(
                "UPDATE project_changes SET conflict_json=?2 WHERE id=?1 AND conflict_json IS NULL",
                params![
                    change.id,
                    serde_json::json!({"revision":observed.revision,"value":remote}).to_string()
                ],
            )?;
        }
        saved[&change.field] = change.value;
    }
    Ok(serde_json::from_value(saved)?)
}

fn pending_in(conn: &Connection, project: &ProjectId) -> StoreResult<Vec<PlanningChange>> {
    let mut query = conn.prepare(
        "SELECT id,field,value_json,base_json,conflict_json FROM project_changes c
         WHERE project_id=?1 AND acknowledged=0 AND seq=(SELECT max(seq) FROM project_changes
             WHERE project_id=c.project_id AND field=c.field) ORDER BY seq",
    )?;
    let rows = query.query_map([project.as_str()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, Option<String>>(4)?,
        ))
    })?;
    rows.map(|row| {
        let (id, field, value, base, conflict) = row?;
        Ok(PlanningChange {
            id,
            field,
            value: serde_json::from_str(&value)?,
            base: base.map(|v| serde_json::from_str(&v)).transpose()?,
            conflict: conflict.map(|v| serde_json::from_str(&v)).transpose()?,
        })
    })
    .collect()
}

impl SqliteStore {
    pub(crate) fn project_with_changes(
        &self,
        project: &ProjectId,
    ) -> StoreResult<(crate::pm::PmProject, Vec<PlanningChange>)> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.unchecked_transaction()?;
        let result = (
            super::plan_read::project_in(&tx, project)?,
            pending_in(&tx, project)?,
        );
        tx.commit()?;
        Ok(result)
    }

    pub fn pending_project_changes(&self, project: &ProjectId) -> StoreResult<Vec<PlanningChange>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        pending_in(&conn, project)
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
        for (field, previous, value) in [
            ("name", current.name, name),
            ("summary", current.summary, summary),
        ] {
            if let Some(value) = value {
                record_change(
                    &tx,
                    project,
                    field,
                    Value::String(previous),
                    Value::String(value.into()),
                )?;
            }
        }
        tx.execute(
            "UPDATE projects SET project_name=COALESCE(?2,project_name),
             project_summary=COALESCE(?3,project_summary),updated_at=?4 WHERE id=?1",
            params![project.as_str(), name, summary, now_unix()],
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
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = super::plan_read::project_in(&tx, project)?;
        write_content(
            &tx,
            project,
            &crate::pm::ProjectContent {
                workflow: name.into(),
                krs: current.krs,
                metric_targets: current.metric_targets,
            },
            Some(definition),
        )?;
        tx.commit()?;
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
        write_content(&tx, project, content, workflow_definition)?;
        tx.commit()?;
        Ok(())
    }
}

fn write_content(
    conn: &Connection,
    project: &ProjectId,
    content: &crate::pm::ProjectContent,
    workflow_definition: Option<&str>,
) -> StoreResult<()> {
    let current = super::plan_read::project_in(conn, project)?;
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
        record_change(conn, project, field, previous, value)?;
    }
    if let Some(definition) = workflow_definition {
        let name = &content.workflow;
        if name.trim().is_empty() || name.contains([':', '/', '\\']) {
            return Err(StoreError::InvalidData("invalid Workflow name".into()));
        }
        conn.execute(
            "INSERT INTO wave_workflows(wave_id,name,content)
                SELECT wave_id,?2,?3 FROM projects WHERE id=?1
                ON CONFLICT(wave_id,name) DO UPDATE SET content=excluded.content",
            params![project.as_str(), name, definition],
        )?;
    }
    let changed = conn.execute(
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
