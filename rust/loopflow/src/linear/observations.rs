//! Retained Linear acquisition evidence, independent of accepted Work and delivery.
use crate::id::WaveId;
use crate::store::sqlite::{planning_write, SqliteStore};
use crate::store::{PlanningState, StoreError, StoreResult};
use crate::work::edit::WorkValue as Edit;
use rusqlite::{params, OptionalExtension};

impl SqliteStore {
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
                        &[Edit::TaskDeletedAt(Some(crate::store::rows::now_unix()))],
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
