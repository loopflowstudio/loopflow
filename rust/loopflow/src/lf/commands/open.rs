//! Conversation selection and host presentation. `open` reaches one Session;
//! `workspace` applies layouts and follows Task arrivals. Both use the shared
//! non-replacing Session connection; explicit resume owns input takeover.

mod cmux;
mod conversation;
mod files;
mod herdr;
mod rex;
mod tmux;
mod workspace;

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};
use sha2::Digest as _;

use crate::ops::context::{ContextExplanation, ContextFact};
use crate::session::PrimaryScope;
use crate::store::Store;
use crate::work::link::{WorkTarget, WorkUrl};
use crate::work::selection::WorkWorkspaceKind;

pub(crate) const DESKTOP_BUNDLE: &str = "com.loopflow.mac";
const HOST_TIMEOUT: Duration = Duration::from_secs(2);
pub(crate) const NO_DESKTOP: &str = "no installed Loopflow Desktop; `lf install` places it";
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum OpenHost {
    Auto,
    Desktop,
    Cmux,
    Herdr,
    Tmux,
    Rex,
    Terminal,
}

impl OpenHost {
    fn label(self) -> &'static str {
        match self {
            Self::Auto => "the automatic route",
            Self::Desktop => "Loopflow Desktop",
            Self::Cmux => "cmux",
            Self::Herdr => "Herdr",
            Self::Tmux => "tmux",
            Self::Rex => "Rex",
            Self::Terminal => "a terminal",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LinkFormat {
    Text,
    Markdown,
}

/// Whether the caller is already in the conversation a link opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ConversationRelation {
    CurrentConversation,
    OtherConversation,
    Unknown,
}

/// A readable, activatable Work link. Printing one opens nothing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkLink {
    pub label: String,
    pub url: String,
    pub markdown: String,
    pub fallback_argv: Vec<String>,
    pub target: WorkLinkTarget,
    pub conversation: ConversationRelation,
    pub conversation_reason: Option<String>,
    pub route: OpenRoute,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct WorkLinkTarget {
    pub kind: WorkWorkspaceKind,
    #[serde(rename = "repo_path")]
    pub repository_path: String,
    #[serde(rename = "repo_id")]
    pub repository_id: Option<String>,
    /// The Task's readable identifier, such as LOO-507.
    pub task: Option<String>,
    pub task_id: Option<String>,
    pub wave: Option<String>,
    /// An explicit Session selection; ordinary Work links leave it absent.
    pub session: Option<String>,
    pub diff: bool,
    /// The execution owner when it is another Machine.
    pub execution_machine: Option<String>,
}

/// The selected host, or no host when focusing a recorded view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OpenRoute {
    pub hosts: Vec<OpenHost>,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OpenStatus {
    /// The destination accepted the request; input readiness is not observed.
    Opening,
    /// The conversation attached, or its existing view was focused.
    Usable,
    /// Another client holds the conversation; nothing new was opened.
    Elsewhere,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OpenReceipt {
    pub label: String,
    pub url: String,
    pub host: Option<OpenHost>,
    /// The host's own identifier for the destination it created.
    pub destination: Option<String>,
    /// The native container (Rex session, Herdr workspace, cmux group) holding it.
    pub container: Option<String>,
    pub status: OpenStatus,
    pub reason: Option<String>,
    pub fallback_argv: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, schemars::JsonSchema)]
pub struct OpenReport {
    pub receipts: Vec<OpenReceipt>,
    /// Inventory that could not be read; an empty report is complete only without these.
    pub unknown: Vec<String>,
}

// ---------------------------------------------------------------------------
// Resolution
// ---------------------------------------------------------------------------

/// One Work conversation, resolved through the shared owners.
#[derive(Debug, Clone)]
struct Resolved {
    url: WorkUrl,
    label: String,
    repository_path: PathBuf,
    repository_id: Option<String>,
    task: Option<crate::work::task::Task>,
    wave: Option<String>,
    /// An exact Session: explicit, or an available secondary conversation.
    session: Option<String>,
    /// The cwd an exact Session reconnects from.
    session_cwd: Option<PathBuf>,
    agent: Option<String>,
    /// The Work's current primary conversation, read without admitting one.
    primary: Option<String>,
    /// The other Machine of yours whose store holds this Work's primary; the
    /// routed connect reads and reuses it there.
    primary_machine: Option<String>,
    execution_machine: Option<String>,
    /// Why the Task's execution owner could not be read; opening refuses.
    execution_unavailable: Option<String>,
}

impl Resolved {
    fn conversation(&self) -> Option<&str> {
        self.session.as_deref().or(self.primary.as_deref())
    }

    /// What a host names the destination it creates: a Task's workspace name,
    /// otherwise the label. Destinations are found by their IDs, never this name.
    fn destination_name(&self) -> String {
        match &self.task {
            Some(task) => crate::terminal_title::workspace_name(&task.identifier, &task.title),
            None => self.label.clone(),
        }
    }

    fn resume_argv(&self, lf: &str) -> Vec<String> {
        let mut argv = vec![
            lf.to_string(),
            "session".into(),
            "resume".into(),
            "--start-if-free".into(),
        ];
        match (&self.session, &self.url.target) {
            (Some(session), _) => argv.push(session.clone()),
            (None, WorkTarget::Task(id)) => argv.extend(["--task".into(), id.clone()]),
            (None, WorkTarget::Wave(name)) => argv.extend(["--wave".into(), name.clone()]),
            (None, WorkTarget::Repository) => {}
        }
        if let Some(agent) = &self.agent {
            argv.extend(["--agent".into(), agent.clone()]);
        }
        argv
    }

    fn cwd(&self) -> &Path {
        self.session_cwd
            .as_deref()
            .filter(|cwd| cwd.is_dir())
            .unwrap_or(&self.repository_path)
    }

    fn fallback_argv(&self) -> Vec<String> {
        if self.session.is_some() && !matches!(self.url.target, WorkTarget::Task(_)) {
            return vec![
                "lf".into(),
                "open".into(),
                "--session".into(),
                self.session.clone().expect("exact Session"),
            ];
        }
        vec!["lf".into(), "open".into(), self.url.to_string()]
    }

    fn receipt(
        &self,
        host: Option<OpenHost>,
        destination: Option<String>,
        status: OpenStatus,
        reason: Option<String>,
    ) -> OpenReceipt {
        OpenReceipt {
            label: self.label.clone(),
            url: self.url.to_string(),
            host,
            destination,
            container: None,
            status,
            reason,
            fallback_argv: self.fallback_argv(),
        }
    }

