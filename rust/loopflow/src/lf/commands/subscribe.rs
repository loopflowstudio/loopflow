//! `--subscribe` on an ordinary read: the command a client runs once is the one it
//! streams. Each frame is one NDJSON line whose `body` is what `--json` prints,
//! sent first and then each time that result changes.
//!
//! The store's change revisions say when a commit can have changed the result.
//! A reading runs only then, or on the read's clock for facts outside the store
//! (Process liveness, Git), on a read-only connection with one pinned snapshot.
//! A read whose outside facts can be observed cheaply probes them on its clock
//! and reads only when they moved. An unchanged result sends nothing. A slow
//! reading rests `BUSY_SHARE` times its duration before a change is read again.
//! Configuration inputs are captured during each reading and checked on the
//! short observation clock, independently of Git and planning refreshes.
//!
//! Each stdin line is a request, `{"id": 1, "args": {...}}`. `args` replaces
//! the read's arguments; without it the request confirms the current result.
//! Either way the next frame answers it, changed or not. The subscription ends, after
//! answering what it was asked, when stdin closes or its parent exits.
//!
//! [`Subscription`] serves the same reading inside this process, as Ghostty's
//! embedding API does: a data-free wakeup from any thread says a result may
//! have changed, and the app pulls on its own thread, where the reading runs
//! and coalesces a burst into one frame. The app reads exactly what the `lf`
//! command would print. A subscription's arguments are fixed; it takes only
//! checkpoints.

mod checkouts;

use std::collections::HashMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, OnceLock, Weak};
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::config::observation;
use crate::store::changes::StoreChanges;
use crate::store::sqlite::{SqliteStore, StoreRevisions};
use crate::store::{SharedStore, StorageConfig};

/// A burst is read once it has been quiet this long…
const QUIET: Duration = Duration::from_millis(100);
/// …or this long after its first commit, so a steady writer cannot postpone it.
const BURST: Duration = Duration::from_millis(250);
/// Revisions are read again on this clock in case a filesystem event was lost.
const CHECK: Duration = Duration::from_secs(1);
/// A read that keeps failing is tried again after 1 s, 2 s, 4 s… up to this.
const RETRY_CAP: Duration = Duration::from_secs(60);
/// A read spends at most this share of the subscription's time: after a reading that
/// took `d`, a change is read once `BUSY_SHARE × d` has passed since it began.
/// Requests read at once.
const BUSY_SHARE: u32 = 5;
/// Git and filesystem facts are not store commits; this bounds how stale they get.
pub(crate) const PLANNING_CLOCK: Duration = Duration::from_secs(300);
/// Process liveness is observed, never committed.
pub(crate) const ACTIVITY_CLOCK: Duration = Duration::from_secs(2);
const MAX_FRAME: usize = 64 * 1024 * 1024;
const MAX_REQUEST: usize = 16 * 1024;

thread_local! {
    static SELECTED: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

/// Keep only `paths` of this invocation's JSON answers and subscription
/// frames: `lf <read> --json --select a.b,c`. Served requests carry their
/// own `select` and never read this.
pub fn select_output(paths: Option<Vec<String>>) {
    SELECTED.with(|selected| *selected.borrow_mut() = paths);
}

fn selected() -> Option<Vec<String>> {
    SELECTED.with(|selected| selected.borrow().clone())
}

/// `answer` as this invocation's `--json` prints it, keeping its `--select`.
pub fn selected_answer<T: Serialize>(answer: &T) -> Result<serde_json::Value> {
    let answer = serde_json::to_value(answer)?;
    Ok(match selected() {
        Some(paths) => crate::engine::select::select(&answer, &paths),
        None => answer,
    })
}

/// One line of a subscription.
#[derive(Debug, Serialize, schemars::JsonSchema)]
pub struct SubscriptionFrame<'a, T> {
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

/// What a subscribed read depends on beyond the store: every displayed revision
/// domain except token totals can change its result.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Wake {
    /// Facts outside the store are read again this often.
    pub clock: Duration,
    /// The result shows Git facts of Task checkouts: read again when one
    /// changes on disk and Git's answer about it differs.
    pub checkouts: bool,
    /// The facts outside the store the result reads, observed far more
    /// cheaply than the reading. Without one, every clock tick reads.
    pub probe: Option<Probe>,
}

