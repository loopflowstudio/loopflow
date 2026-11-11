//! When a Session owner ends itself.
//!
//! A terminal owner nobody views ends after `session.end_detached_after` of
//! quiet: no viewer, no output from its command, and nothing in the store
//! holding it (a turn, pending request, running Graph, a Task conversation's
//! running Task Graph, or a headless Run).
//! A held terminal has no such grace: it runs until its Task is abandoned or
//! its completed Task's workspace retires, or its agent exits.
//! Task abandonment ends unviewed conversations even during a turn. Completed
//! Task retirement can also end eligible owners after its absence grace. Desktop
//! shells use Close and retirement policy instead of idle-agent ending.
//! A removed, rebuilt or superseded lf executable leaves its owner serving;
//! `lf resume` or `lf monitor rollover` move it when a person or operator decides to. Ending
//! writes the last screen, stops the agent and records the exit, so `lf resume`
//! reopens the conversation from provider history.
//!
//! The clock is wall time: a laptop asleep overnight ends its idle Sessions at
//! the first poll after it wakes. A store that cannot be read is judged again
//! next time; the policy never stops on an error.

use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime};

use anyhow::Result;

use super::wire::Event;
use super::Host;
use crate::config::DetachedLimit;
use crate::ops::task::retirement::owner_retiring;
use crate::store::sqlite::sessions::OwnerRetention;
use crate::store::sqlite::ViewerHold;
use crate::store::RecordingGap;

/// How often an owner looks at the clock.
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

    /// Activity the idle grace measures; checkout holds do not restart it.
    fn idle_evidence(&self) -> (bool, u64, u64) {
        (self.viewed, self.viewers, self.output)
    }
}

/// An ending candidate carries the activity the policy actually judged when
/// the detached grace expired.
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
        owner: &Host,
        activity: Activity,
        input_queued: bool,
    ) -> Result<Option<Event>> {
        let store = crate::journal::process_ledger()?;
        let shell = match owner.owner() {
            Some(owner) => crate::desktop::shells::close_allowed(&store, owner, input_queued)?,
            None => None,
        };
        let owner = &owner.transport();
        if let Some(may_close) = shell {
            return Ok(may_close.then_some(Event::Control {
                held: false,
                notice: Some("Shell closed".into()),
            }));
        }
        if !activity.viewed && !input_queued && store.owner_task_abandoned(*owner)? {
            return Ok(Some(Event::Control {
                held: false,
                notice: Some("Task abandoned".into()),
            }));
        }
        let held = matches!(owner, crate::store::sqlite::Transport::Terminal(_));
        if !self.may_end(activity, input_queued)
            || (!held && store.idle_retention(*owner)?.is_some())
        {
            return Ok(None);
        }
        if owner_retiring(&store, *owner)? {
            return Ok(Some(Event::Control {
                held: false,
                notice: Some("Completed Task workspace retired".into()),
            }));
        }
        // A held terminal has no idle ending: it runs until its scope closes
        // or its agent exits.
        if held {
            return Ok(None);
        }
        tracing::info!(%owner, "ending a Session nobody has viewed");
        Ok(Some(Event::Control {
            held: false,
            notice: None,
        }))
    }
}

/// One thread judging every owner a host watches: the Machine server's held
/// terminals share one, a Session owner process has its own. It ends once its
/// handle and every watched owner's activity channel are gone.
#[derive(Debug)]
pub(super) struct Watcher {
    owners: mpsc::Sender<Watch>,
}

impl Watcher {
    pub fn spawn() -> Self {
        let (owners, added) = mpsc::channel();
        std::thread::Builder::new()
            .name("lifetime".into())
            .spawn(move || judge(&added))
            .expect("spawn the lifetime thread");
        Self { owners }
    }

    /// Record activity sent on the first channel. Once the owner has gone
    /// unviewed and quiet past its limit, send the second channel a candidate
    /// for the serve loop to confirm before ending the owner, then wake it
    /// through `waker`. Only a terminal owner (`terminal`) ends for want of
    /// viewers; a pipe owner is a headless Run.
    pub fn watch(
        &self,
        owner: Host,
        terminal: bool,
        waker: UnixStream,
    ) -> (mpsc::Sender<Activity>, mpsc::Receiver<Ending>) {
        #[cfg(test)]
        if let Some(channels) = super::lifetime_tests::channels() {
            return channels;
        }
        let (activity, changes) = mpsc::channel();
        let (end, ending) = mpsc::channel();
        let _ = self
            .owners
            .send(Watch::new(owner, terminal, changes, end, waker));
        (activity, ending)
    }
}