    fn link(&self, route: OpenRoute) -> WorkLink {
        let (conversation, conversation_reason) = self.relation();
        WorkLink {
            label: self.label.clone(),
            url: self.url.to_string(),
            markdown: crate::work::link::markdown(&self.label, &self.url),
            fallback_argv: self.fallback_argv(),
            target: WorkLinkTarget {
                kind: match self.url.target {
                    WorkTarget::Repository => WorkWorkspaceKind::Repository,
                    WorkTarget::Wave(_) => WorkWorkspaceKind::Wave,
                    WorkTarget::Task(_) => WorkWorkspaceKind::Task,
                },
                repository_path: self.repository_path.display().to_string(),
                repository_id: self.repository_id.clone(),
                task: self.task.as_ref().map(|task| task.identifier.clone()),
                task_id: self.task.as_ref().map(|task| task.id.to_string()),
                wave: self.wave.clone(),
                session: self.session.clone(),
                diff: self.url.diff,
                execution_machine: self.execution_machine.clone(),
            },
            conversation,
            conversation_reason,
            route,
        }
    }

    /// Compare the calling conversation with this Work's selected destination.
    /// Same Work is not enough: a Graph step and the primary can share a Task.
    fn relation(&self) -> (ConversationRelation, Option<String>) {
        let Some(caller) = crate::journal::agent_caller() else {
            return (
                ConversationRelation::Unknown,
                Some("No calling conversation identity was supplied".into()),
            );
        };
        match self.conversation() {
            Some(id) if caller.session_id == id => {
                (ConversationRelation::CurrentConversation, None)
            }
            Some(_) => (ConversationRelation::OtherConversation, None),
            None => (
                ConversationRelation::OtherConversation,
                Some(match &self.primary_machine {
                    Some(machine) => format!(
                        "This Work's conversation runs on Machine {machine}; opening connects there"
                    ),
                    None => "This Work has no conversation yet; opening starts its primary".into(),
                }),
            ),
        }
    }
}

/// Root selectors as opening context, validated against an explicit --repo.
pub(crate) fn selected_context(
    cli: &crate::lf::Cli,
    session: Option<&str>,
) -> Result<ContextExplanation> {
    let mut resolution = work_context(
        &std::env::current_dir()?,
        cli.task.as_deref(),
        cli.wave.as_deref(),
        session,
    )?;
    if cli.selects_repository() {
        if let ContextFact::Bound { value, .. } = &resolution.repository_path {
            anyhow::ensure!(
                crate::repository::CanonicalRepo::current()?.as_ref()
                    == Some(&crate::repository::CanonicalRepo::discover(Path::new(
                        value
                    ))?),
                "Work does not belong to the selected repository. Nothing was opened."
            );
        }
        if cli.task.is_none() && cli.wave.is_none() && session.is_none() {
            resolution.task = ContextFact::Unbound;
            resolution.wave = ContextFact::Unbound;
        }
    }
    Ok(resolution)
}

/// Work, its execution location and an optional exact Session, from
/// `directory`. A plain Git repository opens before planning has ever been
/// initialized; only an absent registry allows that.
fn work_context(
    directory: &Path,
    task: Option<&str>,
    wave: Option<&str>,
    session: Option<&str>,
) -> Result<ContextExplanation> {
    if task.is_none()
        && wave.is_none()
        && session.is_none()
        && crate::store::read_existing_registry()?.is_none()
    {
        let repo = crate::repository::CanonicalRepo::containing(directory)?
            .context("select a Git repository with --repo before opening it")?;
        let mut resolution = ContextExplanation::empty();
        resolution.repository_path = ContextFact::bound(repo, "local_locator");
        return Ok(resolution);
    }
    // Without a Task, an exact Session selects the Work; a Wave must contain it.
    let mut resolution = match (task, session) {
        (None, Some(session)) => {
            let selected = super::context::explain_in(directory, None, None, Some(session), None)?;
            if let Some(wave) = wave {
                let work = super::context::explain_in(directory, Some(wave), None, None, None)?;
                anyhow::ensure!(
                    matches!((&work.wave, &selected.wave),
                    (ContextFact::Bound { value: wave, .. }, ContextFact::Bound { value: session_wave, .. }) if wave == session_wave),
                    "Session does not belong to the selected Work. Nothing was opened."
                );
            }
            selected
        }
        _ => super::context::explain_in(directory, wave, task, None, None)?,
    };
    if let ContextFact::Bound { value, .. } = &resolution.task {
        let task = crate::durable::TaskId::parse(value)?;
        if let Some(store) = crate::store::read_existing_registry()? {
            // The shared reader retains failures in the explanation; opening
            // refuses unavailable execution without losing the other Work facts.
            let _ = crate::ops::task_location::explain(&store.sqlite, &task, &mut resolution);
        }
    }
    if let Some(session) = session.filter(|_| task.is_some()) {
        let remote = matches!((&resolution.execution_machine, &resolution.machine),
            (ContextFact::Bound { value: owner, .. }, ContextFact::Bound { value: local, .. }) if owner != local);
        if remote {
            // Native opening validates membership on this owner. A Session ID
            // absent from the presentation Machine is not a missing conversation.
            resolution.session = ContextFact::Bound {
                value: session.into(),
                source: "explicit_session_on_execution_machine".into(),
            };
        } else {
            let selected = super::context::explain_in(directory, None, None, Some(session), None)?;
            anyhow::ensure!(
                matches!((&resolution.task, &selected.task),
                (ContextFact::Bound { value: task, .. }, ContextFact::Bound { value: session_task, .. }) if task == session_task),
                "Session does not belong to the selected Work. Nothing was opened."
            );
            resolution.session = selected.session;
            resolution.process = selected.process;
        }
    }
    Ok(resolution)
}

fn describe(
    store: Option<&Store>,
    context: &ContextExplanation,
    session: Option<String>,
    diff: bool,
    session_cwd: Option<PathBuf>,
) -> Result<Resolved> {
    if let ContextFact::Unavailable { reason } = &context.task {
        bail!("Task resolution unavailable: {reason}. Nothing was opened.");
    }
    let ContextFact::Bound { value: repo, .. } = &context.repository_path else {
        bail!("Repository location is unavailable; select a local repository with --repo. Nothing was opened.");
    };
    let repository_path = PathBuf::from(repo);
    let task = match &context.task {
        ContextFact::Bound { value, .. } => {
            let id = crate::durable::TaskId::parse(value)?;
            let store = store.context("Task records are unavailable on this Machine")?;
            Some(
                store
                    .sqlite
                    .task(&id)?
                    .context("Task is no longer recorded")?,
            )
        }
        _ => None,
    };
    let wave = match (&task, &context.wave) {
        (None, ContextFact::Bound { value, .. }) => {
            let id = crate::id::WaveId::parse(value)?;
            let store = store.context("Wave records are unavailable on this Machine")?;
            Some(
                store
                    .sqlite
                    .get_wave(&id)?
                    .context("Wave is no longer recorded")?
                    .slug()
                    .to_owned(),
            )
        }
        _ => None,
    };
    anyhow::ensure!(
        !diff || task.is_some(),
        "--diff needs a Task; select --task or a Task-associated --session. Nothing was opened."
    );
    let execution_machine = match (&task, &context.execution_machine, &context.machine) {
        (
            Some(_),
            ContextFact::Bound { value: owner, .. },
            ContextFact::Bound { value: local, .. },
            // The owner creates its checkout when the routed connect arrives.
        ) if owner != local => Some(owner.clone()),
        (Some(_), ContextFact::Unavailable { reason }, _) if session.is_some() || diff => {
            bail!("Task execution location unavailable: {reason}. Nothing was opened.");
        }
        _ => None,
    };
    // A link still prints; opening never falls back to this Machine.
    let execution_unavailable = match (&task, &context.execution_machine) {
        (Some(_), ContextFact::Unavailable { reason }) => Some(reason.clone()),
        _ => None,
    };
    let registered = registered(store)?;
    let canonical = repository_path
        .canonicalize()
        .unwrap_or_else(|_| repository_path.clone());
    let own = registered.iter().find(|repo| repo.canonical == canonical);
    let repository_id = match &context.repository {
        ContextFact::Bound { value, .. } => Some(value.clone()),
        _ => own
            .and_then(|repo| repo.id.as_ref())
            .map(|id| id.to_string()),
    };
    let short_name = folder_name(&repository_path)
        .context("Repository folder has no readable name")?
        .to_owned();
    let target = match (&task, &wave) {
        (Some(task), _) => WorkTarget::Task(task.identifier.clone()),
        (None, Some(wave)) => WorkTarget::Wave(wave.clone()),
        (None, None) => WorkTarget::Repository,
    };
    let mut url = WorkUrl::new(short_name.clone(), target);
    url.qualifier = own.and_then(|own| qualifier_for(&registered, own, &short_name));
    url.session = session.clone();
    url.diff = diff;
    let (primary, primary_machine) = match store {
        Some(store) => {
            let scope = match (&task, &context.wave) {
                (Some(task), _) => PrimaryScope::Task(task.id.clone()),
                (None, ContextFact::Bound { value, .. }) => {
                    PrimaryScope::Wave(crate::id::WaveId::parse(value)?)
                }
                _ => PrimaryScope::Repository(crate::repository::CanonicalRepo::discover(
                    &repository_path,
                )?),
            };
            // A Task's primary follows its claim; a Wave's or repository's
            // follows your Session Machine for it.
            let setting = crate::durable::MachineSetting::Session;
            let local = store.sqlite.local_machine()?.id;
            let remote = |machine: crate::durable::MachineId| {
                (machine != local).then(|| machine.to_string())
            };
            let machine = match (&scope, &repository_id) {
                (PrimaryScope::Task(_), _) => execution_machine.clone(),
                (PrimaryScope::Wave(wave), _) => {
                    remote(store.sqlite.wave_owning_machine(wave, setting)?.0)
                }
                (PrimaryScope::Repository(_), Some(repository)) => remote(
                    store
                        .sqlite
                        .repository_owning_machine(
                            &crate::durable::RepositoryId::from_raw(repository.clone()),
                            setting,
                        )?
                        .0,
                ),
                (PrimaryScope::Repository(_), None) => None,
            };
            match machine {
                Some(machine) => (None, Some(machine)),
                None => (
                    store
                        .sqlite
                        .primary_session(&scope)?
                        .map(|session| session.id.into()),
                    None,
                ),
            }
        }
        None => (None, None),
    };
    let label = match (&task, &wave) {
        (Some(task), _) => crate::terminal_title::workspace_name(&task.identifier, &task.title),
        (None, Some(wave)) => format!("{short_name} · {wave} Wave"),
        (None, None) => short_name,
    };
    Ok(Resolved {
        url,
        label,
        repository_path,
        repository_id,
        task,
        wave,
        session,
        session_cwd,
        agent: None,
        primary,
        primary_machine,
        execution_machine,
        execution_unavailable,
    })
}

/// One registered repository locator and the identity it currently selects.
struct Registered {
    path: PathBuf,
    canonical: PathBuf,
    id: Option<crate::durable::RepositoryId>,
    locators: Vec<crate::durable::RepositoryId>,
}

fn registered(store: Option<&Store>) -> Result<Vec<Registered>> {
    let Some(store) = store else {
        return Ok(Vec::new());
    };
    store
        .sqlite
        .repository_paths()?
        .into_iter()
        .map(|path| {
            let identity = store
                .sqlite
                .repository_identity(&path.display().to_string())?;
            Ok(Registered {
                canonical: path.canonicalize().unwrap_or_else(|_| path.clone()),
                id: identity.as_ref().map(|identity| identity.id.clone()),
                locators: identity
                    .map(|identity| identity.locators)
                    .unwrap_or_default(),
                path,
            })
        })
        .collect()
}

fn folder_name(path: &Path) -> Option<&str> {
    path.file_name().and_then(|name| name.to_str())
}

/// Registered repositories whose folder has this name, collapsed by location
/// and identity. Folder names locate; the registered identity decides.
fn repository_for(store: Option<&Store>, url: &WorkUrl, directory: &Path) -> Result<PathBuf> {
    let registered = registered(store)?;
    if let Some(qualifier) = &url.qualifier {
        return registered
            .iter()
            .find(|repo| repo.id.as_ref() == Some(qualifier) || repo.locators.contains(qualifier))
            .map(|repo| repo.path.clone())
            .with_context(|| {
                format!(
                    "Repository {qualifier} is not registered on this Machine. Nothing was opened."
                )
            });
    }
    let mut candidates: Vec<&Registered> = Vec::new();
    for repo in &registered {
        if folder_name(&repo.path) == Some(url.repository.as_str())
            && !candidates.iter().any(|known| {
                known.canonical == repo.canonical || (repo.id.is_some() && known.id == repo.id)
            })
        {
            candidates.push(repo);
        }
    }
    match candidates.as_slice() {
        [repo] => Ok(repo.path.clone()),
        [] => {
            // The current checkout locates a plain repository before it has a plan.
            let current = crate::repository::CanonicalRepo::containing(directory)
                .ok()
                .flatten()
                .map(|repo| repo.as_path().to_path_buf())
                .filter(|path| folder_name(path) == Some(url.repository.as_str()));
            current.with_context(|| format!(
                "No repository named {} is registered on this Machine. Run `lf open` inside it, or qualify the link with ?repository=<id>. Nothing was opened.",
                url.repository
            ))
        }
        _ => {
            let choices = candidates
                .iter()
                .map(|repo| {
                    let mut qualified = url.work();
                    qualified.qualifier = repo.id.clone();
                    format!("  {}: {qualified}", repo.path.display())
                })
                .collect::<Vec<_>>()
                .join("\n");
            bail!(
                "Several repositories are named {}; choose one. Nothing was opened.\n{choices}",
                url.repository
            )
        }
    }
}

/// Qualify a short name only when another registered repository shares it.
fn qualifier_for(
    registered: &[Registered],
    own: &Registered,
    short_name: &str,
) -> Option<crate::durable::RepositoryId> {
    let shared = registered.iter().any(|repo| {
        repo.canonical != own.canonical
            && folder_name(&repo.path) == Some(short_name)
            && (repo.id.is_none() || repo.id != own.id)
    });
    own.id.clone().filter(|_| shared)
}

/// Every conversation the shared reader judges available, as Work targets
/// where they are a Work's primary and exact Sessions otherwise.
fn available(cli: &crate::lf::Cli) -> Result<(Vec<Resolved>, Vec<String>)> {
    let runtime = tokio::runtime::Runtime::new()?;
    let store = runtime.block_on(crate::store::open_store(
        &crate::store::storage_config_from_env().context("resolve the shared Loopflow store")?,
    ))?;
    let store = std::sync::Arc::new(store);
    let scoped = cli.selects_repository();
    let wave = cli
        .wave
        .as_deref()
        .map(|wave| super::context::explain(Some(wave), None, None, None))
        .transpose()?
        .and_then(|context| match context.wave {
            ContextFact::Bound { value, .. } => Some(value),
            _ => None,
        });
    let records = runtime.block_on(crate::ops::human_session::list(
        &store,
        &crate::session::SessionFilter {
            blocked: false,
            on: None,
            orphan: false,
            repo: if scoped {
                crate::repository::CanonicalRepo::current()?.map(|repo| repo.to_string())
            } else {
                None
            },
            task: cli.task.clone(),
            search: None,
            interactive: Some(true),
            available: true,
            history: false,
            limit: 0,
            offset: 0,
            after: None,
        },
    ))?;
    let caller = crate::journal::agent_caller().map(|caller| caller.session_id);
    let mut targets: Vec<Resolved> = Vec::new();
    let mut unknown = Vec::new();
    for record in records {
        if caller.as_deref() == Some(record.id.as_str())
            || wave.as_ref().is_some_and(|wave| {
                record.wave_id.as_ref().map(|id| id.as_str()) != Some(wave.as_str())
            })
        {
            continue;
        }
        match available_target(&store, &record) {
            Ok(resolved) => {
                if !targets
                    .iter()
                    .any(|known| known.url == resolved.url && known.session == resolved.session)
                {
                    targets.push(resolved);
                }
            }
            Err(error) => unknown.push(format!("{} ({}): {error:#}", record.title, record.id)),
        }
    }
    Ok((targets, unknown))
}

fn available_target(
    store: &Store,
    record: &crate::ops::human_session::SessionRecord,
) -> Result<Resolved> {
    let session = store
        .sqlite
        .session(&record.id)?
        .context("Session is no longer recorded")?;
    let repo = session
        .repo
        .clone()
        .or_else(|| {
            crate::repository::CanonicalRepo::discover(&session.cwd)
                .ok()
                .map(|repo| repo.to_string())
        })
        .context("Session has no repository")?;
    let mut context = ContextExplanation::empty();
    context.repository_path = ContextFact::bound(&repo, "session");
    if let Some(id) = store.sqlite.repository_id(&repo)? {
        context.repository = ContextFact::bound(id, "selected_plan");
    }
    let primary = record.task_primary
        || matches!(record.primary_scope.as_deref(), Some("repository" | "wave"));
    match (&session.task_id, &session.wave_id) {
        (Some(task), _) => context.task = ContextFact::bound(task, "session"),
        (None, Some(wave)) if record.primary_scope.as_deref() == Some("wave") => {
            context.wave = ContextFact::bound(wave, "session")
        }
        _ => {}
    }
    let exact = (!primary).then(|| record.id.to_string());
    let mut resolved = describe(
        Some(store),
        &context,
        exact.clone(),
        false,
        exact.as_ref().map(|_| session.cwd.clone()),
    )?;
    if exact.is_some() {
        resolved.label = match &resolved.task {
            Some(task) => format!("{} · {}", session.title, task.identifier),
            None => session.title.clone(),
        };
    }
    Ok(resolved)
}

// ---------------------------------------------------------------------------
// Routing
// ---------------------------------------------------------------------------

/// Destinations in order. Only explicit choice or the direct caller's own
/// terminal context selects a host; nothing scans running applications.
fn route(requested: OpenHost) -> OpenRoute {
    if requested != OpenHost::Auto {
        return OpenRoute {
            hosts: vec![requested],
            reason: format!("{} was requested", requested.label()),
        };
    }
    // The host the caller is in is the only destination: failure there never
    // falls through to another host. An agent's current viewer outranks its
    // startup terminal, and a Herdr pane or tmux session inside a cmux or
    // Desktop terminal is the innermost, current host.
    let current = if let Some((host, _)) = viewed_placement() {
        Some((host, "where this conversation is viewed"))
    } else if in_rex() {
        Some((OpenHost::Rex, "this Rex terminal"))
    } else if in_herdr() {
        Some((OpenHost::Herdr, "this Herdr pane"))
    } else if in_tmux() {
        Some((OpenHost::Tmux, "this tmux session"))
    } else if in_cmux() {
        Some((OpenHost::Cmux, "this cmux workspace"))
    } else {
        None
    };
    if let Some((host, reason)) = current {
        return OpenRoute {
            hosts: vec![host],
            reason: format!("{reason}; failure there never falls through to another host"),
        };
    }
    OpenRoute {
        hosts: vec![OpenHost::Terminal],
        reason: "the current terminal".into(),
    }
}

fn set(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

const CMUX_WORKSPACE: &str = "CMUX_WORKSPACE_ID";
const CMUX_SOCKET: &str = "CMUX_SOCKET_PATH";
const HERDR_ENV: &str = "HERDR_ENV";
const HERDR_SOCKET: &str = "HERDR_SOCKET_PATH";
const HERDR_WORKSPACE: &str = "HERDR_WORKSPACE_ID";
const HERDR_PANE: &str = "HERDR_PANE_ID";
const TMUX: &str = "TMUX";
const TMUX_PANE: &str = "TMUX_PANE";
/// Set by Loopflow Desktop on every child, including its terminals' agents,
/// to its own bundle identifier.
const DESKTOP: &str = "LF_DESKTOP";

/// Terminal-host context a direct caller carries. The link receiver removes
/// it: an OS receiver's environment belongs to its first launch, not the click.
pub(crate) const HOST_CONTEXT: &[&str] = &[
    "REX_SERVER",
    "REX_SESSION",
    "REX_BLOCK",
    CMUX_WORKSPACE,
    CMUX_SOCKET,
    HERDR_ENV,
    HERDR_SOCKET,
    HERDR_WORKSPACE,
    HERDR_PANE,
    TMUX,
    TMUX_PANE,
    DESKTOP,
];

fn in_cmux() -> bool {
    set(CMUX_WORKSPACE) && set(CMUX_SOCKET)
}

/// The Desktop a caller runs in, else the installed app.
pub(crate) fn desktop_bundle() -> String {
    named_bundle(std::env::var(DESKTOP).ok().as_deref()).to_owned()
}

fn named_bundle(desktop: Option<&str>) -> &str {
    desktop
        .filter(|value| !value.is_empty())
        .unwrap_or(DESKTOP_BUNDLE)
}

/// The Desktop an opening reaches. Launch Services resolves a bundle id to any
/// registered copy, so only a caller's own non-release Desktop is named by id;
/// everything else launches the installed app by path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DesktopApp {
    /// The Desktop this caller runs inside, named by `LF_DESKTOP`, when the
    /// installed app is not it.
    Running(String),
    Installed(PathBuf),
}

impl DesktopApp {
    pub(crate) fn current() -> Option<Self> {
        Self::select(
            std::env::var(DESKTOP).ok().as_deref(),
            crate::installation::installed_app(),
        )
    }

