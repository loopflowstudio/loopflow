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
/// How often the thread checks the wall-clock deadlines. `SystemTime`, unlike
/// `Instant`, keeps counting while a Mac sleeps.
const TICK: Duration = Duration::from_secs(10 * 60);

pub(super) fn start() {
    let spawned = std::thread::Builder::new()
        .name("store-maintenance".into())
        .spawn(|| loopflow_execution::journal::request::isolate(maintain));
    if let Err(error) = spawned {
        tracing::warn!(%error, "the store is not maintained by this server");
    }
}

/// Repositories with no local claims still receive new plans by Git.
const PLAN_EVERY: Duration = Duration::from_secs(60);

pub(super) fn start_plan_exchange() {
    let spawned = std::thread::Builder::new()
        .name("plan-git".into())
        .spawn(|| loopflow_execution::journal::request::isolate(exchange_plans));
    if let Err(error) = spawned {
        tracing::warn!(%error, "this server does not carry plans by Git");
    }
}

fn exchange_plans() {
    let path = loopflow_execution::store::default_db_path();
    loop {
        std::thread::sleep(PLAN_EVERY);
        if !path.exists() {
            continue;
        }
        let exchanged = loopflow_execution::store::open_sqlite(&path)
            .map_err(anyhow::Error::from)
            .and_then(|store| loopflow_execution::ops::plan_git::exchange_unowned(&store));
        if let Err(error) = exchanged {
            tracing::warn!(error = %format!("{error:#}"), "Git plan exchange did not run");
        }
    }
}

fn maintain() {
    let path = loopflow_execution::store::default_db_path();
    let mut statistics_due = SystemTime::now();
    let mut retention_due = SystemTime::now();
    loop {
        if SystemTime::now() >= retention_due {
            match retain(&path) {
                Ok(()) => retention_due = SystemTime::now() + RETENTION_EVERY,
                Err(error) => tracing::warn!(%error, "store retention failed"),
            }
        }
        if SystemTime::now() >= statistics_due {
            match refresh(&path) {
                Ok(()) => statistics_due = SystemTime::now() + STATISTICS_EVERY,
                Err(error) => tracing::warn!(%error, "store statistics refresh failed"),
            }
        }
        std::thread::sleep(TICK);
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
