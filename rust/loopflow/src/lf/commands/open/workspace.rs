//! Fresh native workspaces. Every `lf open` launches new Task tabs, each with its
//! splits, in a Rex session, Herdr workspace or cmux group. The host owns them
//! afterwards: LF records no host, container, tab or creation state. Native
//! handles live only in this command, or in a Rex follower while it runs.
mod layout;
mod tools;

use layout::layout;

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::{IsTerminal, Read};
use std::path::Path;

use anyhow::{bail, Context, Result};
use serde_json::Value;

use super::{
    cmux, herdr,
    rex::{self, Rex},
    OpenArgs, OpenHost, OpenReceipt, OpenReport, OpenStatus, Opening, Resolved,
};
use crate::durable::{MachineId, RepositoryId, TaskId};
use crate::id::WaveId;
use crate::ops::human_session::{session_state, viewed_summary, SessionState};
use crate::store::sqlite::SqliteStore;
use crate::work::selection::{
    Selection, SelectionExpression, SelectionFilter, SelectionId, SelectionState, SplitAxis,
    TaskLayoutTemplate, ViewContent, ViewPlan, WorkWorkspaceKey, WorkWorkspaceKind,
};

/// Where a Task's tools run: its recorded checkout, else the repository.
fn tool_cwd<'a>(content: &'a ViewContent, repo: &'a Path) -> &'a Path {
    content
        .location
        .as_ref()
        .map_or(repo, |location| Path::new(&location.cwd))
}

fn label(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).take(80).collect()
}

fn workspace_plan(
    store: &SqliteStore,
    repository: &RepositoryId,
    selected: Selection,
) -> Result<ViewPlan> {
    // Working-set projections know only local execution. Include backlog in the
    // inventory so the recorded remote Machine can decide whether its Task started.
    let inventory = Selection {
        id: selected.id.clone(),
        expression: match &selected.expression {
            SelectionExpression::Repository => SelectionExpression::Filter {
                filter: SelectionFilter::Kind(WorkWorkspaceKind::Task),
            },
            SelectionExpression::Wave { wave } => SelectionExpression::Filter {
                filter: SelectionFilter::Wave(wave.to_string()),
            },
            expression => expression.clone(),
        },
    };
    let mut plan = store.plan_selection(repository, inventory)?;
    plan.selection = selected;
    Ok(plan)
}

// ---------------------------------------------------------------------------
// Native hosts
// ---------------------------------------------------------------------------

/// One host and its caller context, resolved for this command only.
enum Native {
    Rex(Rex),
    Herdr(herdr::Herdr),
    Cmux(cmux::Cmux),
}

/// The outer container for this launch: a Rex session, Herdr workspace or cmux
/// group. `None` in cmux means the caller's ungrouped window.
#[derive(Debug, Clone)]
struct Container {
    id: Option<String>,
    /// Herdr creates a workspace with one empty tab; the first Task fills it.
    spare_tab: Option<String>,
}

impl Container {
    fn existing(id: impl Into<String>) -> Self {
        Self {
            id: Some(id.into()),
            spare_tab: None,
        }
    }
}

impl Native {
    fn resolve(host: OpenHost, server: Option<&str>) -> Result<Self> {
        Ok(match host {
            OpenHost::Rex => Self::Rex(Rex::local(server)?),
            OpenHost::Herdr => Self::Herdr(herdr::Herdr::local()?),
            OpenHost::Cmux => Self::Cmux(cmux::Cmux::local()?),
            other => bail!("{} has no native workspace renderer", other.label()),
        })
    }

    fn open_host(&self) -> OpenHost {
        match self {
            Self::Rex(_) => OpenHost::Rex,
            Self::Herdr(_) => OpenHost::Herdr,
            Self::Cmux(_) => OpenHost::Cmux,
        }
    }

    /// A Conversation pane. `lf resume` selects the conversation and its harness.
    fn conversation(&self, cwd: &Path, resume: &[&str]) -> Result<Value> {
        let argv = lf_argv(cwd, &[&["resume"], resume].concat())?;
        Ok(self.block("Conversation", cwd, argv))
    }

    fn block(&self, label: &str, cwd: &Path, argv: Vec<String>) -> Value {
        match self {
            Self::Rex(rex) => rex.block(label, cwd, argv),
            Self::Herdr(_) => herdr::terminal(label, cwd, argv),
            Self::Cmux(_) => cmux::terminal(label, cwd, argv),
        }
    }

    fn terminal(&self, label: &str, cwd: &Path, argv: Vec<String>) -> Value {
        match self {
            Self::Rex(rex) => rex.terminal(label, cwd, argv),
            Self::Herdr(_) => herdr::terminal(label, cwd, argv),
            Self::Cmux(_) => cmux::terminal(label, cwd, argv),
        }
    }

    fn split(&self, axis: SplitAxis, ratio: f64, before: Value, after: Value) -> Value {
        match self {
            Self::Rex(_) => rex::split(axis, ratio, before, after),
            Self::Herdr(_) => herdr::split(axis, ratio, before, after),
            Self::Cmux(_) => cmux::split(axis, ratio, before, after),
        }
    }