/// Observe the facts a read takes from outside the store at this moment.
pub(crate) type Probe = fn(&SqliteStore) -> Result<Vec<u8>>;

impl Wake {
    pub(crate) fn every(clock: Duration) -> Self {
        Self {
            clock,
            checkouts: false,
            probe: None,
        }
    }

    /// Read on the clock only when `probe`'s answer differs from the one
    /// observed before the last reading.
    pub(crate) fn probing(self, probe: Probe) -> Self {
        Self {
            probe: Some(probe),
            ..self
        }
    }

    pub(crate) fn following_checkouts(clock: Duration) -> Self {
        Self {
            checkouts: true,
            ..Self::every(clock)
        }
    }
}

/// The store a read needs, or why the subscription has none to read.
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
    /// When an in-process puller should step next; `None` while its wakeup
    /// waits for a pull.
    pull_at: Option<Instant>,
}

impl<A> Mailbox<A> {
    /// Whether the subscription should step before its clock says.
    fn asked(&self) -> bool {
        self.look || !self.requests.is_empty() || self.error.is_some()
    }
}

type Shared<A> = Arc<(Mutex<Mailbox<A>>, Condvar)>;

fn lock<A>(shared: &Shared<A>) -> MutexGuard<'_, Mailbox<A>> {
    shared.0.lock().expect("subscription mailbox poisoned")
}

/// Print `read(store, args)` as frames, keeping the invocation's `--select`,
/// until stdin closes or the parent exits.
/// `store` is `None` until a Loopflow store exists on this Machine.
pub(crate) fn run<A, T, F, Fut>(args: A, wakes: Wake, read: F) -> Result<()>
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
    let mut reader = Reader::new(shared.clone(), parent, args, wakes, read, selected())?;
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

/// Where a subscription's requests come from and its frames go.
pub enum Transport {
    /// stdin and stdout, ending with the parent: `lf <read> --json --subscribe`,
    /// keeping the invocation's `--select`.
    Stdio,
    /// Requests and pulls through the returned [`Subscription`]; `wake` says,
    /// from any thread and carrying nothing, that a pull may find a frame.
    /// `select` keeps only those dotted paths of each body; a change outside
    /// them sends no frame.
    Pull {
        wake: Box<dyn Fn() + Send + Sync>,
        select: Option<Vec<String>>,
    },
}

impl std::fmt::Debug for Transport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Stdio => "Stdio",
            Self::Pull { .. } => "Pull",
        })
    }
}

/// Serve `read` over `transport`. Stdio runs until the subscription ends; a pulled
/// subscription returns its handle at once.
pub(crate) fn start<A, T, F, Fut>(
    args: A,
    wakes: Wake,
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
        Transport::Stdio => run(args, wakes, read).map(|()| None),
        Transport::Pull { wake, select } => start_pull(args, wakes, read, wake, select).map(Some),
    }
}

/// A subscription served inside this process. It takes checkpoints, never argument
/// changes: a client with new arguments subscriptions again. [`Subscription::pull`] reads on
/// the caller's thread and returns the frame that command would print next.
/// Dropping it ends the subscription.
pub struct Subscription {
    inbox: Arc<dyn Inbox>,
    pull: Mutex<Box<dyn FnMut() -> Option<Vec<u8>> + Send>>,
    ended: Arc<AtomicBool>,
}

impl std::fmt::Debug for Subscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Subscription").finish_non_exhaustive()
    }
}

impl Subscription {
    /// `{"id": checkpoint}` on the stdin of the equivalent `lf` subscription: the
    /// next frame answers it, changed or not.
    pub fn checkpoint(&self, checkpoint: u64) {
        self.inbox.checkpoint(checkpoint);
    }

    /// The newest changed frame, or `None` when nothing changed since the last
    /// pull. Any read snapshot is released before this returns. A subscription that
    /// fails or panics returns its error as a last frame.
    pub fn pull(&self) -> Option<Vec<u8>> {
        let mut pull = self
            .pull
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        (*pull)()
    }

    /// The subscription failed and sent its error as a last frame; nothing follows,
    /// as nothing follows the exit of `lf <read> --json --subscribe`.
    pub fn ended(&self) -> bool {
        self.ended.load(Ordering::Acquire)
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.inbox.close();
    }
}

