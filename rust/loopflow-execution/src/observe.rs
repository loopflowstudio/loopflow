//! External facts shared by served reads. Bare reads use the same observers
//! inline; only a bounded reading scope retains answers and dependencies.
use std::any::Any;
use std::cell::RefCell;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Condvar, LazyLock, Mutex, OnceLock, Weak};
use std::time::{Duration, SystemTime};

use notify::{RecursiveMode, Watcher};

use crate::ops::task::TaskFileInventory;
use crate::process::OsProcess;

static SPAWNS: AtomicU64 = AtomicU64::new(0);

/// Record a subprocess spawned by the server or observation layer.
pub fn record_spawn() {
    SPAWNS.fetch_add(1, Ordering::Relaxed);
}

/// Cumulative subprocesses recorded in this process.
pub fn spawns() -> u64 {
    SPAWNS.load(Ordering::Relaxed)
}

/// Number of observation facts currently held by active subscriptions.
pub fn current_held_facts() -> u64 {
    PROCESS_CACHE.held_facts()
}

/// A subscriber woken whenever a held observation fact's revision advances.
pub(crate) trait FactWatcher: Send + Sync {
    fn wake(&self);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Fact {
    CheckoutFiles(PathBuf),
    Git(PathBuf, Vec<String>),
    Github(PathBuf, Vec<String>),
    File(PathBuf),
    Bytes(PathBuf, usize),
    Directory(PathBuf),
    Metadata(PathBuf),
    Process(u32),
    Deadline(i64),
    Clock,
    /// A conversation's current turn from its transcript's end.
    Turn(
        loopflow_harness::HarnessKind,
        loopflow_harness::transcript::Source,
    ),
    /// Conversations open on this Machine that Loopflow never launched.
    RawConversations,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Failure(io::ErrorKind, String);

fn retained<T>(result: io::Result<T>) -> Result<T, Failure> {
    result.map_err(|error| Failure(error.kind(), error.to_string()))
}
fn restored<T>(result: Result<T, Failure>) -> io::Result<T> {
    result.map_err(|Failure(kind, message)| io::Error::new(kind, message))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileMetadata {
    kind: std::fs::FileType,
    symlink: bool,
}

#[derive(Debug, Clone)]
enum Answer {
    CheckoutFiles(Result<Arc<TaskFileInventory>, Failure>),
    Output(Result<Output, Failure>),
    File(Result<String, Failure>),
    Directory(Result<Vec<PathBuf>, Failure>),
    Bytes(Result<(u64, Vec<u8>), Failure>),
    Metadata(Result<FileMetadata, Failure>),
    Bool(bool),
    Time(i64),
    Process(Result<Option<OsProcess>, Failure>),
    Turn(Option<loopflow_harness::transcript::live::CurrentTurn>),
    Raw(Vec<crate::transcript::raw::RawDirectory>),
}

impl PartialEq for Answer {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Output(Ok(a)), Self::Output(Ok(b))) => {
                a.status == b.status && a.stdout == b.stdout && a.stderr == b.stderr
            }
            (Self::Output(Err(a)), Self::Output(Err(b))) => a == b,
            (Self::CheckoutFiles(a), Self::CheckoutFiles(b)) => a == b,
            (Self::File(a), Self::File(b)) => a == b,
            (Self::Directory(a), Self::Directory(b)) => a == b,
            (Self::Bytes(a), Self::Bytes(b)) => a == b,
            (Self::Metadata(a), Self::Metadata(b)) => a == b,
            (Self::Bool(a), Self::Bool(b)) => a == b,
            (Self::Time(a), Self::Time(b)) => a == b,
            (Self::Process(a), Self::Process(b)) => a == b,
            (Self::Turn(a), Self::Turn(b)) => a == b,
            (Self::Raw(a), Self::Raw(b)) => a == b,
            _ => false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum StatStamp {
    Present {
        mtime: Option<SystemTime>,
        ctime: (i64, i64),
        size: u64,
        dev: u64,
        ino: u64,
        symlink_mtime: Option<SystemTime>,
        symlink_ino: u64,
    },
    Missing(io::ErrorKind),
}

fn stat_metadata(meta: &std::fs::Metadata) -> (Option<SystemTime>, (i64, i64), u64, u64, u64) {
    #[cfg(unix)]
    let (ctime, dev, ino) = {
        use std::os::unix::fs::MetadataExt;
        ((meta.ctime(), meta.ctime_nsec()), meta.dev(), meta.ino())
    };
    #[cfg(not(unix))]
    let (ctime, dev, ino) = ((0, 0), 0, 0);
    (meta.modified().ok(), ctime, meta.len(), dev, ino)
}

fn stat_stamp(path: &Path, include_symlink: bool) -> StatStamp {
    match std::fs::metadata(path) {
        Ok(meta) => {
            let (mtime, ctime, size, dev, ino) = stat_metadata(&meta);
            let (symlink_mtime, symlink_ino) = if include_symlink {
                std::fs::symlink_metadata(path)
                    .map(|s| {
                        let (sm, _, _, _, si) = stat_metadata(&s);
                        (sm, si)
                    })
                    .unwrap_or((None, 0))
            } else {
                (None, 0)
            };
            StatStamp::Present {
                mtime,
                ctime,
                size,
                dev,
                ino,
                symlink_mtime,
                symlink_ino,
            }
        }
        Err(err) => StatStamp::Missing(err.kind()),
    }
}

impl Fact {
    fn stat_stamp(&self) -> Option<StatStamp> {
        match self {
            Self::File(path) | Self::Bytes(path, _) | Self::Directory(path) => {
                Some(stat_stamp(path, false))
            }
            Self::Metadata(path) => Some(stat_stamp(path, true)),
            _ => None,
        }
    }

    fn checkout_root(&self) -> Option<&Path> {
        match self {
            Self::Git(path, _) | Self::CheckoutFiles(path) => Some(path.as_path()),
            _ => None,
        }
    }

    fn interval(&self) -> Duration {
        Duration::from_secs(match self {
            Self::Git(..) | Self::CheckoutFiles(..) => 3,
            Self::Github(..) | Self::Clock => 60,
            // Transcript watches say when to look sooner; these bound a missed event.
            Self::Turn(..) => TURN_RECONCILE.as_secs(),
            Self::RawConversations => 10,
            _ => 1,
        })
    }

    fn observe(&self) -> Answer {
        match self {
            Self::CheckoutFiles(path) => {
                Answer::CheckoutFiles(retained(TaskFileInventory::scan(path).map(Arc::new)))
            }
            Self::Git(path, args) | Self::Github(path, args) => {
                record_spawn();
                let mut command = Command::new(if matches!(self, Self::Git(..)) {
                    "git"
                } else {
                    "gh"
                });
                command.current_dir(path).args(args);
                if matches!(self, Self::Git(..)) {
                    command.env("GIT_OPTIONAL_LOCKS", "0");
                }
                Answer::Output(retained(command.output()))
            }
            Self::File(path) => Answer::File(retained(std::fs::read_to_string(path))),
            Self::Directory(path) => Answer::Directory(retained((|| {
                let mut paths = std::fs::read_dir(path)?
                    .map(|entry| entry.map(|e| e.path()))
                    .collect::<io::Result<Vec<_>>>()?;
                paths.sort();
                Ok(paths)
            })())),
            Self::Bytes(path, limit) => Answer::Bytes(retained((|| {
                let file = std::fs::File::open(path)?;
                let size = file.metadata()?.len();
                let mut bytes = Vec::new();
                file.take(*limit as u64).read_to_end(&mut bytes)?;
                Ok((size, bytes))
            })())),
            Self::Metadata(path) => Answer::Metadata(retained((|| {
                Ok(FileMetadata {
                    kind: std::fs::metadata(path)?.file_type(),
                    symlink: std::fs::symlink_metadata(path)?.file_type().is_symlink(),
                })
            })())),
            Self::Process(pid) => Answer::Process(retained(OsProcess::observe(*pid))),
            Self::Clock => Answer::Time(time::OffsetDateTime::now_utc().unix_timestamp()),
            Self::Deadline(at) => {
                Answer::Bool(time::OffsetDateTime::now_utc().unix_timestamp() >= *at)
            }
            Self::Turn(harness, source) => Answer::Turn(
                loopflow_harness::transcript::live::read_current(*harness, source),
            ),
            Self::RawConversations => Answer::Raw(crate::transcript::raw::discover()),
        }
    }
}

/// How often a watched transcript is read again with no event.
pub(crate) const TURN_RECONCILE: Duration = Duration::from_secs(30);
/// A transcript growing steadily is read again at most this often.
pub(crate) const TURN_HINTED: Duration = Duration::from_secs(1);
/// How long an unheld observation entry stays cached before eviction.
const UNHELD_TTL: Duration = Duration::from_secs(300);

#[derive(Debug)]
struct Entry {
    answer: Option<Answer>,
    stat: Option<StatStamp>,
    revision: u64,
    checked: SystemTime,
    used: SystemTime,
    hinted_due: Option<SystemTime>,
    dirty: bool,
}

impl Entry {
    fn new(answer: Option<Answer>, now: SystemTime) -> Self {
        Self {
            answer,
            stat: None,
            revision: 0,
            checked: now,
            used: now,
            hinted_due: None,
            dirty: false,
        }
    }

    fn update(&mut self, answer: Answer, stat: Option<StatStamp>, now: SystemTime) -> bool {
        let changed = self.answer.as_ref() != Some(&answer);
        if changed {
            self.answer = Some(answer);
            self.revision += 1;
        }
        self.stat = stat;
        self.checked = now;
        changed
    }
}

fn min_time<T: Ord>(slot: &mut Option<T>, candidate: T) {
    *slot = Some(match slot.take() {
        Some(prev) => prev.min(candidate),
        None => candidate,
    });
}

struct FactSlot {
    fact: Fact,
    entry: Mutex<Entry>,
    holders: AtomicUsize,
    watchers: Mutex<Vec<Weak<dyn FactWatcher>>>,
}

impl std::fmt::Debug for FactSlot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FactSlot")
            .field("fact", &self.fact)
            .field("holders", &self.holders.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

impl FactSlot {
    fn new(fact: Fact, entry: Entry) -> Arc<Self> {
        Arc::new(Self {
            fact,
            entry: Mutex::new(entry),
            holders: AtomicUsize::new(0),
            watchers: Mutex::new(Vec::new()),
        })
    }

    fn is_held(&self) -> bool {
        self.holders.load(Ordering::Acquire) > 0
    }

    fn add_watcher(&self, watcher: Weak<dyn FactWatcher>) {
        let mut watchers = self.watchers.lock().expect("fact watchers");
        watchers.retain(|existing| existing.strong_count() > 0);
        if !watchers
            .iter()
            .any(|existing| Weak::ptr_eq(existing, &watcher))
        {
            watchers.push(watcher);
        }
    }

    fn notify_watchers(&self) {
        let targets: Vec<Arc<dyn FactWatcher>> = {
            let mut watchers = self.watchers.lock().expect("fact watchers");
            let mut live = Vec::new();
            watchers.retain(|weak| match weak.upgrade() {
                Some(watcher) => {
                    live.push(watcher);
                    true
                }
                None => false,
            });
            live
        };
        for watcher in targets {
            watcher.wake();
        }
    }
}

type SharedSlot = Arc<FactSlot>;

/// RAII hold that keeps observed facts refreshing while a `Dependencies` lives,
/// and releases checkout filesystem watches when dropped.
struct Hold {
    cache: Weak<Cache>,
    slots: Vec<SharedSlot>,
    #[cfg(target_os = "macos")]
    checkouts: Vec<PathBuf>,
}

impl std::fmt::Debug for Hold {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hold")
            .field("slots", &self.slots.len())
            .finish_non_exhaustive()
    }
}

impl Drop for Hold {
    fn drop(&mut self) {
        for slot in &self.slots {
            slot.holders.fetch_sub(1, Ordering::AcqRel);
        }
        #[cfg(target_os = "macos")]
        if !self.checkouts.is_empty() {
            if let Some(cache) = self.cache.upgrade() {
                for checkout in &self.checkouts {
                    cache.release_checkout_watch(checkout);
                }
            }
        }
    }
}

/// A value computed from observed facts. Each variant is the whole identity
/// of one computation: everything it reads that is not an observed fact.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Derived {
    /// Settings resolved for work in a checkout and Wave; `repository` is
    /// the checkout the person's own file is found from.
    Settings {
        checkout: Option<PathBuf>,
        repository: Option<PathBuf>,
        wave: Option<String>,
        outside: crate::config::settings::Outside,
    },
    /// Settings the global `config.yaml` under `outside`'s home declares alone.
    GlobalSettings {
        outside: crate::config::settings::Outside,
    },
    /// The agent of every skill a Wave's settings name.
    SkillAgents {
        checkout: PathBuf,
        wave: String,
        outside: crate::config::settings::Outside,
    },
    /// The skills found under a checkout, a home and the providers' own
    /// directories when the environment moves them.
    SkillCatalog {
        repo: Option<PathBuf>,
        home: Option<PathBuf>,
        claude: Option<PathBuf>,
        codex: Option<PathBuf>,
    },
    /// The remote a repository's plan travels by, which changes only with
    /// its Git config.
    GitRemote { repo: PathBuf },
}

/// The entries and revisions read by one invocation, including bounded workers.
#[derive(Debug, Clone, Default)]
pub struct Dependencies(Arc<Mutex<Observed>>);

/// A derived value's dependencies are held by reference: using a value costs
/// one entry here, whatever its computation read.
#[derive(Debug, Default)]
struct Observed {
    facts: HashMap<Fact, (SharedSlot, u64)>,
    derived: HashMap<Derived, Dependencies>,
    hold: Option<Hold>,
}

impl Dependencies {
    pub fn changed(&self) -> bool {
        let observed = self.0.lock().expect("observed dependencies");
        observed
            .facts
            .values()
            .any(|(slot, revision)| slot.entry.lock().expect("observed fact").revision != *revision)
            || observed.derived.values().any(Dependencies::changed)
    }

    /// Check whether any observed fact moved, validating stat-guarded file,
    /// bytes, directory, and metadata dependencies inline (~3–5 µs) when
    /// `check_held` is true (bare/background reads) or the fact is unheld
    /// (one-shot served reads).
    fn changed_or_stat_stale(&self, check_held: bool) -> bool {
        let observed = self.0.lock().expect("observed dependencies");
        let now = SystemTime::now();
        for (fact, (slot, revision)) in &observed.facts {
            let mut guard = slot.entry.lock().expect("observed fact");
            if guard.revision != *revision {
                return true;
            }
            if check_held || !slot.is_held() {
                if let Some(current_stat) = fact.stat_stamp() {
                    if guard.stat.as_ref() != Some(&current_stat) {
                        let answer = fact.observe();
                        let changed = guard.update(answer, Some(current_stat), now);
                        let moved = changed || guard.revision != *revision;
                        drop(guard);
                        if changed {
                            slot.notify_watchers();
                        }
                        if moved {
                            return true;
                        }
                        continue;
                    }
                }
            }
        }
        observed
            .derived
            .values()
            .any(|dep| dep.changed_or_stat_stale(check_held))
    }

    fn collect_slots(&self, out: &mut HashMap<Fact, SharedSlot>) {
        let observed = self.0.lock().expect("observed dependencies");
        for (fact, (slot, _)) in &observed.facts {
            out.entry(fact.clone()).or_insert_with(|| slot.clone());
        }
        let nested: Vec<Dependencies> = observed.derived.values().cloned().collect();
        drop(observed);
        for dep in nested {
            dep.collect_slots(out);
        }
    }
}

/// One derived value, locked while it is computed so concurrent readings
/// asking for it compute once.
type Slot = Arc<Mutex<Option<Computed>>>;

#[derive(Debug)]
struct Computed {
    value: Arc<dyn Any + Send + Sync>,
    dependencies: Dependencies,
    used: SystemTime,
}

#[derive(Debug)]
pub struct Cache {
    entries: Mutex<HashMap<Fact, SharedSlot>>,
    derived: Mutex<HashMap<Derived, Slot>>,
    wake: (Mutex<u64>, Condvar),
    use_fs_watcher: bool,
    fs_watcher: OnceLock<Option<Mutex<FsWatcher>>>,
    #[cfg(test)]
    computations: Mutex<Vec<Derived>>,
}

impl Default for Cache {
    fn default() -> Self {
        Self::new(false)
    }
}

static PROCESS_CACHE: LazyLock<Arc<Cache>> = LazyLock::new(|| Arc::new(Cache::new(true)));
static REFRESH_STARTED: AtomicBool = AtomicBool::new(false);

impl Cache {
    fn new(use_fs_watcher: bool) -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
            derived: Mutex::new(HashMap::new()),
            wake: (Mutex::new(0), Condvar::new()),
            use_fs_watcher,
            fs_watcher: OnceLock::new(),
            #[cfg(test)]
            computations: Mutex::new(Vec::new()),
        }
    }

