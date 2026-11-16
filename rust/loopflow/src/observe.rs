//! External facts shared by served reads. Bare reads use the same observers
//! inline; only a bounded reading scope retains answers and dependencies.
use std::cell::RefCell;
use std::collections::HashMap;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::{Arc, LazyLock, Mutex, Weak};
use std::time::{Duration, SystemTime};

use crate::journal::OsProcess;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

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
    Turn(crate::harness::HarnessKind, crate::transcript::Source),
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
    Turn(Option<crate::transcript::live::SessionTurn>),
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
            Self::Turn(harness, source) => {
                Answer::Turn(crate::transcript::live::read_turn(*harness, source))
            }
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
    id: String,
    answer: Answer,
    revision: u64,
    checked: SystemTime,
    used: SystemTime,
}

type SharedEntry = Arc<Mutex<Entry>>;

#[derive(Debug, Clone)]
struct Captured {
    entry: SharedEntry,
    token: u64,
    answer: Arc<Answer>,
}

/// The answers and tokens read by one invocation, including bounded workers.
#[derive(Debug, Clone, Default)]
pub(crate) struct Dependencies(Arc<Mutex<HashMap<Fact, Captured>>>);

impl Dependencies {
    pub(crate) fn changed(&self) -> bool {
        self.0
            .lock()
            .expect("observed dependencies")
            .values()
            .any(|captured| {
                captured.entry.lock().expect("observed fact").revision != captured.token
            })
    }
}

/// A coherent capture from one ephemeral observation source. IDs reveal no paths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObservationEvidence {
    pub source: String,
    pub validated_at: u64,
    pub dependencies: Vec<ObservationDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ObservationDependency {
    pub fact: String,
    pub token: u64,
}

#[derive(Debug)]
pub(crate) struct Cache {
    incarnation: String,
    // Serializes answer installation, eviction and validation, not observation I/O.
    clock: Mutex<u64>,
    entries: Mutex<HashMap<Fact, SharedEntry>>,
}

impl Default for Cache {
    fn default() -> Self {
        Self {
            incarnation: uuid::Uuid::new_v4().to_string(),
            clock: Mutex::new(0),
            entries: Mutex::new(HashMap::new()),
        }
    }
}

static SHARED: LazyLock<Mutex<Weak<Cache>>> = LazyLock::new(Mutex::default);

impl Cache {
    pub(crate) fn incarnation(&self) -> &str {
        &self.incarnation
    }

    /// Validate the whole consumed set while answer replacement is excluded.
    pub(crate) fn evidence(&self, dependencies: &Dependencies) -> Option<ObservationEvidence> {
        let captured = dependencies.0.lock().expect("observed dependencies");
        let clock = self.clock.lock().expect("observation clock");
        let entries = self.entries.lock().expect("observation cache");
        let mut tokens = Vec::with_capacity(captured.len());
        for (fact, captured) in captured.iter() {
            if !entries
                .get(fact)
                .is_some_and(|current| Arc::ptr_eq(current, &captured.entry))
            {
                return None;
            }
            let entry = captured.entry.lock().expect("observed fact");
            if entry.revision != captured.token {
                return None;
            }
            tokens.push(ObservationDependency {
                fact: entry.id.clone(),
                token: captured.token,
            });
        }
        tokens.sort_by(|a, b| a.fact.cmp(&b.fact));
        Some(ObservationEvidence {
            source: self.incarnation.clone(),
            validated_at: *clock,
            dependencies: tokens,
        })
    }

