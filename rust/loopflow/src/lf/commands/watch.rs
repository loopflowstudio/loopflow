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
//!
//! [`Subscription`] serves the same watch inside this process, as Ghostty's
//! embedding API does: a data-free wakeup from any thread says a result may
//! have changed, and the app pulls on its own thread, where the reading runs
//! and coalesces a burst into one frame. The app reads exactly what the `lf`
//! command would print.

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
pub(crate) fn run<A, T, F, Fut>(args: A, watched: Watched, read: F) -> Result<()>
where
    A: DeserializeOwned + Clone + Send + 'static,
    T: Serialize,
    F: FnMut(Option<SharedStore>, A) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    // Read before anything can be scheduled: a parent that exits after this
    // is noticed even if it exits before the observer starts.
    let parent = parent_process();
    let shared = mailbox();
    let input = shared.clone();
    std::thread::spawn(move || read_requests(&input));
    let mut reader = Reader::new(shared.clone(), parent, args, watched, read)?;
    let mut stdout = std::io::stdout().lock();
    loop {
        let step = reader.step()?;
        if let Some(frame) = step.frame {
            if stdout
                .write_all(&frame)
                .and_then(|()| stdout.flush())
                .is_err()
            {
                return Ok(());
            }
        }
        if step.closing {
            return Ok(());
        }
        wait(&shared, step.wake);
    }
}

/// Where a watch's requests come from and its frames go.
pub enum Transport {
    /// stdin and stdout, ending with the parent: `lf <read> --json --watch`.
    Stdio,
    /// Requests and pulls through the returned [`Subscription`]; the callback
    /// says, from any thread and carrying nothing, that a pull may find a frame.
    Pull(Box<dyn Fn() + Send + Sync>),
}

impl std::fmt::Debug for Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Stdio => "Stdio",
            Self::Pull(_) => "Pull",
        })
    }
}

/// Serve `read` over `transport`. Stdio runs until the watch ends; a pulled
/// watch returns its subscription at once.
pub(crate) fn start<A, T, F, Fut>(
    args: A,
    watched: Watched,
    read: F,
    transport: Transport,
) -> Result<Option<Subscription>>
where
    A: DeserializeOwned + Clone + Send + 'static,
    T: Serialize,
    F: FnMut(Option<SharedStore>, A) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T>>,
{
    match transport {
        Transport::Stdio => run(args, watched, read).map(|()| None),
        Transport::Pull(wake) => subscribe(args, watched, read, wake).map(Some),
    }
}

/// A watch served inside this process. Requests are the lines
/// `lf <read> --json --watch` takes on stdin; [`Subscription::pull`] reads on
/// the caller's thread and returns the frame that command would print next.
/// Dropping it ends the watch.
pub struct Subscription {
    inbox: Arc<dyn Inbox>,
    pull: Mutex<Box<dyn FnMut() -> Option<Vec<u8>> + Send>>,
}

impl std::fmt::Debug for Subscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Subscription").finish_non_exhaustive()
    }
}

impl Subscription {
    /// One request line, as on the stdin of the equivalent `lf` watch.
    pub fn request(&self, line: &[u8]) {
        self.inbox.deliver(line);
    }

    /// The newest changed frame, or `None` when nothing changed since the last
    /// pull. Any read snapshot is released before this returns. A watch that
    /// fails or panics returns its error as a last frame.
    pub fn pull(&self) -> Option<Vec<u8>> {
        let mut pull = self
            .pull
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        (*pull)()
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.inbox.close();
    }
}

/// A mailbox whose argument type is known only to its watch.
trait Inbox: Send + Sync {
    fn deliver(&self, line: &[u8]);
    fn close(&self);
}

impl<A: DeserializeOwned + Send> Inbox for (Mutex<Mailbox<A>>, Condvar) {
    fn deliver(&self, line: &[u8]) {
        let mut mailbox = self.0.lock().expect("watch mailbox poisoned");
        accept(&mut mailbox, line);
        self.1.notify_all();
    }

    fn close(&self) {
        self.0.lock().expect("watch mailbox poisoned").closed = true;
        self.1.notify_all();
    }
}

