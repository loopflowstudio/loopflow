use anyhow::{Context, Result};
use loopflow_model::id::LfProcessId;

pub fn graph_log_path(id: &str) -> std::path::PathBuf {
    crate::store::lf_home_dir()
        .join("logs")
        .join(format!("graph-{id}.log"))
}

/// A run's output once its launcher lets go: an attempt's until it registers,
/// when it becomes that Graph process's log, or a ship's for its whole run.
#[derive(Debug)]
pub struct PendingLog {
    pub file: std::fs::File,
    pub path: std::path::PathBuf,
}

impl PendingLog {
    pub fn open(name: &str) -> Result<Self> {
        use std::os::unix::fs::OpenOptionsExt;
        let path = graph_log_path(name);
        std::fs::create_dir_all(path.parent().expect("graph logs live in a directory"))?;
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .read(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
            .with_context(|| format!("could not open the Graph log {}", path.display()))?;
        Ok(Self { file, path })
    }

    /// Name the log for the registered Process; open descriptors follow the rename.
    pub fn settle(&mut self, registered: &LfProcessId) -> Result<()> {
        let path = graph_log_path(registered.as_str());
        std::fs::rename(&self.path, &path)
            .with_context(|| format!("could not name the Graph log {}", path.display()))?;
        self.path = path;
        Ok(())
    }

    /// An attempt that ended before registering tells its launcher why.
    pub fn replay(mut self) {
        use std::io::{Read, Seek, Write};
        let mut bytes = Vec::new();
        if self.file.rewind().is_ok() && self.file.read_to_end(&mut bytes).is_ok() {
            let _ = std::io::stderr().write_all(&bytes);
        }
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Hand the launcher its last lines, keep this run's own output in the log
/// from here on, and let the launcher go.
pub fn release_launcher(log: &std::fs::File) -> Result<()> {
    use std::io::Write;
    std::io::stdout().flush()?;
    std::io::stderr().flush()?;
    if crate::transport::current().is_some() {
        crate::transport::detach_viewers()?;
    }
    crate::terminal_output::redirect(log, 1)?;
    crate::terminal_output::redirect(log, 2)?;
    crate::os_process::detached::release();
    Ok(())
}
