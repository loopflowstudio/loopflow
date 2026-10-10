//! Project selection, membership and pending changes settle in one transaction.

use super::planning_changes::PlanningChanges;
use super::planning_write::{self, PlanningEdit as Edit};
use super::{durable, SqliteStore};
use crate::durable::{ProjectId, TaskId};
use crate::engine::planning_exchange::PlanningKind;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use rusqlite::{params, OptionalExtension, TransactionBehavior};

impl SqliteStore {
    pub(crate) fn rotate_projects(
        &self,
        name: &str,
        entries: &[(
            crate::ops::chapter::WaveChapterPlan,
            crate::ops::chapter::WaveRotation,
        )],
    ) -> StoreResult<()> {
        if entries.is_empty() {
            return Ok(());
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for (input, rotation) in entries {
            let selected: Option<String> = tx.query_row(
                "SELECT current_project_id FROM waves WHERE id=?1",
                [&input.wave_id],
                |row| row.get(0),
            )?;
            let original = serde_json::to_string(input)?;
            let settled: Option<(Option<String>, bool, Option<String>)> = tx
                .query_row(
                    "SELECT reset_name, settled_at IS NOT NULL, local_plan_json
                     FROM project_transitions WHERE wave_id=?1 AND successor_id=?2",
                    params![input.wave_id, input.successor_id],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )
                .optional()?;
            if let Some((ref reset, true, ref intent)) = settled {
                if reset.as_deref() != Some(name)
                    || selected.as_deref() != Some(&input.successor_id)
                    || intent.as_deref() != Some(&original)
                {
                    return Err(StoreError::InvalidAuthority(
                        "Project selection or original rotation request changed".into(),
                    ));
                }
                continue;
            }
            if settled.is_some() || tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM project_transitions WHERE wave_id=?1 AND settled_at IS NULL)",
                [&input.wave_id], |row| row.get::<_, bool>(0),
            )? {
                return Err(StoreError::InvalidAuthority("an unfinished transition retains its original effects".into()));
            }
            let predecessor = rotation.predecessor.as_ref().map(|p| p.id.as_str());
            if selected.as_deref() != predecessor {
                return Err(StoreError::InvalidAuthority(
                    "Project selection changed before rotation".into(),
                ));
            }
            let now = now_unix();
            let successor = ProjectId::parse(&input.successor_id)
                .map_err(|error| StoreError::InvalidData(error.to_string()))?;
            if input.create {
                let (teams,initiatives):(String,String)=tx.query_row("SELECT COALESCE((SELECT planning_teams FROM projects WHERE id=?1),'[]'),COALESCE((SELECT planning_initiatives FROM projects WHERE id=?1),'[]')",[predecessor],|r|Ok((r.get(0)?,r.get(1)?)))?;
                planning_write::create(
                    &tx,
                    "",
                    PlanningKind::Project,
                    successor.as_str(),
                    &[
                        Edit::ProjectWave(input.wave_id.to_string()),
                        Edit::ProjectSlug(Some(input.project_name.clone())),
                        Edit::ProjectName(Some(input.project_name.clone())),
                        Edit::ProjectStatus(crate::pm::ProjectStatus::Started),
                        Edit::Workflow(input.content.workflow.clone()),
                        Edit::Krs(input.content.krs.clone()),
                        Edit::MetricTargets(input.content.metric_targets.clone()),
                        Edit::ProjectTeams(teams),
                        Edit::ProjectInitiatives(initiatives),
                    ],
                )?;
                durable::inherit_project_placement(&tx, &successor)?;
            } else {
                let previous = super::plan_read::project_in(&tx, &successor)?;
                PlanningChanges::Project(&successor).record(
                    &tx,
                    "name",
                    serde_json::json!(previous.name),
                    serde_json::json!(input.project_name),
                )?;
                PlanningChanges::Project(&successor).record(
                    &tx,
                    "status",
                    serde_json::to_value(previous.status)?,
                    serde_json::json!("started"),
                )?;
                super::project_content::write_content(&tx, &successor, &previous, &input.content)?;
                let available:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1 AND wave_id=?2 AND status IN ('backlog','planned','started'))",params![successor.as_str(),input.wave_id],|r|r.get(0))?;
                if !available {
                    return Err(StoreError::InvalidAuthority(
                        "destination is no longer available".into(),
                    ));
                }
                planning_write::local(
                    &tx,
                    PlanningKind::Project,
                    successor.as_str(),
                    &[
                        Edit::ProjectName(Some(input.project_name.clone())),
                        Edit::ProjectStatus(crate::pm::ProjectStatus::Started),
                    ],
                )?;
            }
            tx.execute(
                "INSERT INTO project_transitions(wave_id,successor_id,predecessor_id,reset_name,
                     create_successor,created_at,settled_at,local_plan_json)
                 VALUES(?1,?2,?3,?4,?5,?6,?6,?7)",
                params![
                    input.wave_id,
                    successor.as_str(),
                    predecessor,
                    name,
                    input.create,
                    now,
                    original
                ],
            )?;
            for task in rotation
                .tasks
                .iter()
                .filter(|t| t.disposition == crate::ops::chapter::TaskDisposition::Move)
            {
                let task_id = TaskId::parse(&task.task.id)
                    .map_err(|e| StoreError::InvalidData(e.to_string()))?;
                let previous = super::plan_read::task_in(&tx, &task_id)?
                    .record
                    .ok_or(StoreError::NotFound)?
                    .item;
                PlanningChanges::Task(&task_id).record(
                    &tx,
                    "project_id",
                    serde_json::to_value(previous.project_id)?,
                    serde_json::json!(successor),
                )?;
                let member: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND project_id=?2)",
                    params![task.task.id, predecessor],
                    |r| r.get(0),
                )?;
                if !member {
                    return Err(StoreError::InvalidAuthority(
                        "Task membership changed before rotation".into(),
                    ));
                }
                planning_write::local(
                    &tx,
                    PlanningKind::Task,
                    &task.task.id,
                    &[Edit::TaskProject(successor.to_string())],
                )?;
                tx.execute(
                    "INSERT INTO project_transition_items(wave_id,successor_id,issue_id)
                     VALUES(?1,?2,?3)",
                    params![input.wave_id, successor.as_str(), task.task.id],
                )?;
            }
            planning_write::local(
                &tx,
                PlanningKind::Wave,
                input.wave_id.as_str(),
                &[Edit::WaveProject(Some(successor.to_string()))],
            )?;
            if let Some(predecessor) = predecessor {
                let id = ProjectId::from_raw(predecessor);
                let previous = super::plan_read::project_in(&tx, &id)?;
                PlanningChanges::Project(&id).record(
                    &tx,
                    "status",
                    serde_json::to_value(previous.status)?,
                    serde_json::json!("completed"),
                )?;
                planning_write::local(
                    &tx,
                    PlanningKind::Project,
                    predecessor,
                    &[Edit::ProjectStatus(crate::pm::ProjectStatus::Completed)],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}
