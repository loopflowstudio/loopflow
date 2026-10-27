//! `lf monitor work --watch --json`: one foreground reader per window.
//!
//! The store's change revisions say which parts a commit can have changed. A
//! part is projected again only then, on a read-only connection, and sent only
//! when its content differs from the last frame. Bodies are the same wire types
//! the one-shot `--json` reads print. Projections remain read-only. A separate
//! foreground synchronization lifetime updates the selected repository's planning;
//! those commits wake the reader just like edits from another connection.

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
use crate::durable::GraphProcessDetail;
use crate::ops::human_session::SessionRecord;
use crate::repository::CanonicalRepo;
use crate::store::changes::StoreChanges;
use crate::store::sqlite::StoreRevisions;
use crate::store::{SharedStore, StorageConfig};
use crate::task_work::{TaskOverview, TaskWork};

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
/// Agents record usage every few seconds. Token totals are read this often
/// at most, however often they move.
const USAGE_REST: Duration = Duration::from_secs(10);
const MAX_FRAME: usize = 64 * 1024 * 1024;
const MAX_REQUEST: usize = 16 * 1024;
const WORK_ACTIVITY_WINDOW: i64 = 7 * 24 * 3600;
const WORK_ACTIVITY_LIMIT: usize = 50;

/// One line of the stream. `sequence` orders every frame of one process;
/// `answers` is the newest request handled before this frame's reading began.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkFrame {
    pub sequence: u64,
    pub answers: Option<u64>,
    pub home: String,
    /// The preview copy directory this build's drafts run on; `null` on the
    /// shared store.
    pub copy: Option<String>,
    /// The store revisions this frame was read at; `null` before a store exists.
    pub revisions: Option<StoreRevisions>,
    /// Why this part could not be read. Its body is then `null`.
    pub unavailable: Option<String>,
    #[serde(flatten)]
    pub content: WorkContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "part", content = "body", rename_all = "snake_case")]
#[non_exhaustive]
pub enum WorkContent {
    Planning(Option<Box<PlanningPart>>),
    PeerPlanning(Option<PeerPlanningPart>),
    Sessions(Option<SessionsPart>),
    Task(Option<Box<TaskPart>>),
    TaskHistory(Option<Box<TaskHistoryPart>>),
    Wave(Option<Box<WavePart>>),
    WorkActivity(Option<WorkActivityPart>),
    Activity(Option<ActivitySnapshot>),
    Heartbeat(Heartbeat),
    /// All selected database projections have caught up to this read snapshot.
    /// Sent after their changed frames, even when every body stayed equal.
    Checkpoint,
}

/// `lf wave show --all` and `lf wave list --all --current`, read together.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanningPart {
    pub selections: Option<Vec<crate::work::selection::ViewPlan>>,
    pub roadmap: RoadmapSnapshot,
    pub waves: Vec<WaveSnapshot>,
}

/// The selected repository's Git planning receipts, independent of roadmap reads.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerPlanningPart {
    pub repo: String,
    pub destinations: Vec<crate::store::PeerPlanningStatus>,
}

/// Every Session of the scoped repository, as `lf session list` pages them.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionsPart {
    pub repo: String,
    pub includes_headless: bool,
    pub entries: Vec<SessionRecord>,
}

/// Selected workspace facts and `lf graph show --processes` for each of its Graphs,
/// in the same order.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPart {
    pub task: String,
    pub work: TaskOverview,
    pub graph_processes: Vec<GraphProcessDetail>,
    pub comments: crate::work::task::comment::TaskComments,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskHistoryPart {
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
    /// Explicit retry or recovery after a gap in observation.
    Refresh {
        id: u64,
    },
    /// Confirm committed writes without forcing unchanged subscriptions to read.
    Checkpoint {
        id: u64,
        replay: Vec<Part>,
    },
    TaskHistory {
        id: u64,
        task: Option<String>,
    },
}

