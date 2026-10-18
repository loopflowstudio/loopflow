//! `lf open`: resolve Work through its shared owners, choose one presentation
//! destination, and ask that destination to open the Work's conversation.
//!
//! Hosts translate only documented caller context and open/focus operations.
//! They never select a conversation: every terminal destination runs
//! `lf session connect` for the resolved Work, whose admission and attachment
//! fences own reuse. Desktop resolves the same address through `--link --json`.

use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clap::ValueEnum;
use serde::{Deserialize, Serialize};

use super::desktop::WorkWorkspaceKind;
use crate::ops::context::{ContextExplanation, ContextFact};
use crate::session::PrimaryScope;
use crate::store::Store;
use crate::work::link::{WorkTarget, WorkUrl};

const DESKTOP_BUNDLE: &str = "com.loopflow.mac";
const HOST_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum OpenHost {
    Auto,
    Desktop,
    Cmux,
    Herdr,
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
    pub repository_path: String,
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

/// Destinations this invocation would try, in order, from its own context.
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
    /// The conversation ran in this terminal.
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
    /// The Work's current primary conversation, read without admitting one.
    primary: Option<String>,
    execution_machine: Option<String>,
}

impl Resolved {
    fn conversation(&self) -> Option<&str> {
        self.session.as_deref().or(self.primary.as_deref())
    }

    fn connect_argv(&self, lf: &str) -> Vec<String> {
        let mut argv = vec![lf.to_string(), "session".into(), "connect".into()];
        match (&self.session, &self.url.target) {
            (Some(session), _) => argv.push(session.clone()),
            (None, WorkTarget::Task(id)) => argv.extend(["--task".into(), id.clone()]),
            (None, WorkTarget::Wave(name)) => argv.extend(["--wave".into(), name.clone()]),
            (None, WorkTarget::Repository) => {}
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
            return self.connect_argv("lf");
        }
        vec!["lf".into(), "open".into(), self.url.to_string()]
    }

