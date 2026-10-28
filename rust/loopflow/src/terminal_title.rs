//! Session names projected into their attached terminal. Only the pre-spawn
//! write uses OSC: writing alongside a native provider can split escape frames.
//! cmux and tmux have separate control channels for their workspace, tab and
//! window names, so those follow renames. A cmux workspace belongs to its Task
//! and follows only the Task title, unless someone renamed it by hand; tabs and
//! tmux windows follow the Session.

use std::collections::BTreeMap;
use std::io::{IsTerminal, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::os_process::wait_for_exit;
use crate::process::SessionAttachment;
use crate::session::LfSession;
use crate::store::{sqlite::SqliteStore, StoreResult};

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
pub(crate) fn workspace_name(identifier: &str, title: &str) -> String {
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
fn standing(
    store: &SqliteStore,
    task: &crate::durable::TaskId,
    conversation: Option<&str>,
) -> StoreResult<Option<String>> {
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let available = match conversation {
        Some(id) => store
            .session_summary(id, now)?
            .is_some_and(|summary| summary.available),
        None => false,
    };
    let graph = store.task_graph(task)?;
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
    Ok((!parts.is_empty()).then(|| parts.join(" · ")))
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
    store: SqliteStore,
    session: String,
    attachment: Option<SessionAttachment>,
    names: Names,
    provider_name: String,
    host: Option<HostNames>,
    checked: Instant,
}

impl TerminalTitle {
    pub(crate) fn prepare(
        environment: &BTreeMap<String, String>,
        harness: &str,
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
        match harness {
            "claude" => {
                // The provider keeps its name; where the Task stands changes.
                command.args(["--name", &title.provider_name]);
                command.env("CLAUDE_CODE_DISABLE_TERMINAL_TITLE", "1");
            }
            "codex" => {
                command.args(["-c", "tui.terminal_title=[]"]);
            }
            _ => {}
        }
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

    fn viewing(owner: &crate::id::LfProcessId) -> Option<Self> {
        let load = || -> anyhow::Result<Option<Self>> {
            let store =
                SqliteStore::open_processes_read_only(&crate::store::database_path_from_env()?)?;
            let Some(id) = store.transport_session(owner)? else {
                return Ok(None);
            };
            let Some(session) = store.session(&id)? else {
                return Ok(None);
            };
            Self::from_session(store, session).map(Some)
        };
        let mut title = load()
            .inspect_err(|error| tracing::debug!(%error, "cannot read viewed Session name"))
            .ok()
            .flatten()?;
        title.publish(None);
        title.host.is_some().then_some(title)
    }

    fn load(input: &str) -> anyhow::Result<Option<Self>> {
        let store =
            SqliteStore::open_processes_read_only(&crate::store::database_path_from_env()?)?;
        let Some(session) = store.session_for_artifact(input)? else {
            return Ok(None);
        };
        Self::from_session(store, session).map(Some)
    }

    fn from_session(store: SqliteStore, session: LfSession) -> anyhow::Result<Self> {
        let name = session_title(&store, &session)?;
        Ok(Self {
            attachment: store.session_attachment(&session.id)?,
            provider_name: name.provider,
            names: name.names,
            store,
            session: session.id.to_string(),
            host: HostNames::current(),
            checked: Instant::now(),
        })
    }

    pub(crate) fn refresh(&mut self) {
        if self.host.is_none() || self.checked.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.checked = Instant::now();
        match self.read_names() {
            Ok(Some(names)) => {
                let previous = std::mem::replace(&mut self.names, names);
                self.publish(Some(&previous));
            }
            // A handed-off attachment must stop changing the old terminal's names.
            Ok(None) => self.host = None,
            Err(error) => {
                tracing::warn!(%error, "cannot refresh terminal Session name");
                self.host = None;
            }
        }
    }

    fn read_names(&self) -> StoreResult<Option<Names>> {
        if self.store.session_attachment(&self.session)? != self.attachment {
            return Ok(None);
        }
        self.store
            .session(&self.session)?
            .map(|session| session_title(&self.store, &session).map(|name| name.names))
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
    owner: crate::id::LfProcessId,
    title: Option<TerminalTitle>,
    /// Until when to look; executions without a Session never get one.
    looking: Option<Instant>,
    checked: Option<Instant>,
}

impl ViewerTitle {
    pub(crate) fn new(owner: &crate::id::LfProcessId) -> Self {
        Self {
            owner: owner.clone(),
            title: None,
            looking: HostNames::current().map(|_| Instant::now() + Duration::from_secs(10)),
            checked: None,
        }
    }

    pub(crate) fn tick(&mut self) {
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
        self.title = TerminalTitle::viewing(&self.owner);
    }
}

struct SessionTitle {
    /// The Session's own name, Task first.
    provider: String,
    /// That name and where its Task stands now, with the Task's workspace name.
    names: Names,
}

fn session_title(store: &SqliteStore, session: &LfSession) -> StoreResult<SessionTitle> {
    let task = session
        .task_id
        .as_ref()
        .map(|id| store.task(id))
        .transpose()?
        .flatten();
    let standing = task
        .as_ref()
        .map(|task| standing(store, &task.id, Some(&session.id)))
        .transpose()?
        .flatten();
    let identifier = task.as_ref().map(|task| task.identifier.as_str());
    Ok(SessionTitle {
        provider: display_title(&session.title, identifier, None),
        names: Names {
            workspace: task
                .as_ref()
                .map(|task| WorkspaceName::new(&task.identifier, &task.title)),
            terminal: display_title(&session.title, identifier, standing.as_deref()),
        },
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
    use super::{
        display_title, renames_due, workspace_name, HostRename, Names, TerminalTitle, WorkspaceName,
    };

    #[test]
    fn replaced_attachment_stops_observing_session_names() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = crate::store::sqlite::SqliteStore::open_ephemeral(&path).unwrap();
        let session =
            store.test_session("conversation", &crate::session_record::new_artifact_key());
        let process = crate::id::LfProcessId::new();
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'fixture',1)",
                [process.as_str()],
            )
            .unwrap();
        let attachment = store
            .claim_session_attachment(&session.id, None, &process, true)
            .unwrap();
        let title = TerminalTitle {
            store,
            session: session.id.to_string(),
            attachment: Some(attachment.clone()),
            names: Names {
                workspace: None,
                terminal: session.title.clone(),
            },
            provider_name: session.title,
            host: None,
            checked: std::time::Instant::now(),
        };
        title
            .store
            .rename_session(
                &title.session,
                "Release notes",
                crate::session::TitleSource::Human,
            )
            .unwrap();
        assert_eq!(
            title.read_names().unwrap().map(|names| names.terminal),
            Some("Release notes".to_string())
        );
        title
            .store
            .claim_session_attachment(&title.session, Some(&attachment), &process, false)
            .unwrap();
        assert_eq!(title.read_names().unwrap(), None);
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
}
