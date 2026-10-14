//! Startup owns terminal output only until the native client is spawned.

use std::io::{IsTerminal, Write};
use std::sync::mpsc::{self, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub(crate) struct Startup {
    updates: Option<Sender<String>>,
    worker: Option<JoinHandle<()>>,
}

impl Startup {
    pub(crate) fn new(stage: impl Into<String>) -> Self {
        let stage = stage.into();
        if !std::io::stderr().is_terminal()
            || std::env::var("TERM").is_ok_and(|term| term == "dumb")
        {
            return Self {
                updates: None,
                worker: None,
            };
        }
        let (updates, receiver) = mpsc::channel();
        let worker = std::thread::spawn(move || {
            let started = Instant::now();
            let mut stage = stage;
            let mut frame = 0;
            loop {
                let glyph = ['|', '/', '-', '\\'][frame % 4];
                {
                    let mut stderr = std::io::stderr().lock();
                    let _ = write!(
                        stderr,
                        "\r\x1b[2K{glyph} {stage} ({}s)",
                        started.elapsed().as_secs()
                    );
                    let _ = stderr.flush();
                }
                match receiver.recv_timeout(Duration::from_millis(120)) {
                    Ok(next) => stage = next,
                    Err(mpsc::RecvTimeoutError::Timeout) => frame += 1,
                    Err(mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
            let mut stderr = std::io::stderr().lock();
            let _ = write!(stderr, "\r\x1b[2K");
            let _ = stderr.flush();
        });
        Self {
            updates: Some(updates),
            worker: Some(worker),
        }
    }

    pub(crate) fn stage(&self, stage: impl Into<String>) {
        if let Some(updates) = &self.updates {
            let _ = updates.send(stage.into());
        }
    }

    pub(crate) fn finish(&mut self) {
        self.updates.take();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for Startup {
    fn drop(&mut self) {
        self.finish();
    }
}
