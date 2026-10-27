//! Session names projected into their attached terminal. Only the pre-spawn
//! write uses OSC: writing alongside a native provider can split escape frames.
//! cmux and tmux have separate control channels for their workspace, tab and
//! window names, so those follow renames.

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

/// Where a Task stands, for destination names: `Available` when its
/// conversation is, then the Workflow node where a person takes part, such as
/// `Available · Design`. Both come from their shared readers; nothing new is judged.
pub(crate) fn standing(
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

    fn rename(&self, name: &str) -> std::io::Result<()> {
        match self {
            Self::Cmux { workspace, surface } => {
                host_command(
                    "cmux",
                    &["rename-workspace", "--workspace", workspace, "--", name],
                )?;
                host_command(
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
                )
            }
            Self::Tmux { pane } => host_command("tmux", &["rename-window", "-t", pane, "--", name]),
        }
    }
}

#[derive(Debug)]
pub(crate) struct TerminalTitle {
    store: SqliteStore,
    session: String,
    attachment: Option<SessionAttachment>,
    name: String,
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
        if let Err(error) = write!(std::io::stderr().lock(), "\x1b]0;{}\x07", title.name) {
            tracing::warn!(%error, "cannot set terminal Session name");
        }
        title.publish();
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
        title.publish();
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
            name: name.terminal,
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
        match self.read_name() {
            Ok(Some(name)) if name != self.name => {
                self.name = name;
                self.publish();
            }
            Ok(Some(_)) => {}
            // A handed-off attachment must stop changing the old terminal's names.
            Ok(None) => self.host = None,
            Err(error) => {
                tracing::warn!(%error, "cannot refresh terminal Session name");
                self.host = None;
            }
        }
    }

    fn read_name(&self) -> StoreResult<Option<String>> {
        if self.store.session_attachment(&self.session)? != self.attachment {
            return Ok(None);
        }
        self.store
            .session(&self.session)?
            .map(|session| session_title(&self.store, &session).map(|name| name.terminal))
            .transpose()
    }

    fn publish(&mut self) {
        let Some(host) = &self.host else {
            return;
        };
        if let Err(error) = host.rename(&self.name) {
            tracing::warn!(%error, "terminal host title update failed; Session continues");
            self.host = None;
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
    /// That name and where its Task stands now.
    terminal: String,
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
        terminal: display_title(&session.title, identifier, standing.as_deref()),
    })
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
    use super::{display_title, TerminalTitle};

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
            name: session.title.clone(),
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
        assert_eq!(title.read_name().unwrap().as_deref(), Some("Release notes"));
        title
            .store
            .claim_session_attachment(&title.session, Some(&attachment), &process, false)
            .unwrap();
        assert_eq!(title.read_name().unwrap(), None);
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
}