    #[cfg(test)]
    pub(crate) fn with_fs_watcher() -> Arc<Self> {
        Arc::new(Self::new(true))
    }

    pub fn shared() -> Arc<Self> {
        let cache = PROCESS_CACHE.clone();
        if !REFRESH_STARTED.swap(true, Ordering::AcqRel) {
            let bg = cache.clone();
            let _ = std::thread::Builder::new()
                .name("observe-refresh".into())
                .spawn(move || bg.refresh_loop());
        }
        cache
    }

    fn notify_refresh(&self) {
        let (lock, cv) = &self.wake;
        let mut generation = lock.lock().expect("observe wake lock");
        *generation = generation.wrapping_add(1);
        cv.notify_all();
    }

    fn refresh_loop(&self) {
        let (lock, cv) = &self.wake;
        loop {
            let before_gen = *lock.lock().expect("observe wake lock");
            let wait = self.refresh_due_at(SystemTime::now());
            let guard = lock.lock().expect("observe wake lock");
            if *guard != before_gen {
                continue;
            }
            match wait {
                Some(duration) if duration.is_zero() => {}
                Some(duration) => {
                    drop(
                        cv.wait_timeout_while(guard, duration, |g| *g == before_gen)
                            .expect("observe wake condvar"),
                    );
                }
                None => {
                    drop(
                        cv.wait_while(guard, |g| *g == before_gen)
                            .expect("observe wake condvar"),
                    );
                }
            }
        }
    }

    fn held_due_at(&self, fact: &Fact, entry: &Entry) -> Option<SystemTime> {
        entry.answer.as_ref()?;
        match fact {
            Fact::Deadline(at) => {
                if matches!(entry.answer, Some(Answer::Bool(true))) {
                    return None;
                }
                Some(SystemTime::UNIX_EPOCH + Duration::from_secs((*at).max(0) as u64))
            }
            Fact::Git(checkout, _) | Fact::CheckoutFiles(checkout)
                if cfg!(target_os = "macos")
                    && self.use_fs_watcher
                    && self.is_checkout_watched(checkout) =>
            {
                if !entry.dirty {
                    return None;
                }
                Some(entry.checked + fact.interval())
            }
            Fact::Turn(..) | Fact::RawConversations => {
                let regular = entry.checked + fact.interval();
                Some(match entry.hinted_due {
                    Some(hinted) => regular.min(hinted),
                    None => regular,
                })
            }
            _ => Some(entry.checked + fact.interval()),
        }
    }

