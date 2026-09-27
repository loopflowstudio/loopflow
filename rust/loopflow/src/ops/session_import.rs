//! One-time import of the Session files old Homes kept beside their Runs,
//! and of the Runs themselves, which had no rows.
//!
//! A person runs `lf session import` once per Home, offline. No other command
//! reads these files, and this module is the only place that knows their
//! shapes. Sources stay where they are as evidence. Importing again stores
//! nothing new, so an interrupted import is finished by running it again.
//!
//! Delete this module, its command and its test once every Home that ran a
//! release older than the Session tables has been imported.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

use crate::durable::{RunId, TaskId, WorkRef};
use crate::engine::invocation::QueuedInvocation;
use crate::engine::{ConcreteStep, ExecutionCursor};
use crate::id::WaveId;
use crate::ops::flow_run::{FlowRun, StepToken};
use crate::run_record::{AttributionSource, RunFlowMembership, RunManifest};
use crate::session::{Run, Session, SessionKind, TitleSource, WorkSource};
use crate::store::SharedStore;

#[derive(Debug, Serialize)]
pub struct ImportReport {
    pub dry_run: bool,
    pub interactive: usize,
    pub ask: usize,
    pub flow_review: usize,
    /// Task reviews the schema migration stored, given their name and provider.
    pub task_review: usize,
    /// Runs outside any Session.
    pub run: usize,
    pub unchanged: usize,
    /// Tasks whose first stored Run is an imported one: `started_at` is the
    /// time of this import, not of the conversation.
    pub tasks_started: Vec<TaskId>,
    pub failed: Vec<ImportFailure>,
}

#[derive(Debug, Serialize)]
pub struct ImportFailure {
    pub path: PathBuf,
    pub reason: String,
}

// ---- Old file shapes ----

/// `runs/<prefix>/<run>/session-name.json`
#[derive(Deserialize)]
struct NameFile {
    title: String,
    source: TitleSource,
}

/// `runs/<prefix>/<run>/session-resolution.json`
#[derive(Deserialize)]
struct ResolutionFile {
    #[serde(with = "time::serde::rfc3339")]
    resolved_at: OffsetDateTime,
}

/// `human-sessions/<id>.json`
#[derive(Deserialize)]
struct AskFile {
    id: String,
    parent_run_id: RunId,
    work: Option<WorkRef>,
    title: String,
    prompt: String,
    skill: Option<String>,
    cwd: PathBuf,
    model: String,
    session_run_id: Option<RunId>,
    ready_summary: Option<String>,
    status: AskStatus,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum AskStatus {
    Waiting,
    Completed { summary: String },
}

/// The field `flows/<id>/position.json` kept on its active boundary.
#[derive(Deserialize)]
struct FlowFile {
    active: Option<FlowFileBoundary>,
}

#[derive(Deserialize)]
struct FlowFileBoundary {
    ready_summary: Option<String>,
}

// ---- Import ----

struct Import<'a> {
    store: &'a SharedStore,
    home: PathBuf,
    report: ImportReport,
    /// Runs the Ask and Flow files name; the Run scan leaves them alone.
    claimed: HashSet<RunId>,
}

pub(crate) async fn import(store: &SharedStore, dry_run: bool) -> Result<ImportReport> {
    let mut import = Import {
        store,
        home: crate::store::current_home_lf_home_dir(),
        report: ImportReport {
            dry_run,
            interactive: 0,
            ask: 0,
            flow_review: 0,
            task_review: 0,
            run: 0,
            unchanged: 0,
            tasks_started: Vec::new(),
            failed: Vec::new(),
        },
        claimed: HashSet::new(),
    };
    for path in files(&import.home.join("human-sessions"), |path| {
        path.extension()
            .is_some_and(|extension| extension == "json")
    })? {
        let stored = import.ask(&path).await;
        import.count(&path, stored);
    }
    for dir in files(&import.home.join("flows"), |path| path.is_dir())? {
        let path = dir.join("position.json");
        let stored = import.flow_review(&path).await;
        import.count(&path, stored);
    }
    for dir in crate::run_record::record_dirs(&import.home)? {
        let stored = import.run(&dir).await;
        import.count(&dir, stored);
    }
    Ok(import.report)
}

enum Stored {
    Interactive,
    Ask,
    FlowReview,
    TaskReview,
    Run,
    Unchanged,
}