    /// Only release packaging carries `DESKTOP_BUNDLE`, so a caller inside
    /// that Desktop reaches it by the installed path, which names one copy
    /// where the id names any registered one.
    fn select(desktop: Option<&str>, installed: Option<PathBuf>) -> Option<Self> {
        match (desktop.filter(|value| !value.is_empty()), installed) {
            (Some(bundle), Some(app)) if bundle == DESKTOP_BUNDLE => Some(Self::Installed(app)),
            (Some(bundle), _) => Some(Self::Running(bundle.to_owned())),
            (None, installed) => installed.map(Self::Installed),
        }
    }

    /// `open` arguments that address this app.
    pub(crate) fn open_args(&self) -> [&std::ffi::OsStr; 2] {
        match self {
            Self::Running(bundle) => ["-b".as_ref(), bundle.as_ref()],
            Self::Installed(app) => ["-a".as_ref(), app.as_os_str()],
        }
    }
}

fn in_rex() -> bool {
    set("REX_SESSION") || set("REX_BLOCK")
}

fn in_herdr() -> bool {
    std::env::var(HERDR_ENV).as_deref() == Ok("1") && set(HERDR_SOCKET) && set(HERDR_PANE)
}

fn in_tmux() -> bool {
    set(TMUX) && set(TMUX_PANE)
}

/// The host workspace this direct process runs in, recorded as its attached
/// terminal placement (host endpoint and view identity)
/// so a later opening can focus that conversation instead of starting another client.
pub(crate) fn host_placement() -> Option<String> {
    if in_rex() {
        let location = rex::RexLocation::current().ok()?;
        return Some(format!("rex:{}", serde_json::to_string(&location).ok()?));
    }
    if in_herdr() {
        return Some(format!(
            "herdr:{}",
            serde_json::to_string(&herdr::HerdrLocation::current()?).ok()?
        ));
    }
    if in_cmux() {
        return Some(format!(
            "cmux:{}",
            serde_json::to_string(&cmux::CmuxLocation::current()?).ok()?
        ));
    }
    Some(format!(
        "tmux:{}",
        serde_json::to_string(&tmux::TmuxLocation::current()?).ok()?
    ))
}

/// Where an agent caller's own conversation is viewed now. Its environment
/// names the terminal it launched in, which may since be closed or left.
fn viewed_placement() -> Option<(OpenHost, String)> {
    let caller = crate::journal::agent_caller()?;
    let store = crate::store::read_existing_registry().ok().flatten()?;
    let placement = store
        .sqlite
        .session_summary(
            caller.session_id.as_str(),
            time::OffsetDateTime::now_utc().unix_timestamp(),
        )
        .ok()
        .flatten()?
        .terminal_id?;
    placement_host(&placement).map(|(host, id)| (host, id.to_owned()))
}

fn placement_host(placement: &str) -> Option<(OpenHost, &str)> {
    let (prefix, id) = placement.split_once(':')?;
    let host = match prefix {
        "cmux" => OpenHost::Cmux,
        "herdr" => OpenHost::Herdr,
        "tmux" => OpenHost::Tmux,
        "rex" => OpenHost::Rex,
        "desktop" => OpenHost::Desktop,
        _ => return None,
    };
    Some((host, id))
}

/// Why one destination did not open. A definite failure created nothing, so
/// the next destination may proceed; an unknown effect must not be repeated.
enum RouteError {
    Definite(String),
    Unknown(String),
}

impl RouteError {
    fn reason(&self) -> &str {
        let (Self::Definite(reason) | Self::Unknown(reason)) = self;
        reason
    }
}

/// Serialize selection and presentation until attachment settles.
struct Opening {
    _lock: std::fs::File,
}

impl Opening {
    fn lock(target: &Resolved) -> Result<Self> {
        use fs2::FileExt;
        let directory = crate::store::lf_home_dir().join("open");
        std::fs::create_dir_all(&directory).context("create the opening directory")?;
        let key = target
            .conversation()
            .map(str::to_owned)
            .unwrap_or_else(|| target.url.work().to_string());
        let name = hex::encode(&sha2::Sha256::digest(key.as_bytes())[..16]);
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(directory.join(format!("{name}.lock")))
            .context("open the Work opening lock")?;
        let deadline = std::time::Instant::now() + HOST_TIMEOUT * 3;
        loop {
            match file.try_lock_exclusive() {
                Ok(()) => return Ok(Self { _lock: file }),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    anyhow::ensure!(
                        std::time::Instant::now() < deadline,
                        "another opening of this Work is still in progress; retry when it settles"
                    );
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(error) => return Err(error).context("lock the Work opening"),
            }
        }
    }
}

struct Live {
    reason: String,
    /// The attached client's recorded host workspace, when it has one.
    placement: Option<String>,
}

/// Read known live views by Session identity, without changing their input.
fn elsewhere(target: &Resolved) -> Result<Option<Live>> {
    let Some(id) = target.conversation() else {
        return Ok(None);
    };
    if let Some(view) = super::desktop::conversation_view(id)? {
        let location = format!("desktop:{}", serde_json::to_string(&view)?);
        return Ok(Some(Live {
            reason: format!("Already open in Desktop (window {}, pane {}); reveal it with `lf open --session {id} --focus`", view.window, view.pane),
            placement: Some(location),
        }));
    }
    let Some(store) = crate::store::read_existing_registry()? else {
        return Ok(None);
    };
    let Some(summary) = crate::ops::human_session::viewed_summary(&store.sqlite, id)? else {
        return Ok(None);
    };
    let viewed = if let Some(owner) = store.sqlite.session_transport(id)? {
        crate::transport::controlled(&owner).unwrap_or(false)
    } else {
        summary.agent_process_evidence == crate::journal::ProcessIdentityEvidence::Live
            && !summary.unviewed
    };
    Ok(viewed.then(|| Live {
        reason: format!(
            "Already open at {}; reveal it with `lf open --session {id} --focus`",
            summary.terminal_id.as_deref().unwrap_or("another client")
        ),
        placement: summary.terminal_id,
    }))
}

fn focus_destination(host: OpenHost, destination: &str) -> Result<(), RouteError> {
    let result = match host {
        OpenHost::Tmux => return tmux::focus(destination),
        OpenHost::Desktop => super::desktop::focus_conversation(destination),
        OpenHost::Rex => rex::focus_view(destination),
        OpenHost::Herdr => herdr::Herdr::focus_view(destination),
        OpenHost::Cmux => cmux::Cmux::focus_view(destination),
        _ => return Err(RouteError::Definite("This view cannot be focused".into())),
    };
    result.map_err(|error| RouteError::Definite(format!("{error:#}")))
}

/// Send one conversation or arrangement address without changing hosts.
fn open_desktop(url: &str, focus: bool) -> Result<(), RouteError> {
    if !cfg!(target_os = "macos") {
        return Err(RouteError::Definite(
            "Loopflow Desktop requires macOS; use --host terminal".into(),
        ));
    }
    let app = DesktopApp::current().ok_or_else(|| RouteError::Definite(NO_DESKTOP.into()))?;
    let mut command = Command::new("open");
    command.args(app.open_args());
    if !focus {
        command.arg("-g");
    }
    command.arg(url);
    run_host(command, "Launch Services").map(drop)
}

/// Run one host request with a finite timeout and return its stdout.
fn run_host(mut command: Command, host: &str) -> Result<String, RouteError> {
    // Native inventories can exceed a pipe buffer. Spool while the child runs
    // so waiting for exit cannot deadlock behind an unread stdout/stderr pipe.
    let capture = || tempfile::tempfile().map_err(|error| RouteError::Definite(error.to_string()));
    let mut stdout = capture()?;
    let mut stderr = capture()?;
    let output = stdout
        .try_clone()
        .map_err(|error| RouteError::Definite(error.to_string()))?;
    let errors = stderr
        .try_clone()
        .map_err(|error| RouteError::Definite(error.to_string()))?;
    let mut child = command
        .stdin(Stdio::null())
        .stdout(output)
        .stderr(errors)
        .spawn()
        .map_err(|error| RouteError::Definite(format!("cannot run {host}: {error}")))?;
    let (status, timed_out) =
        crate::os_process::wait_for_exit(&mut child, Some(HOST_TIMEOUT), || {})
            .map_err(|error| RouteError::Unknown(error.to_string()))?;
    if timed_out {
        let _ = child.kill();
        let _ = child.wait();
        return Err(RouteError::Unknown(format!(
            "{host} did not answer within {}s",
            HOST_TIMEOUT.as_secs()
        )));
    }
    use std::io::Seek;
    stdout
        .rewind()
        .map_err(|error| RouteError::Unknown(error.to_string()))?;
    stderr
        .rewind()
        .map_err(|error| RouteError::Unknown(error.to_string()))?;
    let stdout =
        std::io::read_to_string(stdout).map_err(|error| RouteError::Unknown(error.to_string()))?;
    let stderr =
        std::io::read_to_string(stderr).map_err(|error| RouteError::Unknown(error.to_string()))?;
    if status.success() {
        Ok(stdout)
    } else {
        let detail = stderr.trim();
        Err(RouteError::Definite(if detail.is_empty() {
            format!("{host} exited with {status}")
        } else {
            format!("{host} refused: {detail}")
        }))
    }
}

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, clap::Args)]
pub struct OpenSelection {
    /// Work address, such as loopflow://loopflow/task/LOO-507
    pub url: Option<String>,
    /// Destination host; auto uses the calling terminal, never a fallback host
    #[arg(long, value_enum, default_value = "auto")]
    pub host: OpenHost,
    /// Open this exact Session rather than its scope's primary
    #[arg(long)]
    pub session: Option<String>,
    /// Show the Task's Changes browser beside its conversation (Desktop)
    #[arg(long)]
    pub diff: bool,
    /// Print a link without opening or preparing anything
    #[arg(long)]
    pub link: bool,
    #[arg(long, value_enum, requires = "link")]
    pub format: Option<LinkFormat>,
    #[arg(long)]
    pub json: bool,
    /// Rex Unix socket endpoint
    #[arg(long)]
    pub server: Option<String>,
}

