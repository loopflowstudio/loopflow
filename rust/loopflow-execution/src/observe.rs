//! External facts shared by served reads. Bare reads use the same observers
//! inline; only a bounded reading scope retains answers and dependencies.
use std::any::Any;
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, LazyLock, Mutex, Weak};
use std::time::{Duration, SystemTime};

use crate::process::OsProcess;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum Fact {
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

#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    Local,
    Git,
    Github,
}

impl Fact {
    fn source(&self) -> Source {
        match self {
            Self::Git(..) => Source::Git,
            Self::Github(..) => Source::Github,
            _ => Source::Local,
        }
    }

    fn interval(&self) -> Duration {
        Duration::from_secs(match self {
            Self::Git(..) => 3,
            Self::Github(..) | Self::Clock => 60,
            // Transcript watches say when to look sooner; these bound a missed event.
            Self::Turn(..) => TURN_RECONCILE.as_secs(),
            Self::RawConversations => 10,
            _ => 1,
        })
    }

    fn observe(&self) -> Answer {
        match self {
            Self::Git(path, args) | Self::Github(path, args) => {
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
const TURN_RECONCILE: Duration = Duration::from_secs(30);
/// A transcript growing steadily is read again at most this often.
const TURN_HINTED: Duration = Duration::from_secs(1);

#[derive(Debug)]
struct Entry {
    answer: Option<Answer>,
    revision: u64,
    checked: SystemTime,
    used: SystemTime,
}

type SharedEntry = Arc<Mutex<Entry>>;

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
}

/// The entries and revisions read by one invocation, including bounded workers.
#[derive(Debug, Clone, Default)]
pub struct Dependencies(Arc<Mutex<Observed>>);

/// A derived value's dependencies are held by reference: using a value costs
/// one entry here, whatever its computation read.
#[derive(Debug, Default)]
struct Observed {
    facts: HashMap<Fact, (SharedEntry, u64)>,
    derived: HashMap<Derived, Dependencies>,
}

impl Dependencies {
    pub fn changed(&self) -> bool {
        let observed = self.0.lock().expect("observed dependencies");
        observed
            .facts
            .values()
            .any(|(entry, revision)| entry.lock().expect("observed fact").revision != *revision)
            || observed.derived.values().any(Dependencies::changed)
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

#[derive(Debug, Default)]
pub struct Cache {
    entries: Mutex<HashMap<Fact, SharedEntry>>,
    derived: Mutex<HashMap<Derived, Slot>>,
    #[cfg(test)]
    computations: Mutex<Vec<Derived>>,
}

static SHARED: LazyLock<Mutex<Weak<Cache>>> = LazyLock::new(Mutex::default);

impl Cache {
    pub fn shared() -> Arc<Self> {
        let mut shared = SHARED.lock().expect("observation cache");
        if let Some(cache) = shared.upgrade() {
            return cache;
        }
        let cache = Arc::new(Self::default());
        *shared = Arc::downgrade(&cache);
        // Slow GitHub requests cannot postpone local files or process exits.
        for source in [Source::Local, Source::Git, Source::Github] {
            let weak = Arc::downgrade(&cache);
            std::thread::spawn(move || loop {
                std::thread::sleep(Duration::from_millis(250));
                let Some(cache) = weak.upgrade() else { break };
                cache.refresh(source);
            });
        }
        cache
    }

    fn refresh(&self, source: Source) {
        // A value leaves like a fact: nothing holds it and nothing asked lately.
        // A slot only this map holds is unlocked; one a panic emptied goes too.
        self.derived
            .lock()
            .expect("derived values")
            .retain(|_, slot| {
                Arc::strong_count(slot) > 1
                    || slot.lock().is_ok_and(|computed| {
                        computed.as_ref().is_some_and(|computed| {
                            Arc::strong_count(&computed.dependencies.0) > 1
                                || computed.used.elapsed().unwrap_or_default()
                                    < Duration::from_secs(300)
                        })
                    })
            });
        let entries = {
            let mut entries = self.entries.lock().expect("observation cache");
            entries.retain(|_, entry| {
                Arc::strong_count(entry) > 1
                    || entry.try_lock().map_or(true, |entry| {
                        entry.used.elapsed().unwrap_or_default() < Duration::from_secs(300)
                    })
            });
            entries
                .iter()
                .filter(|(fact, _)| fact.source() == source)
                .map(|(fact, entry)| (fact.clone(), entry.clone()))
                .collect::<Vec<_>>()
        };
        for (fact, entry) in entries {
            if entry.try_lock().map_or(true, |entry| {
                entry.answer.is_none()
                    || entry.checked.elapsed().unwrap_or_default() < fact.interval()
            }) {
                continue;
            }
            let answer = fact.observe();
            let mut entry = entry.lock().expect("observed fact");
            if entry.answer.as_ref() != Some(&answer) {
                entry.answer = Some(answer);
                entry.revision += 1;
            }
            entry.checked = SystemTime::now();
        }
    }

    /// A transcript at `path` changed: read its turn again within a second.
    /// `created` also looks again for raw conversations; `None` means events
    /// were lost and every transcript is due.
    pub(crate) fn hint_transcript(&self, path: Option<&Path>, created: bool) {
        let due = SystemTime::now() - (TURN_RECONCILE - TURN_HINTED);
        let entries = self.entries.lock().expect("observation cache");
        for (fact, entry) in entries.iter() {
            let hinted = match fact {
                Fact::Turn(_, watched) => path.is_none_or(|path| watched.path() == path),
                Fact::RawConversations => created || path.is_none(),
                _ => false,
            };
            if hinted {
                let mut entry = entry.lock().expect("observed fact");
                entry.checked = entry.checked.min(due);
            }
        }
    }

    /// Run one refresh of local facts now, as the background thread does.
    #[cfg(any(test, feature = "test-support"))]
    pub fn refresh_local(&self) {
        self.refresh(Source::Local);
    }

    /// Observe every fact again now, whatever its interval.
    #[cfg(test)]
    pub(crate) fn refresh_all(&self) {
        for entry in self.entries.lock().unwrap().values() {
            entry.lock().unwrap().checked = SystemTime::now() - Duration::from_secs(61);
        }
        for source in [Source::Local, Source::Git, Source::Github] {
            self.refresh(source);
        }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn with_raw_conversations(raw: Vec<crate::transcript::raw::RawDirectory>) -> Arc<Self> {
        Arc::new(Self {
            entries: Mutex::new(HashMap::from([(
                Fact::RawConversations,
                Arc::new(Mutex::new(Entry {
                    answer: Some(Answer::Raw(raw)),
                    revision: 0,
                    checked: SystemTime::now(),
                    used: SystemTime::now(),
                })),
            )])),
            ..Self::default()
        })
    }

    pub fn capture<T>(self: &Arc<Self>, read: impl FnOnce() -> T) -> (T, Dependencies) {
        let dependencies = Dependencies::default();
        let result = scope(
            Some(Reading {
                cache: self.clone(),
                dependencies: dependencies.clone(),
            }),
            read,
        );
        (result, dependencies)
    }

    fn read(&self, fact: Fact, dependencies: &Dependencies) -> Answer {
        let entry = self
            .entries
            .lock()
            .expect("observation cache")
            .entry(fact.clone())
            .or_insert_with(|| {
                Arc::new(Mutex::new(Entry {
                    answer: None,
                    revision: 0,
                    checked: SystemTime::now(),
                    used: SystemTime::now(),
                }))
            })
            .clone();
        let mut observed = entry.lock().expect("observed fact");
        if observed.answer.is_none() {
            observed.answer = Some(fact.observe());
            observed.checked = SystemTime::now();
        }
        observed.used = SystemTime::now();
        let revision = observed.revision;
        let answer = observed
            .answer
            .clone()
            .expect("fact observed before reading");
        drop(observed);
        // Keep the first revision so a change during the reading stays visible.
        dependencies
            .0
            .lock()
            .expect("observed dependencies")
            .facts
            .entry(fact)
            .or_insert((entry, revision));
        answer
    }

    /// How many times values matching `key` were computed.
    #[cfg(test)]
    pub(crate) fn computations(&self, key: impl Fn(&Derived) -> bool) -> usize {
        let computations = self.computations.lock().unwrap();
        computations.iter().filter(|computed| key(computed)).count()
    }
}

#[derive(Clone)]
struct Reading {
    cache: Arc<Cache>,
    dependencies: Dependencies,
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

pub fn active() -> bool {
    READING.with(|reading| reading.borrow().is_some())
}

/// Resolve a proposed file edit through the ordinary readers without publishing
/// the draft in the serving Machine's observation cache.
pub(crate) fn preview_files<T>(files: &[(PathBuf, String)], read: impl FnOnce() -> T) -> T {
    let cache = Cache::default();
    for (path, text) in files {
        cache.entries.lock().expect("private preview cache").insert(
            Fact::File(absolute(path)),
            Arc::new(Mutex::new(Entry {
                answer: Some(Answer::File(Ok(text.clone()))),
                revision: 0,
                used: SystemTime::now(),
                checked: SystemTime::now(),
            })),
        );
    }
    Arc::new(cache).capture(read).0
}

/// Run `read` as one reading: the served reading already active, else one
/// that lasts for this call, so a bare command observes each fact and
/// computes each derived value once however often `read` asks.
pub fn reading<T>(read: impl FnOnce() -> T) -> T {
    if active() {
        read()
    } else {
        Arc::new(Cache::default()).capture(read).0
    }
}

pub fn inherit<T>(read: impl FnOnce() -> T) -> impl FnOnce() -> T {
    let reading = READING.with(|reading| reading.borrow().clone());
    loopflow_storage::instrumentation::inherit(move || scope(reading, read))
}

/// `compute`'s result, shared by served readings until a fact it observed
/// changes. With no reading active it is computed inline and nothing is kept.
///
/// `compute` reads observed facts and `key`, nothing else: an input from the
/// environment, a thread-local or the store that is missing from `key` leaves
/// the value stale until one of its files changes.
///
/// # Panics
///
/// When two computations of different types share one `key`.
pub fn derived<T: Send + Sync + 'static>(key: Derived, compute: impl FnOnce() -> T) -> Arc<T> {
    let Some(reading) = READING.with(|reading| reading.borrow().clone()) else {
        return Arc::new(compute());
    };
    let slot = reading
        .cache
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
    if computed
        .as_ref()
        .is_none_or(|computed| computed.dependencies.changed())
    {
        let dependencies = Dependencies::default();
        let value = scope(
            Some(Reading {
                cache: reading.cache.clone(),
                dependencies: dependencies.clone(),
            }),
            compute,
        );
        #[cfg(test)]
        reading.cache.computations.lock().unwrap().push(key.clone());
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
    reading
        .dependencies
        .0
        .lock()
        .expect("observed dependencies")
        .derived
        .entry(key)
        .or_insert_with(|| computed.dependencies.clone());
    computed
        .value
        .clone()
        .downcast()
        .unwrap_or_else(|_| panic!("a derived key names one type"))
}

fn read(fact: Fact) -> Answer {
    let reading = READING.with(|reading| reading.borrow().clone());
    match reading {
        Some(reading) => reading.cache.read(fact, &reading.dependencies),
        None => fact.observe(),
    }
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_owned())
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
        for (fact, entry) in cache.entries.lock().unwrap().iter() {
            if matches!(fact, super::Fact::Deadline(_)) {
                let mut entry = entry.lock().unwrap();
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
}
