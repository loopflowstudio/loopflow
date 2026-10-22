//! Bounded retries for read-only GitHub operations. Never wrap provider writes.
use std::process::{Command, Output};
use std::thread;
use std::time::Duration;

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
    let output = crate::os_process::bounded_output(command, timeout)?;
    if !output.status.success() {
        return Err(OpsError::CommandFailed {
            command: "GitHub read".into(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        });
    }
    Ok(output)
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