#[derive(Debug, Clone, clap::Args)]
#[command(
    after_help = "Direct CLI calls attach here; agent calls open a sibling tab. An already-open conversation stays where it is: use --focus to reveal it, or lf resume to move input. Layouts and overviews: lf workspace."
)]
pub struct OpenArgs {
    #[command(flatten)]
    pub selection: OpenSelection,
    /// Attach in this terminal (direct CLI default)
    #[arg(long, conflicts_with_all = ["tab", "focus"])]
    pub here: bool,
    /// Attach in a sibling tab (agent default)
    #[arg(long, conflicts_with = "focus")]
    pub tab: bool,
    /// Reveal an existing conversation view; create nothing
    #[arg(long)]
    pub focus: bool,
    /// Present original files in an attached Desktop Files view
    #[arg(long = "file", value_name = "PATH", conflicts_with_all = ["session", "diff", "link", "here", "tab", "focus"])]
    pub files: Vec<PathBuf>,
}

#[derive(Debug, Clone, clap::Args)]
pub struct WorkspaceArgs {
    #[command(flatten)]
    pub selection: OpenSelection,
    /// Open every conversation available for you to work with
    #[arg(long, conflicts_with_all = ["url", "session"])]
    pub available: bool,
    /// Keep the host's current selection
    #[arg(long)]
    pub no_focus: bool,
    #[arg(long, hide = true, requires = "server")]
    pub follow: Option<String>,
    #[arg(long, hide = true, requires = "follow", value_delimiter = ',')]
    pub seen: Vec<String>,
    #[arg(long, hide = true)]
    pub local_workspace: bool,
    #[arg(long, hide = true, requires = "local_workspace", value_delimiter = ',')]
    pub arrive_task: Vec<String>,
    #[arg(long, hide = true, requires = "arrive_task")]
    pub into: Option<String>,
    #[arg(long = "remote", hide = true, requires = "follow")]
    pub remote: Vec<String>,
}

