//! `--watch` on an ordinary read: the command a client runs once is the one it
//! streams. Each frame is one NDJSON line whose `body` is what `--json` prints,
//! sent first and then each time that result changes.
//!
//! The store's change revisions say when a commit can have changed the result.
//! A reading runs only then, or on the read's clock for facts outside the store
//! (Process liveness, Git), on a read-only connection with one pinned snapshot.
//! An unchanged result sends nothing. A slow reading rests `BUSY_SHARE` times
//! its duration before a change is read again.
//!
//! Each stdin line is a request, `{"id": 1, "args": {...}}`. `args` replaces
//! the read's arguments; without it the request confirms the current result.
//! Either way the next frame answers it, changed or not. The watch ends, after
//! answering what it was asked, when stdin closes or its parent exits.

mod checkouts;

use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::store::changes::StoreChanges;
use crate::store::sqlite::StoreRevisions;
use crate::store::{SharedStore, StorageConfig};

/// A burst is read once it has been quiet this long…
const QUIET: Duration = Duration::from_millis(100);
/// …or this long after its first commit, so a steady writer cannot postpone it.
const BURST: Duration = Duration::from_millis(250);
/// Revisions are read again on this clock in case a filesystem event was lost.
const CHECK: Duration = Duration::from_secs(1);
/// A read that keeps failing is tried again after 1 s, 2 s, 4 s… up to this.
const RETRY_CAP: Duration = Duration::from_secs(60);
/// A read spends at most this share of the watch's time: after a reading that
/// took `d`, a change is read once `BUSY_SHARE × d` has passed since it began.
/// Requests read at once.
const BUSY_SHARE: u32 = 5;
/// Git and filesystem facts are not store commits; this bounds how stale they get.
pub(crate) const PLANNING_CLOCK: Duration = Duration::from_secs(300);
/// Process liveness is observed, never committed.
pub(crate) const ACTIVITY_CLOCK: Duration = Duration::from_secs(2);
const MAX_FRAME: usize = 64 * 1024 * 1024;
const MAX_REQUEST: usize = 16 * 1024;

/// One line of a watch.
#[derive(Debug, Serialize)]
pub struct WatchFrame<'a, T> {
    /// The newest request this reading began after.
    pub answers: Option<u64>,
    /// Why the read failed. `body` is then `null`.
    pub unavailable: Option<String>,
    pub body: Option<&'a T>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request<A> {
    id: u64,
    args: Option<A>,
}

/// What a watched read depends on beyond the store: every displayed revision
/// domain except token totals can change its result.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Watched {
    /// Facts outside the store are read again this often.
    pub clock: Duration,
    /// The result shows Git facts of Task checkouts: read again when one
    /// changes on disk and Git's answer about it differs.
    pub checkouts: bool,
}

impl Watched {
    pub(crate) fn every(clock: Duration) -> Self {
        Self {
            clock,
            checkouts: false,
        }
    }

    pub(crate) fn following_checkouts(clock: Duration) -> Self {
        Self {
            checkouts: true,
            ..Self::every(clock)
        }
    }
}

/// The store a read needs, or why the watch has none to read.
pub(crate) fn require_store(store: Option<SharedStore>) -> Result<SharedStore> {
    store.ok_or_else(|| anyhow::anyhow!("no Loopflow store exists on this Machine"))
}

/// Whether a commit that moved revisions from `old` to `new` can change a result.
fn changed(old: StoreRevisions, new: StoreRevisions) -> bool {
    StoreRevisions { usage: 0, ..old } != StoreRevisions { usage: 0, ..new }
}

#[derive(Debug)]
struct Mailbox<A> {
    requests: Vec<Request<A>>,
    look: bool,
    closed: bool,
    error: Option<String>,
}

type Shared<A> = Arc<(Mutex<Mailbox<A>>, Condvar)>;