/// A mailbox whose argument type is known only to its subscription.
trait Inbox: Send + Sync {
    fn checkpoint(&self, id: u64);
    fn close(&self);
    /// Ask the subscription to look at the store again; false once it has closed.
    fn look(&self) -> bool;
}

impl<A: DeserializeOwned + Send> Inbox for (Mutex<Mailbox<A>>, Condvar) {
    fn checkpoint(&self, id: u64) {
        let mut mailbox = self.0.lock().expect("subscription mailbox poisoned");
        mailbox.requests.push(Request { id, args: None });
        self.1.notify_all();
    }

    fn close(&self) {
        self.0.lock().expect("subscription mailbox poisoned").closed = true;
        self.1.notify_all();
    }

    fn look(&self) -> bool {
        let mut mailbox = self.0.lock().expect("subscription mailbox poisoned");
        mailbox.look = true;
        self.1.notify_all();
        !mailbox.closed
    }
}

/// Serve `read` as `run` does, read on the puller's thread. A thread of this
/// process only waits for the next reason to read and calls `wake`.
fn start_pull<A, T, F, Fut>(
    args: A,
    wakes: Wake,
    read: F,
    wake: Box<dyn Fn() + Send + Sync>,
    select: Option<Vec<String>>,
) -> Result<Subscription>
where
    A: DeserializeOwned + Clone + Send + 'static,
    T: Serialize,
    F: FnMut(Option<SharedStore>, A) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T>>,
{
    let shared = mailbox();
    let inbox: Arc<dyn Inbox> = shared.clone();
    let mut reader = Reader::new(shared.clone(), None, args, wakes, read, select)?;
    lock(&shared).pull_at = Some(Instant::now());
    let waker = shared.clone();
    std::thread::spawn(move || {
        let mut mailbox = lock(&waker);
        while !mailbox.closed {
            mailbox = match mailbox.pull_at {
                Some(at) if mailbox.asked() || Instant::now() >= at => {
                    mailbox.pull_at = None;
                    drop(mailbox);
                    wake();
                    lock(&waker)
                }
                Some(at) => {
                    let wait = at.saturating_duration_since(Instant::now());
                    waker
                        .1
                        .wait_timeout(mailbox, wait)
                        .expect("subscription mailbox poisoned")
                        .0
                }
                None => waker
                    .1
                    .wait(mailbox)
                    .expect("subscription mailbox poisoned"),
            };
        }
    });
    let ended = Arc::new(AtomicBool::new(false));
    let failed = ended.clone();
    let pull = move || -> Option<Vec<u8>> {
        if failed.load(Ordering::Acquire) {
            return None;
        }
        let stepped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| reader.step()));
        let error = match stepped {
            Ok(Ok(step)) => {
                lock(&shared).pull_at = Some(step.wake);
                shared.1.notify_all();
                return step.frame;
            }
            Ok(Err(error)) => format!("{error:#}"),
            Err(panic) => format!("subscription panicked: {}", panic_message(&panic)),
        };
        failed.store(true, Ordering::Release);
        encode(&SubscriptionFrame::<()> {
            answers: None,
            unavailable: Some(error),
            body: None,
        })
        .ok()
    };
    Ok(Subscription {
        inbox,
        pull: Mutex::new(Box::new(pull)),
        ended,
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
            pull_at: None,
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
            .wait_timeout_while(mailbox, wait, |mailbox| !mailbox.asked() && !mailbox.closed)
            .expect("subscription mailbox poisoned"),
    );
}

/// What one step of a subscription did.
struct Step {
    /// The frame to send, when the result changed or a request was answered.
    frame: Option<Vec<u8>>,
    /// The subscription has answered what it was asked and ends.
    closing: bool,
    /// When to step again unless the mailbox has a reason sooner.
    wake: Instant,
}

