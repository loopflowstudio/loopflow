//! When a Session owner ends itself.
//!
//! Conversations stay resident until their scope closes or someone quits them.
//! Task abandonment ends unviewed conversations even during a turn.
//! Task completion ends eligible owners after its viewing grace, once live work
//! settles. Desktop shells end through Close or Task retirement.
//! `lf resume` and `lf process rollover` can replace an owner explicitly; unsent
//! input is lost when replacing its provider. Ending saves the screen and
//! records the exit so the conversation can resume from provider history.
//! Unreadable store state is retried at the next poll, never treated as closure.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use anyhow::Result;

use super::wire::Event;
use crate::id::LfProcessId;
use crate::ops::task::retirement::owner_task_completed;
use crate::store::sqlite::sessions::OwnerRetention;
use crate::store::sqlite::ViewerHold;
use crate::store::RecordingGap;

/// How often an owner records viewing and checks for closure.
pub(super) const POLL: Duration = Duration::from_secs(2);

/// Activity accepted by the serving owner. Revisions include changes whose
/// notification is still queued or throttled on its way to the policy.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(super) struct Activity {
    pub viewed: bool,
    pub viewers: u64,
    pub output: u64,
    /// Dated viewing evidence; a disconnected viewer enters the absence window.
    pub hold: Option<ViewerHold>,
}

impl Activity {
    pub fn view(&mut self, viewed: bool) -> bool {
        if self.viewed == viewed {
            return false;
        }
        self.viewed = viewed;
        self.viewers += 1;
        true
    }

    pub fn hold(&mut self, hold: Option<ViewerHold>) -> bool {
        let changed = self.hold != hold;
        self.hold = hold;
        changed
    }
}

/// An ending candidate carries the activity the policy judged for cleanup.
#[derive(Debug)]
pub(super) struct Ending {
    pub activity: Activity,
}

impl Ending {
    pub fn may_end(&self, activity: Activity, input_queued: bool) -> bool {
        !input_queued
            && activity.output == self.activity.output
            && !activity.viewed
            && activity.viewers == self.activity.viewers
    }

    /// Recheck the serving owner's current evidence before preparing its exit.
    pub fn confirm(
        &self,
        owner: &LfProcessId,
        activity: Activity,
        input_queued: bool,
    ) -> Result<Option<Event>> {
        let store = crate::journal::process_ledger()?;
        if let Some(may_close) = crate::desktop::shells::close_allowed(&store, owner, input_queued)?
        {
            return Ok(may_close.then_some(Event::Control {
                held: false,
                notice: Some("Shell closed".into()),
            }));
        }
        if !activity.viewed && !input_queued && store.owner_task_abandoned(owner)? {
            return Ok(Some(Event::Control {
                held: false,
                notice: Some("Task abandoned".into()),
            }));
        }
        if !self.may_end(activity, input_queued) || store.owner_retention(owner)?.is_some() {
            return Ok(None);
        }
        if owner_task_completed(&store, owner)? {
            return Ok(Some(Event::Control {
                held: false,
                notice: Some("Task completed".into()),
            }));
        }
        Ok(None)
    }
}

/// Record viewing and send cleanup candidates for the serve loop to confirm.
/// Task abandonment applies only to terminal owners; a pipe owner is a headless Run.
pub(super) fn spawn(
    owner: LfProcessId,
    terminal: bool,
    waker: UnixStream,
) -> (mpsc::Sender<Activity>, mpsc::Receiver<Ending>) {
    #[cfg(test)]
    if let Some(channels) = super::lifetime_tests::channels() {
        return channels;
    }
    let (activity, changes) = mpsc::channel();
    let (end, ending) = mpsc::channel();
    std::thread::spawn(move || {
        watch(&owner, terminal, &changes, &end, &waker);
    });
    (activity, ending)
}

