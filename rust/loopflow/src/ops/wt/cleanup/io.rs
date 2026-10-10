//! Workers never acquire checkout admission, Git leases, or removal authority.
//! Reads may be killed. Scheduling hints and history-projection pages retain
//! their owners' atomic publication. File openers transfer unlocked descriptors
//! and exit before parent admission; canceled workers cannot continue to deletion.
mod descriptor;

use std::os::fd::AsRawFd;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};
use std::time::Duration;

use serde::{de::DeserializeOwned, Deserialize, Serialize};

use crate::ops::cron::{self, CronReceipt};
use crate::ops::OpsResult;

const WORKER_FLAG: &str = "--cleanup-io-worker";
const RESPONSE: &str = "loopflow-cleanup-io:";
static WORKER: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

const TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Read {
    Normalize(PathBuf),
    Admission {
        database: PathBuf,
        path: PathBuf,
    },
    Lease {
        repo: PathBuf,
        path: PathBuf,
    },
    RunningPaths,
    ReleaseRegistry,
    ReceiptContext(PathBuf),
    Observation {
        database: PathBuf,
        repo: PathBuf,
        decision: Box<super::CleanupDecision>,
        external: Result<std::collections::HashSet<PathBuf>, String>,
        validate_registration: bool,
    },
    Registrations(PathBuf),
    Checkouts(PathBuf),
    Settled {
        database: PathBuf,
        path: PathBuf,
    },
    Attempt(PathBuf),
    Receipts {
        root: PathBuf,
        flow: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Schedule {
    ProjectEvidence(PathBuf),
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
    OpenLock { path: PathBuf, socket: i32 },
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Attempt {
    pub path: PathBuf,
    pub branch: Option<String>,
    /// Missing hints are initialized by the owner, never by a read worker.
    pub at: Option<i64>,
}

pub(crate) fn read<T: DeserializeOwned>(request: Read) -> OpsResult<T> {
    request_worker(Request::Read(request))
}

pub(crate) fn open_lock(path: PathBuf) -> OpsResult<std::fs::File> {
    let (receiver, sender) = std::os::unix::net::UnixDatagram::pair()?;
    request_worker::<()>(Request::OpenLock {
        path,
        socket: sender.as_raw_fd(),
    })?;
    // bounded_output has reaped the sender. Only this parent may acquire locks.
    drop(sender);
    descriptor::receive(&receiver).map_err(Into::into)
}

pub(crate) fn schedule(request: Schedule) -> OpsResult<()> {
    request_worker(Request::Schedule(request))
}

pub(super) fn output(command: &mut Command, timeout: Duration) -> OpsResult<std::process::Output> {
    if !WORKER.load(std::sync::atomic::Ordering::Relaxed) {
        return crate::ops::read_retry::bounded_output(command, timeout);
    }
    // A setup worker's descendants must stay in its cancellable process group.
    let output = command.output()?;
    if !output.status.success() {
        return Err(super::error("cleanup observation command failed"));
    }
    Ok(output)
}

fn request_worker<T: DeserializeOwned>(request: Request) -> OpsResult<T> {
    let mut command = Command::new(std::env::current_exe()?);
    exclude_inherited_descriptors(&mut command)?;
    if let Request::OpenLock { socket, .. } = &request {
        use std::os::unix::process::CommandExt;
        let socket = *socket;
        // SAFETY: this socket remains owned by open_lock until the child exits.
        // Only the transfer socket, never checkout locks, survives exec.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(socket, libc::F_SETFD, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    let encoded = serde_json::to_string(&request).map_err(super::error)?;
    #[cfg(not(test))]
    command.arg(WORKER_FLAG).arg(encoded);
    #[cfg(test)]
    command
        .args([
            "--exact",
            "ops::wt::cleanup::io::tests::worker",
            "--ignored",
            "--nocapture",
        ])
        .env("LF_TEST_CLEANUP_IO", encoded);
    let timeout = if matches!(&request, Request::Read(Read::RunningPaths)) {
        Duration::from_secs(5)
    } else {
        TIMEOUT
    };
    let output = crate::ops::read_retry::bounded_output(&mut command, timeout)?;
    let output = std::str::from_utf8(&output.stdout).map_err(super::error)?;
    let response = output
        .lines()
        .find_map(|line| line.strip_prefix(RESPONSE))
        .ok_or_else(|| super::error("cleanup I/O worker returned no response"))?;
    let value: Result<T, String> = serde_json::from_str(response).map_err(super::error)?;
    value.map_err(super::error)
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
    WORKER.store(true, std::sync::atomic::Ordering::Relaxed);
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

pub(super) fn checkout_path(admin: &Path) -> OpsResult<PathBuf> {
    let gitdir = std::fs::read_to_string(admin.join("gitdir"))?;
    let path = Path::new(gitdir.trim_end_matches('\n'))
        .parent()
        .ok_or_else(|| super::error("registration has no checkout path"))?;
    crate::store::canonicalize_with_missing_tail(path).map_err(super::error)
}

fn execute(request: Request) -> OpsResult<serde_json::Value> {
    match request {
        Request::OpenLock { path, socket } => {
            let file = crate::store::sqlite::SqliteStore::open_checkout_lock(&path)
                .map_err(super::error)?;
            descriptor::send(socket, &file)?;
            Ok(serde_json::Value::Null)
        }
        Request::Read(Read::Admission { database, path }) => {
            let store = crate::store::sqlite::SqliteStore::open_read_only(&database)
                .map_err(super::error)?;
            serde_json::to_value(
                store
                    .checkout_lock_paths(&[&path], &[])
                    .map_err(super::error)?,
            )
            .map_err(super::error)
        }
        Request::Read(Read::Lease { repo, path }) => serde_json::to_value(
            crate::engine::git::PreparedWorktreeLease::discover(&repo, &path)?,
        )
        .map_err(super::error),
        Request::Read(Read::Normalize(path)) => {
            serde_json::to_value(crate::store::canonicalize_with_missing_tail(&path)?)
                .map_err(super::error)
        }
        Request::Read(Read::ReceiptContext(database)) => {
            let store = crate::store::sqlite::SqliteStore::open_read_only(&database)
                .map_err(super::error)?;
            let machine = store.local_machine().map_err(super::error)?;
            let home = store.home_dir().map_err(super::error)?;
            serde_json::to_value((machine.id, home)).map_err(super::error)
        }
        Request::Read(Read::ReleaseRegistry) => {
            let path = super::release_registry()?
                .map(|store| store.path())
                .transpose()
                .map_err(super::error)?;
            serde_json::to_value(path).map_err(super::error)
        }
        Request::Read(Read::RunningPaths) => {
            serde_json::to_value(super::read_running_paths()?).map_err(super::error)
        }
        Request::Read(Read::Observation {
            database,
            repo,
            mut decision,
            external,
            validate_registration,
        }) => {
            let store = crate::store::sqlite::SqliteStore::open_read_only(&database)
                .map_err(super::error)?;
            super::observe_source(
                &store,
                &repo,
                &mut decision,
                &external.map_err(super::error),
                validate_registration,
            )?;
            serde_json::to_value(decision).map_err(super::error)
        }
        Request::Read(Read::Checkouts(repo)) => {
            // Keep Git and path resolution in the same cancellable process
            // group. A nested bounded_output would create an escaping group.
            let output = super::read_git(&repo, &["worktree", "list", "--porcelain", "-z"])?;
            let registered = crate::engine::worktrees::parse_porcelain(&output)
                .into_iter()
                .map(|(path, branch)| {
                    crate::store::canonicalize_with_missing_tail(&path)
                        .map(|path| (path, branch))
                        .map_err(super::error)
                })
                .collect::<OpsResult<Vec<_>>>()?;
            serde_json::to_value(registered).map_err(super::error)
        }
        Request::Read(Read::Settled { database, path }) => {
            let store = crate::store::sqlite::SqliteStore::open_read_only(&database)
                .map_err(super::error)?;
            serde_json::to_value(store.has_settled_checkout(&path).map_err(super::error)?)
                .map_err(super::error)
        }
        Request::Read(Read::Registrations(common)) => {
            let mut admins = Vec::new();
            match std::fs::read_dir(common.join("worktrees")) {
                Ok(entries) => {
                    for entry in entries {
                        // Do not stat entries here: a stalled registration is
                        // observed separately, not in the repository-wide read.
                        // An unknown name cannot be consumed by the cursor.
                        // Retry discovery rather than certify a partial sweep.
                        admins.push(entry?.path());
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
            serde_json::to_value(admins).map_err(super::error)
        }
        Request::Read(Read::Attempt(admin)) => {
            let path = checkout_path(&admin)?;
            let head = std::fs::read_to_string(admin.join("HEAD"))?;
            let branch = head
                .trim()
                .strip_prefix("ref: refs/heads/")
                .map(str::to_string);
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
            serde_json::to_value(Attempt { path, branch, at }).map_err(super::error)
        }
        Request::Read(Read::Receipts { root, flow }) => {
            serde_json::to_value(cron::read_receipts(&root, "", Some(&flow))?).map_err(super::error)
        }
        Request::Schedule(Schedule::ProjectEvidence(database)) => {
            // Existing compatible store only: no initialization or migrations.
            // The history owner commits the page and cursor in one transaction.
            let store = crate::store::sqlite::SqliteStore::open_existing_processes(&database)
                .map_err(super::error)?;
            store.advance_session_evidence().map_err(super::error)?;
            Ok(serde_json::Value::Null)
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
        std::fs::write(directory.path().join("HEAD"), "ref: refs/heads/fixture\n").unwrap();
        std::fs::write(
            directory.path().join("gitdir"),
            directory.path().join(".git").to_str().unwrap(),
        )
        .unwrap();
        assert!(std::process::Command::new("mkfifo")
            .arg(&marker)
            .status()
            .unwrap()
            .success());
        let admin = directory.path().to_path_buf();
        let worker =
            std::thread::spawn(move || super::read::<super::Attempt>(super::Read::Attempt(admin)));
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
        super::WORKER.store(true, std::sync::atomic::Ordering::Relaxed);
        let request = std::env::var("LF_TEST_CLEANUP_IO").expect("worker request");
        let request: super::Request = serde_json::from_str(&request).unwrap();
        if let super::Request::Schedule(super::Schedule::Attempt { marker, .. }) = &request {
            if std::env::var_os("LF_TEST_CLEANUP_STALL_HINT")
                .is_some_and(|paths| std::env::split_paths(&paths).any(|path| path == *marker))
            {
                let temporary = marker.with_extension(format!("{}.tmp", std::process::id()));
                assert!(std::process::Command::new("mkfifo")
                    .arg(temporary)
                    .status()
                    .unwrap()
                    .success());
            }
        }
        if let super::Request::Read(super::Read::Settled { path, .. }) = &request {
            if std::env::var_os("LF_TEST_CLEANUP_STALL_SETTLED")
                .is_some_and(|value| std::path::Path::new(&value) == path)
            {
                let _ =
                    std::fs::read(std::env::var_os("LF_TEST_CLEANUP_STALL_SETTLED_FIFO").unwrap());
            }
        }
        let stalled_lock = match &request {
            super::Request::OpenLock { path, .. } => std::env::var_os("LF_TEST_CLEANUP_STALL_LOCK")
                .is_some_and(|value| std::path::Path::new(&value) == path),
            _ => false,
        };
        let stall_lock = |phase| {
            if stalled_lock
                && std::env::var("LF_TEST_CLEANUP_STALL_LOCK_PHASE").as_deref() == Ok(phase)
            {
                let _ = std::fs::read(std::env::var_os("LF_TEST_CLEANUP_STALL_LOCK_FIFO").unwrap());
            }
        };
        stall_lock("before");
        let receipt = matches!(
            &request,
            super::Request::Schedule(super::Schedule::Receipt { .. })
        );
        let result = super::execute(request);
        stall_lock("after");
        if receipt && result.is_ok() {
            if let Some(fifo) = std::env::var_os("LF_TEST_CLEANUP_STALL_RECEIPT") {
                // Interrupt after atomic publication but before acknowledgment.
                let _ = std::fs::read(fifo);
            }
        }
        super::respond(result);
    }
}
