//! Bounded subprocess reads and retries for read-only GitHub operations.
//! Never wrap provider writes.
use std::io::Read;
use std::process::{Command, Output, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use crate::engine::process::ProcessGroupGuard;
use crate::ops::{OpsError, OpsResult};

pub(super) fn retry_read<T>(
    operation: &str,
    mut read: impl FnMut() -> OpsResult<T>,
) -> OpsResult<T> {
    for attempt in 1..=3 {
        match read() {
            Ok(value) => return Ok(value),
            Err(error) => {
                let retryable = matches!(&error, OpsError::Io(e) if e.kind() == std::io::ErrorKind::TimedOut)
                    || matches!(&error, OpsError::CommandFailed { stderr, .. } if transient(stderr));
                let cause = if retryable {
                    "transient transport failure"
                } else {
                    "read failed"
                };
                tracing::warn!(operation, attempt, cause, "GitHub read failed");
                if !retryable || attempt == 3 {
                    // Transport stderr can contain signed URLs or credentials.
                    return Err(OpsError::Message(format!(
                        "{operation}: {cause} after {attempt} attempt(s)"
                    )));
                }
                thread::sleep(Duration::from_secs(attempt));
            }
        }
    }
    unreachable!("three attempts always return")
}

fn transient(stderr: &str) -> bool {
    let text = stderr.to_ascii_lowercase();
    [
        "read timeout",
        "timeout awaiting response headers",
        "client.timeout exceeded",
        "context deadline exceeded",
        "i/o timeout",
        "operation timed out",
        "connection reset",
        "unexpected eof",
        "http 502",
        "http 503",
        "http 504",
    ]
    .iter()
    .any(|message| text.contains(message))
}

pub(super) fn bounded_output(command: &mut Command, timeout: Duration) -> OpsResult<Output> {
    let (status, stdout, stderr) = read_child(command, timeout, |mut stdout, _| {
        let mut bytes = Vec::new();
        stdout.read_to_end(&mut bytes)?;
        Ok(bytes)
    })?;
    Ok(Output {
        status,
        stdout,
        stderr,
    })
}

/// Read-only child whose deadline restarts at each instant its reader reports.
/// The reader decides what counts as useful work, bounds its own memory and
/// never waits on the channel: only the latest instant matters.
pub(super) fn progress_output<T: Send + 'static>(
    command: &mut Command,
    timeout: Duration,
    read: impl FnOnce(std::process::ChildStdout, mpsc::SyncSender<Instant>) -> OpsResult<T>
        + Send
        + 'static,
) -> OpsResult<T> {
    read_child(command, timeout, read).map(|(_, value, _)| value)
}

fn read_child<T: Send + 'static>(
    command: &mut Command,
    timeout: Duration,
    read: impl FnOnce(std::process::ChildStdout, mpsc::SyncSender<Instant>) -> OpsResult<T>
        + Send
        + 'static,
) -> OpsResult<(std::process::ExitStatus, T, Vec<u8>)> {
    let program = command.get_program().to_string_lossy().into_owned();
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.process_group(0);
    }
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    let group = ProcessGroupGuard::new(child.id());
    let mut deadline = Instant::now() + timeout;
    let stdout = child.stdout.take().expect("stdout was piped");
    let mut stderr = child.stderr.take().expect("stderr was piped");
    let (progress, updates) = mpsc::sync_channel(1);
    let (sender, output) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(read(stdout, progress));
    });
    let (sender, errors) = mpsc::channel();
    thread::spawn(move || {
        let mut bytes = Vec::new();
        let result = stderr.read_to_end(&mut bytes).map(|_| bytes);
        let _ = sender.send(result);
    });
    let result = (|| {
        let mut stdout = None;
        let mut stderr = None;
        let mut status = None;
        loop {
            if let Ok(at) = updates.try_recv() {
                deadline = at + timeout;
            }
            if let Ok(value) = output.try_recv() {
                stdout = Some(value?);
            }
            if let Ok(value) = errors.try_recv() {
                stderr = Some(value?);
            }
            if status.is_none() {
                status = child.try_wait()?;
            }
            if let Some(status) = status {
                if stdout.is_some() && stderr.is_some() {
                    return Ok((
                        status,
                        stdout.take().expect("stdout completed"),
                        stderr.take().expect("stderr completed"),
                    ));
                }
            }
            if Instant::now() >= deadline {
                return Err(OpsError::Io(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("{program} read deadline exceeded"),
                )));
            }
            thread::sleep(Duration::from_millis(10));
        }
    })();
    // Kill the owned group even if its leader has exited but descendants retain pipes.
    group.terminate();
    let cleanup_deadline = Instant::now() + Duration::from_secs(1);
    while child.try_wait()?.is_none() {
        if Instant::now() >= cleanup_deadline {
            // Retain a reaper if the OS cannot settle the killed child promptly.
            thread::spawn(move || {
                let _ = child.wait();
            });
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!("{program} read child cleanup deadline exceeded"),
            )
            .into());
        }
        thread::sleep(Duration::from_millis(10));
    }
    let (status, stdout, stderr) = result?;
    if !status.success() {
        return Err(OpsError::CommandFailed {
            command: program,
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
        });
    }
    Ok((status, stdout, stderr))
}

#[cfg(test)]
mod tests {
    use super::{bounded_output, retry_read, transient};
    use crate::ops::OpsError;
    use std::process::Command;
    use std::time::{Duration, Instant};

    #[test]
    fn retries_only_transient_reads() {
        for failure in [
            "HTTP 502: Bad Gateway",
            "error downloading artifact: error writing zip archive: read tcp: read: operation timed out",
        ] {
            let mut attempts = 0;
            let result = retry_read("checks", || {
                attempts += 1;
                if attempts == 1 {
                    Err(OpsError::CommandFailed {
                        command: "gh".into(),
                        stderr: failure.into(),
                    })
                } else {
                    Ok("same head")
                }
            })
            .unwrap();
            assert_eq!(result, "same head");
            assert_eq!(attempts, 2);
        }
        for text in [
            "HTTP 401",
            "HTTP 403",
            "artifact not found",
            "malformed JSON",
            "unknown failure",
        ] {
            assert!(!transient(text));
        }
    }

    #[test]
    fn exhaustion_is_an_error_and_does_not_expose_urls() {
        let mut attempts = 0;
        let error = retry_read::<()>("download", || {
            attempts += 1;
            Err(OpsError::CommandFailed {
                command: "gh".into(),
                stderr: "read timeout https://secret.example".into(),
            })
        })
        .unwrap_err()
        .to_string();
        assert_eq!(attempts, 3);
        assert!(!error.contains("secret"));
    }

    #[test]
    fn failed_read_names_the_program_without_exposing_arguments() {
        let error = bounded_output(
            Command::new("sh").args(["-c", "exit 1", "private argument"]),
            Duration::from_secs(1),
        )
        .unwrap_err();
        assert!(matches!(error, OpsError::CommandFailed { command, .. } if command == "sh"));
    }

    #[test]
    fn deadline_covers_child_and_inherited_pipes() {
        for script in ["sleep 30", "sleep 30 & exit 0"] {
            let started = Instant::now();
            let error = bounded_output(
                Command::new("sh").args(["-c", script]),
                Duration::from_millis(100),
            )
            .unwrap_err();
            assert!(matches!(error, OpsError::Io(e) if e.kind() == std::io::ErrorKind::TimedOut));
            assert!(started.elapsed() < Duration::from_secs(2));
        }
    }
}
