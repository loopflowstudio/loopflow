use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use uuid::Uuid;

use crate::engine::worktrees::main_repo_root;
use crate::lf::commands::util::find_repo_root;
use crate::lfd::executor::helpers::resolve_lf_binary;
use crate::ops::util::resolve_wave_name;

/// Dropping this file into `wave/<wave>/` stops the loop after the current pass.
const STOP_FILE: &str = "STOP";

/// Cooldown after a failed pass so a broken inner run can't hot-spin the loop.
/// A successful pass repeats immediately — loopflow owns the cadence, gated only
/// on the inner pass finishing.
const FAILURE_COOLDOWN: Duration = Duration::from_secs(3);

/// Run the progress loop for a wave.
///
/// loopflow owns the *outer* loop: each pass is a single bounded
/// `lf -b goal <wave> --once`, and the loop fires the next pass as soon as the
/// previous one finishes. This is the deterministic controller that replaces
/// relying on the model's own goal loop (which gets stuck). It repeats until
/// interrupted (Ctrl-C) or until `wave/<wave>/STOP` appears.
pub fn run(name: &str) -> Result<()> {
    let repo_root = find_repo_root()?;
    let main_repo = main_repo_root(&repo_root).unwrap_or(repo_root);
    let wave_name = resolve_wave_name(&main_repo, Some(name))
        .ok_or_else(|| anyhow!("invalid wave name: '{name}'"))?;

    let wave_dir = main_repo.join("wave").join(&wave_name);
    let stop_file = wave_dir.join(STOP_FILE);
    let streams_dir = wave_dir.join("streams");
    let lf = resolve_lf_binary();

    println!("lf wave · {wave_name} · loopflow owns the outer loop (Ctrl-C to stop)");

    let mut pass: u32 = 0;
    loop {
        if stop_file.exists() {
            println!(
                "lf wave · {wave_name} · stopping: stop file present ({}) ({pass} passes)",
                stop_file.display()
            );
            return Ok(());
        }

        pass += 1;
        println!("-- lf wave · {wave_name} · pass {pass} --");

        match run_pass(&lf, &main_repo, &wave_name, pass, &streams_dir)? {
            PassOutcome::Ok { log_path } => {
                println!(
                    "lf wave · pass {pass} stream captured at {}",
                    log_path.display()
                );
            }
            PassOutcome::Failed(status) => {
                eprintln!(
                    "lf wave · pass {pass} exited with {status}; cooling down {}s",
                    FAILURE_COOLDOWN.as_secs()
                );
                std::thread::sleep(FAILURE_COOLDOWN);
            }
            PassOutcome::Signaled => {
                // Ctrl-C (or another signal) killed the inner pass — treat it as
                // the operator stopping the loop, not a pass to retry.
                println!("lf wave · {wave_name} · interrupted ({pass} passes)");
                return Ok(());
            }
        }
    }
}

/// The outcome of a pass that actually ran. Setup failures (spawn, pipes, log
/// I/O) propagate as `Err` from `run_pass`, not as a variant here.
enum PassOutcome {
    Ok { log_path: PathBuf },
    Failed(ExitStatus),
    Signaled,
}

/// Run one bounded pass: `lf -b goal <wave> --once`, teeing stdout/stderr to
/// the terminal and to a durable stream log for later monitor/chat summarization.
fn run_pass(
    lf: &Path,
    repo: &Path,
    wave: &str,
    pass: u32,
    streams_dir: &Path,
) -> Result<PassOutcome> {
    let log_path = stream_log_path(streams_dir, pass);
    let log_file = Arc::new(Mutex::new(create_stream_log(&log_path)?));

    let mut child = Command::new(lf)
        .arg("-b")
        .arg("goal")
        .arg(wave)
        .arg("--once")
        .current_dir(repo)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("failed to spawn `lf -b goal {wave} --once`"))?;

    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("missing stdout pipe for `lf -b goal {wave} --once`"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| anyhow!("missing stderr pipe for `lf -b goal {wave} --once`"))?;

    let stdout_thread = tee_stream(stdout, io::stdout(), Arc::clone(&log_file));
    let stderr_thread = tee_stream(stderr, io::stderr(), Arc::clone(&log_file));

    let status = child.wait();
    join_stream_thread(stdout_thread, "stdout")?;
    join_stream_thread(stderr_thread, "stderr")?;
    let status =
        status.with_context(|| format!("failed to wait for `lf -b goal {wave} --once`"))?;

    Ok(if status.success() {
        PassOutcome::Ok { log_path }
    } else if status.code().is_none() {
        PassOutcome::Signaled
    } else {
        PassOutcome::Failed(status)
    })
}

fn stream_log_path(streams_dir: &Path, pass: u32) -> PathBuf {
    streams_dir.join(format!("pass-{pass}-{}.log", Uuid::new_v4()))
}

fn create_stream_log(path: &Path) -> Result<File> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow!("stream log path has no parent: {}", path.display()))?;
    fs::create_dir_all(parent)
        .with_context(|| format!("failed to create stream log directory {}", parent.display()))?;
    File::create(path).with_context(|| format!("failed to create stream log {}", path.display()))
}

fn tee_stream<R, W>(
    mut reader: R,
    mut writer: W,
    log_file: Arc<Mutex<File>>,
) -> JoinHandle<Result<()>>
where
    R: Read + Send + 'static,
    W: Write + Send + 'static,
{
    thread::spawn(move || {
        let mut buffer = [0_u8; 8192];
        loop {
            let bytes = reader.read(&mut buffer)?;
            if bytes == 0 {
                writer.flush()?;
                return Ok(());
            }

            writer.write_all(&buffer[..bytes])?;
            writer.flush()?;

            let mut file = log_file
                .lock()
                .map_err(|_| anyhow!("stream log writer lock was poisoned"))?;
            file.write_all(&buffer[..bytes])?;
            file.flush()?;
        }
    })
}

fn join_stream_thread(thread: JoinHandle<Result<()>>, name: &str) -> Result<()> {
    thread
        .join()
        .map_err(|_| anyhow!("{name} stream thread panicked"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_failure_propagates_as_error() {
        // A missing inner binary is a setup failure, not a pass outcome — it
        // surfaces as `Err`, bubbling out of the loop rather than cooling down.
        let missing = Path::new("/definitely/not/a/real/lf-binary");
        let streams_dir = tempfile::tempdir().unwrap();
        let result = run_pass(missing, Path::new("/tmp"), "ghost", 1, streams_dir.path());
        assert!(result.is_err());
    }

    #[test]
    fn stream_log_path_uses_pass_number_under_streams_dir() {
        let path = stream_log_path(Path::new("/tmp/streams"), 42);
        assert_eq!(path.parent(), Some(Path::new("/tmp/streams")));
        assert!(path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("pass-42-") && name.ends_with(".log")));
    }
}