    /// Refresh every held fact due at `now`, evict stale unheld entries, and
    /// return how long until the next wake (`None` when completely idle).
    pub(crate) fn refresh_due_at(&self, now: SystemTime) -> Option<Duration> {
        // Evict unheld derived values past UNHELD_TTL.
        let mut has_unheld = false;
        {
            let mut derived = self.derived.lock().expect("derived values");
            derived.retain(|_, slot| {
                let keep = Arc::strong_count(slot) > 1
                    || slot.lock().is_ok_and(|computed| {
                        computed.as_ref().is_some_and(|computed| {
                            Arc::strong_count(&computed.dependencies.0) > 1
                                || now.duration_since(computed.used).unwrap_or_default()
                                    < UNHELD_TTL
                        })
                    });
                if keep && Arc::strong_count(slot) == 1 {
                    has_unheld = true;
                }
                keep
            });
        }
        let held_slots: Vec<SharedSlot> = {
            let mut entries = self.entries.lock().expect("observation cache");
            entries.retain(|_, slot| {
                let keep = slot.is_held()
                    || slot.entry.try_lock().map_or(true, |entry| {
                        now.duration_since(entry.used).unwrap_or_default() < UNHELD_TTL
                    });
                if keep && !slot.is_held() {
                    has_unheld = true;
                }
                keep
            });
            trim_idle_checkouts(&mut entries);
            entries
                .values()
                .filter(|slot| slot.is_held())
                .cloned()
                .collect()
        };

        let mut earliest_future: Option<SystemTime> = None;
        let mut due_now = Vec::new();
        for slot in held_slots {
            let Ok(entry) = slot.entry.try_lock() else {
                continue;
            };
            let Some(due_at) = self.held_due_at(&slot.fact, &entry) else {
                continue;
            };
            if due_at <= now {
                due_now.push((slot.clone(), entry.checked, entry.stat.clone()));
            } else {
                min_time(&mut earliest_future, due_at);
            }
        }

        for (slot, checked, prior_stat) in due_now {
            if !slot.is_held() {
                continue;
            }
            let current_stat = slot.fact.stat_stamp();
            if current_stat.is_some() && current_stat == prior_stat {
                let mut entry = slot.entry.lock().expect("observed fact");
                if entry.checked == checked {
                    entry.checked = now;
                }
                if let Some(next) = self.held_due_at(&slot.fact, &entry) {
                    min_time(&mut earliest_future, next);
                }
                continue;
            }
            // Clear dirty/hint before observing so events arriving during the
            // observation schedule another pass.
            {
                let mut entry = slot.entry.lock().expect("observed fact");
                if entry.checked != checked {
                    continue;
                }
                entry.dirty = false;
                entry.hinted_due = None;
            }
            let answer = slot.fact.observe();
            let (changed, next) = {
                let mut entry = slot.entry.lock().expect("observed fact");
                let changed = if entry.checked == checked {
                    entry.update(answer, current_stat, now)
                } else {
                    false
                };
                let next = self.held_due_at(&slot.fact, &entry);
                (changed, next)
            };
            if changed {
                slot.notify_watchers();
            }
            if let Some(next) = next {
                min_time(&mut earliest_future, next);
            }
        }

        if let Some(next) = earliest_future {
            Some(next.duration_since(now).unwrap_or(Duration::ZERO))
        } else if has_unheld {
            Some(UNHELD_TTL)
        } else {
            None
        }
    }

    /// A transcript at `path` changed: read its turn again after [`TURN_HINTED`].
    /// `created` also looks again for raw conversations; `None` means events
    /// were lost and every transcript is due.
    pub(crate) fn hint_transcript(&self, path: Option<&Path>, created: bool) {
        let now = SystemTime::now();
        let target = now + TURN_HINTED;
        let mut any = false;
        let entries = self.entries.lock().expect("observation cache");
        for (fact, slot) in entries.iter() {
            if !slot.is_held() {
                continue;
            }
            let hinted = match fact {
                Fact::Turn(_, watched) => path.is_none_or(|path| watched.path() == path),
                Fact::RawConversations => created || path.is_none(),
                _ => false,
            };
            if hinted {
                let mut entry = slot.entry.lock().expect("observed fact");
                let due = (entry.checked + TURN_HINTED).max(target);
                entry.hinted_due = Some(match entry.hinted_due {
                    Some(existing) => existing.min(due),
                    None => due,
                });
                any = true;
            }
        }
        drop(entries);
        if any {
            self.notify_refresh();
        }
    }

    /// Mark held `Git` and `CheckoutFiles` facts under `checkout` (or all
    /// checkouts when `None`) dirty after a filesystem event.
    #[cfg(target_os = "macos")]
    fn mark_checkout_dirty(&self, checkout: Option<&Path>) {
        let mut any = false;
        let entries = self.entries.lock().expect("observation cache");
        for (fact, slot) in entries.iter() {
            if !slot.is_held() {
                continue;
            }
            let matches = match (fact.checkout_root(), checkout) {
                (Some(root), Some(target)) => root == target,
                (Some(_), None) => true,
                _ => false,
            };
            if matches {
                let mut entry = slot.entry.lock().expect("observed fact");
                entry.dirty = true;
                any = true;
            }
        }
        drop(entries);
        if any {
            self.notify_refresh();
        }
    }

    fn fs_watcher(&self) -> Option<&Mutex<FsWatcher>> {
        self.fs_watcher
            .get_or_init(|| FsWatcher::new().ok().map(Mutex::new))
            .as_ref()
    }

    #[cfg(target_os = "macos")]
    fn is_checkout_watched(&self, checkout: &Path) -> bool {
        let Some(watcher) = self.fs_watcher.get().and_then(Option::as_ref) else {
            return false;
        };
        watcher
            .lock()
            .expect("fs watcher")
            .is_checkout_watched(checkout)
    }

    #[cfg(not(target_os = "macos"))]
    fn is_checkout_watched(&self, _checkout: &Path) -> bool {
        false
    }

    #[cfg(target_os = "macos")]
    fn acquire_checkout_watch(self: &Arc<Self>, checkout: &Path) {
        if let Some(watcher) = self.fs_watcher() {
            watcher
                .lock()
                .expect("fs watcher")
                .acquire_checkout(checkout, Arc::downgrade(self));
        }
    }

    #[cfg(target_os = "macos")]
    fn release_checkout_watch(&self, checkout: &Path) {
        if let Some(watcher) = self.fs_watcher.get().and_then(Option::as_ref) {
            watcher
                .lock()
                .expect("fs watcher")
                .release_checkout(checkout);
        }
    }

    /// Watch all existing harness transcript roots through this cache's single
    /// shared filesystem watcher until the returned guard drops.
    pub(crate) fn watch_transcripts(self: &Arc<Self>) -> TranscriptWatchGuard {
        let mut ids = Vec::new();
        if let Some(watcher) = self.fs_watcher() {
            let mut guard = watcher.lock().expect("fs watcher");
            for kind in loopflow_harness::HarnessKind::ALL {
                let harness = kind.harness();
                let home = harness.transcript_home();
                let root = harness.transcript_root(&home).0;
                let store = harness.transcript_watch_store(&home);
                if !root.is_dir() {
                    continue;
                }
                match guard.add_transcript_root(&root, store, Arc::downgrade(self)) {
                    Ok(id) => {
                        loopflow_harness::transcript::register_watched_root(root);
                        ids.push(id);
                    }
                    Err(error) => {
                        tracing::warn!(root = %root.display(), %error, "Could not watch transcripts");
                    }
                }
            }
        }
        TranscriptWatchGuard {
            cache: Arc::downgrade(self),
            ids,
        }
    }

    /// Run one refresh of held facts due now.
    #[cfg(any(test, feature = "test-support"))]
    pub fn refresh_local(&self) {
        self.refresh_due_at(SystemTime::now());
    }

