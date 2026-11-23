//! `--subscribe` on an ordinary read: the command a client runs once is the one it
//! streams. Each frame is one NDJSON line whose `body` is what `--json` prints,
//! sent first and then each time that result changes.
//!
//! Store revisions and the external facts captured by a reading determine when
//! it runs again. Observations are shared in memory; selected answers are sent
//! only when they change. Database joins use one pinned read snapshot.
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
pub mod events;

#[cfg(any(test, feature = "test-support"))]
use crate::performance::subscription_measurement::{Runtime, Stage};
#[cfg(not(any(test, feature = "test-support")))]
use tokio::runtime::Runtime;

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

use crate::machine::wire::SubscriptionEvent;
use crate::observe::{Cache, Dependencies};
use crate::store::{SharedStore, StorageConfig};
use loopflow_storage::changes::StoreChanges;
use loopflow_storage::sqlite::StoreRevisions;

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

/// This invocation's `--select`, which a subscription on another Machine sends.
pub fn selected() -> Option<Vec<String>> {
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
    pub work: Option<loopflow_storage::instrumentation::SqlReceipt>,
    /// Every store domain's revision in the snapshot this reading used; null
    /// without a store. A write whose answered revisions this reaches shows here.
    pub read_at: Option<loopflow_storage::sqlite::Revisions>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Request<A> {
    id: u64,
    args: Option<A>,
}

/// The store a read needs, or why the subscription has none to read.
pub fn require_store(store: Option<SharedStore>) -> Result<SharedStore> {
    store.ok_or_else(|| anyhow::anyhow!("no Loopflow store exists on this Machine"))
}

/// What a read names beside the store's state. History moves with every
/// turn, so a commit to `history.db` reads again only the subscriptions that
/// name `history.*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Watch {
    /// Observed files only; the reading needs no Work database.
    Files,
    /// State in `loopflow.db` only.
    State,
    /// State, Session history and turns.
    History,
}

