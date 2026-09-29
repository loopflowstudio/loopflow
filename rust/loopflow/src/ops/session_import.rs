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

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use time::OffsetDateTime;

use crate::durable::{FlowSession, RunId, TaskId, WorkRef};
use crate::engine::invocation::QueuedInvocation;
use crate::engine::{ConcreteStep, ExecutionCursor};
use crate::id::WaveId;
use crate::run_record::{AttributionSource, RunFlowMembership, RunManifest};
use crate::session::{AgentSession, ImportedObservation, SessionKind, TitleSource, WorkSource};
use crate::store::SharedStore;

#[derive(Debug, Serialize)]
pub struct ImportReport {
    pub dry_run: bool,
    pub interactive: usize,
    pub ask: usize,
    pub flow_review: usize,
    /// Captured Flows without a pending human review, including completed history.
    pub flow: usize,
    /// Task reviews the schema migration stored, given their name and provider.
    pub task_review: usize,
    /// Runs outside any Session.
    pub run: usize,
    pub unchanged: usize,
    /// Tasks first assigned during import: `started_at` is the
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

/// `flows/<id>/position.json`: the saved Flow's definition, cursor, launch
/// selectors and active review boundary, before the invocation row owned them.
#[derive(Deserialize)]
struct FlowFile {
    id: String,
    flow: String,
    cwd: PathBuf,
    steps: Vec<ConcreteStep>,
    cursor: ExecutionCursor,
    message: Option<String>,
    model: Option<String>,
    wave: Option<String>,
    task: Option<String>,
    as_work: Option<String>,
    active: Option<FlowFileBoundary>,
    failure: Option<crate::durable::TaskFlowBlocker>,
    finished: bool,
}

#[derive(Deserialize)]
struct FlowFileBoundary {
    id: String,
    run_id: Option<RunId>,
    completed: bool,
    ready_summary: Option<String>,
}

impl FlowFile {
    fn current_step(&self) -> Option<&ConcreteStep> {
        let (steps, cursor) = self.cursor.current_body(&self.steps);
        steps.get(cursor.index)
    }

    /// The Work the Flow was launched with, resolved once from its selectors.
    async fn declared_work(&self, store: &SharedStore) -> Result<Option<(Option<TaskId>, WaveId)>> {
        let mut work: Option<(Option<TaskId>, WaveId)> = None;
        for selector in self
            .as_work
            .iter()
            .cloned()
            .chain(self.task.iter().map(|id| format!("task:{id}")))
            .chain(self.wave.iter().map(|id| format!("wave:{id}")))
        {
            let (task, wave) = recorded_work(store, &selector).await?;
            if let Some((known_task, known_wave)) = &work {
                if known_wave != &wave
                    || known_task
                        .as_ref()
                        .zip(task.as_ref())
                        .is_some_and(|(a, b)| a != b)
                {
                    bail!("Flow {} records conflicting Work ancestry", self.id);
                }
            }
            let task = task.or_else(|| work.as_ref().and_then(|(task, _)| task.clone()));
            work = Some((task, wave));
        }
        Ok(work)
    }