#[derive(Debug, Clone, Default)]
struct Scope {
    repo: Option<String>,
    headless: bool,
    task: Option<String>,
    wave: Option<String>,
    activity: Option<WorkActivityScope>,
    history: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Part {
    TaskHistory,
    Sessions,
    Planning,
    PeerPlanning,
    Task,
    Wave,
    WorkActivity,
    Activity,
}

impl Part {
    // Deliver the selected Work before the portfolio; every part still reads
    // the same snapshot, and a checkpoint follows the complete pass.
    const ALL: [Part; 8] = [
        Part::Task,
        Part::Wave,
        Part::Sessions,
        Part::Planning,
        Part::PeerPlanning,
        Part::WorkActivity,
        Part::TaskHistory,
        Part::Activity,
    ];
    const SCOPED: [Part; 5] = [
        Part::Sessions,
        Part::PeerPlanning,
        Part::Task,
        Part::Wave,
        Part::WorkActivity,
    ];

    fn name(self) -> &'static str {
        match self {
            Part::Sessions => "sessions",
            Part::Planning => "planning",
            Part::PeerPlanning => "peer_planning",
            Part::Task => "task",
            Part::TaskHistory => "task_history",
            Part::Wave => "wave",
            Part::WorkActivity => "work_activity",
            Part::Activity => "activity",
        }
    }

    /// Whether a commit that moved revisions from `old` to `new` can change
    /// this part. Planning conditions read Sessions, Graphs and unfinished
    /// Processes. Sessions read their AgentProcess; only Wave history shows
    /// token totals.
    fn changed(self, mut old: StoreRevisions, new: StoreRevisions) -> bool {
        if self == Part::PeerPlanning {
            return old.planning != new.planning;
        }
        if self == Part::Activity {
            return old.processes != new.processes;
        }
        // History reads Work labels/PRs, captured inputs and their Processes;
        // it never reads compiled graphs or steps (activity::build_snapshot).
        if self == Part::WorkActivity {
            old.graphs = new.graphs;
        }
        if self != Part::Wave {
            old.usage = new.usage;
        }
        old != new
    }

    /// Whether a change is to usage alone, which the part can wait for.
    fn rests(self, old: StoreRevisions, new: StoreRevisions) -> bool {
        self == Part::Wave
            && StoreRevisions {
                usage: new.usage,
                ..old
            } == new
    }

    fn clock(self) -> Duration {
        match self {
            Part::Planning | Part::Wave => PLANNING_CLOCK,
            // Transport selection lives in config, so it can change without a store write.
            Part::PeerPlanning => Duration::from_secs(5),
            Part::Activity
            | Part::Sessions
            | Part::Task
            | Part::TaskHistory
            | Part::WorkActivity => ACTIVITY_CLOCK,
        }
    }