/// Serve `read` as `run` does, read on the puller's thread. A thread of this
/// process only waits for the next reason to read and calls `wake`.
fn subscribe<A, T, F, Fut>(
    args: A,
    watched: Watched,
    read: F,
    wake: Box<dyn Fn() + Send + Sync>,
) -> Result<Subscription>
where
    A: DeserializeOwned + Clone + Send + 'static,
    T: Serialize,
    F: FnMut(Option<SharedStore>, A) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T>>,
{
    let shared = mailbox();
    let inbox: Arc<dyn Inbox> = shared.clone();
    let mut reader = Reader::new(shared.clone(), None, args, watched, read)?;
    // When the puller should read next; `None` once it was woken for it.
    let due: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(Some(Instant::now())));
    let waker = (shared.clone(), due.clone());
    std::thread::spawn(move || {
        let (shared, due) = waker;
        loop {
            let mailbox = lock(&shared);
            if mailbox.closed {
                return;
            }
            let wake_at = *due.lock().expect("watch deadline poisoned");
            let Some(wake_at) = wake_at else {
                // Woken already: wait for its pull, or for the watch to close.
                drop(
                    shared
                        .1
                        .wait_timeout(mailbox, CHECK)
                        .expect("watch mailbox poisoned"),
                );
                continue;
            };
            let asked = mailbox.look || !mailbox.requests.is_empty() || mailbox.error.is_some();
            if asked || Instant::now() >= wake_at {
                drop(mailbox);
                *due.lock().expect("watch deadline poisoned") = None;
                wake();
                continue;
            }
            let wait = wake_at.saturating_duration_since(Instant::now()).min(CHECK);
            drop(
                shared
                    .1
                    .wait_timeout(mailbox, wait)
                    .expect("watch mailbox poisoned"),
            );
        }
    });
    let mut ended = false;
    let pull = move || -> Option<Vec<u8>> {
        if ended {
            return None;
        }
        let stepped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| reader.step()));
        let error = match stepped {
            Ok(Ok(step)) => {
                *due.lock().expect("watch deadline poisoned") = Some(step.wake);
                ended = step.closing;
                return step.frame;
            }
            Ok(Err(error)) => format!("{error:#}"),
            Err(panic) => format!("watch panicked: {}", panic_message(&panic)),
        };
        ended = true;
        encode(&WatchFrame::<()> {
            answers: None,
            unavailable: Some(error),
            body: None,
        })
        .ok()
    };
    Ok(Subscription {
        inbox,
        pull: Mutex::new(Box::new(pull)),
    })
}

/// The text a panic was raised with.
pub fn panic_message(panic: &(dyn std::any::Any + Send)) -> &str {
    panic
        .downcast_ref::<&str>()
        .copied()
        .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
        .unwrap_or("unknown panic")
}

fn mailbox<A>() -> Shared<A> {
    Arc::new((
        Mutex::new(Mailbox {
            requests: Vec::new(),
            look: false,
            closed: false,
            error: None,
        }),
        Condvar::new(),
    ))
}

