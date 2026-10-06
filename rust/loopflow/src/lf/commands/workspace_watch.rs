//! `lf monitor workspace --watch --json`: one foreground reader per window.
//!
//! The store's change revisions say which parts a commit can have changed. A
//! part is projected again only then, on a read-only connection, and sent only
//! when its content differs from the last frame. Bodies are the same wire types
//! the one-shot `--json` reads print. The reader commits nothing, so it never
//! wakes itself, and it records one Exec for its whole lifetime.

mod checkouts;

use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::activity::WorkActivitySnapshot;
use super::top::ActivitySnapshot;
use super::waves::{RoadmapSnapshot, WaveDetailSnapshot, WaveSnapshot};
use crate::ops::human_session::SessionRecord;
use crate::repository::CanonicalRepo;
use crate::store::changes::StoreChanges;
use crate::store::sqlite::StoreRevisions;
use crate::store::{SharedStore, StorageConfig};
use crate::task_work::TaskWork;

/// A burst is projected once it has been quiet this long…
const QUIET: Duration = Duration::from_millis(100);
/// …or this long after its first commit, so a steady writer cannot postpone it.
const BURST: Duration = Duration::from_millis(250);
const HEARTBEAT: Duration = Duration::from_secs(2);
/// Revisions are read again on this clock in case a filesystem event was lost.
const CHECK: Duration = Duration::from_secs(1);
/// A part that keeps failing is read again after 1 s, 2 s, 4 s… up to this.
const RETRY_CAP: Duration = Duration::from_secs(60);
/// Git and filesystem facts in planning are not store commits. A watched
/// checkout is asked about when it changes; this covers what no watch saw.
const PLANNING_CLOCK: Duration = Duration::from_secs(300);
/// Process liveness is observed, never committed.
const ACTIVITY_CLOCK: Duration = Duration::from_secs(2);
const MAX_FRAME: usize = 64 * 1024 * 1024;
const MAX_REQUEST: usize = 16 * 1024;
const WORK_ACTIVITY_WINDOW: i64 = 7 * 24 * 3600;
const WORK_ACTIVITY_LIMIT: usize = 50;

/// One line of the stream. `sequence` orders every frame of one process;
/// `answers` is the newest request handled before this frame's reading began.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceFrame {
    pub sequence: u64,
    pub answers: Option<u64>,
    pub home: String,
    /// The store revisions this frame was read at; `null` before a store exists.
    pub revisions: Option<StoreRevisions>,
    /// Why this part could not be read. Its body is then `null`.
    pub unavailable: Option<String>,
    #[serde(flatten)]
    pub content: WorkspaceContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "part", content = "body", rename_all = "snake_case")]
#[non_exhaustive]
pub enum WorkspaceContent {
    Planning(Option<Box<PlanningPart>>),
    Sessions(Option<SessionsPart>),
    Task(Option<TaskPart>),
    Wave(Option<Box<WavePart>>),
    WorkActivity(Option<WorkActivityPart>),
    Activity(Option<ActivitySnapshot>),
    Heartbeat(Heartbeat),
}

/// `lf roadmap --all` and `lf wave list --all --current`, read together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningPart {
    pub roadmap: RoadmapSnapshot,
    pub waves: Vec<WaveSnapshot>,
}

