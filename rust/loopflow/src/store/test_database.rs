//! A private database per fixture, without replaying migrations per test process.

use std::io::Write;
use std::path::Path;

pub(crate) fn copy(path: &Path) -> std::io::Result<()> {
    if path.try_exists()? {
        return Ok(());
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    temporary.write_all(include_bytes!(concat!(env!("OUT_DIR"), "/test-store.db")))?;
    // A copied schema is shared; a Machine identity belongs to one home. No
    // product rows refer to the local Machine in this empty database.
    rusqlite::Connection::open(temporary.path())
        .and_then(|conn| {
            conn.execute(
                "UPDATE machines SET id='home_' || lower(hex(randomblob(16))),
                 created_at=unixepoch(),observed_at=unixepoch() WHERE is_local=1",
                [],
            )
        })
        .map_err(std::io::Error::other)?;
    // Concurrent first opens must see the same complete database. Never replace
    // a store another opener has already started writing.
    match temporary.persist_noclobber(path) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => Ok(()),
        Err(error) => Err(error.error),
    }
}