    fn receipt(
        &self,
        host: Option<OpenHost>,
        status: OpenStatus,
        reason: Option<String>,
    ) -> OpenReceipt {
        OpenReceipt {
            label: self.label.clone(),
            url: self.url.to_string(),
            host,
            destination: None,
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
    /// Same Work is not enough: a Flow step and the primary can share a Task.
    fn relation(&self) -> (ConversationRelation, Option<String>) {
        let Some(caller) = crate::journal::agent_caller() else {
            return (
                ConversationRelation::Unknown,
                Some("No calling conversation identity was supplied".into()),
            );
        };
        match self.conversation() {
            Some(id) if id == caller.session_id => {
                (ConversationRelation::CurrentConversation, None)
            }
            Some(_) => (ConversationRelation::OtherConversation, None),
            None => (
                ConversationRelation::OtherConversation,
                Some("This Work has no conversation yet; opening starts its primary".into()),
            ),
        }
    }
}

/// Root selectors as opening context, validated against an explicit --repo.
pub(crate) fn selected_context(
    cli: &crate::lf::Cli,
    session: Option<&str>,
) -> Result<ContextExplanation> {
    let resolution = if cli.task.is_none() && session.is_some() {
        let selected = work_context(None, None, session)?;
        if let Some(wave) = cli.wave.as_deref() {
            let work = super::context::explain(Some(wave), None, None, None)?;
            anyhow::ensure!(
                matches!((&work.wave, &selected.wave),
                (ContextFact::Bound { value: wave, .. }, ContextFact::Bound { value: session_wave, .. }) if wave == session_wave),
                "Session does not belong to the selected Work. Nothing was opened."
            );
        }
        selected
    } else {
        work_context(cli.task.as_deref(), cli.wave.as_deref(), session)?
    };
    if cli.repo.is_some() || cli.repository.is_some() {
        if let ContextFact::Bound { value, .. } = &resolution.repository_path {
            anyhow::ensure!(
                crate::repository::CanonicalRepo::current()?.as_ref()
                    == Some(&crate::repository::CanonicalRepo::discover(Path::new(
                        value
                    ))?),
                "Work does not belong to the selected repository. Nothing was opened."
            );
        }
    }
    Ok(resolution)
}

/// Work, its execution location and an optional exact Session, from the cwd.
/// A plain Git repository opens before planning has ever been initialized;
/// only an absent registry allows that.
fn work_context(
    task: Option<&str>,
    wave: Option<&str>,
    session: Option<&str>,
) -> Result<ContextExplanation> {
    if task.is_none()
        && wave.is_none()
        && session.is_none()
        && crate::store::read_existing_registry()?.is_none()
    {
        let repo = crate::repository::CanonicalRepo::current()?
            .context("select a Git repository with --repo before opening it")?;
        let mut resolution = ContextExplanation::empty();
        resolution.repository_path = ContextFact::bound(repo, "local_locator");
        return Ok(resolution);
    }
    let mut resolution = if task.is_none() && wave.is_none() && session.is_some() {
        super::context::explain(None, None, session, None)?
    } else {
        super::context::explain(wave, task, None, None)?
    };
    if let ContextFact::Bound { value, .. } = &resolution.task {
        let task = crate::durable::TaskId::parse(value)?;
        if let Some(store) = crate::store::read_existing_registry()? {
            // The shared reader retains failures in the explanation; opening
            // refuses unavailable execution without losing the other Work facts.
            let _ = tokio::runtime::Runtime::new()?.block_on(crate::ops::task_location::explain(
                &store.sqlite,
                &task,
                &mut resolution,
            ));
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
            let selected = super::context::explain(None, None, Some(session), None)?;
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
        ) if owner != local => {
            anyhow::ensure!(
                matches!(&context.checkout, ContextFact::Bound { .. }),
                "Recorded checkout unavailable on Machine {owner}; nothing was opened."
            );
            Some(owner.clone())
        }
        (Some(_), ContextFact::Unavailable { reason }, _) if session.is_some() || diff => {
            bail!("Task execution location unavailable: {reason}. Nothing was opened.");
        }
        _ => None,
    };
    let repository_id = match &context.repository {
        ContextFact::Bound { value, .. } => Some(value.clone()),
        _ => {
            let canonical = repository_path
                .canonicalize()
                .unwrap_or_else(|_| repository_path.clone());
            registered(store)?
                .into_iter()
                .find(|repo| repo.canonical == canonical)
                .and_then(|repo| repo.id)
                .map(|id| id.to_string())
        }
    };
    let short_name = repository_path
        .file_name()
        .and_then(|name| name.to_str())
        .context("Repository folder has no readable name")?
        .to_owned();
    let target = match (&task, &wave) {
        (Some(task), _) => WorkTarget::Task(task.identifier.clone()),
        (None, Some(wave)) => WorkTarget::Wave(wave.clone()),
        (None, None) => WorkTarget::Repository,
    };
    let mut url = WorkUrl::new(short_name.clone(), target);
    url.qualifier = qualifier_for(store, &repository_path, &short_name)?;
    // An exact Session travels with a Task link; elsewhere the connect argv carries it.
    if task.is_some() {
        url.session = session.clone();
        url.diff = diff;
    }
    let primary = store
        .map(|store| {
            let scope = match (&task, &context.wave) {
                (Some(task), _) => PrimaryScope::Task(task.id.clone()),
                (None, ContextFact::Bound { value, .. }) => {
                    PrimaryScope::Wave(crate::id::WaveId::parse(value)?)
                }
                _ => PrimaryScope::Repository(crate::repository::CanonicalRepo::discover(
                    &repository_path,
                )?),
            };
            anyhow::Ok(
                store
                    .sqlite
                    .primary_session(&scope)?
                    .map(|session| session.id),
            )
        })
        .transpose()?
        .flatten();
    let label = match (&task, &wave) {
        (Some(task), _) => format!("{} · {}", task.identifier, task.title),
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
        primary,
        execution_machine,
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
fn repository_for(store: Option<&Store>, url: &WorkUrl) -> Result<PathBuf> {
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
            let current = crate::repository::CanonicalRepo::current()
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
    store: Option<&Store>,
    path: &Path,
    short_name: &str,
) -> Result<Option<crate::durable::RepositoryId>> {
    let registered = registered(store)?;
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let Some(own) = registered.iter().find(|repo| repo.canonical == canonical) else {
        return Ok(None);
    };
    let shared = registered.iter().any(|repo| {
        repo.canonical != own.canonical
            && folder_name(&repo.path) == Some(short_name)
            && (repo.id.is_none() || repo.id != own.id)
    });
    Ok(own.id.clone().filter(|_| shared))
}

/// Every conversation the shared reader judges available, as Work targets
/// where they are a Work's primary and exact Sessions otherwise.
fn available(cli: &crate::lf::Cli) -> Result<(Vec<Resolved>, Vec<String>)> {
    let runtime = tokio::runtime::Runtime::new()?;
    let store = runtime.block_on(crate::store::open_store(
        &crate::store::storage_config_from_env().context("resolve the shared Loopflow store")?,
    ))?;
    let store = std::sync::Arc::new(store);
    let scoped = cli.repo.is_some() || cli.repository.is_some();
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
    let exact = (!primary).then(|| record.id.clone());
    let mut resolved = describe(
        Some(store),
        &context,
        exact.clone(),
        false,
        exact.as_ref().map(|_| session.cwd.clone()),
    )?;
    if exact.is_some() {
        resolved.label = match &resolved.task {
            Some(task) => format!("{} · {}", task.identifier, session.title),
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
    let mut hosts = Vec::new();
    let mut reasons = Vec::new();
    if cfg!(target_os = "macos") {
        hosts.push(OpenHost::Desktop);
        reasons.push("Desktop when Launch Services finds it");
    }
    // A Herdr pane inside a cmux terminal is the innermost, current host.
    if in_herdr() {
        hosts.push(OpenHost::Herdr);
        reasons.push("this Herdr pane");
    }
    if in_cmux() {
        hosts.push(OpenHost::Cmux);
        reasons.push("this cmux workspace");
    }
    hosts.push(OpenHost::Terminal);
    reasons.push("a plain terminal");
    OpenRoute {
        hosts,
        reason: format!("Automatic order: {}", reasons.join(", then ")),
    }
}

fn set(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| !value.is_empty())
}

fn in_cmux() -> bool {
    set("CMUX_WORKSPACE_ID") && set("CMUX_SOCKET_PATH")
}

fn in_herdr() -> bool {
    std::env::var("HERDR_ENV").as_deref() == Ok("1")
        && set("HERDR_SOCKET_PATH")
        && set("HERDR_PANE_ID")
}

/// Why one destination did not open. A definite failure created nothing, so
/// the next destination may proceed; an unknown effect must not be repeated.
enum RouteError {
    Definite(String),
    Unknown(String),
}

struct Opened {
    status: OpenStatus,
    destination: Option<String>,
}

fn open_all(targets: &[Resolved], route: &OpenRoute, in_place: bool) -> Vec<OpenReceipt> {
    let lf = crate::os_process::resolve_lf_binary()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| "lf".into());
    let mut receipts: Vec<OpenReceipt> = Vec::new();
    for (index, target) in targets.iter().enumerate() {
        let focus = index + 1 == targets.len();
        receipts.push(open_one(target, route, focus, in_place, &lf));
    }
    // Raise one destination: the last that opened, when the final item did not.
    if receipts
        .last()
        .is_some_and(|receipt| receipt.status != OpenStatus::Opening)
    {
        if let Some(receipt) = receipts
            .iter()
            .rev()
            .find(|receipt| receipt.status == OpenStatus::Opening)
        {
            if let (Some(host), Some(destination)) = (receipt.host, &receipt.destination) {
                focus_destination(host, destination);
            }
        }
    }
    receipts
}

fn open_one(
    target: &Resolved,
    route: &OpenRoute,
    focus: bool,
    in_place: bool,
    lf: &str,
) -> OpenReceipt {
    let elsewhere = elsewhere(target);
    let mut skipped = Vec::new();
    for &host in &route.hosts {
        if let Some(reason) = elsewhere.clone().filter(|_| host != OpenHost::Desktop) {
            return target.receipt(Some(host), OpenStatus::Elsewhere, Some(reason));
        }
        let attempt = match host {
            OpenHost::Desktop => open_desktop(target, focus),
            OpenHost::Cmux => open_cmux(target, focus, lf),
            OpenHost::Herdr => open_herdr(target, focus, lf),
            OpenHost::Terminal => open_terminal(target, focus, in_place, lf),
            OpenHost::Rex => Err(RouteError::Definite(
                "Rex opening is not supported yet; use --host terminal".into(),
            )),
            OpenHost::Auto => unreachable!("routes contain concrete hosts"),
        };
        match attempt {
            Ok(opened) => {
                let mut receipt = target.receipt(Some(host), opened.status, None);
                receipt.destination = opened.destination;
                return receipt;
            }
            Err(RouteError::Definite(reason)) => {
                skipped.push(format!("{}: {reason}", host.label()));
            }
            Err(RouteError::Unknown(reason)) => {
                return target.receipt(
                    Some(host),
                    OpenStatus::Failed,
                    Some(format!(
                        "{}: {reason}. It may have opened; check there before retrying.",
                        host.label()
                    )),
                );
            }
        }
    }
    target.receipt(None, OpenStatus::Failed, Some(skipped.join("; ")))
}

/// A live provider attached to another client keeps its conversation; ordinary
/// opening never takes it over. Desktop shows and moves its own retained panes.
fn elsewhere(target: &Resolved) -> Option<String> {
    let id = target.conversation()?;
    let store = crate::store::read_existing_registry().ok().flatten()?;
    let summary = store
        .sqlite
        .session_summary(id, time::OffsetDateTime::now_utc().unix_timestamp())
        .ok()
        .flatten()?;
    (summary.agent_process_evidence == crate::journal::ProcessIdentityEvidence::Live).then(|| {
        format!(
            "this conversation is already open in another client; move it here with `lf session connect {id} --takeover` (unsent text there is not transferred)"
        )
    })
}

fn open_desktop(target: &Resolved, focus: bool) -> Result<Opened, RouteError> {
    if !cfg!(target_os = "macos") {
        return Err(RouteError::Definite(
            "Loopflow Desktop requires macOS; use --host terminal".into(),
        ));
    }
    if target.session.is_some() && target.task.is_none() {
        return Err(RouteError::Definite(
            "Desktop opens exact Sessions through their Task".into(),
        ));
    }
    let mut command = Command::new("open");
    command.args(["-b", DESKTOP_BUNDLE]);
    if !focus {
        command.arg("-g");
    }
    command.arg(target.url.to_string());
    run_host(command, "Launch Services").map(|_| Opened {
        status: OpenStatus::Opening,
        destination: None,
    })
}

fn open_cmux(target: &Resolved, focus: bool, lf: &str) -> Result<Opened, RouteError> {
    let shell = shlex::try_join(target.connect_argv(lf).iter().map(String::as_str))
        .map_err(|error| RouteError::Definite(error.to_string()))?;
    let mut command = Command::new("cmux");
    command
        .env("CMUX_QUIET", "1")
        .args(["workspace", "create", "--name", &target.label, "--cwd"])
        .arg(target.cwd())
        .args([
            "--command",
            &shell,
            "--focus",
            if focus { "true" } else { "false" },
        ]);
    let output = run_host(command, "cmux")?;
    let destination = output
        .split_whitespace()
        .find(|word| word.starts_with("workspace:"))
        .map(str::to_owned);
    Ok(Opened {
        status: OpenStatus::Opening,
        destination,
    })
}

fn open_herdr(target: &Resolved, focus: bool, lf: &str) -> Result<Opened, RouteError> {
    let mut create = Command::new("herdr");
    create
        .args(["workspace", "create", "--cwd"])
        .arg(target.cwd())
        .args([
            "--label",
            &target.label,
            if focus { "--focus" } else { "--no-focus" },
        ]);
    let output = run_host(create, "Herdr")?;
    let created: serde_json::Value = serde_json::from_str(&output)
        .map_err(|error| RouteError::Unknown(format!("unreadable Herdr reply: {error}")))?;
    let workspace = created["result"]["workspace"]["workspace_id"]
        .as_str()
        .map(str::to_owned);
    let pane = created["result"]["root_pane"]["pane_id"]
        .as_str()
        .ok_or_else(|| RouteError::Unknown("Herdr reply named no pane".into()))?;
    // Herdr types the command into the newly allocated shell, never an existing one.
    let shell = shlex::try_join(target.connect_argv(lf).iter().map(String::as_str))
        .map_err(|error| RouteError::Unknown(error.to_string()))?;
    let mut run = Command::new("herdr");
    run.args(["pane", "run", pane, &shell]);
    run_host(run, "Herdr").map_err(|error| match error {
        RouteError::Definite(reason) | RouteError::Unknown(reason) => RouteError::Unknown(format!(
            "created workspace {} but could not start its conversation: {reason}",
            workspace.as_deref().unwrap_or(pane)
        )),
    })?;
    Ok(Opened {
        status: OpenStatus::Opening,
        destination: workspace,
    })
}

fn open_terminal(
    target: &Resolved,
    focus: bool,
    in_place: bool,
    lf: &str,
) -> Result<Opened, RouteError> {
    let argv = target.connect_argv(lf);
    if in_place {
        let status = Command::new(&argv[0])
            .args(&argv[1..])
            .current_dir(target.cwd())
            .status()
            .map_err(|error| RouteError::Definite(error.to_string()))?;
        if !status.success() {
            return Err(RouteError::Definite(format!(
                "the conversation exited with {status}"
            )));
        }
        return Ok(Opened {
            status: OpenStatus::Usable,
            destination: None,
        });
    }
    let script = format!(
        "cd {} && exec {}",
        shlex::try_quote(&target.cwd().display().to_string())
            .map_err(|error| RouteError::Definite(error.to_string()))?,
        shlex::try_join(argv.iter().map(String::as_str))
            .map_err(|error| RouteError::Definite(error.to_string()))?
    );
    if cfg!(target_os = "macos") {
        // Launch Services opens a .command in the default terminal, without
        // an automation permission dialog. The script removes itself first.
        let path = std::env::temp_dir().join(format!(
            "loopflow-open-{}.command",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::write(&path, format!("#!/bin/sh\nrm -f \"$0\"\n{script}\n"))
            .and_then(|()| {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))
            })
            .map_err(|error| RouteError::Definite(error.to_string()))?;
        let mut command = Command::new("open");
        if !focus {
            command.arg("-g");
        }
        command.arg(&path);
        return run_host(command, "Launch Services")
            .inspect_err(|_| {
                let _ = std::fs::remove_file(&path);
            })
            .map(|_| Opened {
                status: OpenStatus::Opening,
                destination: None,
            });
    }
    if !set("DISPLAY") && !set("WAYLAND_DISPLAY") {
        return Err(RouteError::Definite(
            "no graphical session or interactive terminal is reachable".into(),
        ));
    }
    for launcher in ["xdg-terminal-exec", "x-terminal-emulator"] {
        let mut command = Command::new(launcher);
        if launcher == "x-terminal-emulator" {
            command.arg("-e");
        }
        command
            .args(["sh", "-c", &script])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        match command.spawn() {
            Ok(_) => {
                return Ok(Opened {
                    status: OpenStatus::Opening,
                    destination: None,
                })
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => return Err(RouteError::Definite(error.to_string())),
        }
    }
    Err(RouteError::Definite(
        "no supported terminal launcher (xdg-terminal-exec or x-terminal-emulator) is installed"
            .into(),
    ))
}

fn focus_destination(host: OpenHost, destination: &str) {
    let command = match host {
        OpenHost::Cmux => {
            let mut command = Command::new("cmux");
            command
                .env("CMUX_QUIET", "1")
                .args(["workspace", "select", destination]);
            command
        }
        OpenHost::Herdr => {
            let mut command = Command::new("herdr");
            command.args(["workspace", "focus", destination]);
            command
        }
        _ => return,
    };
    if let Err(RouteError::Definite(reason) | RouteError::Unknown(reason)) =
        run_host(command, host.label())
    {
        tracing::warn!(%reason, "cannot focus the opened destination");
    }
}

/// Run one host request with a finite timeout and return its stdout.
fn run_host(mut command: Command, host: &str) -> Result<String, RouteError> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| RouteError::Definite(format!("cannot run {host}: {error}")))?;
    let (status, timed_out) =
        crate::os_process::wait_for_exit(&mut child, Some(HOST_TIMEOUT), || {})
            .map_err(|error| RouteError::Unknown(error.to_string()))?;
    if timed_out {
        let _ = child.kill();
        return Err(RouteError::Unknown(format!(
            "{host} did not answer within {}s",
            HOST_TIMEOUT.as_secs()
        )));
    }
    let stdout = drain(child.stdout.take());
    let stderr = drain(child.stderr.take());
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

fn drain(pipe: Option<impl std::io::Read>) -> String {
    pipe.and_then(|pipe| std::io::read_to_string(pipe).ok())
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Command
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, clap::Args)]
pub struct OpenArgs {
    /// Work address, such as loopflow://loopflow/task/LOO-507
    pub url: Option<String>,
    /// Presentation destination; auto prefers Desktop, then this cmux/Herdr host, then a terminal
    #[arg(long, value_enum, default_value = "auto")]
    pub host: OpenHost,
    /// Open every conversation available for you to work with
    #[arg(long, conflicts_with_all = ["url", "session"])]
    pub available: bool,
    /// Open this exact Session rather than its Work's primary conversation
    #[arg(long)]
    pub session: Option<String>,
    /// Show the Task's Changes browser beside its conversation (Desktop)
    #[arg(long)]
    pub diff: bool,
    /// Print the readable link without opening, preparing or creating anything
    #[arg(long)]
    pub link: bool,
    /// Link text format; text uses a clickable span on a terminal
    #[arg(long, value_enum, requires = "link")]
    pub format: Option<LinkFormat>,
    #[arg(long)]
    pub json: bool,
}

/// The Work an invocation names: every available conversation, an address,
/// or the root selectors and checkout.
fn targets(cli: &crate::lf::Cli, args: &OpenArgs) -> Result<(Vec<Resolved>, Vec<String>)> {
    if args.available {
        return available(cli);
    }
    let store = crate::store::read_existing_registry()?;
    let store = store.as_ref();
    let session = args.session.clone();
    let resolved = match &args.url {
        Some(text) => {
            anyhow::ensure!(
                cli.task.is_none() && cli.wave.is_none(),
                "select Work with either a link or --task/--wave, not both"
            );
            let url = WorkUrl::parse(text)?;
            let session = session.or_else(|| url.session.clone());
            let diff = args.diff || url.diff;
            let _cwd = Cwd::enter(&repository_for(store, &url)?)?;
            let (task, wave) = match &url.target {
                WorkTarget::Task(id) => (Some(id.as_str()), None),
                WorkTarget::Wave(name) => (None, Some(name.as_str())),
                WorkTarget::Repository => (None, None),
            };
            let context = work_context(task, wave, session.as_deref())?;
            describe(store, &context, session, diff, None)?
        }
        None => {
            let context = selected_context(cli, session.as_deref())?;
            describe(store, &context, session, args.diff, None)?
        }
    };
    Ok((vec![resolved], Vec::new()))
}

/// Read-only links for `--link` and `--explain`.
pub fn links(cli: &crate::lf::Cli, args: &OpenArgs) -> Result<(Vec<WorkLink>, Vec<String>)> {
    let (targets, unknown) = targets(cli, args)?;
    let route = route(args.host);
    Ok((
        targets
            .iter()
            .map(|target| target.link(route.clone()))
            .collect(),
        unknown,
    ))
}

pub fn explain(cli: &crate::lf::Cli, args: &OpenArgs, json: bool) -> Result<()> {
    let (links, unknown) = links(cli, args)?;
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({"links": links, "unknown": unknown}))?
        );
        return Ok(());
    }
    for link in &links {
        println!("{}  {}", link.label, link.url);
        println!("  Route: {}", link.route.reason);
    }
    print_unknown(&unknown);
    println!("Nothing was executed; no app, checkout or Session was prepared.");
    Ok(())
}

