//! The last known state of each Linear issue, kept apart from the Work it updated and from queued tracker actions.
pub use crate::planning::revision_nanos;
use crate::sqlite::{planning_write, SqliteStore};
use crate::work::edit::WorkValue as Edit;
use crate::{PlanningState, StoreError, StoreResult};
use loopflow_model::id::WaveId;
use rusqlite::{params, OptionalExtension};

impl SqliteStore {
    pub fn record_tracker_event(
        &self,
        issue_id: &str,
        revision: Option<&str>,
        removed: bool,
    ) -> StoreResult<()> {
        let revision = revision_nanos(revision)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO tracker_events(issue_id,revision_ns,removed) VALUES(?1,?2,?3)
             ON CONFLICT(issue_id) DO UPDATE SET
             revision_ns=CASE WHEN excluded.revision_ns > revision_ns OR revision_ns IS NULL
                 THEN excluded.revision_ns ELSE revision_ns END,
             removed=MAX(removed,excluded.removed)",
            params![issue_id, revision, removed],
        )?;
        if removed {
            let ids = {
                let mut q=tx.prepare_cached("SELECT id FROM tasks WHERE external_issue_id=?1 AND planning_deleted_at IS NULL")?;
                let rows = q.query_map([issue_id], |r| r.get::<_, String>(0))?;
                rows.collect::<Result<Vec<_>, _>>()?
            };
            for id in ids {
                planning_write::tracker(
                    &tx,
                    &crate::work::edit::WorkEdit::for_target(
                        &id,
                        &[Edit::TaskDeletedAt(Some(crate::rows::now_unix()))],
                    ),
                )?;
            }
        }
        let mut query = tx.prepare_cached(
            "SELECT repo,revision FROM tracker_items WHERE tracker IN ('linear','asana') AND id=?1",
        )?;
        let rows = query
            .query_map([issue_id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for (repo, cached_revision) in rows {
            let cached = revision_nanos(cached_revision.as_deref())?;
            if removed || revision.is_none() || cached < revision {
                tx.execute("UPDATE tracker_items SET needs_refresh=1 WHERE repo=?1 AND tracker IN ('linear','asana') AND id=?2", params![repo,issue_id])?;
            }
        }
        drop(query);
        tx.commit()?;
        Ok(())
    }

    pub fn invalidate_linear_issue(
        &self,
        repo: &str,
        expected: &crate::LinearIssueState,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // A null detail response invalidates cached admission, not tracker history.
        // Only a complete detail observation can repair it; list omission is ambiguous.
        conn.execute(
            "UPDATE tracker_items SET needs_refresh=1 WHERE repo=?1 AND tracker IN ('linear','asana') AND id=?2
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

    pub fn linear_issue_state(
        &self,
        repo: &str,
        selector: &str,
    ) -> StoreResult<crate::LinearIssueState> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare_cached(
            "SELECT i.id,i.revision,i.observed_at,
             p.team_ids,i.team_id,
             i.needs_refresh OR COALESCE(p.archived,0)
             OR NOT i.project_id_observed OR i.claimed_project_id IS NOT i.project_id
             OR (i.project_id IS NOT NULL AND p.id IS NULL),
             EXISTS(SELECT 1 FROM tracker_events c WHERE c.issue_id=i.id AND c.removed=1),p.id
             FROM tracker_items i LEFT JOIN tracker_projects p
             ON p.repo=i.repo AND p.tracker=i.tracker AND p.id=i.project_id
             WHERE i.repo=?1 AND i.tracker IN ('linear','asana') AND (i.id=?2 OR i.identifier=?2)",
        )?;
        let rows = query
            .query_map(params![repo, selector], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, bool>(5)?,
                    row.get::<_, bool>(6)?,
                    row.get::<_, Option<String>>(7)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if rows.len() > 1 {
            return Err(StoreError::InvalidData(format!(
                "ambiguous planning selector {selector:?}; use a Task UUID"
            )));
        }
        let Some((id, revision, observed_at, teams, team_id, invalid, removed, project_id)) =
            rows.into_iter().next()
        else {
            let (removed, changed) = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM tracker_events WHERE issue_id=?1 AND removed=1),
                 EXISTS(SELECT 1 FROM tracker_events WHERE issue_id=?1)",
                params![selector],
                |row| Ok((row.get::<_, bool>(0)?, row.get::<_, bool>(1)?)),
            )?;
            return Ok(crate::LinearIssueState {
                issue_id: None,
                revision: None,
                observed_at: None,
                project_id: None,
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
        Ok(crate::LinearIssueState {
            issue_id: Some(id),
            revision,
            observed_at: Some(observed_at),
            project_id,
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
    pub fn validate_linear_ownership(
        &self,
        waves: &[(&WaveId, &str)],
        expected_team: StoreResult<Option<String>>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        let mut projects = std::collections::BTreeMap::new();
        let mut issues = std::collections::BTreeMap::new();
        for (wave_id, wave) in waves {
            let metadata = conn
                .query_row(
                    "SELECT w.repo,s.tracker FROM tracker_wave_sync s
                 JOIN waves w ON w.id=s.wave_id WHERE s.wave_id=?1",
                    [wave_id.as_str()],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
                )
                .optional()?;
            let Some((repo, tracker)) = metadata else {
                continue;
            };
            // A malformed observation belongs to this Wave's acquisition status.
            // Read the whole Wave before adding claims so readable siblings survive.
            let observations = (|| -> StoreResult<_> {
                let mut query = conn.prepare_cached(
                    "SELECT p.id,p.team_ids
                     FROM projects l JOIN tracker_projects p ON p.id=l.external_project_id
                     WHERE l.wave_id=?1 AND p.repo=?2 AND p.tracker=?3 AND p.archived=0",
                )?;
                let project_claims = query
                    .query_map(params![wave_id, repo, tracker], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?
                    .map(|row| {
                        let (id, teams) = row?;
                        Ok((id, serde_json::from_str::<Vec<String>>(&teams)?))
                    })
                    .collect::<StoreResult<Vec<_>>>()?;
                let mut query = conn.prepare_cached(
                    "SELECT i.id,i.identifier,i.project_id,i.team_id
                     FROM projects l JOIN tracker_items i ON i.project_id=l.external_project_id
                     JOIN tracker_projects p ON p.id=i.project_id AND p.repo=i.repo AND p.tracker=i.tracker
                     WHERE l.wave_id=?1 AND i.repo=?2 AND i.tracker=?3 AND i.needs_refresh=0
                     AND p.archived=0 ORDER BY i.id",
                )?;
                let issue_claims = query
                    .query_map(params![wave_id, repo, tracker], |row| {
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
            let (mut project_claims, mut issue_claims) = match observations {
                Ok(observations) => observations,
                Err(_) => continue,
            };
            // Missing or malformed Linear evidence must not make local Work
            // depend on readable repository files or Linear configuration.
            let expected_team = expected_team
                .as_ref()
                .map_err(|error| StoreError::InvalidData(error.to_string()))?
                .as_deref();
            // Foreign historical observations do not claim this repository's
            // Work. Their Task-specific planning evidence still refuses mutations;
            // they must not prevent unrelated local Tasks from being read.
            project_claims.retain(|(_, teams)| {
                teams.len() == 1 && expected_team.is_none_or(|expected| teams[0] == expected)
            });
            issue_claims.retain(|(_, _, project_id, team)| {
                project_claims
                    .iter()
                    .any(|(id, teams)| id == project_id && team.as_ref() == teams.first())
            });
            for (project, _) in project_claims {
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
