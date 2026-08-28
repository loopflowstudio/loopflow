//! SQLite persistence for watched pull-request landings.

use std::path::PathBuf;

use rusqlite::{params, types::Type, OptionalExtension, TransactionBehavior};
use time::OffsetDateTime;

use crate::durable::HomeId;
use crate::pr_landing::{
    LandingClaim, LandingPlacement, LandingSupervisor, PrLanding, PrLandingId,
};
use crate::store::{StoreError, StoreResult};
use crate::work::task::{AfterMerge, TaskId};

fn timestamp(value: OffsetDateTime) -> i64 {
    value.unix_timestamp()
}

fn datetime(index: usize, value: i64) -> rusqlite::Result<OffsetDateTime> {
    OffsetDateTime::from_unix_timestamp(value).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(index, Type::Integer, Box::new(error))
    })
}

fn invalid_column(
    index: usize,
    error: impl std::error::Error + Send + Sync + 'static,
) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(index, Type::Text, Box::new(error))
}

fn map_landing(row: &rusqlite::Row<'_>) -> rusqlite::Result<PrLanding> {
    let generation = row.get::<_, i64>(12)? as u64;
    let placement = match row.get::<_, Option<String>>(13)?.as_deref() {
        None => None,
        Some("local") => Some(LandingPlacement::Local),
        Some("home") => {
            let home = row.get::<_, Option<String>>(14)?.ok_or_else(|| {
                invalid_column(
                    14,
                    crate::durable::DurableDataError::InvalidId(
                        "home supervisor has no Home id".to_string(),
                    ),
                )
            })?;
            Some(LandingPlacement::Home {
                home_id: HomeId::parse(&home).map_err(|error| invalid_column(14, error))?,
            })
        }
        Some(value) => {
            return Err(invalid_column(
                13,
                crate::pr_landing::PrLandingDataError::InvalidInvariant(format!(
                    "invalid stored landing placement: {value}"
                )),
            ))
        }
    };
    let supervisor = match placement {
        Some(placement) => Some(LandingSupervisor {
            placement,
            process_id: row.get::<_, i64>(15)? as u32,
            generation,
            heartbeat_at: datetime(16, row.get(16)?)?,
        }),
        None => None,
    };
    let state = row
        .get::<_, String>(11)?
        .parse()
        .map_err(|error| invalid_column(11, error))?;
    let after_merge = row
        .get::<_, Option<String>>(9)?
        .map(|value| value.parse())
        .transpose()
        .map_err(|error| invalid_column(9, error))?;
    Ok(PrLanding {
        id: PrLandingId::from_raw(row.get::<_, String>(0)?),
        repo: row.get(1)?,
        pr_number: row.get::<_, i64>(2)? as u32,
        worktree: PathBuf::from(row.get::<_, String>(3)?),
        branch: row.get(4)?,
        task_id: row.get::<_, Option<String>>(5)?.map(TaskId::from_raw),
        requested_head_sha: row.get(6)?,
        observed_head_sha: row.get(7)?,
        merge_commit: row.get(8)?,
        after_merge,
        next_slug: row.get(10)?,
        state,
        generation,
        supervisor,
        repair_count: row.get::<_, i64>(17)? as u32,
        blocked_reason: row.get(18)?,
        created_at: datetime(19, row.get(19)?)?,
        updated_at: datetime(20, row.get(20)?)?,
    })
}

const LANDING_COLUMNS: &str = "
    id, repo, pr_number, worktree, branch, task_id,
    requested_head_sha, observed_head_sha, merge_commit, after_merge,
    next_slug, state, generation, supervisor_placement, supervisor_home_id,
    supervisor_process_id, supervisor_heartbeat_at, repair_count,
    blocked_reason, created_at, updated_at";

