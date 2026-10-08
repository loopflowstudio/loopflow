//! Read saved planning independently of Workflow position and provider availability.

use rusqlite::{Connection, OptionalExtension};

use crate::durable::{ProjectId, TaskId};
use crate::id::WaveId;
use crate::pm::{PmItem, PmProject, PmSnapshot};
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

    pub(crate) fn planning_project(&self, id: &ProjectId) -> StoreResult<PmProject> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        project_in(&conn, id)
    }

    pub(crate) fn planning_projects(&self, wave: &WaveId) -> StoreResult<Vec<PmProject>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.unchecked_transaction()?;
        let projects = projects_in(&tx, wave)?;
        tx.commit()?;
        Ok(projects)
    }

    pub(crate) fn planning_wave(&self, wave: &WaveId) -> StoreResult<PmSnapshot> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.unchecked_transaction()?;
        let projects = projects_in(&tx, wave)?;
        let task_ids = {
            let mut query = tx.prepare("SELECT t.id FROM tasks t JOIN projects p ON p.id=t.project_id
                WHERE p.wave_id=?1 AND t.planning_deleted_at IS NULL ORDER BY t.planning_rank,t.created_at,t.id")?;
            let ids = query
                .query_map([wave], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            ids
        };
        let mut items = Vec::new();
        for id in task_ids {
            let observation = task_in(&tx, &TaskId::from_raw(id))?;
            if observation.state != PlanningState::Removed {
                if let Some(record) = observation.record {
                    items.push(record.item);
                }
            }
        }
        tx.commit()?;
        Ok(PmSnapshot { projects, items })
    }
}

fn projects_in(conn: &Connection, wave: &WaveId) -> StoreResult<Vec<PmProject>> {
    let project_ids = {
        let mut query = conn.prepare(
            "SELECT id FROM projects WHERE wave_id=?1 ORDER BY planning_rank,created_at,id",
        )?;
        let ids = query
            .query_map([wave], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids
    };
    project_ids
        .iter()
        .map(|id| project_in(conn, &ProjectId::from_raw(id)))
        .collect::<StoreResult<Vec<_>>>()
}

pub(super) fn project_in(conn: &Connection, id: &ProjectId) -> StoreResult<PmProject> {
    let (mut project, context, initiatives, teams): (PmProject, String, String, String) = conn.query_row(
        "SELECT COALESCE(p.external_project_id,p.id),p.project_slug,p.project_name,p.project_summary,
            p.project_prompt_context,p.workflow,p.status,p.planning_provider_revision,
            p.planning_initiatives,p.planning_teams
         FROM projects p
         WHERE p.id=?1",
        [id.as_str()], |row| Ok((PmProject {
            id: row.get(0)?, slug: row.get(1)?, name: row.get(2)?, summary: row.get(3)?,
            workflow: row.get(5)?, status: serde_json::from_value(serde_json::Value::String(row.get(6)?))
                .map_err(|error| rusqlite::Error::FromSqlConversionFailure(6,rusqlite::types::Type::Text,Box::new(error)))?, revision: row.get(7)?,
            krs: Vec::new(), metric_targets: Vec::new(), initiative_ids: Vec::new(), team_ids: Vec::new(),
        },row.get(4)?,row.get(8)?,row.get(9)?)),
    )?;
    let content = crate::pm::parse_project_content(&context)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    project.krs = content.krs;
    project.metric_targets = content.metric_targets;
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
            t.external_issue_id,t.planning_deleted_at IS NOT NULL
         FROM tasks t JOIN projects p ON p.id=t.project_id JOIN waves w ON w.id=p.wave_id WHERE t.id=?1"),
        [id.as_str()], |row| Ok((PmItem {
            revision: row.get(0)?, id: row.get(1)?, identifier: row.get(2)?,
            branch_name: row.get(3)?, url: row.get(4)?, name: row.get(5)?, description: row.get(6)?,
            rank: row.get(7)?, completed: row.get(8)?, completed_at: row.get(9)?, state: row.get(10)?,
            team_id: row.get(11)?, assignee: row.get(12)?,
            project_id: None, project: None,
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
