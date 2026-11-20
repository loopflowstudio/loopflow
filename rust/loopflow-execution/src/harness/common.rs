use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::task::JoinHandle;

#[derive(Debug)]
pub(super) struct TurnInProgressGuard {
    flag: Arc<AtomicBool>,
    armed: bool,
}

impl TurnInProgressGuard {
    pub fn new(flag: Arc<AtomicBool>) -> Self {
        Self { flag, armed: true }
    }

    pub fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for TurnInProgressGuard {
    fn drop(&mut self) {
        if self.armed {
            self.flag.store(false, Ordering::SeqCst);
        }
    }
}

pub(super) fn spawn_stderr_logger<R>(
    stderr: R,
    target: &'static str,
    diagnostic: Option<Arc<Mutex<String>>>,
) -> JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if !line.trim().is_empty() {
                tracing::debug!(stderr_target = target, "{line}");
                if let Some(diagnostic) = &diagnostic {
                    *diagnostic.lock().expect("stderr diagnostic lock poisoned") = line;
                }
            }
        }
    })
}

/// A provider's stderr, kept beside the launch's capture as
/// `<harness>.stderr.log`: its own failures and its hooks' recording gaps.
/// `None` for a launch that has no capture.
pub(super) fn diagnostics(
    config: &crate::agent::AgentConfig,
    harness: &str,
) -> anyhow::Result<Option<std::fs::File>> {
    use anyhow::Context;
    let Some(key) = config.env.get(crate::session_record::CAPTURE_KEY_ENV) else {
        return Ok(None);
    };
    let path = crate::session_record::capture_dir(key)?.join(format!("{harness}.stderr.log"));
    let mut options = std::fs::OpenOptions::new();
    options.create(true).append(true).read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let log = options
        .open(&path)
        .with_context(|| format!("open {harness} diagnostics {}", path.display()))?;
    Ok(Some(log))
}
