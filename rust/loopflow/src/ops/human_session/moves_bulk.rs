//! Bulk moves over the Session selector: `lf session move`. Interactive
//! Sessions only. Each selected Session continues on another agent through a
//! fresh seeded thread, now when idle, after its turn when busy. A login is
//! not a Session's to move: every open Claude follows the Machine's login,
//! which `lf account use` switches, and a running Codex keeps its own until
//! `lf --account codex=<login> resume`. Nothing here writes preferences.

use std::os::unix::process::CommandExt;
use std::sync::Arc;

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

use super::{is_running, list, new_thread, SessionRecord};
use crate::id::LfSessionId;
use crate::session::SessionFilter;
use crate::store::sqlite::SqliteStore;
use crate::store::{SharedStore, StorageConfig};

/// The `harness[:model]` a move continues the selected Sessions on.
pub(crate) fn parse_target(agent: Option<&str>) -> Result<String> {
    let Some(agent) = agent else {
        bail!("a Session move needs a target: -a harness[:model]");
    };
    let (harness, _) = crate::config::parse_agent(agent);
    if crate::harness::HarnessKind::parse(&harness).is_none() {
        bail!("unknown agent {agent}: name harness[:model], such as claude:opus or codex");
    }
    Ok(agent.to_owned())
}

#[derive(Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MoveReport {
    pub moved: Vec<Moved>,
    /// Busy Sessions; each moves when its turn ends.
    pub pending: Vec<Moved>,
    pub skipped: Vec<Skipped>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Moved {
    pub id: LfSessionId,
    pub title: String,
    pub how: String,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Skipped {
    pub id: LfSessionId,
    pub title: String,
    pub reason: String,
}

impl MoveReport {
    pub(crate) fn print(&self) {
        for moved in self.moved.iter().chain(&self.pending) {
            println!("{}  {}: {}", moved.id, moved.title, moved.how);
        }
        for skipped in &self.skipped {
            println!(
                "{}  {}: skipped, {}",
                skipped.id, skipped.title, skipped.reason
            );
        }
        if self.moved.is_empty() && self.pending.is_empty() && self.skipped.is_empty() {
            println!("No Sessions selected.");
        }
    }

    fn moved(&mut self, session: &SessionRecord, how: String) {
        self.moved.push(Moved::new(session, how));
    }

    fn pending(&mut self, session: &SessionRecord, how: String) {
        self.pending.push(Moved::new(session, how));
    }

    fn skip(&mut self, session: &SessionRecord, reason: impl Into<String>) {
        self.skipped.push(Skipped {
            id: session.id.clone(),
            title: session.title.clone(),
            reason: reason.into(),
        });
    }
}

impl Moved {
    fn new(session: &SessionRecord, how: String) -> Self {
        Self {
            id: session.id.clone(),
            title: session.title.clone(),
            how,
        }
    }
}

/// Move the interactive Sessions `filter` selects to `agent`. The filter
/// must select by `blocked` or `on`.
pub(crate) async fn move_sessions(
    store: &SharedStore,
    filter: &SessionFilter,
    agent: &str,
) -> Result<MoveReport> {
    if !filter.blocked && filter.on.is_none() {
        bail!("a Session move needs a selector: --blocked or --on <account|harness[:model]>");
    }
    let mut selection = filter.clone();
    selection.interactive = Some(true);
    selection.limit = 0;
    selection.offset = 0;
    selection.after = None;
    let mut report = MoveReport::default();
    for session in &list(store, &selection).await? {
        move_agent(store, session, agent, &mut report).await;
    }
    Ok(report)
}

/// An idle Session continues on `agent` at its next open; a busy one after
/// its turn, through its Stop hook.
async fn move_agent(
    store: &SharedStore,
    session: &SessionRecord,
    agent: &str,
    report: &mut MoveReport,
) {
    let busy = session.availability.is_none() && is_running(session.state);
    if busy {
        match store.sqlite.record_session_move(session.id.as_str(), agent) {
            Ok(()) => report.pending(session, format!("continues on {agent} after its turn")),
            Err(error) => report.skip(session, format!("{error:#}")),
        }
        return;
    }
    match new_thread(store, session.id.as_str(), Some(agent), false, false).await {
        Ok(_) => report.moved(session, format!("continues on {agent} at its next open")),
        Err(error) => report.skip(session, format!("{error:#}")),
    }
}

/// At a Stop hook: when a move waits for `session`, start its applier beside
/// the provider and return at once. The hook only peeks, through the store it
/// opened for the turn; the applier stops the provider, so it must outlive the
/// hook rather than run inside it.
pub(crate) fn start_pending_move(store: &SqliteStore, session: &str) -> Result<bool> {
    if store.pending_session_move(session)?.is_none() {
        return Ok(false);
    }
    let mut command = std::process::Command::new(crate::os_process::resolve_lf_binary()?);
    command
        .args(["__context-block", "--moment", "move"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // SAFETY: only an async-signal-safe syscall runs between fork and exec.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() < 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
    command.spawn().context("start the pending Session move")?;
    Ok(true)
}

/// The applier a Stop hook started. Once the hook has exited, so the turn has
/// ended, the Session continues on the requested agent the way an idle
/// Session's move does.
pub(crate) fn apply_pending_move() -> Result<()> {
    let adapter: crate::steer::SteerAdapter =
        serde_json::from_str(&std::env::var(crate::steer::PROVIDER_CONTEXT_ENV)?)?;
    wait_for_hook_exit();
    let home = crate::store::lf_home_dir();
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = Arc::new(
            crate::store::open_store(&StorageConfig::sqlite(home.join("loopflow.db"))).await?,
        );
        apply_move(&store, &adapter.session).await
    })
}

/// The hook that started this applier is its parent until it exits; the
/// provider ends its turn right after. The wait outlasts the hook's timeout.
fn wait_for_hook_exit() {
    let hook = std::os::unix::process::parent_id();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    while std::os::unix::process::parent_id() == hook && std::time::Instant::now() < deadline {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    std::thread::sleep(std::time::Duration::from_secs(1));
}

/// Continue `session` on its pending move's agent. The request is cleared only
/// after the new input is appended, so a failed or killed apply is retried at
/// the Session's next Stop instead of lost.
async fn apply_move(store: &SharedStore, session: &str) -> Result<()> {
    let Some(agent) = store.sqlite.pending_session_move(session)? else {
        return Ok(());
    };
    new_thread(store, session, Some(&agent), false, false).await?;
    store.sqlite.clear_session_move(session, &agent)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{apply_move, start_pending_move};

    #[test]
    fn a_stop_with_no_pending_move_starts_nothing() {
        let home = tempfile::tempdir().unwrap();
        let store =
            crate::store::sqlite::SqliteStore::open_ephemeral(&home.path().join("loopflow.db"))
                .unwrap();
        assert!(!start_pending_move(&store, "session_idle").unwrap());
    }

    #[test]
    fn a_failed_apply_leaves_the_move_for_the_next_stop() {
        let home = super::super::tests::SessionHome::new();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        runtime.block_on(async {
            let store = home.store().await;
            store
                .sqlite
                .record_session_move("session_missing", "codex")
                .unwrap();
            assert!(apply_move(&store, "session_missing").await.is_err());
            assert_eq!(
                store
                    .sqlite
                    .pending_session_move("session_missing")
                    .unwrap(),
                Some("codex".into())
            );
            store
                .sqlite
                .clear_session_move("session_missing", "claude:opus")
                .unwrap();
            assert!(store
                .sqlite
                .pending_session_move("session_missing")
                .unwrap()
                .is_some());
        });
    }
}