    fn invocation(&self, work: Option<(Option<TaskId>, WaveId)>) -> FlowSession {
        let (task_id, wave_id) = match work {
            Some((task, wave)) => (task, Some(wave)),
            None => (None, None),
        };
        FlowSession {
            invocation: QueuedInvocation {
                id: self.id.clone(),
                flow: self.flow.clone(),
                steps: self.steps.clone(),
            },
            cursor: self.cursor.clone(),
            version: 1,
            task_id,
            wave_id,
            cwd: self.cwd.clone(),
            message: self.message.clone(),
            model: self.model.clone(),
            current_attempt: None,
            pending_session_id: None,
            ready_summary: None,
            worker_generation: 0,
            claim: None,
            failure: self.failure.clone(),
            finished: self.finished,
            updated_at: time::OffsetDateTime::now_utc(),
        }
    }
}

// ---- Import ----

struct Import<'a> {
    store: &'a SharedStore,
    home: PathBuf,
    report: ImportReport,
    /// Runs the Ask and Flow files name; the Run scan leaves them alone.
    claimed: HashSet<RunId>,
    captures: HashMap<String, FlowSession>,
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
            flow: 0,
            task_review: 0,
            run: 0,
            unchanged: 0,
            tasks_started: Vec::new(),
            failed: Vec::new(),
        },
        claimed: HashSet::new(),
        captures: HashMap::new(),
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
        if stored.is_ok() {
            let file: FlowFile = serde_json::from_slice(&std::fs::read(&path)?)?;
            let work = file.declared_work(store).await?;
            let mut flow = file.invocation(work);
            flow.updated_at = OffsetDateTime::from_unix_timestamp(modified(&path)?)?;
            import.captures.insert(file.id, flow);
        }
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
    Flow,
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

