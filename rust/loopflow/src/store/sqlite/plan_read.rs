//! Read saved planning independently of Workflow position and provider availability.

use rusqlite::{Connection, OptionalExtension};

use crate::durable::{ProjectId, TaskId};
use crate::pm::linear::LinearProject;
use crate::pm::PmItem;
use crate::store::{PlanningState, PmTaskObservation, PmTaskRecord, StoreError, StoreResult};

use super::children::TASK_IDENTIFIER;
use super::SqliteStore;

impl SqliteStore {
    pub fn planning_task(&self, id: &TaskId) -> StoreResult<PmTaskObservation> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.unchecked_transaction()?;
        let result = task_in(&tx, id)?;
        tx.commit()?;
        Ok(result)
    }
}

pub(super) fn project_in(conn: &Connection, id: &ProjectId) -> StoreResult<LinearProject> {
    let (mut project, krs, targets, initiatives, teams): (LinearProject, String, String, String, String) = conn.query_row(
        "SELECT COALESCE(p.external_project_id,p.id),p.project_slug,p.project_name,p.project_summary,
            p.krs,p.workflow,p.status,p.planning_provider_revision,
            p.planning_initiatives,p.planning_teams,p.metric_targets
         FROM projects p
         WHERE p.id=?1",
        [id.as_str()], |row| Ok((LinearProject {
            description: None,
            id: row.get(0)?, slug: row.get(1)?, name: row.get(2)?, summary: row.get(3)?,
            workflow: row.get(5)?, status: serde_json::from_value(serde_json::Value::String(row.get(6)?))
                .map_err(|error| rusqlite::Error::FromSqlConversionFailure(6,rusqlite::types::Type::Text,Box::new(error)))?, revision: row.get(7)?,
            krs: Vec::new(), metric_targets: Vec::new(), initiative_ids: Vec::new(), team_ids: Vec::new(),
        },row.get(4)?,row.get(10)?,row.get(8)?,row.get(9)?)),
    )?;
    project.krs = serde_json::from_str(&krs)?;
    project.metric_targets = serde_json::from_str(&targets)?;
    project.initiative_ids = serde_json::from_str(&initiatives)?;
    project.team_ids = serde_json::from_str(&teams)?;
    Ok(project)
}

pub(super) fn task_in(conn: &Connection, id: &TaskId) -> StoreResult<PmTaskObservation> {
    let (mut item, observed_at, repo, project_id, issue, deleted):
        (PmItem, i64, String, String, Option<String>, bool) = conn.query_row(
        &format!("SELECT t.planning_provider_revision,COALESCE(t.external_issue_id,t.id),{TASK_IDENTIFIER},
            planning_branch_name,planning_url,issue_title,issue_description,t.planning_rank,
            planning_completed,planning_completed_at,planning_state,planning_team_id,planning_assignee,
            COALESCE(t.pm_snapshot_synced_at,t.updated_at,t.created_at),w.repo,t.project_id,
            t.external_issue_id,t.planning_deleted_at IS NOT NULL,t.planning_due_date
         FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id WHERE t.id=?1"),
        [id.as_str()], |row| Ok((PmItem {
            revision: row.get(0)?, id: row.get(1)?, identifier: row.get(2)?,
            branch_name: row.get(3)?, url: row.get(4)?, name: row.get(5)?, description: row.get(6)?,
            rank: row.get(7)?, completed: row.get(8)?, completed_at: row.get(9)?, state: row.get(10)?,
            team_id: row.get(11)?, assignee: row.get(12)?,
            project_id: None, project: None, due_date: row.get(18)?,
        },row.get(13)?,row.get(14)?,row.get(15)?,row.get(16)?,row.get(17)?)),
    ).optional()?.ok_or(StoreError::NotFound)?;
    let project = project_in(conn, &ProjectId::from_raw(project_id))?;
    item.project_id = Some(project.id.clone());
    item.project = Some(project.slug.clone());
    let mut state = PlanningState::Available;
    if let Some(issue) = issue {
        let observation =
            match super::planning::pm_task_observation_in(conn, &repo, "linear", &issue) {
                Ok(observation) => observation,
                Err(StoreError::Serde(_)) => PmTaskObservation {
                    record: None,
                    state: PlanningState::Invalid,
                },
                Err(error) => return Err(error),
            };
        item.due_date = observation
            .record
            .as_ref()
            .and_then(|record| record.item.due_date.clone());
        if matches!(
            observation.state,
            PlanningState::Invalid | PlanningState::Removed
        ) {
            state = observation.state;
        }
        let pending_membership = super::planning_changes::PlanningChanges::Task(id)
            .pending(conn)?
            .into_iter()
            .find(|change| change.field == "project_id");
        if observation.record.as_ref().is_some_and(|record| {
            (record.item.project_id != item.project_id && pending_membership.is_none())
                || record
                    .item
                    .team_id
                    .as_ref()
                    .is_some_and(|team| !project.team_ids.contains(team))
        }) && state != PlanningState::Removed
        {
            state = PlanningState::Invalid;
        }
    }
    if deleted {
        state = PlanningState::Removed;
    }
    Ok(PmTaskObservation {
        record: Some(PmTaskRecord {
            item,
            project: Some(project),
            observed_at,
        }),
        state,
    })
}
