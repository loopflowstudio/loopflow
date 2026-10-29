//! Accept fetched Linear facts and canonical Work atomically. Foreign observations,
//! membership disputes and independent revisions belong to this adapter.

use super::observations::revision_nanos;
use super::LinearProject;
use crate::durable::ProjectId;
use crate::id::WaveId;
use crate::linear::LinearIssue;
use crate::store::sqlite::{self, SqliteStore};
use crate::store::sqlite::{
    planning_changes, planning_export, planning_write, task_state_delivery,
};
use crate::store::{run_planning_write, PlanningLocks, Store, StoreError, StoreResult};
use crate::work::edit::{Disposition, WorkValue as Edit};
use rusqlite::{named_params, params, Connection, OptionalExtension, Transaction};
use std::sync::Arc;

pub(crate) async fn accept_linear_inventory(
    store: &Store,
    wave: WaveId,
    initiative: String,
    projects: Vec<crate::linear::LinearProject>,
    issues: Vec<crate::linear::LinearIssue>,
    observed_at: i64,
    acquisition: Option<Arc<PlanningLocks>>,
) -> StoreResult<()> {
    run_planning_write(&store.sqlite, acquisition, move |store| {
        accept_linear_inventory_in(&store, &wave, &initiative, &projects, &issues, observed_at)
    })
    .await
}

pub(crate) async fn reconcile_linear_project_teams(
    store: &Store,
    wave: &WaveId,
    initiative: &str,
    project: crate::linear::LinearProject,
    observed_at: i64,
    acquisition: Arc<PlanningLocks>,
) -> StoreResult<()> {
    let wave = wave.clone();
    let initiative = initiative.to_string();
    run_planning_write(&store.sqlite, Some(acquisition), move |store| {
        reconcile_linear_project_teams_in(&store, &wave, &initiative, &project, observed_at)
    })
    .await
}

pub(crate) async fn accept_linear_issue(
    store: &Store,
    repo: &str,
    issue: crate::linear::LinearIssue,
    project: Option<crate::linear::LinearProject>,
    observed_at: i64,
    confirmed_wave: Option<(WaveId, String)>,
    acquisition: Option<Arc<PlanningLocks>>,
) -> StoreResult<()> {
    let repo = repo.to_string();
    run_planning_write(&store.sqlite, acquisition, move |store| {
        accept_linear_issue_in(
            &store,
            &repo,
            &issue,
            project.as_ref(),
            observed_at,
            confirmed_wave
                .as_ref()
                .map(|(wave, initiative)| (wave, initiative.as_str())),
        )
    })
    .await
}

pub(crate) fn accept_linear_project_in(
    store: &SqliteStore,
    wave: &WaveId,
    initiative: &str,
    project: &LinearProject,
    observed_at: i64,
) -> StoreResult<()> {
    let mut conn = store.conn.lock().expect("store mutex poisoned");
    let repo: String = conn.query_row(
        "SELECT repo FROM waves WHERE id=?1",
        [wave.as_str()],
        |row| row.get(0),
    )?;
    if project.initiative_ids.as_slice() != [initiative] {
        return Err(StoreError::InvalidData(format!(
            "Project {} does not belong to Initiative {initiative}",
            project.id
        )));
    }
    validate_project_membership(&conn, &repo, project)?;
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    put_project(&tx, &repo, observed_at, project)?;
    associate_project(&tx, &repo, &project.id, wave, initiative)?;
    project_accepted_planning(
        &tx,
        &repo,
        &[project.id.as_str()],
        &[],
        Some((wave, initiative)),
    )?;
    tx.commit()?;
    Ok(())
}

