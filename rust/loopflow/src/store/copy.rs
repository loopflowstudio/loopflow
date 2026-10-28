//! A build carrying unreleased migration drafts never opens the shared store.
//!
//! It works on `~/.lf/copies/<draft digest>/loopflow.db`, a snapshot of the
//! shared store with this build's drafts applied. The digest names the exact
//! draft set, so editing a draft gets a fresh copy and every lf process of
//! one build finds the same one. Everything else in `~/.lf` (captures, account
//! homes) stays shared: agents run on real credentials against real history.
//!
//! A copy acts on nothing outside this Machine. Its processes get no GitHub
//! credentials and a push URL that cannot resolve, and the copy holds no Linear
//! token, so planning, PR and push effects fail rather than act twice.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};

use super::{StoreError, StoreResult};

const COPIES: &str = "copies";
/// Names a snapshot's staging directory until it is renamed into place.
const STAGING: &str = ".tmp";

/// The copy this build works on, when it must use one: a validation-only
/// build with drafts on the Machine's default home.
pub(crate) fn selected() -> Option<PathBuf> {
    if super::custom_home_selected()
        || crate::build_info::migration_authority()
            == crate::build_info::MigrationAuthority::Published
    {
        return None;
    }
    let drafts = crate::build_info::migration_draft_manifest();
    (!drafts.is_empty()).then(|| copy_dir(&super::lf_home_dir(), drafts).join("loopflow.db"))
}

/// Make the selected copy if it is missing and isolate this process's remote
/// effects. Runs once at startup, before any thread or child exists.
///
/// # Errors
/// When the shared store cannot be snapshotted or this build's drafts fail on it.
pub fn enter() -> StoreResult<()> {
    let Some(path) = selected() else {
        return Ok(());
    };
    let dir = path.parent().expect("copy database has a directory");
    if !path.exists() {
        eprintln!(
            "lf: this build has unreleased migrations; copying {} to {}",
            super::lf_home_dir().join("loopflow.db").display(),
            dir.display()
        );
        create(&super::lf_home_dir().join("loopflow.db"), dir)?;
    }
    isolate_remotes(dir)?;
    eprintln!(
        "lf: working on the copy in {}; work here is not in the shared store (remove that directory to refresh it)",
        dir.display()
    );
    Ok(())
}

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

fn create(source: &Path, dir: &Path) -> StoreResult<()> {
    let parent = dir.parent().expect("copy directory has a parent");
    std::fs::create_dir_all(parent).map_err(io)?;
    let _lock = lock(&parent.join(".lock"))?;
    if dir.join("loopflow.db").exists() {
        return Ok(());
    }
    // Holding the lock, every staging directory is a killed snapshot's.
    for entry in std::fs::read_dir(parent).map_err(io)?.flatten() {
        let staging = entry.file_name().to_string_lossy().starts_with(STAGING);
        if staging && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
    let staging = tempfile::Builder::new()
        .prefix(STAGING)
        .tempdir_in(parent)
        .map_err(io)?;
    let db = staging.path().join("loopflow.db");
    snapshot(source, &db)?;
    let conn = Connection::open(&db)?;
    super::migrations::prepare_clone_sqlite(&conn)?;
    sanitize(&conn)?;
    drop(conn);
    std::fs::rename(staging.path(), dir).map_err(io)?;
    let _ = staging.keep();
    Ok(())
}

/// The shared store's processes are not the copy's: forget their OS identity
/// so no copy command signals or attaches to them. The copy cannot reach
/// Linear, so it keeps no Linear token to rotate behind the shared store's back.
fn sanitize(conn: &Connection) -> StoreResult<()> {
    conn.execute_batch(
        "BEGIN;
         UPDATE agent_sessions SET agent_process_id=NULL;
         UPDATE processes SET pid=NULL, os_started_at=NULL, endpoint=NULL,
             attached_lf_process_id=NULL, attachment_token=NULL;
         DELETE FROM session_activity;
         DELETE FROM provider_tokens WHERE provider='linear';
         COMMIT;",
    )?;
    Ok(())
}

/// Children inherit an empty GitHub CLI config and a push URL rewrite that
/// matches every remote, so `gh` and `git push` fail from anything run here.
fn isolate_remotes(dir: &Path) -> StoreResult<()> {
    let gh = dir.join("gh");
    std::fs::create_dir_all(&gh).map_err(io)?;
    std::env::set_var("GH_CONFIG_DIR", &gh);
    std::env::remove_var("GH_TOKEN");
    std::env::remove_var("GITHUB_TOKEN");
    std::env::set_var("GIT_CONFIG_COUNT", "1");
    std::env::set_var("GIT_CONFIG_KEY_0", "url.lf-copy-cannot-push:.pushInsteadOf");
    std::env::set_var("GIT_CONFIG_VALUE_0", "");
    Ok(())
}

fn copy_dir(home: &Path, drafts: &[crate::build_info::MigrationDraft]) -> PathBuf {
    let mut digest = Sha256::new();
    for draft in drafts {
        digest.update(draft.name.as_bytes());
        digest.update([0]);
        digest.update(draft.sql.as_bytes());
        digest.update([0]);
    }
    let digest = hex::encode(digest.finalize());
    home.join(COPIES).join(&digest[..16])
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

fn lock(path: &Path) -> StoreResult<std::fs::File> {
    use fs2::FileExt;
    let file = open_lock(path)?;
    file.lock_exclusive().map_err(io)?;
    Ok(file)
}

fn open_lock(path: &Path) -> StoreResult<std::fs::File> {
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)
        .map_err(io)
}

fn io(error: std::io::Error) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::{copy_dir, sanitize, snapshot};
    use crate::build_info::MigrationDraft;
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

    #[test]
    fn editing_a_draft_selects_a_different_copy() {
        let home = std::path::Path::new("/home");
        let before = [MigrationDraft {
            name: "wave_colour",
            sql: "ALTER TABLE waves ADD COLUMN colour TEXT;",
        }];
        let after = [MigrationDraft {
            name: "wave_colour",
            sql: "ALTER TABLE waves ADD COLUMN colour TEXT NOT NULL DEFAULT '';",
        }];
        assert_ne!(copy_dir(home, &before), copy_dir(home, &after));
        assert_eq!(copy_dir(home, &before), copy_dir(home, &before));
    }

    #[test]
    fn a_copy_forgets_shared_process_identity_and_the_linear_token() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE agent_sessions(id TEXT, agent_process_id TEXT);
             CREATE TABLE processes(id TEXT, pid INTEGER, os_started_at INTEGER, endpoint TEXT,
                 attached_lf_process_id TEXT, attachment_token TEXT);
             CREATE TABLE session_activity(id TEXT);
             CREATE TABLE provider_tokens(provider TEXT);
             INSERT INTO agent_sessions VALUES('s','p');
             INSERT INTO processes VALUES('p',42,1,'e','a','t');
             INSERT INTO session_activity VALUES('s');
             INSERT INTO provider_tokens VALUES('linear'),('claude');",
        )
        .unwrap();
        sanitize(&conn).unwrap();
        let pid: Option<i64> = conn
            .query_row("SELECT pid FROM processes", [], |row| row.get(0))
            .unwrap();
        let providers: String = conn
            .query_row(
                "SELECT group_concat(provider) FROM provider_tokens",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(pid, None);
        assert_eq!(providers, "claude");
    }
}
