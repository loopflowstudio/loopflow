//! Linear observations retain claims that canonical Work cannot own, including
//! rejected membership and foreign records. Acceptance and projection share a
//! transaction; the released `provider` columns remain storage discriminators,
//! not a choice exposed by the ingestion API.

use crate::work::edit::Disposition;
use crate::work::edit::WorkValue as Edit;
use rusqlite::{params, Connection, OptionalExtension, Transaction};

use super::planning_write;
use crate::id::WaveId;
use crate::store::sqlite::SqliteStore;
use crate::store::{PlanningState, StoreError, StoreResult};
use crate::work::project::ProjectId;

impl SqliteStore {
    /// Delivery requires the fetched revision to have reached this Task's Work row.
    /// Retaining an observation (including a foreign or rejected one) is insufficient.
    pub(crate) fn accepted_task_revision(
        &self,
        task: &crate::durable::TaskId,
        issue: &str,
        revision: Option<&str>,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM tasks WHERE id=?1 AND external_issue_id=?2
             AND planning_provider_revision=?3)",
            params![task.as_str(), issue, revision],
            |row| row.get(0),
        )?)
    }

    /// Retained observations do not authorize delivery until accepted into Work.
    pub(crate) fn accepted_project_revision(
        &self,
        project: &ProjectId,
        external: &str,
        revision: Option<&str>,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects p
             JOIN waves w ON w.id=p.wave_id
             JOIN pm_projects observed ON observed.repo=w.repo
                 AND observed.provider='linear' AND observed.id=p.external_project_id
             WHERE p.id=?1 AND p.external_project_id=?2 AND p.planning_provider_revision=?3
                 AND observed.archived=0 AND observed.membership_unresolved=0)",
            params![project.as_str(), external, revision],
            |row| row.get(0),
        )?)
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
        if removed {
            let ids = {
                let mut q=tx.prepare("SELECT id FROM tasks WHERE external_issue_id=?1 AND planning_deleted_at IS NULL")?;
                let rows = q.query_map([issue_id], |r| r.get::<_, String>(0))?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            for id in ids {
                planning_write::local(
                    &tx,
                    &crate::work::edit::WorkEdit::for_target(
                        &id,
                        &[Edit::TaskDeletedAt(Some(super::super::rows::now_unix()))],
                    ),
                )?;
            }
        }
        let mut query =
            tx.prepare("SELECT repo,revision FROM pm_items WHERE provider='linear' AND id=?1")?;
        let rows = query
            .query_map([issue_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (repo, cached_revision) in rows {
            let cached = revision_nanos(cached_revision.as_deref())?;
            if removed || revision.is_none() || cached < revision {
                tx.execute("UPDATE pm_items SET needs_refresh=1 WHERE repo=?1 AND provider='linear' AND id=?2", params![repo,issue_id])?;
            }
        }
        drop(query);
        tx.commit()?;
        Ok(())
    }

    pub fn invalidate_linear_issue(
        &self,
        repo: &str,
        expected: &crate::store::LinearIssueEvidence,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // A null detail response invalidates cached admission, not provider history.
        // Only a complete detail observation can repair it; list omission is ambiguous.
        conn.execute(
            "UPDATE pm_items SET needs_refresh=1 WHERE repo=?1 AND provider='linear' AND id=?2
            AND observed_at=?3 AND revision IS ?4",
            params![
                repo,
                expected.issue_id,
                expected.observed_at,
                expected.revision
            ],
        )?;
        Ok(())
    }

    pub fn linear_issue_evidence(
        &self,
        repo: &str,
        selector: &str,
    ) -> StoreResult<crate::store::LinearIssueEvidence> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT i.id,i.revision,i.observed_at,
             p.initiative_ids,
             p.team_ids,i.team_id,
             i.needs_refresh OR COALESCE(p.membership_unresolved,0) OR COALESCE(p.archived,0),
             EXISTS(SELECT 1 FROM task_deletions d JOIN waves w ON w.id=d.wave_id
                    WHERE w.repo=i.repo AND d.issue_id=i.id)
             OR EXISTS(SELECT 1 FROM pm_issue_changes c WHERE c.issue_id=i.id AND c.removed=1),p.id
             FROM pm_items i LEFT JOIN pm_projects p
             ON p.repo=i.repo AND p.provider=i.provider AND p.id=i.project_id
             WHERE i.repo=?1 AND i.provider='linear' AND (i.id=?2 OR i.identifier=?2)",
        )?;
        let rows = query
            .query_map(params![repo, selector], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, Option<String>>(5)?,
                    row.get::<_, bool>(6)?,
                    row.get::<_, bool>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if rows.len() > 1 {
            return Err(StoreError::InvalidData(format!(
                "ambiguous planning selector {selector:?}; use a Task UUID"
            )));
        }
        let Some((
            id,
            revision,
            observed_at,
            initiatives,
            teams,
            team_id,
            invalid,
            removed,
            project_id,
        )) = rows.into_iter().next()
        else {
            let (removed, changed) = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM pm_issue_changes WHERE issue_id=?1 AND removed=1)
                 OR EXISTS(SELECT 1 FROM task_deletions d JOIN waves w ON w.id=d.wave_id
                           WHERE w.repo=?2 AND (d.issue_id=?1 OR d.identifier=?1 COLLATE NOCASE)),
                 EXISTS(SELECT 1 FROM pm_issue_changes WHERE issue_id=?1)",
                params![selector, repo],
                |row| Ok((row.get::<_, bool>(0)?, row.get::<_, bool>(1)?)),
            )?;
            return Ok(crate::store::LinearIssueEvidence {
                issue_id: None,
                revision: None,
                observed_at: None,
                project_id: None,
                initiative_ids: Vec::new(),
                project_team_ids: Vec::new(),
                team_id: None,
                state: if removed {
                    PlanningState::Removed
                } else if changed {
                    PlanningState::Invalid
                } else {
                    PlanningState::Unavailable
                },
            });
        };
        Ok(crate::store::LinearIssueEvidence {
            issue_id: Some(id),
            revision,
            observed_at: Some(observed_at),
            project_id,
            initiative_ids: initiatives
                .map(|value| serde_json::from_str(&value))
                .transpose()?
                .unwrap_or_default(),
            project_team_ids: teams
                .map(|value| serde_json::from_str(&value))
                .transpose()?
                .unwrap_or_default(),
            team_id,
            state: if removed {
                PlanningState::Removed
            } else if invalid {
                PlanningState::Invalid
            } else {
                PlanningState::Available
            },
        })
    }

    /// Validate retained Linear claims without exposing a second Work inventory.
    pub(crate) fn validate_linear_ownership(
        &self,
        waves: &[(&WaveId, &str)],
        expected_team: StoreResult<Option<String>>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.transaction()?;
        let mut initiatives = std::collections::BTreeMap::new();
        let mut projects = std::collections::BTreeMap::new();
        let mut issues = std::collections::BTreeMap::new();
        for (wave_id, wave) in waves {
            let metadata = conn
                .query_row(
                    "SELECT w.repo,s.provider,s.initiative FROM pm_wave_sync s
                 JOIN waves w ON w.id=s.wave_id WHERE s.wave_id=?1",
                    [wave_id.as_str()],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?;
            let Some((repo, provider, initiative)) = metadata else {
                continue;
            };
            // A malformed observation belongs to this Wave's acquisition status.
            // Read the whole Wave before adding claims so readable siblings survive.
            let observations = (|| -> StoreResult<_> {
                let mut query = conn.prepare(
                    "SELECT p.id,p.initiative_ids,p.team_ids
                     FROM pm_wave_projects m JOIN pm_projects p ON p.id=m.project_id
                     WHERE m.wave_id=?1 AND p.repo=?2 AND p.provider=?3
                     AND p.archived=0 AND p.membership_unresolved=0 ORDER BY m.position,p.id",
                )?;
                let mut project_claims = query
                    .query_map(params![wave_id, repo, provider], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    })?
                    .map(|row| {
                        let (id, initiatives, teams) = row?;
                        Ok((
                            id,
                            serde_json::from_str::<Vec<String>>(&initiatives)?,
                            serde_json::from_str::<Vec<String>>(&teams)?,
                        ))
                    })
                    .collect::<StoreResult<Vec<_>>>()?;
                project_claims.retain(|(_, initiatives, _)| initiatives.contains(&initiative));
                let mut query = conn.prepare(
                    "SELECT i.id,i.identifier,i.project_id,i.team_id
                     FROM pm_items i JOIN pm_wave_projects m ON m.project_id=i.project_id
                     JOIN pm_projects p ON p.id=i.project_id AND p.repo=i.repo AND p.provider=i.provider
                     WHERE m.wave_id=?1 AND i.repo=?2 AND i.provider=?3 AND i.needs_refresh=0
                     AND p.archived=0 AND p.membership_unresolved=0
                     AND EXISTS(SELECT 1 FROM json_each(p.initiative_ids) WHERE value=?4)
                     AND NOT EXISTS(SELECT 1 FROM task_deletions d JOIN waves w ON w.id=d.wave_id
                                    WHERE w.repo=i.repo AND d.issue_id=i.id)
                     ORDER BY m.position,i.id",
                )?;
                let issue_claims = query
                    .query_map(params![wave_id, repo, provider, initiative], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                            row.get::<_, Option<String>>(3)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                Ok((project_claims, issue_claims))
            })();
            let (project_claims, issue_claims) = match observations {
                Ok(observations) => observations,
                Err(_) => continue,
            };
            // Missing or malformed Linear evidence must not make local Work
            // depend on readable repository files or Linear configuration.
            let expected_team = expected_team
                .as_ref()
                .map_err(|error| StoreError::InvalidData(error.to_string()))?
                .as_deref();
            for (id, initiatives, teams) in &project_claims {
                if initiatives.as_slice() != [initiative.as_str()] {
                    return Err(StoreError::InvalidData(format!(
                        "Linear Project {id} in wave/{wave} belongs to Initiatives [{}]; expected exactly {initiative}", initiatives.join(", ")
                    )));
                }
                if teams.len() != 1 || expected_team.is_some_and(|expected| teams[0] != expected) {
                    return Err(StoreError::InvalidData(format!(
                        "Linear Project {id} in wave/{wave} belongs to Teams [{}]; expected repository Team {}",
                        teams.join(", "), expected_team.unwrap_or("(one Team)")
                    )));
                }
            }
            for (_, identifier, project_id, team) in &issue_claims {
                let (_, _, teams) = project_claims.iter().find(|(id, _, _)| id == project_id)
                    .ok_or_else(|| StoreError::InvalidData(format!(
                        "Linear task {identifier} in wave/{wave} points to missing Project {project_id}"
                    )))?;
                if team.as_ref() != teams.first() {
                    return Err(StoreError::InvalidData(format!(
                        "Linear task {identifier} in wave/{wave} belongs to Team {}, but Project {project_id} belongs to Teams [{}]",
                        team.as_deref().unwrap_or("unmapped"), teams.join(", ")
                    )));
                }
            }
            if let Some(owner) = initiatives.insert(initiative.clone(), *wave) {
                return Err(StoreError::InvalidData(format!(
                    "Linear Initiative {initiative} is bound by both wave/{owner} and wave/{wave} observations"
                )));
            }
            for (project, _, _) in project_claims {
                if let Some(owner) = projects.insert(project.clone(), *wave) {
                    return Err(StoreError::InvalidData(format!(
                        "Linear Project {project} belongs to both wave/{owner} and wave/{wave} observations"
                    )));
                }
            }
            for (id, identifier, _, _) in issue_claims {
                if let Some(owner) = issues.insert(id, *wave) {
                    return Err(StoreError::InvalidData(format!(
                        "Linear task {identifier} belongs to both wave/{owner} and wave/{wave} observations"
                    )));
                }
            }
        }
        Ok(())
    }
}