/// One subscription's reading state: what it last read and sent, and when.
struct Reader<A, F> {
    shared: Shared<A>,
    database: PathBuf,
    wakes: Wake,
    read: F,
    args: A,
    runtime: Option<tokio::runtime::Runtime>,
    checkouts: checkouts::Checkouts,
    configuration: observation::Files,
    configuration_checked: Option<Instant>,
    followed_at: Option<i64>,
    stale: bool,
    store: Option<SharedStore>,
    // Why the store that exists cannot be opened; a refused store is not an empty one.
    refused: Option<String>,
    answers: Option<u64>,
    force: bool,
    read_at: Option<StoreRevisions>,
    read_on: Option<Instant>,
    /// When the clock last read or probed the facts outside the store.
    clocked_on: Option<Instant>,
    /// What the probe observed just before the last reading.
    probed: Option<Vec<u8>>,
    cost: Duration,
    failures: u32,
    sent: Option<[u8; 32]>,
    /// The dotted paths each body keeps; none keeps it whole.
    select: Option<Vec<String>>,
}

// A served subscription may be cancelled while its async owner is shutting
// down. Its private runtime has finished its blocking reads and must not wait
// for runtime shutdown from that async worker.
impl<A, F> Drop for Reader<A, F> {
    fn drop(&mut self) {
        if let Some(runtime) = self.runtime.take() {
            runtime.shutdown_background();
        }
    }
}