    /// The container the caller is in, when there is one.
    fn here(&self) -> Result<Option<Container>> {
        Ok(match self {
            Self::Rex(rex) => rex
                .caller
                .as_ref()
                .map(|caller| Container::existing(&caller.session)),
            Self::Herdr(herdr) => herdr.here()?.map(Container::existing),
            // The caller's group, else its window (cmux's default placement).
            Self::Cmux(cmux) => Some(Container {
                id: cmux.caller_group()?,
                spare_tab: None,
            }),
        })
    }

    fn create_container(&self, name: &str, cwd: &Path) -> Result<Container> {
        let name = label(name);
        match self {
            Self::Rex(rex) => Ok(Container::existing(rex.create_session(&name)?)),
            Self::Herdr(herdr) => {
                let (workspace, tab) = herdr.create_workspace(&name, cwd)?;
                Ok(Container {
                    id: Some(workspace),
                    spare_tab: Some(tab),
                })
            }
            Self::Cmux(cmux) => Ok(Container::existing(cmux.create_group(&name, cwd)?)),
        }
    }

    /// Add one Task tab (or Wave conversation tab) with its splits.
    fn add_tab(
        &self,
        container: &mut Container,
        name: &str,
        cwd: &Path,
        layout: Value,
    ) -> Result<String> {
        let name = label(name);
        match self {
            Self::Rex(rex) => {
                let session = container
                    .id
                    .as_deref()
                    .context("Rex session is unavailable")?;
                rex.create_window(session, &name, layout)
            }
            Self::Herdr(herdr) => {
                let tab = match container.spare_tab.take() {
                    Some(tab) => tab,
                    None => herdr.create_tab(
                        container
                            .id
                            .as_deref()
                            .context("Herdr workspace is unavailable")?,
                        &name,
                        cwd,
                    )?,
                };
                herdr.apply(&tab, &name, layout)
            }
            Self::Cmux(cmux) => cmux.create_workspace(container.id.as_deref(), &name, cwd, &layout),
        }
    }

    /// Add one Task's tab with its splits; returns the tab and its new shell count.
    fn add_task_tab(
        &self,
        templates: &mut Templates<'_>,
        container: &mut Container,
        content: &ViewContent,
        presentation: Presentation<'_>,
    ) -> Result<(String, usize)> {
        let template = templates.for_task(&TaskId::parse(&content.work.id)?)?;
        let repo = templates.repo;
        let (native_layout, shells) = layout(
            templates.store,
            content,
            repo,
            template.as_ref(),
            presentation,
            self,
        )?;
        let tab = self.add_tab(
            container,
            &content.title,
            tool_cwd(content, repo),
            native_layout,
        )?;
        Ok((tab, shells))
    }

    fn focus(&self, container: &Container, tab: &str) -> String {
        let result = match self {
            Self::Rex(rex) => rex.focus(
                container.id.as_deref().expect("Rex tabs have a session"),
                tab,
            ),
            Self::Herdr(herdr) => herdr.focus(tab).map(|()| "Selected in Herdr".into()),
            Self::Cmux(cmux) => cmux.focus(tab).map(|()| "Selected in cmux".into()),
        };
        result.unwrap_or_else(|error| format!("Created; display selection failed: {error:#}"))
    }
}

// ---------------------------------------------------------------------------
// Opening
// ---------------------------------------------------------------------------

/// Which conversations a pane shows; Task & workflow takes the place of the rest.
/// Resuming anything but a detached Session starts a provider or takes its input.
#[derive(Clone, Copy, Default)]
enum Resume {
    /// Background arrivals do not take input from an existing terminal.
    #[default]
    None,
    /// A repository or Wave opening reconnects what no terminal shows and
    /// starts nothing, as `session_actions` offers Reconnect only then.
    Detached,
    /// A selected Task or Session.
    Any,
}

impl Resume {
    /// Whether a pane runs `session`, or starts a conversation when there is
    /// none. The conversation running this command stays where it is.
    fn admits(self, store: &SqliteStore, session: Option<&str>) -> Result<bool> {
        let is_caller = |session: &str| {
            crate::journal::agent_caller().is_some_and(|caller| caller.session_id == session)
        };
        Ok(match (self, session) {
            (_, Some(session)) if is_caller(session) => false,
            (Self::None, _) | (Self::Detached, None) => false,
            (Self::Detached, Some(session)) => viewed_summary(store, session)?
                .is_some_and(|summary| session_state(&summary) == SessionState::Detached),
            (Self::Any, _) => true,
        })
    }
}

#[derive(Clone, Copy, Default)]
struct Presentation<'a> {
    session: Option<&'a str>,
    diff: bool,
    resume: Resume,
}

pub(super) fn run(cli: &crate::lf::Cli, args: &OpenArgs, host: OpenHost) -> Result<()> {
    if let Some(session) = &args.follow {
        return follow(cli, session, args);
    }
    let (targets, unknown) = super::targets(cli, args)?;
    crate::store::read_existing_registry()?.context("Work registry is unavailable")?;
    let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
    let mut report = OpenReport {
        receipts: Vec::new(),
        unknown,
    };
    for target in &targets {
        // The same opening lock protects ordinary and workspace opening.
        let _opening = Opening::lock(target)?;
        match open_machines(&store, host, target, args) {
            Ok(opened) => {
                report.receipts.extend(opened.receipts);
                report.unknown.extend(opened.unknown);
            }
            Err(error) => report
                .receipts
                .push(failed(target, host, format!("{error:#}"))),
        }
    }
    if args.json || cli.json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        for receipt in &report.receipts {
            println!(
                "{}: {}{}",
                receipt.label,
                receipt.reason.as_deref().unwrap_or("opened"),
                receipt
                    .destination
                    .as_ref()
                    .map(|id| format!(" ({id})"))
                    .unwrap_or_default()
            );
        }
        super::print_unknown(&report.unknown);
    }
    // The inter-Machine reader transports per-item failure receipts as data.
    if args.local_workspace {
        return Ok(());
    }
    anyhow::ensure!(
        !report
            .receipts
            .iter()
            .any(|r| r.status == OpenStatus::Failed),
        "Some workspace items could not be opened; successful native work was retained"
    );
    Ok(())
}