/// The one conversation named by an address, root selectors or checkout.
fn target(cli: &crate::lf::Cli, args: &OpenSelection) -> Result<Resolved> {
    let store = crate::store::read_existing_registry()?;
    let store = store.as_ref();
    let session = args.session.clone();
    let resolved = match &args.url {
        Some(text) => {
            anyhow::ensure!(
                cli.task.is_none() && cli.wave.is_none(),
                "select Work with either a link or --task/--wave, not both"
            );
            url_target(store, &std::env::current_dir()?, text, session, args.diff)?
        }
        None => {
            let context = selected_context(cli, session.as_deref())?;
            describe(store, &context, session, args.diff, None)?
        }
    };
    Ok(resolved)
}

/// The Work a link names, resolved as if `lf open` ran in `directory`.
fn url_target(
    store: Option<&Store>,
    directory: &Path,
    text: &str,
    session: Option<String>,
    diff: bool,
) -> Result<Resolved> {
    let url = WorkUrl::parse(text)?;
    let session = session.or_else(|| url.session.clone());
    let diff = diff || url.diff;
    let repository = repository_for(store, &url, directory)?;
    let (task, wave) = match &url.target {
        WorkTarget::Task(id) => (Some(id.as_str()), None),
        WorkTarget::Wave(name) => (None, Some(name.as_str())),
        WorkTarget::Repository => (None, None),
    };
    let mut context = work_context(&repository, task, wave, session.as_deref())?;
    // An explicit repository address does not inherit the checkout's Task.
    if matches!(url.target, WorkTarget::Repository) && session.is_none() {
        context.task = ContextFact::Unbound;
        context.wave = ContextFact::Unbound;
    }
    describe(store, &context, session, diff, None)
}

