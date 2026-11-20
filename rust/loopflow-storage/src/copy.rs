//! Consistent copies of a live store: the install switch's backup and the
//! candidate's isolated executable check both start from one. A copy that is
//! sealed stays as it was taken: its file mode is how a server knows it.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use rusqlite::{Connection, OpenFlags};

use super::StoreResult;

/// A consistent copy of a live WAL database. A read transaction pins the
/// snapshot: the WAL cannot reset while it is held, so cloning the database
/// first and the WAL second yields committed state. APFS clones share blocks,
/// so this is instant at any size; elsewhere the online backup API copies.
pub fn snapshot(source: &Path, destination: &Path) -> StoreResult<()> {
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

/// Make the copy at `store` read-only as of now: fold its WAL into the file,
/// stamp the file with the time and take every write bit off it.
pub fn seal(store: &Path) -> StoreResult<()> {
    Connection::open(store)?.query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))?;
    let io = |error: std::io::Error| {
        super::StoreError::InvalidData(format!("seal {}: {error}", store.display()))
    };
    let file = std::fs::File::options()
        .write(true)
        .open(store)
        .map_err(io)?;
    file.set_modified(SystemTime::now()).map_err(io)?;
    let mut permissions = file.metadata().map_err(io)?.permissions();
    permissions.set_readonly(true);
    file.set_permissions(permissions).map_err(io)
}

/// When the sealed copy at `store` was taken; `None` for a store that can be
/// written, or that is absent.
pub fn sealed_at(store: &Path) -> Option<SystemTime> {
    let metadata = std::fs::metadata(store).ok()?;
    metadata
        .permissions()
        .readonly()
        .then(|| metadata.modified().ok())
        .flatten()
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
    use super::{seal, sealed_at, snapshot};
    use rusqlite::Connection;

    #[test]
    fn a_sealed_copy_reads_and_refuses_writes() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("live.db");
        Connection::open(&source)
            .unwrap()
            .execute_batch(
                "PRAGMA journal_mode=WAL; CREATE TABLE t(v INTEGER); INSERT INTO t VALUES(1);",
            )
            .unwrap();
        let copy = dir.path().join("copy.db");
        snapshot(&source, &copy).unwrap();
        assert_eq!(sealed_at(&copy), None);
        let before = std::time::SystemTime::now() - std::time::Duration::from_secs(2);
        seal(&copy).unwrap();
        assert!(sealed_at(&copy).unwrap() >= before);
        let reader = Connection::open(&copy).unwrap();
        let rows: i64 = reader
            .query_row("SELECT count(*) FROM t", [], |row| row.get(0))
            .unwrap();
        assert_eq!(rows, 1);
        assert!(reader.execute("INSERT INTO t VALUES(2)", []).is_err());
    }

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