    /// Observe every held fact again now, whatever its interval.
    #[cfg(test)]
    pub(crate) fn refresh_all(&self) {
        let now = SystemTime::now();
        let past = now - Duration::from_secs(61);
        for slot in self.entries.lock().unwrap().values() {
            if slot.is_held() {
                let mut entry = slot.entry.lock().unwrap();
                entry.checked = past;
                entry.dirty = true;
            }
        }
        self.refresh_due_at(now);
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_raw_conversations(raw: Vec<crate::transcript::raw::RawDirectory>) -> Arc<Self> {
        let cache = Self::default();
        cache.entries.lock().unwrap().insert(
            Fact::RawConversations,
            FactSlot::new(
                Fact::RawConversations,
                Entry::new(Some(Answer::Raw(raw)), SystemTime::now()),
            ),
        );
        Arc::new(cache)
    }

    /// Run `read` with shared caching without holding its facts after it
    /// returns. Used by one-shot served reads so they never keep background
    /// refreshes or checkout watches alive.
    pub fn read_once<T>(self: &Arc<Self>, read: impl FnOnce() -> T) -> T {
        scope(
            Some(Reading {
                cache: self.clone(),
                dependencies: None,
            }),
            read,
        )
    }

    pub fn capture<T>(self: &Arc<Self>, read: impl FnOnce() -> T) -> (T, Dependencies) {
        self.capture_inner(None, read)
    }

    pub(crate) fn capture_for<T>(
        self: &Arc<Self>,
        watcher: Weak<dyn FactWatcher>,
        read: impl FnOnce() -> T,
    ) -> (T, Dependencies) {
        self.capture_inner(Some(watcher), read)
    }

    fn capture_inner<T>(
        self: &Arc<Self>,
        watcher: Option<Weak<dyn FactWatcher>>,
        read: impl FnOnce() -> T,
    ) -> (T, Dependencies) {
        let dependencies = Dependencies::default();
        let result = scope(
            Some(Reading {
                cache: self.clone(),
                dependencies: Some(dependencies.clone()),
            }),
            read,
        );
        let mut collected = HashMap::new();
        dependencies.collect_slots(&mut collected);
        let mut slots = Vec::with_capacity(collected.len());
        #[cfg(target_os = "macos")]
        let mut checkout_set = HashSet::new();
        for (fact, slot) in collected {
            slot.holders.fetch_add(1, Ordering::AcqRel);
            if let Some(ref w) = watcher {
                slot.add_watcher(w.clone());
            }
            #[cfg(target_os = "macos")]
            if self.use_fs_watcher {
                if let Some(root) = fact.checkout_root() {
                    checkout_set.insert(root.to_path_buf());
                }
            }
            let _ = fact;
            slots.push(slot);
        }
        #[cfg(target_os = "macos")]
        let checkouts: Vec<PathBuf> = checkout_set.into_iter().collect();
        #[cfg(target_os = "macos")]
        for checkout in &checkouts {
            self.acquire_checkout_watch(checkout);
        }
        dependencies.0.lock().expect("observed dependencies").hold = Some(Hold {
            cache: Arc::downgrade(self),
            slots,
            #[cfg(target_os = "macos")]
            checkouts,
        });
        self.notify_refresh();
        (result, dependencies)
    }

    fn read(&self, fact: Fact, dependencies: Option<&Dependencies>) -> Answer {
        let now = SystemTime::now();
        let slot = self
            .entries
            .lock()
            .expect("observation cache")
            .entry(fact.clone())
            .or_insert_with(|| FactSlot::new(fact.clone(), Entry::new(None, now)))
            .clone();
        let mut observed = slot.entry.lock().expect("observed fact");
        let (must_observe, stat) = if observed.answer.is_none() {
            (true, fact.stat_stamp())
        } else if !slot.is_held() {
            match fact.stat_stamp() {
                Some(current) if observed.stat.as_ref() == Some(&current) => {
                    observed.checked = now;
                    (false, None)
                }
                Some(current) => (true, Some(current)),
                None => (
                    now.duration_since(observed.checked).unwrap_or_default() >= fact.interval(),
                    None,
                ),
            }
        } else {
            (false, None)
        };
        if must_observe {
            let answer = fact.observe();
            observed.update(answer, stat, now);
        }
        observed.used = now;
        let revision = observed.revision;
        let answer = observed
            .answer
            .clone()
            .expect("fact observed before reading");
        drop(observed);
        if let Some(dependencies) = dependencies {
            // Keep the first revision so a change during the reading stays visible.
            dependencies
                .0
                .lock()
                .expect("observed dependencies")
                .facts
                .entry(fact)
                .or_insert((slot, revision));
        }
        answer
    }

    /// Number of entries currently held by at least one live `Dependencies`.
    pub fn held_facts(&self) -> u64 {
        self.entries
            .lock()
            .expect("observation cache")
            .values()
            .filter(|slot| slot.is_held())
            .count() as u64
    }

    /// How many times values matching `key` were computed.
    #[cfg(test)]
    pub(crate) fn computations(&self, key: impl Fn(&Derived) -> bool) -> usize {
        let computations = self.computations.lock().unwrap();
        computations.iter().filter(|computed| key(computed)).count()
    }
}

/// Keep the most recently searched inventories. Live readings own their entries
/// until they finish, including a checkout larger than the idle cache.
fn trim_idle_checkouts(entries: &mut HashMap<Fact, SharedSlot>) {
    let mut idle: Vec<_> = entries
        .iter()
        .filter_map(|(fact, slot)| {
            if !matches!(fact, Fact::CheckoutFiles(_)) || slot.is_held() {
                return None;
            }
            let entry = slot.entry.try_lock().ok()?;
            let Some(Answer::CheckoutFiles(Ok(files))) = &entry.answer else {
                return None;
            };
            Some((
                entry.used,
                files.paths.len() + files.scratch.len(),
                fact.clone(),
            ))
        })
        .collect();
    idle.sort_unstable_by_key(|(used, _, _)| std::cmp::Reverse(*used));
    let mut remaining = 100_000usize;
    for (_, paths, fact) in idle {
        if paths > remaining {
            entries.remove(&fact);
        } else {
            remaining -= paths;
        }
    }
}

// ---------------------------------------------------------------------------
// Shared process-wide filesystem watcher
// ---------------------------------------------------------------------------

/// Guard that unregisters transcript roots from the shared watcher on drop.
#[derive(Debug)]
pub struct TranscriptWatchGuard {
    cache: Weak<Cache>,
    ids: Vec<u64>,
}

impl Drop for TranscriptWatchGuard {
    fn drop(&mut self) {
        if self.ids.is_empty() {
            return;
        }
        if let Some(cache) = self.cache.upgrade() {
            if let Some(watcher) = cache.fs_watcher.get().and_then(Option::as_ref) {
                let mut guard = watcher.lock().expect("fs watcher");
                for id in &self.ids {
                    guard.remove_transcript_root(*id);
                }
            }
        }
    }
}

/// Guard that unregisters a `TaskFilesWatch` listener from the shared watcher on drop.
#[derive(Debug)]
pub(crate) struct CheckoutWatchGuard {
    cache: Weak<Cache>,
    id: u64,
}

impl Drop for CheckoutWatchGuard {
    fn drop(&mut self) {
        if let Some(cache) = self.cache.upgrade() {
            if let Some(watcher) = cache.fs_watcher.get().and_then(Option::as_ref) {
                watcher
                    .lock()
                    .expect("fs watcher")
                    .remove_path_listener(self.id);
            }
        }
    }
}

/// Subscribe a `TaskFilesWatch` callback to the process's single shared
/// filesystem watcher for `checkout` (and its Git metadata directory when
/// outside `checkout`).
pub(crate) fn watch_checkout_paths(
    checkout: &Path,
    changed: impl FnMut(notify::Result<Vec<String>>) + Send + 'static,
) -> anyhow::Result<CheckoutWatchGuard> {
    let cache = Cache::shared();
    let watcher = cache
        .fs_watcher()
        .ok_or_else(|| anyhow::anyhow!("shared filesystem watcher could not start"))?;
    let id = watcher
        .lock()
        .expect("fs watcher")
        .add_path_listener(checkout, Box::new(changed))?;
    Ok(CheckoutWatchGuard {
        cache: Arc::downgrade(&cache),
        id,
    })
}

struct FsWatcher {
    watcher: notify::RecommendedWatcher,
    watched_paths: HashMap<PathBuf, usize>,
    routes: Arc<Mutex<FsWatchRoutes>>,
    next_id: u64,
}

impl std::fmt::Debug for FsWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FsWatcher")
            .field("watched_paths", &self.watched_paths.len())
            .finish_non_exhaustive()
    }
}

#[derive(Default)]
struct FsWatchRoutes {
    transcripts: Vec<TranscriptRoute>,
    #[cfg(target_os = "macos")]
    checkouts: HashMap<PathBuf, CheckoutRoute>,
    path_listeners: Vec<PathListener>,
}

struct TranscriptRoute {
    id: u64,
    watched_path: PathBuf,
    roots: [PathBuf; 2],
    store: Option<PathBuf>,
    cache: Weak<Cache>,
}

#[cfg(target_os = "macos")]
struct CheckoutRoute {
    refcount: usize,
    watched_paths: Vec<PathBuf>,
    prefixes: Vec<PathBuf>,
    cache: Weak<Cache>,
}

struct PathListener {
    id: u64,
    watched_paths: Vec<PathBuf>,
    checkout: [PathBuf; 2],
    metadata: [PathBuf; 2],
    callback: Box<dyn FnMut(notify::Result<Vec<String>>) + Send>,
}

impl PathListener {
    fn relative(&self, path: &Path) -> Option<String> {
        if let Some(inside) = self
            .metadata
            .iter()
            .find_map(|root| path.strip_prefix(root).ok())
        {
            return Some(
                Path::new(".git")
                    .join(inside)
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        let inside = self
            .checkout
            .iter()
            .find_map(|root| path.strip_prefix(root).ok())?;
        Some(inside.to_string_lossy().into_owned())
    }
}

impl FsWatcher {
    fn new() -> notify::Result<Self> {
        let routes = Arc::new(Mutex::new(FsWatchRoutes::default()));
        let dispatch_routes = routes.clone();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            dispatch_fs_event(&dispatch_routes, event);
        })?;
        Ok(Self {
            watcher,
            watched_paths: HashMap::new(),
            routes,
            next_id: 1,
        })
    }

    fn watch_path(&mut self, path: &Path) -> notify::Result<()> {
        let entry = self.watched_paths.entry(path.to_path_buf()).or_insert(0);
        if *entry == 0 {
            if let Err(error) = self.watcher.watch(path, RecursiveMode::Recursive) {
                self.watched_paths.remove(path);
                return Err(error);
            }
        }
        *entry += 1;
        Ok(())
    }

    fn unwatch_path(&mut self, path: &Path) {
        let Some(count) = self.watched_paths.get_mut(path) else {
            return;
        };
        if *count <= 1 {
            self.watched_paths.remove(path);
            let _ = self.watcher.unwatch(path);
        } else {
            *count -= 1;
        }
    }

    fn add_transcript_root(
        &mut self,
        root: &Path,
        store: Option<PathBuf>,
        cache: Weak<Cache>,
    ) -> notify::Result<u64> {
        self.watch_path(root)?;
        let id = self.next_id;
        self.next_id += 1;
        let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        self.routes
            .lock()
            .expect("fs routes")
            .transcripts
            .push(TranscriptRoute {
                id,
                watched_path: root.to_path_buf(),
                roots: [root.to_path_buf(), canonical],
                store,
                cache,
            });
        Ok(id)
    }

    fn remove_transcript_root(&mut self, id: u64) {
        let removed = {
            let mut routes = self.routes.lock().expect("fs routes");
            routes
                .transcripts
                .iter()
                .position(|r| r.id == id)
                .map(|idx| routes.transcripts.remove(idx).watched_path)
        };
        if let Some(path) = removed {
            self.unwatch_path(&path);
        }
    }

    #[cfg(target_os = "macos")]
    fn is_checkout_watched(&self, checkout: &Path) -> bool {
        self.routes
            .lock()
            .expect("fs routes")
            .checkouts
            .contains_key(checkout)
    }

