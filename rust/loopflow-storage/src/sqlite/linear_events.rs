//! Accept supplied Linear facts and canonical Work in one SQL transaction.

use super::tracker_actions::{self, ActionKind, Resolution, TrackerAction};
use super::{self as sqlite, planning_write, SqliteStore};
use crate::planning::{revision_nanos, TrackerItem, TrackerProject};
use crate::planning_sync::TrackerKind;
use crate::work::edit::{Disposition, WorkValue as Edit};
use crate::{StoreError, StoreResult};
use loopflow_model::durable::ProjectId;
use loopflow_model::id::WaveId;
use rusqlite::{named_params, params, Connection, OptionalExtension, Transaction};

/// Accept a tracker Project as `wave`'s: the Wave's Project links it.
pub fn accept_linear_project_in(
    store: &SqliteStore,
    wave: &WaveId,
    project: &TrackerProject,
    observed_at: i64,
    tracker: TrackerKind,
) -> StoreResult<()> {
    let tracker = tracker.as_str();
    let mut conn = store.conn.lock().expect("store mutex poisoned");
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let repo: String = tx.query_row("SELECT repo FROM waves WHERE id=?1", [wave], |row| {
        row.get(0)
    })?;
    put_project(&tx, &repo, observed_at, project, tracker)?;
    project_accepted_planning(&tx, &repo, &[project.id.as_str()], &[], Some(wave), tracker)?;
    tx.commit()?;
    Ok(())
}

