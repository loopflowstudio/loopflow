//! Store upkeep on the server's clock. Planner statistics: unanalyzed, SQLite
//! guesses every table holds a million rows and can pick a scan over an index;
//! a bounded `PRAGMA optimize` measures the tables that changed, once when the
//! server starts and then about once a day. Retention: the store prune that
//! `lf machine prune` runs, at start and then every hour, so `history.db`
//! keeps 90 days and history an earlier lf wrote into `loopflow.db` moves out.
//! Neither runs per connection or per command. Failures are logged and retried
//! at the next tick. The Git plan exchange shares this clock: once a minute
//! for every repository without Linear or Asana that has a remote.
use std::path::Path;
use std::time::{Duration, SystemTime};

const STATISTICS_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
const RETENTION_EVERY: Duration = Duration::from_secs(60 * 60);
/// Active repositories with no local claims receive new plans by Git on this cadence.
const PLAN_EVERY: Duration = Duration::from_secs(60);

pub(super) fn start() {
    let spawned = std::thread::Builder::new()
        .name("store-maintenance".into())
        .spawn(|| loopflow_execution::journal::request::isolate(maintain));
    if let Err(error) = spawned {
        tracing::warn!(%error, "the store is not maintained by this server");
    }
}

fn maintain() {
    let path = loopflow_execution::store::default_db_path();
    let now = SystemTime::now();
    let mut statistics_due = now;
    let mut retention_due = now;
    let mut plan_due = now + PLAN_EVERY;
    loop {
        let now = SystemTime::now();
        if now >= retention_due {
            match retain(&path) {
                Ok(()) => retention_due = SystemTime::now() + RETENTION_EVERY,
                Err(error) => {
                    tracing::warn!(%error, "store retention failed");
                    retention_due = SystemTime::now() + PLAN_EVERY;
                }
            }
        }
        if SystemTime::now() >= statistics_due {
            match refresh(&path) {
                Ok(()) => statistics_due = SystemTime::now() + STATISTICS_EVERY,
                Err(error) => {
                    tracing::warn!(%error, "store statistics refresh failed");
                    statistics_due = SystemTime::now() + PLAN_EVERY;
                }
            }
        }
        if SystemTime::now() >= plan_due {
            if path.exists() {
                let exchanged = loopflow_execution::store::open_sqlite(&path)
                    .map_err(anyhow::Error::from)
                    .and_then(|store| loopflow_execution::ops::plan_git::exchange_unowned(&store));
                if let Err(error) = exchanged {
                    tracing::warn!(error = %format!("{error:#}"), "Git plan exchange did not run");
                }
            }
            plan_due = SystemTime::now() + PLAN_EVERY;
        }
        let next = retention_due.min(statistics_due).min(plan_due);
        if let Ok(wait) = next.duration_since(SystemTime::now()) {
            if !wait.is_zero() {
                std::thread::sleep(wait);
            }
        }
    }
}

fn retain(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let report = loopflow_execution::store::retention::prune(path, now)?;
    tracing::info!(?report, "store retention");
    Ok(())
}

fn refresh(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    loopflow_execution::store::open_sqlite(path)?.refresh_statistics()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_refresh_measures_a_seeded_store() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("loopflow.db");
        let store = loopflow_execution::store::opening::open_ephemeral(&path).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "INSERT INTO repository_plans(id,repo,selected)
                 WITH RECURSIVE n(i) AS (SELECT 0 UNION ALL SELECT i+1 FROM n WHERE i<999)
                 SELECT 'repo_'||printf('%032x',i),'/repo-'||i,0 FROM n",
            )
            .unwrap();
        drop(store);
        super::refresh(&path).unwrap();
        let measured: i64 = rusqlite::Connection::open(&path)
            .unwrap()
            .query_row(
                "SELECT count(*) FROM sqlite_stat1 WHERE tbl='repository_plans'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(measured > 0);
    }
}