    #[cfg(target_os = "macos")]
    fn acquire_checkout(&mut self, checkout: &Path, cache: Weak<Cache>) {
        {
            let mut routes = self.routes.lock().expect("fs routes");
            if let Some(existing) = routes.checkouts.get_mut(checkout) {
                existing.refcount += 1;
                return;
            }
        }
        if !checkout.is_dir() {
            return;
        }
        let mut watched_paths = Vec::new();
        let mut prefixes = vec![checkout.to_path_buf()];
        if let Ok(canonical) = checkout.canonicalize() {
            if canonical != checkout {
                prefixes.push(canonical);
            }
        }
        if self.watch_path(checkout).is_err() {
            return;
        }
        watched_paths.push(checkout.to_path_buf());
        if let Ok(metadata) = crate::git::absolute_git_dir(checkout) {
            if !metadata.starts_with(checkout)
                && metadata.is_dir()
                && self.watch_path(&metadata).is_ok()
            {
                watched_paths.push(metadata.clone());
                if let Ok(canonical) = metadata.canonicalize() {
                    if canonical != metadata {
                        prefixes.push(canonical);
                    }
                }
                prefixes.push(metadata);
            }
        }
        self.routes.lock().expect("fs routes").checkouts.insert(
            checkout.to_path_buf(),
            CheckoutRoute {
                refcount: 1,
                watched_paths,
                prefixes,
                cache,
            },
        );
    }

    #[cfg(target_os = "macos")]
    fn release_checkout(&mut self, checkout: &Path) {
        let to_unwatch = {
            let mut routes = self.routes.lock().expect("fs routes");
            match routes.checkouts.get_mut(checkout) {
                Some(entry) if entry.refcount > 1 => {
                    entry.refcount -= 1;
                    None
                }
                Some(_) => routes
                    .checkouts
                    .remove(checkout)
                    .map(|entry| entry.watched_paths),
                None => None,
            }
        };
        if let Some(paths) = to_unwatch {
            for path in paths {
                self.unwatch_path(&path);
            }
        }
    }

    fn add_path_listener(
        &mut self,
        checkout: &Path,
        callback: Box<dyn FnMut(notify::Result<Vec<String>>) + Send>,
    ) -> anyhow::Result<u64> {
        let metadata = crate::git::absolute_git_dir(checkout)?;
        let checkout_roots = [checkout.to_path_buf(), checkout.canonicalize()?];
        let metadata_roots = [metadata.clone(), metadata.canonicalize()?];
        self.watch_path(checkout)?;
        let mut watched_paths = vec![checkout.to_path_buf()];
        if !metadata.starts_with(checkout) {
            if let Err(error) = self.watch_path(&metadata) {
                self.unwatch_path(checkout);
                return Err(error.into());
            }
            watched_paths.push(metadata);
        }
        let id = self.next_id;
        self.next_id += 1;
        self.routes
            .lock()
            .expect("fs routes")
            .path_listeners
            .push(PathListener {
                id,
                watched_paths,
                checkout: checkout_roots,
                metadata: metadata_roots,
                callback,
            });
        Ok(id)
    }

    fn remove_path_listener(&mut self, id: u64) {
        let removed = {
            let mut routes = self.routes.lock().expect("fs routes");
            routes
                .path_listeners
                .iter()
                .position(|l| l.id == id)
                .map(|idx| routes.path_listeners.remove(idx).watched_paths)
        };
        if let Some(paths) = removed {
            for path in paths {
                self.unwatch_path(&path);
            }
        }
    }
}

fn dispatch_fs_event(routes: &Mutex<FsWatchRoutes>, event: notify::Result<notify::Event>) {
    enum TranscriptAction {
        Rescan(Weak<Cache>),
        Paths {
            cache: Weak<Cache>,
            store: Option<PathBuf>,
            created: bool,
            paths: Vec<PathBuf>,
        },
    }
    #[cfg(target_os = "macos")]
    let mut dirty_checkouts: Vec<(Weak<Cache>, Option<PathBuf>)> = Vec::new();
    let mut transcript_actions: Vec<TranscriptAction> = Vec::new();

    {
        let mut routes = routes.lock().expect("fs routes");
        let mut queue_rescan = |routes: &FsWatchRoutes| {
            for tr in &routes.transcripts {
                transcript_actions.push(TranscriptAction::Rescan(tr.cache.clone()));
            }
            #[cfg(target_os = "macos")]
            {
                let mut seen_caches = HashSet::new();
                for route in routes.checkouts.values() {
                    if seen_caches.insert(route.cache.as_ptr() as usize) {
                        dirty_checkouts.push((route.cache.clone(), None));
                    }
                }
            }
        };
        match event {
            Ok(ref ev) if ev.kind.is_access() => return,
            Ok(ref ev) if ev.need_rescan() => {
                for listener in &mut routes.path_listeners {
                    (listener.callback)(Ok(vec![String::new()]));
                }
                queue_rescan(&routes);
            }
            Err(err) => {
                let message = err.to_string();
                for listener in &mut routes.path_listeners {
                    (listener.callback)(Err(notify::Error::generic(&message)));
                }
                queue_rescan(&routes);
            }
            Ok(ev) => {
                for listener in &mut routes.path_listeners {
                    let paths: Vec<_> = ev
                        .paths
                        .iter()
                        .filter_map(|path| listener.relative(path))
                        .collect();
                    if !paths.is_empty() {
                        (listener.callback)(Ok(paths));
                    }
                }
                for tr in &routes.transcripts {
                    let matched: Vec<PathBuf> = ev
                        .paths
                        .iter()
                        .filter(|path| tr.roots.iter().any(|root| path.starts_with(root)))
                        .cloned()
                        .collect();
                    if !matched.is_empty() {
                        transcript_actions.push(TranscriptAction::Paths {
                            cache: tr.cache.clone(),
                            store: tr.store.clone(),
                            created: ev.kind.is_create() && tr.store.is_none(),
                            paths: matched,
                        });
                    }
                }
                #[cfg(target_os = "macos")]
                for (checkout, route) in &routes.checkouts {
                    if ev
                        .paths
                        .iter()
                        .any(|path| route.prefixes.iter().any(|prefix| path.starts_with(prefix)))
                    {
                        dirty_checkouts.push((route.cache.clone(), Some(checkout.clone())));
                    }
                }
            }
        }
    }

    for action in transcript_actions {
        match action {
            TranscriptAction::Rescan(cache) => {
                loopflow_harness::transcript::invalidate_misses(None);
                if let Some(cache) = cache.upgrade() {
                    cache.hint_transcript(None, true);
                }
            }
            TranscriptAction::Paths {
                cache,
                store,
                created,
                paths,
            } => {
                if created {
                    for path in &paths {
                        loopflow_harness::transcript::invalidate_misses(Some(path));
                    }
                }
                if let Some(cache) = cache.upgrade() {
                    for path in crate::transcript::watch::transcripts(&paths, store.as_deref()) {
                        cache.hint_transcript(Some(&path), created);
                    }
                }
            }
        }
    }
    #[cfg(target_os = "macos")]
    for (cache, checkout) in dirty_checkouts {
        if let Some(cache) = cache.upgrade() {
            cache.mark_checkout_dirty(checkout.as_deref());
        }
    }
}

#[derive(Clone)]
struct Reading {
    cache: Arc<Cache>,
    dependencies: Option<Dependencies>,
}
thread_local! {
    static READING: RefCell<Option<Reading>> = const { RefCell::new(None) };
}

fn scope<T>(reading: Option<Reading>, read: impl FnOnce() -> T) -> T {
    struct Restore(Option<Reading>);
    impl Drop for Restore {
        fn drop(&mut self) {
            READING.set(self.0.take());
        }
    }
    let _restore = Restore(READING.replace(reading));
    read()
}

/// Publish a file change to existing served reads before returning to the writer.
pub(crate) fn invalidate_path(path: &Path) {
    let cache = &*PROCESS_CACHE;
    let path = absolute(path);
    let facts = [
        Some(Fact::File(path.clone())),
        Some(Fact::Metadata(path.clone())),
        path.parent()
            .map(|parent| Fact::Directory(parent.to_owned())),
    ];
    let now = SystemTime::now();
    for fact in facts.into_iter().flatten() {
        let slot = cache
            .entries
            .lock()
            .expect("observation cache")
            .get(&fact)
            .cloned();
        if let Some(slot) = slot {
            let stat = fact.stat_stamp();
            let changed = {
                let mut entry = slot.entry.lock().expect("observed fact");
                entry.update(fact.observe(), stat, now)
            };
            if changed {
                slot.notify_watchers();
            }
        }
    }
}

pub fn active() -> bool {
    READING.with(|reading| reading.borrow().is_some())
}

/// Resolve a proposed file edit through the ordinary readers without publishing
/// the draft in the serving Machine's observation cache.
pub(crate) fn preview_files<T>(files: &BTreeMap<PathBuf, String>, read: impl FnOnce() -> T) -> T {
    let cache = Cache::default();
    let now = SystemTime::now();
    for (path, text) in files {
        let fact = Fact::File(absolute(path));
        cache.entries.lock().expect("private preview cache").insert(
            fact.clone(),
            FactSlot::new(fact, Entry::new(Some(Answer::File(Ok(text.clone()))), now)),
        );
    }
    Arc::new(cache).read_once(read)
}

/// Run `read` as one reading: the served reading already active, else one
/// that lasts for this call, so a bare command observes each fact and
/// computes each derived value once however often `read` asks.
pub fn reading<T>(read: impl FnOnce() -> T) -> T {
    if active() {
        read()
    } else {
        Arc::new(Cache::default()).read_once(read)
    }
}

pub fn inherit<T>(read: impl FnOnce() -> T) -> impl FnOnce() -> T {
    let reading = READING.with(|reading| reading.borrow().clone());
    loopflow_storage::instrumentation::inherit(move || scope(reading, read))
}