impl super::SqliteStore {
    pub fn start_or_join_pr_landing(&self, landing: &PrLanding) -> StoreResult<PrLanding> {
        landing
            .validate()
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing = transaction
            .query_row(
                &format!(
                    "SELECT {LANDING_COLUMNS} FROM pr_landings
                     WHERE repo=?1 AND pr_number=?2
                       AND state IN ('watching', 'repairing')"
                ),
                params![landing.repo, i64::from(landing.pr_number)],
                map_landing,
            )
            .optional()?;
        if let Some(existing) = existing {
            if existing.task_id != landing.task_id {
                return Err(StoreError::InvalidData(format!(
                    "active landing {} belongs to a different Task identity",
                    existing.id
                )));
            }
            transaction.execute(
                "UPDATE pr_landings
                 SET requested_head_sha=?2, after_merge=?3, next_slug=?4, updated_at=?5
                 WHERE id=?1 AND generation=?6 AND state IN ('watching', 'repairing')",
                params![
                    existing.id.as_str(),
                    landing.requested_head_sha,
                    landing.after_merge.map(AfterMerge::as_str),
                    landing.next_slug,
                    timestamp(landing.updated_at),
                    existing.generation as i64,
                ],
            )?;
            let joined = transaction.query_row(
                &format!("SELECT {LANDING_COLUMNS} FROM pr_landings WHERE id=?1"),
                [existing.id.as_str()],
                map_landing,
            )?;
            transaction.commit()?;
            return Ok(joined);
        }
        let settled_same_head = transaction
            .query_row(
                &format!(
                    "SELECT {LANDING_COLUMNS} FROM pr_landings
                     WHERE repo=?1 AND pr_number=?2 AND requested_head_sha=?3
                     ORDER BY created_at DESC LIMIT 1"
                ),
                params![
                    landing.repo,
                    i64::from(landing.pr_number),
                    landing.requested_head_sha
                ],
                map_landing,
            )
            .optional()?;
        if let Some(existing) = settled_same_head {
            transaction.commit()?;
            return Ok(existing);
        }
        transaction.execute(
            "INSERT INTO pr_landings (
                id, repo, pr_number, worktree, branch, task_id,
                requested_head_sha, observed_head_sha, merge_commit, after_merge,
                next_slug, state, generation, supervisor_placement, supervisor_home_id,
                supervisor_process_id, supervisor_heartbeat_at, repair_count,
                blocked_reason, created_at, updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?10, ?11, ?12,
                NULL, NULL, NULL, NULL, 0, NULL, ?13, ?13
             )",
            params![
                landing.id.as_str(),
                landing.repo,
                i64::from(landing.pr_number),
                landing.worktree.display().to_string(),
                landing.branch,
                landing.task_id.as_ref().map(TaskId::as_str),
                landing.requested_head_sha,
                landing.observed_head_sha,
                landing.after_merge.map(AfterMerge::as_str),
                landing.next_slug,
                landing.state.as_str(),
                landing.generation as i64,
                timestamp(landing.created_at),
            ],
        )?;
        transaction.commit()?;
        Ok(landing.clone())
    }

    pub fn get_pr_landing(&self, landing_id: &PrLandingId) -> StoreResult<Option<PrLanding>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            &format!("SELECT {LANDING_COLUMNS} FROM pr_landings WHERE id=?1"),
            [landing_id.as_str()],
            map_landing,
        )
        .optional()
        .map_err(StoreError::from)
    }

    pub fn recoverable_pr_landings(
        &self,
        stale_before: OffsetDateTime,
    ) -> StoreResult<Vec<PrLanding>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(&format!(
            "SELECT {LANDING_COLUMNS} FROM pr_landings
             WHERE state IN ('watching', 'repairing')
               AND (supervisor_process_id IS NULL OR supervisor_heartbeat_at <= ?1)
             ORDER BY created_at"
        ))?;
        let rows = statement.query_map([timestamp(stale_before)], map_landing)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn claim_pr_landing(
        &self,
        landing_id: &PrLandingId,
        expected_generation: u64,
        claim: &LandingClaim,
        stale_before: OffsetDateTime,
    ) -> StoreResult<Option<PrLanding>> {
        if claim.process_id == 0 {
            return Err(StoreError::InvalidData(
                "landing supervisor process id cannot be zero".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let transaction = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = transaction
            .query_row(
                &format!("SELECT {LANDING_COLUMNS} FROM pr_landings WHERE id=?1"),
                [landing_id.as_str()],
                map_landing,
            )
            .optional()?;
        let Some(current) = current else {
            transaction.commit()?;
            return Ok(None);
        };
        if current.state.is_terminal() || current.generation != expected_generation {
            transaction.commit()?;
            return Ok(None);
        }
        let stale = current
            .supervisor
            .as_ref()
            .is_some_and(|owner| owner.heartbeat_at <= stale_before);
        if current.supervisor.is_some() && !stale {
            transaction.commit()?;
            return Ok(None);
        }
        let generation = if stale {
            expected_generation.checked_add(1).ok_or_else(|| {
                StoreError::InvalidData("landing generation is exhausted".to_string())
            })?
        } else {
            expected_generation
        };
        let now = claim.heartbeat_at;
        let changed = transaction.execute(
            "UPDATE pr_landings
             SET generation=?2, supervisor_placement=?3, supervisor_home_id=?4,
                 supervisor_process_id=?5, supervisor_heartbeat_at=?6, updated_at=?6
             WHERE id=?1 AND generation=?7 AND state IN ('watching', 'repairing')",
            params![
                landing_id.as_str(),
                generation as i64,
                claim.placement.storage_str(),
                claim.placement.home_id().map(HomeId::as_str),
                i64::from(claim.process_id),
                timestamp(now),
                expected_generation as i64,
            ],
        )?;
        let claimed = if changed == 1 {
            transaction.query_row(
                &format!("SELECT {LANDING_COLUMNS} FROM pr_landings WHERE id=?1"),
                [landing_id.as_str()],
                map_landing,
            )?
        } else {
            transaction.commit()?;
            return Ok(None);
        };
        transaction.commit()?;
        Ok(Some(claimed))
    }

    pub fn heartbeat_pr_landing(
        &self,
        landing_id: &PrLandingId,
        expected_generation: u64,
        now: OffsetDateTime,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE pr_landings
             SET supervisor_heartbeat_at=?3, updated_at=?3
             WHERE id=?1 AND generation=?2
               AND state IN ('watching', 'repairing')
               AND supervisor_process_id IS NOT NULL",
            params![
                landing_id.as_str(),
                expected_generation as i64,
                timestamp(now)
            ],
        )? == 1)
    }

    pub fn update_pr_landing(&self, landing: &PrLanding) -> StoreResult<bool> {
        landing
            .validate()
            .map_err(|error| StoreError::InvalidData(error.to_string()))?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.execute(
            "UPDATE pr_landings
             SET state=?3, observed_head_sha=?4, merge_commit=?5,
                 repair_count=?6, blocked_reason=?7,
                 supervisor_heartbeat_at=?8, updated_at=?8
             WHERE id=?1 AND generation=?2
               AND state IN ('watching', 'repairing')",
            params![
                landing.id.as_str(),
                landing.generation as i64,
                landing.state.as_str(),
                landing.observed_head_sha,
                landing.merge_commit,
                i64::from(landing.repair_count),
                landing.blocked_reason,
                timestamp(landing.updated_at),
            ],
        )? == 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pr_landing::{NewPrLanding, PrLandingState};
    use crate::store::migrations::migration_sql_for_test;
    use crate::store::sqlite::SqliteStore;

    fn landing(now: OffsetDateTime) -> PrLanding {
        PrLanding::new(
            NewPrLanding {
                repo: "loopflowstudio/loopflow".to_string(),
                pr_number: 248,
                worktree: PathBuf::from("/tmp/loopflow.make-pr-landing-a-watched"),
                branch: "jack/make-pr-landing-a-watched".to_string(),
                task_id: None,
                requested_head_sha: "head-a".to_string(),
                after_merge: None,
                next_slug: None,
            },
            now,
        )
        .unwrap()
    }

    fn store() -> (tempfile::TempDir, SqliteStore) {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("registry.db");
        let store = SqliteStore::new(&path).unwrap();
        let conn = rusqlite::Connection::open(path).unwrap();
        let migrated = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type='table' AND name='pr_landings')",
                [],
                |row| row.get::<_, bool>(0),
            )
            .unwrap();
        if !migrated {
            conn.execute_batch(&migration_sql_for_test("pr_landings"))
                .unwrap();
        }
        (directory, store)
    }

    #[test]
    fn landing_claims_join_and_fence_stale_generations() {
        let (_directory, store) = store();
        let now = OffsetDateTime::now_utc();
        let candidate = landing(now);
        let created = store.start_or_join_pr_landing(&candidate).unwrap();
        let joined = store.start_or_join_pr_landing(&landing(now)).unwrap();
        assert_eq!(joined.id, created.id);

        let local = LandingClaim {
            placement: LandingPlacement::Local,
            process_id: 41,
            heartbeat_at: now,
        };
        let claimed = store
            .claim_pr_landing(&created.id, 1, &local, now - time::Duration::minutes(1))
            .unwrap()
            .unwrap();
        assert_eq!(claimed.generation, 1);
        assert!(store
            .claim_pr_landing(
                &created.id,
                1,
                &LandingClaim {
                    placement: LandingPlacement::Local,
                    process_id: 42,
                    heartbeat_at: now,
                },
                now - time::Duration::minutes(1),
            )
            .unwrap()
            .is_none());

        let replacement = store
            .claim_pr_landing(
                &created.id,
                1,
                &LandingClaim {
                    placement: LandingPlacement::Local,
                    process_id: 42,
                    heartbeat_at: now + time::Duration::minutes(2),
                },
                now + time::Duration::minutes(1),
            )
            .unwrap()
            .unwrap();
        assert_eq!(replacement.generation, 2);
        let mut stale = claimed;
        stale.state = PrLandingState::Merged;
        stale.merge_commit = Some("merge-a".to_string());
        assert!(!store.update_pr_landing(&stale).unwrap());
        let mut merged = replacement;
        merged.state = PrLandingState::Merged;
        merged.merge_commit = Some("merge-a".to_string());
        assert!(store.update_pr_landing(&merged).unwrap());

        let same_head = store.start_or_join_pr_landing(&landing(now)).unwrap();
        assert_eq!(same_head.id, created.id);
        let mut next_head = landing(now);
        next_head.requested_head_sha = "head-b".to_string();
        next_head.observed_head_sha = "head-b".to_string();
        let next_head = store.start_or_join_pr_landing(&next_head).unwrap();
        assert_ne!(next_head.id, created.id);
    }

    #[test]
    fn joining_active_landing_refreshes_head_and_task_disposition() {
        let (_directory, store) = store();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("PRAGMA foreign_keys=OFF")
            .unwrap();
        let now = OffsetDateTime::now_utc();
        let mut initial = landing(now);
        initial.task_id = Some(TaskId::new());
        initial.after_merge = Some(AfterMerge::CompleteTask);
        let created = store.start_or_join_pr_landing(&initial).unwrap();

        let mut revised = initial;
        revised.requested_head_sha = "head-b".to_string();
        revised.observed_head_sha = "head-b".to_string();
        revised.after_merge = Some(AfterMerge::ContinueTask);
        revised.next_slug = Some("follow-up-proof".to_string());
        let joined = store.start_or_join_pr_landing(&revised).unwrap();

        assert_eq!(joined.id, created.id);
        assert_eq!(joined.requested_head_sha, "head-b");
        assert_eq!(joined.after_merge, Some(AfterMerge::ContinueTask));
        assert_eq!(joined.next_slug.as_deref(), Some("follow-up-proof"));
    }
}