pub(crate) fn reconcile_linear_project_teams_in(
    store: &SqliteStore,
    wave: &WaveId,
    initiative: &str,
    project: &LinearProject,
    observed_at: i64,
) -> StoreResult<()> {
    let mut conn = store.conn.lock().expect("store mutex poisoned");
    let repo: String = conn.query_row(
        "SELECT repo FROM waves WHERE id=?1",
        [wave.as_str()],
        |row| row.get(0),
    )?;
    if project.initiative_ids.as_slice() != [initiative] {
        return Err(StoreError::InvalidData(format!(
            "Project {} changed Initiative during reteam",
            project.id
        )));
    }
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let previous: Option<String> = tx
        .query_row(
            "SELECT initiative_ids FROM pm_projects WHERE repo=?1 AND provider=?2 AND id=?3",
            params![repo, "linear", project.id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(initiatives) = previous {
        if serde_json::from_str::<Vec<String>>(&initiatives)?.as_slice() != [initiative] {
            return Err(StoreError::InvalidData(format!(
                "Project {} changed Initiative during reteam",
                project.id
            )));
        }
        // Reteam orders Team relationships only. Retain independently newer entity facts.
        tx.execute(
            "UPDATE pm_projects SET team_ids=?4,membership_unresolved=0
             WHERE repo=?1 AND provider=?2 AND id=?3",
            params![
                repo,
                "linear",
                project.id,
                serde_json::to_string(&project.team_ids)?
            ],
        )?;
    }
    put_project(&tx, &repo, observed_at, project)?;
    associate_project(&tx, &repo, &project.id, wave, initiative)?;
    project_accepted_planning(
        &tx,
        &repo,
        &[project.id.as_str()],
        &[],
        Some((wave, initiative)),
    )?;
    tx.commit()?;
    Ok(())
}

pub(crate) fn accept_linear_issue_in(
    store: &SqliteStore,
    repo: &str,
    issue: &LinearIssue,
    project: Option<&LinearProject>,
    observed_at: i64,
    confirmed_wave: Option<(&WaveId, &str)>,
) -> StoreResult<()> {
    let mut conn = store.conn.lock().expect("store mutex poisoned");
    if let Some(project) = project {
        validate_project_membership(&conn, repo, project)?;
    }
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut item = issue.clone();
    // Detail queries do not observe relative list order.
    if let Some(rank) = tx.query_row(
        "SELECT rank FROM pm_items WHERE repo=?1 AND provider=?2 AND id=?3 AND project_id IS ?4",
        params![repo,"linear",item.id,item.project_id], |row| row.get::<_,u32>(0),
    ).optional()? {
        item.rank = rank;
    }
    // Project revisions are independent of the issue revision and request age.
    if let Some(project) = project {
        put_project(&tx, repo, observed_at, project)?;
    }
    if put_item(&tx, repo, observed_at, &item)? {
        tx.execute(
            "UPDATE pm_items SET needs_refresh=0 WHERE repo=?1 AND provider=?2 AND id=?3",
            params![repo, "linear", issue.id],
        )?;
    }
    if let Some((wave, initiative)) = confirmed_wave {
        let project_id = issue.project_id.as_deref().ok_or_else(|| {
            StoreError::InvalidData("confirmed Wave requires a Project association".into())
        })?;
        associate_project(&tx, repo, project_id, wave, initiative)?;
    }
    project_accepted_planning(
        &tx,
        repo,
        project.map(|project| project.id.as_str()).as_slice(),
        &[issue.id.as_str()],
        confirmed_wave,
    )?;
    tx.commit()?;
    Ok(())
}

pub(crate) fn accept_linear_inventory_in(
    store: &SqliteStore,
    wave: &WaveId,
    initiative: &str,
    projects: &[LinearProject],
    issues: &[LinearIssue],
    observed_at: i64,
) -> StoreResult<()> {
    let mut conn = store.conn.lock().expect("store mutex poisoned");
    let repo: String = conn.query_row(
        "SELECT repo FROM waves WHERE id=?1",
        [wave.as_str()],
        |row| row.get(0),
    )?;
    for project in projects {
        validate_project_membership(&conn, &repo, project)?;
    }
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let known = {
        let mut query = tx.prepare(
            "SELECT m.project_id FROM pm_wave_projects m JOIN pm_projects p ON p.id=m.project_id
             WHERE m.wave_id=?1 AND p.repo=?2 AND p.provider=?3 AND p.archived=0",
        )?;
        let rows = query
            .query_map(params![wave, repo, "linear"], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        rows
    };
    if let Some(missing) = known
        .iter()
        .find(|id| !projects.iter().any(|project| &project.id == *id))
    {
        return Err(StoreError::InvalidData(format!(
            "Project {missing} omitted without membership removal evidence; refresh planning"
        )));
    }
    for project in projects {
        put_project(&tx, &repo, observed_at, project)?;
        require_accepted_initiative(&tx, &repo, &project.id, initiative)?;
    }
    for item in issues {
        put_item(&tx, &repo, observed_at, item)?;
    }
    tx.execute(
        "INSERT INTO pm_wave_sync(wave_id,provider,initiative,synced_at) VALUES(?1,?2,?3,?4)
         ON CONFLICT(wave_id) DO UPDATE SET provider=excluded.provider,
         initiative=excluded.initiative,synced_at=MAX(pm_wave_sync.synced_at,excluded.synced_at)",
        params![wave, "linear", initiative, observed_at],
    )?;
    for (position, project) in projects.iter().enumerate() {
        tx.execute(
            "INSERT INTO pm_wave_projects(wave_id,project_id,position) VALUES(?1,?2,?3)
             ON CONFLICT(wave_id,project_id) DO NOTHING",
            params![wave, project.id, position as i64],
        )?;
    }
    project_accepted_planning(
        &tx,
        &repo,
        &projects.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        &issues.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        None,
    )?;
    for project in projects {
        observe_order(
            &tx,
            &repo,
            &project.id,
            &issues
                .iter()
                .filter(|i| i.project_id.as_deref() == Some(&project.id))
                .cloned()
                .collect::<Vec<_>>(),
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub(crate) fn accept_linear_project_order_in(
    store: &SqliteStore,
    repo: &str,
    project: &str,
    items: &[LinearIssue],
) -> StoreResult<bool> {
    let mut conn = store.conn.lock().expect("store mutex poisoned");
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let now = crate::store::rows::now_unix();
    for item in items {
        put_item(&tx, repo, now, item)?;
    }
    project_accepted_planning(
        &tx,
        repo,
        &[],
        &items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        None,
    )?;
    let accepted = observe_order(&tx, repo, project, items)?;
    tx.commit()?;
    Ok(accepted)
}

fn same_ids(left: &[String], right: &[String]) -> bool {
    left.iter().all(|id| right.contains(id)) && right.iter().all(|id| left.contains(id))
}

fn validate_project_membership(
    conn: &Connection,
    repo: &str,
    project: &LinearProject,
) -> StoreResult<()> {
    // Run before the ingestion transaction: disputed ownership must remain
    // recorded even when rejecting the incoming facts rolls that transaction back.
    let previous = conn
        .query_row(
            "SELECT revision,initiative_ids,
                    team_ids
             FROM pm_projects WHERE repo=?1 AND provider=?2 AND id=?3",
            params![repo, "linear", project.id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((previous_revision, initiatives, teams)) = previous else {
        return Ok(());
    };
    let revision = revision_nanos(project.revision.as_deref())?;
    if revision.is_some() && revision < revision_nanos(previous_revision.as_deref())? {
        return Ok(());
    }
    // A Project revision does not order its separate Initiative/Team relationships.
    // Preserve the facts, but do not let managed readers use disputed ownership.
    if !same_ids(
        &project.initiative_ids,
        &serde_json::from_str::<Vec<String>>(&initiatives)?,
    ) || !same_ids(
        &project.team_ids,
        &serde_json::from_str::<Vec<String>>(&teams)?,
    ) {
        conn.execute("UPDATE pm_projects SET membership_unresolved=1 WHERE repo=?1 AND provider=?2 AND id=?3",
            params![repo, "linear", project.id])?;
        return Err(StoreError::InvalidData(format!(
            "Project {} membership changed without relationship ordering evidence",
            project.id
        )));
    }
    Ok(())
}

fn put_project(
    conn: &Connection,
    repo: &str,
    observed_at: i64,
    project: &LinearProject,
) -> StoreResult<()> {
    let krs = serde_json::to_string(&project.krs)?;
    let metric_targets = serde_json::to_string(&project.metric_targets)?;
    let initiative_ids = serde_json::to_string(&project.initiative_ids)?;
    let team_ids = serde_json::to_string(&project.team_ids)?;
    let status = project.status.as_str();
    let values = named_params! {
        ":repo": repo, ":provider": "linear", ":id": &project.id,
        ":observed_at": observed_at,
        ":slug": &project.slug,
        ":name": &project.name,
        ":summary": &project.summary,
        ":status": &status,
        ":krs": &krs,
        ":metric_targets": &metric_targets,
        ":initiative_ids": &initiative_ids,
        ":team_ids": &team_ids,
        ":description": &project.description,
        ":revision": &project.revision,
    };
    let pending_name_cutover: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pm_project_name_cutover
         WHERE repo=?1 AND provider=?2 AND id=?3 AND converted_at IS NULL)",
        params![repo, "linear", project.id],
        |row| row.get(0),
    )?;
    let previous = conn
        .query_row(
            "SELECT revision,observed_at,archived,
            ((slug IS NOT NULL AND slug IS NOT :slug) OR (name IS NOT NULL AND name IS NOT :name)),
            (summary IS NOT NULL AND summary IS NOT :summary) OR (status IS NOT NULL AND status IS NOT :status)
            OR (description_revision IS revision AND description IS NOT NULL
                AND :description IS NOT NULL AND description IS NOT :description),krs,metric_targets
         FROM pm_projects WHERE repo=:repo AND provider=:provider AND id=:id",
            named_params! { ":repo": repo, ":provider": "linear", ":id": &project.id,
            ":slug": &project.slug, ":name": &project.name, ":summary": &project.summary,
            ":status": &status, ":description": &project.description },
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, bool>(2)?,
                    row.get::<_, bool>(3)?,
                    row.get::<_, bool>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, Option<String>>(6)?,
                ))
            },
        )
        .optional()?;
    let revision = revision_nanos(project.revision.as_deref())?;
    if let Some((
        previous,
        acquired,
        archived,
        name_changed,
        content_changed,
        previous_krs,
        previous_targets,
    )) = previous
    {
        if archived {
            return Ok(());
        }
        let previous_revision = revision_nanos(previous.as_deref())?;
        if revision.is_some() && revision < previous_revision {
            return Ok(());
        }
        if revision.is_none() || revision == previous_revision {
            let criteria_changed = previous_krs
                .map(|value| serde_json::from_str::<Vec<crate::work::project::ProjectKR>>(&value))
                .transpose()?
                .is_some_and(|value| value != project.krs)
                || previous_targets
                    .map(|value| {
                        serde_json::from_str::<Vec<crate::work::wave::metrics::MetricTarget>>(
                            &value,
                        )
                    })
                    .transpose()?
                    .is_some_and(|value| value != project.metric_targets);
            if content_changed || criteria_changed || (name_changed && !pending_name_cutover) {
                return Err(StoreError::InvalidData(format!(
                    "unordered or conflicting Project facts for {}; refresh planning",
                    project.id
                )));
            }
        }
        if revision.is_none() && previous_revision.is_some()
            || revision == previous_revision && observed_at < acquired
        {
            return Ok(());
        }
    }
    conn.execute(
        "INSERT INTO pm_projects(repo,provider,id,observed_at,slug,name,summary,status,krs,metric_targets,initiative_ids,team_ids,description,revision,description_revision,description_observed_at)
         VALUES(:repo,:provider,:id,:observed_at,:slug,:name,:summary,:status,:krs,:metric_targets,:initiative_ids,:team_ids,:description,:revision,
             CASE WHEN :description IS NOT NULL THEN :revision END,
             CASE WHEN :description IS NOT NULL THEN :observed_at END)
         ON CONFLICT(repo,provider,id) DO UPDATE SET
         observed_at=excluded.observed_at,revision=excluded.revision,
         slug=excluded.slug,name=excluded.name,summary=excluded.summary,status=excluded.status,krs=excluded.krs,metric_targets=excluded.metric_targets,initiative_ids=excluded.initiative_ids,team_ids=excluded.team_ids,
         description=COALESCE(excluded.description,pm_projects.description),
         description_revision=CASE WHEN excluded.description IS NOT NULL THEN excluded.revision ELSE description_revision END,
         description_observed_at=COALESCE(excluded.description_observed_at,description_observed_at)", values,
    )?;
    if pending_name_cutover {
        conn.execute(
            "UPDATE pm_project_name_cutover SET converted_at=?4
             WHERE repo=?1 AND provider=?2 AND id=?3 AND converted_at IS NULL",
            params![repo, "linear", project.id, observed_at],
        )?;
    }
    Ok(())
}

pub(crate) fn put_item(
    conn: &Connection,
    repo: &str,
    observed_at: i64,
    item: &LinearIssue,
) -> StoreResult<bool> {
    let revision = revision_nanos(item.revision.as_deref())?;
    let change: Option<(Option<i64>, bool)> = conn
        .query_row(
            "SELECT revision_ns,removed FROM pm_issue_changes WHERE issue_id=?1",
            [&item.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if change.is_some_and(|(floor, removed)| removed || revision < floor) {
        return Ok(false);
    }
    let previous = conn.query_row(
        "SELECT revision,observed_at,identifier IS NOT :identifier OR claimed_project_id IS NOT :project_id OR url IS NOT :url OR name IS NOT :name OR description IS NOT :description OR completed IS NOT :completed OR state IS NOT :state OR team_id IS NOT :team_id OR assignee IS NOT :assignee OR (branch_name IS NOT NULL AND branch_name IS NOT :branch_name) OR (completed_at_observed AND completed_at IS NOT :completed_at) OR (due_date_observed AND due_date IS NOT :due_date),
            state IS NOT :state OR completed IS NOT :completed
         FROM pm_items WHERE repo=:repo AND provider=:provider AND id=:id",
        named_params! { ":repo": repo, ":provider": "linear", ":id": &item.id,
            ":identifier": &item.identifier,
            ":project_id": &item.project_id,
            ":branch_name": &item.branch_name,
            ":url": &item.url,
            ":name": &item.name,
            ":description": &item.description,
            ":completed": &item.completed,
            ":completed_at": &item.completed_at,
            ":due_date": &item.due_date,
            ":state": &item.state,
            ":team_id": &item.team_id,
            ":assignee": &item.assignee,
        },
        |row| Ok((row.get::<_,Option<String>>(0)?,row.get::<_,i64>(1)?,
            row.get::<_,bool>(2)?,row.get::<_,bool>(3)?)),
    ).optional()?;
    let mut status_changed = previous.is_none();
    if let Some((previous, acquired, conflicting, changed)) = previous {
        let previous_revision = revision_nanos(previous.as_deref())?;
        if revision < previous_revision {
            return Ok(false);
        }
        if revision == previous_revision && conflicting {
            return Err(StoreError::InvalidData(format!(
                "conflicting planning facts at the same provider revision for {}; refresh planning",
                item.identifier
            )));
        }
        status_changed = revision > previous_revision && changed;
        if revision == previous_revision && observed_at < acquired {
            return Ok(false);
        }
    }
    conn.execute(
        "INSERT INTO pm_items(repo,provider,id,observed_at,identifier,project_id,revision,branch_name,url,name,description,rank,completed,completed_at,due_date,state,team_id,assignee,claimed_project_id,completed_at_observed,due_date_observed,project_id_observed,assignee_observed)
         VALUES(:repo,:provider,:id,:observed_at,:identifier,:project_id,:revision,:branch_name,:url,:name,:description,:rank,:completed,:completed_at,:due_date,:state,:team_id,:assignee,:project_id,1,1,1,1)
         ON CONFLICT(repo,provider,id) DO UPDATE SET observed_at=excluded.observed_at,
         identifier=excluded.identifier,project_id=excluded.project_id,revision=excluded.revision,branch_name=excluded.branch_name,url=excluded.url,name=excluded.name,description=excluded.description,rank=excluded.rank,completed=excluded.completed,completed_at=excluded.completed_at,due_date=excluded.due_date,state=excluded.state,team_id=excluded.team_id,assignee=excluded.assignee,
         claimed_project_id=excluded.claimed_project_id,completed_at_observed=1,due_date_observed=1,project_id_observed=1,assignee_observed=1",
        named_params! { ":repo": repo, ":provider": "linear", ":id": &item.id,
            ":observed_at": observed_at, ":rank": item.rank, ":revision": &item.revision,
            ":identifier": &item.identifier,
            ":project_id": &item.project_id,
            ":branch_name": &item.branch_name,
            ":url": &item.url,
            ":name": &item.name,
            ":description": &item.description,
            ":completed": &item.completed,
            ":completed_at": &item.completed_at,
            ":due_date": &item.due_date,
            ":state": &item.state,
            ":team_id": &item.team_id,
            ":assignee": &item.assignee,
        },
    )?;
    if status_changed {
        // A newer authored status supersedes an old end trigger or writeback.
        // Execution and delivery evidence are deliberately untouched.
        if item.is_complete() {
            conn.execute(
                "INSERT INTO task_events(task_id,kind_json,created_at)
                 SELECT t.id,json_object('kind','completed','summary','Completion observed in Linear'),?3
                 FROM tasks t JOIN repository_plans w ON w.id=t.repository_id
                 WHERE t.external_issue_id=?1 AND w.repo=?2 AND t.planning_completed=0",
                params![item.id,repo,observed_at])?;
        }
        conn.execute(
            "UPDATE tasks SET completion_request=NULL,completion_error=NULL
             WHERE external_issue_id=?1 AND repository_id IN
             (SELECT id FROM repository_plans WHERE repo=?2)",
            params![item.id, repo],
        )?;
    }
    Ok(true)
}

/// Reconcile a delivery's read; true means its saved intention remains pending.
pub(crate) fn observe_task_state(
    store: &SqliteStore,
    delivery: &sqlite::task_state_delivery::TaskStateDelivery,
    observed: &crate::linear::LinearIssue,
    history: crate::linear::StateHistory,
) -> StoreResult<bool> {
    let mut conn = store.conn.lock().expect("store mutex poisoned");
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    // Retain the provider baseline for the next local save. A response for an
    // older delivery cannot ingest over the current decision.
    let repo: Option<String> = tx
        .query_row(
            "SELECT w.repo FROM task_state_deliveries d JOIN tasks t ON t.id=d.task_id
         JOIN repository_plans w ON w.id=t.repository_id
         WHERE d.id=?1 AND d.task_id=?2 AND d.settled=0 AND t.external_issue_id=?3
         AND d.seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=t.id)",
            params![delivery.id, delivery.task_id.as_str(), observed.id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(repo) = repo else { return Ok(false) };
    if !put_item(&tx, &repo, crate::store::rows::now_unix(), observed)? {
        return Ok(false);
    }
    let pending = sqlite::task_state_delivery::reconcile_in(
        &tx,
        &delivery.task_id,
        observed.revision.as_deref(),
        &crate::work::edit::Disposition {
            planning_state: observed.state.clone(),
            planning_completed: i64::from(observed.completed),
            planning_completed_at: observed.completed_at.clone(),
        },
        &serde_json::json!({
            "revision": observed.revision,
            "state": observed.state,
            "completed": observed.completed,
            "completed_at": observed.completed_at,
        })
        .to_string(),
        history,
    )?;
    tx.commit()?;
    Ok(pending)
}

fn observe_order(
    conn: &Connection,
    repo: &str,
    external: &str,
    items: &[LinearIssue],
) -> StoreResult<bool> {
    let project: Option<String> = conn.query_row("SELECT p.id FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.external_project_id=?1 AND w.repo=?2", params![external,repo], |row| row.get(0)).optional()?;
    let Some(project) = project.map(ProjectId::from_raw) else {
        return Ok(false);
    };
    let mut items = items.iter().collect::<Vec<_>>();
    items.sort_by_key(|i| i.rank);
    let mut observed = Vec::new();
    for item in items {
        // An older list must not order a mixture of old and retained newer facts.
        let retained = conn.query_row(
            "SELECT t.id,i.revision,i.rank,i.project_id
             FROM pm_items i JOIN tasks t ON t.external_issue_id=i.id
             WHERE i.repo=?1 AND i.provider=?2 AND i.id=?3 AND i.needs_refresh=0 AND t.project_id=?4 AND t.planning_deleted_at IS NULL",
            params![repo,"linear",item.id,project.as_str()], |row| Ok((
                row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?,
                row.get::<_, u32>(2)?, row.get::<_, Option<String>>(3)?,
            ))).optional()?;
        let Some((id, revision, rank, membership)) = retained else {
            return Ok(false);
        };
        if revision != item.revision || rank != item.rank || membership != item.project_id {
            return Ok(false);
        }
        observed.push(id);
    }
    sqlite::planning_order::observe_in(conn, repo, external, &project, observed)
}

// Both durable projections require the same confirmed Initiative/Wave association.
// A partial read supplies that association without advancing full-Wave freshness.
const ACCEPTED_WAVE_PROJECTS: &str = "
    SELECT p.repo,p.provider,p.id,p.observed_at,p.archived,p.membership_unresolved,p.task_order_json,p.revision,p.slug,p.name,p.summary,p.status,p.krs,p.metric_targets,p.initiative_ids,p.team_ids,p.description,p.description_revision,p.description_observed_at,m.wave_id FROM pm_projects p
    JOIN pm_wave_projects m ON m.project_id=p.id
    LEFT JOIN pm_wave_sync sync ON sync.wave_id=m.wave_id AND sync.provider=p.provider
    JOIN waves w ON w.id=m.wave_id AND w.repo=p.repo
    WHERE p.repo=?1 AND p.provider=?2 AND p.archived=0 AND p.membership_unresolved=0
    AND json_array_length(p.initiative_ids)=1
    AND json_extract(p.initiative_ids,'$[0]')=
        CASE WHEN m.wave_id=?4 THEN ?5 ELSE sync.initiative END";

/// Inputs select IDs only; project their accepted rows inside the ingestion transaction.
fn project_accepted_planning(
    tx: &Transaction<'_>,
    repo: &str,
    projects: &[&str],
    items: &[&str],
    confirmed_wave: Option<(&WaveId, &str)>,
) -> StoreResult<()> {
    let (confirmed_wave, confirmed_initiative) = confirmed_wave.unzip();
    let project_ids = serde_json::to_string(projects)?;
    let item_ids = serde_json::to_string(items)?;
    let mut query = tx.prepare(&format!(
        "WITH accepted AS ({ACCEPTED_WAVE_PROJECTS})
         SELECT id,observed_at,wave_id,revision,
             slug,name,
             summary,json_quote(status),
             krs,
             metric_targets,initiative_ids,
             team_ids,
             CASE WHEN description_revision IS revision THEN description END
         FROM accepted WHERE id IN (SELECT value FROM json_each(?3))"
    ))?;
    let observations = query
        .query_and_then(
            params![
                repo,
                "linear",
                project_ids,
                confirmed_wave,
                confirmed_initiative
            ],
            |row| -> StoreResult<_> {
                let external: String = row.get(0)?;
                let wave: WaveId = row.get(2)?;
                let mut edits = vec![
                    Edit::ProjectWave(wave.to_string()),
                    Edit::ProjectLinearId(Some(external.clone())),
                    Edit::ProjectSlug(Some(row.get(4)?)),
                    Edit::ProjectName(Some(row.get(5)?)),
                    Edit::ProjectSummary(row.get(6)?),
                    Edit::ProjectStatus(serde_json::from_str(&row.get::<_, String>(7)?)?),
                    Edit::Krs(serde_json::from_str(&row.get::<_, String>(8)?)?),
                    Edit::MetricTargets(serde_json::from_str(&row.get::<_, String>(9)?)?),
                    Edit::ProjectInitiatives(serde_json::from_str(&row.get::<_, String>(10)?)?),
                    Edit::ProjectTeams(serde_json::from_str(&row.get::<_, String>(11)?)?),
                ];
                if let Some(description) = row.get::<_, Option<String>>(12)? {
                    edits.push(Edit::ProjectDescription(Some(description)));
                }
                Ok((
                    external,
                    row.get::<_, i64>(1)?,
                    wave,
                    row.get::<_, Option<String>>(3)?,
                    edits,
                ))
            },
        )?
        .collect::<StoreResult<Vec<_>>>()?;
    drop(query);
    for (external, observed_at, wave_id, revision, mut edits) in observations {
        planning_export::attach_in(tx, repo, true, &external, revision.as_deref(), &edits)?;
        let existing: Option<(String, WaveId)> = tx
            .query_row(
                "SELECT id,wave_id FROM projects WHERE external_project_id=?1",
                [&external],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let id = match existing {
            Some((_, owner)) if owner != wave_id => {
                return Err(StoreError::InvalidData(format!(
                    "Project {external} changed Wave ownership"
                )));
            }
            Some((id, _)) => {
                ProjectId::parse(&id).map_err(|error| StoreError::InvalidData(error.to_string()))
            }?,
            None => ProjectId::new(),
        };
        let rank: Option<u32> = tx
            .query_row(
                "SELECT position FROM pm_wave_projects WHERE wave_id=?1 AND project_id=?2",
                params![wave_id, external],
                |r| r.get(0),
            )
            .optional()?;
        if let Some(rank) = rank {
            edits.push(Edit::ProjectRank(rank));
        }
        planning_changes::PlanningChanges::Project(&id).reconcile_edits(
            tx,
            revision.as_deref(),
            &mut edits,
        )?;
        planning_write::create(
            tx,
            repo,
            &crate::work::edit::WorkEdit::for_target(id.as_str(), &edits),
        )?;
        tx.execute("UPDATE projects SET pm_snapshot_synced_at=?2,planning_provider_revision=?3 WHERE id=?1",params![id.as_str(),observed_at,revision])?;
    }
    let mut query = tx.prepare(
        "SELECT i.id,i.revision,i.name,i.description,i.assignee,p.id,i.assignee_observed,i.project_id_observed,i.claimed_project_id
         FROM pm_items i LEFT JOIN projects p ON p.external_project_id=i.project_id
         WHERE i.repo=?1 AND i.provider='linear' AND i.needs_refresh=0
         AND i.id IN (SELECT value FROM json_each(?2))",
    )?;
    let observed = query
        .query_map(params![repo, item_ids], |row| {
            let mut edits = vec![
                Edit::TaskTitle(Some(row.get(2)?)),
                Edit::TaskDescription(Some(row.get(3)?)),
            ];
            if row.get::<_, bool>(6)? {
                edits.push(Edit::TaskAssignee(row.get(4)?));
            }
            if let Some(project) = row.get::<_, Option<String>>(5)? {
                edits.push(Edit::TaskProject(Some(project)));
            } else if row.get::<_, bool>(7)? && row.get::<_, Option<String>>(8)?.is_none() {
                edits.push(Edit::TaskProject(None));
            }
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                edits,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(query);
    for (id, revision, edits) in observed {
        planning_export::attach_in(tx, repo, false, &id, revision.as_deref(), &edits)?;
    }
    // Admission and refresh share one accepted observation. Existing Tasks retain
    // their IDs, local order and pending disposition; new Tasks receive only Work.
    let mut query = tx.prepare(&format!(
        "WITH accepted AS ({ACCEPTED_WAVE_PROJECTS})
         SELECT target.id,json_object('revision',i.revision,'state',i.state,'completed',json(CASE i.completed WHEN 1 THEN 'true' ELSE 'false' END),'completed_at',i.completed_at),i.observed_at,p.id,i.id,
             i.revision,i.identifier,
             i.name,i.description,
             i.rank,i.url,
             i.branch_name,i.team_id,
             i.assignee,i.due_date,
             i.state,i.completed,
             i.completed_at FROM pm_items i
         LEFT JOIN projects p ON p.external_project_id=i.project_id
         LEFT JOIN accepted observed ON observed.id=i.project_id AND observed.wave_id=p.wave_id
         LEFT JOIN tasks target ON target.external_issue_id=i.id
         LEFT JOIN projects current ON current.id=target.project_id
         WHERE i.repo=?1 AND i.provider=?2 AND i.needs_refresh=0
         AND i.id IN (SELECT value FROM json_each(?3))
         AND ((observed.id IS NOT NULL AND observed.wave_id=p.wave_id)
              OR (i.project_id_observed=1 AND i.claimed_project_id IS NULL))
         AND (target.id IS NULL OR target.repository_id=(SELECT id FROM repository_plans WHERE repo=?1 AND selected=1))
         AND NOT EXISTS(SELECT 1 FROM task_deletions d
             WHERE d.issue_id=i.id AND EXISTS(SELECT 1 FROM waves dw WHERE dw.id=d.wave_id AND dw.repo=?1))
         AND NOT EXISTS(SELECT 1 FROM pm_issue_changes c WHERE c.issue_id=i.id
             AND c.removed=1)"
    ))?;
    let observations = query
        .query_map(
            params![
                repo,
                "linear",
                item_ids,
                confirmed_wave,
                confirmed_initiative
            ],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    vec![
                        Edit::TaskProject(row.get(3)?),
                        Edit::TaskLinearId(Some(row.get(4)?)),
                        Edit::TaskIdentifier(row.get(6)?),
                        Edit::TaskTitle(Some(row.get(7)?)),
                        Edit::TaskDescription(Some(row.get(8)?)),
                        Edit::TaskRank(row.get(9)?),
                        Edit::TaskUrl(row.get(10)?),
                        Edit::TaskBranch(row.get(11)?),
                        Edit::TaskTeam(row.get(12)?),
                        Edit::TaskAssignee(row.get(13)?),
                        Edit::TaskDueDate(row.get(14)?),
                    ],
                    Disposition {
                        planning_state: row.get(15)?,
                        planning_completed: i64::from(row.get::<_, bool>(16)?),
                        planning_completed_at: row.get(17)?,
                    },
                ))
            },
        )?
        .collect::<Result<Vec<_>, _>>()?;
    drop(query);
    for (existing, body, observed_at, external, revision, mut edits, disposition) in observations {
        let id = existing.as_ref().map_or_else(
            || crate::durable::TaskId::from_issue(&external),
            crate::durable::TaskId::from_raw,
        );
        edits.push(Edit::Disposition(disposition.clone()));
        if existing.is_some() {
            task_state_delivery::reconcile_in(
                tx,
                &id,
                revision.as_deref(),
                &disposition,
                &body,
                crate::linear::StateHistory::Unknown,
            )?;
            let pending: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM task_state_deliveries WHERE task_id=?1 AND settled=0
                 AND seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=?1))",
                [id.as_str()],
                |row| row.get(0),
            )?;
            planning_changes::PlanningChanges::Task(&id).reconcile_edits(
                tx,
                revision.as_deref(),
                &mut edits,
            )?;
            edits.retain(|edit| {
                !matches!(edit, Edit::TaskRank(_))
                    && (!pending || !matches!(edit, Edit::Disposition(_)))
            });
        }
        planning_write::create(
            tx,
            repo,
            &crate::work::edit::WorkEdit::for_target(id.as_str(), &edits),
        )?;
        tx.execute(
            "UPDATE tasks SET planning_provider_revision=?2,pm_snapshot_synced_at=?3 WHERE id=?1",
            params![id.as_str(), revision, observed_at],
        )?;
    }
    Ok(())
}

fn associate_project(
    tx: &Transaction<'_>,
    repo: &str,
    project_id: &str,
    wave: &WaveId,
    initiative: &str,
) -> StoreResult<()> {
    require_accepted_initiative(tx, repo, project_id, initiative)?;
    let wave_repo: String = tx.query_row("SELECT repo FROM waves WHERE id=?1", [wave], |row| {
        row.get(0)
    })?;
    if wave_repo != repo {
        return Err(StoreError::InvalidData(
            "confirmed Wave belongs to another repository".into(),
        ));
    }
    tx.execute(
        "INSERT INTO pm_wave_projects(wave_id,project_id,position)
         VALUES(?1,?2,(SELECT COALESCE(MAX(position)+1,0) FROM pm_wave_projects WHERE wave_id=?1))
         ON CONFLICT(wave_id,project_id) DO NOTHING",
        params![wave, project_id],
    )?;
    Ok(())
}

fn require_accepted_initiative(
    conn: &Connection,
    repo: &str,
    project_id: &str,
    initiative: &str,
) -> StoreResult<()> {
    let initiatives: String = conn.query_row(
        "SELECT initiative_ids FROM pm_projects WHERE repo=?1 AND provider=?2 AND id=?3",
        params![repo, "linear", project_id],
        |row| row.get(0),
    )?;
    if serde_json::from_str::<Vec<String>>(&initiatives)?.as_slice() != [initiative] {
        return Err(StoreError::InvalidData(format!(
            "accepted Project {project_id} does not belong to Initiative {initiative}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::test_linear_input::SeedLinearSqlite;
    use rusqlite::{params, Connection};

    use crate::id::WaveId;
    use crate::store::sqlite::SqliteStore;
    use crate::store::PlanningState;

    #[test]
    fn explicit_null_detaches_but_unknown_linear_membership_retains_task() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let wave = crate::work::wave::Wave::new(WaveId::new(), "product".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        let mut snapshot: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        snapshot.projects.truncate(1);
        snapshot
            .items
            .retain(|item| item.project_id.as_deref() == Some(&snapshot.projects[0].id));
        snapshot.items.truncate(1);
        let mut issue = snapshot.items[0].clone();
        store
            .seed_inventory(&crate::test_linear_input::InventoryFixture {
                wave_id: wave.id().clone(),
                provider: "linear".into(),
                initiative: snapshot.projects[0].initiative_ids[0].clone(),
                synced_at: 42,
                snapshot,
            })
            .unwrap();
        let original = store.task_by_issue(&issue.id).unwrap().unwrap();
        // Unmapped Project evidence cannot manufacture a standalone Task.
        issue.project_id = Some("unknown-project".into());
        issue.revision = Some("2026-10-17T01:00:00Z".into());
        super::accept_linear_issue_in(&store, "/repo", &issue, None, 43, None).unwrap();
        assert_eq!(
            store.task(&original.id).unwrap().unwrap().project_id,
            original.project_id
        );
        assert_eq!(
            store.task_acquisition(&original.id).unwrap().state,
            PlanningState::Invalid
        );
        // An omitted field in retained historical evidence remains unknown.
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE pm_items SET project_id=NULL,claimed_project_id=NULL,project_id_observed=0",
                [],
            )
            .unwrap();
        assert_eq!(
            store
                .linear_issue_evidence("/repo", &issue.id)
                .unwrap()
                .state,
            PlanningState::Invalid
        );
        assert_eq!(
            store.task(&original.id).unwrap().unwrap().project_id,
            original.project_id
        );
        // A complete, newer observation explicitly clearing Project detaches it.
        issue.project_id = None;
        issue.revision = Some("2026-10-17T02:00:00Z".into());
        super::accept_linear_issue_in(&store, "/repo", &issue, None, 44, None).unwrap();
        let detached = store.task(&original.id).unwrap().unwrap();
        assert!(detached.project_id.is_none());
        assert!(detached.wave_id.is_none());
        assert_eq!(detached.repository_id, original.repository_id);
        assert_eq!(
            store.task_acquisition(&original.id).unwrap().state,
            PlanningState::Available
        );
    }

    #[test]
    fn task_admission_reads_released_evidence_without_reconstructing_provider_content() {
        use crate::store::migrations::{apply_before_migration, upgrade_from_migration};
        use crate::work::project::ProjectId;
        use crate::work::task::TaskId;
        use std::sync::{Arc, Mutex};

        let cases = [
            ("", PlanningState::Available),
            ("UPDATE pm_items SET needs_refresh=1 WHERE id='issue'", PlanningState::Invalid),
            ("UPDATE pm_projects SET membership_unresolved=1 WHERE id='project'", PlanningState::Invalid),
            ("UPDATE pm_projects SET archived=1 WHERE id='project'", PlanningState::Invalid),
            ("UPDATE pm_items SET project_id='unmapped' WHERE id='issue'", PlanningState::Invalid),
            ("UPDATE pm_items SET body=json_set(body,'$.project_id','unmapped') WHERE id='issue'", PlanningState::Invalid),
            ("UPDATE pm_items SET body=json_set(body,'$.team_id','foreign') WHERE id='issue'", PlanningState::Invalid),
            ("UPDATE pm_items SET body=json_remove(body,'$.team_id') WHERE id='issue'", PlanningState::Invalid),
            ("UPDATE pm_items SET body=json_remove(body,'$.project_id') WHERE id='issue'", PlanningState::Invalid),
            ("UPDATE pm_projects SET body=json_set(body,'$.team_ids',json('[\"foreign\"]')) WHERE id='project'", PlanningState::Invalid),
            ("INSERT INTO pm_issue_changes(issue_id,removed) VALUES('issue',1)", PlanningState::Removed),
            ("INSERT INTO task_deletions(wave_id,issue_id,identifier,confirmed_at) SELECT id,'issue','TEAM-1',55 FROM waves", PlanningState::Removed),
            ("DELETE FROM pm_items WHERE id='issue'", PlanningState::Unavailable),
            ("DELETE FROM pm_items WHERE id='issue'; UPDATE pm_projects SET membership_unresolved=1 WHERE id='project'", PlanningState::Invalid),
            ("DELETE FROM pm_items WHERE id='issue'; UPDATE pm_projects SET archived=1 WHERE id='project'", PlanningState::Invalid),
            ("DELETE FROM pm_items WHERE id='issue'; INSERT INTO pm_issue_changes(issue_id,removed) VALUES('issue',0)", PlanningState::Invalid),
            ("DELETE FROM pm_items WHERE id='issue'; INSERT INTO pm_issue_changes(issue_id,removed) VALUES('issue',1)", PlanningState::Removed),
        ];
        for (sql, expected) in cases {
            let conn = Connection::open_in_memory().unwrap();
            apply_before_migration(&conn, "planning_peers");
            let wave = WaveId::new();
            let project = ProjectId::new();
            let task = TaskId::new();
            conn.execute_batch(
                &include_str!("../../tests/fixtures/migrations/planning_task_evidence.sql")
                    .replace("{wave}", wave.as_str())
                    .replace("{project}", project.as_str())
                    .replace("{task}", task.as_str()),
            )
            .unwrap();
            conn.execute_batch(sql).unwrap();
            let retained: Vec<(String, String, i64)> = conn
                .prepare("SELECT 'issue:'||id,json_array(json_extract(body,'$.team_id'),json_extract(body,'$.project_id'),needs_refresh),observed_at FROM pm_items UNION ALL SELECT 'project:'||id,json_array(json_extract(body,'$.team_ids'),archived,membership_unresolved),observed_at FROM pm_projects ORDER BY 1")
                .unwrap()
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            upgrade_from_migration(&conn, "planning_peers");
            let store = SqliteStore {
                conn: Arc::new(Mutex::new(conn)),
            };
            let saved = store.task(&task).unwrap().unwrap();
            assert_eq!(saved.title, "Saved Task");
            assert_eq!(saved.description, "Saved brief");
            assert_eq!(
                store.project(&project).unwrap().unwrap().description,
                "Preserved prose"
            );
            assert_eq!(
                saved.worktree.as_deref(),
                Some(std::path::Path::new("/retained/checkout"))
            );
            assert!(store.task_by_issue("OTHER-2").unwrap().is_none());
            let acquisition = store.task_acquisition(&task).unwrap();
            assert_eq!(acquisition.state, expected, "{sql}");
            assert_eq!(
                acquisition.observed_at,
                (!sql.contains("DELETE FROM pm_items")).then_some(42),
                "{sql}"
            );
            assert_eq!(
                store.validate_task_planning(&saved).is_err(),
                matches!(expected, PlanningState::Invalid | PlanningState::Removed),
                "{sql}"
            );
            assert_eq!(store.task(&task).unwrap().unwrap(), saved);
            let conn = store.conn.lock().unwrap();
            let after: Vec<(String, String, i64)> = conn
                .prepare("SELECT 'issue:'||id,json_array(team_id,claimed_project_id,needs_refresh),observed_at FROM pm_items UNION ALL SELECT 'project:'||id,json_array(json(team_ids),archived,membership_unresolved),observed_at FROM pm_projects ORDER BY 1")
                .unwrap()
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
                .unwrap()
                .collect::<Result<_, _>>()
                .unwrap();
            assert_eq!(after, retained);
            drop(conn);
        }
    }

    #[test]
    fn ownership_reads_retained_claims_without_changing_work_or_observation_age() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let wave = crate::work::wave::Wave::new(WaveId::new(), "product".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        let mut snapshot: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        snapshot.projects.truncate(1);
        snapshot
            .items
            .retain(|item| item.project_id.as_deref() == Some(&snapshot.projects[0].id));
        snapshot.items.truncate(1);
        let project = snapshot.projects[0].clone();
        let item = snapshot.items[0].clone();
        store
            .seed_inventory(&crate::test_linear_input::InventoryFixture {
                wave_id: wave.id().clone(),
                provider: "linear".into(),
                initiative: project.initiative_ids[0].clone(),
                synced_at: 42,
                snapshot,
            })
            .unwrap();
        let task = store.task_by_issue(&item.id).unwrap().unwrap();
        let validate = || {
            store.validate_linear_ownership(
                &[(wave.id(), wave.slug())],
                Ok(Some(project.team_ids[0].clone())),
            )
        };
        validate().unwrap();
        {
            let conn = store.conn.lock().unwrap();
            // Foreign evidence blocks its Task, not the repository inventory.
            conn.execute("UPDATE pm_items SET team_id='foreign'", [])
                .unwrap();
        }
        validate().unwrap();
        assert!(store.validate_task_planning(&task).is_err());
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("UPDATE pm_projects SET team_ids='[\"foreign\"]'", [])
                .unwrap();
        }
        validate().unwrap();
        assert!(store.validate_task_planning(&task).is_err());
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("UPDATE pm_items SET needs_refresh=1", [])
                .unwrap();
        }
        validate().unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("UPDATE pm_items SET needs_refresh=0", [])
                .unwrap();
            conn.execute("UPDATE pm_projects SET membership_unresolved=1", [])
                .unwrap();
        }
        validate().unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE pm_projects SET membership_unresolved=0,archived=1",
                [],
            )
            .unwrap();
        }
        validate().unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE pm_projects SET archived=0,initiative_ids='[\"removed\"]'",
                [],
            )
            .unwrap();
        }
        validate().unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE pm_projects SET initiative_ids=?1",
                [serde_json::to_string(&project.initiative_ids).unwrap()],
            )
            .unwrap();
            conn.execute("INSERT INTO task_deletions(wave_id,issue_id,identifier,confirmed_at) VALUES(?1,?2,?3,50)",
                params![wave.id(),item.id,item.identifier]).unwrap();
        }
        validate().unwrap();
        assert_eq!(store.task(&task.id).unwrap().unwrap(), task);
        let conn = store.conn.lock().unwrap();
        let ages: (i64, i64) = conn.query_row(
            "SELECT p.observed_at,i.observed_at FROM pm_projects p JOIN pm_items i ON i.project_id=p.id",
            [], |row| Ok((row.get(0)?,row.get(1)?)),
        ).unwrap();
        assert_eq!(ages, (42, 42));
    }

    #[test]
    fn owned_issue_import_retains_identity_across_alias_changes_without_placement() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let wave = crate::work::wave::Wave::new(WaveId::new(), "product".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        let mut snapshot: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        let initiative = snapshot.projects[0].initiative_ids[0].clone();
        snapshot.projects.truncate(1);
        snapshot
            .items
            .retain(|item| item.project_id.as_deref() == Some(&snapshot.projects[0].id));
        snapshot.items.truncate(1);
        let mut row = crate::test_linear_input::InventoryFixture {
            wave_id: wave.id().clone(),
            provider: "linear".into(),
            initiative,
            synced_at: 42,
            snapshot,
        };
        store.seed_inventory(&row).unwrap();
        let item = &row.snapshot.items[0];
        let task = store.task_by_issue(&item.id).unwrap().unwrap();
        assert!(task.worktree.is_none());
        assert!(store.task_prs(&task.id).unwrap().is_empty());
        assert_eq!(task.title, item.name);
        assert_eq!(task.pm_snapshot_synced_at, Some(42));
        store.seed_inventory(&row).unwrap();
        assert_eq!(store.task_by_issue(&item.id).unwrap().unwrap().id, task.id);
        row.synced_at = 43;
        row.snapshot.items[0].identifier = "MOVED-42".into();
        row.snapshot.items[0].revision = Some("2099-01-01T00:00:00Z".into());
        store.seed_inventory(&row).unwrap();
        assert_eq!(
            store.task_by_issue("MOVED-42").unwrap().unwrap().id,
            task.id
        );
        // A detail acquisition refreshes content, never the order observed by a list.
        let mut detail = row.snapshot.items[0].clone();
        detail.name = "Updated on another machine".into();
        detail.rank = 99;
        detail.revision = Some("2099-01-02T00:00:00Z".into());
        store
            .seed_issue(
                "/repo",
                "linear",
                &crate::test_linear_input::IssueFixture {
                    item: detail.clone(),
                    project: Some(row.snapshot.projects[0].clone()),
                    observed_at: 44,
                },
                Some((wave.id(), &row.initiative)),
            )
            .unwrap();
        let refreshed = store.task(&task.id).unwrap().unwrap();
        assert_eq!(refreshed.title, detail.name);
        assert_eq!(refreshed.pm_snapshot_synced_at, Some(44));
        assert!(refreshed.worktree.is_none());
        assert!(store.task_prs(&task.id).unwrap().is_empty());
        assert_eq!(
            store.task(&task.id).unwrap().unwrap().rank,
            row.snapshot.items[0].rank
        );
        // Replaying the older list cannot roll content back or duplicate the Task.
        store.seed_inventory(&row).unwrap();
        assert_eq!(store.task(&task.id).unwrap().unwrap().title, detail.name);
        assert_eq!(
            store.task_acquisition(&task.id).unwrap().observed_at,
            Some(44)
        );
        store
            .observe_pm_issue_change(&detail.id, Some("2099-01-03T00:00:00Z"), false)
            .unwrap();
        store.seed_inventory(&row).unwrap();
        assert_eq!(
            store.task_acquisition(&task.id).unwrap(),
            crate::store::TaskAcquisition {
                state: PlanningState::Invalid,
                observed_at: Some(44),
            }
        );
        assert!(store.validate_task_planning(&refreshed).is_err());
        assert_eq!(store.task(&task.id).unwrap().unwrap(), refreshed);
        store
            .observe_pm_issue_change(&row.snapshot.items[0].id, None, true)
            .unwrap();
        store.seed_inventory(&row).unwrap();
        assert_eq!(
            store.task_by_issue("MOVED-42").unwrap().unwrap().id,
            task.id
        );
        let conn = store.conn.lock().unwrap();
        assert_eq!(
            conn.query_row("SELECT count(*) FROM tasks", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM task_ownership", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn project_name_cutover_retains_both_histories_and_rejects_later_conflicts() {
        // "Customer requests" represents both a projected prefixed name and a
        // verbatim plain provider name. The same input must accept both histories.
        // An already exact prefixed name also consumes the exception once.
        for historical_name in ["Customer requests", "Product — Customer requests"] {
            let mut conn = Connection::open_in_memory().unwrap();
            crate::store::migrations::apply_before_migration(&conn, "planning_peers");
            let snapshot: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
                "../../../../tests/fixtures/dto/task_history_planning.json"
            ))
            .unwrap();
            let mut project = snapshot.projects[0].clone();
            project.revision = Some("2026-10-05T12:00:00Z".into());
            project.name = historical_name.into();
            project.slug = crate::linear::project_slug(historical_name);
            let original = serde_json::to_string(&project).unwrap();
            conn.execute(
                "INSERT INTO pm_projects(repo,provider,id,observed_at,body)
                VALUES('/repo','linear',?1,10,?2)",
                params![project.id, original],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO pm_project_name_cutover(repo,provider,id,body,observed_at,archived,membership_unresolved)
                 VALUES('/repo','linear',?1,?2,10,0,0)",
                params![project.id, original],
            ).unwrap();
            conn.execute_batch(&crate::store::migrations::migration_section_sql(
                "planning_peers",
            ))
            .unwrap();
            project.name = "Product — Customer requests".into();
            project.slug = crate::linear::project_slug(&project.name);

            let mut conflict = project.clone();
            conflict.summary.push_str("conflict");
            assert!(super::put_project(&conn, "/repo", 11, &conflict).is_err());
            super::put_project(&conn, "/repo", 9, &project).unwrap();
            let retained: String = conn
                .query_row("SELECT name FROM pm_projects", [], |r| r.get(0))
                .unwrap();
            assert_eq!(retained, historical_name);
            // Failed outer work cannot spend the one-time exception.
            {
                let tx = conn.transaction().unwrap();
                super::put_project(&tx, "/repo", 11, &project).unwrap();
            }
            let pending: Option<i64> = conn
                .query_row(
                    "SELECT converted_at FROM pm_project_name_cutover",
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(pending, None);
            let tx = conn.transaction().unwrap();
            super::put_project(&tx, "/repo", 12, &project).unwrap();
            tx.commit().unwrap();
            let evidence: (String, i64, Option<i64>) = conn
                .query_row(
                    "SELECT body,observed_at,converted_at FROM pm_project_name_cutover",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap();
            assert_eq!(evidence, (original, 10, Some(12)));
            let accepted = project.name.clone();
            project.name = "Another name".into();
            assert!(super::put_project(&conn, "/repo", 13, &project).is_err());
            let retained: String = conn
                .query_row("SELECT name FROM pm_projects", [], |r| r.get(0))
                .unwrap();
            assert_eq!(retained, accepted);
            project.id = "post-cutover".into();
            super::put_project(&conn, "/repo", 13, &project).unwrap();
            project.name = "Conflict on a new record".into();
            assert!(super::put_project(&conn, "/repo", 14, &project).is_err());
        }
    }

    #[test]
    fn confirming_refresh_uses_released_claim_instead_of_membership_index() {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::migrations::apply_before_migration(&conn, "planning_peers");
        let snapshot: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        let item = &snapshot.items[0];
        conn.execute(
            "INSERT INTO pm_items(repo,provider,id,identifier,project_id,observed_at,body)
             VALUES('/repo','linear',?1,?2,'contradictory-index',17,?3)",
            params![
                item.id,
                item.identifier,
                serde_json::to_string(item).unwrap()
            ],
        )
        .unwrap();
        conn.execute_batch(&crate::store::migrations::migration_section_sql(
            "planning_peers",
        ))
        .unwrap();
        assert!(!super::put_item(&conn, "/repo", 16, item).unwrap());
        let read = || {
            conn.query_row(
                "SELECT project_id,claimed_project_id,observed_at FROM pm_items",
                [],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .unwrap()
        };
        assert_eq!(
            read(),
            (
                Some("contradictory-index".into()),
                item.project_id.clone(),
                17
            )
        );
        assert!(super::put_item(&conn, "/repo", 18, item).unwrap());
        assert_eq!(
            read(),
            (item.project_id.clone(), item.project_id.clone(), 18)
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM tasks", [], |row| row.get::<_, i64>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn task_history_enriches_only_absent_completion_dates_at_equal_revision() {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::migrations::apply_before_migration(&conn, "planning_peers");
        let planning: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        let mut item = planning
            .items
            .iter()
            .find(|item| item.id == "recent")
            .unwrap()
            .clone();
        let mut old = serde_json::to_value(&item).unwrap();
        old.as_object_mut().unwrap().remove("completed_at");
        conn.execute(
            "INSERT INTO pm_items(repo,provider,id,identifier,project_id,observed_at,body) VALUES('/repo','linear',?1,?2,?3,1,?4)",
            params![item.id, item.identifier, item.project_id, old.to_string()],
        )
        .unwrap();
        conn.execute_batch(&crate::store::migrations::migration_section_sql(
            "planning_peers",
        ))
        .unwrap();
        assert!(super::put_item(&conn, "/repo", 2, &item).unwrap());
        let observed: Option<String> = conn
            .query_row("SELECT completed_at FROM pm_items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(observed, item.completed_at);
        item.completed_at = None;
        assert!(super::put_item(&conn, "/repo", 3, &item).is_err());
        item.revision = Some("2026-10-01T12:00:00Z".into());
        assert!(!super::put_item(&conn, "/repo", 4, &item).unwrap());
        item.revision = Some("2026-10-03T12:00:00Z".into());
        assert!(super::put_item(&conn, "/repo", 5, &item).unwrap());
        item.completed_at = Some("2026-10-01T12:00:00Z".into());
        assert!(super::put_item(&conn, "/repo", 6, &item).is_err());
    }

    #[test]
    fn due_dates_enrich_absent_history_but_preserve_observed_null_at_equal_revision() {
        let conn = Connection::open_in_memory().unwrap();
        crate::store::migrations::apply_before_migration(&conn, "planning_peers");
        let planning: crate::test_linear_input::Inventory = serde_json::from_str(include_str!(
            "../../../../tests/fixtures/dto/task_history_planning.json"
        ))
        .unwrap();
        let mut item = planning.items[0].clone();
        let mut old = serde_json::to_value(&item).unwrap();
        old.as_object_mut().unwrap().remove("due_date");
        conn.execute(
            "INSERT INTO pm_items(repo,provider,id,identifier,project_id,observed_at,body) VALUES('/repo','linear',?1,?2,?3,1,?4)",
            params![item.id, item.identifier, item.project_id, old.to_string()],
        )
        .unwrap();
        crate::store::migrations::upgrade_from_migration(&conn, "planning_peers");
        item.due_date = Some("2026-10-08".into());
        assert!(super::put_item(&conn, "/repo", 2, &item).unwrap());
        let observed: Option<String> = conn
            .query_row("SELECT due_date FROM pm_items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(observed, item.due_date);
        item.due_date = None;
        assert!(super::put_item(&conn, "/repo", 3, &item).is_err());
        item.id = "observed-null".into();
        assert!(super::put_item(&conn, "/repo", 4, &item).unwrap());
        item.due_date = Some("2026-10-09".into());
        assert!(super::put_item(&conn, "/repo", 5, &item).is_err());
    }
}
