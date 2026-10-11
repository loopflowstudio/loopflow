//! Project selection, membership and pending changes settle in one transaction.

use super::planning_changes::PlanningChanges;
use super::planning_write::{self, PlanningEdit as Edit};
use super::{durable, SqliteStore};
use crate::durable::ProjectId;
use crate::planning_exchange::PlanningKind;
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
            // Keep the released receipt shape; input fields no longer need a domain wrapper.
            // Compare JSON values so the original bytes need not be rewritten on retry.
            let original = serde_json::json!({
                "wave_id": input.wave_id, "successor_id": input.successor_id,
                "create": input.create, "project_name": input.project_name,
                "content": {"workflow": input.workflow, "krs": input.krs,
                            "metric_targets": input.metric_targets}
            });
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
                    || intent
                        .as_deref()
                        .map(serde_json::from_str::<serde_json::Value>)
                        .transpose()?
                        .as_ref()
                        != Some(&original)
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
                        Edit::ProjectStatus(crate::work::project::ProjectStatus::Started),
                        Edit::Workflow(input.workflow.clone()),
                        Edit::Krs(input.krs.clone()),
                        Edit::MetricTargets(input.metric_targets.clone()),
                        Edit::ProjectTeams(serde_json::from_str(&teams)?),
                        Edit::ProjectInitiatives(serde_json::from_str(&initiatives)?),
                    ],
                )?;
                durable::inherit_project_placement(&tx, &successor)?;
            } else {
                let available:bool=tx.query_row("SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1 AND wave_id=?2 AND status IN ('backlog','planned','started'))",params![successor.as_str(),input.wave_id],|r|r.get(0))?;
                if !available {
                    return Err(StoreError::InvalidAuthority(
                        "destination is no longer available".into(),
                    ));
                }
                PlanningChanges::Project(&successor).save(
                    &tx,
                    &[
                        Edit::ProjectName(Some(input.project_name.clone())),
                        Edit::ProjectStatus(crate::work::project::ProjectStatus::Started),
                        Edit::Workflow(input.workflow.clone()),
                        Edit::Krs(input.krs.clone()),
                        Edit::MetricTargets(input.metric_targets.clone()),
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
                    original.to_string()
                ],
            )?;
            for task in rotation
                .tasks
                .iter()
                .filter(|t| t.disposition == crate::ops::chapter::TaskDisposition::Move)
            {
                let task_id = &task.task.id;
                let member: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND project_id=?2)",
                    params![task_id.as_str(), predecessor],
                    |r| r.get(0),
                )?;
                if !member {
                    return Err(StoreError::InvalidAuthority(
                        "Task membership changed before rotation".into(),
                    ));
                }
                PlanningChanges::Task(task_id)
                    .save(&tx, &[Edit::TaskProject(successor.to_string())])?;
                tx.execute(
                    "INSERT INTO project_transition_items(wave_id,successor_id,issue_id)
                     VALUES(?1,?2,?3)",
                    params![input.wave_id, successor.as_str(), task_id.as_str()],
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
                PlanningChanges::Project(&id).save(
                    &tx,
                    &[Edit::ProjectStatus(
                        crate::work::project::ProjectStatus::Completed,
                    )],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}