/// When an unviewed, quiet owner is due to end, in wall time.
#[derive(Debug)]
struct Grace {
    /// Unknown until the store names the owner's repository; a pipe owner
    /// never learns one and never ends for want of viewers.
    limit: Option<DetachedLimit>,
    activity: Activity,
    quiet_since: SystemTime,
}

impl Grace {
    fn note(&mut self, activity: Activity, now: SystemTime) {
        if activity.idle_evidence() != self.activity.idle_evidence() {
            self.quiet_since = now;
        }
        self.activity = activity;
    }

    fn expired(&self, now: SystemTime) -> bool {
        match self.limit {
            Some(DetachedLimit::After(limit)) if !self.activity.viewed => {
                now >= self.quiet_since + limit
            }
            _ => false,
        }
    }
}

/// Activity arrives at any rate but is read once per poll: it moves the clock,
/// never the file or store work.
fn judge(added: &mpsc::Receiver<Watch>) {
    let mut watches: Vec<Watch> = Vec::new();
    let mut adding = true;
    let mut store = None;
    let mut store_failure = RecordingGap::default();
    let mut next_poll = Instant::now() + POLL;
    loop {
        if adding {
            match added.recv_timeout(next_poll.saturating_duration_since(Instant::now())) {
                Ok(watch) => watches.push(watch),
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => adding = false,
            }
        } else if watches.is_empty() {
            return;
        } else {
            std::thread::sleep(next_poll.saturating_duration_since(Instant::now()));
        }
        if Instant::now() < next_poll {
            continue;
        }
        next_poll = Instant::now() + POLL;
        watches.retain_mut(Watch::receive);
        if watches.is_empty() {
            continue;
        }
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
        watches.retain_mut(|watch| watch.judge(store));
    }
}

/// One watched owner's activity and the store reads that judge it.
#[derive(Debug)]
struct Watch {
    host: Host,
    terminal: bool,
    changes: mpsc::Receiver<Activity>,
    end: mpsc::Sender<Ending>,
    waker: UnixStream,
    grace: Grace,
    /// None means no observation has been committed, including initial absence.
    recorded_hold: Option<Option<ViewerHold>>,
    retained: Option<OwnerRetention>,
    hold_failure: RecordingGap,
    shell_failure: RecordingGap,
    repository_failure: RecordingGap,
    retention_failure: RecordingGap,
    abandonment_failure: RecordingGap,
}

impl Watch {
    fn new(
        host: Host,
        terminal: bool,
        changes: mpsc::Receiver<Activity>,
        end: mpsc::Sender<Ending>,
        waker: UnixStream,
    ) -> Self {
        Self {
            host,
            terminal,
            changes,
            end,
            waker,
            grace: Grace {
                limit: None,
                activity: Activity::default(),
                quiet_since: SystemTime::now(),
            },
            recorded_hold: None,
            retained: None,
            hold_failure: RecordingGap::default(),
            shell_failure: RecordingGap::default(),
            repository_failure: RecordingGap::default(),
            retention_failure: RecordingGap::default(),
            abandonment_failure: RecordingGap::default(),
        }
    }

    /// Note queued activity; false once the serving owner has gone.
    fn receive(&mut self) -> bool {
        loop {
            match self.changes.try_recv() {
                Ok(activity) => self.grace.note(activity, SystemTime::now()),
                Err(mpsc::TryRecvError::Empty) => return true,
                Err(mpsc::TryRecvError::Disconnected) => return false,
            }
        }
    }

