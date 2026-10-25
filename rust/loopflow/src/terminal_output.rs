//! A native renderer exclusively owns its original output descriptors. Loopflow
//! and its background helpers write diagnostics elsewhere until it exits.

use std::fs::{File, OpenOptions};
use std::io::{self, IsTerminal, Write};
use std::os::fd::{AsRawFd, BorrowedFd};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result};

#[derive(Debug)]
pub(crate) struct TerminalOutput {
    pub(crate) stdout: File,
    pub(crate) stderr: File,
    log: File,
    path: PathBuf,
    initial_size: u64,
}

impl TerminalOutput {
    /// A waiting launcher must yield output to its foreground child too.
    pub(crate) fn for_child(command: &mut Command) -> Result<Option<Self>> {
        if !io::stdout().is_terminal() && !io::stderr().is_terminal() {
            return Ok(None);
        }
        let output = Self::enter()?;
        output.configure_child(command)?;
        Ok(Some(output))
    }

    pub(crate) fn enter() -> Result<Self> {
        let directory = crate::store::lf_home_dir().join("logs");
        std::fs::create_dir_all(&directory)?;
        let id = crate::journal::current_lf_process_id()
            .map(|id| id.to_string())
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
        let path = directory.join(format!("terminal-{id}.log"));
        let log = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&path)
            .with_context(|| format!("Open Loopflow diagnostics at {}", path.display()))?;
        let initial_size = log.metadata()?.len();
        io::stdout().flush()?;
        io::stderr().flush()?;
        // SAFETY: stdout and stderr are borrowed only to duplicate their live
        // descriptors. The owned copies are close-on-exec and outlive restoration.
        let (stdout, stderr) = unsafe {
            (
                File::from(BorrowedFd::borrow_raw(1).try_clone_to_owned()?),
                File::from(BorrowedFd::borrow_raw(2).try_clone_to_owned()?),
            )
        };
        let output = Self {
            stdout,
            stderr,
            log,
            path,
            initial_size,
        };
        // Drop restores both descriptors even if the second handoff fails.
        redirect(&output.log, 1)?;
        redirect(&output.log, 2)?;
        Ok(output)
    }

    pub(crate) fn configure_child(&self, command: &mut Command) -> io::Result<()> {
        command
            .stdout(self.stdout.try_clone()?)
            .stderr(self.stderr.try_clone()?);
        Ok(())
    }
}

pub(crate) fn redirect(source: &File, destination: i32) -> io::Result<()> {
    // SAFETY: source is live; dup2 atomically replaces the standard descriptor.
    if unsafe { libc::dup2(source.as_raw_fd(), destination) } < 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

impl Drop for TerminalOutput {
    fn drop(&mut self) {
        let _ = io::stdout().flush();
        let _ = io::stderr().flush();
        for (source, destination) in [(&self.stdout, 1), (&self.stderr, 2)] {
            if let Err(error) = redirect(source, destination) {
                let _ = writeln!(&self.stderr, "Cannot restore terminal output: {error}");
            }
        }
        if self
            .log
            .metadata()
            .is_ok_and(|info| info.len() > self.initial_size)
        {
            let _ = writeln!(
                self.stderr,
                "Loopflow recorded diagnostics: {}",
                self.path.display()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;
    use std::process::Command;

    use super::TerminalOutput;

    #[test]
    fn native_output_is_exclusive_and_diagnostics_survive_exit() {
        // Descriptor ownership is process-wide. Exercise it in a disposable
        // process, never redirect another concurrently running test's output.
        if std::env::var_os("TERMINAL_OUTPUT_FIXTURE").is_some() {
            tracing_subscriber::fmt()
                .with_writer(std::io::stderr)
                .without_time()
                .with_ansi(false)
                .init();
            let output = TerminalOutput::enter().unwrap();
            let path = output.path.clone();
            let mut provider = Command::new("/bin/sh");
            provider.args([
                "-c",
                "printf 'native stdout 🦀\\n'; printf 'native stderr\\n' >&2",
            ]);
            output.configure_child(&mut provider).unwrap();
            assert!(provider.status().unwrap().success());
            std::thread::spawn(|| {
                tracing::warn!("background warning during draft");
                tracing::error!("background error during draft");
                writeln!(std::io::stdout(), "direct parent output").unwrap();
                writeln!(std::io::stderr(), "direct parent error").unwrap();
                assert!(Command::new("/bin/sh")
                    .args(["-c", "echo helper failure >&2"])
                    .status()
                    .unwrap()
                    .success());
            })
            .join()
            .unwrap();
            drop(output);
            let log = std::fs::read_to_string(&path).unwrap();
            for message in [
                "background warning",
                "background error",
                "direct parent output",
                "direct parent error",
                "helper failure",
            ] {
                assert!(log.contains(message), "{log}");
            }
            assert!(!log.contains("native stdout") && !log.contains("native stderr"));
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            // Spawn failure must also restore ordinary CLI error delivery.
            let result = (|| -> anyhow::Result<()> {
                let output = TerminalOutput::enter()?;
                let mut missing = Command::new("/nonexistent-loopflow-provider");
                output.configure_child(&mut missing)?;
                missing.spawn()?;
                Ok(())
            })();
            assert!(result.is_err());
            writeln!(std::io::stdout(), "ordinary stdout restored").unwrap();
            writeln!(std::io::stderr(), "ordinary error restored").unwrap();
            return;
        }
        let home = tempfile::tempdir().unwrap();
        let result = Command::new(std::env::current_exe().unwrap())
            .env_clear()
            .env("LF_HOME", home.path())
            .env("TERMINAL_OUTPUT_FIXTURE", "1")
            .args([
                "--exact",
                "terminal_output::tests::native_output_is_exclusive_and_diagnostics_survive_exit",
                "--nocapture",
            ])
            .output()
            .unwrap();
        let stdout = String::from_utf8_lossy(&result.stdout);
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(result.status.success(), "{stdout}\n{stderr}");
        assert!(stdout.contains("native stdout 🦀"), "{stdout}");
        assert!(stderr.contains("native stderr"), "{stderr}");
        assert!(stdout.contains("ordinary stdout restored"));
        assert!(stderr.contains("ordinary error restored"));
        assert!(stderr.contains("Loopflow recorded diagnostics:"));
        assert!(!stdout.contains("direct parent") && !stderr.contains("background warning"));
    }
}
