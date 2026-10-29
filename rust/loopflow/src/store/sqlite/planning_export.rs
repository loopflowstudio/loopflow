//! Creation effects remain on the original receipt, including after response loss.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::planning_changes::PlanningChanges;
use super::SqliteStore;
use crate::durable::{ProjectId, TaskId, WorkRef};
use crate::store::planning_sync::PlanningBaseline;
use crate::store::{StoreError, StoreResult};
use crate::work::edit::WorkValue;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct PlanningExport {
    pub id: String,
    pub baseline: Vec<WorkValue>,
    pub task_state: Option<String>,
    pub through: i64,
    pub input: Value,
    pub initiative: Option<String>,
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
            UNION ALL SELECT 'task',t.id FROM tasks t JOIN repository_plans w ON w.id=t.repository_id JOIN task_creation_intents c ON c.task_id=t.id
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
        initiative: Option<&str>,
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
        let (baseline, task_state, input) = match owner {
            PlanningChanges::Task(id) => {
                let task = super::children::task_on(&tx, id)?.ok_or(StoreError::NotFound)?;
                let external: Option<String> = tx.query_row(
                    "SELECT p.external_project_id FROM tasks t LEFT JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
                    [id.as_str()], |row| row.get(0),
                )?;
                if task.project_id.is_some() && external.is_none() {
                    return Err(StoreError::InvalidData(
                        "Task export awaits its Project mapping".into(),
                    ));
                }
                let input = json!({"id":uuid,"teamId":team,"projectId":external,"title":task.title,
                    "description":task.description,"assigneeId":task.assignee,"dueDate":task.due_date});
                (task_creation_baseline(&task), task.state.clone(), input)
            }
            PlanningChanges::Project(id) => {
                let project = super::children::project_in(&tx, id)?;
                let description = &project.description;
                let input = json!({"id":uuid,"teamIds":[team],"name":project.name,"description":project.summary,
                    "content":format!("{}\n{}", description, crate::linear::project_document::render_project_content(&project.krs, &project.metric_targets)),"useDefaultTemplate":false});
                (
                    vec![
                        WorkValue::ProjectName(Some(project.name)),
                        WorkValue::ProjectSummary(project.summary),
                        WorkValue::ProjectDescription(Some(project.description)),
                        WorkValue::ProjectStatus(project.status),
                        WorkValue::Krs(project.krs),
                        WorkValue::MetricTargets(project.metric_targets),
                    ],
                    None,
                    input,
                )
            }
        };
        let through = tx.query_row(
            &format!("SELECT COALESCE(max(seq),0) FROM {kind}_changes WHERE {kind}_id=?1"),
            [id],
            |row| row.get(0),
        )?;
        let export = PlanningExport {
            id: uuid,
            baseline,
            task_state,
            through,
            input,
            initiative: initiative.map(str::to_owned),
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
        retry_creation: bool,
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
            WHERE {key}=?1 AND ({column}=0 OR ?4) AND export_json IS NOT NULL {deleted}"), params![owner.owner().1,input.to_string(),link,retry_creation && !link])? == 1)
    }

    pub(crate) fn reject_planning_export(
        &self,
        owner: PlanningChanges<'_>,
        link: bool,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (table, key) = owner.receipt();
        let column = if link {
            "export_link_attempted"
        } else {
            "export_attempted"
        };
        conn.execute(
            &format!("UPDATE {table} SET {column}=0 WHERE {key}=?1 AND export_json IS NOT NULL"),
            [owner.owner().1],
        )?;
        Ok(())
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
pub(crate) fn attach_in(
    conn: &Connection,
    repo: &str,
    project: bool,
    external: &str,
    revision: Option<&str>,
    observed: &[crate::work::edit::WorkValue],
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
            params![repo, external],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((id, receipt)) = receipt else {
        return Ok(());
    };
    let export: PlanningExport = serde_json::from_str(&receipt)?;
    // The retained creation values, never current Work or changed readback, seed
    // later saves. The captured sequence boundary limits acknowledgments.
    for value in &export.baseline {
        let field = value.field();
        let baseline = PlanningBaseline {
            revision: revision.map(str::to_owned),
            value: Some(value.clone()),
        };
        conn.execute(&format!("UPDATE {kind}_changes SET base_json=?3 WHERE {kind}_id=?1 AND field=?2 AND base_json IS NULL AND acknowledged=0 AND conflict_json IS NULL"),
            params![id,field,serde_json::to_string(&baseline)?])?;
        if observed.contains(value) {
            let mut query = conn.prepare(&format!("SELECT id,value_json FROM {kind}_changes WHERE {kind}_id=?1 AND field=?2 AND seq<=?3 AND conflict_json IS NULL"))?;
            let changes = query
                .query_map(params![id, field, export.through], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (receipt, saved) in changes {
                if serde_json::from_str::<WorkValue>(&saved)? == *value {
                    conn.execute(&format!("UPDATE {kind}_changes SET acknowledged=1,acknowledged_revision=?2,error=NULL WHERE id=?1"),params![receipt,revision])?;
                }
            }
        }
    }
    if !project {
        conn.execute("UPDATE task_state_deliveries SET base_state=?2,base_revision=?3 WHERE task_id=?1 AND base_state IS NULL AND attempted=0 AND settled=0",
            params![id,export.task_state.as_deref(),revision])?;
        conn.execute("UPDATE task_changes SET base_json=?2 WHERE task_id=?1 AND field='deleted' AND base_json IS NULL",
            params![id,json!({"revision":revision,"value":false}).to_string()])?;
    }
    let edit = if project {
        crate::work::edit::WorkValue::ProjectLinearId(Some(export.id.clone()))
    } else {
        crate::work::edit::WorkValue::TaskLinearId(Some(export.id.clone()))
    };
    super::planning_write::local(conn, &crate::work::edit::WorkEdit::for_target(&id, &[edit]))?;
    conn.execute(
        &format!("UPDATE {table} SET export_error=NULL WHERE {key}=?1"),
        [id],
    )?;
    Ok(())
}

// Historical filing persisted intent before its network call, but recorded no
// attempt boundary. Conversion must preserve uncertainty, never guess no send.
pub(super) fn retain_filing_export_in(
    conn: &Connection,
    task: &TaskId,
    intent: &crate::store::sqlite::task_history::RetainedFiling,
) -> StoreResult<()> {
    let project: Option<String> = conn
        .query_row(
            "SELECT id FROM projects WHERE id=?1 OR external_project_id=?1",
            [&intent.project_id],
            |row| row.get(0),
        )
        .optional()?;
    let export = PlanningExport {
        id: intent.issue_id.clone(),
        // The historical intent is the attempted input. Today's Task may have
        // been edited or moved; it cannot supply this attempt's baseline.
        baseline: vec![
            WorkValue::TaskTitle(Some(intent.title.clone())),
            WorkValue::TaskDescription(Some(intent.notes.clone())),
            WorkValue::TaskDueDate(intent.due.clone()),
            WorkValue::TaskProject(Some(project.unwrap_or_else(|| intent.project_id.clone()))),
        ],
        task_state: None,
        through: 0,
        input: json!({"id":intent.issue_id,"teamId":intent.team_id,"projectId":intent.project_id,
            "stateId":intent.state_id,"title":intent.title,"description":intent.notes,"dueDate":intent.due}),
        initiative: None,
        link_id: intent.relation_id.clone(),
    };
    conn.execute(
        "UPDATE task_creation_intents SET export_json=?2,export_attempted=1,
        export_error='Historical creation outcome is unknown; awaiting exact provider readback'
        WHERE task_id=?1 AND export_json IS NULL",
        params![task.as_str(), serde_json::to_string(&export)?],
    )?;
    Ok(())
}

// Only creation fields can seed delivery baselines. Never persist the whole Task:
// checkout, Workflow and execution fields are not part of the external effect.
fn task_creation_baseline(task: &crate::work::task::Task) -> Vec<WorkValue> {
    vec![
        WorkValue::TaskTitle(Some(task.title.clone())),
        WorkValue::TaskDescription(Some(task.description.clone())),
        WorkValue::TaskAssignee(task.assignee.clone()),
        WorkValue::TaskDueDate(task.due_date.clone()),
        WorkValue::TaskProject(task.project_id.as_ref().map(ToString::to_string)),
    ]
}

impl PlanningExport {
    pub(crate) fn project_status(&self) -> StoreResult<crate::work::project::ProjectStatus> {
        self.baseline
            .iter()
            .find_map(|value| match value {
                WorkValue::ProjectStatus(value) => Some(*value),
                _ => None,
            })
            .ok_or_else(|| StoreError::InvalidData("creation Project status is missing".into()))
    }

    /// The created Project as an Asana or Notion mirror holds it. Those
    /// providers show its name and summary; status, KRs and targets stay the
    /// creation values Loopflow owns, so acceptance reads them from the receipt.
    pub(crate) fn mirrored_project(
        &self,
        team: &str,
        revision: Option<String>,
    ) -> StoreResult<crate::linear::LinearProject> {
        let mut project = crate::linear::LinearProject {
            id: self.id.clone(),
            revision,
            slug: String::new(),
            name: String::new(),
            summary: String::new(),
            description: None,
            metric_targets: Vec::new(),
            status: self.project_status()?,
            krs: Vec::new(),
            initiative_ids: vec![team.to_string()],
            team_ids: vec![team.to_string()],
        };
        for value in &self.baseline {
            match value {
                WorkValue::ProjectName(Some(name)) => {
                    project.slug = crate::linear::project_slug(name);
                    project.name = name.clone();
                }
                WorkValue::ProjectSummary(summary) => project.summary = summary.clone(),
                WorkValue::ProjectDescription(description) => {
                    project.description = description.clone()
                }
                WorkValue::Krs(krs) => project.krs = krs.clone(),
                WorkValue::MetricTargets(targets) => project.metric_targets = targets.clone(),
                _ => {}
            }
        }
        Ok(project)
    }
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};
    use serde_json::{json, Value};

    use super::{attach_in, retain_filing_export_in, PlanningExport};
    use crate::durable::{ProjectId, TaskId};
    use crate::store::migrations::{
        apply_before_migration, convert_released_data, migration_section_sql,
    };
    use crate::store::sqlite::planning_changes::PlanningChanges;
    use crate::store::sqlite::task_history::RetainedFiling;
    use crate::work::edit::WorkValue;

    #[test]
    fn historical_creation_keeps_intent_values_after_local_edits() {
        let conn = Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "planning_peers");
        conn.execute_batch(include_str!(
            "../../../tests/fixtures/migrations/creation_baselines.sql"
        ))
        .unwrap();
        let sql = migration_section_sql("planning_peers");
        conn.execute_batch(&sql).unwrap();
        convert_released_data(&conn, &sql).unwrap();
        conn.execute("UPDATE task_creation_intents SET export_json=NULL", [])
            .unwrap();
        let intent = RetainedFiling {
            key: "historical-key".into(),
            issue_id: "historical-issue".into(),
            relation_id: "historical-relation".into(),
            project_id: "project-external".into(),
            team_id: "historical-team".into(),
            state_id: Some("historical-state".into()),
            wave: "proof".into(),
            title: "Attempted title".into(),
            notes: "Attempted notes".into(),
            due: Some("2026-10-20".into()),
            existing: false,
        };
        for _ in 0..2 {
            retain_filing_export_in(&conn, &TaskId::from_raw("task-proof"), &intent).unwrap();
        }
        let (receipt, attempted, error): (String, bool, String) = conn
            .query_row(
                "SELECT export_json,export_attempted,export_error FROM task_creation_intents",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        let export: PlanningExport = serde_json::from_str(&receipt).unwrap();
        assert_eq!(
            export.input,
            json!({"id":"historical-issue","teamId":"historical-team",
            "projectId":"project-external","stateId":"historical-state","title":"Attempted title",
            "description":"Attempted notes","dueDate":"2026-10-20"})
        );
        assert!(export
            .baseline
            .contains(&WorkValue::TaskTitle(Some("Attempted title".into()))));
        assert!(export
            .baseline
            .contains(&WorkValue::TaskProject(Some("project-proof".into()))));
        assert_eq!(export.task_state, None); // A state UUID does not prove a semantic state.
        assert_eq!(export.link_id, "historical-relation");
        assert!(attempted);
        assert!(error.contains("unknown"));
        assert_eq!(
            conn.query_row(
                "SELECT issue_title FROM tasks WHERE id='task-proof'",
                [],
                |row| row.get::<_, String>(0)
            )
            .unwrap(),
            "Later Task"
        );
    }

    #[test]
    fn malformed_creation_baseline_keeps_released_receipts_and_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "planning_peers");
        conn.execute_batch(include_str!(
            "../../../tests/fixtures/migrations/creation_baselines.sql"
        ))
        .unwrap();
        conn.execute("UPDATE task_creation_intents SET export_json=json_set(export_json,'$.model.assignee',42)", []).unwrap();
        let receipt = |conn: &Connection| {
            conn.query_row("SELECT export_json FROM task_creation_intents", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap()
        };
        let before = receipt(&conn);
        let tx = conn.transaction().unwrap();
        let sql = migration_section_sql("planning_peers");
        tx.execute_batch(&sql).unwrap();
        assert!(convert_released_data(&tx, &sql).is_err());
        tx.rollback().unwrap();
        assert_eq!(receipt(&conn), before);
        assert!(conn.prepare("SELECT body FROM pm_items").is_ok());
        assert!(conn
            .prepare("SELECT mutation FROM planning_peer_changes")
            .is_err());
    }

    #[test]
    fn released_creation_retains_attempts_and_rebases_only_captured_values() {
        for project in [false, true] {
            for changed_readback in [false, true] {
                let conn = Connection::open_in_memory().unwrap();
                apply_before_migration(&conn, "planning_peers");
                conn.execute_batch(include_str!(
                    "../../../tests/fixtures/migrations/creation_baselines.sql"
                ))
                .unwrap();
                let (kind, table, key, original, later, external) = if project {
                    (
                        "project",
                        "project_transitions",
                        "successor_id",
                        "Original Project",
                        "Later Project",
                        "project-external",
                    )
                } else {
                    (
                        "task",
                        "task_creation_intents",
                        "task_id",
                        "Original Task",
                        "Later Task",
                        "task-external",
                    )
                };
                let id = format!("{kind}-proof");
                let receipt = || {
                    conn.query_row(
                        &format!("SELECT export_json FROM {table} WHERE {key}=?1"),
                        [&id],
                        |r| r.get::<_, String>(0),
                    )
                    .unwrap()
                };
                let before: Value = serde_json::from_str(&receipt()).unwrap();
                let sql = migration_section_sql("planning_peers");
                conn.execute_batch(&sql).unwrap();
                convert_released_data(&conn, &sql).unwrap();
                let after: Value = serde_json::from_str(&receipt()).unwrap();
                for key in ["id", "input", "through", "initiative", "link_id"] {
                    assert_eq!(after[key], before[key]);
                }
                assert!(after.get("model").is_none());
                let export: PlanningExport = serde_json::from_value(after).unwrap();
                let title = |value: &str| {
                    if project {
                        WorkValue::ProjectName(Some(value.into()))
                    } else {
                        WorkValue::TaskTitle(Some(value.into()))
                    }
                };
                assert!(export.baseline.contains(&title(original)));
                assert!(!export.baseline.contains(&title(later)));
                if !project {
                    assert!(export
                        .baseline
                        .contains(&WorkValue::TaskProject(Some("project-proof".into()))));
                    assert_eq!(export.task_state.as_deref(), Some("unstarted"));
                }
                conn.execute(
                    "UPDATE projects SET external_project_id=NULL WHERE id='project-proof'",
                    [],
                )
                .unwrap();
                let revision = "2026-10-11T00:00:00Z";
                let remote = if changed_readback {
                    "Provider changed"
                } else {
                    original
                };
                // Repeated acquisition cannot duplicate identity or settle the later save.
                for _ in 0..2 {
                    attach_in(
                        &conn,
                        "/repo",
                        project,
                        external,
                        Some(revision),
                        &[title(remote)],
                    )
                    .unwrap();
                }
                let base: Value = conn
                    .query_row(
                        &format!("SELECT base_json FROM {kind}_changes WHERE id=?1"),
                        [format!("later-{kind}")],
                        |row| row.get::<_, String>(0),
                    )
                    .map(|v| serde_json::from_str(&v).unwrap())
                    .unwrap();
                assert_eq!(base, json!({"revision":revision,"value":title(original)}));
                let ack = |name: &str| {
                    conn.query_row(
                        &format!("SELECT acknowledged FROM {kind}_changes WHERE id=?1"),
                        [format!("{name}-{kind}")],
                        |row| row.get::<_, bool>(0),
                    )
                    .unwrap()
                };
                assert_eq!(ack("original"), !changed_readback);
                assert!(!ack("later"));
                let task = TaskId::from_raw(&id);
                let project_id = ProjectId::from_raw(&id);
                let owner = if project {
                    PlanningChanges::Project(&project_id)
                } else {
                    PlanningChanges::Task(&task)
                };
                let mut edits = vec![title(remote)];
                owner
                    .reconcile_edits(&conn, Some(revision), &mut edits)
                    .unwrap();
                // The unsent later save is sent last, so it wins and rebases
                // onto any provider change it will replace.
                assert_eq!(edits, vec![title(later)]);
                let (conflict, base): (Option<String>, String) = conn
                    .query_row(
                        &format!("SELECT conflict_json,base_json FROM {kind}_changes WHERE id=?1"),
                        [format!("later-{kind}")],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .unwrap();
                assert!(conflict.is_none());
                assert_eq!(
                    serde_json::from_str::<Value>(&base).unwrap(),
                    json!({"revision":revision,"value":title(if changed_readback { remote } else { original })})
                );
                let attempted: bool = conn
                    .query_row(
                        &format!("SELECT export_attempted FROM {table} WHERE {key}=?1"),
                        [&id],
                        |row| row.get(0),
                    )
                    .unwrap();
                assert!(attempted);
                let retained: (bool, String) = conn
                    .query_row(
                        "SELECT attempted,error FROM task_changes WHERE id='conflicted-task'",
                        [],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .unwrap();
                assert_eq!(retained, (true, "Retained conflict".into()));
                assert!(conn
                    .query_row(
                        "SELECT acknowledged FROM task_changes WHERE id='acked-task'",
                        [],
                        |row| row.get::<_, bool>(0)
                    )
                    .unwrap());
                conn.execute(
                    &format!(
                        "INSERT INTO {kind}_changes(id,{kind}_id,value_json) VALUES('next',?1,?2)"
                    ),
                    params![id, serde_json::to_string(&title("Next")).unwrap()],
                )
                .unwrap();
                assert_eq!(
                    conn.query_row(
                        &format!("SELECT seq FROM {kind}_changes WHERE id='next'"),
                        [],
                        |row| row.get::<_, i64>(0)
                    )
                    .unwrap(),
                    43
                );
            }
        }
    }
}