/// What `lf --cwd <directory> open <url> --link --json` prints.
pub fn url_links(directory: &Path, url: &str) -> Result<Vec<WorkLink>> {
    let store = crate::store::read_existing_registry()?;
    let target = url_target(store.as_ref(), directory, url, None, false)?;
    let presentation = if crate::journal::agent_caller().is_some() {
        route(OpenHost::Auto)
    } else {
        conversation::here_route()
    };
    Ok(vec![target.link(presentation)])
}

pub fn explain(cli: &crate::lf::Cli, args: &OpenArgs, json: bool) -> Result<()> {
    if !args.files.is_empty() {
        return files::run(cli, args, true);
    }
    let link = target(cli, &args.selection)?.link(conversation::presentation(args)?);
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({"links": [link], "unknown": []}))?
        );
        return Ok(());
    }
    println!("{}  {}", link.label, link.url);
    println!("  Route: {}", link.route.reason);
    println!("Nothing was executed; no app, checkout or Session was prepared.");
    Ok(())
}

pub fn run(cli: &crate::lf::Cli, args: &OpenArgs) -> Result<()> {
    if !args.files.is_empty() {
        return files::run(cli, args, false);
    }
    if args.selection.link {
        return print_links(
            cli,
            &args.selection,
            false,
            conversation::presentation(args)?,
        );
    }
    let receipt = conversation::open(target(cli, &args.selection)?, args, true);
    print_report(
        OpenReport {
            receipts: vec![receipt],
            unknown: Vec::new(),
        },
        args.selection.json || cli.json,
    )
}

