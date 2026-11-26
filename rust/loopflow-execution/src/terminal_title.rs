//! Session names projected into their attached terminal. The held terminal
//! serializes OSC renames between complete provider sequences; direct launches
//! write OSC only before spawning, while they still own terminal output.
//! cmux and tmux have separate control channels for their workspace, tab and
//! window names. A cmux workspace belongs to its Task
//! and follows only the Task title, unless someone renamed it by hand; tabs and
//! tmux windows follow the Session.

use crate::store::session_projection;
use std::collections::{BTreeMap, HashMap};
use std::io::{IsTerminal, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use crate::os_process::wait_for_exit;
use crate::process::SessionAttachment;
use crate::session::LfSession;
use crate::store::{sqlite::SqliteStore, StoreResult};
use loopflow_model::id::TerminalId;
use loopflow_storage::sqlite::StoreRevisions;

fn display_title(name: &str, task: Option<&str>, standing: Option<&str>) -> String {
    let clean: String = name.chars().filter(|ch| !ch.is_control()).collect();
    let Some(task) = task else { return clean };
    let purpose = clean
        .strip_prefix(task)
        .filter(|suffix| suffix.is_empty() || suffix.starts_with(char::is_whitespace))
        .unwrap_or(&clean)
        .split_whitespace()
        .take(3)
        .collect::<Vec<_>>()
        .join(" ");
    let purpose = if purpose.is_empty() {
        "session"
    } else {
        &purpose
    };
    match standing {
        Some(standing) => format!("{task} {purpose} · {standing}"),
        None => format!("{task} {purpose}"),
    }
}

/// The one name a Task's host workspace carries, at creation and after:
/// `<Task title> · <identifier>`. Only a Task title change moves it.
pub fn workspace_name(identifier: &str, title: &str) -> String {
    let title: String = title.chars().filter(|ch| !ch.is_control()).collect();
    let title = title.trim();
    if title.is_empty() {
        identifier.to_string()
    } else {
        format!("{title} · {identifier}")
    }
}

/// Where a Task stands, for terminal titles: `Available` when its
/// conversation is, then the interactive step where its Graph waits, such as
/// `Available · Design`. Both come from their shared readers; nothing new is judged.
/// Also returns the earliest Unix timestamp when `Available` can turn true with no write.
fn standing(
    store: &SqliteStore,
    task: &loopflow_model::durable::TaskId,
    conversation: Option<&str>,
) -> StoreResult<(Option<String>, Option<i64>)> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let (available, next_deadline) = match conversation {
        Some(id) => {
            let (summary, deadline) =
                session_projection::session_summary_with_deadline(store, id, now)?;
            (summary.is_some_and(|summary| summary.available), deadline)
        }
        None => (false, None),
    };
    let graph = crate::store::task_work::task_graph(store, task)?;
    let node = graph
        .as_ref()
        .and_then(|graph| graph.waiting_step())
        .map(|step| step.name.as_str());
    let mut parts = Vec::new();
    if available {
        parts.push("Available".to_string());
    }
    if let Some(node) = node {
        let mut letters = node.chars();
        parts.extend(
            letters
                .next()
                .map(|first| first.to_uppercase().chain(letters).collect::<String>()),
        );
    }
    Ok((
        (!parts.is_empty()).then(|| parts.join(" · ")),
        next_deadline,
    ))
}

/// A host whose own names for this terminal Loopflow keeps current.
#[derive(Debug)]
enum HostNames {
    Cmux { workspace: String, surface: String },
    Tmux { pane: String },
}

impl HostNames {
    fn current() -> Option<Self> {
        let var = |name| std::env::var(name).ok().filter(|value| !value.is_empty());
        if let (Some(workspace), Some(surface)) = (var("CMUX_WORKSPACE_ID"), var("CMUX_SURFACE_ID"))
        {
            return Some(Self::Cmux { workspace, surface });
        }
        var("TMUX")
            .and(var("TMUX_PANE"))
            .map(|pane| Self::Tmux { pane })
    }

    /// The cmux workspace's name as shown now; `None` when it cannot be read.
    fn shown_workspace(&self) -> Option<String> {
        let Self::Cmux { workspace, .. } = self else {
            return None;
        };
        let listing = host_output("cmux", &["workspace", "list", "--json"])
            .inspect_err(|error| tracing::debug!(%error, "cannot read cmux workspace names"))
            .ok()?;
        let listing: serde_json::Value = serde_json::from_str(&listing).ok()?;
        listing["workspaces"]
            .as_array()?
            .iter()
            .find(|entry| entry["id"].as_str() == Some(workspace))?["title"]
            .as_str()
            .map(str::to_string)
    }

    fn rename(&self, rename: &HostRename) -> std::io::Result<()> {
        match (self, rename) {
            (Self::Cmux { workspace, .. }, HostRename::Workspace(name)) => host_command(
                "cmux",
                &[
                    "rename-workspace",
                    "--workspace",
                    workspace,
                    "--",
                    &name.name,
                ],
            ),
            (Self::Cmux { workspace, surface }, HostRename::Terminal(name)) => host_command(
                "cmux",
                &[
                    "rename-tab",
                    "--workspace",
                    workspace,
                    "--surface",
                    surface,
                    "--",
                    name,
                ],
            ),
            // A tmux window is the Session's, like a cmux tab.
            (Self::Tmux { .. }, HostRename::Workspace(_)) => Ok(()),
            (Self::Tmux { pane }, HostRename::Terminal(name)) => {
                host_command("tmux", &["rename-window", "-t", pane, "--", name])
            }
        }
    }
}

/// The names one attached terminal projects onto its host.
#[derive(Debug, PartialEq)]
struct Names {
    /// The Task's workspace name; taskless Sessions leave the workspace alone.
    workspace: Option<WorkspaceName>,
    /// The Session's terminal title, for the tab or window.
    terminal: String,
}

#[derive(Debug, PartialEq)]
struct WorkspaceName {
    identifier: String,
    name: String,
}

impl WorkspaceName {
    fn new(identifier: &str, title: &str) -> Self {
        Self {
            identifier: identifier.to_string(),
            name: workspace_name(identifier, title),
        }
    }

    /// Whether `shown` is a name lf wrote, so lf may replace it: the name it
    /// last published, any `<title> · <identifier>` name, or an earlier lf's
    /// identifier-first name. Anything else someone chose in cmux by hand.
    fn replaces(&self, shown: &str, published: Option<&str>) -> bool {
        let identifier = self.identifier.as_str();
        shown != self.name
            && (Some(shown) == published
                || shown == identifier
                || shown
                    .strip_suffix(identifier)
                    .is_some_and(|title| title.ends_with(" · "))
                || shown
                    .strip_prefix(identifier)
                    .is_some_and(|rest| rest.starts_with(' ')))
    }
}