pub fn run(cli: &crate::lf::Cli, args: &OpenArgs) -> Result<()> {
    let json = args.json || cli.json;
    if args.link {
        let (links, unknown) = links(cli, args)?;
        if json {
            println!("{}", serde_json::to_string_pretty(&links)?);
            return Ok(());
        }
        let clickable = std::io::stdout().is_terminal();
        for link in &links {
            match args.format.unwrap_or(LinkFormat::Text) {
                LinkFormat::Markdown => println!("{}", link.markdown),
                LinkFormat::Text => {
                    let label = if clickable {
                        crate::work::link::osc8(&link.label, &link.url)
                    } else {
                        link.label.clone()
                    };
                    println!("{label}  {}", fallback_text(&link.fallback_argv));
                }
            }
        }
        if links.is_empty() && args.available {
            println!("No available conversations.");
        }
        print_unknown(&unknown);
        return Ok(());
    }
    let (targets, unknown) = targets(cli, args)?;
    let route = route(args.host);
    // A person typing in a terminal continues the conversation right here.
    let in_place = targets.len() == 1
        && route.hosts.last() == Some(&OpenHost::Terminal)
        && std::io::stdin().is_terminal()
        && std::io::stdout().is_terminal()
        && crate::journal::agent_caller().is_none();
    if let [target] = targets.as_slice() {
        if target.relation().0 == ConversationRelation::CurrentConversation {
            bail!(
                "{} is this conversation; continue here. Nothing was opened.",
                target.label
            );
        }
    }
    let receipts = open_all(&targets, &route, in_place);
    let report = OpenReport { receipts, unknown };
    if json {
        println!("{}", serde_json::to_string_pretty(&report)?);
    } else {
        if report.receipts.is_empty() && report.unknown.is_empty() {
            println!("No available conversations.");
        }
        for receipt in &report.receipts {
            match receipt.status {
                OpenStatus::Usable => {}
                OpenStatus::Opening => println!(
                    "Opening {} in {}.",
                    receipt.label,
                    receipt.host.map_or("its destination", OpenHost::label)
                ),
                OpenStatus::Elsewhere => println!(
                    "{}: {}",
                    receipt.label,
                    receipt
                        .reason
                        .as_deref()
                        .unwrap_or("open in another client")
                ),
                OpenStatus::Failed => println!(
                    "Could not open {}: {}\n  Run: {}",
                    receipt.label,
                    receipt
                        .reason
                        .as_deref()
                        .unwrap_or("no destination accepted it"),
                    fallback_text(&receipt.fallback_argv)
                ),
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

struct Cwd(PathBuf);

impl Cwd {
    fn enter(path: &Path) -> Result<Self> {
        let previous = std::env::current_dir()?;
        std::env::set_current_dir(path)
            .with_context(|| format!("enter repository {}", path.display()))?;
        Ok(Self(previous))
    }
}

impl Drop for Cwd {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}