/// Link `wave`'s Project to a tracker Project that already exists. The
/// Project's own unsent `create` is retired: the existing one stands in for
/// it. A Project linked elsewhere moves its link while none of its Tasks is in
/// the tracker yet; the tracker Project it leaves stays in the tracker.
pub fn link_wave_project_in(
    store: &SqliteStore,
    wave: &WaveId,
    project: &TrackerProject,
    observed_at: i64,
    tracker: TrackerKind,
) -> StoreResult<()> {
    let tracker = tracker.as_str();
    let mut conn = store.conn.lock().expect("store mutex poisoned");
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let repo: String = tx.query_row("SELECT repo FROM waves WHERE id=?1", [wave], |row| {
        row.get(0)
    })?;
    if let Some(own) = sqlite::children::held_wave_project_in(&tx, wave)? {
        let (linked, creating, tracked): (Option<String>, bool, i64) = tx.query_row(
            "SELECT external_project_id,
                EXISTS(SELECT 1 FROM tracker_actions WHERE project_id=?1 AND kind='create'
                    AND phase IN ('sent','conflicted')),
                (SELECT count(*) FROM tasks WHERE project_id=?1 AND external_issue_id IS NOT NULL)
             FROM projects WHERE id=?1",
            [own.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        if creating {
            return Err(StoreError::InvalidData(format!(
                "Wave {wave}'s Project is being created in the tracker; link it once that lands"
            )));
        }
        if let Some(linked) = linked.filter(|linked| *linked != project.id) {
            if tracked > 0 {
                return Err(StoreError::InvalidData(format!(
                    "Wave {wave}'s Project links tracker Project {linked}, which holds {tracked} of its Tasks; move them to {} in the tracker first",
                    project.id
                )));
            }
            tx.execute(
                "UPDATE projects SET external_project_id=NULL WHERE id=?1",
                [own.as_str()],
            )?;
        }
        tx.execute(
            "UPDATE tracker_actions SET phase='retired',error='linked to an existing tracker Project'
             WHERE project_id=?1 AND kind='create' AND phase='queued'",
            [own.as_str()],
        )?;
    }
    put_project(&tx, &repo, observed_at, project, tracker)?;
    project_accepted_planning(&tx, &repo, &[project.id.as_str()], &[], Some(wave), tracker)?;
    tx.commit()?;
    Ok(())
}

pub fn reconcile_linear_project_teams_in(
    store: &SqliteStore,
    wave: &WaveId,
    project: &TrackerProject,
    observed_at: i64,
) -> StoreResult<()> {
    // Linear-only: Teams and ordered issues.
    let tracker = TrackerKind::Linear.as_str();
    let mut conn = store.conn.lock().expect("store mutex poisoned");
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let repo: String = tx.query_row("SELECT repo FROM waves WHERE id=?1", [wave], |row| {
        row.get(0)
    })?;
    // Reteam orders Team relationships only. Retain independently newer entity facts.
    tx.execute(
        "UPDATE tracker_projects SET team_ids=?4 WHERE repo=?1 AND tracker=?2 AND id=?3",
        params![
            repo,
            tracker,
            project.id,
            serde_json::to_string(&project.team_ids)?
        ],
    )?;
    put_project(&tx, &repo, observed_at, project, tracker)?;
    project_accepted_planning(&tx, &repo, &[project.id.as_str()], &[], Some(wave), tracker)?;
    tx.commit()?;
    Ok(())
}

pub fn accept_linear_issue_in(
    store: &SqliteStore,
    repo: &str,
    issue: &TrackerItem,
    project: Option<&TrackerProject>,
    observed_at: i64,
    tracker: TrackerKind,
) -> StoreResult<()> {
    let tracker = tracker.as_str();
    let mut conn = store.conn.lock().expect("store mutex poisoned");
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let mut item = issue.clone();
    // Detail queries do not observe relative list order.
    if let Some(rank) = tx.query_row(
        "SELECT rank FROM tracker_items WHERE repo=?1 AND tracker=?2 AND id=?3 AND project_id IS ?4",
        params![repo,tracker,item.id,item.project_id], |row| row.get::<_,u32>(0),
    ).optional()? {
        item.rank = rank;
    }
    // Project revisions are independent of the issue revision and request age.
    if let Some(project) = project {
        put_project(&tx, repo, observed_at, project, tracker)?;
    }
    if put_item(&tx, repo, observed_at, &item, tracker)? {
        tx.execute(
            "UPDATE tracker_items SET needs_refresh=0 WHERE repo=?1 AND tracker=?2 AND id=?3",
            params![repo, tracker, issue.id],
        )?;
    }
    project_accepted_planning(
        &tx,
        repo,
        project.map(|project| project.id.as_str()).as_slice(),
        &[issue.id.as_str()],
        None,
        tracker,
    )?;
    tx.commit()?;
    Ok(())
}

/// A Wave's whole tracker inventory: the Project its Project links and that
/// Project's issues.
pub fn accept_linear_inventory_in(
    store: &SqliteStore,
    wave: &WaveId,
    projects: &[TrackerProject],
    issues: &[TrackerItem],
    observed_at: i64,
) -> StoreResult<()> {
    // Linear-only: Teams and ordered issues.
    let tracker = TrackerKind::Linear.as_str();
    let mut conn = store.conn.lock().expect("store mutex poisoned");
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let repo: String = tx.query_row("SELECT repo FROM waves WHERE id=?1", [wave], |row| {
        row.get(0)
    })?;
    for project in projects {
        put_project(&tx, &repo, observed_at, project, tracker)?;
    }
    for item in issues {
        put_item(&tx, &repo, observed_at, item, tracker)?;
    }
    tx.execute(
        "INSERT INTO tracker_wave_sync(wave_id,tracker,initiative,synced_at) VALUES(?1,?2,'',?3)
         ON CONFLICT(wave_id) DO UPDATE SET tracker=excluded.tracker,
         synced_at=MAX(tracker_wave_sync.synced_at,excluded.synced_at)",
        params![wave, tracker, observed_at],
    )?;
    project_accepted_planning(
        &tx,
        &repo,
        &projects.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(),
        &issues.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        Some(wave),
        tracker,
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
            tracker,
        )?;
    }
    tx.commit()?;
    Ok(())
}

pub fn accept_linear_project_order_in(
    store: &SqliteStore,
    repo: &str,
    project: &str,
    items: &[TrackerItem],
) -> StoreResult<bool> {
    // Linear-only: Teams and ordered issues.
    let tracker = TrackerKind::Linear.as_str();
    let mut conn = store.conn.lock().expect("store mutex poisoned");
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    let now = crate::rows::now_unix();
    for item in items {
        put_item(&tx, repo, now, item, tracker)?;
    }
    project_accepted_planning(
        &tx,
        repo,
        &[],
        &items.iter().map(|i| i.id.as_str()).collect::<Vec<_>>(),
        None,
        tracker,
    )?;
    let accepted = observe_order(&tx, repo, project, items, tracker)?;
    tx.commit()?;
    Ok(accepted)
}

fn put_project(
    conn: &Connection,
    repo: &str,
    observed_at: i64,
    project: &TrackerProject,
    tracker: &str,
) -> StoreResult<()> {
    let team_ids = serde_json::to_string(&project.team_ids)?;
    let status = project.status.as_str();
    let values = named_params! {
        ":repo": repo, ":tracker": tracker, ":id": &project.id,
        ":observed_at": observed_at,
        ":slug": &project.slug,
        ":name": &project.name,
        ":summary": &project.summary,
        ":status": &status,
        ":team_ids": &team_ids,
        ":description": &project.description,
        ":revision": &project.revision,
    };
    let revision = revision_nanos(project.revision.as_deref())?;
    let previous = conn
        .query_row(
            "SELECT revision,observed_at,archived FROM tracker_projects
             WHERE repo=?1 AND tracker=?2 AND id=?3",
            params![repo, tracker, project.id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, bool>(2)?,
                ))
            },
        )
        .optional()?;
    if let Some((previous, acquired, archived)) = previous {
        // Reads order like issues: by revision, then by read time. A read
        // without a revision sorts below one that has it.
        if archived || (revision, observed_at) < (revision_nanos(previous.as_deref())?, acquired) {
            return Ok(());
        }
    }
    conn.execute(
        "INSERT INTO tracker_projects(repo,tracker,id,observed_at,slug,name,summary,status,team_ids,description,revision,description_revision,description_observed_at)
         VALUES(:repo,:tracker,:id,:observed_at,:slug,:name,:summary,:status,:team_ids,:description,:revision,
             CASE WHEN :description IS NOT NULL THEN :revision END,
             CASE WHEN :description IS NOT NULL THEN :observed_at END)
         ON CONFLICT(repo,tracker,id) DO UPDATE SET
         observed_at=excluded.observed_at,revision=excluded.revision,
         slug=excluded.slug,name=excluded.name,summary=excluded.summary,status=excluded.status,team_ids=excluded.team_ids,
         description=COALESCE(excluded.description,tracker_projects.description),
         description_revision=CASE WHEN excluded.description IS NOT NULL THEN excluded.revision ELSE description_revision END,
         description_observed_at=COALESCE(excluded.description_observed_at,description_observed_at)", values,
    )?;
    Ok(())
}