#[derive(Debug, PartialEq)]
enum HostRename<'a> {
    Workspace(&'a WorkspaceName),
    Terminal(&'a str),
}

/// Renames that bring the host from the `previous` names lf published to `current`.
/// lf writes only on its own changes; a workspace rename is then still gated
/// on the name cmux shows, so a hand-edited name stays.
fn renames_due<'a>(previous: Option<&Names>, current: &'a Names) -> Vec<HostRename<'a>> {
    let mut due = Vec::new();
    if let Some(workspace) = &current.workspace {
        if previous.and_then(|names| names.workspace.as_ref()) != Some(workspace) {
            due.push(HostRename::Workspace(workspace));
        }
    }
    if previous.map(|names| &names.terminal) != Some(&current.terminal) {
        due.push(HostRename::Terminal(&current.terminal));
    }
    due
}

#[derive(Debug)]
pub(crate) struct TerminalTitle {
    /// Borrowed from the pool per check: a pinned connection would keep a replaced
    /// database's unlinked inode.
    database: PathBuf,
    session: String,
    attachment: Option<SessionAttachment>,
    names: Names,
    provider_name: String,
    host: Option<HostNames>,
    checked: Instant,
    seen_revisions: Option<TitleRevisions>,
    next_deadline: Option<i64>,
}

#[derive(Debug, PartialEq)]
enum ViewerPoll {
    Unchanged,
    Read(Names),
    Detached,
}

impl TerminalTitle {
    pub fn prepare(
        environment: &BTreeMap<String, String>,
        harness: loopflow_harness::HarnessKind,
        command: &mut Command,
    ) -> Option<Self> {
        if !std::io::stderr().is_terminal() {
            return None;
        }
        let input = environment.get(crate::session_record::CAPTURE_KEY_ENV)?;
        let mut title = match Self::load(input) {
            Ok(Some(mut title)) => {
                // Under a retained execution, each viewer names its own host window.
                if crate::transport::current().is_some() {
                    title.host = None;
                }
                title
            }
            Ok(None) => return None,
            Err(error) => {
                tracing::warn!(%error, "cannot read terminal Session name");
                return None;
            }
        };
        // Called before adding any subcommand, positional input or `--` boundary.
        command.args(harness.harness().title_args(&title.provider_name));
        if let Err(error) = write!(
            std::io::stderr().lock(),
            "\x1b]0;{}\x07",
            title.names.terminal
        ) {
            tracing::warn!(%error, "cannot set terminal Session name");
        }
        title.publish(None);
        // Only a host control channel can carry renames while the provider owns terminal output.
        title.host.is_some().then_some(title)
    }

    fn viewing(held: &loopflow_model::id::TerminalId) -> Option<Self> {
        let load = || -> anyhow::Result<Option<Self>> {
            let database = crate::store::database_path_from_env()?;
            let store =
                SqliteStore::open_read_only(&database, crate::store::token_key::encryption_key)?;
            let Some(id) = store.terminal_session(held)? else {
                return Ok(None);
            };
            let Some(session) = store.session(&id)? else {
                return Ok(None);
            };
            Self::from_session(database, &store, session).map(Some)
        };
        let mut title = load()
            .inspect_err(|error| tracing::debug!(%error, "cannot read viewed Session name"))
            .ok()
            .flatten()?;
        title.publish(None);
        title.host.is_some().then_some(title)
    }

    fn load(input: &str) -> anyhow::Result<Option<Self>> {
        let database = crate::store::database_path_from_env()?;
        let store =
            SqliteStore::open_read_only(&database, crate::store::token_key::encryption_key)?;
        let Some(session) = store.session_for_artifact(input)? else {
            return Ok(None);
        };
        Self::from_session(database, &store, session).map(Some)
    }

    fn from_session(
        database: PathBuf,
        store: &SqliteStore,
        session: LfSession,
    ) -> anyhow::Result<Self> {
        let seen_revisions = Some(TitleRevisions::of(store.revisions()?));
        let name = session_title(store, &session)?;
        Ok(Self {
            attachment: store.session_attachment(&session.id)?,
            provider_name: name.provider,
            names: name.names,
            next_deadline: name.next_deadline,
            seen_revisions,
            database,
            session: session.id.to_string(),
            host: HostNames::current(),
            checked: Instant::now(),
        })
    }

    pub fn refresh(&mut self) {
        if self.host.is_none() || self.checked.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.checked = Instant::now();
        match self.poll_names() {
            Ok(ViewerPoll::Unchanged) => {}
            Ok(ViewerPoll::Read(names)) => {
                let previous = std::mem::replace(&mut self.names, names);
                self.publish(Some(&previous));
            }
            // A replaced or released attachment must stop changing the old terminal's names.
            Ok(ViewerPoll::Detached) => self.host = None,
            // A busy or replaced store is read again on the next check: the seen
            // revisions only advance after a read succeeds.
            Err(error) => tracing::debug!(%error, "cannot refresh terminal Session name"),
        }
    }

    fn poll_names(&mut self) -> StoreResult<ViewerPoll> {
        let store =
            SqliteStore::open_read_only(&self.database, crate::store::token_key::encryption_key)?;
        let current_revisions = TitleRevisions::of(store.revisions()?);
        let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();
        let revision_moved = self.seen_revisions != Some(current_revisions);
        let deadline_due = self
            .next_deadline
            .is_some_and(|deadline| now_unix >= deadline);
        if !revision_moved && !deadline_due {
            return Ok(ViewerPoll::Unchanged);
        }
        match self.read_names_with_deadline(&store)? {
            Some((names, next_deadline)) => {
                self.seen_revisions = Some(current_revisions);
                self.next_deadline = next_deadline;
                Ok(ViewerPoll::Read(names))
            }
            None => Ok(ViewerPoll::Detached),
        }
    }

    fn read_names_with_deadline(
        &self,
        store: &SqliteStore,
    ) -> StoreResult<Option<(Names, Option<i64>)>> {
        if store.session_attachment(&self.session)? != self.attachment {
            return Ok(None);
        }
        store
            .session(&self.session)?
            .map(|session| {
                session_title(store, &session).map(|title| (title.names, title.next_deadline))
            })
            .transpose()
    }