pub fn explain_workspace(cli: &crate::lf::Cli, args: &WorkspaceArgs, json: bool) -> Result<()> {
    let mut selection = args.selection.clone();
    selection.json = json;
    print_links(cli, &selection, args.available, route(selection.host))
}

pub fn run_workspace(cli: &crate::lf::Cli, args: &WorkspaceArgs) -> Result<()> {
    if args.selection.link {
        return print_links(
            cli,
            &args.selection,
            args.available,
            route(args.selection.host),
        );
    }
    let host = route(args.selection.host).hosts[0];
    if host == OpenHost::Desktop {
        let (targets, unknown) = workspace_targets(cli, args)?;
        let receipts = targets
            .iter()
            .enumerate()
            .map(|(index, target)| {
                let focus = !args.no_focus && index + 1 == targets.len();
                if target.session.is_some() || args.available {
                    return conversation::open(
                        target.clone(),
                        &OpenArgs {
                            selection: args.selection.clone(),
                            here: false,
                            tab: false,
                            focus: false,
                            files: Vec::new(),
                        },
                        focus,
                    );
                }
                let mut url = reqwest::Url::parse("loopflow://open").expect("fixed Desktop URL");
                let expression = match (&target.task, &target.wave) {
                    (Some(task), _) => format!("task:{}", task.id),
                    (_, Some(wave)) => format!("wave:{wave} kind:task state:open"),
                    _ => "kind:task state:open".into(),
                };
                url.query_pairs_mut()
                    .append_pair("repo", &target.repository_path.to_string_lossy())
                    .append_pair("view", &expression);
                match open_desktop(url.as_str(), focus) {
                    Ok(()) => target.receipt(Some(host), None, OpenStatus::Opening, None),
                    Err(error) => target.receipt(
                        Some(host),
                        None,
                        OpenStatus::Failed,
                        Some(error.reason().into()),
                    ),
                }
            })
            .collect();
        return print_report(
            OpenReport { receipts, unknown },
            args.selection.json || cli.json,
        );
    }
    anyhow::ensure!(
        matches!(host, OpenHost::Rex | OpenHost::Herdr | OpenHost::Cmux),
        "{} does not support workspaces; use lf open, lf task view or lf diff",
        host.label()
    );
    anyhow::ensure!(
        args.selection.server.is_none() || host == OpenHost::Rex,
        "--server selects a Rex server"
    );
    workspace::run(cli, args, host)
}

