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

/// Why a headless turn failed when its agent asked a person something.
pub(super) fn unanswered_question(harness: &str, question: &str) -> String {
    let question = question.trim();
    if question.is_empty() {
        format!("{harness} asked a question with nobody there to answer it")
    } else {
        format!("{harness} asked a question with nobody there to answer it: {question}")
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
