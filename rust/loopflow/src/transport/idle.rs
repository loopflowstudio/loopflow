//! End a Session nobody has viewed for `session.end_detached_after`.
//!
//! Each owner times its own viewers: when the last one leaves it waits out the
//! limit, then ends itself if its Sessions are still waiting for a person. Ending
//! stops its agent and records its exit, so `lf resume` reopens the conversation
//! from provider history and `lf resume --all` leaves it alone.

use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::config::DetachedLimit;
use crate::id::LfProcessId;

/// How often an owner past its limit looks again while a turn or Graph holds it.
const LONGEST_PAUSE: Duration = Duration::from_secs(5 * 60);

/// Record viewer changes sent on the returned channel and end this owner once
/// it has gone unviewed past its limit.
pub(super) fn spawn(owner: LfProcessId) -> mpsc::Sender<bool> {
    let (viewers, changes) = mpsc::channel();
    std::thread::spawn(move || {
        if let Err(error) = watch(&owner, &changes) {
            tracing::warn!(%error, %owner, "detached Session policy stopped");
        }
    });
    viewers
}

fn watch(owner: &LfProcessId, changes: &mpsc::Receiver<bool>) -> Result<()> {
    let store = crate::journal::process_ledger()?;
    let repo = store.process(owner)?.and_then(|process| process.repo);
    let Some(DetachedLimit::After(limit)) = limit(repo.as_deref()) else {
        // Off or unreadable: keep recording viewers and never end.
        while let Ok(viewed) = changes.recv() {
            store.record_owner_viewers(owner, viewed)?;
        }
        return Ok(());
    };
    let pause = (limit / 12).clamp(Duration::from_secs(1), LONGEST_PAUSE);
    let mut due: Option<Instant> = None;
    loop {
        let change = match due {
            Some(due) => changes.recv_timeout(due.saturating_duration_since(Instant::now())),
            None => changes
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
        };
        match change {
            Ok(viewed) => {
                store.record_owner_viewers(owner, viewed)?;
                due = (!viewed).then(|| Instant::now() + limit);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if store.owner_waits_for_person(owner)? {
                    tracing::info!(%owner, "ending a Session nobody has viewed");
                    crate::platform::kill_process(std::process::id());
                    return Ok(());
                }
                due = Some(Instant::now() + pause);
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}

/// The limit configured for `repo`; `None` when its configuration is unreadable.
fn limit(repo: Option<&str>) -> Option<DetachedLimit> {
    match crate::config::load_config(repo.map(Path::new)) {
        Ok(config) => Some(config.unwrap_or_default().session.end_detached_after),
        Err(error) => {
            tracing::warn!(%error, ?repo, "keep detached Sessions: configuration unreadable");
            None
        }
    }
}
