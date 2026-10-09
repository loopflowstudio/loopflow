//! Creation/link effects are keyed by their original Work identity.
//! The local projection is a separate foreign key; captured payloads and journal
//! mutations always retain the origin. Selection and correspondence live elsewhere.
use std::collections::BTreeSet;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::planning_changes::PlanningChanges;
use super::SqliteStore;
use crate::durable::{ProjectId, TaskId, WorkRef};
use crate::pm::{PmItem, PmProject, ProjectContent};
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

// Capture and peer validation use the same provider payload. Validation still
// compares the complete model and input, so unknown fields cannot travel.
fn task_creation_input(id: &str, team: &str, item: &PmItem) -> Value {
    json!({"id":id,"teamId":team,"projectId":item.project_id,"title":item.name,
        "description":item.description,"assigneeId":item.assignee})
}

fn project_creation_input(id: &str, team: &str, project: &PmProject) -> Value {
    json!({"id":id,"teamIds":[team],"name":project.name,"description":project.summary,
        "content":crate::pm::render_project_content(&ProjectContent {
            workflow:project.workflow.clone(),krs:project.krs.clone(),metric_targets:project.metric_targets.clone(),
        }),"useDefaultTemplate":false})
}

impl SqliteStore {
    pub(crate) fn planning_export_owners(&self, repo: &str) -> StoreResult<Vec<WorkRef>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn
            .prepare("SELECT DISTINCT kind,id FROM planning_exports WHERE repo=?1 ORDER BY kind")?;
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

    pub(crate) fn planning_export_origins(
        &self,
        owner: PlanningChanges<'_>,
    ) -> StoreResult<Vec<WorkRef>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (kind, id) = owner.owner();
        let mut query = conn.prepare(
            "SELECT origin_id FROM planning_exports WHERE kind=?1 AND id=?2 ORDER BY origin_id",
        )?;
        let rows = query.query_map(params![kind, id], |row| {
            let id: String = row.get(0)?;
            Ok(if kind == "task" {
                WorkRef::Task(TaskId::from_raw(id))
            } else {
                WorkRef::Project(ProjectId::from_raw(id))
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub(crate) fn planning_export_pending(&self, origin: PlanningChanges<'_>) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (kind, id) = origin.owner();
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM planning_exports WHERE kind=?1 AND origin_id=?2)",
            params![kind, id],
            |row| row.get(0),
        )?)
    }

    pub(crate) fn planning_export_attempts(
        &self,
        origin: PlanningChanges<'_>,
    ) -> StoreResult<(bool, bool)> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (kind, id) = origin.owner();
        conn.query_row(
            "SELECT export_attempted,export_link_attempted FROM planning_creations WHERE kind=?1 AND origin_id=?2 AND export_json IS NOT NULL",
            params![kind,id], |row| Ok((row.get(0)?,row.get(1)?)),
        ).optional()?.ok_or_else(|| StoreError::InvalidData("creation receipt is missing".into()))
    }