fn workspace_targets(
    cli: &crate::lf::Cli,
    args: &WorkspaceArgs,
) -> Result<(Vec<Resolved>, Vec<String>)> {
    if args.available {
        available(cli)
    } else {
        Ok((vec![target(cli, &args.selection)?], Vec::new()))
    }
}

fn print_links(
    cli: &crate::lf::Cli,
    args: &OpenSelection,
    all: bool,
    presentation: OpenRoute,
) -> Result<()> {
    let (targets, unknown) = if all {
        available(cli)?
    } else {
        (vec![target(cli, args)?], Vec::new())
    };
    let links: Vec<_> = targets
        .iter()
        .map(|target| target.link(presentation.clone()))
        .collect();
    if args.json || cli.json {
        println!("{}", serde_json::to_string_pretty(&links)?);
    } else {
        if all && links.is_empty() {
            println!("No available conversations.");
        }
        for link in &links {
            match args.format.unwrap_or(LinkFormat::Text) {
                LinkFormat::Markdown => println!("{}", link.markdown),
                LinkFormat::Text => {
                    let label = if std::io::stdout().is_terminal() {
                        crate::work::link::osc8(&link.label, &link.url)
                    } else {
                        link.label.clone()
                    };
                    println!("{label}  {}", fallback_text(&link.fallback_argv));
                }
            }
        }
        print_unknown(&unknown);
    }
    Ok(())
}

fn print_report(report: OpenReport, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        for receipt in &report.receipts {
            println!(
                "{}: {}",
                receipt.label,
                receipt.reason.as_deref().unwrap_or(match receipt.status {
                    OpenStatus::Opening => "Opened workspace",
                    OpenStatus::Usable => "Attached",
                    OpenStatus::Elsewhere => "Already open",
                    OpenStatus::Failed => "Opening failed",
                })
            );
            if receipt.status == OpenStatus::Failed {
                if let Some(destination) = &receipt.destination {
                    println!("  Destination: {destination}");
                }
            }
        }
        print_unknown(&report.unknown);
    }
    if report
        .receipts
        .iter()
        .any(|receipt| receipt.status == OpenStatus::Failed)
    {
        return Err(crate::process::CommandExit(1).into());
    }
    Ok(())
}

fn fallback_text(argv: &[String]) -> String {
    shlex::try_join(argv.iter().map(String::as_str)).unwrap_or_else(|_| argv.join(" "))
}

fn print_unknown(unknown: &[String]) {
    for item in unknown {
        println!("Unknown: {item}");
    }
}

#[cfg(test)]
mod tests {
    use super::{named_bundle, DesktopApp, DESKTOP_BUNDLE};
    use std::path::PathBuf;

    #[test]
    fn host_inventory_can_exceed_pipe_capacity() {
        let mut command = std::process::Command::new("/bin/sh");
        command.args([
            "-c",
            "head -c 131072 /dev/zero; head -c 131072 /dev/zero >&2",
        ]);
        assert_eq!(
            super::run_host(command, "fixture")
                .unwrap_or_else(|error| panic!("{}", error.reason()))
                .len(),
            131072
        );
    }

    #[test]
    fn desktop_bundle_defaults_to_the_release_id() {
        for desktop in [None, Some("")] {
            assert_eq!(named_bundle(desktop), DESKTOP_BUNDLE, "{desktop:?}");
        }
        assert_eq!(
            named_bundle(Some("com.loopflow.mac.dev")),
            "com.loopflow.mac.dev"
        );
    }

    #[test]
    fn desktop_launches_the_installed_app_by_path() {
        // Another registered copy may carry the same bundle id; a path names one.
        let installed = PathBuf::from("/Applications/Loopflow.app");
        let app = DesktopApp::select(Some(""), Some(installed.clone())).unwrap();
        assert_eq!(app.open_args(), ["-a".as_ref(), installed.as_os_str()]);
        assert_eq!(DesktopApp::select(None, None), None);
    }

    #[test]
    fn desktop_inside_a_build_routes_back_to_it_by_bundle_id() {
        let installed = PathBuf::from("/Applications/Loopflow.app");
        let app = DesktopApp::select(Some("com.loopflow.mac.dev"), Some(installed)).unwrap();
        assert_eq!(
            app.open_args(),
            ["-b".as_ref(), std::ffi::OsStr::new("com.loopflow.mac.dev")]
        );
    }

    #[test]
    fn desktop_inside_the_installed_app_reaches_it_by_path() {
        // `open -b com.loopflow.mac` was the 2026-11-02 incident command: the
        // release id resolves to any registered copy, the path to one.
        let installed = PathBuf::from("/Applications/Loopflow.app");
        let app = DesktopApp::select(Some(DESKTOP_BUNDLE), Some(installed.clone())).unwrap();
        assert_eq!(app.open_args(), ["-a".as_ref(), installed.as_os_str()]);

        let app = DesktopApp::select(Some(DESKTOP_BUNDLE), None).unwrap();
        assert_eq!(
            app.open_args(),
            ["-b".as_ref(), std::ffi::OsStr::new(DESKTOP_BUNDLE)]
        );
    }
}
