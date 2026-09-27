//! One foreground reader. Pipe backpressure retains only the latest frame.

use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use anyhow::Result;
use serde::Deserialize;
use tokio_util::sync::CancellationToken;

use crate::durable::WorkRef;
use crate::run_record::active::{ActiveRunReader, ActiveRunsSnapshot, DiscoveryState};
use crate::store::SharedStore;

const PERIOD: Duration = Duration::from_secs(2);
const MAX_FRAME: usize = 16 * 1024 * 1024;

#[derive(Debug, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Refresh,
    Rescan,
}

#[derive(Debug, Default)]
struct Delivery {
    frame: Option<Vec<u8>>,
    busy: bool,
    refresh: bool,
    rescan: bool,
    error: Option<String>,
}

type Mailbox = Arc<(Mutex<Delivery>, Condvar)>;

fn frame(mut snapshot: ActiveRunsSnapshot) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(&snapshot)?;
    if bytes.len() > MAX_FRAME {
        snapshot.runs.clear();
        snapshot.discovery = DiscoveryState::Unavailable;
        snapshot.gaps = vec!["active Run snapshot exceeds the 16 MiB transport limit".into()];
        bytes = serde_json::to_vec(&snapshot)?;
    }
    bytes.push(b'\n');
    Ok(bytes)
}

pub(super) fn run(
    home: &Path,
    store: &SharedStore,
    task: Option<WorkRef>,
    runtime: &tokio::runtime::Runtime,
) -> Result<()> {
    let cancel = CancellationToken::new();
    let mut reader = ActiveRunReader::start(home, true, cancel.clone())?;
    let mailbox: Mailbox = Arc::new((Mutex::new(Delivery::default()), Condvar::new()));
    let pending = ActiveRunsSnapshot {
        home: reader.home().to_owned(),
        observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
        task: task.clone(),
        discovery: DiscoveryState::Scanning,
        runs: Vec::new(),
        gaps: vec!["Discovering active Run ownership".into()],
    };

    // These pipe threads belong to the foreground command. They never own or
    // signal providers. On EOF the main thread cancels/drains the native reader;
    // process exit also releases a writer blocked on an undrained output pipe.
    let input = mailbox.clone();
    let input_cancel = cancel.clone();
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        loop {
            // Bound requests before allocation, including missing newlines.
            let mut line = Vec::new();
            let read = std::io::Read::take(&mut stdin, 1025).read_until(b'\n', &mut line);
            let mut state = input.0.lock().expect("active reader mailbox poisoned");
            match read {
                Ok(0) => {
                    input_cancel.cancel();
                    input.1.notify_all();
                    break;
                }
                Ok(_) if line.len() <= 1024 => match serde_json::from_slice::<Request>(&line) {
                    Ok(Request::Refresh) => state.refresh = true,
                    Ok(Request::Rescan) => state.rescan = true,
                    Err(error) => {
                        state.error = Some(format!("invalid reader request: {error}"));
                        input_cancel.cancel();
                    }
                },
                _ => {
                    state.error = Some("reader stdin failed or request exceeds 1 KiB".into());
                    input_cancel.cancel();
                }
            }
            input.1.notify_all();
            if input_cancel.is_cancelled() {
                break;
            }
        }
    });
    let output = mailbox.clone();
    let output_cancel = cancel.clone();
    std::thread::spawn(move || {
        let mut stdout = std::io::stdout().lock();
        loop {
            let bytes = {
                let mut state = output.0.lock().expect("active reader mailbox poisoned");
                while state.frame.is_none() && !output_cancel.is_cancelled() {
                    state = output
                        .1
                        .wait(state)
                        .expect("active reader mailbox poisoned");
                }
                match state.frame.take() {
                    Some(frame) => frame,
                    None => return,
                }
            };
            if stdout
                .write_all(&bytes)
                .and_then(|()| stdout.flush())
                .is_err()
            {
                output_cancel.cancel();
                output.1.notify_all();
                return;
            }
        }
    });
    let heartbeat = mailbox.clone();
    let heartbeat_cancel = cancel.clone();
    let heartbeat_frame = frame(pending.clone())?;
    let heartbeat_thread = std::thread::spawn(move || {
        while !heartbeat_cancel.is_cancelled() {
            let state = heartbeat.0.lock().expect("active reader mailbox poisoned");
            let (mut state, _) = heartbeat
                .1
                .wait_timeout_while(state, PERIOD, |_| !heartbeat_cancel.is_cancelled())
                .expect("active reader mailbox poisoned");
            if state.busy && !heartbeat_cancel.is_cancelled() {
                state.frame = Some(heartbeat_frame.clone());
                heartbeat.1.notify_all();
            }
        }
    });
    let result = (|| -> Result<()> {
        let mut initial = true;
        loop {
            {
                let mut state = mailbox.0.lock().expect("active reader mailbox poisoned");
                if let Some(error) = state.error.take() {
                    anyhow::bail!(error);
                }
                if cancel.is_cancelled() {
                    break;
                }
                let rescan = std::mem::take(&mut state.rescan);
                if rescan {
                    reader.invalidate();
                }
                state.refresh = false;
                state.busy = true;
                if initial || rescan {
                    state.frame = Some(frame(pending.clone())?);
                    initial = false;
                }
                mailbox.1.notify_all();
            }
            let snapshot = runtime.block_on(reader.observe(store, task.clone()));
            let unavailable = snapshot.discovery == DiscoveryState::Unavailable;
            let mut state = mailbox.0.lock().expect("active reader mailbox poisoned");
            state.busy = false;
            state.frame = Some(frame(snapshot)?);
            mailbox.1.notify_all();
            // Persistent failures require an explicit rescan/Retry. Do not spin
            // through retained history on every cadence tick after a read error.
            loop {
                if cancel.is_cancelled() || state.refresh || state.rescan {
                    break;
                }
                let (next, timeout) = mailbox
                    .1
                    .wait_timeout(state, PERIOD)
                    .expect("active reader mailbox poisoned");
                state = next;
                if timeout.timed_out() && !unavailable {
                    break;
                }
            }
        }
        Ok(())
    })();
    cancel.cancel();
    mailbox.1.notify_all();
    heartbeat_thread
        .join()
        .expect("active reader heartbeat panicked");
    drop(reader);
    result
}

#[cfg(test)]
mod tests {
    use super::{frame, MAX_FRAME};
    use crate::run_record::active::{ActiveRunsSnapshot, DiscoveryState};

    #[test]
    fn oversized_frame_reports_unavailable_instead_of_truncating_to_empty() {
        let snapshot = ActiveRunsSnapshot {
            home: "/fixture".into(),
            observed_at: 1,
            task: None,
            discovery: DiscoveryState::Ready,
            runs: Vec::new(),
            gaps: vec!["x".repeat(MAX_FRAME)],
        };
        let bytes = frame(snapshot).unwrap();
        assert!(bytes.len() < MAX_FRAME);
        let received: ActiveRunsSnapshot = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(received.discovery, DiscoveryState::Unavailable);
        assert!(!received.gaps.is_empty());
    }
}