impl<A, T, F, Fut> Reader<A, F>
where
    A: DeserializeOwned + Clone + Send + 'static,
    T: Serialize,
    F: FnMut(Option<SharedStore>, A) -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    /// Start observing the store for `shared`'s subscription, closing it when
    /// `parent` (when it follows one) exits.
    fn new(
        shared: Shared<A>,
        parent: Option<u32>,
        args: A,
        wakes: Wake,
        read: F,
        select: Option<Vec<String>>,
    ) -> Result<Self> {
        let database = crate::store::database_path_from_env()?;
        let inbox: Arc<dyn Inbox> = shared.clone();
        follow_store(&database, &inbox, parent);
        Ok(Self {
            shared,
            database,
            wakes,
            read,
            args,
            select,
            runtime: Some(tokio::runtime::Runtime::new()?),
            checkouts: checkouts::Checkouts::default(),
            configuration: observation::Files::default(),
            configuration_checked: None,
            followed_at: None,
            stale: false,
            store: None,
            refused: None,
            answers: None,
            force: true,
            read_at: None,
            read_on: None,
            clocked_on: None,
            probed: None,
            cost: Duration::ZERO,
            failures: 0,
            sent: None,
        })
    }

    /// Take requests, read when the result may have changed, and say what to
    /// send and when to step again.
    fn step(&mut self) -> Result<Step> {
        // A closed subscription still answers what it was asked, then stops.
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
                .as_ref()
                .expect("the reader owns its runtime until drop")
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
        if self.wakes.checkouts {
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
        if self
            .configuration_checked
            .is_none_or(|at| now - at >= CHECK)
        {
            self.stale |= self.configuration.changed();
            self.configuration_checked = Some(now);
        }
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
                            && (changed || self.stale || self.clock_moved(now)))
                }
            };
        let mut frame = None;
        if due {
            if !self.force && (self.stale || (self.read_at.is_some() && self.read_at != revisions))
            {
                settle(&self.shared);
            }
            // The first revision query pins the snapshot the whole reading uses.
            let store = self.store.clone();
            let snapshot = store
                .as_ref()
                .map(|store| store.sqlite.read_snapshot())
                .transpose()?;
            let revisions = self
                .store
                .as_ref()
                .map(|store| store.sqlite.revisions())
                .transpose()?;
            let began = Instant::now();
            let ((probed, reading), configuration) = observation::Files::capture(|| {
                let probed = self.probe();
                let reading = match &self.refused {
                    Some(reason) => Err(anyhow::anyhow!("Loopflow store cannot be read: {reason}")),
                    None => crate::git::retain_reads(self.wakes.checkouts, || {
                        self.runtime
                            .as_ref()
                            .expect("the reader owns its runtime until drop")
                            .block_on((self.read)(self.store.clone(), self.args.clone()))
                    }),
                };
                (probed, reading)
            });
            self.probed = probed;
            self.configuration = configuration;
            self.configuration_checked = Some(began);
            let (unavailable, body) = match reading.and_then(|body| {
                let body = serde_json::to_value(body)?;
                Ok(match &self.select {
                    Some(paths) => crate::engine::select::select(&body, paths),
                    None => body,
                })
            }) {
                Ok(body) => (None, Some(body)),
                Err(error) => (Some(format!("{error:#}")), None),
            };
            drop(snapshot);
            self.cost = began.elapsed();
            let identity = identity(&unavailable, &body)?;
            if self.force || self.sent != Some(identity) {
                frame = Some(encode(&SubscriptionFrame {
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
            self.clocked_on = Some(began);
            self.force = false;
            self.stale = false;
        }
        let read_on = self.read_on.expect("a subscription reads before it waits");
        let rest_end = read_on + self.cost.saturating_mul(BUSY_SHARE);
        let wake = if rest_end > now {
            rest_end
        } else {
            self.clocked_on.expect("a reading sets the clock") + self.wakes.clock
        };
        Ok(Step {
            frame,
            closing,
            wake,
        })
    }
}

impl<A, F> Reader<A, F> {
    /// Whether the clock says facts outside the store may have moved: its
    /// period has passed and, for a probed read, the probe's answer differs.
    fn clock_moved(&mut self, now: Instant) -> bool {
        let clocked_on = self.clocked_on.expect("a reading sets the clock");
        if now - clocked_on < self.wakes.clock {
            return false;
        }
        self.clocked_on = Some(now);
        match self.probe() {
            Some(observed) => self.probed.as_ref() != Some(&observed),
            None => true,
        }
    }

    /// The probe's answer now; `None` without a probe, a store or an answer.
    fn probe(&self) -> Option<Vec<u8>> {
        let probe = self.wakes.probe?;
        let store = self.store.as_ref()?;
        match probe(&store.sqlite) {
            Ok(observed) => Some(observed),
            Err(error) => {
                tracing::debug!(%error, "subscription probe unavailable");
                None
            }
        }
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

fn identity(unavailable: &Option<String>, body: &Option<serde_json::Value>) -> Result<[u8; 32]> {
    let mut value = body.clone().unwrap_or(serde_json::Value::Null);
    strip_reading_time(&mut value);
    // Streamed: a large reading is never held a third time as bytes.
    let mut hash = Sha256::new();
    serde_json::to_writer(&mut hash, unavailable)?;
    serde_json::to_writer(&mut hash, &value)?;
    Ok(hash.finalize().into())
}

fn encode<T: Serialize>(frame: &SubscriptionFrame<'_, T>) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(frame)?;
    if bytes.len() > MAX_FRAME {
        bytes = serde_json::to_vec(&SubscriptionFrame::<()> {
            answers: frame.answers,
            unavailable: Some("subscription frame exceeds the 64 MiB transport limit".into()),
            body: None,
        })?;
    }
    bytes.push(b'\n');
    Ok(bytes)
}

/// Requests arrive one JSON line each on stdin. Its end closes the subscription.
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
            Err(_) => mailbox.error = Some("subscription stdin failed".into()),
        }
        let done = mailbox.closed || mailbox.error.is_some();
        shared.1.notify_all();
        if done {
            break;
        }
    }
}

/// Take one request line, or the error that ends the subscription.
fn accept<A: DeserializeOwned>(mailbox: &mut Mailbox<A>, line: &[u8]) {
    if line.len() > MAX_REQUEST {
        mailbox.error = Some("subscription request exceeds 16 KiB".into());
        return;
    }
    match serde_json::from_slice(line) {
        Ok(request) => mailbox.requests.push(request),
        Err(error) => mailbox.error = Some(format!("invalid subscription request: {error}")),
    }
}

/// The subscriptions of one store in this process, and the parent each follows.
type Followers = Vec<(Weak<dyn Inbox>, Option<u32>)>;

/// Every subscription of a store in this process shares one observer of it, so an
/// app serving many subscriptions follows its store once.
static OBSERVERS: OnceLock<Mutex<HashMap<PathBuf, Followers>>> = OnceLock::new();

fn observers() -> MutexGuard<'static, HashMap<PathBuf, Followers>> {
    OBSERVERS
        .get_or_init(Default::default)
        .lock()
        .unwrap_or_else(|poison| poison.into_inner())
}

/// Wake `inbox` when `database` may have changed, and close it when `parent`
/// (when it follows one) exits.
fn follow_store(database: &Path, inbox: &Arc<dyn Inbox>, parent: Option<u32>) {
    let mut observers = observers();
    let followers = observers.entry(database.to_owned()).or_default();
    followers.push((Arc::downgrade(inbox), parent));
    if followers.len() == 1 {
        let database = database.to_owned();
        std::thread::spawn(move || observe(&database));
    }
}