    // Capture before provider discovery; a concurrent local save cannot alter this effect.
    pub(crate) fn prepare_planning_export(
        &self,
        origin: PlanningChanges<'_>,
        team: &str,
        initiative: &str,
    ) -> StoreResult<PlanningExport> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (kind, id) = origin.owner();
        let retained: Option<String> = tx
            .query_row(
                "SELECT export_json FROM planning_creations WHERE kind=?1 AND origin_id=?2",
                params![kind, id],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        if let Some(retained) = retained {
            return Ok(serde_json::from_str(&retained)?);
        }
        let uuid = uuid::Uuid::parse_str(id.split_once('_').map_or(id, |(_, id)| id))
            .map_err(|error| StoreError::InvalidData(error.to_string()))?
            .to_string();
        let (model, input) = match origin {
            PlanningChanges::Task(id) => {
                let record = super::plan_read::task_in(&tx, id)?
                    .record
                    .ok_or(StoreError::NotFound)?;
                let external: Option<String> = tx.query_row(
                    "SELECT p.external_project_id FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
                    [id.as_str()], |row| row.get(0),
                )?;
                external.ok_or_else(|| {
                    StoreError::InvalidData("Task export awaits its Project mapping".into())
                })?;
                let item = record.item;
                let input = task_creation_input(&uuid, team, &item);
                (serde_json::to_value(item)?, input)
            }
            PlanningChanges::Project(id) => {
                let project = super::plan_read::project_in(&tx, id)?;
                let input = project_creation_input(&uuid, team, &project);
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
            "INSERT INTO planning_creations(kind,origin_id,task_id,project_id,export_json)
             VALUES(?1,?2,CASE WHEN ?1='task' THEN ?2 END,CASE WHEN ?1='project' THEN ?2 END,?3)
             ON CONFLICT(kind,origin_id) DO UPDATE SET export_json=excluded.export_json",
            params![kind, id, serde_json::to_string(&export)?],
        )?;
        tx.commit()?;
        Ok(export)
    }

    pub(crate) fn attempt_planning_export(
        &self,
        origin: PlanningChanges<'_>,
        input: &Value,
        link: bool,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        super::planning_peers::require_projected_effects(&tx, origin)?;
        let (kind, id) = origin.owner();
        let local: String = tx.query_row(
            "SELECT COALESCE(task_id,project_id) FROM planning_creations WHERE kind=?1 AND origin_id=?2",
            params![kind,id], |row| row.get(0))?;
        if local != id {
            match origin {
                PlanningChanges::Task(_) => super::planning_peers::require_projected_effects(
                    &tx,
                    PlanningChanges::Task(&TaskId::from_raw(local)),
                )?,
                PlanningChanges::Project(_) => super::planning_peers::require_projected_effects(
                    &tx,
                    PlanningChanges::Project(&ProjectId::from_raw(local)),
                )?,
            }
        }
        let column = if link {
            "export_link_attempted"
        } else {
            "export_attempted"
        };
        let deleted = if matches!(origin, PlanningChanges::Task(_)) {
            "AND EXISTS(SELECT 1 FROM tasks WHERE id=planning_creations.task_id AND planning_deleted_at IS NULL AND external_issue_id IS NULL)"
        } else if !link {
            "AND EXISTS(SELECT 1 FROM projects WHERE id=planning_creations.project_id AND external_project_id IS NULL)"
        } else {
            ""
        };
        let attempted = tx.execute(&format!("UPDATE planning_creations SET {column}=1,export_error=NULL,export_json=CASE WHEN ?3 THEN export_json ELSE json_set(export_json,'$.input',json(?2)) END
            WHERE origin_id=?1 AND kind=?4 AND {column}=0 AND export_acknowledged=0 {deleted}"), params![id,input.to_string(),link,kind])? == 1;
        tx.commit()?;
        Ok(attempted)
    }

    pub(crate) fn planning_export_error(
        &self,
        origin: PlanningChanges<'_>,
        error: &str,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (kind, id) = origin.owner();
        tx.execute(
            "UPDATE planning_creations SET export_error=?2 WHERE origin_id=?1 AND kind=?3 AND export_acknowledged=0 AND export_error IS NOT ?2",
            params![id, error,kind],
        )?;
        // Discovery can fail before capture (for example, a missing Initiative).
        // Preserve that diagnostic without inventing an attempted operation.
        tx.execute(
            &format!(
                "INSERT INTO planning_creations(kind,origin_id,{kind}_id,export_error)
             SELECT ?1,?2,id,?3 FROM {kind}s WHERE id=?2
             ON CONFLICT(kind,origin_id) DO NOTHING"
            ),
            params![kind, id, error],
        )?;
        tx.commit()?;
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
    let (kind, mapping, join) = if project {
        (
            "project",
            "external_project_id",
            "JOIN waves w ON w.id=o.wave_id",
        )
    } else {
        (
            "task",
            "external_issue_id",
            "JOIN projects p ON p.id=o.project_id JOIN waves w ON w.id=p.wave_id",
        )
    };
    let receipt: Option<(String, String, String)> = conn
        .query_row(
            &format!(
                "SELECT o.id,c.origin_id,c.export_json FROM {kind}s o
        {join} JOIN planning_creations c ON c.{kind}_id=o.id WHERE w.repo=?1
        AND c.kind=?3 AND (o.{mapping} IS NULL OR o.{mapping}=?2)
        AND c.export_attempted=1 AND json_extract(c.export_json,'$.id')=?2"
            ),
            params![repo, observed["id"].as_str(), kind],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    let Some((id, origin, receipt)) = receipt else {
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
            "UPDATE planning_creations SET export_acknowledged=1,export_error=NULL WHERE origin_id=?1 AND kind=?2 AND (export_acknowledged=0 OR export_error IS NOT NULL)",
            params![origin,kind],
        )?;
    }

    Ok(())
}

/// Portable creation evidence. Rotation, selection and activation do not travel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CreationReceipt {
    export: PlanningExport,
    attempted: bool,
    link_attempted: bool,
    error: Option<String>,
    acknowledged: bool,
}

pub(crate) fn validate_peer_receipt(
    object: &crate::engine::planning_exchange::PlanningObject,
    value: &Value,
) -> StoreResult<CreationReceipt> {
    use crate::engine::planning_exchange::PlanningKind;
    let receipt: CreationReceipt = serde_json::from_value(value.clone())?;
    let export = &receipt.export;
    let project = object.kind == PlanningKind::Project;
    let team = if project {
        export.input["teamIds"]
            .as_array()
            .filter(|ids| ids.len() == 1)
            .and_then(|ids| ids[0].as_str())
    } else {
        export.input["teamId"].as_str()
    }
    .filter(|team| !team.is_empty())
    .ok_or_else(|| StoreError::InvalidData("invalid planning creation receipt".into()))?;
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
            let item: PmItem = serde_json::from_value(export.model.clone())?;
            (
                serde_json::to_value(&item)?,
                task_creation_input(&expected, team, &item),
            )
        }
        PlanningKind::Project => {
            let project: PmProject = serde_json::from_value(export.model.clone())?;
            (
                serde_json::to_value(&project)?,
                project_creation_input(&expected, team, &project),
            )
        }
        _ => {
            return Err(StoreError::InvalidData(
                "creation requires a Task or Project".into(),
            ))
        }
    };
    let status = if project { "statusId" } else { "stateId" };
    if let Some(id) = export.input.get(status) {
        if !id.is_string() {
            return Err(StoreError::InvalidData("invalid creation state".into()));
        }
        input[status] = id.clone();
    }
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
        || !project && export.input["projectId"].as_str().is_none_or(str::is_empty)
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
    Ok(receipt)
}

pub(super) fn import_peer_receipts(
    conn: &Connection,
    object: &crate::engine::planning_exchange::PlanningObject,
    owner: PlanningChanges<'_>,
    winner: &CreationReceipt,
    history: &[CreationReceipt],
) -> StoreResult<()> {
    let mut receipt = winner.clone();
    let (kind, local) = owner.owner();
    if kind != object.kind.as_str() {
        return Err(StoreError::InvalidData(
            "creation origin and projection kinds differ".into(),
        ));
    }
    for value in history {
        merge_receipt(&mut receipt, value)?;
    }
    let mut query = conn.prepare(
        "SELECT COALESCE(task_id,project_id),export_json,export_attempted,
            export_link_attempted,export_error,export_acknowledged
         FROM planning_creations WHERE kind=?1 AND origin_id=?2",
    )?;
    let saved = query
        .query_and_then(params![kind, object.id], |row| -> StoreResult<_> {
            let export: Option<String> = row.get(1)?;
            let receipt = match export {
                Some(export) => Some(CreationReceipt {
                    export: serde_json::from_str(&export)?,
                    attempted: row.get(2)?,
                    link_attempted: row.get(3)?,
                    error: row.get(4)?,
                    acknowledged: row.get(5)?,
                }),
                None => None,
            };
            Ok((row.get::<_, String>(0)?, receipt))
        })?
        .next()
        .transpose()?;
    if let Some((projection, saved)) = saved {
        if projection != local {
            return Err(StoreError::PlanningReceiptConflict {
                effect: "creation ownership",
            });
        }
        if let Some(mut retained) = saved {
            merge_receipt(&mut retained, &receipt)?;
            receipt = retained;
        }
    }
    // Import retains provider effects, never a local Task-creation request.
    let export = serde_json::to_string(&receipt.export)?;
    conn.execute("INSERT INTO planning_creations(kind,origin_id,task_id,project_id,export_json,export_attempted,export_link_attempted,export_error,export_acknowledged)
        VALUES(?1,?2,CASE WHEN ?1='task' THEN ?3 END,CASE WHEN ?1='project' THEN ?3 END,?4,?5,?6,?7,?8)
        ON CONFLICT(kind,origin_id) DO UPDATE SET export_json=?4,export_attempted=?5,export_link_attempted=?6,export_error=?7,export_acknowledged=?8
        WHERE export_json IS NOT ?4 OR export_attempted IS NOT ?5 OR export_link_attempted IS NOT ?6 OR export_error IS NOT ?7 OR export_acknowledged IS NOT ?8",
        params![kind,object.id,local,export,receipt.attempted,receipt.link_attempted,receipt.error,receipt.acknowledged])?;
    Ok(())
}

fn merge_receipt(saved: &mut CreationReceipt, incoming: &CreationReceipt) -> StoreResult<()> {
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
        saved.export.input.clone_from(&incoming.export.input);
    }
    if (!saved.attempted && incoming.attempted)
        || (!saved.link_attempted && incoming.link_attempted)
    {
        saved.error.clone_from(&incoming.error);
    }
    saved.attempted |= incoming.attempted;
    saved.link_attempted |= incoming.link_attempted;
    saved.acknowledged |= incoming.acknowledged;
    if saved.acknowledged {
        saved.error = None;
    }
    Ok(())
}