fn lock<A>(shared: &Shared<A>) -> MutexGuard<'_, Mailbox<A>> {
    shared.0.lock().expect("watch mailbox poisoned")
}

/// Print `read(store, args)` as frames until stdin closes or the parent exits.
/// `store` is `None` until a Loopflow store exists on this Machine.
pub(crate) fn run<A, T, F, Fut>(args: A, watched: Watched, mut read: F) -> Result<()>
where
    A: DeserializeOwned + Clone + Send + 'static,
    T: Serialize,
    F: FnMut(Option<SharedStore>, A) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let database = crate::store::database_path_from_env()?;
    if watched.checkouts {
        // Every reading asks Git the same questions about the same checkouts;
        // only a checkout that changed on disk is asked again.
        crate::git::retain_reads();
    }
    let mut checkouts = checkouts::Checkouts::default();
    let mut followed_at = None;
    let mut stale = false;
    let shared: Shared<A> = Arc::new((
        Mutex::new(Mailbox {
            requests: Vec::new(),
            look: false,
            closed: false,
            error: None,
        }),
        Condvar::new(),
    ));
    let input = shared.clone();
    std::thread::spawn(move || read_requests(&input));
    let changes = shared.clone();
    let wal = database.clone();
    std::thread::spawn(move || observe(&changes, &wal));

    let runtime = tokio::runtime::Runtime::new()?;
    let mut stdout = std::io::stdout().lock();
    let mut args = args;
    let mut store: Option<SharedStore> = None;
    // Why the store that exists cannot be opened; a refused store is not an empty one.
    let mut refused: Option<String> = None;
    let mut answers = None;
    let mut force = true;
    let mut read_at = None;
    let mut read_on: Option<Instant> = None;
    let mut cost = Duration::ZERO;
    let mut failures = 0u32;
    let mut sent = None;
    loop {
        // A closed watch still answers what it was asked, then stops.
        let closing = {
            let mut mailbox = lock(&shared);
            if let Some(error) = mailbox.error.take() {
                anyhow::bail!(error);
            }
            mailbox.look = false;
            for request in std::mem::take(&mut mailbox.requests) {
                if let Some(next) = request.args {
                    args = next;
                }
                answers = Some(request.id);
                force = true;
            }
            mailbox.closed
        };
        if store.is_none() && database.exists() {
            let config = StorageConfig::sqlite(database.clone());
            match runtime.block_on(crate::store::open_read_only_store(&config)) {
                Ok(opened) => {
                    store = Some(Arc::new(opened));
                    refused = None;
                }
                Err(error) => refused = Some(format!("{error:#}")),
            }
        }
        let revisions = store
            .as_ref()
            .and_then(|store| store.sqlite.revisions().ok());
        if watched.checkouts {
            if let (Some(store), Some(revisions)) = (&store, revisions) {
                if followed_at != Some(revisions.planning) {
                    follow_checkouts(&mut checkouts, store, &shared);
                    followed_at = Some(revisions.planning);
                }
            }
            let changed = checkouts.take(Instant::now());
            // Every one is asked again, not only up to the first that differs.
            for worktree in &changed {
                stale |= crate::git::reread_retained(worktree);
            }
        }
        let now = Instant::now();
        let due = force
            || match read_on {
                None => true,
                Some(read_on) => {
                    let changed = match (read_at, revisions) {
                        (Some(old), Some(new)) => changed(old, new),
                        (None, None) => false,
                        _ => true,
                    };
                    retry_due(failures, read_on, now)
                        || (now - read_on >= cost.saturating_mul(BUSY_SHARE)
                            && (changed || stale || now - read_on >= watched.clock))
                }
            };
        if due {
            if !force && read_at.is_some() && read_at != revisions {
                settle(&shared);
            }
            // The first revision query pins the snapshot the whole reading uses.
            let snapshot = store
                .as_ref()
                .map(|store| store.sqlite.read_snapshot())
                .transpose()?;
            let revisions = store
                .as_ref()
                .map(|store| store.sqlite.revisions())
                .transpose()?;
            let began = Instant::now();
            let reading = match &refused {
                Some(reason) => Err(anyhow::anyhow!("Loopflow store cannot be read: {reason}")),
                None => runtime.block_on(read(store.clone(), args.clone())),
            };
            let (unavailable, body) = match reading {
                Ok(body) => (None, Some(body)),
                Err(error) => (Some(format!("{error:#}")), None),
            };
            drop(snapshot);
            cost = began.elapsed();
            let identity = identity(&unavailable, &body)?;
            if force || sent != Some(identity) {
                let frame = WatchFrame {
                    answers,
                    unavailable,
                    body: body.as_ref(),
                };
                if stdout
                    .write_all(&encode(&frame)?)
                    .and_then(|()| stdout.flush())
                    .is_err()
                {
                    return Ok(());
                }
            }
            failures = if body.is_none() {
                failures.saturating_add(1)
            } else {
                0
            };
            sent = Some(identity);
            read_at = revisions;
            read_on = Some(began);
            force = false;
            stale = false;
        }
        if closing {
            return Ok(());
        }
        let read_on = read_on.expect("a watch reads before it waits");
        let rest_end = read_on + cost.saturating_mul(BUSY_SHARE);
        let wake = if rest_end > now {
            rest_end
        } else {
            read_on + watched.clock
        };
        let wait = wake.saturating_duration_since(Instant::now()).min(CHECK);
        let mailbox = lock(&shared);
        drop(
            shared
                .1
                .wait_timeout_while(mailbox, wait, |mailbox| {
                    !mailbox.look
                        && mailbox.requests.is_empty()
                        && !mailbox.closed
                        && mailbox.error.is_none()
                })
                .expect("watch mailbox poisoned"),
        );
    }
}