fn watch(
    owner: &LfProcessId,
    terminal: bool,
    changes: &mpsc::Receiver<Activity>,
    end: &mpsc::Sender<Ending>,
    mut waker: &UnixStream,
) {
    let mut store = None;
    let mut store_failure = RecordingGap::default();
    // None means no observation has been committed, including initial absence.
    let mut recorded_hold = None;
    let mut hold_failure = RecordingGap::default();
    let mut shell_failure = RecordingGap::default();
    let mut retention_failure = RecordingGap::default();
    let mut abandonment_failure = RecordingGap::default();
    let mut completion_failure = RecordingGap::default();
    let mut retained: Option<OwnerRetention> = None;
    let mut next_poll = Instant::now() + POLL;
    let mut activity = Activity::default();
    loop {
        match changes.recv_timeout(next_poll.saturating_duration_since(Instant::now())) {
            Ok(changed) => activity = changed,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return,
        }
        // Activity updates the snapshot immediately but never accelerates file or
        // store work. Output can arrive much faster than the policy's poll.
        if Instant::now() < next_poll {
            continue;
        }
        next_poll = Instant::now() + POLL;
        if store.is_none() {
            store = report_result(
                &mut store_failure,
                "Session store",
                crate::journal::process_ledger(),
            );
        }
        let Some(store) = store.as_ref() else {
            continue;
        };
        let hold = activity.hold;
        if recorded_hold != Some(hold)
            && report_result(
                &mut hold_failure,
                "Session viewer hold",
                store.record_owner_viewer_hold(owner, hold),
            )
            .is_some()
        {
            recorded_hold = Some(hold);
        }
        // Stale viewing cannot authorize closure while recording is unavailable.
        if recorded_hold != Some(hold) {
            continue;
        }
        // Shell purpose is fixed at creation. Running lf inside an ordinary
        // shell must keep its Close policy even when it runs nested lf work.
        match report_result(
            &mut shell_failure,
            "Shell close request",
            store.shell_close_request(owner),
        ) {
            Some(Some(1 | 2)) => {
                if end.send(Ending { activity }).is_err() {
                    return;
                }
                let _ = waker.write_all(&[1]);
                continue;
            }
            Some(Some(_)) | None => continue,
            Some(None) => {}
        }
        let abandoned = terminal
            && !activity.viewed
            && report_result(
                &mut abandonment_failure,
                "Task abandonment",
                store.owner_task_abandoned(owner),
            )
            .unwrap_or(false);
        // Abandonment ends even live work; only completed-Task closure
        // needs a retention judgment.
        if !abandoned {
            if !report_result(
                &mut completion_failure,
                "Task completion",
                owner_task_completed(store, owner),
            )
            .unwrap_or(false)
            {
                continue;
            }
            let Some(retention) = report_result(
                &mut retention_failure,
                "Session retention, retrying",
                store.owner_retention(owner),
            ) else {
                continue;
            };
            if retained != retention {
                if let Some(reason) = retention {
                    tracing::warn!(%owner, "retaining Session during Task cleanup: {reason}");
                }
                retained = retention;
            }
            if retention.is_some() {
                continue;
            }
        }
        if end.send(Ending { activity }).is_err() {
            return;
        }
        let _ = waker.write_all(&[1]);
        // Input accepted since the judgment can veto it in the serving owner.
        // Keep judging until that owner exits or drops this channel.
    }
}

