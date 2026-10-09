//! Session names projected into their attached terminal. Only the pre-spawn
//! write uses OSC: writing alongside a native provider can split escape frames.
//! cmux has a separate control channel for its workspace/tab names and renames.

use std::collections::BTreeMap;
use std::io::{IsTerminal, Write};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use crate::engine::process::wait_for_exit;
use crate::process::SessionAttachment;
use crate::session::AgentSession;
use crate::store::{sqlite::SqliteStore, StoreResult};

fn display_title(name: &str, task: Option<&str>) -> String {
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
    format!("{task} {purpose}")
}

#[derive(Debug)]
pub(crate) struct TerminalTitle {
    store: SqliteStore,
    session: String,
    driver: Option<SessionAttachment>,
    name: String,
    cmux: Option<(String, String)>,
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
            Ok(Some(title)) => title,
            Ok(None) => return None,
            Err(error) => {
                tracing::warn!(%error, "cannot read terminal Session name");
                return None;
            }
        };
        // Called before adding any subcommand, positional input or `--` boundary.
        match harness {
            "claude" => {
                command.args(["--name", &title.name]);
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
        // Only cmux can observe renames while the provider owns terminal output.
        title.cmux.is_some().then_some(title)
    }

    fn load(input: &str) -> anyhow::Result<Option<Self>> {
        let store =
            SqliteStore::open_processes_read_only(&crate::store::database_path_from_env()?)?;
        let Some(session) = store.session_for_artifact(input)? else {
            return Ok(None);
        };
        Ok(Some(Self {
            driver: store.session_attachment(&session.id)?,
            name: session_title(&store, &session)?,
            store,
            session: session.id,
            cmux: std::env::var("CMUX_WORKSPACE_ID")
                .ok()
                .zip(std::env::var("CMUX_SURFACE_ID").ok())
                .filter(|(workspace, surface)| !workspace.is_empty() && !surface.is_empty()),
            checked: Instant::now(),
        }))
    }

    pub(crate) fn refresh(&mut self) {
        if self.cmux.is_none() || self.checked.elapsed() < Duration::from_millis(500) {
            return;
        }
        self.checked = Instant::now();
        match self.read_name() {
            Ok(Some(name)) if name != self.name => {
                self.name = name;
                self.publish();
            }
            Ok(Some(_)) => {}
            // A transferred driver must stop changing the old terminal's names.
            Ok(None) => self.cmux = None,
            Err(error) => {
                tracing::warn!(%error, "cannot refresh terminal Session name");
                self.cmux = None;
            }
        }
    }

    fn read_name(&self) -> StoreResult<Option<String>> {
        if self.store.session_attachment(&self.session)? != self.driver {
            return Ok(None);
        }
        self.store
            .session(&self.session)?
            .map(|session| session_title(&self.store, &session))
            .transpose()
    }

    fn publish(&mut self) {
        let Some((workspace, surface)) = &self.cmux else {
            return;
        };
        let result = rename_cmux(&[
            "rename-workspace",
            "--workspace",
            workspace,
            "--",
            &self.name,
        ])
        .and_then(|()| {
            rename_cmux(&[
                "rename-tab",
                "--workspace",
                workspace,
                "--surface",
                surface,
                "--",
                &self.name,
            ])
        });
        if let Err(error) = result {
            tracing::warn!(%error, "cmux title update failed; Session continues");
            self.cmux = None;
        }
    }
}

fn session_title(store: &SqliteStore, session: &AgentSession) -> StoreResult<String> {
    let task = session
        .task_id
        .as_ref()
        .map(|id| store.task(id))
        .transpose()?
        .flatten();
    Ok(display_title(
        &session.title,
        task.as_ref().map(|task| task.plan.identifier.as_str()),
    ))
}

fn rename_cmux(args: &[&str]) -> std::io::Result<()> {
    let mut child = Command::new("cmux")
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    let (status, timed_out) = wait_for_exit(&mut child, Some(Duration::from_secs(1)), || {})?;
    if timed_out {
        Err(std::io::Error::new(
            std::io::ErrorKind::TimedOut,
            "cmux title update timed out",
        ))
    } else if status.success() {
        Ok(())
    } else {
        Err(std::io::Error::other(format!("cmux exited {status}")))
    }
}

#[cfg(test)]
mod tests {
    use super::{display_title, TerminalTitle};

    #[test]
    fn replaced_driver_stops_observing_session_names() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = crate::store::sqlite::SqliteStore::open_ephemeral(&path).unwrap();
        let session =
            store.test_session("conversation", &crate::session_record::new_artifact_key());
        let process = crate::id::ProcessLfid::new();
        rusqlite::Connection::open(&path)
            .unwrap()
            .execute(
                "INSERT INTO processes(lfid,trace_id,started_at) VALUES(?1,'fixture',1)",
                [process.as_str()],
            )
            .unwrap();
        let driver = store
            .claim_session_attachment(&session.id, None, &process, true)
            .unwrap();
        let title = TerminalTitle {
            store,
            session: session.id,
            driver: Some(driver.clone()),
            name: session.title,
            cmux: None,
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
            .claim_session_attachment(&title.session, Some(&driver), &process, false)
            .unwrap();
        assert_eq!(title.read_name().unwrap(), None);
    }

    #[test]
    fn task_first_and_short_purpose_without_duplicate_identifier() {
        assert_eq!(display_title("demo", Some("LOO-412")), "LOO-412 demo");
        assert_eq!(
            display_title("LOO-412 plan store migration details", Some("LOO-412")),
            "LOO-412 plan store migration"
        );
        assert_eq!(
            display_title("LOO-4120 demo", Some("LOO-412")),
            "LOO-412 LOO-4120 demo"
        );
    }

    #[test]
    fn taskless_name_is_preserved_without_terminal_controls() {
        assert_eq!(
            display_title("Hérdr and machine work", None),
            "Hérdr and machine work"
        );
        assert_eq!(display_title("demo\x1b\x07\u{009c}", None), "demo");
    }
}