    /// A failed rename detaches the host, so `previous` is always what it shows.
    fn publish(&mut self, previous: Option<&Names>) {
        let Some(host) = &self.host else {
            return;
        };
        let published = previous
            .and_then(|names| names.workspace.as_ref())
            .map(|workspace| workspace.name.as_str());
        for rename in renames_due(previous, &self.names) {
            if let HostRename::Workspace(workspace) = rename {
                // An unreadable name might be someone's; leave it.
                let shown = host.shown_workspace();
                if !shown.is_some_and(|shown| workspace.replaces(&shown, published)) {
                    continue;
                }
            }
            if let Err(error) = host.rename(&rename) {
                tracing::warn!(%error, "terminal host title update failed; Session continues");
                self.host = None;
                return;
            }
        }
    }
}

impl Drop for TerminalTitle {
    /// A tmux window names its running program again once the Session leaves it.
    fn drop(&mut self) {
        if let Some(HostNames::Tmux { pane }) = &self.host {
            let _ = host_command(
                "tmux",
                &["set-option", "-wu", "-t", pane, "automatic-rename"],
            );
        }
    }
}

/// A viewer's host window, named for the Session it views. The execution may
/// link its Session after the viewer attaches, so the viewer keeps looking.
#[derive(Debug)]
pub(crate) struct ViewerTitle {
    held: loopflow_model::id::TerminalId,
    title: Option<TerminalTitle>,
    /// Until when to look; executions without a Session never get one.
    looking: Option<Instant>,
    checked: Option<Instant>,
}

/// The three store domains a terminal title reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct TitleRevisions {
    sessions: i64,
    planning: i64,
    graphs: i64,
}

impl TitleRevisions {
    fn of(revisions: StoreRevisions) -> Self {
        Self {
            sessions: revisions.sessions,
            planning: revisions.planning,
            graphs: revisions.graphs,
        }
    }
}

/// Minimum pause between revision- or deadline-driven title sweeps.
const SWEEP_THROTTLE: Duration = Duration::from_millis(500);
/// Initial retry wait after a failed sweep; doubles up to [`RETRY_CAP`].
const RETRY_BASE: Duration = Duration::from_secs(1);
const RETRY_CAP: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy)]
struct ReaderConfig {
    throttle: Duration,
    retry_base: Duration,
    retry_cap: Duration,
}

impl ReaderConfig {
    fn production() -> Self {
        Self {
            throttle: SWEEP_THROTTLE,
            retry_base: RETRY_BASE,
            retry_cap: RETRY_CAP,
        }
    }

    fn retry_delay(self, failures: u32) -> Duration {
        self.retry_base
            .saturating_mul(1 << failures.saturating_sub(1).min(16))
            .min(self.retry_cap)
    }
}

/// One worker reading Session names for every viewed terminal a host serves;
/// store reads never run while the store is quiet and never hold up provider
/// output or terminal input. It ends once its handle and every watched
/// `OutputTitle` are gone.
#[derive(Debug, Clone)]
pub(crate) struct TitleReader {
    messages: mpsc::Sender<TitleMessage>,
    next: Arc<AtomicU64>,
}

#[derive(Debug)]
struct TitleWatch {
    key: u64,
    messages: mpsc::Sender<TitleMessage>,
}

impl Drop for TitleWatch {
    fn drop(&mut self) {
        let _ = self.messages.send(TitleMessage::Unwatch(self.key));
    }
}

#[derive(Debug)]
struct WatchedTerminal {
    held: TerminalId,
    waker: UnixStream,
    answer: mpsc::Sender<String>,
    last_sent: Option<String>,
    next_deadline: Option<i64>,
    needs_initial: bool,
}

#[derive(Debug)]
enum TitleMessage {
    Watch(u64, Box<WatchedTerminal>, mpsc::Sender<TitleMessage>),
    Unwatch(u64),
    Revisions(Option<TitleRevisions>),
}

impl TitleReader {
    pub fn spawn() -> Self {
        #[cfg(any(test, feature = "test-support"))]
        let path = crate::journal::test_ledger_path()
            .map(Ok)
            .unwrap_or_else(crate::store::database_path_from_env);
        #[cfg(not(any(test, feature = "test-support")))]
        let path = crate::store::database_path_from_env();
        let (messages, received) = mpsc::channel();
        let _ = std::thread::Builder::new()
            .name("title-reader".into())
            .spawn(move || {
                let Ok(path) = path else { return };
                run_title_reader(path, received, ReaderConfig::production(), output_title);
            });
        Self {
            messages,
            next: Arc::new(AtomicU64::new(0)),
        }
    }

    fn watch(
        &self,
        held: TerminalId,
        waker: UnixStream,
        answer: mpsc::Sender<String>,
    ) -> TitleWatch {
        let key = self.next.fetch_add(1, Ordering::Relaxed);
        let watched = WatchedTerminal {
            held,
            waker,
            answer,
            last_sent: None,
            next_deadline: None,
            needs_initial: true,
        };
        let _ = self.messages.send(TitleMessage::Watch(
            key,
            Box::new(watched),
            self.messages.clone(),
        ));
        TitleWatch {
            key,
            messages: self.messages.clone(),
        }
    }
}

fn duration_until_unix(deadline_secs: i64) -> Duration {
    if deadline_secs <= 0 {
        return Duration::ZERO;
    }
    let target = UNIX_EPOCH + Duration::from_secs(deadline_secs as u64);
    target
        .duration_since(SystemTime::now())
        .unwrap_or(Duration::ZERO)
}

