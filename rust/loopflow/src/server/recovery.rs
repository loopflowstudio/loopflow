//! The recovery pass: this Machine's work settles, and Task conversations
//! that died, or were prepared and never started, come back. One watcher reads
//! the store when its planning, Session or process revisions move, and at
//! least every minute for backoffs and deaths nothing recorded. Each due
//! conversation is started the way `session.continue` starts one, in a held
//! terminal, one at a time. The store counts each start, so a server restart
//! forgets nothing and one death never starts two providers.

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
/// The shortest time between two settlements: each one writes what it
/// settles, which moves the revisions the watcher reads.
const SETTLE_EVERY: Duration = Duration::from_secs(10);
/// The longest one start holds the next: after a reboot, conversations come
/// back one at a time rather than all at once.
const PACE: Duration = Duration::from_secs(3);
/// How soon the watcher reads again after a due conversation still had a
/// terminal: the registry forgets an ended terminal just after the store
/// records its end, and nothing writes after that.
const ENDING: Duration = Duration::from_secs(1);

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
        changes.wait(if watcher.ending { ENDING } else { UNREAD });
    }
}

/// The watcher's store, what it read last and when, when it last settled,
/// and whether a due conversation's terminal was still ending.
#[derive(Default)]
struct Watcher {
    store: Option<SqliteStore>,
    seen: Option<(StoreRevisions, SystemTime)>,
    settled: Option<SystemTime>,
    ending: bool,
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
        if unchanged && !self.ending {
            return;
        }
        self.seen = Some((revisions, now));
        if self.settled.is_none_or(|at| {
            now.duration_since(at)
                .is_ok_and(|since| since >= SETTLE_EVERY)
        }) {
            self.settled = Some(now);
            loopflow_execution::ops::recovery::settle();
        }
        let held = |session: &LfSessionId| {
            terminals.session_terminal(session).is_some()
                || terminals.pending_terminal(session, None).is_some()
        };
        let start = |conversation: &RecoverableConversation, request: String| {
            if let Some(failure) = loopflow_execution::ops::recovery::launch_failure(conversation) {
                anyhow::bail!(failure);
            }
            let start = super::service::continuing(conversation.session.clone(), request, 0, 0)
                .map_err(|error| anyhow::anyhow!("{}", error.message))?;
            terminals.start(start)?;
            pace(store, conversation);
            Ok(())
        };
        match recover_conversations(store, unix(now), held, start) {
            Ok(ending) => self.ending = ending,
            Err(error) => {
                tracing::warn!(error = %format!("{error:#}"), "recovery pass failed; it runs again at the next change");
            }
        }
    }
}

/// Hold the next start until this one's agent has attached, or [`PACE`].
fn pace(store: &SqliteStore, conversation: &RecoverableConversation) {
    let deadline = std::time::Instant::now() + PACE;
    while std::time::Instant::now() < deadline {
        let attached = store
            .session_attachment(conversation.session.as_str())
            .is_ok_and(|attachment| {
                attachment.is_some_and(|attachment| {
                    Some(&attachment.agent_process_id) != conversation.agent.as_ref()
                })
            });
        if attached {
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn unix(at: SystemTime) -> i64 {
    at.duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |since| since.as_secs() as i64)
}

/// Start every due Task conversation that has no terminal, one at a time,
/// and record why a start failed on its Session. Each start's request names
/// the ended agent and the attempt, so the registry answers a repeat with the
/// terminal it already started. `true` when a due one still had its terminal.
fn recover_conversations(
    store: &SqliteStore,
    now: i64,
    held: impl Fn(&LfSessionId) -> bool,
    mut start: impl FnMut(&RecoverableConversation, String) -> Result<()>,
) -> Result<bool> {
    let mut ending = false;
    for conversation in loopflow_execution::ops::recovery::due_task_conversations(store, now)? {
        let RecoverableConversation { session, agent, .. } = &conversation;
        if held(session) {
            ending = true;
            continue;
        }
        // A start after a death is counted before it is made. A first start
        // is counted only when it fails: its terminal holds the next pass off,
        // and the first death after it comes back at once.
        let attempt = match agent {
            Some(agent) => match store.record_recovery_start(session, Some(agent), now)? {
                Some(attempt) => attempt,
                None => continue,
            },
            None => conversation.recovery_attempts + 1,
        };
        let generation = agent.as_ref().map_or("new", |agent| agent.as_str());
        let request = format!("recover:{session}:{generation}:{attempt}");
        match start(&conversation, request) {
            Ok(()) => {
                store.record_recovery_failure(session, None)?;
                tracing::info!(%session, task = %conversation.task, exit = ?conversation.exit_code, attempt, "Task conversation started")
            }
            Err(error) => {
                if agent.is_none() {
                    store.record_recovery_start(session, None, now)?;
                }
                let failure = format!("{error:#}");
                store.record_recovery_failure(session, Some(&failure))?;
                tracing::warn!(%session, task = %conversation.task, attempt, error = %failure, "Task conversation did not start")
            }
        }
    }
    Ok(ending)
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

        fn failure(&self) -> Option<String> {
            self.store
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT recovery_failure FROM agent_sessions WHERE id=?1",
                    [self.session.as_str()],
                    |row| row.get(0),
                )
                .unwrap()
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
        // Its terminal is ending: the watcher reads again soon.
        let ending = recover_conversations(&fixture.store, NOW, |_| true, |_, _| Ok(())).unwrap();
        assert!(ending);
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

    #[test]
    fn a_prepared_task_conversation_that_never_ran_is_started() {
        let fixture = fixture();
        fixture
            .store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET agent_process_id=NULL WHERE id=?1",
                [fixture.session.as_str()],
            )
            .unwrap();
        assert_eq!(
            fixture.pass(NOW, false),
            [format!("recover:{}:new:1", fixture.session)]
        );
        assert_eq!(
            fixture.attempts(),
            (0, None),
            "a first start is not a death"
        );
        assert!(fixture.pass(NOW, true).is_empty(), "its terminal holds it");
        // A first start that fails counts, and waits its backoff.
        recover_conversations(
            &fixture.store,
            NOW,
            |_| false,
            |_, _| Err(anyhow::anyhow!("claude is not installed")),
        )
        .unwrap();
        assert_eq!(fixture.attempts(), (1, Some(NOW)));
        assert!(fixture.pass(NOW + 30, false).is_empty());
    }

    #[test]
    fn a_failed_start_says_why_until_one_succeeds() {
        let fixture = fixture();
        fixture.exited(137);
        recover_conversations(
            &fixture.store,
            NOW,
            |_| false,
            |_, _| Err(anyhow::anyhow!("no PTY left")),
        )
        .unwrap();
        assert_eq!(fixture.failure().as_deref(), Some("no PTY left"));
        assert_eq!(fixture.attempts(), (1, Some(NOW)), "a failure is a start");
        assert_eq!(fixture.pass(NOW + 60, false).len(), 1);
        assert_eq!(fixture.failure(), None);
    }
}