fn failed(target: &Resolved, host: OpenHost, reason: String) -> OpenReceipt {
    target.receipt(Some(host), None, OpenStatus::Failed, Some(reason))
}

/// Opening a Task starts it, so one without a checkout is placed before its
/// tools launch. Broad openings never create worktrees, a finished or abandoned
/// Task's checkout is not rebuilt, and a remote Task is placed by its own Machine.
fn place_task(
    store: &SqliteStore,
    target: &Resolved,
    plan: &ViewPlan,
    local: &MachineId,
) -> Result<bool> {
    let Some(task) = &target.task else {
        return Ok(false);
    };
    let unplaced = plan.contents.iter().any(|content| {
        content.location.is_none() && !content.states.contains(&SelectionState::Done)
    });
    if !unplaced || store.task_execution_route(&task.id)?.machine_id != *local {
        return Ok(false);
    }
    crate::ops::task::task_checkout(
        &target.repository_path,
        task.id.as_str(),
        Default::default(),
    )?;
    Ok(true)
}

fn open_machines(
    store: &SqliteStore,
    host: OpenHost,
    target: &Resolved,
    args: &OpenArgs,
) -> Result<OpenReport> {
    if target.task.is_none() && (args.available || target.session.is_some()) {
        return Ok(OpenReport {
            receipts: vec![open_conversation(store, host, target, args)?],
            unknown: Vec::new(),
        });
    }
    let repository = target
        .repository_id
        .as_deref()
        .context("Repository has no Work identity")?;
    let repository = RepositoryId::parse(repository)?;
    let selected = scope(store, target)?;
    let local = store.local_machine()?.id;
    let mut plan = workspace_plan(store, &repository, selected.clone())?;
    if place_task(store, target, &plan, &local)? {
        plan = workspace_plan(store, &repository, selected.clone())?;
    }
    let explicit = target.task.is_some();
    let routed = route(
        store,
        plan.contents.iter().filter(|content| {
            args.arrive_task.is_empty() || args.arrive_task.contains(&content.work.id)
        }),
        &local,
        explicit,
    )?;
    let mut receipts = Vec::new();
    let mut unknown = plan.resolution.unavailable.clone();
    unknown.extend(routed.unknown);
    // A broad Rex launch is the one follower for every Machine it reaches.
    let follows = host == OpenHost::Rex && !explicit && !args.local_workspace;
    let mut remotes = Remotes::default();
    if !args.local_workspace {
        for (peer, candidates) in routed.peers {
            let mut remote_args = args.clone();
            remote_args.session = target.session.clone();
            remote_args.diff |= target.url.diff;
            let outcome = remote_open(
                store,
                host,
                peer.as_str(),
                repository.as_str(),
                &selected,
                &remote_args,
            );
            let (placement, opened) = settle_remote(&outcome, None, &candidates);
            remotes.seen.extend(opened);
            if let Some(entry) = placement.argument(&peer) {
                remotes.containers.push(entry);
            }
            match outcome {
                Ok(mut report) => {
                    receipts.append(&mut report.receipts);
                    unknown.append(&mut report.unknown);
                }
                Err(error) => receipts.push(failed(
                    target,
                    host,
                    format!("Machine {peer}: {error:#}; no local fallback"),
                )),
            }
        }
    }
    if !routed.local.is_empty() || follows {
        let result = Native::resolve(host, args.server.as_deref()).and_then(|native| {
            launch(
                store,
                &native,
                target,
                &selected,
                args,
                &routed.local,
                follows.then_some(&remotes),
            )
        });
        match result {
            Ok(mut opened) => receipts.append(&mut opened),
            Err(error) => receipts.push(failed(target, host, format!("{error:#}"))),
        }
    } else if receipts.is_empty() {
        receipts.push(target.receipt(
            Some(host),
            None,
            OpenStatus::Opening,
            Some("No started, unfinished Tasks to open here; nothing was created".into()),
        ));
    }
    Ok(OpenReport { receipts, unknown })
}

/// A selection's Tasks by the Machine that runs them. Another Machine decides for
/// itself whether its Task started; an unknown route never becomes a local tab.
#[derive(Default)]
struct Routed<'a> {
    local: Vec<&'a ViewContent>,
    peers: BTreeMap<String, Vec<Candidate>>,
    unknown: Vec<String>,
}