    /// Offer an ending when one is due; false once the serving owner has gone.
    fn judge(&mut self, store: &crate::store::sqlite::SqliteStore) -> bool {
        let owner = self.host.transport();
        let hold = self.grace.activity.hold;
        if self.recorded_hold != Some(hold)
            && report_result(
                &mut self.hold_failure,
                "Session viewer hold",
                store.record_owner_viewer_hold(owner, hold),
            )
            .is_some()
        {
            self.recorded_hold = Some(hold);
        }
        let held = matches!(self.host, Host::Terminal(_));
        if held {
            self.grace.limit = Some(DetachedLimit::Off);
        } else if self.terminal && self.grace.limit.is_none() {
            self.grace.limit = report_result(
                &mut self.repository_failure,
                "Session repository",
                store.host_repository(owner),
            )
            .map(|repo| limit(repo.as_deref()));
        }
        // Shell purpose is fixed at creation. Running lf inside an ordinary
        // shell must never enroll it in the idle-agent policy.
        let shell = match self.host.owner() {
            Some(owner) => store.shell_close_request(owner),
            None => Ok(None),
        };
        match report_result(&mut self.shell_failure, "Shell close request", shell) {
            Some(Some(1 | 2)) => return self.offer(),
            Some(Some(_)) | None => return true,
            Some(None) => {}
        }
        let now = SystemTime::now();
        let abandoned = self.terminal
            && !self.grace.activity.viewed
            && report_result(
                &mut self.abandonment_failure,
                "Task abandonment",
                store.owner_task_abandoned(owner),
            )
            .unwrap_or(false);
        let retiring = owner_retiring(store, owner).unwrap_or(false);
        if !abandoned && !retiring && !self.grace.expired(now) {
            return true;
        }
        if held {
            return self.offer();
        }
        let Some(retention) = report_result(
            &mut self.retention_failure,
            "Session retention, retrying",
            store.idle_retention(owner),
        ) else {
            return true;
        };
        if self.retained != retention {
            if let Some(reason) = retention {
                tracing::warn!(%owner, "retaining detached Session past its limit: {reason}");
            }
            self.retained = retention;
        }
        // Retained work counts as activity: the grace runs again from here.
        if !abandoned && retention.is_some() {
            self.grace.quiet_since = now;
            return true;
        }
        // Input accepted since the judgment can veto it in the serving owner.
        // Keep judging until that owner exits or drops its channels.
        self.offer()
    }