/// Every Session of the scoped repository, as `lf session list` pages them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsPart {
    pub repo: String,
    pub includes_headless: bool,
    pub entries: Vec<SessionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPart {
    pub task: String,
    pub work: TaskWork,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WavePart {
    pub wave: String,
    pub detail: WaveDetailSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkActivityPart {
    pub scope: WorkActivityScope,
    pub snapshot: WorkActivitySnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkActivityScope {
    pub wave: Option<String>,
    pub project: Option<String>,
    pub task: Option<String>,
}

/// Sent on a clock so a silent stream is distinguishable from a stalled one.
/// `projections` counts readings per part, including those that sent nothing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Heartbeat {
    pub projections: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    /// What this window shows. Replaces the previous scope.
    Scope {
        id: u64,
        repo: Option<String>,
        headless: bool,
        task: Option<String>,
        wave: Option<String>,
        activity: Option<WorkActivityScope>,
    },
    /// Read every part again and answer, changed or not: after a local
    /// write, sleep, or any other gap in observation.
    Refresh { id: u64 },
}

#[derive(Debug, Clone, Default)]
struct Scope {
    repo: Option<String>,
    headless: bool,
    task: Option<String>,
    wave: Option<String>,
    activity: Option<WorkActivityScope>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Part {
    Sessions,
    Planning,
    Task,
    Wave,
    WorkActivity,
    Activity,
}

impl Part {
    const ALL: [Part; 6] = [
        Part::Sessions,
        Part::Planning,
        Part::Task,
        Part::Wave,
        Part::WorkActivity,
        Part::Activity,
    ];
    const SCOPED: [Part; 4] = [Part::Sessions, Part::Task, Part::Wave, Part::WorkActivity];

    fn name(self) -> &'static str {
        match self {
            Part::Sessions => "sessions",
            Part::Planning => "planning",
            Part::Task => "task",
            Part::Wave => "wave",
            Part::WorkActivity => "work_activity",
            Part::Activity => "activity",
        }
    }

    /// Whether a commit that moved revisions from `old` to `new` can change
    /// this part. Planning conditions read Sessions, Flows and unfinished Execs.
    fn changed(self, old: StoreRevisions, new: StoreRevisions) -> bool {
        match self {
            Part::Sessions => {
                StoreRevisions { execs: 0, ..old } != StoreRevisions { execs: 0, ..new }
            }
            Part::Activity => old.execs != new.execs,
            Part::Planning | Part::Task | Part::Wave | Part::WorkActivity => old != new,
        }
    }

    fn clock(self) -> Option<Duration> {
        match self {
            Part::Planning | Part::Wave => Some(PLANNING_CLOCK),
            Part::Activity => Some(ACTIVITY_CLOCK),
            _ => None,
        }
    }

    fn selected(self, scope: &Scope) -> bool {
        match self {
            Part::Sessions => scope.repo.is_some(),
            Part::Task => scope.task.is_some(),
            Part::Wave => scope.wave.is_some(),
            Part::WorkActivity => scope.activity.is_some(),
            Part::Planning | Part::Activity => true,
        }
    }
}

/// What was last read for one part, and what was last sent.
#[derive(Debug, Default)]
struct PartState {
    read_at: Option<StoreRevisions>,
    read_on: Option<Instant>,
    sent: Option<[u8; 32]>,
    force: bool,
    /// Something it reads outside the store changed.
    stale: bool,
    /// Consecutive failed readings.
    failures: u32,
}

impl PartState {
    /// A failed reading is retried without a change, each time after twice
    /// the wait, so a part that cannot be read does not hold the one loop.
    fn retry_due(&self, read_on: Instant, now: Instant) -> bool {
        self.failures > 0
            && now - read_on
                >= CHECK
                    .saturating_mul(1 << (self.failures - 1).min(16))
                    .min(RETRY_CAP)
    }
}

#[derive(Debug, Default)]
struct Mailbox {
    requests: Vec<Request>,
    look: bool,
    closed: bool,
    error: Option<String>,
    /// At most one unsent frame per part; a newer one replaces it in place.
    unsent: VecDeque<(&'static str, WorkspaceFrame)>,
    sequence: u64,
    answers: Option<u64>,
    revisions: Option<StoreRevisions>,
    projections: BTreeMap<String, u64>,
}

type Shared = Arc<(Mutex<Mailbox>, Condvar)>;

fn lock(shared: &Shared) -> MutexGuard<'_, Mailbox> {
    shared.0.lock().expect("workspace reader mailbox poisoned")
}

fn enqueue(
    mailbox: &mut Mailbox,
    key: &'static str,
    home: &str,
    unavailable: Option<String>,
    content: WorkspaceContent,
) {
    mailbox.sequence += 1;
    let frame = WorkspaceFrame {
        sequence: mailbox.sequence,
        answers: mailbox.answers,
        home: home.to_owned(),
        revisions: mailbox.revisions,
        unavailable,
        content,
    };
    match mailbox.unsent.iter_mut().find(|(unsent, _)| *unsent == key) {
        Some(slot) => slot.1 = frame,
        None => mailbox.unsent.push_back((key, frame)),
    }
}

fn encode(frame: &WorkspaceFrame) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(frame)?;
    if bytes.len() > MAX_FRAME {
        // Only a part's body can be this large; a heartbeat never is.
        let mut value = serde_json::to_value(frame)?;
        value["unavailable"] = "workspace frame exceeds the 64 MiB transport limit".into();
        value["body"] = serde_json::Value::Null;
        bytes = serde_json::to_vec(&value)?;
    }
    bytes.push(b'\n');
    Ok(bytes)
}

/// Content identity, ignoring the fields that restate when it was read.
fn fingerprint(unavailable: &Option<String>, content: &WorkspaceContent) -> Result<[u8; 32]> {
    fn strip(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(fields) => {
                fields.remove("generated_at");
                fields.remove("evidence_age_secs");
                // A Task condition restates when it was derived.
                if let Some(serde_json::Value::Object(condition)) = fields.get_mut("condition") {
                    condition.remove("observed_at");
                }
                fields.values_mut().for_each(strip);
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(content)?;
    strip(&mut value);
    match content {
        WorkspaceContent::Activity(_) => {
            value
                .pointer_mut("/body")
                .and_then(|body| body.as_object_mut())
                .map(|body| body.remove("observed_at"));
        }
        WorkspaceContent::WorkActivity(_) => {
            value
                .pointer_mut("/body/snapshot")
                .and_then(|body| body.as_object_mut())
                .map(|body| body.remove("since"));
        }
        _ => {}
    }
    let mut hash = Sha256::new();
    hash.update(serde_json::to_vec(unavailable)?);
    hash.update(serde_json::to_vec(&value)?);
    Ok(hash.finalize().into())
}

struct Reader {
    database: PathBuf,
    store: Option<SharedStore>,
    /// Why the store that exists could not be opened.
    refused: Option<String>,
    scope: Scope,
    parts: BTreeMap<Part, PartState>,
    checkouts: checkouts::Checkouts,
    /// The planning revision whose checkouts are watched.
    watched_at: Option<i64>,
    runtime: tokio::runtime::Runtime,
}

impl Reader {
    /// A store that appears later is opened then; nothing here creates one.
    fn open(&mut self) {
        if self.store.is_some() || !self.database.exists() {
            return;
        }
        let config = StorageConfig::sqlite(self.database.clone());
        match self
            .runtime
            .block_on(crate::store::open_read_only_store(&config))
        {
            Ok(store) => {
                self.store = Some(Arc::new(store));
                self.refused = None;
            }
            Err(error) => {
                let reason = format!("{error:#}");
                if self.refused.as_ref() != Some(&reason) {
                    tracing::warn!(%error, "workspace reader cannot open the store");
                }
                self.refused = Some(reason);
            }
        }
    }

    fn revisions(&self) -> Option<StoreRevisions> {
        self.store
            .as_ref()
            .and_then(|store| store.sqlite.revisions().ok())
    }

    /// Follow the checkouts of the Tasks registered now, and ask Git again
    /// about those that changed on disk. The parts showing Git facts are read
    /// again only when an answer differs.
    fn observe_checkouts(&mut self, revisions: Option<StoreRevisions>, shared: &Shared) {
        let (Some(store), Some(revisions)) = (&self.store, revisions) else {
            return;
        };
        if self.watched_at != Some(revisions.planning) {
            let Ok(checkouts) = store.sqlite.task_checkouts() else {
                return;
            };
            let wake = shared.clone();
            self.checkouts.watch(
                checkouts.into_iter().map(|row| row.worktree).collect(),
                move || {
                    lock(&wake).look = true;
                    wake.1.notify_all();
                },
            );
            self.watched_at = Some(revisions.planning);
        }
        let changed = self.checkouts.take(Instant::now());
        // Every one is asked: `any` would stop at the first that differs.
        let differs = changed
            .iter()
            .map(|worktree| crate::engine::git::reread_retained(worktree))
            .fold(false, |any, differs| any || differs);
        if differs {
            for part in [Part::Planning, Part::Wave] {
                self.parts.entry(part).or_default().stale = true;
            }
        }
    }

    fn accept(&mut self, request: Request) -> u64 {
        match request {
            Request::Scope {
                id,
                repo,
                headless,
                task,
                wave,
                activity,
            } => {
                self.scope = Scope {
                    repo,
                    headless,
                    task,
                    wave,
                    activity,
                };
                for part in Part::SCOPED {
                    let state = self.parts.entry(part).or_default();
                    state.force = true;
                    state.sent = None;
                }
                id
            }
            Request::Refresh { id } => {
                for state in self.parts.values_mut() {
                    state.force = true;
                }
                id
            }
        }
    }

    fn due(&self, part: Part, revisions: Option<StoreRevisions>, now: Instant) -> bool {
        if !part.selected(&self.scope) {
            return false;
        }
        let Some(state) = self.parts.get(&part) else {
            return true;
        };
        let Some(read_on) = state.read_on else {
            return true;
        };
        state.force
            || state.stale
            || state.retry_due(read_on, now)
            || match (state.read_at, revisions) {
                (Some(old), Some(new)) => part.changed(old, new),
                (None, None) => false,
                _ => true,
            }
            || part.clock().is_some_and(|clock| now - read_on >= clock)
    }

    /// When the next clock-driven reading is due.
    fn next_clock(&self, now: Instant) -> Duration {
        Part::ALL
            .into_iter()
            .filter(|part| part.selected(&self.scope))
            .filter_map(|part| {
                let clock = part.clock()?;
                let read_on = self.parts.get(&part)?.read_on?;
                Some((read_on + clock).saturating_duration_since(now))
            })
            .min()
            .unwrap_or(CHECK)
            .min(CHECK)
    }

    fn project(&self, part: Part) -> Result<WorkspaceContent> {
        let Some(store) = &self.store else {
            // A store that cannot be opened is not an empty one.
            return match &self.refused {
                Some(reason) if part != Part::Activity => {
                    Err(anyhow!("Loopflow store cannot be read: {reason}"))
                }
                _ => self.absent(part),
            };
        };
        self.runtime.block_on(async {
            Ok(match part {
                Part::Planning => WorkspaceContent::Planning(Some(Box::new(PlanningPart {
                    roadmap: super::waves::roadmap_all(store).await?,
                    waves: super::waves::wave_snapshots(store, true, true).await?,
                }))),
                Part::Sessions => {
                    let repo = self.scope.repo.clone().context("no repository in scope")?;
                    let filter = crate::session::SessionFilter {
                        repo: Some(CanonicalRepo::discover(Path::new(&repo))?.to_string()),
                        interactive: if self.scope.headless {
                            None
                        } else {
                            Some(true)
                        },
                        limit: 0,
                        after: Some(String::new()),
                        ..Default::default()
                    };
                    WorkspaceContent::Sessions(Some(SessionsPart {
                        repo,
                        includes_headless: self.scope.headless,
                        entries: crate::ops::human_session::list(store, &filter).await?,
                    }))
                }
                Part::Task => {
                    let selector = self.scope.task.clone().context("no Task in scope")?;
                    let task = store
                        .sqlite
                        .resolve_task_id(&selector, None)?
                        .ok_or_else(|| anyhow!("Task {selector} is not registered"))?;
                    WorkspaceContent::Task(Some(TaskPart {
                        task: selector,
                        work: store.sqlite.task_work(&task)?,
                    }))
                }
                Part::Wave => {
                    let id = self.scope.wave.clone().context("no Wave in scope")?;
                    let wave = store
                        .get_wave(&crate::id::WaveId::parse(&id)?)
                        .await?
                        .ok_or_else(|| anyhow!("Wave {id} is not registered"))?;
                    WorkspaceContent::Wave(Some(Box::new(WavePart {
                        wave: id,
                        detail: super::waves::wave_detail(store, &wave).await?,
                    })))
                }
                Part::WorkActivity => {
                    let scope = self.scope.activity.clone().context("no Work in scope")?;
                    let now = time::OffsetDateTime::now_utc().unix_timestamp();
                    let snapshot = super::activity::build_snapshot(
                        &store.sqlite,
                        now,
                        now - WORK_ACTIVITY_WINDOW,
                        WORK_ACTIVITY_LIMIT,
                        super::WorkFilter {
                            wave: scope.wave.as_deref(),
                            project: scope.project.as_deref(),
                            task: scope.task.as_deref(),
                        },
                    )?;
                    WorkspaceContent::WorkActivity(Some(WorkActivityPart { scope, snapshot }))
                }
                Part::Activity => WorkspaceContent::Activity(Some(super::top::load_snapshot()?)),
            })
        })
    }

    /// Before a store exists there is nothing registered, which is a reading.
    fn absent(&self, part: Part) -> Result<WorkspaceContent> {
        Ok(match part {
            Part::Planning => WorkspaceContent::Planning(Some(Box::new(PlanningPart {
                roadmap: RoadmapSnapshot {
                    generated_at: time::OffsetDateTime::now_utc()
                        .format(&time::format_description::well_known::Rfc3339)?,
                    waves: Vec::new(),
                },
                waves: Vec::new(),
            }))),
            Part::Sessions => WorkspaceContent::Sessions(Some(SessionsPart {
                repo: self.scope.repo.clone().context("no repository in scope")?,
                includes_headless: self.scope.headless,
                entries: Vec::new(),
            })),
            Part::Activity => WorkspaceContent::Activity(Some(super::top::load_snapshot()?)),
            Part::Task | Part::Wave | Part::WorkActivity => {
                anyhow::bail!("no Loopflow store exists in this Home yet")
            }
        })
    }
}

fn unavailable(part: Part) -> WorkspaceContent {
    match part {
        Part::Planning => WorkspaceContent::Planning(None),
        Part::Sessions => WorkspaceContent::Sessions(None),
        Part::Task => WorkspaceContent::Task(None),
        Part::Wave => WorkspaceContent::Wave(None),
        Part::WorkActivity => WorkspaceContent::WorkActivity(None),
        Part::Activity => WorkspaceContent::Activity(None),
    }
}

pub(super) fn run(watch: bool) -> Result<()> {
    let database = crate::store::database_path_from_env()?;
    let home = crate::store::lf_home_dir();
    let home = home
        .canonicalize()
        .unwrap_or(home)
        .to_string_lossy()
        .into_owned();
    if watch {
        // Every reading asks Git the same questions about the same checkouts.
        crate::engine::git::retain_reads();
    }
    let shared: Shared = Arc::new((Mutex::new(Mailbox::default()), Condvar::new()));
    let mut reader = Reader {
        database: database.clone(),
        store: None,
        refused: None,
        scope: Scope::default(),
        parts: BTreeMap::new(),
        checkouts: Default::default(),
        watched_at: None,
        runtime: tokio::runtime::Runtime::new()?,
    };

    // These threads belong to the foreground command and end with it. Closing
    // stdin is the only way a parent stops the reader.
    if watch {
        let input = shared.clone();
        std::thread::spawn(move || {
            let mut stdin = std::io::stdin().lock();
            loop {
                let mut line = Vec::new();
                let read = std::io::Read::take(&mut stdin, MAX_REQUEST as u64 + 1)
                    .read_until(b'\n', &mut line);
                let mut mailbox = lock(&input);
                match read {
                    Ok(0) => mailbox.closed = true,
                    Ok(_) if line.len() <= MAX_REQUEST => match serde_json::from_slice(&line) {
                        Ok(request) => mailbox.requests.push(request),
                        Err(error) => {
                            mailbox.error = Some(format!("invalid reader request: {error}"))
                        }
                    },
                    _ => {
                        mailbox.error = Some("reader stdin failed or request exceeds 16 KiB".into())
                    }
                }
                let done = mailbox.closed || mailbox.error.is_some();
                input.1.notify_all();
                if done {
                    break;
                }
            }
        });
        let changes = shared.clone();
        std::thread::spawn(move || {
            let mut store = StoreChanges::watch(&database);
            loop {
                store.wait(CHECK);
                let mut mailbox = lock(&changes);
                if mailbox.closed {
                    break;
                }
                mailbox.look = true;
                changes.1.notify_all();
            }
        });
        let heartbeat = shared.clone();
        let heartbeat_home = home.clone();
        std::thread::spawn(move || loop {
            std::thread::sleep(HEARTBEAT);
            let mut mailbox = lock(&heartbeat);
            if mailbox.closed {
                break;
            }
            let projections = mailbox.projections.clone();
            enqueue(
                &mut mailbox,
                "heartbeat",
                &heartbeat_home,
                None,
                WorkspaceContent::Heartbeat(Heartbeat { projections }),
            );
            heartbeat.1.notify_all();
        });
    }
    let output = shared.clone();
    let writer = std::thread::spawn(move || {
        let mut stdout = std::io::stdout().lock();
        loop {
            let frame = {
                let mut mailbox = lock(&output);
                loop {
                    if let Some((_, frame)) = mailbox.unsent.pop_front() {
                        break frame;
                    }
                    if mailbox.closed {
                        return;
                    }
                    mailbox = output
                        .1
                        .wait(mailbox)
                        .expect("workspace reader mailbox poisoned");
                }
            };
            let written = encode(&frame)
                .map_err(std::io::Error::other)
                .and_then(|bytes| stdout.write_all(&bytes))
                .and_then(|()| stdout.flush());
            if written.is_err() {
                lock(&output).closed = true;
                output.1.notify_all();
                return;
            }
        }
    });

    let result = (|| -> Result<()> {
        let mut seen = None;
        loop {
            let requests = {
                let mut mailbox = lock(&shared);
                if let Some(error) = mailbox.error.take() {
                    anyhow::bail!(error);
                }
                if mailbox.closed {
                    return Ok(());
                }
                mailbox.look = false;
                std::mem::take(&mut mailbox.requests)
            };
            let mut answers = None;
            for request in requests {
                answers = Some(reader.accept(request));
            }
            reader.open();
            let mut revisions = reader.revisions();
            if seen != revisions && seen.is_some() {
                // Let a burst of related commits land before reading any of it.
                let first = Instant::now();
                loop {
                    let mailbox = lock(&shared);
                    let (mut mailbox, _) = shared
                        .1
                        .wait_timeout_while(mailbox, QUIET, |mailbox| {
                            !mailbox.look && !mailbox.closed
                        })
                        .expect("workspace reader mailbox poisoned");
                    let again = std::mem::take(&mut mailbox.look);
                    if !again || first.elapsed() >= BURST {
                        break;
                    }
                }
                revisions = reader.revisions();
            }
            seen = revisions;
            if watch {
                reader.observe_checkouts(revisions, &shared);
            }
            {
                let mut mailbox = lock(&shared);
                mailbox.revisions = revisions;
                if answers.is_some() {
                    mailbox.answers = answers;
                }
            }
            for part in Part::ALL {
                let now = Instant::now();
                if !reader.due(part, revisions, now) {
                    continue;
                }
                let (reason, content) = match reader.project(part) {
                    Ok(content) => (None, content),
                    Err(error) => (Some(format!("{error:#}")), unavailable(part)),
                };
                let identity = fingerprint(&reason, &content)?;
                let state = reader.parts.entry(part).or_default();
                let send = state.force || state.sent != Some(identity);
                state.read_at = revisions;
                state.read_on = Some(now);
                state.force = false;
                state.stale = false;
                state.failures = if reason.is_some() {
                    state.failures.saturating_add(1)
                } else {
                    0
                };
                state.sent = Some(identity);
                let mut mailbox = lock(&shared);
                *mailbox.projections.entry(part.name().into()).or_default() += 1;
                if send {
                    enqueue(&mut mailbox, part.name(), &home, reason, content);
                    shared.1.notify_all();
                }
                if mailbox.closed {
                    return Ok(());
                }
            }
            if !watch {
                return Ok(());
            }
            let mailbox = lock(&shared);
            let wait = reader.next_clock(Instant::now());
            drop(
                shared
                    .1
                    .wait_timeout_while(mailbox, wait, |mailbox| {
                        !mailbox.look
                            && mailbox.requests.is_empty()
                            && !mailbox.closed
                            && mailbox.error.is_none()
                    })
                    .expect("workspace reader mailbox poisoned"),
            );
        }
    })();
    lock(&shared).closed = true;
    shared.1.notify_all();
    writer.join().expect("workspace reader output panicked");
    result
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{fingerprint, Part, PartState, StoreRevisions, WorkspaceContent};
    use crate::lf::commands::top::ActivitySnapshot;

    fn revisions(planning: i64, sessions: i64, flows: i64, execs: i64) -> StoreRevisions {
        StoreRevisions {
            planning,
            sessions,
            flows,
            execs,
        }
    }

    #[test]
    fn an_exec_alone_does_not_reread_sessions() {
        let old = revisions(1, 1, 1, 1);
        assert!(!Part::Sessions.changed(old, revisions(1, 1, 1, 2)));
        assert!(Part::Planning.changed(old, revisions(1, 1, 1, 2)));
        assert!(Part::Sessions.changed(old, revisions(2, 1, 1, 1)));
        assert!(!Part::Activity.changed(old, revisions(2, 2, 2, 1)));
    }

    #[test]
    fn a_failing_part_waits_longer_before_each_retry() {
        let read_on = Instant::now();
        let due = |failures, secs| {
            PartState {
                failures,
                ..Default::default()
            }
            .retry_due(read_on, read_on + Duration::from_secs(secs))
        };
        assert!(!due(0, 3600));
        assert!(due(1, 1));
        assert!(!due(3, 3));
        assert!(due(3, 4));
        assert!(!due(40, 59));
        assert!(due(40, 60));
    }

    #[test]
    fn the_time_of_a_reading_is_not_a_change() {
        let at = |observed_at| {
            WorkspaceContent::Activity(Some(ActivitySnapshot {
                schema_version: 1,
                observed_at,
                nodes: Vec::new(),
                provider_processes: Vec::new(),
            }))
        };
        assert_eq!(
            fingerprint(&None, &at(1)).unwrap(),
            fingerprint(&None, &at(2)).unwrap()
        );
        assert_ne!(
            fingerprint(&None, &at(1)).unwrap(),
            fingerprint(&Some("gone".into()), &at(1)).unwrap()
        );
    }
}