/// `compute`'s result, shared across the process until a fact it observed
/// changes. Works both inside a served `READING` and in background loops or
/// bare commands (validating file/directory/metadata dependencies inline via
/// `stat` so external edits are seen immediately).
///
/// `compute` reads observed facts and `key`, nothing else: an input from the
/// environment, a thread-local or the store that is missing from `key` leaves
/// the value stale until one of its files changes.
///
/// # Panics
///
/// When two computations of different types share one `key`.
pub fn derived<T: Send + Sync + 'static>(key: Derived, compute: impl FnOnce() -> T) -> Arc<T> {
    let reading = READING.with(|reading| reading.borrow().clone());
    let cache = match &reading {
        Some(reading) => reading.cache.clone(),
        None => PROCESS_CACHE.clone(),
    };
    let slot = cache
        .derived
        .lock()
        .expect("derived values")
        .entry(key.clone())
        .or_default()
        .clone();
    // A computation that panicked stored nothing; the next caller computes.
    let mut computed = slot.lock().unwrap_or_else(|poisoned| {
        slot.clear_poison();
        poisoned.into_inner()
    });
    let stale = match computed.as_ref() {
        None => true,
        Some(computed) => computed
            .dependencies
            .changed_or_stat_stale(reading.is_none()),
    };
    if stale {
        let dependencies = Dependencies::default();
        let value = scope(
            Some(Reading {
                cache: cache.clone(),
                dependencies: Some(dependencies.clone()),
            }),
            compute,
        );
        #[cfg(test)]
        cache.computations.lock().unwrap().push(key.clone());
        *computed = Some(Computed {
            value: Arc::new(value),
            dependencies,
            used: SystemTime::now(),
        });
    }
    let computed = computed.as_mut().expect("computed above");
    computed.used = SystemTime::now();
    // Keep the first dependencies so a value computed again during the
    // reading stays visible as a change.
    if let Some(dependencies) = reading.as_ref().and_then(|r| r.dependencies.as_ref()) {
        dependencies
            .0
            .lock()
            .expect("observed dependencies")
            .derived
            .entry(key)
            .or_insert_with(|| computed.dependencies.clone());
    }
    computed
        .value
        .clone()
        .downcast()
        .unwrap_or_else(|_| panic!("a derived key names one type"))
}

fn read(fact: Fact) -> Answer {
    let reading = READING.with(|reading| reading.borrow().clone());
    match reading {
        Some(reading) => reading.cache.read(fact, reading.dependencies.as_ref()),
        None => fact.observe(),
    }
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_owned())
}

pub(crate) fn checkout_files(path: &Path) -> io::Result<Arc<TaskFileInventory>> {
    let Answer::CheckoutFiles(answer) = read(Fact::CheckoutFiles(absolute(path))) else {
        unreachable!("checkout files returns an inventory")
    };
    restored(answer)
}

pub fn git(path: &Path, args: &[&str]) -> io::Result<Output> {
    let Answer::Output(answer) = read(Fact::Git(
        absolute(path),
        args.iter().map(|s| s.to_string()).collect(),
    )) else {
        unreachable!("Git returns output")
    };
    restored(answer)
}

pub fn github(path: &Path, args: &[&str]) -> io::Result<Output> {
    let Answer::Output(answer) = read(Fact::Github(
        absolute(path),
        args.iter().map(|s| s.to_string()).collect(),
    )) else {
        unreachable!("GitHub returns output")
    };
    restored(answer)
}

pub fn read_to_string(path: &Path) -> io::Result<String> {
    let Answer::File(answer) = read(Fact::File(absolute(path))) else {
        unreachable!("file returns text")
    };
    restored(answer)
}

pub fn read_dir(path: &Path) -> io::Result<Vec<PathBuf>> {
    let Answer::Directory(answer) = read(Fact::Directory(absolute(path))) else {
        unreachable!("directory returns paths")
    };
    restored(answer)
}

pub(crate) fn file_bytes(path: &Path, limit: usize) -> io::Result<(u64, Vec<u8>)> {
    let Answer::Bytes(answer) = read(Fact::Bytes(absolute(path), limit)) else {
        unreachable!("file returns bytes")
    };
    restored(answer)
}

fn metadata(path: &Path) -> io::Result<FileMetadata> {
    let Answer::Metadata(answer) = read(Fact::Metadata(absolute(path))) else {
        unreachable!("metadata returns file type")
    };
    restored(answer)
}

pub fn try_exists(path: &Path) -> io::Result<bool> {
    match metadata(path) {
        Ok(_) => Ok(true),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            Ok(false)
        }
        Err(error) => Err(error),
    }
}

pub fn exists(path: &Path) -> bool {
    metadata(path).is_ok()
}
pub fn is_dir(path: &Path) -> bool {
    metadata(path).is_ok_and(|metadata| metadata.kind.is_dir())
}
pub fn is_file(path: &Path) -> bool {
    metadata(path).is_ok_and(|metadata| metadata.kind.is_file())
}

pub fn is_symlink(path: &Path) -> bool {
    metadata(path).is_ok_and(|metadata| metadata.symlink)
}

pub fn process(pid: u32) -> io::Result<Option<OsProcess>> {
    let Answer::Process(answer) = read(Fact::Process(pid)) else {
        unreachable!("process returns OS identity")
    };
    restored(answer)
}

/// The current or last turn of a conversation's transcript.
pub(crate) fn transcript_turn(
    harness: loopflow_harness::HarnessKind,
    source: &loopflow_harness::transcript::Source,
) -> Option<loopflow_harness::transcript::live::SessionTurn> {
    transcript_current(harness, source).map(|reading| reading.turn)
}

pub(crate) fn transcript_current(
    harness: loopflow_harness::HarnessKind,
    source: &loopflow_harness::transcript::Source,
) -> Option<loopflow_harness::transcript::live::CurrentTurn> {
    use loopflow_harness::transcript::Source;
    let source = match source {
        Source::File(path) => Source::File(absolute(path)),
        Source::OpenCode { store, id } => Source::OpenCode {
            store: absolute(store),
            id: id.clone(),
        },
    };
    let Answer::Turn(answer) = read(Fact::Turn(harness, source)) else {
        unreachable!("a transcript returns its turn")
    };
    answer
}

/// Directories with provider processes open on this Machine that Loopflow
/// never launched.
pub(crate) fn raw_conversations() -> Vec<crate::transcript::raw::RawDirectory> {
    let Answer::Raw(answer) = read(Fact::RawConversations) else {
        unreachable!("raw conversations return conversations")
    };
    answer
}

/// Time for rolling history windows, observed at minute resolution when served.
pub fn now() -> time::OffsetDateTime {
    let Answer::Time(at) = read(Fact::Clock) else {
        unreachable!("clock returns time")
    };
    time::OffsetDateTime::from_unix_timestamp(at).expect("observed system timestamp")
}

/// Register the next change of a time-dependent result without ticking the read.
pub fn deadline(at: i64) -> bool {
    let Answer::Bool(answer) = read(Fact::Deadline(at)) else {
        unreachable!("deadline returns a boolean")
    };
    answer
}

#[cfg(test)]
mod tests {
    use super::{derived, file_bytes, git, inherit, read_to_string, Cache, Derived};
    use crate::process::os_process::process_identity_evidence as process;
    use crate::store::ProcessIdentityEvidence;
    use std::sync::Arc;

    #[test]
    fn shared_git_answers_refresh_without_changing_bare_reads_or_writes() {
        let repo = loopflow_test_support::TestRepo::new();
        let cache = Arc::new(Cache::default());
        let status = || git(repo.path(), &["status", "--porcelain"]).unwrap().stdout;
        let (first, dependencies) = cache.capture(status);
        assert!(first.is_empty());
        std::fs::write(repo.path().join("new.txt"), "text").unwrap();
        assert!(!status().is_empty());
        assert_eq!(cache.capture(status).0, first);
        cache.refresh_all();
        assert!(dependencies.changed());
        let (second, dependencies) = cache.capture(status);
        assert_eq!(second, status());
        cache.refresh_all();
        assert!(!dependencies.changed());
        // A new server starts with no retained observations.
        let replacement = Arc::new(Cache::default());
        assert_eq!(replacement.capture(status).0, second);
    }

    #[test]
    fn checkout_inventory_observes_ignore_membership_and_scratch_changes() {
        let repo = loopflow_test_support::TestRepo::new();
        let cache = Arc::new(Cache::default());
        let inventory = || super::checkout_files(repo.path()).unwrap();
        let (first, dependencies) = cache.capture(inventory);
        assert!(first.latest_scratch_page.is_none());
        repo.create_file(".gitignore", "scratch/\nwork/\n");
        repo.create_file("work/ignored", "");
        repo.create_file("scratch/live.md", "");
        cache.refresh_all();
        let updated = cache.capture(inventory).0;
        assert!(dependencies.changed());
        assert_eq!(
            updated.latest_scratch_page.as_deref(),
            Some("scratch/live.md")
        );
        assert!(!updated.paths.iter().any(|path| path == "work/ignored"));
        repo.create_file(".gitignore", "scratch/\n");
        std::fs::remove_file(repo.path().join("scratch/live.md")).unwrap();
        cache.refresh_all();
        let updated = cache.capture(inventory).0;
        assert!(updated.latest_scratch_page.is_none());
        assert!(updated.paths.iter().any(|path| path == "work/ignored"));
    }