    fn selected(self, scope: &Scope) -> bool {
        match self {
            Part::Sessions | Part::PeerPlanning => scope.repo.is_some(),
            Part::Task => scope.task.is_some(),
            Part::TaskHistory => scope.history.is_some(),
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
    /// Replay a reading superseded by an authoritative command result without
    /// querying unchanged data again.
    replay: bool,
    last: Option<(Option<String>, WorkContent)>,
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
    /// At most one unsent frame per part, ordered by its latest reading.
    unsent: VecDeque<(&'static str, WorkFrame)>,
    sequence: u64,
    answers: Option<u64>,
    revisions: Option<StoreRevisions>,
    projections: BTreeMap<String, u64>,
}

type Shared = Arc<(Mutex<Mailbox>, Condvar)>;

fn lock(shared: &Shared) -> MutexGuard<'_, Mailbox> {
    shared.0.lock().expect("work reader mailbox poisoned")
}

/// Where this reader's data lives; every frame names it.
#[derive(Debug, Clone)]
struct Origin {
    home: String,
    copy: Option<String>,
}

fn enqueue(
    mailbox: &mut Mailbox,
    key: &'static str,
    origin: &Origin,
    unavailable: Option<String>,
    content: WorkContent,
) {
    mailbox.sequence += 1;
    let frame = WorkFrame {
        sequence: mailbox.sequence,
        answers: mailbox.answers,
        home: origin.home.clone(),
        copy: origin.copy.clone(),
        // Activity uses a separate OS/process reader, outside the Work snapshot.
        revisions: if matches!(content, WorkContent::Activity(_)) {
            None
        } else {
            mailbox.revisions
        },
        unavailable,
        content,
    };
    if let Some(index) = mailbox.unsent.iter().position(|(unsent, _)| *unsent == key) {
        mailbox.unsent.remove(index);
    }
    mailbox.unsent.push_back((key, frame));
}

fn encode(frame: &WorkFrame) -> Result<Vec<u8>> {
    let mut bytes = serde_json::to_vec(frame)?;
    if bytes.len() > MAX_FRAME {
        // Only a part's body can be this large; a heartbeat never is.
        let mut value = serde_json::to_value(frame)?;
        value["unavailable"] = "work frame exceeds the 64 MiB transport limit".into();
        value["body"] = serde_json::Value::Null;
        bytes = serde_json::to_vec(&value)?;
    }
    bytes.push(b'\n');
    Ok(bytes)
}

/// Content identity, ignoring the fields that restate when it was read.
fn fingerprint(unavailable: &Option<String>, content: &WorkContent) -> Result<[u8; 32]> {
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
        WorkContent::Activity(_) => {
            value
                .pointer_mut("/body")
                .and_then(|body| body.as_object_mut())
                .map(|body| body.remove("observed_at"));
        }
        WorkContent::WorkActivity(_) => {
            value
                .pointer_mut("/body/snapshot")
                .and_then(|body| body.as_object_mut())
                .map(|body| body.remove("since"));
        }
        // Each Dashboard plan restates when it was read.
        WorkContent::Planning(_) => {
            if let Some(plans) = value
                .pointer_mut("/body/selections")
                .and_then(|plans| plans.as_array_mut())
            {
                for plan in plans.iter_mut().filter_map(|plan| plan.as_object_mut()) {
                    plan.remove("observed_at");
                }
            }
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
    /// The sessions revision a quiet deadline was read at, and that deadline.
    quiet: Option<(i64, Option<i64>)>,
    runtime: tokio::runtime::Runtime,
    planning_sync: Option<(String, crate::ops::planning_sync::PlanningSync)>,
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
                    tracing::warn!(%error, "work reader cannot open the store");
                }
                self.refused = Some(reason);
            }
        }
    }

    fn sync_planning(&mut self) {
        if self.planning_sync.as_ref().map(|(repo, _)| repo) == self.scope.repo.as_ref() {
            return;
        }
        self.planning_sync = None;
        let Some(repo) = &self.scope.repo else {
            return;
        };
        if self.store.is_none() {
            return;
        }
        let result = self.runtime.block_on(async {
            let store = Arc::new(
                crate::store::open_store(&StorageConfig::sqlite(self.database.clone())).await?,
            );
            crate::ops::planning_sync::PlanningSync::start_watch(store, repo.clone())
                .map_err(anyhow::Error::from)
        });
        match result {
            Ok(sync) => self.planning_sync = Some((repo.clone(), sync)),
            Err(error) => tracing::warn!(%error, "cannot start repository planning sync"),
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
            let mut paths: Vec<_> = checkouts.into_iter().map(|row| row.worktree).collect();
            if let Ok(repositories) = store.sqlite.repository_paths() {
                paths.extend(repositories);
            }
            if let Some(repo) = &self.scope.repo {
                paths.push(std::path::PathBuf::from(repo));
            }
            let wake = shared.clone();
            self.checkouts.watch(paths, move || {
                lock(&wake).look = true;
                wake.1.notify_all();
            });
            self.watched_at = Some(revisions.planning);
        }
        let changed = self.checkouts.take(Instant::now());
        // Every one is asked again, not only up to the first that differs.
        let mut differs = !changed.is_empty();
        for worktree in &changed {
            differs |= crate::git::reread_retained(worktree);
        }
        if differs {
            for part in [Part::Planning, Part::Wave] {
                self.parts.entry(part).or_default().stale = true;
            }
        }
    }

    /// A Session becomes Waiting after quiet without any commit. Read the
    /// Sessions again when the earliest such moment passes; the loop looks
    /// at least every `CHECK`.
    fn observe_quiet(&mut self, revisions: Option<StoreRevisions>) {
        let (Some(store), Some(revisions)) = (&self.store, revisions) else {
            return;
        };
        if !Part::Sessions.selected(&self.scope) {
            self.quiet = None;
            return;
        }
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let passed = matches!(self.quiet, Some((_, Some(due))) if now >= due);
        if passed {
            self.parts.entry(Part::Sessions).or_default().stale = true;
        }
        if passed || self.quiet.map(|(read_at, _)| read_at) != Some(revisions.sessions) {
            self.quiet = store
                .sqlite
                .next_quiet_available(now)
                .ok()
                .map(|due| (revisions.sessions, due));
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
                let next = Scope {
                    repo,
                    headless,
                    task: task.clone(),
                    wave,
                    activity,
                    history: if self.scope.task == task {
                        self.scope.history.clone()
                    } else {
                        None
                    },
                };
                for part in Part::SCOPED {
                    let changed = match part {
                        Part::Sessions => {
                            self.scope.repo != next.repo || self.scope.headless != next.headless
                        }
                        Part::PeerPlanning => self.scope.repo != next.repo,
                        Part::Task => self.scope.task != next.task,
                        Part::Wave => self.scope.wave != next.wave,
                        Part::WorkActivity => self.scope.activity != next.activity,
                        _ => false,
                    };
                    let state = self.parts.entry(part).or_default();
                    if changed {
                        state.force = true;
                    } else {
                        // Swift advances its scope floor for every scoped part.
                        // Deliver unchanged siblings from cache, without queries.
                        state.replay = true;
                    }
                }
                if self.scope.repo != next.repo {
                    // Planning is unscoped except for its repository Dashboard plans.
                    self.parts.entry(Part::Planning).or_default().force = true;
                }
                self.scope = next;
                id
            }
            Request::TaskHistory { id, task } => {
                self.scope.history = task;
                self.parts.entry(Part::TaskHistory).or_default().force = true;
                id
            }
            Request::Refresh { id } => {
                for state in self.parts.values_mut() {
                    state.force = true;
                }
                id
            }
            Request::Checkpoint { id, replay } => {
                for part in replay {
                    self.parts.entry(part).or_default().replay = true;
                }
                id
            }
        }
    }

    fn due(
        &self,
        part: Part,
        revisions: Option<StoreRevisions>,
        now: Instant,
        checkpoint: bool,
    ) -> bool {
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
            // Confirmation cannot defer a failed or throttled database reading.
            || (checkpoint && state.failures > 0)
            || state.retry_due(read_on, now)
            || match (state.read_at, revisions) {
                (Some(old), Some(new)) => {
                    part.changed(old, new)
                        && (checkpoint || !part.rests(old, new) || now - read_on >= USAGE_REST)
                }
                (None, None) => false,
                _ => true,
            }
            || now - read_on >= part.clock()
    }

    /// When the next clock-driven reading is due.
    fn next_clock(&self, now: Instant) -> Duration {
        Part::ALL
            .into_iter()
            .filter(|part| part.selected(&self.scope))
            .filter_map(|part| {
                let clock = part.clock();
                let read_on = self.parts.get(&part)?.read_on?;
                Some((read_on + clock).saturating_duration_since(now))
            })
            .min()
            .unwrap_or(CHECK)
            .min(CHECK)
    }

    fn project(&self, part: Part) -> Result<WorkContent> {
        let Some(store) = &self.store else {
            // A store that cannot be opened is not an empty one.
            return match &self.refused {
                Some(reason) if part != Part::Activity => {
                    Err(anyhow!("Loopflow store cannot be read: {reason}"))
                }
                _ => self.absent(part),
            };
        };
        // Reading the Machine's planning recovers its work first.
        if part == Part::Planning {
            crate::ops::recovery::recover();
        }
        self.runtime.block_on(async {
            Ok(match part {
                Part::Planning => {
                    let selections = match &self.scope.repo {
                        Some(repo) => {
                            let canonical = CanonicalRepo::discover(Path::new(repo))?;
                            store
                                .sqlite
                                .repository_id(&canonical.to_string())?
                                .map(|id| store.sqlite.selection_plans(&id))
                                .transpose()?
                        }
                        None => None,
                    };
                    let roadmap = super::waves::roadmap_all(store).await?;
                    WorkContent::Planning(Some(Box::new(PlanningPart {
                        selections,
                        waves: super::waves::current_waves(&roadmap),
                        roadmap,
                    })))
                }
                Part::PeerPlanning => {
                    let repo = self.scope.repo.clone().context("no repository in scope")?;
                    let root = CanonicalRepo::discover(Path::new(&repo))?;
                    let destinations =
                        crate::ops::planning_sync::peer_status(store, &root.to_string()).await?;
                    WorkContent::PeerPlanning(Some(PeerPlanningPart { repo, destinations }))
                }
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
                    WorkContent::Sessions(Some(SessionsPart {
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
                    let work = store.sqlite.task_overview(&task)?;
                    let mut graph_processes = Vec::with_capacity(work.graph_processes.len());
                    for graph in &work.graph_processes {
                        let id = graph.summary.id.as_str();
                        let (process, entry) = store
                            .sqlite
                            .graph_process(id)?
                            .ok_or_else(|| anyhow!("Graph {id} has no Graph process record"))?;
                        graph_processes.push(process.detail(entry));
                    }
                    let comments = store.sqlite.task_comments(&task)?;
                    WorkContent::Task(Some(Box::new(TaskPart {
                        comments,
                        task: selector,
                        work,
                        graph_processes,
                    })))
                }
                Part::TaskHistory => {
                    let selector = self
                        .scope
                        .history
                        .clone()
                        .context("no diagnostic Task in scope")?;
                    let task = store
                        .sqlite
                        .resolve_task_id(&selector, None)?
                        .ok_or_else(|| anyhow!("Task {selector} is not registered"))?;
                    WorkContent::TaskHistory(Some(Box::new(TaskHistoryPart {
                        task: selector,
                        work: store.sqlite.task_work(&task)?,
                    })))
                }
                Part::Wave => {
                    let id = self.scope.wave.clone().context("no Wave in scope")?;
                    let wave = store
                        .get_wave(&crate::id::WaveId::parse(&id)?)
                        .await?
                        .ok_or_else(|| anyhow!("Wave {id} is not registered"))?;
                    WorkContent::Wave(Some(Box::new(WavePart {
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
                    WorkContent::WorkActivity(Some(WorkActivityPart { scope, snapshot }))
                }
                Part::Activity => WorkContent::Activity(Some(super::top::load_snapshot()?)),
            })
        })
    }

    /// Before a store exists there is nothing registered, which is a reading.
    fn absent(&self, part: Part) -> Result<WorkContent> {
        Ok(match part {
            Part::Planning => WorkContent::Planning(Some(Box::new(PlanningPart {
                selections: None,
                roadmap: RoadmapSnapshot {
                    generated_at: time::OffsetDateTime::now_utc()
                        .format(&time::format_description::well_known::Rfc3339)?,
                    waves: Vec::new(),
                    tasks: super::waves::Evidence::Ok {
                        items: Vec::new(),
                        truncated: false,
                    },
                },
                waves: Vec::new(),
            }))),
            Part::PeerPlanning => WorkContent::PeerPlanning(Some(PeerPlanningPart {
                repo: self.scope.repo.clone().context("no repository in scope")?,
                destinations: Vec::new(),
            })),
            Part::Sessions => WorkContent::Sessions(Some(SessionsPart {
                repo: self.scope.repo.clone().context("no repository in scope")?,
                includes_headless: self.scope.headless,
                entries: Vec::new(),
            })),
            Part::Activity => WorkContent::Activity(Some(super::top::load_snapshot()?)),
            Part::Task | Part::TaskHistory | Part::Wave | Part::WorkActivity => {
                anyhow::bail!("no Loopflow store exists in this Machine yet")
            }
        })
    }
}

fn unavailable(part: Part) -> WorkContent {
    match part {
        Part::Planning => WorkContent::Planning(None),
        Part::PeerPlanning => WorkContent::PeerPlanning(None),
        Part::Sessions => WorkContent::Sessions(None),
        Part::Task => WorkContent::Task(None),
        Part::TaskHistory => WorkContent::TaskHistory(None),
        Part::Wave => WorkContent::Wave(None),
        Part::WorkActivity => WorkContent::WorkActivity(None),
        Part::Activity => WorkContent::Activity(None),
    }
}

/// Requests arrive one JSON line each on stdin. Its end closes the reader.
fn read_requests(shared: &Shared) {
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
                Err(error) => mailbox.error = Some(format!("invalid reader request: {error}")),
            },
            _ => mailbox.error = Some("reader stdin failed or request exceeds 16 KiB".into()),
        }
        let done = mailbox.closed || mailbox.error.is_some();
        shared.1.notify_all();
        if done {
            break;
        }
    }
}

/// Write unsent frames until the reader closes. A parent that stopped reading
/// closes it.
fn write_frames(shared: &Shared) {
    let mut stdout = std::io::stdout().lock();
    loop {
        let frame = {
            let mut mailbox = lock(shared);
            loop {
                if let Some((_, frame)) = mailbox.unsent.pop_front() {
                    break frame;
                }
                if mailbox.closed {
                    return;
                }
                mailbox = shared
                    .1
                    .wait(mailbox)
                    .expect("work reader mailbox poisoned");
            }
        };
        let written = encode(&frame)
            .map_err(std::io::Error::other)
            .and_then(|bytes| stdout.write_all(&bytes))
            .and_then(|()| stdout.flush());
        if written.is_err() {
            lock(shared).closed = true;
            shared.1.notify_all();
            return;
        }
    }
}

/// Let a burst of related commits land before reading any of it.
fn settle(shared: &Shared) {
    let first = Instant::now();
    loop {
        let mailbox = lock(shared);
        let (mut mailbox, _) = shared
            .1
            .wait_timeout_while(mailbox, QUIET, |mailbox| !mailbox.look && !mailbox.closed)
            .expect("work reader mailbox poisoned");
        let again = std::mem::take(&mut mailbox.look);
        if !again || first.elapsed() >= BURST {
            break;
        }
    }
}

pub(super) fn run(watch: bool) -> Result<()> {
    let database = crate::store::database_path_from_env()?;
    let home = crate::store::lf_home_dir();
    let origin = Origin {
        home: home
            .canonicalize()
            .unwrap_or(home)
            .to_string_lossy()
            .into_owned(),
        copy: crate::store::copy::selected().map(|database| {
            database
                .parent()
                .expect("copy database has a directory")
                .to_string_lossy()
                .into_owned()
        }),
    };
    if watch {
        // Every reading asks Git the same questions about the same checkouts.
        crate::git::retain_reads();
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
        quiet: None,
        runtime: tokio::runtime::Runtime::new()?,
        planning_sync: None,
    };

    // These threads belong to the foreground command and end with it. Closing
    // stdin is the only way a parent stops the reader.
    if watch {
        let input = shared.clone();
        std::thread::spawn(move || read_requests(&input));
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
        let heartbeat_origin = origin.clone();
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
                &heartbeat_origin,
                None,
                WorkContent::Heartbeat(Heartbeat { projections }),
            );
            heartbeat.1.notify_all();
        });
    }
    let output = shared.clone();
    let writer = std::thread::spawn(move || write_frames(&output));

    let result = (|| -> Result<()> {
        let mut seen = None;
        loop {
            let (requests, mut checkpoint) = {
                let mut mailbox = lock(&shared);
                if let Some(error) = mailbox.error.take() {
                    anyhow::bail!(error);
                }
                if mailbox.closed {
                    return Ok(());
                }
                mailbox.look = false;
                // A pending confirmation follows any replacement frames from
                // this pass, including normally throttled dependencies.
                let pending = mailbox
                    .unsent
                    .iter()
                    .position(|(key, _)| *key == "checkpoint");
                if let Some(index) = pending {
                    mailbox.unsent.remove(index);
                }
                (std::mem::take(&mut mailbox.requests), pending.is_some())
            };
            let mut answers = None;
            for request in requests {
                checkpoint |= matches!(request, Request::Checkpoint { .. });
                answers = Some(reader.accept(request));
            }
            reader.open();
            let mut revisions = reader.revisions();
            if seen != revisions && seen.is_some() {
                settle(&shared);
                revisions = reader.revisions();
            }
            seen = revisions;
            if watch {
                reader.sync_planning();
                reader.observe_checkouts(revisions, &shared);
                reader.observe_quiet(revisions);
            }
            // Opening the connection can overlap the first scope request. Apply
            // it before reading the whole portfolio, not on the following pass.
            for request in std::mem::take(&mut lock(&shared).requests) {
                checkpoint |= matches!(request, Request::Checkpoint { .. });
                answers = Some(reader.accept(request));
            }
            // Only this loop uses the read-only connection. The first revision
            // query pins the WAL snapshot used by every joined projection below.
            // OS, Git and config observations are not transactional database facts.
            let store = reader.store.clone();
            let snapshot = store
                .as_ref()
                .map(|store| store.sqlite.read_snapshot())
                .transpose()?;
            let revisions = store
                .as_ref()
                .map(|store| store.sqlite.revisions())
                .transpose()?;
            {
                let mut mailbox = lock(&shared);
                mailbox.revisions = revisions;
                if answers.is_some() {
                    mailbox.answers = answers;
                }
            }
            for part in Part::ALL {
                let now = Instant::now();
                if !reader.due(part, revisions, now, checkpoint) {
                    continue;
                }
                let (reason, content) = match reader.project(part) {
                    Ok(content) => (None, content),
                    Err(error) => (Some(format!("{error:#}")), unavailable(part)),
                };
                tracing::debug!(
                    part = part.name(),
                    elapsed_us = now.elapsed().as_micros(),
                    "projected Work"
                );
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
                *lock(&shared)
                    .projections
                    .entry(part.name().into())
                    .or_default() += 1;
                if send {
                    state.last = Some((reason.clone(), content.clone()));
                    state.replay = false;
                    let mut mailbox = lock(&shared);
                    enqueue(&mut mailbox, part.name(), &origin, reason, content);
                    shared.1.notify_all();
                }
            }
            for (part, state) in &mut reader.parts {
                if part.selected(&reader.scope) && std::mem::take(&mut state.replay) {
                    if let Some((reason, content)) = &state.last {
                        // This is delivery of an existing projection, not query work.
                        let mut mailbox = lock(&shared);
                        enqueue(
                            &mut mailbox,
                            part.name(),
                            &origin,
                            reason.clone(),
                            content.clone(),
                        );
                        shared.1.notify_all();
                    }
                }
            }
            drop(snapshot);
            {
                let mut mailbox = lock(&shared);
                if checkpoint {
                    let failed: Vec<_> = reader
                        .parts
                        .iter()
                        .filter(|(part, state)| part.selected(&reader.scope) && state.failures > 0)
                        .map(|(part, _)| part.name())
                        .collect();
                    let reason = (!failed.is_empty())
                        .then(|| format!("Work confirmation unavailable: {}", failed.join(", ")));
                    enqueue(
                        &mut mailbox,
                        "checkpoint",
                        &origin,
                        reason,
                        WorkContent::Checkpoint,
                    );
                }
                shared.1.notify_all();
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
                    .expect("work reader mailbox poisoned"),
            );
        }
    })();
    lock(&shared).closed = true;
    shared.1.notify_all();
    writer.join().expect("work reader output panicked");
    result
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::{fingerprint, Part, PartState, StoreRevisions, WorkContent};
    use crate::lf::commands::top::ActivitySnapshot;

    #[test]
    fn planning_failure_keeps_the_stored_workflow_readable() {
        let _ledger = crate::journal::TestLedgerGuard::new();
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("released.db");
        let conn = rusqlite::Connection::open(&database).unwrap();
        crate::store::migrations::apply_before_current_draft(&conn, "task_paused");
        let task = crate::durable::TaskId::new();
        let fixture = include_str!("../../../tests/fixtures/migrations/work_stream_released.sql")
            .replace("{wave}", crate::id::WaveId::new().as_str())
            .replace("{project}", crate::durable::ProjectId::new().as_str())
            .replace("{task}", task.as_str())
            .replace("{repo}", directory.path().to_str().unwrap());
        conn.execute_batch(&fixture).unwrap();
        crate::store::migrations::upgrade_from_draft(&conn, "task_paused");
        conn.execute_batch("ALTER TABLE tasks DROP COLUMN paused")
            .unwrap();
        let frontier = crate::store::migrations::latest_applied_version_sqlite(&conn).unwrap();
        // Inject a planning-only failure after upgrading retained work, then
        // prove the independent Task read needs no repair or writer.
        let reader = super::Reader {
            store: Some(std::sync::Arc::new(crate::store::Store {
                sqlite: crate::store::sqlite::SqliteStore::open_read_only(&database).unwrap(),
            })),
            database,
            refused: None,
            scope: super::Scope {
                task: Some("PROOF-1".into()),
                ..Default::default()
            },
            parts: Default::default(),
            checkouts: Default::default(),
            watched_at: None,
            quiet: None,
            runtime: tokio::runtime::Runtime::new().unwrap(),
            planning_sync: None,
        };
        let reason = reader.project(Part::Planning).unwrap_err().to_string();
        assert!(reason.contains("no such column: t.paused"), "{reason}");
        let WorkContent::Task(Some(reading)) = reader.project(Part::Task).unwrap() else {
            panic!("the independent Task reading must remain available");
        };
        // The released row stood at a node; graphs replaced that definition, so
        // the next run takes one up again and the Task reads with none.
        assert!(reading.work.graph.is_none());
        assert_eq!(
            crate::store::migrations::latest_applied_version_sqlite(&conn).unwrap(),
            frontier
        );
        assert_eq!(
            conn.query_row(
                "SELECT COUNT(*) FROM pragma_table_info('tasks') WHERE name='paused'",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
    }

    fn revisions(planning: i64, sessions: i64, graphs: i64, processes: i64) -> StoreRevisions {
        StoreRevisions {
            planning,
            sessions,
            graphs,
            processes,
            usage: 0,
        }
    }

    #[test]
    fn process_changes_reread_sessions_and_os_death_has_a_clock() {
        let old = revisions(1, 1, 1, 1);
        assert!(Part::Sessions.changed(old, revisions(1, 1, 1, 2)));
        assert_eq!(Part::Sessions.clock(), Part::Activity.clock());
        assert_eq!(Part::Task.clock(), Part::Activity.clock());
        assert!(Part::Planning.changed(old, revisions(1, 1, 1, 2)));
        assert!(Part::Sessions.changed(old, revisions(2, 1, 1, 1)));
        assert!(!Part::Activity.changed(old, revisions(2, 2, 2, 1)));
    }

    #[test]
    fn usage_alone_rereads_only_wave_detail_and_lets_it_rest() {
        let old = revisions(1, 1, 1, 1);
        let used = StoreRevisions { usage: 5, ..old };
        for part in Part::ALL {
            assert_eq!(part.changed(old, used), part == Part::Wave, "{part:?}");
        }
        assert!(Part::Wave.rests(old, used));
        assert!(!Part::Wave.rests(
            old,
            StoreRevisions {
                sessions: 2,
                ..used
            }
        ));
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
            WorkContent::Activity(Some(ActivitySnapshot {
                schema_version: 1,
                observed_at,
                nodes: Vec::new(),
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