fn route<'a>(
    store: &SqliteStore,
    contents: impl Iterator<Item = &'a ViewContent>,
    local: &MachineId,
    explicit: bool,
) -> Result<Routed<'a>> {
    let mut routed = Routed::default();
    for content in contents
        .filter(|content| content.work.kind == WorkWorkspaceKind::Task)
        .filter(|content| explicit || !content.states.contains(&SelectionState::Done))
    {
        let task = TaskId::parse(&content.work.id)?;
        match store.task_execution_route(&task) {
            Ok(route) if route.machine_id == *local => {
                if explicit || started(content) {
                    routed.local.push(content);
                }
            }
            Ok(route) => routed
                .peers
                .entry(route.machine_id.to_string())
                .or_default()
                .push(Candidate::read(store, &task)?),
            Err(error) => routed.unknown.push(format!(
                "{}: execution Machine unavailable: {error}",
                content.title
            )),
        }
    }
    Ok(routed)
}

/// A Task another Machine may open, with the identifier its receipts name.
#[derive(Debug, Clone)]
struct Candidate {
    id: String,
    identifier: String,
}

impl Candidate {
    fn read(store: &SqliteStore, task: &TaskId) -> Result<Self> {
        Ok(Self {
            id: task.to_string(),
            identifier: store.task(task)?.context("Task is unavailable")?.identifier,
        })
    }
}

/// What a broad launch hands its follower about other Machines, through argv.
#[derive(Debug, Default)]
struct Remotes {
    /// Tasks another Machine opened, or may have opened.
    seen: Vec<String>,
    /// `--remote` values: `<machine>=<container>`, empty when creation is uncertain.
    containers: Vec<String>,
}

/// One Machine's native container, held only for a follower's lifetime.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Placement {
    /// Nothing was opened there; the first arrival creates its container.
    Unopened,
    Container(String),
    /// A create may have succeeded without a reply; nothing more is created there.
    Uncertain,
}

impl Placement {
    fn argument(&self, machine: &str) -> Option<String> {
        match self {
            Self::Unopened => None,
            Self::Container(container) => Some(format!("{machine}={container}")),
            Self::Uncertain => Some(format!("{machine}=")),
        }
    }

    fn parse(values: &[String]) -> Result<HashMap<String, Placement>> {
        values
            .iter()
            .map(|value| {
                let (machine, container) = value
                    .split_once('=')
                    .context("--remote takes <machine>=<container>")?;
                Ok((
                    machine.to_owned(),
                    if container.is_empty() {
                        Self::Uncertain
                    } else {
                        Self::Container(container.to_owned())
                    },
                ))
            })
            .collect()
    }
}

/// Read one remote launch: the container it used and the candidates it opened or
/// may have opened. A launch-wide failure is uncertain for every candidate.
fn settle_remote(
    outcome: &Result<OpenReport>,
    known: Option<&str>,
    candidates: &[Candidate],
) -> (Placement, Vec<String>) {
    let everything = || candidates.iter().map(|task| task.id.clone()).collect();
    let known = known.map(|container| Placement::Container(container.into()));
    let Ok(report) = outcome else {
        return (known.unwrap_or(Placement::Uncertain), everything());
    };
    let task_of = |receipt: &OpenReceipt| match crate::work::link::WorkUrl::parse(&receipt.url)
        .map(|url| url.target)
    {
        Ok(crate::work::link::WorkTarget::Task(identifier)) => candidates
            .iter()
            .find(|task| task.identifier == identifier)
            .map(|task| task.id.clone()),
        _ => None,
    };
    let mut opened = Vec::new();
    let mut failed = false;
    let mut launch_failed = false;
    for receipt in &report.receipts {
        let task = task_of(receipt);
        failed |= receipt.status == OpenStatus::Failed;
        launch_failed |= receipt.status == OpenStatus::Failed && task.is_none();
        opened.extend(task);
    }
    let container = report
        .receipts
        .iter()
        .find_map(|receipt| receipt.container.clone())
        .map(Placement::Container)
        .or(known);
    let placement = match container {
        Some(container) => container,
        None if failed => Placement::Uncertain,
        None => Placement::Unopened,
    };
    (placement, if launch_failed { everything() } else { opened })
}

fn remote_open(
    store: &SqliteStore,
    host: OpenHost,
    machine: &str,
    repository: &str,
    selection: &Selection,
    args: &OpenArgs,
) -> Result<OpenReport> {
    let machine = store
        .machines()?
        .into_iter()
        .find(|entry| entry.id.as_str() == machine)
        .context("Task execution Machine has no configured route")?;
    let mut argv = vec![
        "--repo-id".into(),
        repository.into(),
        "open".into(),
        "--host".into(),
        host.label().to_lowercase(),
        "--local-workspace".into(),
        "--json".into(),
    ];
    match &selection.expression {
        SelectionExpression::Work { work } => argv.extend(["--task".into(), work.id.clone()]),
        SelectionExpression::Wave { wave } => argv.extend(["--wave".into(), wave.to_string()]),
        SelectionExpression::Repository => {}
        _ => bail!("This workspace selection cannot be routed"),
    }
    if let Some(session) = &args.session {
        argv.extend(["--session".into(), session.clone()]);
    }
    if let Some(agent) = crate::config::agent_override() {
        argv.extend(["--agent".into(), agent]);
    }
    if args.diff {
        argv.push("--diff".into());
    }
    if args.no_focus {
        argv.push("--no-focus".into());
    }
    if !args.arrive_task.is_empty() {
        argv.extend(["--arrive-task".into(), args.arrive_task.join(",")]);
    }
    if let Some(into) = &args.into {
        argv.extend(["--into".into(), into.clone()]);
    }
    let bytes = tokio::runtime::Runtime::new()?
        .block_on(super::super::ssh::request(&machine, &argv, None))?;
    serde_json::from_slice(&bytes)
        .context("Remote workspace receipt is unavailable; creation may have succeeded")
}

