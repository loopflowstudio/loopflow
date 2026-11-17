//! The recovery pass: Task conversations that died come back. One watcher
//! reads the store when its planning, Session or process revisions move, and
//! at least every minute for backoffs and deaths nothing recorded. Each due
//! conversation is started the way `session.continue` starts one, in a held
//! terminal. The store counts each start, so a server restart forgets nothing
//! and one death never starts two providers.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use anyhow::Result;

use loopflow_execution::transport::registry::Registry;
use loopflow_model::id::LfSessionId;
use loopflow_storage::sqlite::session_recovery::RecoverableConversation;
use loopflow_storage::sqlite::{SqliteStore, StoreRevisions};

/// How long the watcher goes without reading when nothing it reads moved:
/// backoffs come due, and a reboot leaves deaths nobody recorded.
const UNREAD: Duration = Duration::from_secs(60);

/// Start the Machine's one recovery watcher. It runs as long as the server.
pub(super) fn start(terminals: Arc<Registry>) {
    let spawned = std::thread::Builder::new()
        .name("recovery".into())
        .spawn(move || {
            loopflow_execution::journal::request::isolate(|| watch(&terminals));
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "Task conversations that die stay down until resumed");
    }
}

fn watch(terminals: &Registry) {
    let path = loopflow_execution::store::default_db_path();
    let mut changes = loopflow_storage::changes::StoreChanges::watch(&path);
    let mut watcher = Watcher::default();
    loop {
        watcher.read(&path, terminals);
        changes.wait(UNREAD);
    }
}

/// The watcher's store, and what it read last and when.
#[derive(Default)]
struct Watcher {
    store: Option<SqliteStore>,
    seen: Option<(StoreRevisions, SystemTime)>,
}

impl Watcher {
    /// Run the pass if the revisions moved or a minute passed. The first
    /// reading runs it: after a reboot or a server restart, that is when what
    /// died meanwhile comes back.
    fn read(&mut self, path: &Path, terminals: &Registry) {
        if self.store.is_none() && path.exists() {
            self.store = loopflow_execution::store::open_sqlite(path)
                .inspect_err(|error| tracing::warn!(%error, "recovery pass has no store"))
                .ok();
        }
        let Some(store) = &self.store else {
            return;
        };
        let revisions = match store.revisions() {
            Ok(revisions) => revisions,
            Err(error) => {
                tracing::warn!(%error, "recovery pass cannot read store revisions");
                self.store = None;
                return;
            }
        };
        let now = SystemTime::now();
        let unchanged = self.seen.is_some_and(|(seen, at)| {
            (seen.planning, seen.sessions, seen.processes)
                == (revisions.planning, revisions.sessions, revisions.processes)
                && now.duration_since(at).is_ok_and(|waited| waited < UNREAD)
        });
        if unchanged {
            return;
        }
        self.seen = Some((revisions, now));
        let held = |session: &LfSessionId| {
            terminals.session_terminal(session).is_some()
                || terminals.pending_terminal(session, None).is_some()
        };
        let start = |conversation: &RecoverableConversation, request: String| {
            let start = super::service::continuing(conversation.session.clone(), request, 0, 0)
                .map_err(|error| anyhow::anyhow!("{}", error.message))?;
            terminals.start(start).map(|_| ())
        };
        if let Err(error) = recover_conversations(store, unix(now), held, start) {
            tracing::warn!(error = %format!("{error:#}"), "recovery pass failed; it runs again at the next change");
        }
    }
}

fn unix(at: SystemTime) -> i64 {
    at.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() as i64)
}