fn run_title_reader<F>(
    path: PathBuf,
    received: mpsc::Receiver<TitleMessage>,
    config: ReaderConfig,
    mut read_title: F,
) where
    F: FnMut(&SqliteStore, &TerminalId) -> anyhow::Result<Option<OutputTitleReading>>,
{
    let mut watches: HashMap<u64, WatchedTerminal> = HashMap::new();
    let mut subscription: Option<crate::read_stream::RevisionFollow> = None;
    let mut seen_revisions: Option<TitleRevisions> = None;
    let mut dirty = false;
    let mut last_sweep: Option<Instant> = None;
    let mut failures: u32 = 0;
    let mut retry_at: Option<Instant> = None;

    loop {
        let now_instant = Instant::now();
        let now_unix = time::OffsetDateTime::now_utc().unix_timestamp();

        let mut sweep_now = false;
        let mut wait = None;
        if !watches.is_empty() {
            if let Some(at) = retry_at {
                if now_instant < at {
                    wait = Some(at.saturating_duration_since(now_instant));
                } else {
                    sweep_now = true;
                }
            } else {
                let any_initial = watches.values().any(|w| w.needs_initial);
                let any_deadline_due = watches
                    .values()
                    .any(|w| w.next_deadline.is_some_and(|deadline| now_unix >= deadline));
                if any_initial || dirty || any_deadline_due {
                    let throttled = !any_initial
                        && last_sweep.is_some_and(|last| now_instant < last + config.throttle);
                    if throttled {
                        let until = last_sweep.expect("checked above") + config.throttle;
                        wait = Some(until.saturating_duration_since(now_instant));
                    } else {
                        sweep_now = true;
                    }
                } else if let Some(earliest) =
                    watches.values().filter_map(|w| w.next_deadline).min()
                {
                    let remaining = duration_until_unix(earliest);
                    // Avoid spinning if SystemTime and unix_timestamp straddle a second edge.
                    wait = Some(remaining.max(Duration::from_millis(1)));
                }
            }
        }

        if !sweep_now {
            let message = match wait {
                Some(timeout) => received.recv_timeout(timeout),
                None => received
                    .recv()
                    .map_err(|_| mpsc::RecvTimeoutError::Disconnected),
            };
            match message {
                Ok(message) => {
                    apply_title_message(
                        message,
                        &path,
                        &mut watches,
                        &mut subscription,
                        &mut seen_revisions,
                        &mut dirty,
                        &mut last_sweep,
                        &mut failures,
                        &mut retry_at,
                    );
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return,
            }
            for message in received.try_iter() {
                apply_title_message(
                    message,
                    &path,
                    &mut watches,
                    &mut subscription,
                    &mut seen_revisions,
                    &mut dirty,
                    &mut last_sweep,
                    &mut failures,
                    &mut retry_at,
                );
            }
            continue;
        }

        // Store file not created yet: the observer retries and will notify us.
        if !path.exists() {
            dirty = false;
            retry_at = None;
            failures = 0;
            for watch in watches.values_mut() {
                watch.needs_initial = false;
            }
            continue;
        }

        let sweep_result = (|| -> anyhow::Result<()> {
            let store =
                SqliteStore::open_read_only(&path, crate::store::token_key::encryption_key)?;
            let current_revisions = TitleRevisions::of(store.revisions()?);
            let sweep_all = dirty || failures > 0 || seen_revisions != Some(current_revisions);
            let sweep_unix = time::OffsetDateTime::now_utc().unix_timestamp();
            for watch in watches.values_mut() {
                let due = sweep_all
                    || watch.needs_initial
                    || watch
                        .next_deadline
                        .is_some_and(|deadline| sweep_unix >= deadline);
                if !due {
                    continue;
                }
                let reading = read_title(&store, &watch.held)?;
                watch.needs_initial = false;
                match reading {
                    Some(OutputTitleReading {
                        terminal,
                        next_deadline,
                    }) => {
                        watch.next_deadline = next_deadline;
                        if watch.last_sent.as_deref() != Some(terminal.as_str()) {
                            watch.last_sent = Some(terminal.clone());
                            let _ = watch.answer.send(terminal);
                            let _ = (&watch.waker).write_all(&[1]);
                        }
                    }
                    None => {
                        watch.next_deadline = None;
                    }
                }
            }
            seen_revisions = Some(current_revisions);
            Ok(())
        })();

        match sweep_result {
            Ok(()) => {
                dirty = false;
                failures = 0;
                retry_at = None;
                last_sweep = Some(Instant::now());
            }
            Err(error) => {
                tracing::debug!(%error, "cannot read output Session name");
                failures = failures.saturating_add(1);
                retry_at = Some(Instant::now() + config.retry_delay(failures));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_title_message(
    message: TitleMessage,
    path: &std::path::Path,
    watches: &mut HashMap<u64, WatchedTerminal>,
    subscription: &mut Option<crate::read_stream::RevisionFollow>,
    seen_revisions: &mut Option<TitleRevisions>,
    dirty: &mut bool,
    last_sweep: &mut Option<Instant>,
    failures: &mut u32,
    retry_at: &mut Option<Instant>,
) {
    match message {
        TitleMessage::Watch(key, watched, wake_tx) => {
            let was_empty = watches.is_empty();
            watches.insert(key, *watched);
            if was_empty && subscription.is_none() {
                // The reader compares against the revisions its last sweep read.
                let follow = crate::read_stream::follow_revisions(path, move |next| {
                    let _ = wake_tx.send(TitleMessage::Revisions(next.map(TitleRevisions::of)));
                });
                *subscription = Some(follow);
            }
        }
        TitleMessage::Unwatch(key) => {
            watches.remove(&key);
            if watches.is_empty() {
                *subscription = None;
                *seen_revisions = None;
                *dirty = false;
                *last_sweep = None;
                *failures = 0;
                *retry_at = None;
            }
        }
        TitleMessage::Revisions(next) => {
            if next != *seen_revisions {
                *dirty = true;
            }
        }
    }
}

/// The held terminal's Session name, sent through its serialized terminal stream.
/// Host-specific names remain with each viewer, including remote viewers.
#[derive(Debug)]
pub(crate) struct OutputTitle {
    held: TerminalId,
    waker: UnixStream,
    reader: TitleReader,
    watch: Option<TitleWatch>,
    answer: mpsc::Sender<String>,
    names: mpsc::Receiver<String>,
}

impl OutputTitle {
    pub fn new(held: TerminalId, waker: UnixStream, reader: TitleReader) -> Self {
        let (answer, names) = mpsc::channel();
        Self {
            held,
            waker,
            reader,
            watch: None,
            answer,
            names,
        }
    }

    pub fn set_viewed(&mut self, viewed: bool) {
        match (viewed, self.watch.is_some()) {
            (true, false) => {
                if let Ok(waker) = self.waker.try_clone() {
                    self.watch = Some(self.reader.watch(
                        self.held.clone(),
                        waker,
                        self.answer.clone(),
                    ));
                }
            }
            (false, true) => {
                self.watch = None;
            }
            _ => {}
        }
    }

    pub fn tick(&mut self) -> Option<String> {
        let mut latest = None;
        while let Ok(name) = self.names.try_recv() {
            latest = Some(name);
        }
        latest
    }
}

#[derive(Debug, PartialEq, Eq)]
struct OutputTitleReading {
    terminal: String,
    next_deadline: Option<i64>,
}

fn output_title(
    store: &SqliteStore,
    held: &TerminalId,
) -> anyhow::Result<Option<OutputTitleReading>> {
    let Some(id) = store.terminal_session(held)? else {
        return Ok(None);
    };
    let Some(session) = store.session(&id)? else {
        return Ok(None);
    };
    // A departing attachment no longer owns publication to this terminal.
    if store
        .session_attachment(&id)?
        .is_none_or(|attachment| attachment.lf_process_id.is_none())
    {
        return Ok(None);
    }
    let title = session_title(store, &session)?;
    Ok(Some(OutputTitleReading {
        terminal: title.names.terminal,
        next_deadline: title.next_deadline,
    }))
}

impl ViewerTitle {
    pub fn new(held: &loopflow_model::id::TerminalId) -> Self {
        Self {
            held: held.clone(),
            title: None,
            looking: HostNames::current().map(|_| Instant::now() + Duration::from_secs(10)),
            checked: None,
        }
    }

    pub fn tick(&mut self) {
        if let Some(title) = &mut self.title {
            title.refresh();
            return;
        }
        if self.looking.is_none_or(|until| Instant::now() > until)
            || self
                .checked
                .is_some_and(|checked| checked.elapsed() < Duration::from_millis(500))
        {
            return;
        }
        self.checked = Some(Instant::now());
        self.title = TerminalTitle::viewing(&self.held);
    }
}

struct SessionTitle {
    /// The Session's own name, Task first.
    provider: String,
    /// That name and where its Task stands now, with the Task's workspace name.
    names: Names,
    /// Earliest Unix timestamp when `Available` may turn true with no write.
    next_deadline: Option<i64>,
}

fn session_title(store: &SqliteStore, session: &LfSession) -> StoreResult<SessionTitle> {
    let task = session
        .task_id
        .as_ref()
        .map(|id| store.task(id))
        .transpose()?
        .flatten();
    let (standing, next_deadline) = match task.as_ref() {
        Some(task) => standing(store, &task.id, Some(&session.id))?,
        None => (None, None),
    };
    let identifier = task.as_ref().map(|task| task.identifier.as_str());
    Ok(SessionTitle {
        provider: display_title(&session.title, identifier, None),
        names: Names {
            workspace: task
                .as_ref()
                .map(|task| WorkspaceName::new(&task.identifier, &task.title)),
            terminal: display_title(&session.title, identifier, standing.as_deref()),
        },
        next_deadline,
    })
}

fn host_output(program: &str, args: &[&str]) -> std::io::Result<String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let mut stdout = child.stdout.take().expect("stdout is piped");
    // Drain while waiting: a long listing would otherwise fill the pipe.
    let reader = std::thread::spawn(move || {
        let mut text = String::new();
        std::io::Read::read_to_string(&mut stdout, &mut text).map(|_| text)
    });
    let (status, timed_out) = wait_for_exit(&mut child, Some(Duration::from_secs(1)), || {})?;
    let text = reader.join().expect("reader thread does not panic")?;
    if timed_out {
        Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("{program} read timed out"),
        ))
    } else if status.success() {
        Ok(text)
    } else {
        Err(std::io::Error::other(format!("{program} exited {status}")))
    }
}

fn host_command(program: &str, args: &[&str]) -> std::io::Result<()> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let (status, timed_out) = wait_for_exit(&mut child, Some(Duration::from_secs(1)), || {})?;
    if timed_out {
        Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            format!("{program} title update timed out"),
        ))
    } else if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("{program} exited {status}")))
    }
}