    fn offer(&mut self) -> bool {
        if self
            .end
            .send(Ending {
                activity: self.grace.activity,
            })
            .is_err()
        {
            return false;
        }
        let _ = (&self.waker).write_all(&[1]);
        true
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

/// The detached limit configured for `repo`. An unreadable configuration
/// keeps the default rather than keeping the Session.
fn limit(repo: Option<&str>) -> DetachedLimit {
    match crate::config::load_config(repo.map(Path::new)) {
        Ok(config) => config.unwrap_or_default().session.end_detached_after,
        Err(error) => {
            tracing::warn!(%error, ?repo, "configuration unreadable; using the default detached limit");
            DetachedLimit::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    use super::{Activity, DetachedLimit, Ending, Grace};

    /// Judge one owner on this thread until its activity channel closes.
    fn judge_alone(
        host: super::Host,
        terminal: bool,
        changes: std::sync::mpsc::Receiver<Activity>,
        end: std::sync::mpsc::Sender<Ending>,
        waker: std::os::unix::net::UnixStream,
    ) {
        let (owners, added) = std::sync::mpsc::channel();
        owners
            .send(super::Watch::new(host, terminal, changes, end, waker))
            .unwrap();
        drop(owners);
        super::judge(&added);
    }

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
        assert!(candidate
            .confirm(&super::Host::Owner(owner.clone()), active, false)
            .unwrap()
            .is_some());
        assert!(candidate
            .confirm(&super::Host::Owner(owner.clone()), active, true)
            .unwrap()
            .is_none());
        assert!(candidate
            .confirm(
                &super::Host::Owner(owner.clone()),
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
        assert!(candidate
            .confirm(&super::Host::Owner(owner.clone()), active, false)
            .unwrap()
            .is_none());
    }

    #[test]
    fn queued_endings_require_current_output_and_idle_viewer_evidence() {
        let mut activity = Activity::default();
        let candidate = Ending { activity };
        assert!(candidate.may_end(activity, false));
        activity.view(true);
        assert!(!candidate.may_end(activity, false));
        activity.view(false);
        // Attaching and leaving again still restarts the idle grace.
        assert!(!candidate.may_end(activity, false));
        activity.output += 1;
        assert!(!candidate.may_end(activity, false));
        let fresh = Ending { activity };
        assert!(fresh.may_end(activity, false));
        assert!(!fresh.may_end(activity, true));
    }

    #[test]
    fn workspace_retirement_waits_for_final_work_drafts_and_pending_requests() {
        let guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::process_ledger().unwrap();
        let owner = store.test_lf_process();
        let conn = rusqlite::Connection::open(guard.home().join("loopflow.db")).unwrap();
        let repo = store.ensure_repository("/repo").unwrap();
        let task = crate::durable::TaskId::new();
        conn.execute("INSERT INTO tasks(id,repository_id,issue_identifier,created_at,worktree,branch,planning_completed,planning_completed_at) VALUES(?1,?2,'CLOSE-1',1,'/repo.task','task',1,'completed')", rusqlite::params![task.as_str(),repo.as_str()]).unwrap();
        store.test_session("session", &crate::session_record::new_artifact_key());
        conn.execute("UPDATE agent_sessions SET task_id=?1,interactive=1,transport_lf_process_id=?2,agent_process_id=?2 WHERE id='session'", rusqlite::params![task.as_str(),owner]).unwrap();
        conn.execute("INSERT INTO session_activity(session_id,agent_process_id,observed_at,open_tools,pending_input,yielded) VALUES('session',?1,1,1,0,0)", [&owner]).unwrap();
        let intent = store
            .reconcile_task_retirement(&task, 0, None)
            .unwrap()
            .unwrap();
        assert_eq!(intent.due_at, Some(900));
        // Silent work remains protected long after both deadlines, including
        // a queued ending whose activity never saw another output byte.
        let candidate = Ending {
            activity: Activity::default(),
        };
        store.reconcile_task_retirement(&task, 3600, None).unwrap();
        store.retirement_progress(&intent, "closing", None).unwrap();
        for (tools, pending, yielded) in [(1, 0, 0), (0, 1, 1), (0, 0, 0)] {
            conn.execute(
                "UPDATE session_activity SET open_tools=?1,pending_input=?2,yielded=?3",
                rusqlite::params![tools, pending, yielded],
            )
            .unwrap();
            assert!(candidate
                .confirm(
                    &super::Host::Owner(owner.clone()),
                    Activity::default(),
                    false
                )
                .unwrap()
                .is_none());
        }
        conn.execute(
            "UPDATE session_activity SET open_tools=0,pending_input=0,yielded=1",
            [],
        )
        .unwrap();
        assert!(store
            .try_record_owner_input(&owner, b"unsubmitted draft")
            .unwrap());
        assert!(candidate
            .confirm(
                &super::Host::Owner(owner.clone()),
                Activity::default(),
                false
            )
            .unwrap()
            .is_none());
        store.clear_owner_input(&owner).unwrap();
        conn.execute(
            "UPDATE agent_sessions SET interactive=0 WHERE id='session'",
            [],
        )
        .unwrap();
        assert!(candidate
            .confirm(
                &super::Host::Owner(owner.clone()),
                Activity::default(),
                false
            )
            .unwrap()
            .is_none());
        conn.execute(
            "UPDATE agent_sessions SET interactive=1 WHERE id='session'",
            [],
        )
        .unwrap();
        assert!(candidate
            .confirm(
                &super::Host::Owner(owner.clone()),
                Activity::default(),
                true
            )
            .unwrap()
            .is_none());
        assert!(candidate
            .confirm(
                &super::Host::Owner(owner.clone()),
                Activity::default(),
                false
            )
            .unwrap()
            .is_some());
        assert_eq!(
            store.task_retirement(&task).unwrap().unwrap().due_at,
            Some(900)
        );
        // Explicit Close of a shell also respects a nested requested turn.
        store
            .create_desktop_shell("shell", &task, "/repo.task")
            .unwrap();
        store.bind_desktop_shell("shell", &owner).unwrap();
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
            assert!(candidate
                .confirm(&super::Host::Owner(owner.clone()), viewed, false)
                .unwrap()
                .is_none());
            conn.execute("UPDATE session_activity SET open_tools=0,yielded=1", [])
                .unwrap();
            assert!(candidate
                .confirm(&super::Host::Owner(owner.clone()), viewed, false)
                .unwrap()
                .is_some());
        }
    }

    #[test]
    fn viewer_state_retries_after_a_store_failure_without_another_viewer_change() {
        let guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::process_ledger().unwrap();
        let owner = store.test_lf_process();
        let db = guard.home().join("loopflow.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
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
        let (end, _ending) = std::sync::mpsc::channel();
        let (_wake, waker) = std::os::unix::net::UnixStream::pair().unwrap();
        let watched = owner.clone();
        let policy = std::thread::spawn(move || {
            crate::journal::with_test_ledger(db, || {
                judge_alone(
                    super::Host::Owner(watched.clone()),
                    false,
                    changes,
                    end,
                    waker,
                );
            });
        });
        activity
            .send(Activity {
                viewed: false,
                viewers: 2,
                output: 0,
                hold: None,
            })
            .unwrap();
        std::thread::sleep(super::POLL * 2);
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
        drop(activity);
        policy.join().unwrap();
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
        assert!(candidate
            .confirm(&super::Host::Owner(shell.clone()), viewed, false)
            .unwrap()
            .is_some());
        // Seed stale intent directly: the final check must protect even if a
        // writer did not atomically cancel it. No sweep runs in this test.
        for hold in ["viewed", "absent"] {
            conn.execute(
                "UPDATE processes SET viewer_hold=?2,viewer_hold_at=unixepoch() WHERE id=?1",
                rusqlite::params![viewer, hold],
            )
            .unwrap();
            assert!(candidate
                .confirm(&super::Host::Owner(shell.clone()), viewed, false)
                .unwrap()
                .is_none());
        }
        store.close_desktop_shell("shell").unwrap();
        assert!(candidate
            .confirm(&super::Host::Owner(shell.clone()), viewed, false)
            .unwrap()
            .is_some());
        // Unreadable retirement state cannot authorize automatic closure.
        conn.execute("UPDATE desktop_shells SET close_requested=2", [])
            .unwrap();
        conn.execute(
            "UPDATE processes SET viewer_hold='absent',viewer_hold_at=0 WHERE id=?1",
            [&viewer],
        )
        .unwrap();
        assert!(candidate
            .confirm(&super::Host::Owner(shell.clone()), viewed, false)
            .unwrap()
            .is_some());
        conn.execute_batch("DROP TABLE task_retirements").unwrap();
        assert!(candidate
            .confirm(&super::Host::Owner(shell.clone()), viewed, false)
            .is_err());
    }

    #[test]
    fn output_restarts_the_grace_then_quiet_allows_ending() {
        let guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::process_ledger().unwrap();
        let owner = store.test_lf_process();
        let db = guard.home().join("loopflow.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        store.test_session("session", &crate::session_record::new_artifact_key());
        conn.execute(
            "UPDATE agent_sessions SET interactive=1, transport_lf_process_id=?1 WHERE id='session'",
            [owner.as_str()],
        ).unwrap();
        std::fs::write(
            guard.home().join("config.yaml"),
            "session:\n  end_detached_after: 2s\n",
        )
        .unwrap();
        // An unreadable Process must not latch the global grace in place of
        // the repository's setting. Repair it without activity.
        conn.execute(
            "UPDATE processes SET trace_id='unreadable' WHERE id=?1",
            [&owner],
        )
        .unwrap();
        let (activity, changes) = std::sync::mpsc::channel();
        let (end, ending) = std::sync::mpsc::channel();
        let (_wake, waker) = std::os::unix::net::UnixStream::pair().unwrap();
        let watched = owner.clone();
        let policy = std::thread::spawn(move || {
            crate::journal::with_test_ledger(db, || {
                judge_alone(
                    super::Host::Owner(watched.clone()),
                    true,
                    changes,
                    end,
                    waker,
                );
            });
        });
        assert!(matches!(
            ending.recv_timeout(super::POLL * 2),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ));
        conn.execute("UPDATE processes SET trace_id=?1 WHERE id=?1", [&owner])
            .unwrap();
        let candidate = ending.recv_timeout(Duration::from_secs(8)).unwrap();
        let observed = Activity {
            output: 1,
            ..Activity::default()
        };
        assert!(!candidate.may_end(observed, false));
        activity.send(observed).unwrap();
        assert!(matches!(
            ending.recv_timeout(Duration::from_secs(1)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ));
        let fresh = ending.recv_timeout(Duration::from_secs(8)).unwrap();
        assert!(fresh.may_end(observed, false));
        drop(activity);
        policy.join().unwrap();
    }

    #[test]
    fn one_watcher_ends_each_quiet_owner_and_keeps_the_viewed_one() {
        let guard = crate::journal::TestLedgerGuard::new();
        let store = crate::journal::process_ledger().unwrap();
        let db = guard.home().join("loopflow.db");
        let conn = rusqlite::Connection::open(&db).unwrap();
        std::fs::write(
            guard.home().join("config.yaml"),
            "session:\n  end_detached_after: 2s\n",
        )
        .unwrap();
        let (owners, added) = std::sync::mpsc::channel();
        let mut watched = Vec::new();
        for name in ["quiet", "viewed"] {
            let owner = store.test_lf_process();
            store.test_session(name, &crate::session_record::new_artifact_key());
            conn.execute(
                "UPDATE agent_sessions SET interactive=1, transport_lf_process_id=?1 WHERE id=?2",
                [owner.as_str(), name],
            )
            .unwrap();
            let (activity, changes) = std::sync::mpsc::channel();
            let (end, ending) = std::sync::mpsc::channel();
            let (_wake, waker) = std::os::unix::net::UnixStream::pair().unwrap();
            owners
                .send(super::Watch::new(
                    super::Host::Owner(owner),
                    true,
                    changes,
                    end,
                    waker,
                ))
                .unwrap();
            watched.push((activity, ending, _wake));
        }
        drop(owners);
        let mut viewed = Activity::default();
        viewed.view(true);
        watched[1].0.send(viewed).unwrap();
        let policy = std::thread::spawn(move || {
            crate::journal::with_test_ledger(db, || super::judge(&added))
        });
        assert!(watched[0].1.recv_timeout(Duration::from_secs(8)).is_ok());
        assert!(matches!(
            watched[1].1.try_recv(),
            Err(std::sync::mpsc::TryRecvError::Empty)
        ));
        // The thread ends once every owner it judged has gone.
        drop(watched);
        policy.join().unwrap();
    }

    #[test]
    fn the_grace_runs_from_the_last_viewer_or_output_in_wall_time() {
        let start = SystemTime::UNIX_EPOCH + Duration::from_secs(1_000);
        let mut grace = Grace {
            limit: Some(DetachedLimit::After(Duration::from_secs(120))),
            activity: Activity::default(),
            quiet_since: start,
        };
        assert!(!grace.expired(start + Duration::from_secs(119)));
        // A night asleep counts: the clock, not uptime, decides.
        assert!(grace.expired(start + Duration::from_secs(8 * 3_600)));
        let mut activity = Activity {
            output: 1,
            ..Activity::default()
        };
        grace.note(activity, start + Duration::from_secs(100));
        assert!(!grace.expired(start + Duration::from_secs(200)));
        assert!(grace.expired(start + Duration::from_secs(220)));
        activity.view(true);
        grace.note(activity, start + Duration::from_secs(300));
        assert!(!grace.expired(start + Duration::from_secs(1_000)));
        activity.view(false);
        grace.note(activity, start + Duration::from_secs(400));
        assert!(!grace.expired(start + Duration::from_secs(519)));
        assert!(grace.expired(start + Duration::from_secs(520)));
        // A veto can repeat an already reported snapshot without extending quiet.
        grace.note(activity, start + Duration::from_secs(519));
        assert!(grace.expired(start + Duration::from_secs(520)));
        grace.limit = Some(DetachedLimit::Off);
        assert!(!grace.expired(start + Duration::from_secs(1_000)));
        grace.limit = None;
        assert!(!grace.expired(start + Duration::from_secs(1_000)));
    }
}