/// Each operation keeps its own gap until that operation succeeds. Other policy
/// work can succeed during an outage without closing or reopening it.
pub(super) fn report_result<T>(
    gap: &mut RecordingGap,
    what: &str,
    result: Result<T, impl std::fmt::Display>,
) -> Option<T> {
    result
        .inspect(|_| gap.recovered())
        .inspect_err(|error| gap.note(what, error))
        .ok()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{Activity, Ending};

    #[test]
    fn abandonment_ends_background_output_but_keeps_viewed_or_queued_input() {
        let guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::process_ledger().unwrap();
        let conn = rusqlite::Connection::open(guard.home().join("loopflow.db")).unwrap();
        let repo = store.ensure_repository("/repo").unwrap();
        let task = crate::durable::TaskId::new();
        conn.execute("INSERT INTO tasks(id,repository_id,issue_identifier,created_at,abandoned_at) VALUES(?1,?2,'STOP-1',1,2)", rusqlite::params![task.as_str(), repo.as_str()]).unwrap();
        let owner = store.test_lf_process();
        store.test_session("background", &crate::session_record::new_artifact_key());
        conn.execute("UPDATE agent_sessions SET task_id=?1,interactive=1,transport_lf_process_id=?2 WHERE id='background'",
            rusqlite::params![task.as_str(), owner]).unwrap();
        let candidate = Ending {
            activity: Activity::default(),
        };
        let active = Activity {
            output: 1,
            ..Activity::default()
        };
        assert!(candidate.confirm(&owner, active, false).unwrap().is_some());
        assert!(candidate.confirm(&owner, active, true).unwrap().is_none());
        assert!(candidate
            .confirm(
                &owner,
                Activity {
                    viewed: true,
                    ..active
                },
                false
            )
            .unwrap()
            .is_none());
        conn.execute(
            "UPDATE tasks SET abandoned_at=NULL WHERE id=?1",
            [task.as_str()],
        )
        .unwrap();
        assert!(candidate.confirm(&owner, active, false).unwrap().is_none());
    }

    #[test]
    fn queued_endings_require_current_output_and_viewer_evidence() {
        let mut activity = Activity::default();
        let candidate = Ending { activity };
        assert!(candidate.may_end(activity, false));
        activity.view(true);
        assert!(!candidate.may_end(activity, false));
        activity.view(false);
        // A changed viewer invalidates a queued retirement judgment.
        assert!(!candidate.may_end(activity, false));
        activity.output += 1;
        assert!(!candidate.may_end(activity, false));
        let fresh = Ending { activity };
        assert!(fresh.may_end(activity, false));
        assert!(!fresh.may_end(activity, true));
    }

    #[test]
    fn completed_task_waits_for_final_work_and_pending_requests() {
        let guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::process_ledger().unwrap();
        let owner = store.test_lf_process();
        let conn = rusqlite::Connection::open(guard.home().join("loopflow.db")).unwrap();
        let repo = store.ensure_repository("/repo").unwrap();
        let task = crate::durable::TaskId::new();
        conn.execute("INSERT INTO tasks(id,repository_id,issue_identifier,created_at,planning_completed,planning_completed_at) VALUES(?1,?2,'CLOSE-1',1,1,'2000-01-01T00:00:00Z')", rusqlite::params![task.as_str(),repo.as_str()]).unwrap();
        store.test_session("session", &crate::session_record::new_artifact_key());
        conn.execute("UPDATE agent_sessions SET task_id=?1,interactive=1,transport_lf_process_id=?2,agent_process_id=?2 WHERE id='session'", rusqlite::params![task.as_str(),owner]).unwrap();
        conn.execute("INSERT INTO session_activity(session_id,agent_process_id,observed_at,open_tools,pending_input,yielded) VALUES('session',?1,1,1,0,0)", [&owner]).unwrap();
        // Silent work remains protected past the viewing deadline, including
        // a queued ending whose activity never saw another output byte.
        let candidate = Ending {
            activity: Activity::default(),
        };
        for (tools, pending, yielded) in [(1, 0, 0), (0, 1, 1), (0, 0, 0)] {
            conn.execute(
                "UPDATE session_activity SET open_tools=?1,pending_input=?2,yielded=?3",
                rusqlite::params![tools, pending, yielded],
            )
            .unwrap();
            assert!(candidate
                .confirm(&owner, Activity::default(), false)
                .unwrap()
                .is_none());
        }
        conn.execute(
            "UPDATE session_activity SET open_tools=0,pending_input=0,yielded=1",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE agent_sessions SET interactive=0 WHERE id='session'",
            [],
        )
        .unwrap();
        assert!(candidate
            .confirm(&owner, Activity::default(), false)
            .unwrap()
            .is_none());
        conn.execute(
            "UPDATE agent_sessions SET interactive=1 WHERE id='session'",
            [],
        )
        .unwrap();
        assert!(candidate
            .confirm(&owner, Activity::default(), true)
            .unwrap()
            .is_none());
        // The Task's Graph may run outside this owner's ancestry. A queued
        // retirement must still wait for it, without affecting explicit rollover.
        let graph = store.test_lf_process();
        conn.execute(
            "INSERT INTO task_graphs(task_id,graph,step,run_start,lf_process_id,updated_at)
            VALUES(?1,'{\"name\":\"test\",\"steps\":[]}','start','{\"index\":0}',?2,1)",
            rusqlite::params![task.as_str(), graph],
        )
        .unwrap();
        assert_eq!(store.owner_retention(&owner).unwrap(), None);
        assert!(candidate
            .confirm(&owner, Activity::default(), false)
            .unwrap()
            .is_none());
        conn.execute("UPDATE processes SET completed_at=1 WHERE id=?1", [&graph])
            .unwrap();
        assert!(candidate
            .confirm(&owner, Activity::default(), false)
            .unwrap()
            .is_some());
        assert!(store.task_retirement(&task).unwrap().is_none());
        // Another conversation's pending request holds the whole Task, including
        // when its Session belongs through the checkout instead of a binding.
        let other = store.test_lf_process();
        let live = crate::journal::OsProcess::read(std::process::id())
            .unwrap()
            .unwrap();
        conn.execute(
            "UPDATE processes SET pid=?2,os_started_at=?3 WHERE id=?1",
            rusqlite::params![other, live.pid, live.started_at],
        )
        .unwrap();
        store.test_session("other", &crate::session_record::new_artifact_key());
        conn.execute("UPDATE tasks SET worktree='/repo.task',checkout_machine_id=(SELECT id FROM machines WHERE is_local=1) WHERE id=?1", [task.as_str()]).unwrap();
        conn.execute("UPDATE agent_sessions SET cwd='/repo.task/nested',interactive=1,transport_lf_process_id=?1,agent_process_id=?1 WHERE id='other'", [&other]).unwrap();
        conn.execute("INSERT INTO session_activity(session_id,agent_process_id,observed_at,open_tools,pending_input,yielded) VALUES('other',?1,1,0,1,1)", [&other]).unwrap();
        assert!(candidate
            .confirm(&owner, Activity::default(), false)
            .unwrap()
            .is_none());
        conn.execute(
            "UPDATE agent_sessions SET completed_at=1 WHERE id='other'",
            [],
        )
        .unwrap();
        assert!(candidate
            .confirm(&owner, Activity::default(), false)
            .unwrap()
            .is_some());
        // Explicit Close of a shell also respects a nested requested turn.
        store
            .create_desktop_shell("shell", &task, "/repo.task")
            .unwrap();
        store.bind_desktop_shell("shell", &owner).unwrap();
        conn.execute(
            "UPDATE tasks SET worktree='/repo.task',branch='task' WHERE id=?1",
            [task.as_str()],
        )
        .unwrap();
        let intent = store
            .reconcile_task_retirement(&task, 0, None)
            .unwrap()
            .unwrap();
        store.retirement_progress(&intent, "closing", None).unwrap();
        for close in [1, 2] {
            conn.execute("UPDATE desktop_shells SET close_requested=?1", [close])
                .unwrap();
            // A shell view adds no retention, for explicit or retirement Close.
            let viewed = Activity {
                viewed: true,
                ..Activity::default()
            };
            conn.execute("UPDATE session_activity SET open_tools=1,yielded=0", [])
                .unwrap();
            assert!(candidate.confirm(&owner, viewed, false).unwrap().is_none());
            conn.execute("UPDATE session_activity SET open_tools=0,yielded=1", [])
                .unwrap();
            assert!(candidate.confirm(&owner, viewed, false).unwrap().is_some());
        }
    }

    #[test]
    fn completed_task_closure_rechecks_scope_grace_and_store_availability() {
        let guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::process_ledger().unwrap();
        let conn = rusqlite::Connection::open(guard.home().join("loopflow.db")).unwrap();
        let repo = store.ensure_repository("/repo").unwrap();
        let task = crate::durable::TaskId::new();
        conn.execute("INSERT INTO tasks(id,repository_id,issue_identifier,created_at,planning_completed,planning_completed_at)
            VALUES(?1,?2,'CLOSE-2',1,1,'2000-01-01T00:00:00Z')", rusqlite::params![task.as_str(),repo.as_str()]).unwrap();
        let owner = store.test_lf_process();
        store.test_session("session", &crate::session_record::new_artifact_key());
        conn.execute("UPDATE agent_sessions SET task_id=?1,interactive=1,transport_lf_process_id=?2 WHERE id='session'",
            rusqlite::params![task.as_str(),owner]).unwrap();
        let activity = Activity::default();
        let candidate = Ending { activity };
        assert!(candidate
            .confirm(&owner, activity, false)
            .unwrap()
            .is_some());
        for change in [
            "UPDATE tasks SET planning_completed=0",
            "UPDATE tasks SET planning_completed_at=strftime('%Y-%m-%dT%H:%M:%SZ','now')",
            "UPDATE agent_sessions SET transport_lf_process_id=NULL",
            "UPDATE agent_sessions SET primary_scope='repository'",
            "UPDATE agent_sessions SET primary_scope='wave'",
            "UPDATE processes SET started_at=unixepoch()",
            "UPDATE processes SET viewer_hold='absent',viewer_hold_at=unixepoch()",
        ] {
            conn.execute_batch(change).unwrap();
            assert!(
                candidate
                    .confirm(&owner, activity, false)
                    .unwrap()
                    .is_none(),
                "{change}"
            );
            conn.execute_batch("UPDATE tasks SET planning_completed=1,planning_completed_at='2000-01-01T00:00:00Z';
                UPDATE agent_sessions SET primary_scope=NULL;
                UPDATE processes SET started_at=1,viewer_hold=NULL,viewer_hold_at=NULL;").unwrap();
            conn.execute(
                "UPDATE agent_sessions SET transport_lf_process_id=?1",
                [&owner],
            )
            .unwrap();
        }
        conn.execute_batch("UPDATE tasks SET planning_completed_at=NULL,updated_at=unixepoch()")
            .unwrap();
        assert!(candidate
            .confirm(&owner, activity, false)
            .unwrap()
            .is_none());
        conn.execute_batch("UPDATE tasks SET updated_at=1").unwrap();
        assert!(candidate
            .confirm(&owner, activity, false)
            .unwrap()
            .is_some());
        assert!(store.task_retirement(&task).unwrap().is_none());
        // An unreadable store cannot authorize an already queued ending.
        conn.execute_batch("DROP TABLE task_graphs").unwrap();
        assert!(candidate.confirm(&owner, activity, false).is_err());
    }

    #[test]
    fn viewer_state_retries_after_a_store_failure_without_another_viewer_change() {
        let guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::process_ledger().unwrap();
        let owner = store.test_lf_process();
        let db = guard.home().join("loopflow.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        let repo = store.ensure_repository("/repo").unwrap();
        let task = crate::durable::TaskId::new();
        conn.execute("INSERT INTO tasks(id,repository_id,issue_identifier,created_at,planning_completed,planning_completed_at)
            VALUES(?1,?2,'RETRY-1',1,1,'2000-01-01T00:00:00Z')", rusqlite::params![task.as_str(),repo.as_str()]).unwrap();
        store.test_session("session", &crate::session_record::new_artifact_key());
        conn.execute("UPDATE agent_sessions SET task_id=?1,interactive=1,transport_lf_process_id=?2 WHERE id='session'",
            rusqlite::params![task.as_str(),owner]).unwrap();
        conn.execute(
            "UPDATE processes SET viewed=1 WHERE id=?1",
            [owner.as_str()],
        )
        .unwrap();
        conn.execute_batch(
            "CREATE TRIGGER reject_viewer_change BEFORE UPDATE OF viewed ON processes
             BEGIN SELECT RAISE(FAIL, 'temporary viewer write failure'); END;",
        )
        .unwrap();
        let (activity, changes) = std::sync::mpsc::channel();
        let (end, ending) = std::sync::mpsc::channel();
        let (_wake, waker) = std::os::unix::net::UnixStream::pair().unwrap();
        let watched = owner.clone();
        let policy = std::thread::spawn(move || {
            crate::journal::with_test_ledger(db, || {
                super::watch(&watched, false, &changes, &end, &waker);
            });
        });
        activity
            .send(Activity {
                viewed: false,
                viewers: 2,
                output: 0,
                hold: Some(crate::store::sqlite::ViewerHold::Absent(0)),
            })
            .unwrap();
        std::thread::sleep(super::POLL * 2);
        assert!(
            ending.try_recv().is_err(),
            "failed viewing writes cannot authorize closure"
        );
        conn.execute_batch("DROP TRIGGER reject_viewer_change")
            .unwrap();
        let deadline = std::time::Instant::now() + super::POLL * 3;
        let mut viewed = true;
        while viewed && std::time::Instant::now() < deadline {
            viewed = conn
                .query_row("SELECT viewed FROM processes WHERE id=?1", [&owner], |r| {
                    r.get(0)
                })
                .unwrap();
            std::thread::sleep(Duration::from_millis(20));
        }
        let recovered = ending.recv_timeout(super::POLL * 2);
        drop(activity);
        policy.join().unwrap();
        assert!(
            recovered.is_ok(),
            "closure retries after recording recovers"
        );
        assert!(
            !viewed,
            "a recovered store must not retain stale viewer state"
        );
    }

    #[test]
    fn automatic_shell_close_rechecks_task_viewing_before_consuming_stale_intent() {
        let guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::process_ledger().unwrap();
        let conn = rusqlite::Connection::open(guard.home().join("loopflow.db")).unwrap();
        let repo = store.ensure_repository("/repo").unwrap();
        let task = crate::durable::TaskId::new();
        conn.execute("INSERT INTO tasks(id,repository_id,issue_identifier,created_at,worktree,branch,planning_completed,planning_completed_at) VALUES(?1,?2,'CLOSE-1',1,'/repo.task','task',1,'completed')", rusqlite::params![task.as_str(),repo.as_str()]).unwrap();
        let shell = store.test_lf_process();
        store
            .create_desktop_shell("shell", &task, "/repo.task")
            .unwrap();
        store.bind_desktop_shell("shell", &shell).unwrap();
        let viewer = store.test_lf_process();
        let live = crate::journal::OsProcess::read(std::process::id())
            .unwrap()
            .unwrap();
        conn.execute(
            "UPDATE processes SET pid=?2,started_at=?3,os_started_at=?3 WHERE id=?1",
            rusqlite::params![viewer, live.pid, live.started_at],
        )
        .unwrap();
        store.test_session("passive", &crate::session_record::new_artifact_key());
        conn.execute("UPDATE agent_sessions SET task_id=?1,interactive=1,transport_lf_process_id=?2 WHERE id='passive'",
            rusqlite::params![task.as_str(),viewer]).unwrap();
        let intent = store
            .reconcile_task_retirement(&task, 0, None)
            .unwrap()
            .unwrap();
        store.retirement_progress(&intent, "closing", None).unwrap();
        crate::desktop::shells::request_retirement(&store, &task).unwrap();
        let candidate = Ending {
            activity: Activity::default(),
        };
        // A shell's own viewer adds no Task retention, and output cannot keep
        // an ordinary shell past an eligible automatic close.
        store
            .record_owner_viewer_hold(
                &shell,
                Some(crate::store::sqlite::ViewerHold::Viewed(
                    time::OffsetDateTime::now_utc().unix_timestamp(),
                )),
            )
            .unwrap();
        let viewed = Activity {
            viewed: true,
            output: 1,
            ..Activity::default()
        };
        assert!(candidate.confirm(&shell, viewed, false).unwrap().is_some());
        // Seed stale intent directly: the final check must protect even if a
        // writer did not atomically cancel it. No sweep runs in this test.
        for hold in ["viewed", "absent"] {
            conn.execute(
                "UPDATE processes SET viewer_hold=?2,viewer_hold_at=unixepoch() WHERE id=?1",
                rusqlite::params![viewer, hold],
            )
            .unwrap();
            assert!(candidate.confirm(&shell, viewed, false).unwrap().is_none());
        }
        store.close_desktop_shell("shell").unwrap();
        assert!(candidate.confirm(&shell, viewed, false).unwrap().is_some());
        // Unreadable retirement state cannot authorize automatic closure.
        conn.execute("UPDATE desktop_shells SET close_requested=2", [])
            .unwrap();
        conn.execute(
            "UPDATE processes SET viewer_hold='absent',viewer_hold_at=0 WHERE id=?1",
            [&viewer],
        )
        .unwrap();
        assert!(candidate.confirm(&shell, viewed, false).unwrap().is_some());
        conn.execute_batch("DROP TABLE task_retirements").unwrap();
        assert!(candidate.confirm(&shell, viewed, false).is_err());
    }
}
