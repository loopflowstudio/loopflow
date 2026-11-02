//! Consistent copies of a live store: the install switch's backup and the
//! candidate's isolated executable check both start from one.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use super::StoreResult;

/// A consistent copy of a live WAL database. A read transaction pins the
/// snapshot: the WAL cannot reset while it is held, so cloning the database
/// first and the WAL second yields committed state. APFS clones share blocks,
/// so this is instant at any size; elsewhere the online backup API copies.
pub(crate) fn snapshot(source: &Path, destination: &Path) -> StoreResult<()> {
    let pinned = Connection::open_with_flags(source, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    pinned.busy_timeout(std::time::Duration::from_secs(15))?;
    pinned.execute_batch("BEGIN; SELECT count(*) FROM sqlite_schema;")?;
    if clone_file(source, destination).is_ok() {
        let wal = wal_path(source);
        if !wal.exists() || clone_file(&wal, &wal_path(destination)).is_ok() {
            return Ok(());
        }
        let _ = std::fs::remove_file(destination);
    }
    let mut copy = Connection::open(destination)?;
    rusqlite::backup::Backup::new(&pinned, &mut copy)?.run_to_completion(
        256,
        std::time::Duration::from_millis(10),
        None,
    )?;
    Ok(())
}

fn wal_path(db: &Path) -> PathBuf {
    let mut wal = db.as_os_str().to_owned();
    wal.push("-wal");
    PathBuf::from(wal)
}

#[cfg(target_os = "macos")]
fn clone_file(source: &Path, destination: &Path) -> std::io::Result<()> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;
    let source = CString::new(source.as_os_str().as_bytes())?;
    let destination = CString::new(destination.as_os_str().as_bytes())?;
    // SAFETY: both arguments are NUL-terminated paths that outlive the call,
    // and clonefile reads them without retaining either pointer.
    if unsafe { libc::clonefile(source.as_ptr(), destination.as_ptr(), 0) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(target_os = "macos"))]
fn clone_file(_source: &Path, _destination: &Path) -> std::io::Result<()> {
    Err(std::io::ErrorKind::Unsupported.into())
}

#[cfg(test)]
mod tests {
    use super::snapshot;
    use rusqlite::Connection;

    #[test]
    fn snapshot_keeps_rows_committed_only_to_the_wal() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("live.db");
        let writer = Connection::open(&source).unwrap();
        writer
            .execute_batch(
                "PRAGMA journal_mode=WAL; PRAGMA wal_autocheckpoint=0;
                 CREATE TABLE t(v INTEGER); INSERT INTO t VALUES(1),(2),(3);",
            )
            .unwrap();
        let copy = dir.path().join("copy.db");
        snapshot(&source, &copy).unwrap();
        drop(writer);
        let copied: i64 = Connection::open(&copy)
            .unwrap()
            .query_row("SELECT count(*) FROM t", [], |row| row.get(0))
            .unwrap();
        assert_eq!(copied, 3);
    }
}
