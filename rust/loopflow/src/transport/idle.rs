//! End Sessions nobody has viewed for `session.end_detached_after`.
//!
//! Each owner records when its last viewer leaves and periodically judges every
//! running owner on the Machine, so owners from builds that never recorded a
//! detach time are judged too. Ending one signals that owner: it stops its
//! agent and records its exit, so `lf resume` reopens the conversation from
//! provider history and `lf resume --all` leaves it alone.

use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;

use crate::config::DetachedLimit;
use crate::id::LfProcessId;
use crate::store::sqlite::sessions::IdleSessionOwner;
use crate::store::sqlite::SqliteStore;

/// Owners that never recorded a detach time end this long after they started.
const UNRECORDED_LIMIT: Duration = Duration::from_secs(48 * 60 * 60);
const LONGEST_PAUSE: Duration = Duration::from_secs(5 * 60);

/// Record viewer changes sent on the returned channel and judge owners until
/// this owner drops it.
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
    let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
    let repo = store.process(owner)?.and_then(|process| process.repo);
    let pause = match limit(repo.as_deref()) {
        Some(DetachedLimit::After(limit)) => {
            (limit / 12).clamp(Duration::from_secs(1), LONGEST_PAUSE)
        }
        _ => LONGEST_PAUSE,
    };
    let mut next = Instant::now() + pause;
    loop {
        match changes.recv_timeout(next.saturating_duration_since(Instant::now())) {
            Ok(viewed) => store.record_owner_viewers(owner, viewed, now())?,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Err(error) = sweep(&store) {
                    tracing::warn!(%error, "judge detached Sessions");
                }
                next = Instant::now() + pause;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}

fn now() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
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

fn sweep(store: &SqliteStore) -> Result<()> {
    let now = now();
    let mut limits = HashMap::new();
    for owner in store.idle_session_owners()? {
        let limit = *limits
            .entry(owner.repo.clone())
            .or_insert_with(|| limit(owner.repo.as_deref()));
        if !limit.is_some_and(|limit| due(limit, &owner, now)) {
            continue;
        }
        if crate::journal::process_evidence(store, &owner.id)
            != crate::journal::ProcessIdentityEvidence::Live
        {
            continue;
        }
        let Some(pid) = store.process(&owner.id)?.and_then(|process| process.pid) else {
            continue;
        };
        tracing::info!(owner = %owner.id, pid, "ending a Session nobody has viewed");
        // SAFETY: kill only signals; the pid's identity was just verified live.
        unsafe {
            libc::kill(pid as libc::pid_t, libc::SIGTERM);
        }
    }
    Ok(())
}

/// Whether `owner` has gone unviewed past `limit` at `now`.
fn due(limit: DetachedLimit, owner: &IdleSessionOwner, now: i64) -> bool {
    let DetachedLimit::After(limit) = limit else {
        return false;
    };
    let (since, limit) = match (owner.viewed, owner.detached_at) {
        (Some(true), _) => return false,
        (Some(false), Some(at)) => (at, limit),
        _ => (owner.started_at, UNRECORDED_LIMIT),
    };
    now - since >= i64::try_from(limit.as_secs()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{due, IdleSessionOwner, UNRECORDED_LIMIT};
    use crate::config::DetachedLimit;
    use crate::id::LfProcessId;

    const DAY: i64 = 24 * 60 * 60;

    fn owner(viewed: Option<bool>, detached_at: Option<i64>) -> IdleSessionOwner {
        IdleSessionOwner {
            id: LfProcessId::new(),
            repo: None,
            started_at: 0,
            viewed,
            detached_at,
        }
    }

    #[test]
    fn counts_from_when_the_last_viewer_left() {
        let limit = DetachedLimit::default();
        let left = owner(Some(false), Some(10 * DAY));
        assert!(!due(limit, &left, 11 * DAY - 1));
        assert!(due(limit, &left, 11 * DAY));
    }

    #[test]
    fn an_attached_viewer_keeps_it_running() {
        assert!(!due(
            DetachedLimit::default(),
            &owner(Some(true), None),
            30 * DAY
        ));
    }

    #[test]
    fn off_keeps_every_session() {
        assert!(!due(
            DetachedLimit::Off,
            &owner(Some(false), Some(0)),
            30 * DAY
        ));
        assert!(!due(DetachedLimit::Off, &owner(None, None), 30 * DAY));
    }

    #[test]
    fn unrecorded_owners_end_two_days_after_starting() {
        let short = DetachedLimit::After(Duration::from_secs(60));
        let unrecorded = owner(None, None);
        let limit = UNRECORDED_LIMIT.as_secs() as i64;
        assert!(!due(short, &unrecorded, limit - 1));
        assert!(due(short, &unrecorded, limit));
    }
}