    pub(crate) fn shared() -> Arc<Self> {
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
        let entries = {
            let _clock = self.clock.lock().expect("observation clock");
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
                entry.checked.elapsed().unwrap_or_default() < fact.interval()
            }) {
                continue;
            }
            let answer = fact.observe();
            let mut clock = self.clock.lock().expect("observation clock");
            let entries = self.entries.lock().expect("observation cache");
            if !entries
                .get(&fact)
                .is_some_and(|current| Arc::ptr_eq(current, &entry))
            {
                continue;
            }
            let mut entry = entry.lock().expect("observed fact");
            if entry.answer != answer {
                entry.answer = answer;
                *clock = clock.checked_add(1).expect("observation clock exhausted");
                entry.revision = *clock;
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
    #[cfg(test)]
    pub(crate) fn refresh_local(&self) {
        self.refresh(Source::Local);
    }

    #[cfg(test)]
    pub(crate) fn with_raw_conversations(
        raw: Vec<crate::transcript::raw::RawDirectory>,
    ) -> Arc<Self> {
        Arc::new(Self {
            entries: Mutex::new(HashMap::from([(
                Fact::RawConversations,
                Arc::new(Mutex::new(Entry {
                    id: uuid::Uuid::new_v4().to_string(),
                    answer: Answer::Raw(raw),
                    revision: 1,
                    checked: SystemTime::now(),
                    used: SystemTime::now(),
                })),
            )])),
            clock: Mutex::new(1),
            ..Self::default()
        })
    }

    pub(crate) fn capture<T>(self: &Arc<Self>, read: impl FnOnce() -> T) -> (T, Dependencies) {
        let dependencies = Dependencies::default();
        let result = scope(
            Some(Reading {
                cache: self.clone(),
                dependencies: dependencies.clone(),
                fields: Vec::new(),
            }),
            read,
        );
        (result, dependencies)
    }

    fn read(&self, fact: Fact, dependencies: &Dependencies) -> Answer {
        // One captured answer per invocation, also shared by inherited workers.
        if let Some(captured) = dependencies
            .0
            .lock()
            .expect("observed dependencies")
            .get(&fact)
        {
            return captured.answer.as_ref().clone();
        }
        let entry = {
            let entries = self.entries.lock().expect("observation cache");
            entries.get(&fact).cloned()
        };
        // Retaining an existing entry keeps eviction from removing it while
        // installation waits. Only absent entries need observation I/O.
        let initial = entry.is_none().then(|| scope(None, || fact.observe()));
        let mut captured = dependencies.0.lock().expect("observed dependencies");
        // Another worker may have installed this invocation's first answer
        // while we observed. Every worker consumes that same captured answer.
        if let Some(captured) = captured.get(&fact) {
            return captured.answer.as_ref().clone();
        }
        let mut clock = self.clock.lock().expect("observation clock");
        let mut entries = self.entries.lock().expect("observation cache");
        let entry = entries
            .entry(fact.clone())
            .or_insert_with(|| {
                *clock = clock.checked_add(1).expect("observation clock exhausted");
                Arc::new(Mutex::new(Entry {
                    id: uuid::Uuid::new_v4().to_string(),
                    answer: initial.expect("absent fact observed before installation"),
                    revision: *clock,
                    checked: SystemTime::now(),
                    used: SystemTime::now(),
                }))
            })
            .clone();
        let mut observed = entry.lock().expect("observed fact");
        observed.used = SystemTime::now();
        let revision = observed.revision;
        let answer = observed.answer.clone();
        drop(observed);
        captured.insert(
            fact,
            Captured {
                entry,
                token: revision,
                answer: Arc::new(answer.clone()),
            },
        );
        answer
    }
}

#[derive(Clone)]
struct Reading {
    cache: Arc<Cache>,
    dependencies: Dependencies,
    fields: Vec<Dependencies>,
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

/// A field records consumed inputs without creating another answer cache.
#[derive(Debug, Clone)]
pub(crate) struct FieldCapture {
    cache: Arc<Cache>,
    dependencies: Dependencies,
}

impl FieldCapture {
    pub(crate) fn evidence(&self) -> Option<ObservationEvidence> {
        self.cache.evidence(&self.dependencies)
    }
}

pub(crate) fn field<T>(read: impl FnOnce() -> T) -> (T, Option<FieldCapture>) {
    let Some(mut reading) = READING.with(|reading| reading.borrow().clone()) else {
        return (read(), None);
    };
    let capture = FieldCapture {
        cache: reading.cache.clone(),
        dependencies: Dependencies::default(),
    };
    reading.fields.push(capture.dependencies.clone());
    (scope(Some(reading), read), Some(capture))
}

pub(crate) fn active() -> bool {
    READING.with(|reading| reading.borrow().is_some())
}

pub(crate) fn inherit<T>(read: impl FnOnce() -> T) -> impl FnOnce() -> T {
    let reading = READING.with(|reading| reading.borrow().clone());
    move || scope(reading, read)
}

fn read(fact: Fact) -> Answer {
    let reading = READING.with(|reading| reading.borrow().clone());
    match reading {
        Some(reading) => {
            let answer = reading.cache.read(fact.clone(), &reading.dependencies);
            if reading.fields.is_empty() {
                return answer;
            }
            let captured = reading
                .dependencies
                .0
                .lock()
                .expect("observed dependencies")
                .get(&fact)
                .expect("read captured its answer")
                .clone();
            for field in &reading.fields {
                field
                    .0
                    .lock()
                    .expect("field dependencies")
                    .insert(fact.clone(), captured.clone());
            }
            answer
        }
        None => fact.observe(),
    }
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_owned())
}

pub(crate) fn git(path: &Path, args: &[&str]) -> io::Result<Output> {
    let Answer::Output(answer) = read(Fact::Git(
        absolute(path),
        args.iter().map(|s| s.to_string()).collect(),
    )) else {
        unreachable!("Git returns output")
    };
    restored(answer)
}

pub(crate) fn github(path: &Path, args: &[&str]) -> io::Result<Output> {
    let Answer::Output(answer) = read(Fact::Github(
        absolute(path),
        args.iter().map(|s| s.to_string()).collect(),
    )) else {
        unreachable!("GitHub returns output")
    };
    restored(answer)
}

pub(crate) fn read_to_string(path: &Path) -> io::Result<String> {
    let Answer::File(answer) = read(Fact::File(absolute(path))) else {
        unreachable!("file returns text")
    };
    restored(answer)
}

pub(crate) fn read_dir(path: &Path) -> io::Result<Vec<PathBuf>> {
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

pub(crate) fn try_exists(path: &Path) -> io::Result<bool> {
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

pub(crate) fn exists(path: &Path) -> bool {
    metadata(path).is_ok()
}
pub(crate) fn is_dir(path: &Path) -> bool {
    metadata(path).is_ok_and(|metadata| metadata.kind.is_dir())
}
pub(crate) fn is_file(path: &Path) -> bool {
    metadata(path).is_ok_and(|metadata| metadata.kind.is_file())
}

pub(crate) fn is_symlink(path: &Path) -> bool {
    metadata(path).is_ok_and(|metadata| metadata.symlink)
}

pub(crate) fn process(pid: u32) -> io::Result<Option<OsProcess>> {
    let Answer::Process(answer) = read(Fact::Process(pid)) else {
        unreachable!("process returns OS identity")
    };
    restored(answer)
}

/// The current or last turn of a conversation's transcript.
pub(crate) fn transcript_turn(
    harness: crate::harness::HarnessKind,
    source: &crate::transcript::Source,
) -> Option<crate::transcript::live::SessionTurn> {
    use crate::transcript::Source;
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
pub(crate) fn now() -> time::OffsetDateTime {
    let Answer::Time(at) = read(Fact::Clock) else {
        unreachable!("clock returns time")
    };
    time::OffsetDateTime::from_unix_timestamp(at).expect("observed system timestamp")
}

/// Register the next change of a time-dependent result without ticking the read.
pub(crate) fn deadline(at: i64) -> bool {
    let Answer::Bool(answer) = read(Fact::Deadline(at)) else {
        unreachable!("deadline returns a boolean")
    };
    answer
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::{Duration, SystemTime};

    use super::{file_bytes, git, inherit, read_to_string, Cache, Source};
    use crate::journal::{process_identity_evidence as process, ProcessIdentityEvidence};

    fn refresh(cache: &Cache) {
        for entry in cache.entries.lock().unwrap().values() {
            entry.lock().unwrap().checked = SystemTime::now() - Duration::from_secs(61);
        }
        for source in [Source::Local, Source::Git, Source::Github] {
            cache.refresh(source);
        }
    }

    #[test]
    fn fields_validate_only_their_consumed_inputs_and_share_invocation_answers() {
        let dir = tempfile::tempdir().unwrap();
        let activity = dir.path().join("activity");
        let other = dir.path().join("other");
        std::fs::write(&activity, "busy").unwrap();
        std::fs::write(&other, "stable").unwrap();
        let cache = Arc::new(Cache::default());
        let ((invalid, independent, empty), invocation) = cache.capture(|| {
            let (_, invalid) = super::field(|| read_to_string(&activity).unwrap());
            let (_, independent) = super::field(|| {
                std::thread::scope(|scope| {
                    scope
                        .spawn(inherit(|| read_to_string(&other).unwrap()))
                        .join()
                        .unwrap()
                })
            });
            let (_, empty) = super::field(|| ());
            std::fs::write(&activity, "idle").unwrap();
            refresh(&cache);
            let (repeated, repeated_capture) = super::field(|| read_to_string(&activity).unwrap());
            assert_eq!(repeated, "busy", "a field must reuse the invocation answer");
            assert!(repeated_capture.unwrap().evidence().is_none());
            (invalid.unwrap(), independent.unwrap(), empty.unwrap())
        });
        assert!(cache.evidence(&invocation).is_none());
        assert!(invalid.evidence().is_none());
        let supported = independent.evidence().unwrap();
        assert_eq!(supported.dependencies.len(), 1);
        assert!(empty.evidence().unwrap().dependencies.is_empty());
        let (_, bare) = super::field(|| ());
        assert!(bare.is_none(), "bare reads cannot certify external absence");
    }

    #[test]
    fn a_capture_reuses_answers_across_changes_and_inherited_workers() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("private-file");
        std::fs::write(&path, "before").unwrap();
        let cache = Arc::new(Cache::default());
        let (_, dependencies) = cache.capture(|| {
            assert_eq!(read_to_string(&path).unwrap(), "before");
            std::fs::write(&path, "after").unwrap();
            refresh(&cache);
            assert_eq!(read_to_string(&path).unwrap(), "before");
            std::thread::scope(|scope| {
                assert_eq!(
                    scope
                        .spawn(inherit(|| read_to_string(&path).unwrap()))
                        .join()
                        .unwrap(),
                    "before"
                );
            });
        });
        assert!(dependencies.changed());
        assert!(
            cache.evidence(&dependencies).is_none(),
            "mixed-time computation cannot normalize"
        );
        let (answer, dependencies) = cache.capture(|| read_to_string(&path).unwrap());
        assert_eq!(answer, "after");
        let evidence = cache.evidence(&dependencies).unwrap();
        assert_eq!(evidence.source, cache.incarnation());
        assert!(!serde_json::to_string(&evidence)
            .unwrap()
            .contains("private-file"));
        // A later change doesn't mutate captured historical evidence.
        std::fs::write(&path, "later").unwrap();
        refresh(&cache);
        let (_, later) = cache.capture(|| read_to_string(&path).unwrap());
        let later = cache.evidence(&later).unwrap();
        assert!(later.validated_at > evidence.validated_at);
        assert_eq!(later.dependencies[0].fact, evidence.dependencies[0].fact);
        assert!(later.dependencies[0].token > evidence.dependencies[0].token);
    }

    #[test]
    fn held_observation_io_does_not_block_an_unrelated_inherited_read() {
        use std::os::unix::fs::PermissionsExt;
        let _env = loopflow_test_support::env_lock().lock().unwrap();
        let directory = tempfile::tempdir().unwrap();
        let entered = directory.path().join("entered");
        let release = directory.path().join("release");
        assert!(std::process::Command::new("mkfifo")
            .args([&entered, &release])
            .status()
            .unwrap()
            .success());
        let executable = directory.path().join("gh");
        std::fs::write(
            &executable,
            "#!/bin/sh\nprintf started > entered\n/bin/cat release >/dev/null\nprintf answer\n",
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();
        let _path = crate::lf::commands::graph::EnvVarGuard::set(
            "PATH",
            &format!(
                "{}:{}",
                directory.path().display(),
                std::env::var("PATH").unwrap()
            ),
        );
        let file = directory.path().join("file");
        std::fs::write(&file, "independent").unwrap();
        let cache = Arc::new(Cache::default());
        cache.capture(|| {
            std::thread::scope(|scope| {
                let held = scope.spawn(inherit(|| {
                    super::github(directory.path(), &["pr", "checks"]).unwrap()
                }));
                assert_eq!(std::fs::read_to_string(&entered).unwrap(), "started");
                let (sender, arrived) = std::sync::mpsc::channel();
                let sibling = scope.spawn(inherit(move || {
                    sender.send(read_to_string(&file).unwrap()).unwrap()
                }));
                let answer = arrived.recv_timeout(Duration::from_secs(5));
                // Release even on failure, so the fixture always joins its command.
                std::fs::write(&release, "release").unwrap();
                assert_eq!(held.join().unwrap().stdout, b"answer");
                sibling.join().unwrap();
                assert_eq!(answer.unwrap(), "independent");
            })
        });
    }

    #[test]
    fn eviction_never_reuses_tokens_and_replacement_has_another_source() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("file");
        std::fs::write(&path, "same").unwrap();
        let cache = Arc::new(Cache::default());
        let (_, dependencies) = cache.capture(|| read_to_string(&path).unwrap());
        let before = cache.evidence(&dependencies).unwrap();
        drop(dependencies);
        for entry in cache.entries.lock().unwrap().values() {
            entry.lock().unwrap().used = SystemTime::now() - Duration::from_secs(301);
        }
        cache.refresh(Source::Local);
        assert!(cache.entries.lock().unwrap().is_empty());
        let (_, dependencies) = cache.capture(|| read_to_string(&path).unwrap());
        let after = cache.evidence(&dependencies).unwrap();
        assert_eq!(before.source, after.source);
        assert!(after.dependencies[0].token > before.dependencies[0].token);
        let replacement = Arc::new(Cache::default());
        let (_, dependencies) = replacement.capture(|| read_to_string(&path).unwrap());
        assert_ne!(
            replacement.evidence(&dependencies).unwrap().source,
            before.source
        );
    }

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
        refresh(&cache);
        assert!(dependencies.changed());
        let (second, dependencies) = cache.capture(status);
        assert_eq!(second, status());
        refresh(&cache);
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
        refresh(&cache);
        assert!(missing.changed());
        assert!(!unrelated.changed());
        assert_eq!(
            cache.capture(|| read_to_string(&first)).0.unwrap(),
            "created"
        );
        // A new reading replaces its dependencies; it does not retain the first path.
        let (_, replaced) = cache.capture(|| read_to_string(&other).unwrap());
        std::fs::remove_file(&first).unwrap();
        refresh(&cache);
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
        refresh(&cache);
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
        refresh(&cache);
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
        let _path = crate::lf::commands::graph::EnvVarGuard::set(
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
        refresh(&cache);
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
        refresh(&cache);
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
        refresh(&cache);
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
        refresh(&cache);
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
        let started = crate::journal::process_started_at(pid).unwrap().unwrap();
        let cache = Arc::new(Cache::default());
        let (before, dependencies) = cache.capture(|| process(pid, started));
        let live = cache.evidence(&dependencies).unwrap();
        child.kill().unwrap();
        child.wait().unwrap();
        assert_eq!(before, ProcessIdentityEvidence::Live);
        assert_eq!(process(pid, started), ProcessIdentityEvidence::Dead);
        refresh(&cache);
        assert!(dependencies.changed());
        let (after, dependencies) = cache.capture(|| process(pid, started));
        assert_eq!(after, ProcessIdentityEvidence::Dead);
        let dead = cache.evidence(&dependencies).unwrap();
        assert_eq!(live.source, dead.source);
        assert_eq!(live.dependencies[0].fact, dead.dependencies[0].fact);
        assert!(dead.dependencies[0].token > live.dependencies[0].token);
    }
}