fn scope(store: &SqliteStore, target: &Resolved) -> Result<Selection> {
    let expression = if let Some(task) = &target.task {
        SelectionExpression::Work {
            work: WorkWorkspaceKey {
                repository: task.repository_id.to_string(),
                kind: WorkWorkspaceKind::Task,
                id: task.id.to_string(),
            },
        }
    } else if let Some(wave) = &target.wave {
        SelectionExpression::Wave {
            wave: store
                .resolve_wave_id(wave, Some(&target.repository_path.to_string_lossy()))?
                .context("Wave is unavailable")?,
        }
    } else {
        SelectionExpression::Repository
    };
    Ok(Selection {
        id: SelectionId("workspace".into()),
        expression,
    })
}

/// Settings that apply to a Task in this Wave.
fn wave_config(
    store: &SqliteStore,
    repo: &Path,
    wave: Option<&WaveId>,
) -> Result<Option<crate::config::Config>> {
    let slug = wave
        .map(|wave| store.get_wave(wave))
        .transpose()?
        .flatten()
        .map(|wave| wave.slug().to_owned());
    Ok(crate::config::load_wave_config(
        Some(repo),
        slug.as_deref(),
    )?)
}

/// Configured `task_layout` per Wave, read once per launch.
struct Templates<'a> {
    store: &'a SqliteStore,
    repo: &'a Path,
    read: HashMap<Option<WaveId>, Option<TaskLayoutTemplate>>,
}

impl Templates<'_> {
    fn for_task(&mut self, task: &TaskId) -> Result<Option<TaskLayoutTemplate>> {
        let wave = self
            .store
            .task(task)?
            .context("Task is unavailable")?
            .wave_id;
        if let Some(template) = self.read.get(&wave) {
            return Ok(template.clone());
        }
        let template =
            wave_config(self.store, self.repo, wave.as_ref())?.and_then(|c| c.task_layout);
        self.read.insert(wave, template.clone());
        Ok(template)
    }
}

/// The container's name: the selected Wave, else the Task's Wave, else the repository.
fn container_name(store: &SqliteStore, target: &Resolved, selected: &Selection) -> Result<String> {
    let wave = match &selected.expression {
        SelectionExpression::Wave { wave } => Some(wave.clone()),
        SelectionExpression::Work { work } => store
            .task(&TaskId::parse(&work.id)?)?
            .and_then(|task| task.wave_id),
        _ => None,
    };
    Ok(
        match wave
            .map(|wave| store.get_wave(&wave))
            .transpose()?
            .flatten()
        {
            Some(wave) => wave.slug().to_owned(),
            None => target.repository_path.file_name().map_or_else(
                || target.label.clone(),
                |name| name.to_string_lossy().into_owned(),
            ),
        },
    )
}

/// A Session with no Task is one conversation, not a selection of Tasks: one
/// tab in the caller's container, and nothing arrives after it.
fn open_conversation(
    store: &SqliteStore,
    host: OpenHost,
    target: &Resolved,
    args: &OpenArgs,
) -> Result<OpenReceipt> {
    if let Some(reason) = target.current_conversation() {
        return Ok(target.receipt(Some(host), None, OpenStatus::Opening, Some(reason)));
    }
    let session = target
        .conversation()
        .context("This Work has no conversation on this Machine")?;
    let native = Native::resolve(host, args.server.as_deref())?;
    let cwd = target.cwd();
    let mut container = match native.here()? {
        Some(here) => here,
        None => {
            let name = container_name(store, target, &scope(store, target)?)?;
            native.create_container(&name, cwd)?
        }
    };
    let mut receipt = conversation_tab(&native, &mut container, target, session, cwd);
    if let (false, Some(tab)) = (args.no_focus, &receipt.destination) {
        receipt.reason = Some(native.focus(&container, tab));
    }
    receipt.container = container.id;
    Ok(receipt)
}

fn conversation_tab(
    native: &Native,
    container: &mut Container,
    target: &Resolved,
    session: &str,
    cwd: &Path,
) -> OpenReceipt {
    let mut receipt = target.receipt(Some(native.open_host()), None, OpenStatus::Opening, None);
    let opened = native
        .conversation(cwd, &[session])
        .and_then(|layout| native.add_tab(container, "Conversation", cwd, layout));
    settle_receipt(&mut receipt, opened);
    receipt
}

