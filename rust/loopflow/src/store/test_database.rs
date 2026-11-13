//! A private database per fixture, without replaying migrations per test process.

use std::path::Path;

pub(crate) fn copy(path: &Path) -> std::io::Result<()> {
    if std::fs::metadata(path).is_ok_and(|metadata| metadata.len() > 0) {
        return Ok(());
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(
        path,
        include_bytes!(concat!(env!("OUT_DIR"), "/test-store.db")),
    )?;
    // A copied schema is shared; a Machine identity belongs to one home. No
    // product rows refer to the local Machine in this empty database.
    rusqlite::Connection::open(path)
        .and_then(|conn| {
            conn.execute(
                "UPDATE machines SET id='home_' || lower(hex(randomblob(16))),
                 created_at=unixepoch(),observed_at=unixepoch() WHERE is_local=1",
                [],
            )
        })
        .map_err(std::io::Error::other)?;
    Ok(())
}
