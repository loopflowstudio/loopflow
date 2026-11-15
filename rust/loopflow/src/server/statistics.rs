//! Keep the store's planner statistics current. Unanalyzed, SQLite guesses
//! every table holds a million rows and can pick a scan over an index; a
//! bounded `PRAGMA optimize` measures the tables that changed. The server runs
//! it once when it starts and then about once a day, never per connection or
//! per command. Failures are logged and retried at the next tick.
use std::path::Path;
use std::time::{Duration, SystemTime};

use crate::store::sqlite::SqliteStore;

const EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// How often the thread checks the wall-clock deadline. `SystemTime`, unlike
/// `Instant`, keeps counting while a Mac sleeps.
const TICK: Duration = Duration::from_secs(10 * 60);

pub(super) fn start() {
    let spawned = std::thread::Builder::new()
        .name("store-statistics".into())
        .spawn(|| crate::journal::request::isolate(maintain));
    if let Err(error) = spawned {
        tracing::warn!(%error, "store statistics are not maintained by this server");
    }
}

fn maintain() {
    let path = crate::store::default_db_path();
    let mut due = SystemTime::now();
    loop {
        if SystemTime::now() >= due {
            match refresh(&path) {
                Ok(()) => due = SystemTime::now() + EVERY,
                Err(error) => tracing::warn!(%error, "store statistics refresh failed"),
            }
        }
        std::thread::sleep(TICK);
    }
}

fn refresh(path: &Path) -> anyhow::Result<()> {
    SqliteStore::new(path)?.refresh_statistics()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    #[test]
    fn a_refresh_measures_a_seeded_store() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("loopflow.db");
        let store = crate::store::sqlite::SqliteStore::open_ephemeral(&path).unwrap();
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
