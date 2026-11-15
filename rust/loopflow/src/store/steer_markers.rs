//! Storage-owned hook notification files.
use std::path::{Path, PathBuf};

/// Touched when a Wave or repository steer is saved; a Task's or a Session's
/// steer touches [`task_marker`] or [`session_marker`] instead, so it starts
/// lf only in the conversations it can reach. A tool-boundary hook skips lf
/// entirely while its provider's seen marker is strictly newer than all three,
/// since no steer arrived after its last check. A managed prompt boundary
/// always checks, which also catches Sessions newly associated with earlier
/// steers.
pub(crate) fn latest_marker(home: &Path) -> PathBuf {
    home.join("steers").join("latest")
}

pub(crate) fn task_marker(home: &Path, task: &str) -> PathBuf {
    home.join("steers").join("task").join(task)
}

pub(crate) fn session_marker(home: &Path, session: &str) -> PathBuf {
    home.join("steers").join("session").join(session)
}

pub(crate) fn seen_marker(home: &Path, process: &loopflow_model::id::AgentProcessId) -> PathBuf {
    home.join("steers").join("seen").join(process.as_str())
}

pub(crate) fn touch(path: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(path.parent().expect("marker has a parent"))?;
    std::fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?
        .set_modified(std::time::SystemTime::now())
}
