//! Setup workers have no checkout admission, Git lease, or removal capability.
//! Reads may be killed. Scheduling writes are atomic retry hints, not authority:
//! interruption may commit a hint/receipt or leave a temporary file, never a
//! partially published record. The owner must receive success before proceeding.
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::ops::cron::{self, CronReceipt};
use crate::ops::OpsResult;

const WORKER_FLAG: &str = "--cleanup-io-worker";
const RESPONSE: &str = "loopflow-cleanup-io:";
const TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Read {
    Registrations(PathBuf),
    Attempt {
        admin: PathBuf,
        primary: Option<PathBuf>,
    },
    Receipts {
        root: PathBuf,
        flow: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Schedule {
    Attempt {
        marker: PathBuf,
        at: i64,
    },
    Receipt {
        root: PathBuf,
        receipt: Box<CronReceipt>,
    },
    Prune {
        root: PathBuf,
        flow: String,
        current: crate::durable::CronReceiptId,
    },
}

#[derive(Debug, Serialize, Deserialize)]
enum Request {
    Read(Read),
    Schedule(Schedule),
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Attempt {
    pub path: PathBuf,
    pub marker: PathBuf,
    /// Missing hints are initialized by the owner, never by a read worker.
    pub at: Option<i64>,
}

pub(crate) fn read<T: DeserializeOwned>(request: Read) -> OpsResult<T> {
    request_worker(Request::Read(request))
}

pub(crate) fn schedule(request: Schedule) -> OpsResult<()> {
    request_worker(Request::Schedule(request))
}

fn request_worker<T: DeserializeOwned>(request: Request) -> OpsResult<T> {
    let mut command = Command::new(std::env::current_exe()?);
    exclude_inherited_descriptors(&mut command)?;
    let request = serde_json::to_string(&request).map_err(super::error)?;
    #[cfg(not(test))]
    command.arg(WORKER_FLAG).arg(request);
    #[cfg(test)]
    command
        .args([
            "--exact",
            "ops::wt::cleanup::io::tests::worker",
            "--ignored",
            "--nocapture",
        ])
        .env("LF_TEST_CLEANUP_IO", request);
    let output = crate::ops::read_retry::bounded_output(&mut command, TIMEOUT)?;
    let output = std::str::from_utf8(&output.stdout).map_err(super::error)?;
    let response = output
        .lines()
        .find_map(|line| line.strip_prefix(RESPONSE))
        .ok_or_else(|| super::error("cleanup I/O worker returned no response"))?;
    let value: Result<serde_json::Value, String> =
        serde_json::from_str(response).map_err(super::error)?;
    serde_json::from_value(value.map_err(super::error)?).map_err(super::error)
}

// Outer Flow/release/Git commands may deliberately inherit exclusion fds.
// Mark every existing descriptor close-on-exec in this child, not merely the
// descriptors created by cleanup (Rust already creates those close-on-exec).
fn exclude_inherited_descriptors(command: &mut Command) -> OpsResult<()> {
    use std::os::unix::process::CommandExt;
    let descriptors: Vec<i32> = std::fs::read_dir("/dev/fd")?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<std::io::Result<Vec<_>>>()?
        .into_iter()
        .filter_map(|name| name.to_str()?.parse().ok())
        .filter(|fd| *fd > 2)
        .collect();
    // SAFETY: fcntl is async-signal-safe. The child changes only descriptor
    // inheritance; it neither closes the exec-error pipe nor touches parent fds.
    // Descriptors may have closed since enumeration, so EBADF is harmless.
    unsafe {
        command.pre_exec(move || {
            for fd in &descriptors {
                if libc::fcntl(*fd, libc::F_SETFD, libc::FD_CLOEXEC) == -1 {
                    let error = std::io::Error::last_os_error();
                    if error.raw_os_error() != Some(libc::EBADF) {
                        return Err(error);
                    }
                }
            }
            Ok(())
        });
    }
    Ok(())
}

/// Internal executable entry point, before installation/journal/store admission.
/// No request can execute cleanup, acquire checkout locks, or remove source.
#[doc(hidden)]
pub fn worker_entry() -> Option<ExitCode> {
    let mut args = std::env::args_os().skip(1);
    if args.next().as_deref() != Some(std::ffi::OsStr::new(WORKER_FLAG)) {
        return None;
    }
    let result = args
        .next()
        .ok_or_else(|| super::error("missing cleanup I/O request"))
        .and_then(|value| serde_json::from_slice(value.as_encoded_bytes()).map_err(super::error))
        .and_then(execute);
    respond(result);
    Some(ExitCode::SUCCESS)
}

fn respond(result: OpsResult<serde_json::Value>) {
    println!(
        "{RESPONSE}{}",
        serde_json::to_string(&result.map_err(|error| error.to_string()))
            .expect("worker response is JSON")
    );
}

fn execute(request: Request) -> OpsResult<serde_json::Value> {
    match request {
        Request::Read(Read::Registrations(common)) => {
            let mut admins = Vec::new();
            match std::fs::read_dir(common.join("worktrees")) {
                Ok(entries) => {
                    for entry in entries {
                        // Do not stat entries here: a stalled registration is
                        // observed separately, not in the repository-wide read.
                        match entry {
                            Ok(entry) => admins.push(entry.path()),
                            Err(error) => {
                                tracing::warn!(%error, "cleanup registration entry unavailable")
                            }
                        }
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            serde_json::to_value(admins).map_err(super::error)
        }
        Request::Read(Read::Attempt { admin, primary }) => {
            let path = match primary {
                Some(path) => path,
                None => {
                    let gitdir = std::fs::read_to_string(admin.join("gitdir"))?;
                    Path::new(gitdir.trim_end_matches('\n'))
                        .parent()
                        .ok_or_else(|| super::error("registration has no checkout path"))?
                        .to_path_buf()
                }
            };
            let path = crate::store::canonicalize_with_missing_tail(&path).map_err(super::error)?;
            let marker = admin.join("lf-cleanup-attempt");
            let at = match std::fs::read_to_string(&marker) {
                Ok(value) => Some(
                    value
                        .parse::<i64>()
                        .ok()
                        .filter(|at| *at <= chrono::Utc::now().timestamp_micros())
                        .unwrap_or(0),
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                // An unreadable hint is oldest priority, never ownership.
                Err(_) => Some(0),
            };
            serde_json::to_value(Attempt { path, marker, at }).map_err(super::error)
        }
        Request::Read(Read::Receipts { root, flow }) => {
            serde_json::to_value(cron::read_receipts(&root, "", Some(&flow))?).map_err(super::error)
        }
        Request::Schedule(Schedule::Attempt { marker, at }) => {
            // A killed worker cannot share a temporary inode with its successor.
            let temporary = marker.with_extension(format!("{}.tmp", std::process::id()));
            let result = (|| {
                cron::write_private_file(&temporary, at.to_string().as_bytes())?;
                std::fs::rename(&temporary, marker)?;
                Ok(serde_json::Value::Null)
            })();
            if result.is_err() {
                let _ = std::fs::remove_file(&temporary);
            }
            result
        }
        Request::Schedule(Schedule::Receipt { root, receipt }) => {
            cron::write_receipt(&root, &receipt)?;
            Ok(serde_json::Value::Null)
        }
        Request::Schedule(Schedule::Prune {
            root,
            flow,
            current,
        }) => {
            cron::prune_minute_receipts(&root, "", &flow, &current)?;
            Ok(serde_json::Value::Null)
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn cleanup_setup_workers_do_not_inherit_locks() {
        use std::os::fd::AsRawFd;
        use std::os::unix::fs::OpenOptionsExt;
        let _guard = crate::journal::TestLedgerGuard::new();
        let directory = tempfile::tempdir().unwrap();
        let lock_path = directory.path().join("outer-lock");
        let lock = std::fs::File::create(&lock_path).unwrap();
        fs2::FileExt::try_lock_exclusive(&lock).unwrap();
        // SAFETY: this owned file stays alive until the child has entered its read.
        assert_eq!(
            unsafe { libc::fcntl(lock.as_raw_fd(), libc::F_SETFD, 0) },
            0
        );
        let marker = directory.path().join("lf-cleanup-attempt");
        assert!(std::process::Command::new("mkfifo")
            .arg(&marker)
            .status()
            .unwrap()
            .success());
        let admin = directory.path().to_path_buf();
        let worker = std::thread::spawn(move || {
            super::read::<super::Attempt>(super::Read::Attempt {
                primary: Some(admin.clone()),
                admin,
            })
        });
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(1);
        let writer = loop {
            match std::fs::OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(&marker)
            {
                Ok(writer) => break writer,
                Err(error)
                    if error.raw_os_error() == Some(libc::ENXIO)
                        && std::time::Instant::now() < deadline =>
                {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => panic!("worker did not reach its read: {error}"),
            }
        };
        drop(lock);
        let next = std::fs::File::open(&lock_path).unwrap();
        fs2::FileExt::try_lock_exclusive(&next).expect("blocked worker must not inherit exclusion");
        assert!(worker.join().unwrap().is_err());
        drop(writer);
    }

    #[test]
    #[ignore = "subprocess entry point for the real cleanup I/O protocol"]
    fn worker() {
        let request = std::env::var("LF_TEST_CLEANUP_IO").expect("worker request");
        let request: super::Request = serde_json::from_str(&request).unwrap();
        if let super::Request::Schedule(super::Schedule::Attempt { marker, .. }) = &request {
            if std::env::var_os("LF_TEST_CLEANUP_STALL_HINT").as_deref() == Some(marker.as_os_str())
            {
                let temporary = marker.with_extension(format!("{}.tmp", std::process::id()));
                assert!(std::process::Command::new("mkfifo")
                    .arg(temporary)
                    .status()
                    .unwrap()
                    .success());
            }
        }
        let receipt = matches!(
            &request,
            super::Request::Schedule(super::Schedule::Receipt { .. })
        );
        let result = super::execute(request);
        if receipt && result.is_ok() {
            if let Some(fifo) = std::env::var_os("LF_TEST_CLEANUP_STALL_RECEIPT") {
                // Interrupt after atomic publication but before acknowledgment.
                let _ = std::fs::read(fifo);
            }
        }
        super::respond(result);
    }
}