fn launch(
    store: &SqliteStore,
    native: &Native,
    target: &Resolved,
    selected: &Selection,
    args: &OpenArgs,
    local_tasks: &[&ViewContent],
    follow: Option<&Remotes>,
) -> Result<Vec<OpenReceipt>> {
    let host = native.open_host();
    let repo = target.repository_path.as_path();
    let explicit = target.task.is_some();
    // An arrival appends to its follower's container. A Task joins the container
    // the caller is in; a repository or Wave gets its own.
    let mut container = if let Some(into) = &args.into {
        Container::existing(into)
    } else {
        match native.here()?.filter(|_| explicit) {
            Some(here) => here,
            None => native.create_container(&container_name(store, target, selected)?, repo)?,
        }
    };
    let presentation = if args.arrive_task.is_empty() {
        Presentation {
            session: target.session.as_deref(),
            diff: args.diff || target.url.diff,
            resume: if explicit {
                Resume::Any
            } else {
                Resume::Detached
            },
        }
    } else {
        Presentation::default()
    };
    let mut receipts = Vec::new();
    if let SelectionExpression::Wave { wave } = &selected.expression {
        match store.primary_session(&crate::session::PrimaryScope::Wave(wave.clone()))? {
            Some(session)
                if presentation
                    .resume
                    .admits(store, Some(session.id.as_str()))? =>
            {
                let mut receipt =
                    conversation_tab(native, &mut container, target, session.id.as_str(), repo);
                receipt.label = "Wave conversation".into();
                receipts.push(receipt);
            }
            _ => {}
        }
    }
    let mut templates = Templates {
        store,
        repo,
        read: HashMap::new(),
    };
    let mut focused = args.no_focus;
    let mut seen = Vec::new();
    for content in local_tasks {
        let task = store
            .task(&TaskId::parse(&content.work.id)?)?
            .context("Task is unavailable")?;
        seen.push(task.id.to_string());
        let mut receipt = target.receipt(Some(host), None, OpenStatus::Opening, None);
        receipt.label = format!("{} · {}", task.identifier, task.title);
        receipt.url = crate::work::link::WorkUrl::new(
            target.url.repository.clone(),
            crate::work::link::WorkTarget::Task(task.identifier.clone()),
        )
        .to_string();
        let opened = native
            .add_task_tab(&mut templates, &mut container, content, presentation)
            .map(|(tab, shells)| {
                let showing = if focused {
                    "Focus preserved".into()
                } else {
                    focused = true;
                    native.focus(&container, &tab)
                };
                receipt.reason = Some(format!(
                    "Created a new Task tab; {shells} new shells; {showing}"
                ));
                tab
            });
        settle_receipt(&mut receipt, opened);
        receipts.push(receipt);
    }
    if let (Native::Rex(rex), Some(remotes)) = (native, follow) {
        let session = container.id.clone().expect("Rex containers are sessions");
        seen.extend(remotes.seen.iter().cloned());
        let mut receipt =
            target.receipt(Some(host), Some(session.clone()), OpenStatus::Opening, None);
        receipt.label = "Following".into();
        let url = target.url.to_string();
        receipt.reason = Some(
            match start_follower(rex, &session, &url, repo, &seen, &remotes.containers) {
                Ok(()) => {
                    "Newly started Tasks arrive in this Rex session while its Workspace tab runs"
                        .into()
                }
                Err(error) => {
                    receipt.status = OpenStatus::Failed;
                    format!("Follower was not started: {error:#}")
                }
            },
        );
        receipts.push(receipt);
    }
    for receipt in &mut receipts {
        receipt.container = container.id.clone();
    }
    Ok(receipts)
}

/// Record a tab's native result. A lost reply is reported, never retried.
fn settle_receipt(receipt: &mut OpenReceipt, opened: Result<String>) {
    match opened {
        Ok(tab) => {
            receipt.destination = Some(tab);
            if receipt.reason.is_none() {
                receipt.reason = Some("Created".into());
            }
        }
        Err(error) => {
            receipt.status = OpenStatus::Failed;
            receipt.reason = Some(format!(
                "{error:#}; nothing was retried, so inspect the host before opening again"
            ));
        }
    }
}

fn started(content: &ViewContent) -> bool {
    content.states.iter().any(|state| {
        matches!(
            state,
            SelectionState::Idle
                | SelectionState::Active
                | SelectionState::Moving
                | SelectionState::Shipping
        )
    })
}

// ---------------------------------------------------------------------------
// Following
// ---------------------------------------------------------------------------

/// The follower's argv carries everything it needs: server, session, the Work
/// address, the Tasks this launch already opened and other Machines' containers.
/// Nothing is written for it.
fn start_follower(
    rex: &Rex,
    session: &str,
    url: &str,
    repo: &Path,
    seen: &[String],
    remotes: &[String],
) -> Result<()> {
    let mut words = vec![
        "open".to_owned(),
        url.into(),
        "--host".into(),
        "rex".into(),
        "--server".into(),
        rex.server.clone(),
        "--follow".into(),
        session.into(),
    ];
    if !seen.is_empty() {
        words.extend(["--seen".into(), seen.join(",")]);
    }
    for remote in remotes {
        words.extend(["--remote".into(), remote.clone()]);
    }
    let words: Vec<&str> = words.iter().map(String::as_str).collect();
    rex.create_window(
        session,
        "Workspace",
        rex.block("Workspace", repo, lf_argv(repo, &words)?),
    )?;
    Ok(())
}

fn lf_argv(cwd: &Path, words: &[&str]) -> Result<Vec<String>> {
    // Tools belong to this opening. Keep an explicit private-Machine runtime, but
    // never hand a dev build's real-Machine panes to the installed release.
    let binary = if crate::store::custom_home_selected() {
        crate::os_process::resolve_lf_binary()?
    } else {
        std::env::current_exe()?
    };
    let mut argv = vec![
        binary.to_string_lossy().into_owned(),
        "--cwd".into(),
        cwd.to_string_lossy().into_owned(),
    ];
    argv.extend(words.iter().map(|word| (*word).into()));
    Ok(with_machine_home(argv))
}

