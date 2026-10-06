use rusqlite::{params, Connection, OptionalExtension, Transaction};

use crate::id::WaveId;
use crate::pm::{PmItem, PmProject, PmSnapshot};
use crate::store::sqlite::SqliteStore;
use crate::store::{
    PlanningState, PmSnapshotRow, PmTaskObservation, PmTaskRecord, StoreError, StoreResult,
};
use crate::work::project::ProjectId;

impl SqliteStore {
    /// Accept one confirmed Project without claiming a complete Wave refresh.
    pub fn put_pm_project(
        &self,
        wave: &WaveId,
        provider: &str,
        initiative: &str,
        project: &PmProject,
        observed_at: i64,
    ) -> StoreResult<PmProject> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
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
        validate_project_membership(&conn, &repo, provider, project)?;
        let tx = conn.transaction()?;
        put_project(&tx, &repo, provider, observed_at, project)?;
        let accepted = associate_project(&tx, &repo, provider, &project.id, wave, initiative)?;
        project_accepted_planning(
            &tx,
            &repo,
            provider,
            std::slice::from_ref(project),
            &[],
            Some((wave, initiative)),
        )?;
        tx.commit()?;
        Ok(accepted)
    }

    pub(crate) fn reconcile_pm_project_teams(
        &self,
        wave: &WaveId,
        provider: &str,
        initiative: &str,
        project: &PmProject,
        observed_at: i64,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
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
        let tx = conn.transaction()?;
        let previous: Option<String> = tx
            .query_row(
                "SELECT body FROM pm_projects WHERE repo=?1 AND provider=?2 AND id=?3",
                params![repo, provider, project.id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(previous) = previous {
            let mut previous: PmProject = serde_json::from_str(&previous)?;
            if previous.initiative_ids.as_slice() != [initiative] {
                return Err(StoreError::InvalidData(format!(
                    "Project {} changed Initiative during reteam",
                    project.id
                )));
            }
            // Reteam orders Team relationships only. Retain independently newer entity facts.
            previous.team_ids.clone_from(&project.team_ids);
            tx.execute("UPDATE pm_projects SET body=?4,membership_unresolved=0 WHERE repo=?1 AND provider=?2 AND id=?3",
                params![repo, provider, project.id, serde_json::to_string(&previous)?])?;
        }
        put_project(&tx, &repo, provider, observed_at, project)?;
        associate_project(&tx, &repo, provider, &project.id, wave, initiative)?;
        project_accepted_planning(
            &tx,
            &repo,
            provider,
            std::slice::from_ref(project),
            &[],
            Some((wave, initiative)),
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn put_pm_task(
        &self,
        repo: &str,
        provider: &str,
        record: &PmTaskRecord,
        confirmed_wave: Option<(&WaveId, &str)>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        if let Some(project) = &record.project {
            validate_project_membership(&conn, repo, provider, project)?;
        }
        let tx = conn.transaction()?;
        let mut item = record.item.clone();
        // Detail queries do not observe relative list order.
        if let Some(rank) = tx.query_row(
            "SELECT json_extract(body,'$.rank') FROM pm_items WHERE repo=?1 AND provider=?2 AND id=?3 AND project_id IS ?4",
            params![repo,provider,item.id,item.project_id], |row| row.get::<_,u32>(0),
        ).optional()? {
            item.rank = rank;
        }
        // Project revisions are independent of the issue revision and request age.
        if let Some(project) = &record.project {
            put_project(&tx, repo, provider, record.observed_at, project)?;
        }
        if put_item(&tx, repo, provider, record.observed_at, &item)? {
            tx.execute(
                "UPDATE pm_items SET needs_refresh=0 WHERE repo=?1 AND provider=?2 AND id=?3",
                params![repo, provider, record.item.id],
            )?;
        }
        if let Some((wave, initiative)) = confirmed_wave {
            let project_id = record.item.project_id.as_deref().ok_or_else(|| {
                StoreError::InvalidData("confirmed Wave requires a Project association".into())
            })?;
            associate_project(&tx, repo, provider, project_id, wave, initiative)?;
        }
        project_accepted_planning(
            &tx,
            repo,
            provider,
            record.project.as_slice(),
            std::slice::from_ref(&record.item),
            confirmed_wave,
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn confirm_pm_project_archival(
        &self,
        repo: &str,
        provider: &str,
        project: &PmProject,
        observed_at: i64,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO pm_projects(repo,provider,id,observed_at,body,archived) VALUES(?1,?2,?3,?4,?5,1)
             ON CONFLICT(repo,provider,id) DO UPDATE SET archived=1",
            params![repo, provider, project.id, observed_at, serde_json::to_string(project)?],
        )?;
        Ok(())
    }

    pub fn observe_pm_issue_change(
        &self,
        issue_id: &str,
        revision: Option<&str>,
        removed: bool,
    ) -> StoreResult<()> {
        let revision = revision_nanos(revision)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO pm_issue_changes(issue_id,revision_ns,removed) VALUES(?1,?2,?3)
             ON CONFLICT(issue_id) DO UPDATE SET
             revision_ns=CASE WHEN excluded.revision_ns > revision_ns OR revision_ns IS NULL
                 THEN excluded.revision_ns ELSE revision_ns END,
             removed=MAX(removed,excluded.removed)",
            params![issue_id, revision, removed],
        )?;
        let mut query =
            tx.prepare("SELECT repo,body FROM pm_items WHERE provider='linear' AND id=?1")?;
        let rows = query
            .query_map([issue_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (repo, body) in rows {
            let item: PmItem = serde_json::from_str(&body)?;
            let cached = revision_nanos(item.revision.as_deref())?;
            if removed || revision.is_none() || cached < revision {
                tx.execute("UPDATE pm_items SET needs_refresh=1 WHERE repo=?1 AND provider='linear' AND id=?2", params![repo,issue_id])?;
            }
        }
        drop(query);
        tx.commit()?;
        Ok(())
    }

    pub fn invalidate_pm_task(
        &self,
        repo: &str,
        provider: &str,
        expected: &PmTaskRecord,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // A null detail response invalidates cached admission, not provider history.
        // Only a complete detail observation can repair it; list omission is ambiguous.
        conn.execute(
            "UPDATE pm_items SET needs_refresh=1 WHERE repo=?1 AND provider=?2 AND id=?3
            AND observed_at=?4 AND json_extract(body,'$.revision') IS ?5",
            params![
                repo,
                provider,
                expected.item.id,
                expected.observed_at,
                expected.item.revision
            ],
        )?;
        Ok(())
    }

    pub fn pm_task_observation(
        &self,
        repo: &str,
        provider: &str,
        selector: &str,
    ) -> StoreResult<PmTaskObservation> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        pm_task_observation_in(&conn, repo, provider, selector)
    }

    pub fn put_pm_snapshot(&self, snapshot: &PmSnapshotRow) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let repo: String = conn.query_row(
            "SELECT repo FROM waves WHERE id=?1",
            [snapshot.wave_id.as_str()],
            |row| row.get(0),
        )?;
        for project in &snapshot.snapshot.projects {
            validate_project_membership(&conn, &repo, &snapshot.provider, project)?;
        }
        let tx = conn.transaction()?;
        let known = {
            let mut query = tx.prepare(
                "SELECT m.project_id FROM pm_wave_projects m JOIN pm_projects p ON p.id=m.project_id
                 WHERE m.wave_id=?1 AND p.repo=?2 AND p.provider=?3 AND p.archived=0"
            )?;
            let rows = query
                .query_map(params![snapshot.wave_id, repo, snapshot.provider], |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        if let Some(missing) = known.iter().find(|id| {
            !snapshot
                .snapshot
                .projects
                .iter()
                .any(|project| &project.id == *id)
        }) {
            return Err(StoreError::InvalidData(format!(
                "Project {missing} omitted without membership removal evidence; refresh planning"
            )));
        }
        for project in &snapshot.snapshot.projects {
            put_project(&tx, &repo, &snapshot.provider, snapshot.synced_at, project)?;
            require_accepted_initiative(
                &tx,
                &repo,
                &snapshot.provider,
                &project.id,
                &snapshot.initiative,
            )?;
        }
        for item in &snapshot.snapshot.items {
            put_item(&tx, &repo, &snapshot.provider, snapshot.synced_at, item)?;
        }
        tx.execute(
            "INSERT INTO pm_wave_sync(wave_id,provider,initiative,synced_at) VALUES(?1,?2,?3,?4)
             ON CONFLICT(wave_id) DO UPDATE SET provider=excluded.provider,
             initiative=excluded.initiative,synced_at=MAX(pm_wave_sync.synced_at,excluded.synced_at)",
            params![
                snapshot.wave_id,
                snapshot.provider,
                snapshot.initiative,
                snapshot.synced_at
            ],
        )?;
        for (position, project) in snapshot.snapshot.projects.iter().enumerate() {
            tx.execute(
                "INSERT INTO pm_wave_projects(wave_id,project_id,position) VALUES(?1,?2,?3)
                 ON CONFLICT(wave_id,project_id) DO NOTHING",
                params![snapshot.wave_id, project.id, position as i64],
            )?;
        }
        project_accepted_planning(
            &tx,
            &repo,
            &snapshot.provider,
            &snapshot.snapshot.projects,
            &snapshot.snapshot.items,
            None,
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn pm_snapshot(&self, wave_id: &WaveId) -> StoreResult<Option<PmSnapshotRow>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let metadata = tx
            .query_row(
                "SELECT w.repo,s.provider,s.initiative,s.synced_at FROM pm_wave_sync s
             JOIN waves w ON w.id=s.wave_id WHERE s.wave_id=?1",
                [wave_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((repo, provider, initiative, synced_at)) = metadata else {
            return Ok(None);
        };
        let mut query = tx.prepare(
            "SELECT p.body FROM pm_wave_projects m JOIN pm_projects p ON p.id=m.project_id
             WHERE m.wave_id=?1 AND p.repo=?2 AND p.provider=?3 AND p.archived=0 AND p.membership_unresolved=0
             ORDER BY m.position,p.id",
        )?;
        // Decode before membership filtering: malformed retained bodies are unavailable
        // evidence, not an empty plan.
        let mut projects = query
            .query_map(params![wave_id, repo, provider], |row| {
                row.get::<_, String>(0)
            })?
            .map(|row| Ok(serde_json::from_str::<PmProject>(&row?)?))
            .collect::<StoreResult<Vec<_>>>()?;
        projects.retain(|project| project.initiative_ids.contains(&initiative));
        let mut query = tx.prepare(
            "SELECT i.body,json_extract(p.body,'$.slug') FROM pm_items i JOIN pm_wave_projects m ON m.project_id=i.project_id
             JOIN pm_projects p ON p.id=i.project_id AND p.repo=i.repo AND p.provider=i.provider
             WHERE m.wave_id=?1 AND i.repo=?2 AND i.provider=?3 AND i.needs_refresh=0
             AND p.archived=0 AND p.membership_unresolved=0
             AND EXISTS(SELECT 1 FROM json_each(p.body,'$.initiative_ids') WHERE value=?4)
             AND NOT EXISTS(SELECT 1 FROM task_deletions d JOIN waves w ON w.id=d.wave_id
                            WHERE w.repo=i.repo AND d.issue_id=i.id)
             ORDER BY m.position,json_extract(i.body,'$.rank'),i.id",
        )?;
        let items = query
            .query_map(params![wave_id, repo, provider, initiative], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .map(|row| {
                let (body, slug) = row?;
                let mut item: PmItem = serde_json::from_str(&body)?;
                item.project = Some(slug);
                Ok(item)
            })
            .collect::<StoreResult<Vec<_>>>()?;
        Ok(Some(PmSnapshotRow {
            wave_id: wave_id.clone(),
            provider,
            initiative,
            synced_at,
            snapshot: PmSnapshot { projects, items },
        }))
    }
}

pub(super) fn pm_task_observation_in(
    conn: &Connection,
    repo: &str,
    provider: &str,
    selector: &str,
) -> StoreResult<PmTaskObservation> {
    let mut query = conn.prepare(
            "SELECT i.body,p.body,i.observed_at,i.needs_refresh OR COALESCE(p.membership_unresolved,0) OR COALESCE(p.archived,0),
             EXISTS(SELECT 1 FROM task_deletions d JOIN waves w ON w.id=d.wave_id
                    WHERE w.repo=i.repo AND d.issue_id=i.id)
             OR EXISTS(SELECT 1 FROM pm_issue_changes c WHERE c.issue_id=i.id
                       AND i.provider='linear' AND c.removed=1)
             FROM pm_items i LEFT JOIN pm_projects p
             ON p.repo=i.repo AND p.provider=i.provider AND p.id=i.project_id
             WHERE i.repo=?1 AND i.provider=?2 AND (i.id=?3 OR i.identifier=?3)",
        )?;
    let rows = query
        .query_map(params![repo, provider, selector], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<String>>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, bool>(3)?,
                row.get::<_, bool>(4)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    if rows.len() > 1 {
        return Err(StoreError::InvalidData(format!(
            "ambiguous planning selector {selector:?}; use a Task UUID"
        )));
    }
    let Some((item, project, observed_at, invalid, removed)) = rows.into_iter().next() else {
        let removed = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM pm_issue_changes WHERE issue_id=?1 AND removed=1 AND ?2='linear')
                 OR EXISTS(SELECT 1 FROM task_deletions d JOIN waves w ON w.id=d.wave_id
                           WHERE w.repo=?3 AND (d.issue_id=?1 OR d.identifier=?1 COLLATE NOCASE))",
                params![selector, provider, repo], |row| row.get::<_, bool>(0),
            )?;
        return Ok(PmTaskObservation {
            record: None,
            state: if removed {
                PlanningState::Removed
            } else {
                PlanningState::Unavailable
            },
        });
    };
    let mut item: PmItem = serde_json::from_str(&item)?;
    let project: Option<PmProject> = project
        .map(|body| serde_json::from_str(&body))
        .transpose()?;
    if let Some(project) = &project {
        item.project = Some(project.slug.clone());
    }
    Ok(PmTaskObservation {
        record: Some(PmTaskRecord {
            item,
            project,
            observed_at,
        }),
        state: if removed {
            PlanningState::Removed
        } else if invalid {
            PlanningState::Invalid
        } else {
            PlanningState::Available
        },
    })
}

// Both durable projections require the same confirmed Initiative/Wave association.
// A partial read supplies that association without advancing full-Wave freshness.
const ACCEPTED_WAVE_PROJECTS: &str = "
    SELECT p.id,p.body,p.observed_at,m.wave_id FROM pm_projects p
    JOIN pm_wave_projects m ON m.project_id=p.id
    LEFT JOIN pm_wave_sync sync ON sync.wave_id=m.wave_id AND sync.provider=p.provider
    JOIN waves w ON w.id=m.wave_id AND w.repo=p.repo
    WHERE p.repo=?1 AND p.provider=?2 AND p.archived=0 AND p.membership_unresolved=0
    AND json_array_length(p.body,'$.initiative_ids')=1
    AND json_extract(p.body,'$.initiative_ids[0]')=
        CASE WHEN m.wave_id=?4 THEN ?5 ELSE sync.initiative END";

/// Inputs select IDs only; project their accepted rows inside the ingestion transaction.
fn project_accepted_planning(
    tx: &Transaction<'_>,
    repo: &str,
    provider: &str,
    projects: &[PmProject],
    items: &[PmItem],
    confirmed_wave: Option<(&WaveId, &str)>,
) -> StoreResult<()> {
    let (confirmed_wave, confirmed_initiative) = confirmed_wave.unzip();
    let project_ids = serde_json::to_string(&projects.iter().map(|p| &p.id).collect::<Vec<_>>())?;
    let item_ids = serde_json::to_string(&items.iter().map(|i| &i.id).collect::<Vec<_>>())?;
    let mut query = tx.prepare(&format!(
        "WITH accepted AS ({ACCEPTED_WAVE_PROJECTS})
         SELECT body,observed_at,wave_id FROM accepted
         WHERE id IN (SELECT value FROM json_each(?3))"
    ))?;
    let projects = query.query_map(
        params![
            repo,
            provider,
            project_ids,
            confirmed_wave,
            confirmed_initiative
        ],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, WaveId>(2)?,
            ))
        },
    )?;
    for project in projects {
        let (body, observed_at, wave_id) = project?;
        let project: PmProject = serde_json::from_str(&body)?;
        let existing: Option<(String, WaveId)> = tx
            .query_row(
                "SELECT id,wave_id FROM projects WHERE external_project_id=?1",
                [&project.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let id = match existing {
            Some((_, owner)) if owner != wave_id => {
                return Err(StoreError::InvalidData(format!(
                    "Project {} changed Wave ownership",
                    project.id
                )));
            }
            Some((id, _)) => {
                ProjectId::parse(&id).map_err(|error| StoreError::InvalidData(error.to_string()))
            }?,
            None => ProjectId::new(),
        };
        tx.execute(
            "INSERT INTO projects(id,wave_id,external_project_id,project_slug,project_name,
             project_prompt_context,pm_snapshot_synced_at,created_at,updated_at,flow,status)
             VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?8,?9,?10)
             ON CONFLICT(id) DO UPDATE SET project_slug=excluded.project_slug,
             project_name=excluded.project_name,project_prompt_context=excluded.project_prompt_context,
             pm_snapshot_synced_at=excluded.pm_snapshot_synced_at,flow=excluded.flow,status=excluded.status",
            params![id.as_str(),wave_id,project.id,project.slug,project.name,
                project.prompt_context(),observed_at,super::super::rows::now_unix(),project.flow,project.status.as_str()],
        )?;
        super::durable::inherit_project_placement(tx, &id)?;
    }
    // Copy accepted facts directly; execution fields and unobserved Tasks stay intact.
    tx.execute(
        &format!("WITH accepted AS ({ACCEPTED_WAVE_PROJECTS})
         UPDATE tasks AS target SET
             issue_identifier=json_extract(i.body,'$.identifier'),
             issue_title=json_extract(i.body,'$.name'),
             issue_description=json_extract(i.body,'$.description'),
             pm_snapshot_synced_at=i.observed_at,project_id=p.id
         FROM pm_items i
         JOIN projects p ON p.external_project_id=i.project_id
         JOIN projects current ON current.wave_id=p.wave_id
         JOIN accepted observed ON observed.id=i.project_id AND observed.wave_id=p.wave_id
         WHERE target.external_issue_id=i.id AND target.project_id=current.id
         AND i.repo=?1 AND i.provider=?2 AND i.needs_refresh=0
         AND i.id IN (SELECT value FROM json_each(?3))
         AND NOT EXISTS(SELECT 1 FROM task_deletions d WHERE d.wave_id=current.wave_id AND d.issue_id=i.id)
         AND NOT EXISTS(SELECT 1 FROM pm_issue_changes c WHERE c.issue_id=i.id AND i.provider='linear' AND c.removed=1)"),
        params![repo, provider, item_ids, confirmed_wave, confirmed_initiative],
    )?;
    Ok(())
}

fn same_ids(left: &[String], right: &[String]) -> bool {
    left.iter().all(|id| right.contains(id)) && right.iter().all(|id| left.contains(id))
}

fn associate_project(
    tx: &Transaction<'_>,
    repo: &str,
    provider: &str,
    project_id: &str,
    wave: &WaveId,
    initiative: &str,
) -> StoreResult<PmProject> {
    let accepted = require_accepted_initiative(tx, repo, provider, project_id, initiative)?;
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
    Ok(accepted)
}

fn require_accepted_initiative(
    conn: &Connection,
    repo: &str,
    provider: &str,
    project_id: &str,
    initiative: &str,
) -> StoreResult<PmProject> {
    let body: String = conn.query_row(
        "SELECT body FROM pm_projects WHERE repo=?1 AND provider=?2 AND id=?3",
        params![repo, provider, project_id],
        |row| row.get(0),
    )?;
    let accepted: PmProject = serde_json::from_str(&body)?;
    if accepted.initiative_ids.as_slice() != [initiative] {
        return Err(StoreError::InvalidData(format!(
            "accepted Project {project_id} does not belong to Initiative {initiative}"
        )));
    }
    Ok(accepted)
}

fn validate_project_membership(
    conn: &Connection,
    repo: &str,
    provider: &str,
    project: &PmProject,
) -> StoreResult<()> {
    // Run before the ingestion transaction: disputed ownership must remain
    // recorded even when rejecting the incoming facts rolls that transaction back.
    let previous: Option<String> = conn
        .query_row(
            "SELECT body FROM pm_projects WHERE repo=?1 AND provider=?2 AND id=?3",
            params![repo, provider, project.id],
            |row| row.get(0),
        )
        .optional()?;
    let Some(previous) = previous else {
        return Ok(());
    };
    let previous: PmProject = serde_json::from_str(&previous)?;
    let revision = revision_nanos(project.revision.as_deref())?;
    if revision.is_some() && revision < revision_nanos(previous.revision.as_deref())? {
        return Ok(());
    }
    // A Project revision does not order its separate Initiative/Team relationships.
    // Preserve the facts, but do not let managed readers use disputed ownership.
    if !same_ids(&project.initiative_ids, &previous.initiative_ids)
        || !same_ids(&project.team_ids, &previous.team_ids)
    {
        conn.execute("UPDATE pm_projects SET membership_unresolved=1 WHERE repo=?1 AND provider=?2 AND id=?3",
            params![repo, provider, project.id])?;
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
    provider: &str,
    observed_at: i64,
    project: &PmProject,
) -> StoreResult<()> {
    let previous: Option<(String, i64, bool)> = conn
        .query_row(
            "SELECT body,observed_at,archived FROM pm_projects WHERE repo=?1 AND provider=?2 AND id=?3",
            params![repo, provider, project.id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    // A later list/detail response cannot undo a confirmed archive receipt.
    if previous.as_ref().is_some_and(|(_, _, archived)| *archived) {
        return Ok(());
    }
    let pending_name_cutover: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pm_project_name_cutover
         WHERE repo=?1 AND provider=?2 AND id=?3 AND converted_at IS NULL)",
        params![repo, provider, project.id],
        |row| row.get(0),
    )?;
    let revision = revision_nanos(project.revision.as_deref())?;
    if let Some((previous, acquired, _)) = previous {
        let previous: PmProject = serde_json::from_str(&previous)?;
        let previous_revision = revision_nanos(previous.revision.as_deref())?;
        if revision.is_some() && revision < previous_revision {
            return Ok(());
        }
        let mut comparable = project.clone();
        comparable.revision = previous.revision.clone();
        // Relationship sets were checked independently; their traversal order is immaterial.
        comparable.initiative_ids = previous.initiative_ids.clone();
        comparable.team_ids = previous.team_ids.clone();
        if pending_name_cutover {
            comparable.name.clone_from(&previous.name);
            comparable.slug.clone_from(&previous.slug);
        }
        if comparable != previous && (revision.is_none() || revision == previous_revision) {
            return Err(StoreError::InvalidData(format!(
                "unordered or conflicting Project facts for {}; refresh planning",
                project.id
            )));
        }
        if revision.is_none() && previous_revision.is_some()
            || revision == previous_revision && observed_at < acquired
        {
            return Ok(());
        }
    }
    conn.execute(
        "INSERT INTO pm_projects(repo,provider,id,observed_at,body) VALUES(?1,?2,?3,?4,?5)
         ON CONFLICT(repo,provider,id) DO UPDATE SET observed_at=excluded.observed_at,body=excluded.body",
        params![repo,provider,project.id,observed_at,serde_json::to_string(project)?],
    )?;
    if pending_name_cutover {
        conn.execute(
            "UPDATE pm_project_name_cutover SET converted_at=?4
             WHERE repo=?1 AND provider=?2 AND id=?3 AND converted_at IS NULL",
            params![repo, provider, project.id, observed_at],
        )?;
    }
    Ok(())
}

fn revision_nanos(revision: Option<&str>) -> StoreResult<Option<i64>> {
    revision
        .map(|revision| {
            time::OffsetDateTime::parse(revision, &time::format_description::well_known::Rfc3339)
                .map_err(|error| {
                    StoreError::InvalidData(format!("invalid planning revision: {error}"))
                })
                .and_then(|time| {
                    i64::try_from(time.unix_timestamp_nanos()).map_err(|error| {
                        StoreError::InvalidData(format!("planning revision out of range: {error}"))
                    })
                })
        })
        .transpose()
}

fn put_item(
    conn: &Connection,
    repo: &str,
    provider: &str,
    observed_at: i64,
    item: &PmItem,
) -> StoreResult<bool> {
    let revision = revision_nanos(item.revision.as_deref())?;
    if provider == "linear" {
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
    }
    let previous: Option<(String, i64)> = conn
        .query_row(
            "SELECT body,observed_at FROM pm_items WHERE repo=?1 AND provider=?2 AND id=?3",
            params![repo, provider, item.id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    if let Some((previous, acquired)) = previous {
        let previous_json: serde_json::Value = serde_json::from_str(&previous)?;
        let has_completed_at = previous_json.get("completed_at").is_some();
        let previous: PmItem = serde_json::from_value(previous_json)?;
        let previous_revision = revision_nanos(previous.revision.as_deref())?;
        if revision < previous_revision {
            return Ok(false);
        }
        if revision == previous_revision {
            // Rank comes from a list; the Project display name has its own revision.
            let mut comparable = item.clone();
            comparable.rank = previous.rank;
            comparable.project = previous.project.clone();
            comparable.revision = previous.revision.clone();
            // Earlier observations did not request Linear's branch name.
            // Enrich unknown placement without accepting conflicting known facts.
            if previous.branch_name.is_none() {
                comparable.branch_name = None;
            }
            if !has_completed_at {
                comparable.completed_at = None;
            }
            if comparable != previous {
                return Err(StoreError::InvalidData(format!(
                    "conflicting planning facts at the same provider revision for {}; refresh planning", item.identifier
                )));
            }
        }
        // For equal revisions, keep the later acquisition's list rank and freshness.
        if revision == previous_revision && observed_at < acquired {
            return Ok(false);
        }
    }
    conn.execute(
        "INSERT INTO pm_items(repo,provider,id,identifier,project_id,observed_at,body) VALUES(?1,?2,?3,?4,?5,?6,?7)
         ON CONFLICT(repo,provider,id) DO UPDATE SET identifier=excluded.identifier,
         project_id=excluded.project_id,observed_at=excluded.observed_at,body=excluded.body",
        params![repo,provider,item.id,item.identifier,item.project_id,observed_at,serde_json::to_string(item)?],
    )?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};
    use serde_json::json;

    use crate::build_info::MigrationAuthority;
    use crate::id::WaveId;
    use crate::store::sqlite::SqliteStore;
    use crate::store::{FrontierAdvance, PlanningState, Store};

    #[test]
    fn project_name_cutover_retains_both_histories_and_rejects_later_conflicts() {
        // "Customer requests" represents both a projected prefixed name and a
        // verbatim plain provider name. The same input must accept both histories.
        // An already exact prefixed name also consumes the exception once.
        for historical_name in ["Customer requests", "Product — Customer requests"] {
            let mut conn = Connection::open_in_memory().unwrap();
            crate::store::migrations::apply_before_current_draft(&conn, "project_readiness");
            let snapshot: crate::pm::PmSnapshot = serde_json::from_str(include_str!(
                "../../../../../tests/fixtures/dto/task_history_planning.json"
            ))
            .unwrap();
            let mut project = snapshot.projects[0].clone();
            project.revision = Some("2026-10-05T12:00:00Z".into());
            project.name = historical_name.into();
            project.slug = crate::pm::project_slug(historical_name);
            let original = serde_json::to_string(&project).unwrap();
            conn.execute(
                "INSERT INTO pm_projects(repo,provider,id,observed_at,body)
                VALUES('/repo','linear',?1,10,?2)",
                params![project.id, original],
            )
            .unwrap();
            conn.execute_batch(&crate::store::migrations::current_draft_sql(
                "project_readiness",
            ))
            .unwrap();
            project.name = "Product — Customer requests".into();
            project.slug = crate::pm::project_slug(&project.name);

            let mut conflict = project.clone();
            conflict.summary.push_str("conflict");
            assert!(super::put_project(&conn, "/repo", "linear", 11, &conflict).is_err());
            super::put_project(&conn, "/repo", "linear", 9, &project).unwrap();
            let retained: String = conn
                .query_row("SELECT body FROM pm_projects", [], |r| r.get(0))
                .unwrap();
            assert_eq!(retained, original);
            // Failed outer work cannot spend the one-time exception.
            {
                let tx = conn.transaction().unwrap();
                super::put_project(&tx, "/repo", "linear", 11, &project).unwrap();
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
            super::put_project(&tx, "/repo", "linear", 12, &project).unwrap();
            tx.commit().unwrap();
            let evidence: (String, i64, Option<i64>) = conn
                .query_row(
                    "SELECT body,observed_at,converted_at FROM pm_project_name_cutover",
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
                )
                .unwrap();
            assert_eq!(evidence, (original, 10, Some(12)));
            let accepted = serde_json::to_string(&project).unwrap();
            project.name = "Another name".into();
            assert!(super::put_project(&conn, "/repo", "linear", 13, &project).is_err());
            let retained: String = conn
                .query_row("SELECT body FROM pm_projects", [], |r| r.get(0))
                .unwrap();
            assert_eq!(retained, accepted);
            project.id = "post-cutover".into();
            super::put_project(&conn, "/repo", "linear", 13, &project).unwrap();
            project.name = "Conflict on a new record".into();
            assert!(super::put_project(&conn, "/repo", "linear", 14, &project).is_err());
        }
    }

    #[test]
    fn task_history_enriches_only_absent_completion_dates_at_equal_revision() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE pm_items(repo TEXT,provider TEXT,id TEXT,identifier TEXT,project_id TEXT,observed_at INTEGER,body TEXT,PRIMARY KEY(repo,provider,id)); CREATE TABLE pm_issue_changes(issue_id TEXT,revision_ns INTEGER,removed INTEGER);").unwrap();
        let planning: crate::pm::PmSnapshot = serde_json::from_str(include_str!(
            "../../../../../tests/fixtures/dto/task_history_planning.json"
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
            "INSERT INTO pm_items VALUES('/repo','linear',?1,?2,?3,1,?4)",
            params![item.id, item.identifier, item.project_id, old.to_string()],
        )
        .unwrap();
        assert!(super::put_item(&conn, "/repo", "linear", 2, &item).unwrap());
        let observed: String = conn
            .query_row("SELECT body FROM pm_items", [], |row| row.get(0))
            .unwrap();
        assert_eq!(
            serde_json::from_str::<crate::pm::PmItem>(&observed)
                .unwrap()
                .completed_at,
            item.completed_at
        );
        item.completed_at = None;
        assert!(super::put_item(&conn, "/repo", "linear", 3, &item).is_err());
        item.revision = Some("2026-10-01T12:00:00Z".into());
        assert!(!super::put_item(&conn, "/repo", "linear", 4, &item).unwrap());
        item.revision = Some("2026-10-03T12:00:00Z".into());
        assert!(super::put_item(&conn, "/repo", "linear", 5, &item).unwrap());
        item.completed_at = Some("2026-10-01T12:00:00Z".into());
        assert!(super::put_item(&conn, "/repo", "linear", 6, &item).is_err());
    }

    #[tokio::test]
    async fn migration_preserves_planning_identity_and_removes_snapshot_storage() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join(".lf/loopflow.db");
        std::fs::create_dir_all(database.parent().unwrap()).unwrap();
        let wave = WaveId::new();
        let snapshot = json!({"projects":[{
            "id":"project", "slug":"chapter", "name":"Chapter", "summary":"Proof",
            "metric_targets":[], "flow":"feature", "status":"started", "krs":[],
            "initiative_ids":["initiative"], "team_ids":["team"]
        }],"items":[{
            "id":"issue", "identifier":"FIX-1", "url":null, "name":"Retained title",
            "description":"Retained notes", "rank":2, "completed":false, "state":"unstarted",
            "project_id":"project", "project":"chapter", "team_id":"team", "assignee":null
        }]});
        {
            let conn = Connection::open(&database).unwrap();
            crate::store::migrations::apply_released_planning_fixture(&conn);
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'product','/repo',1)",
                [wave.as_str()],
            )
            .unwrap();
            conn.execute("INSERT INTO pm_snapshots(wave_id,provider,initiative,synced_at,payload) VALUES(?1,'linear','initiative',42,?2)",params![wave,snapshot.to_string()]).unwrap();
        }
        // Upgrades belong to the published installation, not an experiment.
        let sqlite = SqliteStore::open_with(
            &database,
            MigrationAuthority::Published,
            directory.path(),
            FrontierAdvance::Authorized,
        )
        .unwrap();
        for name in [
            "normalize_pm_planning",
            "pm_issue_revisions",
            "pm_project_evidence",
        ] {
            sqlite.apply_migration_for_test(name).unwrap();
        }
        let store = Store { sqlite };
        let observation = store
            .pm_task_observation("/repo", "linear", "fix-1")
            .await
            .unwrap();
        assert_eq!(observation.state, PlanningState::Available);
        let detail = observation.record.unwrap();
        let list = store.pm_snapshot(&wave).await.unwrap().unwrap();
        assert_eq!(detail.observed_at, 42);
        assert_eq!(detail.item, list.snapshot.items[0]);
        assert_eq!(detail.item.name, "Retained title");
        assert_eq!(detail.item.description, "Retained notes");
        assert_eq!(detail.project.as_ref(), Some(&list.snapshot.projects[0]));
        assert_eq!(store.get_wave(&wave).await.unwrap().unwrap().id(), &wave);
        assert!(store.list_tasks(None).await.unwrap().is_empty());
        assert!(!Connection::open(database)
            .unwrap()
            .prepare("SELECT payload FROM pm_snapshots")
            .is_ok());

        // A delayed list cannot resurrect a confirmed removal in either reader.
        store
            .confirm_task_deletion(&wave, "issue", "FIX-1")
            .await
            .unwrap();
        store.put_pm_snapshot(list, None).await.unwrap();
        let removed = store
            .pm_task_observation("/repo", "linear", "FIX-1")
            .await
            .unwrap();
        assert_eq!(removed.state, PlanningState::Removed);
        assert_eq!(removed.record, Some(detail));
        assert!(store
            .pm_snapshot(&wave)
            .await
            .unwrap()
            .unwrap()
            .snapshot
            .items
            .is_empty());
    }
}