/// Whether a commit that moved revisions from `old` to `new` can change a
/// result of a read that watches `watch`.
fn changed(watch: Watch, old: StoreRevisions, new: StoreRevisions) -> bool {
    match watch {
        Watch::Files => false,
        Watch::History => old != new,
        Watch::State => {
            StoreRevisions { history: 0, ..old } != StoreRevisions { history: 0, ..new }
        }
    }
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
    /// Whether the subscription should step before its next scheduled check.
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
pub fn run<A, T, F, Fut>(watch: Watch, args: A, read: F) -> Result<()>
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
    let mut reader = Reader::new(shared.clone(), parent, watch, args, read, selected())?;
    let mut stdout = std::io::stdout().lock();
    loop {
        let step = reader.step()?;
        if let Some(frame) = step.update.frame {
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
        /// Collect Task occurrences before selection, alongside the same snapshot.
        events: bool,
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
pub fn start<A, T, F, Fut>(
    watch: Watch,
    args: A,
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
        Transport::Stdio => run(watch, args, read).map(|()| None),
        Transport::Pull {
            wake,
            select,
            events,
        } => start_pull(watch, args, read, wake, select, events).map(Some),
    }
}

/// A subscription served inside this process. It takes checkpoints, never argument
/// changes: a client with new arguments subscribes again. [`Subscription::pull`] reads on
/// the caller's thread and returns the reading with its occurrences.
/// Dropping it ends the subscription.
pub struct Subscription {
    inbox: Arc<dyn Inbox>,
    pull: Mutex<Box<dyn FnMut() -> SubscriptionUpdate + Send>>,
    ended: Arc<AtomicBool>,
}

/// A reading and its occurrences leave the pinned snapshot together.
#[derive(Debug, Default)]
pub struct SubscriptionUpdate {
    pub frame: Option<Vec<u8>>,
    pub events: Vec<SubscriptionEvent>,
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

    /// The newest changed frame and observed occurrences; either can be empty.
    /// Any read snapshot is released before this returns. A subscription that
    /// fails or panics returns its error as a last frame.
    pub fn pull(&self) -> SubscriptionUpdate {
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
    watch: Watch,
    args: A,
    read: F,
    wake: Box<dyn Fn() + Send + Sync>,
    select: Option<Vec<String>>,
    events: bool,
) -> Result<Subscription>
where
    A: DeserializeOwned + Clone + Send + 'static,
    T: Serialize,
    F: FnMut(Option<SharedStore>, A) -> Fut + Send + 'static,
    Fut: std::future::Future<Output = Result<T>>,
{
    let shared = mailbox();
    let inbox: Arc<dyn Inbox> = shared.clone();
    let mut reader = Reader::new(shared.clone(), None, watch, args, read, select)?;
    reader.observer = events.then(crate::read_stream::events::Observer::default);
    lock(&shared).pull_at = Some(Instant::now());
    let waker = shared.clone();
    std::thread::spawn(move || {
        #[cfg(any(test, feature = "test-support"))]
        let _waker = Stage::new("wake_thread_lifetime");
        let mut mailbox = lock(&waker);
        while !mailbox.closed {
            mailbox = match mailbox.pull_at {
                Some(at) if mailbox.asked() || Instant::now() >= at => {
                    mailbox.pull_at = None;
                    drop(mailbox);
                    #[cfg(any(test, feature = "test-support"))]
                    let _wake = Stage::new("wake_callback");
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
    let pull = move || -> SubscriptionUpdate {
        if failed.load(Ordering::Acquire) {
            return SubscriptionUpdate::default();
        }
        let stepped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| reader.step()));
        let error = match stepped {
            Ok(Ok(step)) => {
                lock(&shared).pull_at = Some(step.wake);
                shared.1.notify_all();
                return step.update;
            }
            Ok(Err(error)) => format!("{error:#}"),
            Err(panic) => format!("subscription panicked: {}", panic_message(&panic)),
        };
        failed.store(true, Ordering::Release);
        SubscriptionUpdate {
            frame: encode(&SubscriptionFrame::<()> {
                answers: None,
                unavailable: Some(error),
                body: None,
                work: None,
                read_at: None,
            })
            .ok(),
            events: Vec::new(),
        }
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
    update: SubscriptionUpdate,
    /// The subscription has answered what it was asked and ends.
    closing: bool,
    /// When to step again unless the mailbox has a reason sooner.
    wake: Instant,
}

fn reader_runtime() -> std::io::Result<Runtime> {
    let mut builder = tokio::runtime::Builder::new_multi_thread();
    builder.enable_all();
    #[cfg(any(test, feature = "test-support"))]
    return Runtime::build(&mut builder);
    #[cfg(not(any(test, feature = "test-support")))]
    builder.build()
}

/// One subscription's reading state: what it last read and sent, and when.
struct Reader<A, F> {
    shared: Shared<A>,
    database: PathBuf,
    read: F,
    watch: Watch,
    args: A,
    runtime: Runtime,
    observations: Arc<Cache>,
    dependencies: Dependencies,
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
    /// The dotted paths each body keeps; none keeps it whole.
    select: Option<Vec<String>>,
    observer: Option<crate::read_stream::events::Observer>,
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
        watch: Watch,
        args: A,
        read: F,
        select: Option<Vec<String>>,
    ) -> Result<Self> {
        let database = crate::store::database_path_from_env()?;
        let inbox: Arc<dyn Inbox> = shared.clone();
        // Stdio uses the same observer to notice its parent exiting.
        if watch != Watch::Files || parent.is_some() {
            follow_store(&database, &inbox, parent);
        }
        Ok(Self {
            shared,
            database,
            read,
            watch,
            args,
            select,
            observer: None,
            runtime: reader_runtime()?,
            observations: Cache::shared(),
            dependencies: Dependencies::default(),
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
        #[cfg(any(test, feature = "test-support"))]
        let _step = Stage::new("reader_step");
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
        if self.watch != Watch::Files && self.store.is_none() && self.database.exists() {
            #[cfg(any(test, feature = "test-support"))]
            let _open = Stage::new("reader_store_open");
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
        let now = Instant::now();
        let observed_changes = self.dependencies.changed();
        let due = self.force
            || match self.read_on {
                None => true,
                Some(read_on) => {
                    let changed = match (self.read_at, revisions) {
                        (Some(old), Some(new)) => changed(self.watch, old, new),
                        (None, None) => false,
                        _ => true,
                    };
                    retry_due(self.failures, read_on, now)
                        || (now - read_on >= self.cost.saturating_mul(BUSY_SHARE)
                            && (changed || observed_changes))
                }
            };
        let mut update = SubscriptionUpdate::default();
        if due {
            #[cfg(any(test, feature = "test-support"))]
            let _projection = Stage::new("projection");
            if !self.force
                && (observed_changes || (self.read_at.is_some() && self.read_at != revisions))
            {
                settle(&self.shared);
            }
            // The first revision query pins the snapshot the whole reading uses.
            let store = self.store.clone();
            let snapshot = store
                .as_ref()
                .map(|store| store.sqlite.read_snapshot())
                .transpose()?;
            let read_at = self
                .store
                .as_ref()
                .map(|store| store.sqlite.revision_map())
                .transpose()?;
            let revisions = read_at.as_ref().map(|map| map.to_store_revisions());
            let began = Instant::now();
            let (reading, dependencies) = self.observations.capture(|| match &self.refused {
                Some(reason) => Err(anyhow::anyhow!("Loopflow store cannot be read: {reason}")),
                None => self
                    .runtime
                    .block_on((self.read)(self.store.clone(), self.args.clone())),
            });
            self.dependencies = dependencies;
            let reading = reading.and_then(|body| Ok(serde_json::to_value(body)?));
            if let (Some(observer), Some(store), Ok(body)) =
                (&mut self.observer, &self.store, &reading)
            {
                update.events = observer.read(&store.sqlite, body)?;
            }
            let (unavailable, body) = match reading.map(|body| match &self.select {
                Some(paths) => crate::engine::select::select(&body, paths),
                None => body,
            }) {
                Ok(body) => (None, Some(body)),
                Err(error) => (Some(format!("{error:#}")), None),
            };
            drop(snapshot);
            self.cost = began.elapsed();
            let identity = identity(unavailable.as_deref(), body.as_ref())?;
            if self.force || self.sent != Some(identity) {
                update.frame = Some(encode(&SubscriptionFrame {
                    answers: self.answers,
                    unavailable,
                    body: body.as_ref(),
                    work: None,
                    read_at,
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
        }
        let read_on = self.read_on.expect("a subscription reads before it waits");
        let rest_end = read_on + self.cost.saturating_mul(BUSY_SHARE);
        let wake = if rest_end > now {
            rest_end
        } else {
            now + CHECK
        };
        Ok(Step {
            update,
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
            // A Task condition restates when it was derived; tracker state and
            // Project readiness restate when the tracker was last read.
            for (observation, time) in [
                ("condition", "observed_at"),
                ("tracker_state", "synced_at"),
                ("project_readiness", "observed_at"),
            ] {
                if let Some(serde_json::Value::Object(observed)) = fields.get_mut(observation) {
                    observed.remove(time);
                }
            }
            fields.values_mut().for_each(strip_reading_time);
        }
        serde_json::Value::Array(items) => items.iter_mut().for_each(strip_reading_time),
        _ => {}
    }
}

pub fn identity(unavailable: Option<&str>, body: Option<&serde_json::Value>) -> Result<[u8; 32]> {
    let mut value = body.cloned().unwrap_or(serde_json::Value::Null);
    strip_reading_time(&mut value);
    // Streamed: a large reading is never held a third time as bytes.
    let mut hash = Sha256::new();
    serde_json::to_writer(&mut hash, &unavailable)?;
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
            work: frame.work.clone(),
            read_at: frame.read_at.clone(),
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
    #[cfg(any(test, feature = "test-support"))]
    let _observer = Stage::new("observer_lifetime");
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

    use super::{changed, identity, retry_due, start_pull, Subscription, Watch};
    use loopflow_storage::sqlite::StoreRevisions;

    fn next_frame(subscription: &Subscription, wakes: &mpsc::Receiver<()>) -> serde_json::Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            wakes
                .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                .unwrap();
            if let Some(frame) = subscription.pull().frame {
                return serde_json::from_slice(&frame).unwrap();
            }
        }
    }

    #[test]
    fn git_changes_reach_shared_subscriptions_without_registered_checkouts() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::config::environment::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = loopflow_test_support::TestRepo::new();
        let subscribe = || {
            let (sender, wakes) = mpsc::channel();
            let directory = repo.path().to_owned();
            let subscription = start_pull(
                Watch::State,
                (),
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
                false,
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
        let (second, second_wakes) = subscribe();
        let initial = next_frame(&second, &second_wakes);
        let changed = next_frame(&first, &wakes);
        assert!(changed["body"].as_str().unwrap().contains("new.txt"));
        if initial["body"] != changed["body"] {
            assert_eq!(next_frame(&second, &second_wakes)["body"], changed["body"]);
        }
        assert!(!home.path().join("loopflow.db").exists());
    }

    #[test]
    fn configuration_changes_reach_an_existing_subscription_without_store_writes() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::config::environment::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
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
            Watch::State,
            (),
            move |_, ()| {
                counted.fetch_add(1, Ordering::Relaxed);
                let directory = directory.clone();
                async move {
                    // Task summaries resolve configuration in bounded workers.
                    std::thread::scope(|scope| {
                        scope
                            .spawn(crate::observe::inherit(|| {
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
            false,
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
        std::fs::create_dir_all(private.parent().unwrap()).unwrap();
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
        agent("claude");
        std::fs::write(&wave, "agent: codex\n").unwrap();
        agent("codex");

        let before = reads.load(Ordering::Relaxed);
        std::fs::write(checkout.join("unrelated.txt"), "changed\n").unwrap();
        let until = Instant::now() + Duration::from_millis(1200);
        while Instant::now() < until {
            wakes.recv_timeout(Duration::from_secs(2)).unwrap();
            assert!(subscription.pull().frame.is_none());
        }
        assert_eq!(reads.load(Ordering::Relaxed), before);
        assert!(!home.path().join("loopflow.db").exists());
    }

    #[test]
    fn readings_share_resolved_settings_until_a_settings_file_changes() {
        use crate::observe::{Cache, Derived};
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::config::environment::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = tempfile::tempdir().unwrap();
        let shared = repo.path().join(".lf/config.yaml");
        std::fs::create_dir_all(shared.parent().unwrap()).unwrap();
        std::fs::write(&shared, "agent: claude\n").unwrap();
        let agent = {
            let directory = repo.path().to_owned();
            move || -> Result<String, crate::error::LoadError> {
                let config = crate::config::load_wave_config(Some(&directory), None)?;
                Ok(config.expect("resolved").agent()?.to_owned())
            }
        };

        let cache = Cache::shared();
        let resolutions = || {
            cache.computations(|key| {
                matches!(key, Derived::Settings { checkout: Some(path), .. } if path == repo.path())
            })
        };
        let (sender, wakes) = mpsc::channel();
        let read = agent.clone();
        let subscription = start_pull(
            Watch::State,
            (),
            move |_, ()| {
                let agent = read().map_err(anyhow::Error::from);
                async move { agent }
            },
            Box::new(move || {
                let _ = sender.send(());
            }),
            None,
            false,
        )
        .unwrap();
        let mut checkpoint = 0;
        let mut forced = |expected: &str| {
            checkpoint += 1;
            subscription.checkpoint(checkpoint);
            let frame = next_frame(&subscription, &wakes);
            assert_eq!(frame["answers"], checkpoint, "{frame}");
            assert_eq!(frame["body"], expected, "{frame}");
        };

        assert_eq!(next_frame(&subscription, &wakes)["body"], "claude");
        for _ in 0..3 {
            forced("claude");
        }
        assert_eq!(resolutions(), 1, "four readings of unchanged files");

        std::fs::write(&shared, "agent: pi\n").unwrap();
        assert_eq!(next_frame(&subscription, &wakes)["body"], "pi");
        forced("pi");
        assert_eq!(resolutions(), 2, "one edit resolves once more");

        // A file that does not parse reads as it does for a bare command.
        std::fs::write(&shared, "agent: [\n").unwrap();
        let bare = agent().unwrap();
        assert_eq!(bare, "codex", "the only installed harness");
        assert_eq!(next_frame(&subscription, &wakes)["body"], bare);
        std::fs::write(&shared, "agent: pi\n").unwrap();
        assert_eq!(next_frame(&subscription, &wakes)["body"], "pi");
        forced("pi");
        let resolved = resolutions();
        assert_eq!(resolved, 4);

        // An override belongs to its own reading and never enters the shared value.
        let overridden = crate::config::settings::select_invocation(
            serde_yaml_ng::from_str("agent: claude").unwrap(),
        );
        forced("claude");
        drop(overridden);
        forced("pi");
        assert_eq!(resolutions(), resolved);

        // Another home read through the same cache has its own settings.
        let other = tempfile::tempdir().unwrap();
        std::fs::write(other.path().join("config.yaml"), "agent: opencode\n").unwrap();
        let other_home =
            crate::config::environment::EnvVarGuard::set("LF_HOME", other.path().to_str().unwrap());
        forced("opencode");
        drop(other_home);
        forced("pi");
        assert_eq!(resolutions(), resolved + 1);
    }

    #[test]
    fn personal_settings_reach_a_task_whose_worktree_appears_after_a_reading() {
        use crate::observe::Cache;
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::config::environment::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        // No checked-in `.lf/config.yaml`: the personal file is the only layer.
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(crate::config::files::PRIVATE_FILE, "agent: pi\n");
        let checkout = home.path().join("task");
        let agent = || {
            let config = crate::config::load_wave_config(Some(&checkout), None).unwrap();
            config.expect("resolved").agent().unwrap().to_owned()
        };

        let cache = Arc::new(Cache::default());
        let (before, dependencies) = cache.capture(agent);
        assert_eq!(before, "codex", "the only installed harness");

        assert!(std::process::Command::new("git")
            .arg("-C")
            .arg(repo.path())
            .args(["worktree", "add", "-b", "task"])
            .arg(&checkout)
            .output()
            .unwrap()
            .status
            .success());
        cache.refresh_all();
        assert!(dependencies.changed());
        assert_eq!(cache.capture(agent).0, "pi");
    }

    #[test]
    fn personal_settings_reach_a_task_whose_checkout_is_gone() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::config::environment::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = loopflow_test_support::TestRepo::new();
        repo.create_file(crate::config::files::PRIVATE_FILE, "agent: pi\n");
        let removed = home.path().join("removed");

        let agent =
            crate::ops::task::resolve_task_agent(&removed, repo.path(), None, None).unwrap();

        assert_eq!(agent.harness.as_str(), "pi");
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
            crate::config::environment::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let repo = tempfile::tempdir().unwrap();
        let (sender, wakes) = mpsc::channel();
        let directory = repo.path().to_owned();
        let personal = home.path().to_owned();
        let subscription = start_pull(
            Watch::State,
            (),
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
            false,
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
    fn a_turn_reaches_a_subscription_that_watches_turns() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _home =
            crate::config::environment::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let store =
            crate::store::opening::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
        store.test_session(
            "conversation",
            &loopflow_storage::session_record::new_artifact_key(),
        );
        let (sender, wakes) = mpsc::channel();
        let subscription = start_pull(
            Watch::History,
            (),
            |store, ()| async move {
                let store = super::require_store(store)?;
                Ok(store.sqlite.session_turns("conversation")?.len())
            },
            Box::new(move || {
                let _ = sender.send(());
            }),
            None,
            false,
        )
        .unwrap();
        assert_eq!(next_frame(&subscription, &wakes)["body"], 0);
        store.record_turn_start("conversation", &"thread".into(), "turn", 1);
        assert_eq!(next_frame(&subscription, &wakes)["body"], 1);
    }

    #[test]
    fn history_and_turns_reach_only_the_reads_that_watch_them() {
        let old = StoreRevisions {
            planning: 1,
            sessions: 1,
            graphs: 1,
            processes: 1,
            history: 1,
        };
        let turn = StoreRevisions { history: 9, ..old };
        assert!(!changed(Watch::State, old, turn));
        assert!(changed(Watch::History, old, turn));
        assert!(changed(
            Watch::State,
            old,
            StoreRevisions {
                processes: 2,
                ..old
            }
        ));
    }

    #[test]
    fn the_time_of_a_reading_is_not_a_change() {
        let rows = |at| {
            serde_json::json!([{"id": 1, "condition": {"observed_at": at},
                "task": {"tracker_state": {"state": "available", "synced_at": at}},
                "project_readiness": {"state": "ready", "observed_at": at}}])
        };
        let read = |body| identity(None, Some(&body)).unwrap();
        assert_eq!(read(rows(1)), read(rows(2)));
        assert_ne!(
            read(rows(1)),
            identity(Some("gone"), Some(&rows(1))).unwrap()
        );
    }
}