/// Recorded attribution is independent of whether the Task may launch now.
async fn recorded_work(store: &SharedStore, selector: &str) -> Result<(Option<TaskId>, WaveId)> {
    if let Some(selector) = selector.strip_prefix("task:") {
        let task = match TaskId::parse(selector) {
            Ok(id) => store.get_task(&id).await?,
            Err(_) => store.get_task_by_issue(selector).await?,
        }
        .ok_or_else(|| anyhow!("Task {selector} is not registered"))?;
        return Ok((Some(task.id), task.wave_id));
    }
    if let Some(selector) = selector.strip_prefix("wave:") {
        if let Ok(id) = WaveId::parse(selector) {
            if let Some(wave) = store.get_wave(&id).await? {
                return Ok((None, wave.id().clone()));
            }
        }
        let waves = store.find_waves_by_slug(selector).await?;
        let [wave] = waves.as_slice() else {
            bail!("Wave {selector} names {} registered Waves", waves.len());
        };
        return Ok((None, wave.id().clone()));
    }
    bail!("historical attribution must name a Task or Wave: {selector}")
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
            Ok(Some(Stored::Flow)) => report.flow += 1,
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

    /// Import compares captured facts; Session identity alone is not equality.
    async fn store(
        &mut self,
        kind: Stored,
        session: AgentSession,
        review: Option<FlowSession>,
    ) -> Result<Option<Stored>> {
        let input = session.input_id.clone();
        let starts = self.first_assignment(session.task_id.as_ref()).await?;
        let history = self.history(&session.input_id).await?;
        let changed = self
            .store
            .import_session(session, review, history, self.report.dry_run)
            .await?;
        self.claimed.insert(input);
        if changed {
            self.report.tasks_started.extend(starts);
        }
        Ok(Some(if changed { kind } else { Stored::Unchanged }))
    }

    async fn history(&self, input: &RunId) -> Result<Vec<ImportedObservation>> {
        let dir = self.run_dir(input)?;
        let mut history = Vec::new();
        let (task_id, wave_id, _) = match crate::run_record::read_manifest(&dir) {
            Ok(manifest) => self.work(&manifest).await?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => (None, None, None),
            Err(error) => return Err(error.into()),
        };
        let mut record = |source: String, evidence: serde_json::Value| {
            let at = evidence
                .get("observed_at")
                .or_else(|| evidence.get("ended_at"))
                .or_else(|| evidence.get("created_at"))
                .and_then(serde_json::Value::as_str)
                .and_then(|at| {
                    OffsetDateTime::parse(at, &time::format_description::well_known::Rfc3339).ok()
                });
            history.push(ImportedObservation {
                input_id: input.clone(),
                source: source.clone(),
                observed_at: at.unwrap_or_else(OffsetDateTime::now_utc).unix_timestamp(),
                task_id: task_id.clone(),
                wave_id: wave_id.clone(),
                payload: serde_json::json!({"input_id": input, "source": source, "evidence": evidence}),
            });
        };
        for name in ["manifest.json", "terminal.json", "provider-session.json"] {
            match std::fs::read(dir.join(name)) {
                Ok(bytes) => record(
                    name.into(),
                    serde_json::from_slice(&bytes).with_context(|| name.to_string())?,
                ),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        match std::fs::read_to_string(dir.join("events.jsonl")) {
            Ok(events) => {
                for (line, bytes) in events.lines().enumerate() {
                    // Partial JSONL remains explicit evidence, never silently discarded.
                    let evidence = serde_json::from_str(bytes)
                        .unwrap_or_else(|_| serde_json::json!({"unparsed": bytes}));
                    record(format!("events.jsonl:{line}"), evidence);
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        Ok(history)
    }

    /// Report first assignment, including existing conversation bindings.
    async fn first_assignment(&self, task: Option<&TaskId>) -> Result<Option<TaskId>> {
        let Some(task) = task else {
            return Ok(None);
        };
        if self.store.get_task(task).await?.is_none() {
            bail!("Task {task} is not registered");
        }
        if !self.store.task_started(task).await? && !self.report.tasks_started.contains(task) {
            return Ok(Some(task.clone()));
        }
        Ok(None)
    }

    /// Historical agent inputs retain their exact recorded Flow occurrence.
    async fn headless(&mut self, dir: &Path, manifest: RunManifest) -> Result<Option<Stored>> {
        let (task_id, wave_id, work_source) = self.work(&manifest).await?;
        let mut capture = None;
        let (flow_session_id, node, iterations) = match &manifest.flow {
            Some(RunFlowMembership::Step(step)) => {
                let flow = self
                    .captures
                    .get(&step.invocation_id)
                    .cloned()
                    .or(self.store.flow(&step.invocation_id).await?)
                    .ok_or_else(|| {
                        anyhow!(
                            "input {} names unavailable capture {}",
                            manifest.run_id,
                            step.invocation_id
                        )
                    })?;
                let graph = crate::engine::flow_graph::FlowGraph::new(
                    &flow.invocation.flow,
                    &flow.invocation.steps,
                );
                let node = step
                    .node
                    .as_ref()
                    .map(|key| {
                        let mut index = 0;
                        while let Some(node) = graph.node_at(index) {
                            if &node.key == key {
                                return Ok(index);
                            }
                            index += 1;
                        }
                        bail!("input {} names an uncaptured node {key}", manifest.run_id)
                    })
                    .transpose()?;
                capture = Some(flow);
                (
                    Some(step.invocation_id.clone()),
                    node,
                    step.iterations.clone(),
                )
            }
            _ => (None, None, None),
        };
        let (title, title_source) = name(
            dir,
            manifest
                .skill
                .clone()
                .unwrap_or_else(|| crate::engine::naming::word_pair(manifest.run_id.as_str())),
        )?;
        let session = AgentSession {
            id: manifest.run_id.to_string(),
            input_id: manifest.run_id,
            caller_input_id: manifest.parent_run_id,
            input_published: true,
            cwd: manifest.cwd,
            skill: manifest.skill,
            provider: Some(manifest.harness),
            model: manifest.model,
            node,
            iterations,
            task_id,
            wave_id,
            flow_session_id,
            work_source,
            bound_at: None,
            kind: SessionKind::Conversation,
            interactive: false,
            repo: manifest
                .repo
                .map(|path| path.to_string_lossy().into_owned()),
            title,
            title_source,
            request: None,
            ready_summary: None,
            completed_at: None,
            created_at: manifest.created_at.unix_timestamp(),
        };
        self.store(Stored::Run, session, capture).await
    }

    fn run_dir(&self, run: &RunId) -> Result<PathBuf> {
        crate::run_record::record_dir(&self.home, run)
            .ok_or_else(|| anyhow!("Run {run} has an invalid id"))
    }

    async fn ask(&mut self, path: &Path) -> Result<Option<Stored>> {
        let file: AskFile = serde_json::from_slice(&std::fs::read(path)?)?;
        let (run_id, published, dir) = match file.session_run_id {
            Some(id) => {
                let dir = self.run_dir(&id)?;
                (id, dir.join("manifest.json").exists(), Some(dir))
            }
            None => {
                let digest = Sha256::digest(file.id.as_bytes());
                let id = RunId::parse(&format!("run_{}", hex::encode(&digest[..16])))?;
                (id, false, None)
            }
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
        let work_source = (task_id.is_some() || wave_id.is_some()).then_some(WorkSource::Inherited);
        let session = AgentSession {
            caller_input_id: Some(file.parent_run_id),
            task_id,
            wave_id,
            flow_session_id: None,
            work_source,
            bound_at: None,
            id: file.id,
            input_id: run_id,
            input_published: published,
            cwd: file.cwd,
            skill: file.skill,
            provider: Some(provider),
            model,
            node: None,
            iterations: None,
            kind: SessionKind::Ask,
            interactive: true,
            repo: None,
            title,
            title_source,
            request: Some(file.prompt),
            ready_summary: answer,
            completed_at,
            created_at,
        };
        self.store(Stored::Ask, session, None).await
    }

    async fn flow_review(&mut self, path: &Path) -> Result<Option<Stored>> {
        let file: FlowFile = serde_json::from_slice(&std::fs::read(path)?)?;
        let work = file.declared_work(self.store).await?;
        let mut flow = file.invocation(work.clone());
        flow.updated_at = OffsetDateTime::from_unix_timestamp(modified(path)?)?;
        let human =
            matches!(file.current_step(), Some(ConcreteStep::Skill(skill)) if skill.policy.human);
        if file.active.is_none() || !human || file.finished {
            return Ok(Some(
                if self.store.import_flow(flow, self.report.dry_run).await? {
                    Stored::Flow
                } else {
                    Stored::Unchanged
                },
            ));
        }
        let Some(active) = &file.active else {
            return Ok(None);
        };
        let skill = match file.current_step() {
            Some(ConcreteStep::Skill(skill)) if skill.policy.human && !file.finished => {
                skill.skill.name.clone()
            }
            _ => return Ok(None),
        };
        let id = format!("flow:{}:{}", file.id, active.id);
        let Some(run_id) = active.run_id.clone() else {
            // Never opened: nothing to preserve but the wait itself.
            let stored = self.store.flow(&file.id).await?;
            if stored.is_some_and(|flow| flow.pending_session_id.is_some()) {
                return Ok(Some(Stored::Unchanged));
            }
            if !self.report.dry_run {
                let flow = self.store.create_flow(flow).await?;
                crate::ops::flow_session::reserve(self.store, &flow).await?;
            }
            return Ok(Some(Stored::FlowReview));
        };
        let dir = self.run_dir(&run_id)?;
        let manifest = crate::run_record::read_manifest(&dir).context("the review's Run record")?;
        let (title, title_source) = name(&dir, skill.clone())?;
        let created_at = manifest.created_at.unix_timestamp();
        let session = AgentSession {
            caller_input_id: manifest.parent_run_id,
            task_id: flow.task_id.clone(),
            wave_id: flow.wave_id.clone(),
            flow_session_id: Some(file.id.clone()),
            work_source: work.as_ref().map(|_| WorkSource::Declared),
            bound_at: None,
            id,
            input_id: run_id,
            input_published: true,
            cwd: manifest.cwd,
            skill: Some(skill),
            provider: Some(manifest.harness),
            model: manifest.model,
            node: None,
            iterations: None,
            kind: SessionKind::FlowReview,
            interactive: true,
            repo: manifest
                .repo
                .map(|path| path.to_string_lossy().into_owned()),
            title,
            title_source,
            request: None,
            ready_summary: active.ready_summary.clone(),
            completed_at: match active.completed {
                true => Some(modified(path)?),
                false => None,
            },
            created_at,
        };
        self.store(Stored::FlowReview, session, Some(flow)).await
    }

    async fn run(&mut self, dir: &Path) -> Result<Option<Stored>> {
        let manifest = crate::run_record::read_manifest(dir)?;
        if self.claimed.contains(&manifest.run_id) {
            return Ok(None);
        }
        if let Some(session) = self.store.session_for_run(&manifest.run_id).await? {
            return self.review_evidence(dir, &manifest, session).await;
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
        let session = AgentSession {
            caller_input_id: manifest.parent_run_id,
            task_id,
            wave_id,
            flow_session_id: None,
            work_source,
            bound_at: None,
            id,
            input_id: manifest.run_id,
            input_published: true,
            cwd: manifest.cwd,
            skill: manifest.skill,
            provider: Some(manifest.harness),
            model: manifest.model,
            node: None,
            iterations: None,
            kind: SessionKind::Conversation,
            interactive: true,
            repo: manifest
                .repo
                .map(|path| path.to_string_lossy().into_owned()),
            title,
            title_source,
            request: None,
            ready_summary: None,
            completed_at,
            created_at,
        };
        self.store(Stored::Interactive, session, None).await
    }

    /// The Task and Wave a Run's manifest names, by the selectors it recorded.
    async fn work(
        &self,
        manifest: &RunManifest,
    ) -> Result<(Option<TaskId>, Option<WaveId>, Option<WorkSource>)> {
        let mut selected: Option<(Option<TaskId>, WaveId, WorkSource)> = None;
        for subject in &manifest.subjects {
            if !subject.selector.starts_with("task:") && !subject.selector.starts_with("wave:") {
                continue;
            }
            let (task, wave) = recorded_work(self.store, &subject.selector).await?;
            let source = match subject.source {
                AttributionSource::Declared => WorkSource::Declared,
                AttributionSource::Inherited => WorkSource::Inherited,
            };
            if let Some((known_task, known_wave, _)) = &selected {
                if known_wave != &wave
                    || known_task
                        .as_ref()
                        .zip(task.as_ref())
                        .is_some_and(|(a, b)| a != b)
                {
                    bail!(
                        "input {} records conflicting Work ancestry",
                        manifest.run_id
                    );
                }
                if known_task.is_some() {
                    continue;
                }
            }
            selected = Some((task, wave, source));
        }
        Ok(match selected {
            Some((task, wave, source)) => (task, Some(wave), Some(source)),
            None => (None, None, None),
        })
    }

    /// A review the schema migration stored takes its name and provider from
    /// its current Run's record. A name a person gave the row stays.
    async fn review_evidence(
        &mut self,
        dir: &Path,
        manifest: &RunManifest,
        session: AgentSession,
    ) -> Result<Option<Stored>> {
        if session.input_id != manifest.run_id {
            let history = self.history(&manifest.run_id).await?;
            let changed = self
                .store
                .import_session(session, None, history, self.report.dry_run)
                .await?;
            return Ok(Some(if changed {
                Stored::TaskReview
            } else {
                Stored::Unchanged
            }));
        }
        let (title, source) = name(dir, session.title.clone())?;
        let renamed = title != session.title && session.title_source == TitleSource::Generated;
        let unnamed = session.provider.is_none();
        let history = self.history(&session.input_id).await?;
        let imported = self
            .store
            .import_session(session.clone(), None, history, self.report.dry_run)
            .await?;
        if !renamed && !unnamed {
            return Ok(Some(if imported {
                Stored::TaskReview
            } else {
                Stored::Unchanged
            }));
        }
        if !self.report.dry_run {
            if renamed {
                self.store
                    .rename_session(&session.id, Some(&session.input_id), &title, source)
                    .await?;
            }
            self.store
                .fill_run_provider(
                    &session.input_id,
                    &manifest.harness,
                    manifest.model.as_deref(),
                )
                .await?;
        }
        Ok(Some(Stored::TaskReview))
    }
}