#[cfg(test)]
mod tests {
    use crate::process::attachment;

    use super::{
        display_title, renames_due, workspace_name, HostRename, Names, TerminalTitle, WorkspaceName,
    };

    #[test]
    fn replaced_attachment_stops_observing_session_names() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = crate::store::opening::open_ephemeral(&path).unwrap();
        let session =
            store.test_session("conversation", &crate::session_record::new_artifact_key());
        let process = loopflow_model::id::LfProcessId::new();
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [process.as_str()],
            )
            .unwrap();
        let attachment =
            attachment::claim_session_attachment(&store, &session.id, None, &process).unwrap();
        let terminal = loopflow_model::id::TerminalId::new();
        store.create_terminal(&terminal, "build").unwrap();
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET held_terminal_id=?1 WHERE id=?2",
                rusqlite::params![terminal, session.id],
            )
            .unwrap();
        let title = TerminalTitle {
            database: path.clone(),
            session: session.id.to_string(),
            attachment: Some(attachment.clone()),
            names: Names {
                workspace: None,
                terminal: session.title.clone(),
            },
            provider_name: session.title,
            host: None,
            checked: std::time::Instant::now(),
            seen_revisions: None,
            next_deadline: None,
        };
        crate::store::sessions::rename_session(
            &store,
            &title.session,
            "Release notes",
            crate::session::TitleSource::Human,
        )
        .unwrap();
        assert_eq!(
            title
                .read_names_with_deadline(&store)
                .unwrap()
                .map(|(names, _)| names.terminal),
            Some("Release notes".to_string())
        );
        assert_eq!(
            super::output_title(&store, &terminal)
                .unwrap()
                .map(|reading| reading.terminal),
            Some("Release notes".into())
        );
        attachment::claim_session_attachment(&store, &title.session, Some(&attachment), &process)
            .unwrap();
        assert_eq!(title.read_names_with_deadline(&store).unwrap(), None);
    }

    #[test]
    fn task_first_and_short_purpose_without_duplicate_identifier() {
        assert_eq!(display_title("demo", Some("LOO-412"), None), "LOO-412 demo");
        assert_eq!(
            display_title(
                "LOO-412 plan store migration details",
                Some("LOO-412"),
                None
            ),
            "LOO-412 plan store migration"
        );
        assert_eq!(
            display_title("LOO-4120 demo", Some("LOO-412"), None),
            "LOO-412 LOO-4120 demo"
        );
        assert_eq!(
            display_title("Release notes", Some("LOO-412"), Some("Available · Design")),
            "LOO-412 Release notes · Available · Design"
        );
    }

    #[test]
    fn taskless_name_is_preserved_without_terminal_controls() {
        assert_eq!(
            display_title("Hérdr and machine work", None, Some("Available")),
            "Hérdr and machine work"
        );
        assert_eq!(display_title("demo\x1b\x07\u{009c}", None, None), "demo");
    }

    #[test]
    fn task_workspace_follows_only_the_task_title() {
        let names = |title: &str, terminal: &str| Names {
            workspace: Some(WorkspaceName::new("LOO-609", title)),
            terminal: terminal.to_string(),
        };
        let published = names("Task-named cmux", "LOO-609 plan names");
        assert_eq!(
            renames_due(None, &published),
            [
                HostRename::Workspace(published.workspace.as_ref().unwrap()),
                HostRename::Terminal("LOO-609 plan names"),
            ]
        );
        let renamed = names("Task-named cmux", "LOO-609 review names · Available");
        assert_eq!(
            renames_due(Some(&published), &renamed),
            [HostRename::Terminal("LOO-609 review names · Available")]
        );
        let retitled = names("Workspaces named after Tasks", "LOO-609 plan names");
        assert_eq!(
            renames_due(Some(&published), &retitled),
            [HostRename::Workspace(retitled.workspace.as_ref().unwrap())]
        );
        let taskless = Names {
            workspace: None,
            terminal: "demo".to_string(),
        };
        assert_eq!(renames_due(None, &taskless), [HostRename::Terminal("demo")]);
    }

    #[test]
    fn workspace_rename_replaces_only_names_lf_wrote() {
        let target = WorkspaceName::new("LOO-609", "Workspaces named after Tasks");
        let published = Some("Task-named cmux · LOO-609");
        assert!(target.replaces("Task-named cmux · LOO-609", published));
        assert!(target.replaces("Older title · LOO-609", None));
        assert!(target.replaces("LOO-609", None));
        assert!(target.replaces("LOO-609 · Task-named cmux · Explore", None));
        assert!(target.replaces("LOO-609 cmux titles", None));
        assert!(!target.replaces("Workspaces named after Tasks · LOO-609", published));
        assert!(!target.replaces("my cmux work", published));
        assert!(!target.replaces("LOO-6090 · Other Task", None));
        assert!(!target.replaces("NotLOO-609", None));
    }

    #[test]
    fn workspace_name_is_title_then_identifier_without_controls() {
        assert_eq!(
            workspace_name("LOO-609", "Name \x1bworkspaces"),
            "Name workspaces · LOO-609"
        );
        assert_eq!(workspace_name("LOO-609", " \x07"), "LOO-609");
    }

    struct TitleFixture {
        _home: tempfile::TempDir,
        path: std::path::PathBuf,
        store: crate::store::sqlite::SqliteStore,
        task_id: loopflow_model::durable::TaskId,
        task_identifier: String,
        session_id: String,
        attachment: crate::process::SessionAttachment,
        terminal: loopflow_model::id::TerminalId,
    }

    impl TitleFixture {
        fn new() -> Self {
            let home = tempfile::tempdir().unwrap();
            let path = home.path().join("loopflow.db");
            let store = crate::store::opening::open_ephemeral(&path).unwrap();
            let repo = loopflow_storage::test_fixture::repository(&store, "/repo");
            let task = store
                .create_task(&loopflow_storage::work::task::NewTask {
                    parent_task_id: None,
                    repository_id: repo,
                    id: loopflow_model::durable::TaskId::new(),
                    project_id: None,
                    title: "Title fixture".into(),
                    description: "Brief".into(),
                    due_date: None,
                })
                .unwrap();
            let session =
                store.test_session("work-session", &crate::session_record::new_artifact_key());
            crate::store::sessions::rename_session(
                &store,
                &session.id,
                "work session",
                crate::session::TitleSource::Human,
            )
            .unwrap();
            store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE agent_sessions SET task_id=?1 WHERE id=?2",
                    rusqlite::params![task.id.as_str(), session.id.as_str()],
                )
                .unwrap();
            let process = store.test_lf_process();
            let attachment =
                attachment::claim_session_attachment(&store, &session.id, None, &process).unwrap();
            let pid = std::process::id();
            let started = crate::process::os_process::process_started_at(pid)
                .unwrap()
                .unwrap();
            store
                .record_agent_process_identity(&session.id, &attachment, pid, started)
                .unwrap();
            let terminal = loopflow_model::id::TerminalId::new();
            store.create_terminal(&terminal, "build").unwrap();
            store
                .bind_session_terminal(&session.id, &attachment, &terminal)
                .unwrap();
            Self {
                _home: home,
                path,
                store,
                task_id: task.id,
                task_identifier: task.identifier,
                session_id: session.id.to_string(),
                attachment,
                terminal,
            }
        }

        fn record_activity(&self, observed_at: i64, yielded: bool) {
            self.store
                .record_session_activity(
                    &self.session_id,
                    &self.attachment,
                    &crate::session::SessionActivity {
                        preview: None,
                        observed_at,
                        open_tools: 0,
                        pending_input: 0,
                        yielded,
                    },
                )
                .unwrap();
        }

        fn put_at_design_step(&self) {
            let graph = crate::task_graph::CapturedGraph {
                name: "code".into(),
                steps: vec![crate::graph::ConcreteStep::Skill(
                    crate::graph::ConcreteSkill {
                        skill: crate::graph::Skill::named("design"),
                        id: None,
                        interactive: true,
                        interactive_name: None,
                        returns: None,
                        sources: Vec::new(),
                    },
                )],
            };
            crate::store::task_work::take_up_task_graph(&self.store, &self.task_id, &graph)
                .unwrap();
            self.store
                .set_task_graph_step(&self.task_id, "design")
                .unwrap();
        }
    }

    fn spawn_test_reader<F>(
        path: std::path::PathBuf,
        config: super::ReaderConfig,
        read: F,
    ) -> (super::TitleReader, std::thread::JoinHandle<()>)
    where
        F: FnMut(
                &crate::store::sqlite::SqliteStore,
                &loopflow_model::id::TerminalId,
            ) -> anyhow::Result<Option<super::OutputTitleReading>>
            + Send
            + 'static,
    {
        let (messages, received) = std::sync::mpsc::channel();
        let handle = std::thread::Builder::new()
            .name("title-reader-test".into())
            .spawn(move || super::run_title_reader(path, received, config, read))
            .unwrap();
        (
            super::TitleReader {
                messages,
                next: std::sync::Arc::new(std::sync::atomic::AtomicU64::new(0)),
            },
            handle,
        )
    }

    fn recv_title(
        output: &mut super::OutputTitle,
        wake: &mut std::os::unix::net::UnixStream,
        timeout: std::time::Duration,
    ) -> Option<String> {
        let deadline = std::time::Instant::now() + timeout;
        wake.set_nonblocking(true).unwrap();
        let mut buf = [0u8; 64];
        while std::time::Instant::now() < deadline {
            let _ = std::io::Read::read(wake, &mut buf);
            if let Some(name) = output.tick() {
                return Some(name);
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        output.tick()
    }

    #[test]
    fn reader_stays_quiet_when_idle_and_follows_rename_step_and_yield() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        let fixture = TitleFixture::new();
        let reads = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&reads);
        let config = super::ReaderConfig {
            throttle: Duration::from_millis(40),
            retry_base: Duration::from_millis(40),
            retry_cap: Duration::from_millis(200),
        };
        let (reader, handle) = spawn_test_reader(fixture.path.clone(), config, move |store, id| {
            counted.fetch_add(1, Ordering::SeqCst);
            super::output_title(store, id)
        });
        let (mut wake, waker) = std::os::unix::net::UnixStream::pair().unwrap();
        let mut output = super::OutputTitle::new(fixture.terminal.clone(), waker, reader);
        output.set_viewed(true);

        let initial = recv_title(&mut output, &mut wake, Duration::from_secs(3)).unwrap();
        assert_eq!(initial, format!("{} work session", fixture.task_identifier));
        let after_initial = reads.load(Ordering::SeqCst);
        assert_eq!(after_initial, 1);

        // Idle across several throttle periods: zero additional reads.
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(reads.load(Ordering::SeqCst), after_initial);

        // Unrelated domain (`processes`) moves: zero additional reads.
        fixture.store.test_lf_process();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(reads.load(Ordering::SeqCst), after_initial);

        // Rename the Session: receives the new name.
        crate::store::sessions::rename_session(
            &fixture.store,
            &fixture.session_id,
            "Fresh title",
            crate::session::TitleSource::Human,
        )
        .unwrap();
        let renamed = recv_title(&mut output, &mut wake, Duration::from_secs(3)).unwrap();
        assert_eq!(renamed, format!("{} Fresh title", fixture.task_identifier));

        // Put the Task at interactive step `design`: receives `· Design`.
        fixture.put_at_design_step();
        let stepped = recv_title(&mut output, &mut wake, Duration::from_secs(3)).unwrap();
        assert_eq!(
            stepped,
            format!("{} Fresh title · Design", fixture.task_identifier)
        );

        // Record a yield: receives `· Available · Design`.
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        fixture.record_activity(now, true);
        let yielded = recv_title(&mut output, &mut wake, Duration::from_secs(3)).unwrap();
        assert_eq!(
            yielded,
            format!(
                "{} Fresh title · Available · Design",
                fixture.task_identifier
            )
        );

        drop(output);
        handle.join().unwrap();
    }

    #[test]
    fn reader_marks_available_when_quiet_deadline_passes_with_no_write() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        let fixture = TitleFixture::new();
        // 2 s left so initial read never straddles a second boundary into availability.
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        fixture.record_activity(now - 118, false);

        let reads = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&reads);
        let config = super::ReaderConfig {
            throttle: Duration::from_millis(40),
            retry_base: Duration::from_millis(40),
            retry_cap: Duration::from_millis(200),
        };
        let (reader, handle) = spawn_test_reader(fixture.path.clone(), config, move |store, id| {
            counted.fetch_add(1, Ordering::SeqCst);
            super::output_title(store, id)
        });
        let (mut wake, waker) = std::os::unix::net::UnixStream::pair().unwrap();
        let mut output = super::OutputTitle::new(fixture.terminal.clone(), waker, reader);
        output.set_viewed(true);

        let initial = recv_title(&mut output, &mut wake, Duration::from_secs(3)).unwrap();
        assert_eq!(initial, format!("{} work session", fixture.task_identifier));

        // No writes: availability arrives once the 120 s quiet window elapses.
        let available = recv_title(&mut output, &mut wake, Duration::from_secs(5)).unwrap();
        assert_eq!(
            available,
            format!("{} work session · Available", fixture.task_identifier)
        );

        // Once available with no further deadlines, read count stays fixed.
        let after_available = reads.load(Ordering::SeqCst);
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(reads.load(Ordering::SeqCst), after_available);

        drop(output);
        handle.join().unwrap();
    }

    #[test]
    fn reader_rearms_deadline_when_observed_at_bumps_without_revision_move() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        let fixture = TitleFixture::new();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        fixture.record_activity(now - 118, false);

        let reads = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&reads);
        let config = super::ReaderConfig {
            throttle: Duration::from_millis(40),
            retry_base: Duration::from_millis(40),
            retry_cap: Duration::from_millis(200),
        };
        let (reader, handle) = spawn_test_reader(fixture.path.clone(), config, move |store, id| {
            counted.fetch_add(1, Ordering::SeqCst);
            super::output_title(store, id)
        });
        let (mut wake, waker) = std::os::unix::net::UnixStream::pair().unwrap();
        let mut output = super::OutputTitle::new(fixture.terminal.clone(), waker, reader);
        output.set_viewed(true);

        let initial = recv_title(&mut output, &mut wake, Duration::from_secs(3)).unwrap();
        assert_eq!(initial, format!("{} work session", fixture.task_identifier));
        assert_eq!(reads.load(Ordering::SeqCst), 1);

        // Bump `observed_at` forward by 2 s without moving `sessions`.
        let before_rev = fixture.store.revisions().unwrap().sessions;
        fixture.record_activity(now - 116, false);
        assert_eq!(fixture.store.revisions().unwrap().sessions, before_rev);

        // Wait past the first deadline (`now + 2`) while still before the bumped deadline (`now + 4`).
        let past_first = super::duration_until_unix(now + 2) + Duration::from_millis(300);
        assert_eq!(recv_title(&mut output, &mut wake, past_first), None);
        assert!(
            reads.load(Ordering::SeqCst) >= 2,
            "first deadline should have fired and re-armed"
        );

        // Later deadline (`now + 4`) arrives and publishes `· Available`.
        let available = recv_title(&mut output, &mut wake, Duration::from_secs(5)).unwrap();
        assert_eq!(
            available,
            format!("{} work session · Available", fixture.task_identifier)
        );

        drop(output);
        handle.join().unwrap();
    }

    #[test]
    fn reader_retries_after_failed_sweep_and_lands_rename() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        let fixture = TitleFixture::new();
        let attempts = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&attempts);
        let config = super::ReaderConfig {
            throttle: Duration::from_millis(40),
            retry_base: Duration::from_millis(50),
            retry_cap: Duration::from_millis(200),
        };
        let (reader, handle) = spawn_test_reader(fixture.path.clone(), config, move |store, id| {
            let n = counted.fetch_add(1, Ordering::SeqCst) + 1;
            if n == 2 {
                anyhow::bail!("simulated SQLITE_BUSY");
            }
            super::output_title(store, id)
        });
        let (mut wake, waker) = std::os::unix::net::UnixStream::pair().unwrap();
        let mut output = super::OutputTitle::new(fixture.terminal.clone(), waker, reader);
        output.set_viewed(true);

        let initial = recv_title(&mut output, &mut wake, Duration::from_secs(3)).unwrap();
        assert_eq!(initial, format!("{} work session", fixture.task_identifier));

        crate::store::sessions::rename_session(
            &fixture.store,
            &fixture.session_id,
            "After busy",
            crate::session::TitleSource::Human,
        )
        .unwrap();

        let renamed = recv_title(&mut output, &mut wake, Duration::from_secs(3)).unwrap();
        assert_eq!(renamed, format!("{} After busy", fixture.task_identifier));
        assert!(attempts.load(Ordering::SeqCst) >= 3);

        drop(output);
        handle.join().unwrap();
    }

    #[test]
    fn unviewed_terminal_does_no_reads_until_viewer_attaches() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;
        use std::time::Duration;

        let fixture = TitleFixture::new();
        let reads = Arc::new(AtomicUsize::new(0));
        let counted = Arc::clone(&reads);
        let config = super::ReaderConfig {
            throttle: Duration::from_millis(40),
            retry_base: Duration::from_millis(40),
            retry_cap: Duration::from_millis(200),
        };
        let (reader, handle) = spawn_test_reader(fixture.path.clone(), config, move |store, id| {
            counted.fetch_add(1, Ordering::SeqCst);
            super::output_title(store, id)
        });
        let (mut wake, waker) = std::os::unix::net::UnixStream::pair().unwrap();
        let mut output = super::OutputTitle::new(fixture.terminal.clone(), waker, reader);

        // No viewer attached: renames and elapsed time trigger zero reads.
        crate::store::sessions::rename_session(
            &fixture.store,
            &fixture.session_id,
            "While unviewed",
            crate::session::TitleSource::Human,
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(reads.load(Ordering::SeqCst), 0);
        assert_eq!(output.tick(), None);

        // Viewer attaches: one immediate read delivers the current name.
        output.set_viewed(true);
        let attached = recv_title(&mut output, &mut wake, Duration::from_secs(3)).unwrap();
        assert_eq!(
            attached,
            format!("{} While unviewed", fixture.task_identifier)
        );
        assert_eq!(reads.load(Ordering::SeqCst), 1);

        // Viewer detaches: subsequent renames trigger zero reads.
        output.set_viewed(false);
        std::thread::sleep(Duration::from_millis(50));
        crate::store::sessions::rename_session(
            &fixture.store,
            &fixture.session_id,
            "After detach",
            crate::session::TitleSource::Human,
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(200));
        assert_eq!(reads.load(Ordering::SeqCst), 1);

        drop(output);
        handle.join().unwrap();
    }

    #[test]
    fn output_title_warm_statement_throughput() {
        let fixture = TitleFixture::new();
        fixture.put_at_design_step();
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        fixture.record_activity(now, false);

        // Warm the pooled read-only connection and its statement cache.
        let store = crate::store::sqlite::SqliteStore::open_read_only(
            &fixture.path,
            crate::store::token_key::encryption_key,
        )
        .unwrap();
        let first = super::output_title(&store, &fixture.terminal)
            .unwrap()
            .unwrap();
        assert_eq!(
            first.terminal,
            format!("{} work session · Design", fixture.task_identifier)
        );

        let started = std::time::Instant::now();
        for _ in 0..1000 {
            let reading = super::output_title(&store, &fixture.terminal)
                .unwrap()
                .unwrap();
            assert_eq!(reading.terminal, first.terminal);
        }
        let same_store_elapsed = started.elapsed();
        drop(store);

        let started_pooled = std::time::Instant::now();
        for _ in 0..1000 {
            let store = crate::store::sqlite::SqliteStore::open_read_only(
                &fixture.path,
                crate::store::token_key::encryption_key,
            )
            .unwrap();
            let reading = super::output_title(&store, &fixture.terminal)
                .unwrap()
                .unwrap();
            assert_eq!(reading.terminal, first.terminal);
        }
        let pooled_elapsed = started_pooled.elapsed();
        eprintln!(
            "output_title x1000: same-store={same_store_elapsed:?} ({:.3} ms/read), pooled-borrow={pooled_elapsed:?} ({:.3} ms/read)",
            same_store_elapsed.as_secs_f64(),
            pooled_elapsed.as_secs_f64(),
        );
    }

    #[test]
    fn viewer_gate_skips_full_reads_until_revision_or_deadline() {
        let fixture = TitleFixture::new();
        let store = crate::store::sqlite::SqliteStore::open_read_only(
            &fixture.path,
            crate::store::token_key::encryption_key,
        )
        .unwrap();
        let session = store.session(&fixture.session_id).unwrap().unwrap();
        let mut title = TerminalTitle::from_session(fixture.path.clone(), &store, session).unwrap();
        drop(store);

        // Quiet store: revision gate short-circuits without re-reading names.
        assert_eq!(title.poll_names().unwrap(), super::ViewerPoll::Unchanged);

        // Unrelated domain (`processes`) does not open the gate.
        fixture.store.test_lf_process();
        assert_eq!(title.poll_names().unwrap(), super::ViewerPoll::Unchanged);

        // Rename moves `sessions` and reads the new title once.
        crate::store::sessions::rename_session(
            &fixture.store,
            &fixture.session_id,
            "Viewer renamed",
            crate::session::TitleSource::Human,
        )
        .unwrap();
        match title.poll_names().unwrap() {
            super::ViewerPoll::Read(names) => {
                assert_eq!(
                    names.terminal,
                    format!("{} Viewer renamed", fixture.task_identifier)
                );
            }
            other => panic!("expected Read after rename, got {other:?}"),
        }
        assert_eq!(title.poll_names().unwrap(), super::ViewerPoll::Unchanged);

        // Quiet deadline fires with no write once 120 s elapses.
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        fixture.record_activity(now - 118, false);
        match title.poll_names().unwrap() {
            super::ViewerPoll::Read(names) => {
                assert_eq!(
                    names.terminal,
                    format!("{} Viewer renamed", fixture.task_identifier)
                );
            }
            other => panic!("expected Read after activity record, got {other:?}"),
        }
        assert_eq!(title.poll_names().unwrap(), super::ViewerPoll::Unchanged);

        let wait = super::duration_until_unix(now + 2) + std::time::Duration::from_millis(50);
        std::thread::sleep(wait);
        match title.poll_names().unwrap() {
            super::ViewerPoll::Read(names) => {
                assert_eq!(
                    names.terminal,
                    format!("{} Viewer renamed · Available", fixture.task_identifier)
                );
            }
            other => panic!("expected Available after deadline, got {other:?}"),
        }
        assert_eq!(title.poll_names().unwrap(), super::ViewerPoll::Unchanged);

        // Replacing attachment detaches the viewer.
        fixture
            .store
            .record_agent_process_exit(&fixture.session_id, &fixture.attachment, true)
            .unwrap();
        let process = fixture.store.test_lf_process();
        attachment::claim_session_attachment(
            &fixture.store,
            &fixture.session_id,
            Some(&fixture.attachment),
            &process,
        )
        .unwrap();
        assert_eq!(title.poll_names().unwrap(), super::ViewerPoll::Detached);
    }
}