fn with_machine_home(mut argv: Vec<String>) -> Vec<String> {
    if crate::store::custom_home_selected() {
        argv.splice(
            0..0,
            [
                "env".into(),
                format!("LF_HOME={}", crate::store::lf_home_dir().display()),
            ],
        );
    }
    argv
}

/// Add newly started Tasks while this tab runs: local ones to this Rex session,
/// another Machine's to the container held for it here. A Machine with none gets
/// one on its first arrival. Seen Tasks, including tabs someone closed, stay in
/// memory and never reappear. Containers live only as long as this process.
fn follow(cli: &crate::lf::Cli, session: &str, args: &OpenArgs) -> Result<()> {
    let server = args
        .server
        .as_deref()
        .context("A follower needs its Rex server")?;
    let rex = Rex {
        server: server.into(),
        caller: None,
    };
    let (targets, _) = super::targets(cli, args)?;
    let target = targets
        .first()
        .context("Follower selection is unavailable")?;
    let store = SqliteStore::new(&crate::store::database_path_from_env()?)?;
    let selected = scope(&store, target)?;
    let repository = RepositoryId::parse(
        target
            .repository_id
            .as_deref()
            .context("Repository has no Work identity")?,
    )?;
    let mut remotes = Placement::parse(&args.remote)?;
    println!("Loopflow workspace\nNewly started Tasks appear without taking focus. Close this tab to stop following.");
    let subscribed = repository.clone();
    let subscribing = super::super::subscribe::start(
        selected.clone(),
        move |store, selected| {
            let repository = subscribed.clone();
            async move {
                let store = super::super::subscribe::require_store(store)?;
                workspace_plan(&store.sqlite, &repository, selected)
            }
        },
        super::super::subscribe::Transport::Pull {
            events: false,
            wake: Box::new(|| {}),
            select: None,
        },
    )?
    .context("Workspace subscription did not start")?;
    let mut seen: BTreeSet<String> = args.seen.iter().cloned().collect();
    let mut templates = Templates {
        store: &store,
        repo: &target.repository_path,
        read: HashMap::new(),
    };
    let native = Native::Rex(rex.clone());
    let mut container = Container::existing(session);
    let local = store.local_machine()?.id;
    let mut refresh = std::time::Instant::now();
    let mut request_id = 0_u64;
    loop {
        if !rex
            .sessions()?
            .iter()
            .any(|item| item["session_id"] == session)
        {
            return Ok(());
        }
        if let Some(bytes) = subscribing.pull().frame {
            let frame: Value = serde_json::from_slice(&bytes)?;
            if let Some(error) = frame["unavailable"].as_str() {
                eprintln!("Work selection unavailable: {error}");
                continue;
            }
            let plan: ViewPlan = serde_json::from_value(frame["body"].clone())?;
            let arriving = route(
                &store,
                plan.contents
                    .iter()
                    .filter(|content| !seen.contains(&content.work.id)),
                &local,
                false,
            )?;
            for content in arriving.local {
                // Mark before creating: an uncertain create is never retried.
                seen.insert(content.work.id.clone());
                match native.add_task_tab(
                    &mut templates,
                    &mut container,
                    content,
                    Presentation::default(),
                ) {
                    Ok(_) => println!("Added {}", content.title),
                    Err(error) => eprintln!("{}: {error:#}; not retried", content.title),
                }
            }
            for (machine, candidates) in arriving.peers {
                let placement = remotes
                    .entry(machine.clone())
                    .or_insert(Placement::Unopened);
                seen.extend(arrive_remote(
                    &store,
                    &machine,
                    placement,
                    &candidates,
                    repository.as_str(),
                    &selected,
                    args,
                ));
            }
        }
        // Remote starts and recovered connections need not change this Machine's
        // Work snapshot. Re-read periodically.
        if refresh.elapsed() >= std::time::Duration::from_secs(15) {
            request_id += 1;
            subscribing.checkpoint(request_id);
            refresh = std::time::Instant::now();
        }
        let mut fd = libc::pollfd {
            fd: 0,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one initialized poll descriptor, bounded wait, no retained pointer.
        if unsafe { libc::poll(&mut fd, 1, 1000) } > 0 && fd.revents != 0 {
            let mut byte = [0];
            if std::io::stdin().read(&mut byte)? == 0 || matches!(byte[0], b'q' | 3) {
                return Ok(());
            }
        }
        if !std::io::stdin().is_terminal() {
            return Ok(());
        }
    }
}

/// Ask `machine` to open its newly started Tasks: into the container held for it
/// here, or a new one on its first arrival. Nothing more is asked of a Machine
/// whose creation is uncertain. Returns the Tasks that must never arrive again.
fn arrive_remote(
    store: &SqliteStore,
    machine: &str,
    placement: &mut Placement,
    candidates: &[Candidate],
    repository: &str,
    selected: &Selection,
    args: &OpenArgs,
) -> Vec<String> {
    let known = match placement {
        Placement::Uncertain => return Vec::new(),
        Placement::Unopened => None,
        Placement::Container(container) => Some(container.clone()),
    };
    // Arrivals select no Session and take no focus.
    let arrival = OpenArgs {
        arrive_task: candidates.iter().map(|task| task.id.clone()).collect(),
        into: known.clone(),
        no_focus: true,
        session: None,
        diff: false,
        ..args.clone()
    };
    let outcome = remote_open(
        store,
        OpenHost::Rex,
        machine,
        repository,
        selected,
        &arrival,
    );
    let (next, opened) = settle_remote(&outcome, known.as_deref(), candidates);
    match &outcome {
        Ok(report) => {
            for receipt in &report.receipts {
                match (receipt.status, &receipt.destination) {
                    (OpenStatus::Failed, _) => eprintln!(
                        "{} on {machine}: {}",
                        receipt.label,
                        receipt.reason.as_deref().unwrap_or("failed")
                    ),
                    (_, Some(_)) => println!("Added {} on {machine}", receipt.label),
                    _ => {}
                }
            }
        }
        Err(error) => eprintln!(
            "Machine {machine}: {error:#}; arrivals there were not retried and no local fallback was used"
        ),
    }
    if next == Placement::Uncertain {
        eprintln!("Machine {machine}: its workspace may exist without a reply; later arrivals there wait for another lf open");
    }
    *placement = next;
    opened
}

#[cfg(test)]
mod tests {
    use super::{settle_remote, started, Candidate, Placement};
    use crate::lf::commands::open::{OpenHost, OpenReceipt, OpenReport, OpenStatus};
    use crate::work::selection::{
        SelectionState, ViewContent, WorkWorkspaceKey, WorkWorkspaceKind,
    };

    #[test]
    fn real_machine_tools_keep_the_build_that_opened_them() {
        let _lock = loopflow_test_support::env_lock().lock().unwrap();
        let _env = crate::test_ambient::EnvGuard::clear(&["LF_HOME", "LF_BIN"]);
        std::env::set_var("LF_BIN", "/old/installed/lf");
        let cwd = std::env::current_dir().unwrap();
        let argv = super::lf_argv(&cwd, &["resume", "existing"]).unwrap();
        assert_eq!(argv[0], std::env::current_exe().unwrap().to_string_lossy());
        assert_eq!(&argv[3..], &["resume", "existing"]);
    }

    #[test]
    fn only_started_unfinished_tasks_arrive_automatically() {
        let mut content = ViewContent {
            pane_revision: None,
            face: None,
            work: WorkWorkspaceKey {
                repository: "r".into(),
                kind: WorkWorkspaceKind::Task,
                id: "t".into(),
            },
            title: "Task".into(),
            states: Default::default(),
            primary_session: None,
            location: None,
            argv: None,
            unavailable: vec![],
        };
        for (state, expected) in [
            (SelectionState::Unstarted, false),
            (SelectionState::Idle, true),
            (SelectionState::Moving, true),
            (SelectionState::Done, false),
        ] {
            content.states = std::collections::BTreeSet::from([state]);
            assert_eq!(started(&content), expected);
        }
    }

    fn remote(receipts: Vec<(&str, Option<&str>, OpenStatus)>) -> anyhow::Result<OpenReport> {
        Ok(OpenReport {
            receipts: receipts
                .into_iter()
                .map(|(url, container, status)| OpenReceipt {
                    label: url.into(),
                    url: url.into(),
                    host: Some(OpenHost::Rex),
                    destination: None,
                    container: container.map(str::to_owned),
                    status,
                    reason: None,
                    fallback_argv: Vec::new(),
                })
                .collect(),
            unknown: Vec::new(),
        })
    }

    #[test]
    fn remote_receipts_settle_the_container_and_what_never_returns() {
        let tasks = [
            Candidate {
                id: "t1".into(),
                identifier: "A-1".into(),
            },
            Candidate {
                id: "t2".into(),
                identifier: "A-2".into(),
            },
        ];
        // Created one Task; the other was not started there and stays a candidate.
        let created = remote(vec![(
            "loopflow://r/task/A-1",
            Some("s"),
            OpenStatus::Opening,
        )]);
        assert_eq!(
            settle_remote(&created, None, &tasks),
            (Placement::Container("s".into()), vec!["t1".into()])
        );
        // Nothing started there: no container, nothing seen.
        let declined = remote(vec![("loopflow://r", None, OpenStatus::Opening)]);
        assert_eq!(
            settle_remote(&declined, None, &tasks),
            (Placement::Unopened, vec![])
        );
        // A launch-wide failure or lost reply may have created anything.
        let failed = remote(vec![("loopflow://r", None, OpenStatus::Failed)]);
        for outcome in [failed, Err(anyhow::anyhow!("timed out"))] {
            assert_eq!(
                settle_remote(&outcome, None, &tasks),
                (Placement::Uncertain, vec!["t1".into(), "t2".into()])
            );
        }
        // A held container survives a lost reply.
        assert_eq!(
            settle_remote(&Err(anyhow::anyhow!("lost")), Some("s"), &tasks).0,
            Placement::Container("s".into())
        );
        let parsed = Placement::parse(&["m1=s".into(), "m2=".into()]).unwrap();
        assert_eq!(parsed["m1"], Placement::Container("s".into()));
        assert_eq!(parsed["m2"], Placement::Uncertain);
    }
}