/// Start every due Task conversation that has no terminal, one at a time.
/// Each start is counted before it is made, and its request names the ended
/// agent and the attempt, so the registry answers a repeat with the terminal
/// it already started.
fn recover_conversations(
    store: &SqliteStore,
    now: i64,
    held: impl Fn(&LfSessionId) -> bool,
    mut start: impl FnMut(&RecoverableConversation, String) -> Result<()>,
) -> Result<()> {
    for conversation in loopflow_execution::ops::recovery::due_task_conversations(store, now)? {
        let RecoverableConversation { session, agent, .. } = &conversation;
        if held(session) {
            continue;
        }
        let Some(attempt) = store.record_recovery_start(session, agent, now)? else {
            continue;
        };
        let request = format!("recover:{session}:{agent}:{attempt}");
        match start(&conversation, request) {
            Ok(()) => {
                tracing::info!(%session, task = %conversation.task, exit = ?conversation.exit_code, attempt, "Task conversation started again")
            }
            Err(error) => {
                tracing::warn!(%session, task = %conversation.task, attempt, error = %format!("{error:#}"), "Task conversation did not start again")
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::recover_conversations;
    use loopflow_execution::process::attachment;
    use loopflow_model::durable::TaskId;
    use loopflow_model::id::{AgentProcessId, LfProcessId, LfSessionId};
    use loopflow_storage::sqlite::SqliteStore;
    use loopflow_test_support::TestRepo;
    use rusqlite::params;

    const NOW: i64 = 1_800_000_000;

    /// A Task claimed here with a checkout and its conversation, whose agent
    /// is running as this test process.
    struct Fixture {
        _home: tempfile::TempDir,
        _repo: TestRepo,
        store: SqliteStore,
        task: TaskId,
        session: LfSessionId,
        launcher: LfProcessId,
        agent: AgentProcessId,
    }

    fn fixture() -> Fixture {
        let home = tempfile::tempdir_in("/tmp").unwrap();
        let store =
            loopflow_execution::store::opening::open_ephemeral(&home.path().join("store.db"))
                .unwrap();
        let repo = TestRepo::new();
        let checkout = repo.path().to_string_lossy().into_owned();
        let task = TaskId::new();
        store
            .create_task(&loopflow_storage::work::task::NewTask {
                repository_id: loopflow_execution::repository::location::ensure_repository(
                    &store, &checkout,
                )
                .unwrap(),
                id: task.clone(),
                project_id: None,
                title: "Recover".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        loopflow_execution::store::ownership::claim_task(&store, &task).unwrap();
        let session = store
            .test_session(
                "session_recovered",
                &loopflow_execution::session_record::new_artifact_key(),
            )
            .id;
        let captured = store.session(session.as_str()).unwrap().unwrap().captured;
        loopflow_execution::store::sessions::bind_session(
            &store,
            session.as_str(),
            captured,
            &task,
        )
        .unwrap();
        let launcher = store.test_lf_process();
        let claim =
            attachment::claim_session_attachment(&store, session.as_str(), None, &launcher, true)
                .unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO task_checkouts(task_id,worktree) VALUES(?1,?2)",
                params![task.as_str(), checkout],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO task_primary_sessions(task_id,session_id) VALUES(?1,?2)",
                params![task.as_str(), session.as_str()],
            )
            .unwrap();
            conn.execute(
                "UPDATE agent_sessions SET interactive=1,cwd=?2 WHERE id=?1",
                params![session.as_str(), format!("{checkout}/")],
            )
            .unwrap();
        }
        let fixture = Fixture {
            _home: home,
            _repo: repo,
            store,
            task,
            session,
            launcher,
            agent: claim.agent_process_id,
        };
        let pid = std::process::id();
        fixture.agent_identity(
            pid,
            loopflow_execution::journal::process_started_at(pid)
                .unwrap()
                .unwrap(),
        );
        fixture
    }

    impl Fixture {
        fn agent_identity(&self, pid: u32, started_at: i64) {
            self.store
                .conn
                .lock()
                .unwrap()
                .execute(
                    "UPDATE processes SET pid=?2,os_started_at=?3 WHERE id=?1",
                    params![self.agent, pid, started_at],
                )
                .unwrap();
        }

        /// The provider exited with `code`, as the holder's settlement records it.
        fn exited(&self, code: i32) {
            let conn = self.store.conn.lock().unwrap();
            conn.execute(
                "UPDATE processes SET completed_at=?2 WHERE id=?1",
                params![self.agent, NOW - 5],
            )
            .unwrap();
            conn.execute(
                "UPDATE processes SET completed_at=?2,exit_code=?3 WHERE id=?1",
                params![self.launcher, NOW - 5, code],
            )
            .unwrap();
        }

        /// The requests one pass at `now` starts, with `held` Sessions skipped.
        fn pass(&self, now: i64, held: bool) -> Vec<String> {
            let started = RefCell::new(Vec::new());
            recover_conversations(
                &self.store,
                now,
                |_| held,
                |conversation, request| {
                    assert_eq!(conversation.session, self.session);
                    started.borrow_mut().push(request);
                    Ok(())
                },
            )
            .unwrap();
            started.into_inner()
        }

        fn attempts(&self) -> (u32, Option<i64>) {
            self.store
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT recovery_attempts,recovered_at FROM agent_sessions WHERE id=?1",
                    [self.session.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .unwrap()
        }
    }

    #[test]
    fn a_killed_task_conversation_starts_again_once_per_backoff() {
        let fixture = fixture();
        assert!(
            fixture.pass(NOW, false).is_empty(),
            "a live agent is left alone"
        );
        fixture.exited(137);
        let first = format!("recover:{}:{}:1", fixture.session, fixture.agent);
        assert_eq!(fixture.pass(NOW, false), [first]);
        assert_eq!(fixture.attempts(), (1, Some(NOW)));
        // The start is in flight: the next change starts nothing.
        assert!(fixture.pass(NOW + 30, false).is_empty());
        // Still down a minute later: started again, as a new request.
        assert_eq!(
            fixture.pass(NOW + 60, false),
            [format!("recover:{}:{}:2", fixture.session, fixture.agent)]
        );
    }

    #[test]
    fn a_quit_task_conversation_stays_quit() {
        for code in [0, 130] {
            let fixture = fixture();
            fixture.exited(code);
            assert!(fixture.pass(NOW, false).is_empty(), "exit {code}");
            assert_eq!(fixture.attempts(), (0, None));
        }
    }

    #[test]
    fn an_agent_gone_with_no_recorded_end_counts_as_died() {
        let fixture = fixture();
        // A reboot or a dead holder: the row is unfinished and its process gone.
        fixture.agent_identity(std::process::id(), 1);
        assert_eq!(fixture.pass(NOW, false).len(), 1);
    }

    #[test]
    fn a_conversation_with_a_terminal_is_not_started_or_counted() {
        let fixture = fixture();
        fixture.exited(1);
        assert!(fixture.pass(NOW, true).is_empty());
        assert_eq!(fixture.attempts(), (0, None));
    }

    #[test]
    fn a_closed_task_or_a_conversation_out_of_starts_is_not_started() {
        let fixture = fixture();
        fixture.exited(137);
        fixture
            .store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET recovery_attempts=5,recovered_at=?2 WHERE id=?1",
                params![fixture.session.as_str(), NOW - 3600],
            )
            .unwrap();
        assert!(fixture.pass(NOW, false).is_empty(), "gave up after 5");

        let fixture = super::tests::fixture();
        fixture.exited(137);
        fixture
            .store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET abandoned_at=?2 WHERE id=?1",
                params![fixture.task.as_str(), NOW - 60],
            )
            .unwrap();
        assert!(fixture.pass(NOW, false).is_empty(), "abandoned");
    }
}
