use std::ffi::OsStr;
use std::fs::{self, File, OpenOptions};
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

use crate::ops::{OpsError, OpsResult};

const LOCK_FD_ENV: &str = "LF_RELEASE_LOCK_FD";

#[derive(Debug)]
pub(crate) struct ReleaseLock {
    file: File,
    inherited: bool,
}

impl ReleaseLock {
    pub(crate) fn acquire(repo: &Path, target: &str) -> OpsResult<Self> {
        let dir = repo.join(".lf/locks");
        fs::create_dir_all(&dir)?;
        let path = dir.join(format!(
            "release-{}.lock",
            hex::encode(Sha256::digest(target))
        ));
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)?;
        // A publisher child can reuse the exact inherited open file description.
        // An arbitrary fd/environment value never skips the OS lock acquisition.
        let inherited = std::env::var(LOCK_FD_ENV)
            .ok()
            .and_then(|v| v.parse::<i32>().ok())
            .filter(|fd| *fd >= 3);
        let mut reused = false;
        let file = if let Some(fd) = inherited {
            // SAFETY: dup validates fd and transfers a new owned descriptor on success.
            let duplicate = unsafe { libc::dup(fd) };
            if duplicate >= 0 {
                // SAFETY: duplicate is a freshly owned descriptor from dup.
                let inherited = unsafe { File::from_raw_fd(duplicate) };
                let actual = inherited.metadata()?;
                let expected = file.metadata()?;
                if actual.dev() == expected.dev() && actual.ino() == expected.ino() {
                    reused = true;
                    inherited
                } else {
                    file
                }
            } else {
                file
            }
        } else {
            file
        };
        fs2::FileExt::try_lock_exclusive(&file).map_err(|error| {
            if error.kind() == std::io::ErrorKind::WouldBlock {
                OpsError::ReleaseDeferred {
                    reason: format!("release target {target} already has an active execution"),
                    continuation:
                        "retry at the next configured firing after the active execution finishes"
                            .into(),
                }
            } else {
                error.into()
            }
        })?;
        Ok(Self {
            file,
            inherited: reused,
        })
    }

    pub(crate) fn is_inherited(&self) -> bool {
        self.inherited
    }

    /// The child retains the target exclusion even if its controller dies.
    pub(crate) fn command(&self, program: impl AsRef<OsStr>) -> Command {
        let mut command = Command::new(program);
        self.inherit(&mut command);
        command
    }

    pub(crate) fn inherit(&self, command: &mut Command) {
        let fd = self.file.as_raw_fd();
        command.env(LOCK_FD_ENV, fd.to_string());
        // SAFETY: the caller retains this lock until the child exits; fcntl is
        // async-signal-safe and only makes this owned lock descriptor inheritable.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(fd, libc::F_SETFD, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ReleaseLock;
    use std::io::Write;
    use std::process::{Command, Stdio};

    #[test]
    fn surviving_child_keeps_target_exclusive_after_parent_drops_lock() {
        let repo = tempfile::tempdir().unwrap();
        let owner = ReleaseLock::acquire(repo.path(), "default").unwrap();
        let mut command = Command::new("sh");
        command.args(["-c", "read -r finish"]).stdin(Stdio::piped());
        owner.inherit(&mut command);
        let mut child = command.spawn().unwrap();
        drop(owner);
        assert!(ReleaseLock::acquire(repo.path(), "default").is_err());
        assert!(ReleaseLock::acquire(repo.path(), "independent").is_ok());
        child.stdin.take().unwrap().write_all(b"finish\n").unwrap();
        assert!(child.wait().unwrap().success());
        assert!(ReleaseLock::acquire(repo.path(), "default").is_ok());
    }
}