    #[test]
    fn file_failures_recover_and_only_read_dependencies_change() {
        let directory = tempfile::tempdir().unwrap();
        let first = directory.path().join("first");
        let other = directory.path().join("other");
        std::fs::write(&other, "unchanged").unwrap();
        let cache = Arc::new(Cache::default());
        let (answer, missing) = cache.capture(|| read_to_string(&first));
        assert_eq!(answer.unwrap_err().kind(), std::io::ErrorKind::NotFound);
        let (_, unrelated) = cache.capture(|| read_to_string(&other).unwrap());
        std::fs::write(&first, "created").unwrap();
        cache.refresh_all();
        assert!(missing.changed());
        assert!(!unrelated.changed());
        assert_eq!(
            cache.capture(|| read_to_string(&first)).0.unwrap(),
            "created"
        );
        // A new reading replaces its dependencies; it does not retain the first path.
        let (_, replaced) = cache.capture(|| read_to_string(&other).unwrap());
        std::fs::remove_file(&first).unwrap();
        cache.refresh_all();
        assert!(
            missing.changed(),
            "returning to the old answer still advances its revision"
        );
        assert!(!replaced.changed());
    }

    #[test]
    fn bounded_file_answers_keep_content_and_size_together() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file");
        std::fs::write(&path, "abcdef").unwrap();
        let cache = Arc::new(Cache::default());
        let (answer, dependencies) = cache.capture(|| file_bytes(&path, 3).unwrap());
        assert_eq!(answer, (6, b"abc".to_vec()));
        std::fs::write(&path, "changed").unwrap();
        assert_eq!(cache.capture(|| file_bytes(&path, 3).unwrap()).0, answer);
        cache.refresh_all();
        assert!(dependencies.changed());
        assert_eq!(
            cache.capture(|| file_bytes(&path, 3).unwrap()).0,
            (7, b"cha".to_vec())
        );
    }

    #[test]
    fn nested_workers_share_dependencies_and_unwind_restores_inline_reads() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file");
        std::fs::write(&path, "before").unwrap();
        let cache = Arc::new(Cache::default());
        let (_, dependencies) = cache.capture(|| {
            std::thread::scope(|scope| {
                scope
                    .spawn(inherit(|| read_to_string(&path).unwrap()))
                    .join()
                    .unwrap()
            })
        });
        let failure = std::panic::catch_unwind(|| cache.capture(|| panic!("failed read")));
        assert!(failure.is_err());
        std::fs::write(&path, "after").unwrap();
        assert_eq!(read_to_string(&path).unwrap(), "after");
        cache.refresh_all();
        assert!(dependencies.changed());
    }

    #[test]
    fn github_reports_are_shared_and_refreshed_without_a_store() {
        use std::os::unix::fs::PermissionsExt;
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("gh");
        std::fs::write(&executable, "#!/bin/sh\n/bin/cat report\n").unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _path = crate::config::environment::EnvVarGuard::set(
            "PATH",
            &format!(
                "{}:{}",
                directory.path().display(),
                std::env::var("PATH").unwrap()
            ),
        );
        let report = directory.path().join("report");
        std::fs::write(&report, "pending").unwrap();
        let cache = Arc::new(Cache::default());
        let read = || {
            super::github(directory.path(), &["pr", "checks"])
                .unwrap()
                .stdout
        };
        let (before, dependencies) = cache.capture(read);
        std::fs::write(&report, "passing").unwrap();
        assert_eq!(read(), b"passing");
        assert_eq!(cache.capture(read).0, before);
        cache.refresh_all();
        assert!(dependencies.changed());
        assert_eq!(cache.capture(read).0, b"passing");
    }

    #[test]
    fn graph_discovery_observes_a_file_replaced_by_a_directory() {
        let repo = tempfile::tempdir().unwrap();
        let root = repo.path().join(".lf/graphs");
        std::fs::create_dir(root.parent().unwrap()).unwrap();
        std::fs::write(&root, "not a directory").unwrap();
        let cache = Arc::new(Cache::default());
        let read = || crate::graph::graph_sources_in(repo.path()).unwrap();
        let (before, dependencies) = cache.capture(read);
        assert!(before.is_empty());

        std::fs::remove_file(&root).unwrap();
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("example.yaml"), "[]").unwrap();
        assert!(read().contains_key("example"));
        assert!(cache.capture(read).0.is_empty());
        cache.refresh_all();
        assert!(dependencies.changed());
        assert!(cache.capture(read).0.contains_key("example"));
    }

    #[test]
    fn graph_discovery_observes_a_directory_replaced_by_a_symlink() {
        let repo = tempfile::tempdir().unwrap();
        let directory = repo.path().join(".lf/graphs/group");
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join("example.yaml"), "[]").unwrap();
        let cache = Arc::new(Cache::default());
        let read = || crate::graph::graph_sources_in(repo.path()).unwrap();
        let (before, dependencies) = cache.capture(read);
        assert!(before.contains_key("group/example"));
        let target = repo.path().join("target");
        std::fs::rename(&directory, &target).unwrap();
        std::os::unix::fs::symlink(&target, &directory).unwrap();
        assert!(read().is_empty());
        cache.refresh_all();
        assert!(dependencies.changed());
        assert!(cache.capture(read).0.is_empty());
    }

    #[test]
    #[cfg(not(target_os = "macos"))]
    fn process_stop_changes_observed_state_without_a_store() {
        let mut child = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .unwrap();
        let pid = child.id();
        let cache = Arc::new(Cache::default());
        let (_, dependencies) = cache.capture(|| super::process(pid).unwrap());
        // SAFETY: this is our live child; waitpid waits for its stop notification.
        unsafe {
            assert_eq!(libc::kill(pid as i32, libc::SIGSTOP), 0);
            let mut status = 0;
            assert_eq!(
                libc::waitpid(pid as i32, &mut status, libc::WUNTRACED),
                pid as i32
            );
        }
        cache.refresh_all();
        let stopped_changed = dependencies.changed();
        let (stopped, _) = cache.capture(|| super::process(pid).unwrap().unwrap());
        child.kill().unwrap();
        child.wait().unwrap();
        assert!(
            stopped_changed,
            "stopping a live process must wake activity views"
        );
        assert!(stopped.kernel_state.starts_with('T'), "{stopped:?}");
    }

    #[test]
    fn process_exit_changes_the_observed_identity_without_a_store() {
        let mut child = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .unwrap();
        let pid = child.id();
        let started = crate::process::os_process::process_started_at(pid)
            .unwrap()
            .unwrap();
        let cache = Arc::new(Cache::default());
        let (before, dependencies) = cache.capture(|| process(pid, started));
        child.kill().unwrap();
        child.wait().unwrap();
        assert_eq!(before, ProcessIdentityEvidence::Live);
        assert_eq!(process(pid, started), ProcessIdentityEvidence::Dead);
        cache.refresh_all();
        assert!(dependencies.changed());
        assert_eq!(
            cache.capture(|| process(pid, started)).0,
            ProcessIdentityEvidence::Dead
        );
    }
    fn key(checkout: &str) -> Derived {
        Derived::Settings {
            checkout: Some(checkout.into()),
            repository: None,
            wave: None,
            outside: crate::config::settings::outside().unwrap(),
        }
    }

    #[test]
    fn a_derived_value_is_shared_until_a_fact_it_read_changes() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file");
        std::fs::write(&path, "before").unwrap();
        let cache = Arc::new(Cache::default());
        let key = key("shared");
        let text = || derived(key.clone(), || read_to_string(&path).unwrap());
        let (first, dependencies) = cache.capture(text);
        let (second, unchanged) = cache.capture(text);
        assert!(Arc::ptr_eq(&first, &second));

        std::fs::write(&path, "after").unwrap();
        // No reading is active: computed inline from the file as it is now.
        assert_eq!(*text(), "after");
        assert_eq!(*cache.capture(text).0, "before");
        cache.refresh_all();
        assert!(dependencies.changed() && unchanged.changed());
        let (third, dependencies) = cache.capture(text);
        assert_eq!(*third, "after");
        cache.refresh_all();
        assert!(!dependencies.changed());
        assert_eq!(cache.computations(|computed| *computed == key), 2);
    }

    #[test]
    fn a_bare_reading_computes_a_derived_value_once_and_keeps_nothing() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file");
        std::fs::write(&path, "before").unwrap();
        let computed = std::sync::atomic::AtomicUsize::new(0);
        let text = || {
            derived(key("bare"), || {
                computed.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                read_to_string(&path).unwrap()
            })
        };
        assert_eq!(*super::reading(|| (text(), text()).1), "before");
        assert_eq!(computed.load(std::sync::atomic::Ordering::Relaxed), 1);
        std::fs::write(&path, "after").unwrap();
        assert_eq!(*super::reading(text), "after");
    }

    #[test]
    fn a_fact_under_a_nested_derived_value_wakes_every_reading_above_it() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file");
        std::fs::write(&path, "before").unwrap();
        let cache = Arc::new(Cache::default());
        let (inner, outer) = (key("inner"), key("outer"));
        let text = || {
            derived(outer.clone(), || {
                let inner = derived(inner.clone(), || read_to_string(&path).unwrap());
                format!("outer {inner}")
            })
        };
        let (_, computed) = cache.capture(text);
        let (_, shared) = cache.capture(text);
        std::fs::write(&path, "after").unwrap();
        cache.refresh_all();
        assert!(computed.changed(), "the reading that computed both");
        assert!(shared.changed(), "a reading that was handed the value");
        assert_eq!(*cache.capture(text).0, "outer after");
        assert_eq!(cache.computations(|computed| *computed == inner), 2);
        assert_eq!(cache.computations(|computed| *computed == outer), 2);
    }

    #[test]
    fn workers_of_one_reading_compute_a_derived_value_once() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file");
        std::fs::write(&path, "text").unwrap();
        let cache = Arc::new(Cache::default());
        let key = key("workers");
        let (values, dependencies) = cache.capture(|| {
            std::thread::scope(|scope| {
                let workers: Vec<_> = (0..4)
                    .map(|_| {
                        scope.spawn(inherit(|| {
                            derived(key.clone(), || read_to_string(&path).unwrap())
                        }))
                    })
                    .collect();
                workers
                    .into_iter()
                    .map(|worker| worker.join().unwrap())
                    .collect::<Vec<_>>()
            })
        });
        assert!(values.iter().all(|value| Arc::ptr_eq(value, &values[0])));
        assert_eq!(cache.computations(|computed| *computed == key), 1);
        std::fs::write(&path, "changed").unwrap();
        cache.refresh_all();
        assert!(dependencies.changed());
    }

    #[test]
    fn a_computation_that_panics_leaves_its_value_to_the_next_caller() {
        let cache = Arc::new(Cache::default());
        let key = key("panics");
        let failed = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cache.capture(|| derived(key.clone(), || -> u32 { panic!("failed computation") }))
        }));
        assert!(failed.is_err());
        assert_eq!(*cache.capture(|| derived(key.clone(), || 7)).0, 7);
        assert_eq!(*cache.capture(|| derived(key.clone(), || 8)).0, 7);
    }

    fn session_projection_fixture(
        pid: u32,
    ) -> (
        tempfile::TempDir,
        loopflow_storage::sqlite::SqliteStore,
        loopflow_model::durable::RepositoryId,
    ) {
        let home = tempfile::tempdir().unwrap();
        let store = crate::store::opening::open_ephemeral(&home.path().join("db")).unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(include_str!("../tests/fixtures/saved_selection_work.sql"))
            .unwrap();
        store.test_session("session", &crate::session_record::new_artifact_key());
        store.conn.lock().unwrap().execute("UPDATE agent_sessions SET repo='/repo',task_id='task_22222222222222222222222222222222',wave_id='11111111-1111-1111-1111-111111111111' WHERE id='session'", []).unwrap();
        let attachment = crate::process::attachment::claim_session_attachment(
            &store,
            "session",
            None,
            &store.test_lf_process(),
        )
        .unwrap();
        store
            .record_agent_process_identity(
                "session",
                &attachment,
                pid,
                crate::process::os_process::process_started_at(pid)
                    .unwrap()
                    .unwrap(),
            )
            .unwrap();
        store
            .record_session_activity(
                "session",
                &attachment,
                &crate::session::SessionActivity {
                    preview: None,
                    observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                    open_tools: 0,
                    pending_input: 0,
                    yielded: false,
                },
            )
            .unwrap();
        let repo =
            loopflow_model::durable::RepositoryId::parse("repo_11111111111111111111111111111111")
                .unwrap();
        (home, store, repo)
    }

    #[test]
    fn quiet_deadline_repages_sessions_and_task_selections_without_a_write() {
        use crate::session::SessionFilter;
        use loopflow_storage::work::selection::Selection;
        let (_home, store, repo) = session_projection_fixture(std::process::id());
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let filter = SessionFilter {
            available: true,
            limit: 1,
            ..Default::default()
        };
        let selection = Selection::parse("state:available", None).unwrap();
        let cache = Arc::new(Cache::default());
        let revision = store.revisions().unwrap();
        let read = || {
            (
                crate::store::session_projection::session_summaries(&store, &filter, now).unwrap(),
                crate::store::selections::resolve_selection(&store, &repo, &selection).unwrap(),
            )
        };
        let (before, dependencies) = cache.capture(read);
        assert!(before.0.is_empty());
        assert!(before.1.members.is_empty());
        // Advance the observed deadline deterministically, without advancing SQL.
        let mut advanced = false;
        for (fact, slot) in cache.entries.lock().unwrap().iter() {
            if matches!(fact, super::Fact::Deadline(_)) {
                let mut entry = slot.entry.lock().unwrap();
                entry.answer = Some(super::Answer::Bool(true));
                entry.revision += 1;
                advanced = true;
            }
        }
        assert!(advanced, "the read must register its quiet deadline");
        assert!(dependencies.changed());
        let (after, _) = cache.capture(read);
        assert_eq!(after.0[0].id.as_str(), "session");
        assert_eq!(
            after.1.members[0].work.id,
            "task_22222222222222222222222222222222"
        );
        assert_eq!(store.revisions().unwrap(), revision);
    }

    #[test]
    fn process_death_invalidates_session_pages_and_task_selections_without_a_write() {
        use crate::session::SessionFilter;
        use loopflow_storage::work::selection::Selection;
        struct Child(std::process::Child);
        impl Drop for Child {
            fn drop(&mut self) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
        let mut child = Child(
            std::process::Command::new("/bin/sleep")
                .arg("60")
                .spawn()
                .unwrap(),
        );
        let (_home, store, repo) = session_projection_fixture(child.0.id());
        store
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE session_activity SET observed_at=1", [])
            .unwrap();
        let filter = SessionFilter {
            available: true,
            limit: 1,
            ..Default::default()
        };
        let selection = Selection::parse("state:available", None).unwrap();
        let cache = Arc::new(Cache::default());
        let revision = store.revisions().unwrap();
        let read = || {
            (
                crate::store::session_projection::session_summaries(&store, &filter, 500).unwrap(),
                crate::store::selections::resolve_selection(&store, &repo, &selection).unwrap(),
            )
        };
        let (before, dependencies) = cache.capture(read);
        assert_eq!(before.0.len(), 1);
        assert_eq!(before.1.members.len(), 1);
        child.0.kill().unwrap();
        child.0.wait().unwrap();
        cache.refresh_all();
        assert!(dependencies.changed());
        let (after, _) = cache.capture(read);
        assert!(after.0.is_empty());
        assert!(after.1.members.is_empty());
        assert!(after.1.unavailable.is_empty(), "proven death is known");
        assert_eq!(store.revisions().unwrap(), revision);
    }

    #[test]
    fn unheld_derived_caches_without_reading_and_sees_external_edit_immediately() {
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let home = tempfile::tempdir().unwrap();
        let _guard =
            crate::config::environment::EnvVarGuard::set("LF_HOME", home.path().to_str().unwrap());
        let config_path = home.path().join("config.yaml");
        std::fs::write(&config_path, "agent: claude\n").unwrap();

        let outside = crate::config::settings::outside().unwrap();
        let key = Derived::GlobalSettings { outside };
        let before_count = super::PROCESS_CACHE.computations(|k| k == &key);

        let first = derived(key.clone(), || {
            read_to_string(&config_path).unwrap_or_default()
        });
        assert_eq!(first.as_str(), "agent: claude\n");
        assert_eq!(
            super::PROCESS_CACHE.computations(|k| k == &key),
            before_count + 1
        );

        // Second call without a READING reuses the cached Arc without recomputing.
        let second = derived(key.clone(), || {
            read_to_string(&config_path).unwrap_or_default()
        });
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(
            super::PROCESS_CACHE.computations(|k| k == &key),
            before_count + 1
        );

        // An external write 1 ms later is detected immediately via inline stat.
        std::thread::sleep(std::time::Duration::from_millis(2));
        std::fs::write(&config_path, "agent: codex\n").unwrap();
        let third = derived(key.clone(), || {
            read_to_string(&config_path).unwrap_or_default()
        });
        assert_eq!(third.as_str(), "agent: codex\n");
        assert_eq!(
            super::PROCESS_CACHE.computations(|k| k == &key),
            before_count + 2
        );
    }

    #[test]
    fn unheld_git_fact_is_not_rerun_after_its_interval() {
        let repo = loopflow_test_support::TestRepo::new();
        let cache = Arc::new(Cache::default());
        let status = || git(repo.path(), &["status", "--porcelain"]).unwrap().stdout;
        let first = cache.read_once(status);
        assert!(first.is_empty());
        assert_eq!(cache.held_facts(), 0);
        let revision_before = cache
            .entries
            .lock()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .entry
            .lock()
            .unwrap()
            .revision;

        // Write a file so a re-run of git status would observe a change.
        std::fs::write(repo.path().join("unheld.txt"), "text").unwrap();
        // Advance past the 3 s Git interval: unheld facts are never background-polled.
        cache.refresh_due_at(std::time::SystemTime::now() + std::time::Duration::from_secs(10));
        let revision_after = cache
            .entries
            .lock()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .entry
            .lock()
            .unwrap()
            .revision;
        assert_eq!(
            revision_after, revision_before,
            "unheld Git fact must not re-run in background refresh"
        );
        // Next inline read past the interval observes the fresh state on use.
        for slot in cache.entries.lock().unwrap().values() {
            slot.entry.lock().unwrap().checked =
                std::time::SystemTime::now() - std::time::Duration::from_secs(10);
        }
        let fresh = cache.read_once(status);
        assert!(String::from_utf8_lossy(&fresh).contains("unheld.txt"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn held_git_fact_reruns_after_checkout_write_and_not_before() {
        use std::time::{Duration, Instant, SystemTime};
        let repo = loopflow_test_support::TestRepo::new();
        let cache = Cache::with_fs_watcher();
        let status = || git(repo.path(), &["status", "--porcelain"]).unwrap().stdout;
        let (first, dependencies) = cache.capture(status);
        assert!(first.is_empty());
        assert_eq!(cache.held_facts(), 1);
        let checked_before = cache
            .entries
            .lock()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .entry
            .lock()
            .unwrap()
            .checked;

        // Nothing written under the checkout: advancing the clock past the 3 s
        // throttle must NOT re-run git.
        let next = cache.refresh_due_at(SystemTime::now() + Duration::from_secs(10));
        let checked_after = cache
            .entries
            .lock()
            .unwrap()
            .values()
            .next()
            .unwrap()
            .entry
            .lock()
            .unwrap()
            .checked;
        assert_eq!(
            checked_after, checked_before,
            "quiet held Git checkout must not re-run git"
        );
        assert!(
            next.is_none(),
            "quiet held Git checkout parks refresh thread: {next:?}"
        );
        assert!(!dependencies.changed());

        // Write a file under the checkout; FSEvents marks the checkout dirty.
        std::fs::write(repo.path().join("edited.txt"), "hello").unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            cache.refresh_due_at(SystemTime::now() + Duration::from_secs(4));
            if dependencies.changed() {
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(
            dependencies.changed(),
            "held Git fact must refresh after a write under its checkout"
        );
        let updated = cache.capture(status).0;
        assert!(String::from_utf8_lossy(&updated).contains("edited.txt"));
    }
}