fn files(dir: &Path, keep: impl Fn(&Path) -> bool) -> Result<Vec<PathBuf>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error).with_context(|| format!("read {}", dir.display())),
    };
    let mut paths = Vec::new();
    for entry in entries {
        let path = entry?.path();
        let hidden = path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().starts_with('.'));
        if !hidden && keep(&path) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn modified(path: &Path) -> Result<i64> {
    let modified = std::fs::metadata(path)?.modified()?;
    Ok(OffsetDateTime::from(modified).unix_timestamp())
}

/// The name a conversation was given, or its seed.
fn name(dir: &Path, seed: String) -> Result<(String, TitleSource)> {
    match std::fs::read(dir.join("session-name.json")) {
        Ok(bytes) => {
            let file: NameFile = serde_json::from_slice(&bytes).context("session-name.json")?;
            Ok((file.title, file.source))
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok((seed, TitleSource::Generated))
        }
        Err(error) => Err(error).context("session-name.json"),
    }
}

impl Import<'_> {
    /// `None` is a file that holds no Session.
    fn count(&mut self, path: &Path, stored: Result<Option<Stored>>) {
        let report = &mut self.report;
        match stored {
            Ok(Some(Stored::Interactive)) => report.interactive += 1,
            Ok(Some(Stored::Ask)) => report.ask += 1,
            Ok(Some(Stored::FlowReview)) => report.flow_review += 1,
            Ok(Some(Stored::TaskReview)) => report.task_review += 1,
            Ok(Some(Stored::Run)) => report.run += 1,
            Ok(Some(Stored::Unchanged)) => report.unchanged += 1,
            Ok(None) => {}
            Err(error) => report.failed.push(ImportFailure {
                path: path.to_path_buf(),
                reason: format!("{error:#}"),
            }),
        }
    }

    /// Store a Session with its Run unless it is stored already.
    async fn store(
        &mut self,
        kind: Stored,
        session: Session,
        run: Run,
        review: Option<(QueuedInvocation, ExecutionCursor)>,
    ) -> Result<Option<Stored>> {
        if self.store.session(&session.id).await?.is_some() {
            return Ok(Some(Stored::Unchanged));
        }
        if self.store.run(&run.id).await?.is_some() {
            bail!("Run {} is already stored outside this Session", run.id);
        }
        self.start(&run).await?;
        if !self.report.dry_run {
            self.store.create_session(session, run, review).await?;
        }
        Ok(Some(kind))
    }

    /// Report the Task this Run starts, if it is the Task's first.
    async fn start(&mut self, run: &Run) -> Result<()> {
        let Some(task) = &run.task_id else {
            return Ok(());
        };
        if self.store.get_task(task).await?.is_none() {
            bail!("Task {task} is not registered");
        }
        let runs = self.store.runs(None, None, Some(task.as_str()), None, 0);
        if runs.await?.is_empty() && !self.report.tasks_started.contains(task) {
            self.report.tasks_started.push(task.clone());
        }
        Ok(())
    }

    /// A Run outside any Session. A Flow step's Run keeps its Task and Wave;
    /// the invocation's cursor has moved on, so the row names no invocation.
    async fn headless(&mut self, dir: &Path, manifest: RunManifest) -> Result<Option<Stored>> {
        let (task_id, wave_id, work_source) = self.work(&manifest).await?;
        let evidence = crate::run_record::read_run_snapshot(dir)?;
        let run = Run {
            id: manifest.run_id,
            session_id: None,
            invocation_id: None,
            node: None,
            iterations: None,
            attempt: None,
            task_id,
            wave_id,
            work_source,
            created_at: manifest.created_at.unix_timestamp(),
            published: true,
            cwd: manifest.cwd,
            skill: manifest.skill,
            provider: Some(manifest.harness),
            model: manifest.model,
            caller_run_id: manifest.parent_run_id,
            ended: evidence
                .outcome
                .zip(evidence.ended)
                .map(|(outcome, at)| crate::session::RunEnd { outcome, at }),
        };
        self.start(&run).await?;
        if !self.report.dry_run {
            self.store.create_run(run).await?;
        }
        Ok(Some(Stored::Run))
    }

    fn run_dir(&self, run: &RunId) -> Result<PathBuf> {
        crate::run_record::record_dir(&self.home, run)
            .ok_or_else(|| anyhow!("Run {run} has an invalid id"))
    }

    async fn ask(&mut self, path: &Path) -> Result<Option<Stored>> {
        let file: AskFile = serde_json::from_slice(&std::fs::read(path)?)?;
        self.claimed.extend(file.session_run_id.clone());
        let (run_id, published, dir) = match file.session_run_id {
            Some(id) => {
                let dir = self.run_dir(&id)?;
                (id, dir.join("manifest.json").exists(), Some(dir))
            }
            None => (RunId::new(), false, None),
        };
        let created_at = match &dir {
            Some(dir) if published => crate::run_record::read_manifest(dir)?
                .created_at
                .unix_timestamp(),
            _ => modified(path)?,
        };
        let (title, title_source) = match &dir {
            Some(dir) => name(dir, file.title)?,
            None => (file.title, TitleSource::Generated),
        };
        let (task_id, wave_id) = match file.work {
            Some(WorkRef::Task(task)) => (Some(task), None),
            Some(WorkRef::Wave(wave)) => (None, Some(wave)),
            Some(WorkRef::Project(project)) => {
                bail!("an Ask belongs to a Task or a Wave, not Project {project}")
            }
            None => (None, None),
        };
        let (answer, completed_at) = match file.status {
            AskStatus::Waiting => (file.ready_summary, None),
            AskStatus::Completed { summary } => (Some(summary), Some(modified(path)?)),
        };
        let (provider, model) = crate::engine::config::parse_agent(&file.model);
        let run = Run {
            id: run_id,
            session_id: Some(file.id.clone()),
            invocation_id: None,
            node: None,
            iterations: None,
            attempt: None,
            work_source: (task_id.is_some() || wave_id.is_some()).then_some(WorkSource::Inherited),
            task_id,
            wave_id,
            created_at,
            published,
            cwd: file.cwd,
            skill: file.skill,
            provider: Some(provider),
            model,
            caller_run_id: Some(file.parent_run_id),
            ended: None,
        };
        let session = Session {
            id: file.id,
            current_run_id: run.id.clone(),
            kind: SessionKind::Ask,
            title,
            title_source,
            request: Some(file.prompt),
            ready_summary: answer,
            completed_at,
            created_at,
        };
        self.store(Stored::Ask, session, run, None).await
    }

    async fn flow_review(&mut self, path: &Path) -> Result<Option<Stored>> {
        let bytes = std::fs::read(path)?;
        let flow: FlowRun = serde_json::from_slice(&bytes)?;
        let file: FlowFile = serde_json::from_slice(&bytes)?;
        let (Some(active), Some(old)) = (flow.active.clone(), file.active) else {
            return Ok(None);
        };
        let skill = match flow.current_step()? {
            ConcreteStep::Skill(skill) if skill.policy.human && !flow.finished => {
                skill.skill.name.clone()
            }
            _ => return Ok(None),
        };
        self.claimed.extend(active.run_id.clone());
        let id = format!("flow:{}:{}", flow.id, active.id);
        let Some(run_id) = active.run_id else {
            // Never opened: nothing to preserve but the wait itself.
            if self.store.session(&id).await?.is_some() {
                return Ok(Some(Stored::Unchanged));
            }
            if !self.report.dry_run {
                let token = StepToken {
                    invocation: flow.id,
                    boundary: active.id,
                };
                crate::ops::flow_session::reserve(self.store, &token).await?;
            }
            return Ok(Some(Stored::FlowReview));
        };
        let dir = self.run_dir(&run_id)?;
        let manifest = crate::run_record::read_manifest(&dir).context("the review's Run record")?;
        let (title, title_source) = name(&dir, skill.clone())?;
        let work = crate::ops::flow_session::declared_work(self.store, &flow).await;
        let created_at = manifest.created_at.unix_timestamp();
        let run = Run {
            id: run_id,
            session_id: Some(id.clone()),
            invocation_id: Some(flow.id.clone()),
            node: None,
            iterations: None,
            attempt: None,
            task_id: work.as_ref().and_then(|work| work.task_id.clone()),
            work_source: work.as_ref().map(|_| WorkSource::Declared),
            wave_id: work.and_then(|work| work.wave_id),
            created_at,
            published: true,
            cwd: manifest.cwd,
            skill: Some(skill),
            provider: Some(manifest.harness),
            model: manifest.model,
            caller_run_id: None,
            ended: None,
        };
        let session = Session {
            id,
            current_run_id: run.id.clone(),
            kind: SessionKind::FlowReview,
            title,
            title_source,
            request: None,
            ready_summary: old.ready_summary,
            completed_at: match active.completed {
                true => Some(modified(path)?),
                false => None,
            },
            created_at,
        };
        let invocation = QueuedInvocation {
            id: flow.id,
            flow: flow.flow,
            steps: flow.steps,
        };
        self.store(
            Stored::FlowReview,
            session,
            run,
            Some((invocation, flow.cursor)),
        )
        .await
    }

    async fn run(&mut self, dir: &Path) -> Result<Option<Stored>> {
        let manifest = crate::run_record::read_manifest(dir)?;
        if self.claimed.contains(&manifest.run_id) {
            return Ok(None);
        }
        if let Some(run) = self.store.run(&manifest.run_id).await? {
            return self.review_evidence(dir, &manifest, run).await;
        }
        let conversation =
            manifest.surface == "tui" || dir.join("provider-clients").try_exists()?;
        // A Flow kept only its current review, so an earlier review's Run and
        // every headless Run are Runs with no Session.
        if !conversation
            || crate::run_record::read_provider_session(dir)?.is_none()
            || matches!(manifest.flow, Some(RunFlowMembership::Step(_)))
        {
            return self.headless(dir, manifest).await;
        }
        let (task_id, wave_id, work_source) = self.work(&manifest).await?;
        let seed = manifest
            .skill
            .clone()
            .unwrap_or_else(|| crate::engine::naming::word_pair(manifest.run_id.as_str()));
        let (title, title_source) = name(dir, seed)?;
        let completed_at = match std::fs::read(dir.join("session-resolution.json")) {
            Ok(bytes) => Some(
                serde_json::from_slice::<ResolutionFile>(&bytes)
                    .context("session-resolution.json")?
                    .resolved_at
                    .unix_timestamp(),
            ),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error).context("session-resolution.json"),
        };
        let created_at = manifest.created_at.unix_timestamp();
        // An old Home named an interactive Session by its Run.
        let id = manifest.run_id.to_string();
        let run = Run {
            id: manifest.run_id,
            session_id: Some(id.clone()),
            invocation_id: None,
            node: None,
            iterations: None,
            attempt: None,
            task_id,
            wave_id,
            work_source,
            created_at,
            published: true,
            cwd: manifest.cwd,
            skill: manifest.skill,
            provider: Some(manifest.harness),
            model: manifest.model,
            caller_run_id: None,
            ended: None,
        };
        let session = Session {
            id,
            current_run_id: run.id.clone(),
            kind: SessionKind::Interactive,
            title,
            title_source,
            request: None,
            ready_summary: None,
            completed_at,
            created_at,
        };
        self.store(Stored::Interactive, session, run, None).await
    }

    /// The Task and Wave a Run's manifest names, by the selectors it recorded.
    async fn work(
        &self,
        manifest: &RunManifest,
    ) -> Result<(Option<TaskId>, Option<WaveId>, Option<WorkSource>)> {
        let subject = |kind: &str| {
            manifest.subjects.iter().find_map(|subject| {
                let selector = subject.selector.strip_prefix(kind)?.strip_prefix(':')?;
                Some((
                    selector,
                    match subject.source {
                        AttributionSource::Declared => WorkSource::Declared,
                        AttributionSource::Inherited => WorkSource::Inherited,
                    },
                ))
            })
        };
        if let Some((selector, source)) = subject("task") {
            let task = match TaskId::parse(selector) {
                Ok(id) => self.store.get_task(&id).await?,
                Err(_) => self.store.get_task_by_issue(selector).await?,
            }
            .ok_or_else(|| anyhow!("Task {selector} is not registered"))?;
            return Ok((Some(task.id), Some(task.wave_id), Some(source)));
        }
        if let Some((selector, source)) = subject("wave") {
            let waves = self.store.find_waves_by_slug(selector).await?;
            let [wave] = waves.as_slice() else {
                bail!("Wave {selector} names {} registered Waves", waves.len());
            };
            return Ok((None, Some(wave.id().clone()), Some(source)));
        }
        Ok((None, None, None))
    }

    /// A review the schema migration stored takes its name and provider from
    /// its current Run's record. A name a person gave the row stays.
    async fn review_evidence(
        &mut self,
        dir: &Path,
        manifest: &RunManifest,
        run: Run,
    ) -> Result<Option<Stored>> {
        let Some((session, current)) = self.store.session_for_run(&run.id).await? else {
            return Ok(None);
        };
        if current.id != run.id {
            return Ok(None);
        }
        let (title, source) = name(dir, session.title.clone())?;
        let renamed = title != session.title && session.title_source == TitleSource::Generated;
        let unnamed = run.provider.is_none();
        if !renamed && !unnamed {
            return Ok(Some(Stored::Unchanged));
        }
        if !self.report.dry_run {
            if renamed {
                self.store
                    .rename_session(&session.id, Some(&run.id), &title, source)
                    .await?;
            }
            self.store
                .fill_run_provider(&run.id, &manifest.harness, manifest.model.as_deref())
                .await?;
        }
        Ok(Some(Stored::TaskReview))
    }
}
