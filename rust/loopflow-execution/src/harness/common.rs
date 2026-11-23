use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use super::fact_writer::FactWriter;
use super::Recording;
use crate::agent::AgentConfig;
use loopflow_harness::fact::Decoded;
use loopflow_model::chat::types::ConversationEvent;

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

// A harness that runs one process per turn (OpenCode, Antigravity)

/// The process running the current turn, until its exit is recorded.
#[derive(Debug, Default)]
pub(super) struct Run {
    pub pid: Option<u32>,
    pub reader: Option<JoinHandle<()>>,
    stopped: Arc<AtomicBool>,
}

impl Run {
    /// Admission makes the child the leader of its own group.
    pub fn group(&self) -> Option<u32> {
        self.pid.filter(|pid| *pid > 1)
    }

    /// Follow the process a turn just started. The flag it hands the turn's
    /// reader says whether lf stopped that process.
    pub fn began(&mut self, pid: Option<u32>) -> Arc<AtomicBool> {
        self.pid = pid;
        self.stopped = Arc::default();
        self.stopped.clone()
    }

    /// End the process as lf's own doing: its turn is interrupted, not failed.
    pub async fn stop(&mut self, owner: &Recording) -> anyhow::Result<()> {
        self.stopped.store(true, Ordering::SeqCst);
        self.settle(owner).await
    }

    /// Record the process's exit and stop whatever of it is left.
    pub async fn settle(&mut self, owner: &Recording) -> anyhow::Result<()> {
        if self.pid.take().is_some() {
            super::agent_process::stop(owner)?;
        }
        if let Some(reader) = self.reader.take() {
            // The process is gone, so its output ends and the reader settles the turn.
            let _ = reader.await;
        }
        Ok(())
    }
}

/// Where a run's stderr goes: the launch's `<harness>.stderr.log`, which
/// earlier runs appended to, or a file nobody keeps.
#[derive(Debug)]
pub(super) struct Stderr {
    file: std::fs::File,
    start: u64,
}

impl Stderr {
    /// Pipe `command`'s stdin and stdout to lf and send its stderr here.
    pub fn pipe(
        command: &mut tokio::process::Command,
        config: &AgentConfig,
        harness: &str,
    ) -> anyhow::Result<Self> {
        let file = match diagnostics(config, harness)? {
            Some(log) => log,
            None => tempfile::tempfile()?,
        };
        command
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(file.try_clone()?)
            // The shared stop owns cleanup; dropping a stale handle must not signal.
            .kill_on_drop(false);
        let start = file.metadata()?.len();
        Ok(Self { file, start })
    }

    /// What this run wrote, once it has exited.
    pub fn read(mut self) -> String {
        use std::io::{Read, Seek, SeekFrom};
        let mut text = String::new();
        let _ = self
            .file
            .seek(SeekFrom::Start(self.start))
            .and_then(|_| self.file.read_to_string(&mut text));
        text
    }
}

/// Writes what one run's turn establishes.
pub(super) fn writer(owner: &Recording) -> FactWriter {
    FactWriter::new(
        owner.store.clone(),
        owner.session.clone(),
        Some(owner.attachment.clone()),
        Default::default(),
    )
}

/// What a turn sends: the task opens the first turn, ahead of the input.
/// `None` when there is nothing to send.
pub(super) fn turn_content(
    content: &str,
    config: &AgentConfig,
    first_turn: bool,
) -> Option<String> {
    let task = if first_turn {
        config.task_prompt.trim()
    } else {
        ""
    };
    let parts: Vec<&str> = [task, content.trim()]
        .into_iter()
        .filter(|part| !part.is_empty())
        .collect();
    (!parts.is_empty()).then(|| parts.join("\n\n"))
}

/// Record what a turn established and hand its display events on. A record
/// lf could not write is a gap, never a lost conversation event.
pub(super) fn publish(
    writer: Option<&mut FactWriter>,
    decoded: Vec<Decoded>,
    events: &mpsc::UnboundedSender<ConversationEvent>,
) {
    if let Some(writer) = writer {
        if let Err(error) = writer.record(&decoded) {
            tracing::warn!(%error, "failed to record provider activity");
        }
    }
    for decoded in decoded {
        if let Decoded::Display(event) = decoded {
            let _ = events.send(event);
        }
    }
}

/// Make the next run a new AgentProcess of the same Session, named as the
/// caller in the run's environment.
pub(super) fn next_agent_process(
    owner: &mut Recording,
    runtime: Option<&crate::process::LfProcessRuntime>,
    config: &mut AgentConfig,
) -> anyhow::Result<()> {
    let Recording {
        store,
        session,
        attachment,
    } = owner;
    *attachment = match runtime {
        Some(runtime) => runtime.prepare_agent_process(session, attachment)?,
        None => {
            crate::process::attachment::prepare_session_agent_process(store, session, attachment)?
        }
    };
    config.env.insert(
        crate::process::AGENT_CALLER_ENV.into(),
        serde_json::to_string(
            &attachment.caller(session.clone().into(), crate::journal::current_trace_id()),
        )?,
    );
    Ok(())
}

/// Write a turn's input to its run as the Session's dispatch.
pub(super) async fn send(
    owner: Recording,
    mut stdin: tokio::process::ChildStdin,
    input: Vec<u8>,
) -> anyhow::Result<()> {
    let Recording {
        store,
        session,
        attachment,
    } = owner;
    tokio::task::spawn_blocking(move || {
        store.with_session_dispatch(&session, &attachment, || {
            super::dispatch::write_fenced(&mut stdin, &input)
        })
    })
    .await??;
    Ok(())
}

/// A turn's status, final answer and reported errors, once it settles.
#[cfg(test)]
pub(super) async fn drive_turn(
    rx: &mut mpsc::UnboundedReceiver<ConversationEvent>,
) -> (loopflow_model::chat::types::Lifecycle, String, Vec<String>) {
    use loopflow_model::chat::types::ConversationItem;
    let (mut text, mut errors) = (String::new(), Vec::new());
    loop {
        let event = tokio::time::timeout(std::time::Duration::from_secs(180), rx.recv())
            .await
            .expect("a turn settles")
            .expect("the driver is alive");
        match event {
            ConversationEvent::ItemCompleted {
                item: ConversationItem::Message { text: t, phase, .. },
                ..
            } if phase.as_deref() == Some("final") => text = t,
            ConversationEvent::Error { message, .. } => errors.push(message),
            ConversationEvent::TurnCompleted { status, .. } => return (status, text, errors),
            _ => {}
        }
    }
}