// Both durable projections require the same confirmed Initiative/Wave association.
// A partial read supplies that association without advancing full-Wave freshness.
const ACCEPTED_WAVE_PROJECTS: &str = "
    SELECT p.*,m.wave_id FROM pm_projects p
    JOIN pm_wave_projects m ON m.project_id=p.id
    LEFT JOIN pm_wave_sync sync ON sync.wave_id=m.wave_id AND sync.provider=p.provider
    JOIN waves w ON w.id=m.wave_id AND w.repo=p.repo
    WHERE p.repo=?1 AND p.provider=?2 AND p.archived=0 AND p.membership_unresolved=0
    AND json_array_length(p.initiative_ids)=1
    AND json_extract(p.initiative_ids,'$[0]')=
        CASE WHEN m.wave_id=?4 THEN ?5 ELSE sync.initiative END";

/// Inputs select IDs only; project their accepted rows inside the ingestion transaction.
pub(crate) fn project_accepted_planning(
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
             workflow,krs,
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
                    Edit::Workflow(row.get(8)?),
                    Edit::Krs(serde_json::from_str(&row.get::<_, String>(9)?)?),
                    Edit::MetricTargets(serde_json::from_str(&row.get::<_, String>(10)?)?),
                    Edit::ProjectInitiatives(serde_json::from_str(&row.get::<_, String>(11)?)?),
                    Edit::ProjectTeams(serde_json::from_str(&row.get::<_, String>(12)?)?),
                ];
                if let Some(description) = row.get::<_, Option<String>>(13)? {
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
        super::planning_export::attach_in(tx, repo, true, &external, revision.as_deref(), &edits)?;
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
        super::planning_changes::PlanningChanges::Project(&id).reconcile_edits(
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
        "SELECT id,revision,name,description,assignee,project_id
         FROM pm_items WHERE repo=?1 AND provider='linear' AND needs_refresh=0
         AND id IN (SELECT value FROM json_each(?2))",
    )?;
    let observed = query
        .query_map(params![repo, item_ids], |row| {
            let mut edits = vec![
                Edit::TaskTitle(Some(row.get(2)?)),
                Edit::TaskDescription(Some(row.get(3)?)),
                Edit::TaskAssignee(row.get(4)?),
            ];
            if let Some(project) = row.get::<_, Option<String>>(5)? {
                edits.push(Edit::TaskProject(project));
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
        super::planning_export::attach_in(tx, repo, false, &id, revision.as_deref(), &edits)?;
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
         JOIN projects p ON p.external_project_id=i.project_id
         JOIN accepted observed ON observed.id=i.project_id AND observed.wave_id=p.wave_id
         LEFT JOIN tasks target ON target.external_issue_id=i.id
         LEFT JOIN projects current ON current.id=target.project_id
         WHERE i.repo=?1 AND i.provider=?2 AND i.needs_refresh=0
         AND i.id IN (SELECT value FROM json_each(?3))
         AND (target.id IS NULL OR current.wave_id=p.wave_id OR EXISTS(
             SELECT 1 FROM task_changes c WHERE c.task_id=target.id AND c.field='project_id'
             AND c.acknowledged=0 AND c.conflict_json IS NULL AND json_extract(c.value_json,'$')=current.id
             AND c.seq=(SELECT max(seq) FROM task_changes WHERE task_id=target.id AND field='project_id')))
         AND NOT EXISTS(SELECT 1 FROM task_deletions d
             WHERE d.wave_id=COALESCE(current.wave_id,p.wave_id) AND d.issue_id=i.id)
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
            super::task_state_delivery::reconcile_in(
                tx,
                &id,
                revision.as_deref(),
                &disposition,
                &body,
            )?;
            let pending: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM task_state_deliveries WHERE task_id=?1 AND settled=0
                 AND seq=(SELECT max(seq) FROM task_state_deliveries WHERE task_id=?1))",
                [id.as_str()],
                |row| row.get(0),
            )?;
            super::planning_changes::PlanningChanges::Task(&id).reconcile_edits(
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

pub(crate) fn associate_project(
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

pub(crate) fn require_accepted_initiative(
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

pub(crate) fn revision_nanos(revision: Option<&str>) -> StoreResult<Option<i64>> {
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

#[cfg(test)]
mod tests {
    use crate::build_info::MigrationAuthority;
    use crate::id::WaveId;
    use crate::store::sqlite::SqliteStore;
    use crate::store::{FrontierAdvance, PlanningState, Store};
    use crate::test_linear_input::SeedLinear;
    use rusqlite::{params, Connection};
    use serde_json::json;
    #[tokio::test]
    async fn migration_preserves_planning_identity_and_removes_snapshot_storage() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join(".lf/loopflow.db");
        std::fs::create_dir_all(database.parent().unwrap()).unwrap();
        let wave = WaveId::new();
        let snapshot = json!({"projects":[{
            "id":"project", "slug":"chapter", "name":"Chapter", "summary":"Proof",
            "metric_targets":[], "workflow":"feature", "status":"started", "krs":[],
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
        sqlite.apply_migration_for_test("planning_peers").unwrap();
        sqlite
            .place_work(
                &crate::durable::WorkRef::Wave(wave.clone()),
                &sqlite.local_machine().unwrap().id,
            )
            .unwrap();
        let store = Store { sqlite };
        let observation = store.linear_issue_evidence("/repo", "fix-1").await.unwrap();
        assert_eq!(observation.state, PlanningState::Available);
        let detail: crate::test_linear_input::IssueFixture =
            loopflow_test_support::retained_task_input(&database, "/repo", "fix-1");
        let list = crate::test_linear_input::InventoryFixture {
            wave_id: wave.clone(),
            provider: "linear".into(),
            initiative: "initiative".into(),
            synced_at: 42,
            snapshot: serde_json::from_value(snapshot).unwrap(),
        };
        assert_eq!(detail.observed_at, 42);
        assert_eq!(detail.item, list.snapshot.items[0]);
        assert_eq!(detail.item.name, "Retained title");
        assert_eq!(detail.item.description, "Retained notes");
        assert_eq!(detail.project.as_ref(), Some(&list.snapshot.projects[0]));
        assert_eq!(store.get_wave(&wave).await.unwrap().unwrap().id(), &wave);
        assert!(store.list_tasks(None).await.unwrap().is_empty());
        assert!(!Connection::open(&database)
            .unwrap()
            .prepare("SELECT payload FROM pm_snapshots")
            .is_ok());

        // A delayed list cannot resurrect a confirmed removal in either reader.
        store.sqlite.conn.lock().unwrap().execute("INSERT INTO task_deletions(wave_id,issue_id,identifier,confirmed_at) VALUES(?1,'issue','FIX-1',1)", [&wave]).unwrap();
        store.seed_inventory(list, None).await.unwrap();
        let removed = store.linear_issue_evidence("/repo", "FIX-1").await.unwrap();
        assert_eq!(removed.state, PlanningState::Removed);
        assert_eq!(removed.observed_at, Some(detail.observed_at));
        assert_eq!(
            loopflow_test_support::retained_task_input::<crate::test_linear_input::IssueFixture>(
                &database, "/repo", "FIX-1"
            ),
            detail
        );
        assert!(store.list_tasks(None).await.unwrap().is_empty());
    }
}