/// Wait until `wake`, or until the mailbox has a reason to read sooner.
fn wait<A>(shared: &Shared<A>, wake: Instant) {
    let wait = wake.saturating_duration_since(Instant::now()).min(CHECK);
    let mailbox = lock(shared);
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

/// What one step of a watch did.
struct Step {
    /// The frame to send, when the result changed or a request was answered.
    frame: Option<Vec<u8>>,
    /// The watch has answered what it was asked and ends.
    closing: bool,
    /// When to step again unless the mailbox has a reason sooner.
    wake: Instant,
}

/// One watch's reading state: what it last read and sent, and when.
struct Reader<A, F> {
    shared: Shared<A>,
    database: std::path::PathBuf,
    watched: Watched,
    read: F,
    args: A,
    runtime: tokio::runtime::Runtime,
    checkouts: checkouts::Checkouts,
    followed_at: Option<i64>,
    stale: bool,
    store: Option<SharedStore>,
    // Why the store that exists cannot be opened; a refused store is not an empty one.
    refused: Option<String>,
    answers: Option<u64>,
    force: bool,
    read_at: Option<StoreRevisions>,
    read_on: Option<Instant>,
    cost: Duration,
    failures: u32,
    sent: Option<[u8; 32]>,
}

impl<A, T, F, Fut> Reader<A, F>
where
    A: DeserializeOwned + Clone + Send + 'static,
    T: Serialize,
    F: FnMut(Option<SharedStore>, A) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    /// Start observing the store for `shared`'s watch, closing it when
    /// `parent` (when it follows one) exits.
    fn new(
        shared: Shared<A>,
        parent: Option<u32>,
        args: A,
        watched: Watched,
        read: F,
    ) -> Result<Self> {
        let database = crate::store::database_path_from_env()?;
        if watched.checkouts {
            // Every reading asks Git the same questions about the same checkouts;
            // only a checkout that changed on disk is asked again.
            crate::git::retain_reads();
        }
        let changes = shared.clone();
        let wal = database.clone();
        std::thread::spawn(move || observe(&changes, &wal, parent));
        Ok(Self {
            shared,
            database,
            watched,
            read,
            args,
            runtime: tokio::runtime::Runtime::new()?,
            checkouts: checkouts::Checkouts::default(),
            followed_at: None,
            stale: false,
            store: None,
            refused: None,
            answers: None,
            force: true,
            read_at: None,
            read_on: None,
            cost: Duration::ZERO,
            failures: 0,
            sent: None,
        })
    }

    /// Take requests, read when the result may have changed, and say what to
    /// send and when to step again.
    fn step(&mut self) -> Result<Step> {
        // A closed watch still answers what it was asked, then stops.
        let closing = {
            let mut mailbox = lock(&self.shared);
            if let Some(error) = mailbox.error.take() {
                anyhow::bail!(error);
            }
            mailbox.look = false;
            for request in std::mem::take(&mut mailbox.requests) {
                if let Some(next) = request.args {
                    self.args = next;
                }
                self.answers = Some(request.id);
                self.force = true;
            }
            mailbox.closed
        };
        if self.store.is_none() && self.database.exists() {
            let config = StorageConfig::sqlite(self.database.clone());
            match self
                .runtime
                .block_on(crate::store::open_read_only_store(&config))
            {
                Ok(opened) => {
                    self.store = Some(Arc::new(opened));
                    self.refused = None;
                }
                Err(error) => self.refused = Some(format!("{error:#}")),
            }
        }
        let revisions = self
            .store
            .as_ref()
            .and_then(|store| store.sqlite.revisions().ok());
        if self.watched.checkouts {
            if let (Some(store), Some(revisions)) = (&self.store, revisions) {
                if self.followed_at != Some(revisions.planning) {
                    follow_checkouts(&mut self.checkouts, store, &self.shared);
                    self.followed_at = Some(revisions.planning);
                }
            }
            let changed = self.checkouts.take(Instant::now());
            // Every one is asked again, not only up to the first that differs.
            for worktree in &changed {
                self.stale |= crate::git::reread_retained(worktree);
            }
        }
        let now = Instant::now();
        let due = self.force
            || match self.read_on {
                None => true,
                Some(read_on) => {
                    let changed = match (self.read_at, revisions) {
                        (Some(old), Some(new)) => changed(old, new),
                        (None, None) => false,
                        _ => true,
                    };
                    retry_due(self.failures, read_on, now)
                        || (now - read_on >= self.cost.saturating_mul(BUSY_SHARE)
                            && (changed || self.stale || now - read_on >= self.watched.clock))
                }
            };
        let mut frame = None;
        if due {
            if !self.force && self.read_at.is_some() && self.read_at != revisions {
                settle(&self.shared);
            }
            // The first revision query pins the snapshot the whole reading uses.
            let snapshot = self
                .store
                .as_ref()
                .map(|store| store.sqlite.read_snapshot())
                .transpose()?;
            let revisions = self
                .store
                .as_ref()
                .map(|store| store.sqlite.revisions())
                .transpose()?;
            let began = Instant::now();
            let reading = match &self.refused {
                Some(reason) => Err(anyhow::anyhow!("Loopflow store cannot be read: {reason}")),
                None => self
                    .runtime
                    .block_on((self.read)(self.store.clone(), self.args.clone())),
            };
            let (unavailable, body) = match reading {
                Ok(body) => (None, Some(body)),
                Err(error) => (Some(format!("{error:#}")), None),
            };
            drop(snapshot);
            self.cost = began.elapsed();
            let identity = identity(&unavailable, &body)?;
            if self.force || self.sent != Some(identity) {
                frame = Some(encode(&WatchFrame {
                    answers: self.answers,
                    unavailable,
                    body: body.as_ref(),
                })?);
            }
            self.failures = if body.is_none() {
                self.failures.saturating_add(1)
            } else {
                0
            };
            self.sent = Some(identity);
            self.read_at = revisions;
            self.read_on = Some(began);
            self.force = false;
            self.stale = false;
        }
        let read_on = self.read_on.expect("a watch reads before it waits");
        let rest_end = read_on + self.cost.saturating_mul(BUSY_SHARE);
        let wake = if rest_end > now {
            rest_end
        } else {
            read_on + self.watched.clock
        };
        Ok(Step {
            frame,
            closing,
            wake,
        })
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
    // Streamed: a large reading is never held a second time as bytes.
    let mut hash = Sha256::new();
    serde_json::to_writer(&mut hash, unavailable)?;
    serde_json::to_writer(&mut hash, &value)?;
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
            Ok(_) => accept(&mut mailbox, &line),
            Err(_) => mailbox.error = Some("watch stdin failed".into()),
        }
        let done = mailbox.closed || mailbox.error.is_some();
        shared.1.notify_all();
        if done {
            break;
        }
    }
}

/// Take one request line, or the error that ends the watch.
fn accept<A: DeserializeOwned>(mailbox: &mut Mailbox<A>, line: &[u8]) {
    if line.len() > MAX_REQUEST {
        mailbox.error = Some("watch request exceeds 16 KiB".into());
        return;
    }
    match serde_json::from_slice(line) {
        Ok(request) => mailbox.requests.push(request),
        Err(error) => mailbox.error = Some(format!("invalid watch request: {error}")),
    }
}

/// Wake the watch when the store may have changed, and close it when its
/// parent exits: a descendant that inherited the pipe can hold stdin open
/// long after the parent is gone.
fn observe<A>(shared: &Shared<A>, database: &Path, parent: Option<u32>) {
    let mut store = StoreChanges::watch(database);
    loop {
        store.wait(CHECK);
        let mut mailbox = lock(shared);
        // A watch served in-process follows no parent.
        if parent.is_some() && parent_process() != parent {
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

/// The process that started this one; an orphan is adopted by another.
fn parent_process() -> Option<u32> {
    #[cfg(unix)]
    return Some(std::os::unix::process::parent_id());
    #[cfg(not(unix))]
    None
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
