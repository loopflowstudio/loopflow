//! Creation effects remain on the original receipt, including after response loss.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::planning_changes::PlanningChanges;
use super::SqliteStore;
use crate::durable::{ProjectId, TaskId, WorkRef};
use crate::store::{StoreError, StoreResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct PlanningExport {
    pub id: String,
    pub model: Value,
    pub through: i64,
    pub input: Value,
    pub initiative: String,
    pub link_id: String,
}

impl PlanningChanges<'_> {
    fn receipt(self) -> (&'static str, &'static str) {
        match self {
            Self::Task(_) => ("task_creation_intents", "task_id"),
            Self::Project(_) => ("project_transitions", "successor_id"),
        }
    }
}

impl SqliteStore {
    pub(crate) fn planning_export_owners(&self, repo: &str) -> StoreResult<Vec<WorkRef>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT 'project',p.id FROM projects p JOIN waves w ON w.id=p.wave_id
            JOIN project_transitions c ON c.successor_id=p.id AND c.wave_id=p.wave_id
            WHERE w.repo=?1 AND p.external_project_id IS NULL AND c.local_plan_json IS NOT NULL
            UNION ALL SELECT 'task',t.id FROM tasks t JOIN projects p ON p.id=t.project_id
            JOIN waves w ON w.id=p.wave_id JOIN task_creation_intents c ON c.task_id=t.id
            WHERE w.repo=?1 AND t.external_issue_id IS NULL
            AND (t.planning_deleted_at IS NULL OR c.export_attempted=1)",
        )?;
        let rows = query.query_map([repo], |row| {
            let kind: String = row.get(0)?;
            let id: String = row.get(1)?;
            Ok(if kind == "project" {
                WorkRef::Project(ProjectId::from_raw(id))
            } else {
                WorkRef::Task(TaskId::from_raw(id))
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub(crate) fn planning_export(
        &self,
        owner: PlanningChanges<'_>,
    ) -> StoreResult<Option<(PlanningExport, bool, bool)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (table, key) = owner.receipt();
        let link = if matches!(owner, PlanningChanges::Project(_)) {
            "export_link_attempted"
        } else {
            "0"
        };
        let row: Option<(String, bool, bool)> = conn.query_row(
            &format!("SELECT export_json,export_attempted,{link} FROM {table} WHERE {key}=?1 AND export_json IS NOT NULL"),
            [owner.owner().1], |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?)),
        ).optional()?;
        row.map(|(value, attempted, link)| Ok((serde_json::from_str(&value)?, attempted, link)))
            .transpose()
    }

    // Capture before provider discovery; a concurrent local save cannot alter this effect.
    pub(crate) fn prepare_planning_export(
        &self,
        owner: PlanningChanges<'_>,
        team: &str,
        initiative: &str,
    ) -> StoreResult<PlanningExport> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (kind, id) = owner.owner();
        let (table, key) = owner.receipt();
        let retained: Option<String> = tx.query_row(
            &format!("SELECT export_json FROM {table} WHERE {key}=?1"),
            [id],
            |row| row.get(0),
        )?;
        if let Some(retained) = retained {
            return Ok(serde_json::from_str(&retained)?);
        }
        let uuid = uuid::Uuid::parse_str(id.split_once('_').map_or(id, |(_, id)| id))
            .map_err(|error| StoreError::InvalidData(error.to_string()))?
            .to_string();
        let (model, input) = match owner {
            PlanningChanges::Task(id) => {
                let record = super::plan_read::task_in(&tx, id)?
                    .record
                    .ok_or(StoreError::NotFound)?;
                let project = record.project.ok_or(StoreError::NotFound)?;
                let external: Option<String> = tx.query_row("SELECT external_project_id FROM projects WHERE id=?1 OR external_project_id=?1", [&project.id], |row| row.get(0))?;
                let external = external.ok_or_else(|| {
                    StoreError::InvalidData("Task export awaits its Project mapping".into())
                })?;
                let item = record.item;
                let input = json!({"id":uuid,"teamId":team,"projectId":external,"title":item.name,
                    "description":item.description,"assigneeId":item.assignee});
                (serde_json::to_value(item)?, input)
            }
            PlanningChanges::Project(id) => {
                let project = super::plan_read::project_in(&tx, id)?;
                let input = json!({"id":uuid,"teamIds":[team],"name":project.name,"description":project.summary,
                    "content":crate::pm::render_project_content(&crate::pm::ProjectContent {
                        workflow:project.workflow.clone(),krs:project.krs.clone(),metric_targets:project.metric_targets.clone()
                    }),"useDefaultTemplate":false});
                (serde_json::to_value(project)?, input)
            }
        };
        let through = tx.query_row(
            &format!("SELECT COALESCE(max(seq),0) FROM {kind}_changes WHERE {kind}_id=?1"),
            [id],
            |row| row.get(0),
        )?;
        let export = PlanningExport {
            id: uuid,
            model,
            through,
            input,
            initiative: initiative.into(),
            link_id: uuid::Uuid::new_v4().to_string(),
        };
        tx.execute(
            &format!("UPDATE {table} SET export_json=?2 WHERE {key}=?1"),
            params![id, serde_json::to_string(&export)?],
        )?;
        tx.commit()?;
        Ok(export)
    }

    pub(crate) fn attempt_planning_export(
        &self,
        owner: PlanningChanges<'_>,
        input: &Value,
        link: bool,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (table, key) = owner.receipt();
        let column = if link {
            "export_link_attempted"
        } else {
            "export_attempted"
        };
        let deleted = if matches!(owner, PlanningChanges::Task(_)) {
            "AND EXISTS(SELECT 1 FROM tasks WHERE id=task_id AND planning_deleted_at IS NULL AND external_issue_id IS NULL)"
        } else {
            ""
        };
        Ok(conn.execute(&format!("UPDATE {table} SET {column}=1,export_error=NULL,export_json=CASE WHEN ?3 THEN export_json ELSE json_set(export_json,'$.input',json(?2)) END
            WHERE {key}=?1 AND {column}=0 AND export_json IS NOT NULL {deleted}"), params![owner.owner().1,input.to_string(),link])? == 1)
    }

    pub(crate) fn planning_export_error(
        &self,
        owner: PlanningChanges<'_>,
        error: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (table, key) = owner.receipt();
        conn.execute(
            &format!(
                "UPDATE {table} SET export_error=?2 WHERE {key}=?1 AND export_error IS NOT ?2"
            ),
            params![owner.owner().1, error],
        )?;
        Ok(())
    }
}

// Called by every ingestion path before it can allocate a new local identity.
// Only an attempted receipt can claim the exact generated UUID. Ordinary title matches do nothing.
pub(super) fn attach_in(
    conn: &Connection,
    repo: &str,
    project: bool,
    observed: &Value,
) -> StoreResult<()> {
    let (kind, table, key, mapping, join) = if project {
        (
            "project",
            "project_transitions",
            "successor_id",
            "external_project_id",
            "JOIN waves w ON w.id=o.wave_id",
        )
    } else {
        (
            "task",
            "task_creation_intents",
            "task_id",
            "external_issue_id",
            "JOIN projects p ON p.id=o.project_id JOIN waves w ON w.id=p.wave_id",
        )
    };
    let receipt: Option<(String, String)> = conn
        .query_row(
            &format!(
                "SELECT o.id,c.export_json FROM {kind}s o
        {join} JOIN {table} c ON c.{key}=o.id WHERE w.repo=?1 AND o.{mapping} IS NULL
        AND c.export_attempted=1 AND json_extract(c.export_json,'$.id')=?2"
            ),
            params![repo, observed["id"].as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((id, receipt)) = receipt else {
        return Ok(());
    };
    let export: PlanningExport = serde_json::from_str(&receipt)?;
    let task_id = TaskId::from_raw(&id);
    let project_id = ProjectId::from_raw(&id);
    let owner = if project {
        PlanningChanges::Project(&project_id)
    } else {
        PlanningChanges::Task(&task_id)
    };
    // Rebase only baseline-free edits. The creation snapshot, not its possibly changed
    // readback, supplies the baseline, so subsequent Linear edits still win conflicts.
    for (field, value) in export
        .model
        .as_object()
        .ok_or_else(|| StoreError::InvalidData("creation snapshot is not an object".into()))?
    {
        if field == "rank" {
            continue;
        }
        let value = owner.normalize(conn, field, value.clone())?;
        conn.execute(&format!("UPDATE {kind}_changes SET base_json=?3 WHERE {kind}_id=?1 AND field=?2 AND base_json IS NULL AND acknowledged=0 AND conflict_json IS NULL"),
            params![id,field,json!({"revision":observed["revision"],"value":value}).to_string()])?;
        if observed.get(field).is_some_and(|remote| {
            owner.normalize(conn, field, remote.clone()).ok().as_ref() == Some(&value)
        }) {
            conn.execute(&format!("UPDATE {kind}_changes SET acknowledged=1,acknowledged_revision=?4,error=NULL WHERE {kind}_id=?1 AND field=?2 AND seq<=?3 AND value_json=?5 AND conflict_json IS NULL"),
                params![id,field,export.through,observed["revision"].as_str(),value.to_string()])?;
        }
    }
    if !project {
        conn.execute("UPDATE task_state_deliveries SET base_state=?2,base_revision=?3 WHERE task_id=?1 AND base_state IS NULL AND attempted=0 AND settled=0",
            params![id,export.model["state"].as_str(),observed["revision"].as_str()])?;
        conn.execute("UPDATE task_changes SET base_json=?2 WHERE task_id=?1 AND field='deleted' AND base_json IS NULL",
            params![id,json!({"revision":observed["revision"],"value":false}).to_string()])?;
    }
    conn.execute(
        &format!("UPDATE {kind}s SET {mapping}=?2 WHERE id=?1"),
        params![id, export.id],
    )?;
    conn.execute(
        &format!("UPDATE {table} SET export_error=NULL WHERE {key}=?1"),
        [id],
    )?;
    Ok(())
}