/// A failed reading is retried without a change, each time after twice the
/// wait, so a read that cannot succeed does not spin.
fn retry_due(failures: u32, read_on: Instant, now: Instant) -> bool {
    failures > 0
        && now - read_on
            >= CHECK
                .saturating_mul(1 << (failures - 1).min(16))
                .min(RETRY_CAP)
}

/// Remove the fields that restate when a result was read.
fn strip_reading_time(value: &mut serde_json::Value) {
    match value {
        serde_json::Value::Object(fields) => {
            fields.remove("generated_at");
            fields.remove("evidence_age_secs");
            // A Task condition restates when it was derived.
            if let Some(serde_json::Value::Object(condition)) = fields.get_mut("condition") {
                condition.remove("observed_at");
            }
            fields.values_mut().for_each(strip_reading_time);
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(strip_reading_time),
        _ => {}
    }
}

fn identity<T: Serialize>(unavailable: &Option<String>, body: &Option<T>) -> Result<[u8; 32]> {
    let mut value = serde_json::to_value(body)?;
    strip_reading_time(&mut value);
    let mut hash = Sha256::new();
    hash.update(serde_json::to_vec(unavailable)?);
    hash.update(serde_json::to_vec(&value)?);
    Ok(hash.finalize().into())
}

fn encode<T: Serialize>(frame: &WatchFrame<'_, T>) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(frame)?;
    if bytes.len() > MAX_FRAME {
        bytes = serde_json::to_vec(&WatchFrame::<()> {
            answers: frame.answers,
            unavailable: Some("watch frame exceeds the 64 MiB transport limit".into()),
            body: None,
        })?;
    }
    bytes.push(b'\n');
    Ok(bytes)
}

/// Requests arrive one JSON line each on stdin. Its end closes the watch.
fn read_requests<A: DeserializeOwned>(shared: &Shared<A>) {
    let mut stdin = std::io::stdin().lock();
    loop {
        let mut line = Vec::new();
        let read =
            std::io::Read::take(&mut stdin, MAX_REQUEST as u64 + 1).read_until(b'\n', &mut line);
        let mut mailbox = lock(shared);
        match read {
            Ok(0) => mailbox.closed = true,
            Ok(_) if line.len() <= MAX_REQUEST => match serde_json::from_slice(&line) {
                Ok(request) => mailbox.requests.push(request),
                Err(error) => mailbox.error = Some(format!("invalid watch request: {error}")),
            },
            _ => mailbox.error = Some("watch stdin failed or request exceeds 16 KiB".into()),
        }
        let done = mailbox.closed || mailbox.error.is_some();
        shared.1.notify_all();
        if done {
            break;
        }
    }
}

/// Wake the watch when the store may have changed, and close it when its
/// parent exits: a descendant that inherited the pipe can hold stdin open
/// long after the parent is gone.
fn observe<A>(shared: &Shared<A>, database: &Path) {
    #[cfg(unix)]
    let parent = std::os::unix::process::parent_id();
    let mut store = StoreChanges::watch(database);
    loop {
        store.wait(CHECK);
        let mut mailbox = lock(shared);
        #[cfg(unix)]
        if std::os::unix::process::parent_id() != parent {
            mailbox.closed = true;
        }
        if mailbox.closed {
            shared.1.notify_all();
            break;
        }
        mailbox.look = true;
        shared.1.notify_all();
    }
}

/// Follow the checkouts of the Tasks registered now and the repositories
/// they belong to; a change wakes the watch.
fn follow_checkouts<A>(
    checkouts: &mut checkouts::Checkouts,
    store: &SharedStore,
    shared: &Shared<A>,
) where
    A: Send + 'static,
{
    let Ok(rows) = store.sqlite.task_checkouts() else {
        return;
    };
    let mut paths: Vec<_> = rows.into_iter().map(|row| row.worktree).collect();
    if let Ok(repositories) = store.sqlite.repository_paths() {
        paths.extend(repositories);
    }
    let wake = shared.clone();
    checkouts.watch(paths, move || {
        lock(&wake).look = true;
        wake.1.notify_all();
    });
}

/// Let a burst of related commits land before reading any of it.
fn settle<A>(shared: &Shared<A>) {
    let first = Instant::now();
    loop {
        let mailbox = lock(shared);
        let (mut mailbox, _) = shared
            .1
            .wait_timeout_while(mailbox, QUIET, |mailbox| !mailbox.look && !mailbox.closed)
            .expect("watch mailbox poisoned");
        let again = std::mem::take(&mut mailbox.look);
        if !again || first.elapsed() >= BURST {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{changed, identity, retry_due};
    use crate::store::sqlite::StoreRevisions;

    #[test]
    fn a_failing_read_waits_longer_before_each_retry() {
        let read_on = Instant::now();
        let due =
            |failures, secs| retry_due(failures, read_on, read_on + Duration::from_secs(secs));
        assert!(!due(0, 3600));
        assert!(due(1, 1));
        assert!(!due(3, 3));
        assert!(due(3, 4));
        assert!(!due(40, 59));
        assert!(due(40, 60));
    }

    #[test]
    fn usage_alone_is_not_a_change() {
        let old = StoreRevisions {
            planning: 1,
            sessions: 1,
            graphs: 1,
            processes: 1,
            usage: 1,
        };
        assert!(!changed(old, StoreRevisions { usage: 9, ..old }));
        assert!(changed(
            old,
            StoreRevisions {
                processes: 2,
                ..old
            }
        ));
    }

    #[test]
    fn the_time_of_a_reading_is_not_a_change() {
        let rows = |at| serde_json::json!([{"id": 1, "condition": {"observed_at": at}}]);
        let read = |body| identity(&None, &Some(body)).unwrap();
        assert_eq!(read(rows(1)), read(rows(2)));
        assert_ne!(
            read(rows(1)),
            identity(&Some("gone".into()), &Some(rows(1))).unwrap()
        );
    }
}