fn put_item(
    conn: &Connection,
    repo: &str,
    observed_at: i64,
    item: &TrackerItem,
    tracker: &str,
) -> StoreResult<bool> {
    let revision = revision_nanos(item.revision.as_deref())?;
    let change: Option<(Option<i64>, bool)> = conn
        .query_row(
            "SELECT revision_ns,removed FROM tracker_events WHERE issue_id=?1",
            [&item.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if change.is_some_and(|(floor, removed)| removed || revision < floor) {
        return Ok(false);
    }
    let previous = conn
        .query_row(
            "SELECT revision,observed_at,state IS NOT :state OR completed IS NOT :completed
             FROM tracker_items WHERE repo=:repo AND tracker=:tracker AND id=:id",
            named_params! { ":repo": repo, ":tracker": tracker, ":id": &item.id,
                ":completed": &item.completed,
                ":state": &item.state,
            },
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, bool>(2)?,
                ))
            },
        )
        .optional()?;
    let mut status_changed = previous.is_none();
    if let Some((previous, acquired, changed)) = previous {
        // Linear changes some fields without moving `updatedAt`, so one revision
        // can carry different facts: reads order by revision, then by read time.
        if (revision, observed_at) < (revision_nanos(previous.as_deref())?, acquired) {
            return Ok(false);
        }
        status_changed = changed;
    }
    conn.execute(
        "INSERT INTO tracker_items(repo,tracker,id,observed_at,identifier,project_id,revision,branch_name,url,name,description,rank,completed,completed_at,due_date,state,team_id,assignee,claimed_project_id,completed_at_observed,due_date_observed,project_id_observed,assignee_observed)
         VALUES(:repo,:tracker,:id,:observed_at,:identifier,:project_id,:revision,:branch_name,:url,:name,:description,:rank,:completed,:completed_at,:due_date,:state,:team_id,:assignee,:project_id,1,1,1,1)
         ON CONFLICT(repo,tracker,id) DO UPDATE SET observed_at=excluded.observed_at,
         identifier=excluded.identifier,project_id=excluded.project_id,revision=excluded.revision,branch_name=excluded.branch_name,url=excluded.url,name=excluded.name,description=excluded.description,rank=excluded.rank,completed=excluded.completed,completed_at=excluded.completed_at,due_date=excluded.due_date,state=excluded.state,team_id=excluded.team_id,assignee=excluded.assignee,
         claimed_project_id=excluded.claimed_project_id,completed_at_observed=1,due_date_observed=1,project_id_observed=1,assignee_observed=1",
        named_params! { ":repo": repo, ":tracker": tracker, ":id": &item.id,
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

fn observe_order(
    conn: &Connection,
    repo: &str,
    external: &str,
    items: &[TrackerItem],
    tracker: &str,
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
             FROM tracker_items i JOIN tasks t ON t.external_issue_id=i.id
             WHERE i.repo=?1 AND i.tracker=?2 AND i.id=?3 AND i.needs_refresh=0 AND t.project_id=?4 AND t.planning_deleted_at IS NULL",
            params![repo,tracker,item.id,project.as_str()], |row| Ok((
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

// Both durable projections read the tracker Projects a Wave's Project links,
// or the one being linked to Wave ?4.
const ACCEPTED_WAVE_PROJECTS: &str = "
    SELECT p.repo,p.tracker,p.id,p.observed_at,p.archived,p.task_order_json,p.revision,p.slug,p.name,p.summary,p.status,p.team_ids,p.description,p.description_revision,p.description_observed_at,w.id AS wave_id FROM tracker_projects p
    LEFT JOIN projects l ON l.external_project_id=p.id
    JOIN waves w ON w.id=COALESCE(?4,l.wave_id) AND w.repo=p.repo
    WHERE p.repo=?1 AND p.tracker=?2 AND p.archived=0";

/// Inputs select IDs only; project their accepted rows inside the ingestion transaction.
fn project_accepted_planning(
    tx: &Transaction<'_>,
    repo: &str,
    projects: &[&str],
    items: &[&str],
    linking: Option<&WaveId>,
    tracker: &str,
) -> StoreResult<()> {
    let project_ids = serde_json::to_string(projects)?;
    let item_ids = serde_json::to_string(items)?;
    let mut query = tx.prepare(&format!(
        "WITH accepted AS ({ACCEPTED_WAVE_PROJECTS})
         SELECT id,observed_at,wave_id,revision,
             slug,name,
             summary,json_quote(status),
             team_ids,
             CASE WHEN description_revision IS revision THEN description END
         FROM accepted WHERE id IN (SELECT value FROM json_each(?3))"
    ))?;
    let observations = query
        .query_and_then(
            params![repo, tracker, project_ids, linking],
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
                    Edit::ProjectTeams(serde_json::from_str(&row.get::<_, String>(8)?)?),
                ];
                if let Some(description) = row.get::<_, Option<String>>(9)? {
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
        tracker_actions::attach_in(tx, repo, true, &external, revision.as_deref(), &edits)?;
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
            // A Wave is exactly one Project. A tracker Project no Wave links
            // becomes the Wave's when its Project links none yet, unless that
            // Project's own `create` is out and this is not it.
            None => {
                let own = sqlite::children::held_wave_project_in(tx, &wave_id)?
                    .unwrap_or_else(|| ProjectId::for_wave(&wave_id));
                let (linked, creating): (bool, bool) = tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM projects WHERE id=?1 AND external_project_id IS NOT NULL),
                            EXISTS(SELECT 1 FROM tracker_actions WHERE project_id=?1 AND kind='create' AND phase IN ('queued','sent','conflicted'))",
                        [own.as_str()],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )?;
                if linked || (creating && !own.made_as(&external)) {
                    continue;
                }
                own
            }
        };
        tracker_actions::reconcile_in(
            tx,
            &tracker_actions::ActionOwner::Project(id.clone()),
            revision.as_deref(),
            &mut edits,
        )?;
        planning_write::create(
            tx,
            repo,
            &crate::work::edit::WorkEdit::for_target(id.as_str(), &edits),
            crate::sqlite::planning_write::WriteOrigin::Tracker,
        )?;
        tx.execute("UPDATE projects SET pm_snapshot_synced_at=?2,planning_provider_revision=?3 WHERE id=?1",params![id.as_str(),observed_at,revision])?;
    }
    let mut query = tx.prepare(
        "SELECT i.id,i.revision,i.name,i.description,i.assignee,p.id,i.assignee_observed,i.project_id_observed,i.claimed_project_id
         FROM tracker_items i LEFT JOIN projects p ON p.external_project_id=i.project_id
         WHERE i.repo=?1 AND i.tracker=?3 AND i.needs_refresh=0
         AND i.id IN (SELECT value FROM json_each(?2))",
    )?;
    let observed = query
        .query_map(params![repo, item_ids, tracker], |row| {
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
        tracker_actions::attach_in(tx, repo, false, &id, revision.as_deref(), &edits)?;
    }
    // Admission and refresh share one accepted observation. Existing Tasks retain
    // their IDs, local order and pending disposition; new Tasks receive only Work.
    let mut query = tx.prepare(&format!(
        "WITH accepted AS ({ACCEPTED_WAVE_PROJECTS})
         SELECT target.id,NULL,i.observed_at,p.id,i.id,
             i.revision,i.identifier,
             i.name,i.description,
             i.rank,i.url,
             i.branch_name,i.team_id,
             i.assignee,i.due_date,
             i.state,i.completed,
             i.completed_at FROM tracker_items i
         LEFT JOIN projects p ON p.external_project_id=i.project_id
         LEFT JOIN accepted observed ON observed.id=i.project_id AND observed.wave_id=p.wave_id
         LEFT JOIN tasks target ON target.external_issue_id=i.id
         LEFT JOIN projects current ON current.id=target.project_id
         WHERE i.repo=?1 AND i.tracker=?2 AND i.needs_refresh=0
         AND i.id IN (SELECT value FROM json_each(?3))
         AND ((observed.id IS NOT NULL AND observed.wave_id=p.wave_id)
              OR (i.project_id_observed=1 AND i.claimed_project_id IS NULL))
         AND (target.id IS NULL OR target.repository_id=(SELECT id FROM repository_plans WHERE repo=?1 AND selected=1))
         AND NOT EXISTS(SELECT 1 FROM tracker_events c WHERE c.issue_id=i.id
             AND c.removed=1)"
    ))?;
    let observations = query
        .query_map(params![repo, tracker, item_ids, linking], |row| {
            Ok((
                row.get::<_, Option<String>>(0)?,
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
        })?
        .collect::<Result<Vec<_>, _>>()?;
    drop(query);
    for (existing, observed_at, external, revision, mut edits, disposition) in observations {
        let id = existing.as_ref().map_or_else(
            || loopflow_model::durable::TaskId::from_issue(&external),
            loopflow_model::durable::TaskId::from_raw,
        );
        edits.push(Edit::Disposition(disposition.clone()));
        if existing.is_some() {
            let resolved = tracker_actions::reconcile_in(
                tx,
                &tracker_actions::ActionOwner::Task(id.clone()),
                revision.as_deref(),
                &mut edits,
            )?;
            record_adoptions(tx, &id, &resolved)?;
            edits.retain(|edit| !matches!(edit, Edit::TaskRank(_)));
        }
        planning_write::create(
            tx,
            repo,
            &crate::work::edit::WorkEdit::for_target(id.as_str(), &edits),
            crate::sqlite::planning_write::WriteOrigin::Tracker,
        )?;
        tx.execute(
            "UPDATE tasks SET planning_provider_revision=?2,pm_snapshot_synced_at=?3 WHERE id=?1",
            params![id.as_str(), revision, observed_at],
        )?;
    }
    Ok(())
}

/// What acceptance resolved against local intent and the Task shows for it: an
/// adopted state is a Task event; a removal the tracker undid restores the Task.
fn record_adoptions(
    tx: &Transaction<'_>,
    task: &loopflow_model::durable::TaskId,
    resolved: &[(TrackerAction, Resolution)],
) -> StoreResult<()> {
    for (action, resolution) in resolved {
        match (action.kind, resolution) {
            (ActionKind::State, Resolution::Conflicted { remote }) => {
                let state = |value: &serde_json::Value| {
                    value["value"]["planning_state"]
                        .as_str()
                        .unwrap_or("unknown")
                        .to_owned()
                };
                sqlite::children::insert_task_event_in(
                    tx,
                    task,
                    &crate::work::task::TaskEventKind::Progress {
                        summary: format!(
                            "Adopted {} state {}; local intention {} remains in action {}. Local Workflow unchanged.",
                            action.tracker.display_name(),
                            state(&remote["value"]),
                            state(&action.value),
                            action.id
                        ),
                    },
                )?;
            }
            (ActionKind::Delete, Resolution::Retire { .. }) => {
                planning_write::tracker(
                    tx,
                    &crate::work::edit::WorkEdit::for_target(
                        task.as_str(),
                        &[Edit::TaskDeletedAt(None)],
                    ),
                )?;
            }
            _ => {}
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::planning::{TrackerItem, TrackerProject};
    use rusqlite::{params, Connection};

    #[derive(Debug, serde::Deserialize)]
    struct Inventory {
        projects: Vec<TrackerProject>,
        items: Vec<TrackerItem>,
    }

    #[test]
    fn project_reads_at_one_revision_keep_the_later_one() {
        // "Customer requests" stands for both a projected prefixed name and a
        // verbatim plain tracker name; Linear may re-render either at the same
        // `updatedAt`.
        for historical_name in ["Customer requests", "Product — Customer requests"] {
            let conn = Connection::open_in_memory().unwrap();
            crate::migration_runner::apply_before_migration(
                &crate::test_fixture::observers(),
                &crate::test_fixture::build(),
                &conn,
                "planning_peers",
            );
            let snapshot: Inventory = serde_json::from_str(include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../tests/fixtures/dto/task_history_planning.json"
            )))
            .unwrap();
            let mut project = snapshot.projects[0].clone();
            project.revision = Some("2026-10-05T12:00:00Z".into());
            project.name = historical_name.into();
            project.slug = crate::planning::project_slug(historical_name);
            conn.execute(
                "INSERT INTO pm_projects(repo,provider,id,observed_at,body)
                VALUES('/repo','linear',?1,10,?2)",
                params![project.id, serde_json::to_string(&project).unwrap()],
            )
            .unwrap();
            crate::migration_runner::upgrade_from_migration(
                &crate::test_fixture::observers(),
                &crate::test_fixture::build(),
                &conn,
                "planning_peers",
            );
            let stored = |conn: &Connection| -> (String, String, Option<String>) {
                conn.query_row(
                    "SELECT name,summary,description FROM tracker_projects",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap()
            };
            let original = stored(&conn);

            project.name = "Product — Customer requests".into();
            project.slug = crate::planning::project_slug(&project.name);
            project.summary.push_str(" re-rendered");
            super::put_project(&conn, "/repo", 9, &project, "linear").unwrap();
            assert_eq!(stored(&conn), original, "an earlier read is ignored");

            project.description = None;
            super::put_project(&conn, "/repo", 11, &project, "linear").unwrap();
            assert_eq!(
                stored(&conn),
                (
                    project.name.clone(),
                    project.summary.clone(),
                    original.2.clone()
                ),
                "a later read wins; a read without a description keeps the stored one"
            );

            project.name = "Another name".into();
            super::put_project(&conn, "/repo", 12, &project, "linear").unwrap();
            assert_eq!(stored(&conn).0, "Another name");

            project.revision = None;
            project.name = "Unrevisioned".into();
            super::put_project(&conn, "/repo", 13, &project, "linear").unwrap();
            assert_eq!(
                stored(&conn).0,
                "Another name",
                "a revisioned read outranks one without"
            );
        }
    }

    #[test]
    fn confirming_refresh_uses_released_claim_instead_of_membership_index() {
        let conn = Connection::open_in_memory().unwrap();
        crate::migration_runner::apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "planning_peers",
        );
        let snapshot: Inventory = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/dto/task_history_planning.json"
        )))
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
        crate::migration_runner::upgrade_from_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "planning_peers",
        );
        assert!(!super::put_item(&conn, "/repo", 16, item, "linear").unwrap());
        let read = || {
            conn.query_row(
                "SELECT project_id,claimed_project_id,observed_at FROM tracker_items",
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
        assert!(super::put_item(&conn, "/repo", 18, item, "linear").unwrap());
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
    fn task_history_enriches_absent_completion_dates_and_follows_later_reads() {
        let conn = Connection::open_in_memory().unwrap();
        crate::migration_runner::apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "planning_peers",
        );
        let planning: Inventory = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/dto/task_history_planning.json"
        )))
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
        crate::migration_runner::upgrade_from_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "planning_peers",
        );
        assert!(super::put_item(&conn, "/repo", 2, &item, "linear").unwrap());
        let observed: Option<String> = conn
            .query_row("SELECT completed_at FROM tracker_items", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(observed, item.completed_at);
        item.completed_at = None;
        assert!(super::put_item(&conn, "/repo", 3, &item, "linear").unwrap());
        item.revision = Some("2026-10-01T12:00:00Z".into());
        assert!(!super::put_item(&conn, "/repo", 4, &item, "linear").unwrap());
        item.revision = Some("2026-10-03T12:00:00Z".into());
        assert!(super::put_item(&conn, "/repo", 5, &item, "linear").unwrap());
        item.completed_at = Some("2026-10-01T12:00:00Z".into());
        assert!(super::put_item(&conn, "/repo", 6, &item, "linear").unwrap());
        let observed: Option<String> = conn
            .query_row("SELECT completed_at FROM tracker_items", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(observed, item.completed_at);
    }

    #[test]
    fn due_dates_enrich_absent_history_and_follow_later_reads_at_equal_revision() {
        let conn = Connection::open_in_memory().unwrap();
        crate::migration_runner::apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "planning_peers",
        );
        let planning: Inventory = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/dto/task_history_planning.json"
        )))
        .unwrap();
        let mut item = planning.items[0].clone();
        let mut old = serde_json::to_value(&item).unwrap();
        old.as_object_mut().unwrap().remove("due_date");
        conn.execute(
            "INSERT INTO pm_items(repo,provider,id,identifier,project_id,observed_at,body) VALUES('/repo','linear',?1,?2,?3,1,?4)",
            params![item.id, item.identifier, item.project_id, old.to_string()],
        )
        .unwrap();
        crate::migration_runner::upgrade_from_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "planning_peers",
        );
        item.due_date = Some("2026-10-08".into());
        assert!(super::put_item(&conn, "/repo", 2, &item, "linear").unwrap());
        let observed: Option<String> = conn
            .query_row("SELECT due_date FROM tracker_items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(observed, item.due_date);
        item.due_date = None;
        assert!(super::put_item(&conn, "/repo", 3, &item, "linear").unwrap());
        let observed: Option<String> = conn
            .query_row("SELECT due_date FROM tracker_items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(observed, None);
    }

    #[test]
    fn shortened_branch_name_at_the_same_revision_replaces_the_stored_one() {
        // Between 2026-11-06 and 2026-11-08 Linear cut `branchName` to 60
        // characters without moving `updatedAt`; LOO-403's read then wedged.
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("store.db")).unwrap();
        let conn = store.conn.lock().unwrap();
        let planning: Inventory = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../tests/fixtures/dto/task_history_planning.json"
        )))
        .unwrap();
        let mut item = planning.items[0].clone();
        let long = "jack/loo-403-make-the-mac-apps-shortcuts-findable-and-put-commands-in-the";
        item.branch_name = Some(long.into());
        assert!(super::put_item(&conn, "/repo", 1, &item, "linear").unwrap());
        let branch = || -> Option<String> {
            conn.query_row("SELECT branch_name FROM tracker_items", [], |row| {
                row.get(0)
            })
            .unwrap()
        };
        item.branch_name = Some("jack/loo-403-make-the-mac-apps-shortcuts-findable-and-put".into());
        assert!(super::put_item(&conn, "/repo", 2, &item, "linear").unwrap());
        assert_eq!(branch(), item.branch_name);
        // A read that started before the stored one cannot bring the old name back.
        let mut delayed = item.clone();
        delayed.branch_name = Some(long.into());
        assert!(!super::put_item(&conn, "/repo", 1, &delayed, "linear").unwrap());
        assert_eq!(branch(), item.branch_name);
    }
}
