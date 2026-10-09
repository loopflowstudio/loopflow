//! Creation effects remain on the original receipt, including after response loss.
use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::planning_changes::PlanningChanges;
use super::SqliteStore;
use crate::durable::{ProjectId, TaskId, WorkRef};
use crate::store::{StoreError, StoreResult};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlanningExport {
    pub id: String,
    pub model: Value,
    pub parent: String,
    pub captured: BTreeSet<String>,
    pub input: Value,
    pub initiative: String,
    pub link_id: String,
}

impl PlanningChanges<'_> {
    fn receipt(self) -> (&'static str, &'static str) {
        match self {
            Self::Task(_) => ("task_creation_intents", "task_id"),
            Self::Project(_) => ("projects", "id"),
        }
    }
}

impl SqliteStore {
    pub(crate) fn planning_export_owners(&self, repo: &str) -> StoreResult<Vec<WorkRef>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query =
            conn.prepare("SELECT kind,id FROM planning_exports WHERE repo=?1 ORDER BY kind")?;
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

    pub(crate) fn planning_export_pending(&self, owner: PlanningChanges<'_>) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (kind, id) = owner.owner();
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM planning_exports WHERE kind=?1 AND id=?2)",
            params![kind, id],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn planning_export_attempts(
        &self,
        owner: PlanningChanges<'_>,
    ) -> StoreResult<(bool, bool)> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (table, key) = owner.receipt();
        let link = if matches!(owner, PlanningChanges::Project(_)) {
            "export_link_attempted"
        } else {
            "0"
        };
        conn.query_row(
            &format!("SELECT export_attempted,{link} FROM {table} WHERE {key}=?1 AND export_json IS NOT NULL"),
            [owner.owner().1], |row| Ok((row.get(0)?,row.get(1)?)),
        ).optional()?.ok_or_else(|| StoreError::InvalidData("creation receipt is missing".into()))
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
        if let PlanningChanges::Task(id) = owner {
            tx.execute(
                "INSERT INTO task_creation_intents(task_id,project_id,title,description)
                 SELECT id,project_id,issue_title,COALESCE(issue_description,'') FROM tasks WHERE id=?1 AND external_issue_id IS NULL
                 ON CONFLICT(task_id) DO NOTHING", [id.as_str()],
            )?;
        }
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
                let external: Option<String> = tx.query_row(
                    "SELECT p.external_project_id FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
                    [id.as_str()], |row| row.get(0),
                )?;
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
        // Local sequence numbers cannot identify captured saves on another machine.
        let mut captured = {
            let mut query =
                tx.prepare(&format!("SELECT id FROM {kind}_changes WHERE {kind}_id=?1"))?;
            let ids = query
                .query_map([id], |row| row.get::<_, String>(0))?
                .collect::<Result<BTreeSet<_>, _>>()?;
            ids
        };
        {
            let peer_kind = if kind == "task" {
                crate::engine::planning_exchange::PlanningKind::Task
            } else {
                crate::engine::planning_exchange::PlanningKind::Project
            };
            let mut query = tx.prepare(
                "SELECT id,field FROM planning_peer_changes WHERE kind=?1 AND object_id=?2",
            )?;
            for row in query.query_map(params![kind, id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })? {
                let (mutation, field) = row?;
                if let Some(field) = super::planning_peers::delivery_field(peer_kind, &field) {
                    captured.insert(format!("peer:{mutation}:{field}"));
                }
            }
        }
        let parent = tx.query_row(
            if kind == "task" {
                "SELECT project_id FROM tasks WHERE id=?1"
            } else {
                "SELECT wave_id FROM projects WHERE id=?1"
            },
            [id],
            |row| row.get(0),
        )?;
        let export = PlanningExport {
            id: uuid,
            parent,
            model,
            captured,
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
        } else if !link {
            "AND external_project_id IS NULL"
        } else {
            ""
        };
        Ok(conn.execute(&format!("UPDATE {table} SET {column}=1,export_error=NULL,export_json=CASE WHEN ?3 THEN export_json ELSE json_set(export_json,'$.input',json(?2)) END
            WHERE {key}=?1 AND {column}=0 AND export_json IS NOT NULL AND export_acknowledged=0 {deleted}"), params![owner.owner().1,input.to_string(),link])? == 1)
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
                "UPDATE {table} SET export_error=?2 WHERE {key}=?1 AND export_acknowledged=0 AND export_error IS NOT ?2"
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
            "projects",
            "id",
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
        {join} JOIN {table} c ON c.{key}=o.id WHERE w.repo=?1
        AND (o.{mapping} IS NULL OR o.{mapping}=?2)
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
    {
        let captured = serde_json::to_string(&export.captured)?;
        let mut rebase = conn.prepare(&format!("UPDATE {kind}_changes SET base_json=?3 WHERE {kind}_id=?1 AND field=?2 AND base_json IS NULL AND acknowledged=0 AND conflict_json IS NULL"))?;
        let mut acknowledge = conn.prepare(&format!("UPDATE {kind}_changes SET acknowledged=1,acknowledged_revision=?4,error=NULL WHERE {kind}_id=?1 AND field=?2 AND id IN (SELECT value FROM json_each(?3)) AND value_json=?5 AND acknowledged=0 AND conflict_json IS NULL"))?;
        for (field, value) in export
            .model
            .as_object()
            .ok_or_else(|| StoreError::InvalidData("creation snapshot is not an object".into()))?
        {
            if field == "rank" {
                continue;
            }
            let value = owner.normalize(conn, field, value.clone())?;
            rebase.execute(params![
                id,
                field,
                json!({"revision":observed["revision"],"value":value}).to_string()
            ])?;
            if observed.get(field).is_some_and(|remote| {
                owner.normalize(conn, field, remote.clone()).ok().as_ref() == Some(&value)
            }) {
                acknowledge.execute(params![
                    id,
                    field,
                    captured,
                    observed["revision"].as_str(),
                    value.to_string()
                ])?;
            }
        }
    }
    if !project {
        conn.execute("UPDATE task_state_deliveries SET base_state=?2,base_revision=?3 WHERE task_id=?1 AND base_state IS NULL AND attempted=0 AND settled=0",
            params![id,export.model["state"].as_str(),observed["revision"].as_str()])?;
        conn.execute("UPDATE task_changes SET base_json=?2 WHERE task_id=?1 AND field='deleted' AND base_json IS NULL",
            params![id,json!({"revision":observed["revision"],"value":false}).to_string()])?;
    }
    conn.execute(
        &format!("UPDATE {kind}s SET {mapping}=?2 WHERE id=?1 AND {mapping} IS NOT ?2"),
        params![id, export.id],
    )?;
    // An entity read proves creation, not an uncertain Initiative attachment.
    // Later accepted membership moves remain legal; never freeze it to creation.
    let link_observed = observed["initiative_ids"]
        .as_array()
        .is_some_and(|ids| ids.iter().any(|id| id.as_str() == Some(&export.initiative)));
    if !project || link_observed {
        conn.execute(
            &format!("UPDATE {table} SET export_acknowledged=1,export_error=NULL WHERE {key}=?1 AND (export_acknowledged=0 OR export_error IS NOT NULL)"),
            [id],
        )?;
    }

    Ok(())
}

/// Portable creation evidence. Rotation, selection and activation do not travel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CreationReceipt {
    export: PlanningExport,
    attempted: bool,
    link_attempted: bool,
    error: Option<String>,
    acknowledged: bool,
}

pub(crate) fn validate_peer_receipt(
    object: &crate::engine::planning_exchange::PlanningObject,
    value: &Value,
) -> StoreResult<()> {
    use crate::engine::planning_exchange::PlanningKind;
    let receipt: CreationReceipt = serde_json::from_value(value.clone())?;
    let export = &receipt.export;
    let expected = uuid::Uuid::parse_str(
        object
            .id
            .split_once('_')
            .map_or(object.id.as_str(), |(_, id)| id),
    )
    .map_err(|e| StoreError::InvalidData(e.to_string()))?
    .to_string();
    let (model, mut input) = match object.kind {
        PlanningKind::Task => {
            let item: crate::pm::PmItem = serde_json::from_value(export.model.clone())?;
            (
                serde_json::to_value(&item)?,
                json!({"id":expected,"teamId":export.input["teamId"],
                "projectId":item.project_id,"title":item.name,"description":item.description,"assigneeId":item.assignee}),
            )
        }
        PlanningKind::Project => {
            let project: crate::pm::PmProject = serde_json::from_value(export.model.clone())?;
            (
                serde_json::to_value(&project)?,
                json!({"id":expected,"teamIds":export.input["teamIds"],
                "name":project.name,"description":project.summary,"content":crate::pm::render_project_content(&crate::pm::ProjectContent {
                    workflow:project.workflow,krs:project.krs,metric_targets:project.metric_targets,
                }),"useDefaultTemplate":false}),
            )
        }
        _ => {
            return Err(StoreError::InvalidData(
                "creation requires a Task or Project".into(),
            ))
        }
    };
    let project = object.kind == PlanningKind::Project;
    let status = if project { "statusId" } else { "stateId" };
    if let Some(id) = export.input.get(status) {
        if !id.is_string() {
            return Err(StoreError::InvalidData("invalid creation state".into()));
        }
        input[status] = id.clone();
    }
    let team_valid = if project {
        export.input["teamIds"]
            .as_array()
            .is_some_and(|ids| ids.len() == 1 && ids[0].as_str().is_some_and(|s| !s.is_empty()))
    } else {
        export.input["teamId"]
            .as_str()
            .is_some_and(|s| !s.is_empty())
            && export.input["projectId"]
                .as_str()
                .is_some_and(|s| !s.is_empty())
    };
    let parent_valid = if project {
        crate::id::WaveId::parse(&export.parent).is_ok()
    } else {
        ProjectId::parse(&export.parent).is_ok()
    };
    if !parent_valid
        || export.id != expected
        || export.model["id"].as_str() != Some(object.id.as_str())
        || model != export.model
        || input != export.input
        || !team_valid
        || uuid::Uuid::parse_str(&export.link_id).is_err()
        || export.initiative.is_empty()
        || receipt.link_attempted && (!project || !receipt.attempted)
        || receipt.acknowledged && !receipt.attempted
        || export.captured.iter().any(String::is_empty)
    {
        return Err(StoreError::InvalidData(
            "invalid planning creation receipt".into(),
        ));
    }
    Ok(())
}

pub(super) fn import_peer_receipts(
    conn: &Connection,
    object: &crate::engine::planning_exchange::PlanningObject,
    winner: &Value,
    history: &[&Value],
) -> StoreResult<()> {
    use crate::engine::planning_exchange::PlanningKind;
    let mut receipt: CreationReceipt = serde_json::from_value(winner.clone())?;
    for &value in history {
        merge_receipt(&mut receipt, serde_json::from_value(value.clone())?)?;
    }
    let task = TaskId::from_raw(&object.id);
    let project = ProjectId::from_raw(&object.id);
    let owner = match object.kind {
        PlanningKind::Task => PlanningChanges::Task(&task),
        PlanningKind::Project => PlanningChanges::Project(&project),
        _ => return Ok(()),
    };
    let (table, key) = owner.receipt();
    let link = if object.kind == PlanningKind::Project {
        "export_link_attempted"
    } else {
        "0"
    };
    let saved: Option<String> = conn.query_row(
        &format!("SELECT json_object('export',json(export_json),'attempted',json(CASE WHEN export_attempted THEN 'true' ELSE 'false' END),
            'link_attempted',json(CASE WHEN {link} THEN 'true' ELSE 'false' END),'error',export_error,
            'acknowledged',json(CASE WHEN export_acknowledged THEN 'true' ELSE 'false' END))
            FROM {table} WHERE {key}=?1 AND export_json IS NOT NULL"),
        [&object.id], |r| r.get(0),
    ).optional()?;
    if let Some(saved) = saved {
        let mut local = serde_json::from_str(&saved)?;
        merge_receipt(&mut local, receipt)?;
        receipt = local;
    }
    if let PlanningChanges::Task(_) = owner {
        conn.execute(
            "INSERT INTO task_creation_intents(task_id,project_id,title,description)
             VALUES(?1,?2,?3,?4) ON CONFLICT(task_id) DO NOTHING",
            params![
                object.id,
                receipt.export.parent,
                receipt.export.model["name"].as_str(),
                receipt.export.model["description"].as_str()
            ],
        )?;
    }
    let export = serde_json::to_string(&receipt.export)?;
    conn.execute(&format!("UPDATE {table} SET export_json=?2,export_attempted=?3,export_error=?4,export_acknowledged=?5
        WHERE {key}=?1 AND (export_json IS NOT ?2 OR export_attempted IS NOT ?3 OR export_error IS NOT ?4 OR export_acknowledged IS NOT ?5)"),
        params![object.id,export,receipt.attempted,receipt.error,receipt.acknowledged])?;
    if object.kind == PlanningKind::Project {
        conn.execute("UPDATE projects SET export_link_attempted=?2 WHERE id=?1 AND export_link_attempted IS NOT ?2",
            params![object.id,receipt.link_attempted])?;
    }
    Ok(())
}

fn merge_receipt(saved: &mut CreationReceipt, incoming: CreationReceipt) -> StoreResult<()> {
    // No clock can discard a competing effect. Keep both in the journal and
    // isolate projection until that ambiguity is resolved.
    let mut before = saved.export.clone();
    let mut after = incoming.export.clone();
    before.input = Value::Null;
    after.input = Value::Null;
    if before != after
        || saved.attempted && incoming.attempted && saved.export.input != incoming.export.input
    {
        return Err(StoreError::PlanningReceiptConflict { effect: "creation" });
    }
    if !saved.attempted && incoming.attempted {
        saved.export.input = incoming.export.input;
    }
    if (!saved.attempted && incoming.attempted)
        || (!saved.link_attempted && incoming.link_attempted)
    {
        saved.error = incoming.error;
    }
    saved.attempted |= incoming.attempted;
    saved.link_attempted |= incoming.link_attempted;
    saved.acknowledged |= incoming.acknowledged;
    if saved.acknowledged {
        saved.error = None;
    }
    Ok(())
}