/// Wake each subscription of `database` when the store may have changed, and close
/// one whose parent exited: a descendant that inherited the pipe can hold
/// stdin open long after the parent is gone. Ends with its last subscription.
fn observe(database: &Path) {
    let mut store = StoreChanges::watch(database);
    loop {
        store.wait(CHECK);
        let current = parent_process();
        let mut observers = observers();
        let followers = observers
            .get_mut(database)
            .expect("a store's followers outlive its observer");
        followers.retain(|(inbox, parent)| {
            let Some(inbox) = inbox.upgrade() else {
                return false;
            };
            // A subscription served in-process follows no parent.
            if parent.is_some() && *parent != current {
                inbox.close();
                return false;
            }
            inbox.look()
        });
        if followers.is_empty() {
            observers.remove(database);
            return;
        }
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
/// they belong to; a change wakes the subscription.
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
    checkouts.subscribe(paths, move || {
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
            .expect("subscription mailbox poisoned");
        let again = std::mem::take(&mut mailbox.look);
        if !again || first.elapsed() >= BURST {
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{mpsc, Arc};
    use std::time::{Duration, Instant};

    use super::{changed, identity, retry_due, start_pull, Subscription, Wake, PLANNING_CLOCK};
    use crate::store::sqlite::StoreRevisions;

    fn next_frame(subscription: &Subscription, wakes: &mpsc::Receiver<()>) -> serde_json::Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            wakes
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap();
            if let Some(frame) = subscription.pull() {
                return serde_json::from_slice(&frame).unwrap();
            }
        }
    }

    #[test]
    fn subscription_git_retention_does_not_escape_readings_or_reset_for_new_subscriptions() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = loopflow_test_support::TestRepo::new();
        let subscribe = || {
            let (sender, wakes) = mpsc::channel();
            let directory = repo.path().to_owned();
            let subscription = start_pull(
                (),
                Wake::following_checkouts(PLANNING_CLOCK),
                move |_, ()| {
                    let directory = directory.clone();
                    async move {
                        Ok(crate::git::git_stdout(
                            &directory,
                            &["status", "--porcelain"],
                        )?)
                    }
                },
                Box::new(move || {
                    let _ = sender.send(());
                }),
                None,
            )
            .unwrap();
            (subscription, wakes)
        };
        let (first, wakes) = subscribe();
        assert_eq!(next_frame(&first, &wakes)["body"], "");
        std::fs::write(repo.path().join("new.txt"), "text").unwrap();
        let seen = crate::journal::request::isolate(|| {
            crate::git::git_stdout(repo.path(), &["status", "--porcelain"]).unwrap()
        });
        assert!(seen.contains("new.txt"), "{seen:?}");
        // No store registers this checkout, so filesystem observation cannot
        // refresh the answer before the second subscription reads it.
        let (second, wakes) = subscribe();
        assert_eq!(next_frame(&second, &wakes)["body"], "");
    }

    #[test]
    fn configuration_changes_reach_an_existing_subscription_without_store_writes() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = loopflow_test_support::TestRepo::new();
        let checkout = home.path().join("task");
        assert!(std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["worktree", "add", "-b", "task"])
            .arg(&checkout)
            .output()
            .unwrap()
            .status
            .success());
        std::fs::create_dir_all(checkout.join(".lf")).unwrap();
        std::fs::write(checkout.join(".lf/config.yaml"), "agent: claude\n").unwrap();

        let (sender, wakes) = mpsc::channel();
        let reads = Arc::new(AtomicUsize::new(0));
        let counted = reads.clone();
        let directory = checkout.clone();
        let subscription = start_pull(
            (),
            Wake::every(PLANNING_CLOCK),
            move |_, ()| {
                counted.fetch_add(1, Ordering::Relaxed);
                let directory = directory.clone();
                async move {
                    // Task summaries resolve configuration in bounded workers.
                    std::thread::scope(|scope| {
                        scope
                            .spawn(crate::config::observation::inherit(|| {
                                Ok(crate::config::load_wave_config(
                                    Some(&directory),
                                    Some("product"),
                                )?
                                .unwrap()
                                .agent()
                                .unwrap()
                                .to_owned())
                            }))
                            .join()
                            .unwrap()
                    })
                }
            },
            Box::new(move || {
                let _ = sender.send(());
            }),
            None,
        )
        .unwrap();
        let agent = |expected: &str| {
            let frame = next_frame(&subscription, &wakes);
            assert!(frame["unavailable"].is_null(), "{frame}");
            assert_eq!(frame["body"], expected);
        };
        agent("claude");

        crate::engine::config::set(
            &crate::config::files::Destination::from_cli(&checkout, None, true, false).unwrap(),
            "agent",
            "codex",
        )
        .unwrap();
        agent("codex");
        crate::engine::config::set(
            &crate::config::files::Destination::from_cli(&checkout, None, false, false).unwrap(),
            "agent",
            "pi",
        )
        .unwrap();
        agent("pi");
        let private = crate::config::files::private_path(&checkout).unwrap();
        assert_eq!(
            private,
            crate::config::files::private_path(repo.path()).unwrap()
        );
        // An external editor's atomic replacement, then removal: neither writes SQLite.
        let replacement = private.with_extension("tmp");
        std::fs::write(&replacement, "agent: opencode\n").unwrap();
        std::fs::rename(&replacement, &private).unwrap();
        agent("opencode");
        std::fs::remove_file(&private).unwrap();
        agent("codex");
        std::fs::remove_file(home.path().join("config.yaml")).unwrap();
        agent("claude");

        // The Wave directory did not exist when observation started.
        let wave = checkout.join(".lf/wave/product/config.yaml");
        std::fs::create_dir_all(wave.parent().unwrap()).unwrap();
        std::fs::write(&wave, "agent: pi\n").unwrap();
        agent("pi");
        std::fs::write(&wave, "agent: [\n").unwrap();
        assert!(!next_frame(&subscription, &wakes)["unavailable"].is_null());
        std::fs::write(&wave, "agent: codex\n").unwrap();
        agent("codex");

        let before = reads.load(Ordering::Relaxed);
        for index in 0..20 {
            std::fs::write(&wave, format!("agent: pi # edit {index}\n")).unwrap();
        }
        agent("pi");
        assert_eq!(reads.load(Ordering::Relaxed), before + 1);

        let before = reads.load(Ordering::Relaxed);
        std::fs::write(checkout.join("unrelated.txt"), "changed\n").unwrap();
        let until = Instant::now() + Duration::from_millis(1200);
        while Instant::now() < until {
            wakes.recv_timeout(Duration::from_secs(2)).unwrap();
            assert!(subscription.pull().is_none());
        }
        assert_eq!(reads.load(Ordering::Relaxed), before);
        assert!(!home.path().join("loopflow.db").exists());
    }

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
    fn new_and_edited_skill_overrides_reach_an_existing_subscription() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::lf::commands::graph::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = tempfile::tempdir().unwrap();
        let (sender, wakes) = mpsc::channel();
        let directory = repo.path().to_owned();
        let personal = home.path().to_owned();
        let subscription = start_pull(
            (),
            Wake::every(PLANNING_CLOCK),
            move |_, ()| {
                let (directory, personal) = (directory.clone(), personal.clone());
                async move {
                    Ok(crate::skills::catalog::SkillCatalog::load(
                        Some(&directory),
                        Some(&personal),
                        false,
                    )?
                    .load_skill("implement")?
                    .agent)
                }
            },
            Box::new(move || {
                let _ = sender.send(());
            }),
            None,
        )
        .unwrap();
        let agent = |expected: serde_json::Value| {
            let frame = next_frame(&subscription, &wakes);
            assert!(frame["unavailable"].is_null(), "{frame}");
            assert_eq!(frame["body"], expected);
        };
        agent(serde_json::Value::Null);
        let path = repo.path().join(".lf/skills/implement/SKILL.md");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "---\nagent: codex\n---\nImplement.").unwrap();
        agent("codex".into());
        std::fs::write(&path, "---\nagent: claude\n---\nImplement.").unwrap();
        agent("claude".into());
        std::fs::remove_file(&path).unwrap();
        agent(serde_json::Value::Null);
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
