//! Session-viewer holds and one checkout cleanup intent pinned to Task closure.
use rusqlite::{params, Connection, OptionalExtension};

use super::SqliteStore;
use crate::durable::TaskId;
use crate::id::TerminalId;
use crate::store::{StoreError, StoreResult};

pub(crate) const WORKSPACE_GRACE: i64 = 15 * 60;
const VIEWED: &str = "Retained by an open Task Session viewer";

/// What a held terminal's host knows about its viewers, recorded on the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ViewerHold {
    /// A connected viewer, observed at this wall-clock second.
    Viewed(i64),
    /// The last connected viewer left or disconnected at this wall-clock second.
    Absent(i64),
}

impl ViewerHold {
    fn at(self) -> i64 {
        match self {
            Self::Viewed(at) | Self::Absent(at) => at,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Viewed(_) => "viewed",
            Self::Absent(_) => "absent",
        }
    }
}

/// One held terminal's dated viewing evidence, retained after it ends.
#[derive(Debug)]
pub(crate) struct TerminalHold {
    pub terminal: TerminalId,
    pub hold: ViewerHold,
    pub ended: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct TaskRetirement {
    pub task_id: TaskId,
    pub completion: String,
    pub checkout: String,
    pub branch: String,
    pub due_at: Option<i64>,
    pub phase: String,
    pub retry_at: Option<i64>,
}

impl SqliteStore {
    /// Completion and each newly launched held terminal receive the same
    /// viewing grace.
    pub(crate) fn task_close_candidates(
        &self,
        task: &TaskId,
        now: i64,
    ) -> StoreResult<Vec<(TerminalId, bool)>> {
        let due = |viewed: &str| {
            format!(
                "(tw.abandoned_at IS NOT NULL OR (tw.planning_completed=1
                 AND (CASE WHEN tw.planning_completed_at IS NULL THEN tw.updated_at
                     ELSE unixepoch(tw.planning_completed_at) END)+?3<=?2
                 AND {viewed}+?3<=?2
                 AND NOT EXISTS(SELECT 1 FROM task_graphs tg JOIN processes g ON g.id=tg.lf_process_id
                     WHERE tg.task_id=tw.id AND g.completed_at IS NULL)))"
            )
        };
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT DISTINCT t.id,tw.abandoned_at IS NOT NULL FROM tasks tw
             JOIN agent_sessions s ON ({members})
             JOIN held_terminals t ON t.id=s.held_terminal_id
             WHERE tw.id=?1 AND s.interactive=1 AND s.completed_at IS NULL AND t.ended_at IS NULL
               AND {terminal_due}",
            members = super::task_work::session_membership("s"),
            terminal_due = due("COALESCE(t.viewer_hold_at,t.created_at)"),
        ))?;
        let terminals = query
            .query_map(params![task.as_str(), now, super::WORKSPACE_GRACE], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })?
            .collect::<rusqlite::Result<_>>()?;
        Ok(terminals)
    }

    /// Record one observation for both Session activity and checkout retention.
    /// Renewed viewing resets checkout cleanup in this same transaction.
    pub(crate) fn record_viewer_hold(
        &self,
        terminal: &TerminalId,
        hold: Option<ViewerHold>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        tx.execute(
            "UPDATE held_terminals SET viewer_hold=?2,viewer_hold_at=?3,viewed=?4
             WHERE id=?1 AND (viewer_hold IS NOT ?2 OR viewer_hold_at IS NOT ?3 OR viewed IS NOT ?4)",
            params![
                terminal,
                hold.map(ViewerHold::as_str),
                hold.map(ViewerHold::at),
                matches!(hold, Some(ViewerHold::Viewed(_))),
            ],
        )?;
        match hold {
            Some(ViewerHold::Viewed(_)) => hold_viewer_retirements(&tx, terminal, None)?,
            // A return and departure can both reach the recorder between polls.
            Some(ViewerHold::Absent(at)) => hold_viewer_retirements(&tx, terminal, Some(at))?,
            None => {}
        }
        tx.commit()?;
        Ok(())
    }

    /// Only Session viewers retain a checkout. Shell receipts own cleanup, not retention.
    pub(crate) fn task_viewer_holds(&self, task: &TaskId) -> StoreResult<Vec<TerminalHold>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT t.id,t.viewer_hold,t.ended_at IS NOT NULL,t.viewer_hold_at FROM held_terminals t
             WHERE t.viewer_hold IS NOT NULL
               AND t.id IN (SELECT s.held_terminal_id FROM agent_sessions s WHERE s.id IN ({sessions}))
             ORDER BY 1",
            sessions = super::task_work::session_ids("?1")
        ))?;
        let rows = query.query_map([task.as_str()], |r| {
            Ok((
                r.get::<_, TerminalId>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, bool>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (terminal, hold, ended, at) = row?;
            let hold = match hold.as_str() {
                "viewed" => ViewerHold::Viewed(at),
                "absent" => ViewerHold::Absent(at),
                other => {
                    return Err(StoreError::InvalidData(format!(
                        "Unknown viewer hold {other}"
                    )))
                }
            };
            Ok(TerminalHold {
                terminal,
                hold,
                ended,
            })
        })
        .collect()
    }

    pub(crate) fn pending_retirements(&self) -> StoreResult<Vec<TaskId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT task_id FROM task_retirements WHERE phase!='retired'
             UNION SELECT id FROM tasks WHERE abandoned_at IS NOT NULL AND worktree IS NOT NULL
               AND checkout_machine_id=(SELECT id FROM machines WHERE is_local=1)
               AND NOT EXISTS(SELECT 1 FROM task_retirements WHERE task_id=tasks.id)
             UNION SELECT tw.id FROM tasks tw JOIN agent_sessions s ON ({members})
               JOIN held_terminals t ON t.id=s.held_terminal_id
               WHERE (tw.planning_completed=1 OR tw.abandoned_at IS NOT NULL)
                 AND s.completed_at IS NULL AND s.interactive=1 AND t.ended_at IS NULL
             ORDER BY 1",
            members = super::task_work::session_membership("s")
        ))?;
        let tasks = query
            .query_map([], |r| Ok(TaskId::from_raw(r.get::<_, String>(0)?)))?
            .collect::<rusqlite::Result<_>>()?;
        Ok(tasks)
    }

    pub(crate) fn fail_retirement(&self, task: &TaskId, now: i64, reason: &str) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE task_retirements SET reason=?3,retry_at=?2+MIN(3600,60*(1 << MIN(failures,6))),failures=failures+1
             WHERE task_id=?1 AND phase!='retired' AND due_at IS NOT NULL",
            params![task.as_str(), now, reason])?;
        Ok(())
    }

    /// Viewing clears the deadline; absence starts one fifteen-minute interval
    /// that later work completion does not restart. Abandonment needs no grace.
    pub(crate) fn reconcile_task_retirement(
        &self,
        task: &TaskId,
        now: i64,
        hold: Option<ViewerHold>,
    ) -> StoreResult<Option<TaskRetirement>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let Some(abandoned) = tx
            .query_row(
                "SELECT abandoned_at IS NOT NULL FROM tasks WHERE id=?1",
                [task.as_str()],
                |r| r.get::<_, bool>(0),
            )
            .optional()?
        else {
            return Ok(None);
        };
        let grace = if abandoned { 0 } else { WORKSPACE_GRACE };
        tx.execute("INSERT INTO task_retirements(task_id,completion,checkout,branch,due_at,phase)
            SELECT id,CASE WHEN abandoned_at IS NOT NULL THEN 'abandoned:'||abandoned_at ELSE planning_completed_at END,
                worktree,branch,?2+?3,'waiting' FROM tasks
            WHERE id=?1 AND (abandoned_at IS NOT NULL OR (planning_completed=1 AND planning_completed_at IS NOT NULL))
                AND worktree IS NOT NULL
            ON CONFLICT(task_id) DO NOTHING", params![task.as_str(),now,grace])?;
        match hold {
            Some(ViewerHold::Viewed(_)) => hold_retirement(&tx, task, Some(VIEWED))?,
            absent => {
                let absent_since = absent.map(ViewerHold::at);
                // A viewer can reconnect and leave between cleanup passes,
                // including after checkout cleanup began. Reset its deadline too.
                let renewed: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM task_retirements WHERE task_id=?1
                     AND phase!='retired' AND due_at<?2+?3)",
                    params![task.as_str(), absent_since, grace],
                    |row| row.get(0),
                )?;
                if renewed {
                    hold_retirement(&tx, task, None)?;
                }
                tx.execute(
                    "UPDATE task_retirements SET due_at=?2+?3,reason=NULL
                     WHERE task_id=?1 AND phase='waiting' AND due_at IS NULL",
                    params![task.as_str(), absent_since.unwrap_or(now), grace],
                )?;
            }
        }
        tx.commit()?;
        drop(conn);
        self.task_retirement(task)
    }

    pub(crate) fn task_retirement(&self, task: &TaskId) -> StoreResult<Option<TaskRetirement>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT task_id,completion,checkout,branch,due_at,phase,retry_at
                 FROM task_retirements WHERE task_id=?1",
                [task.as_str()],
                |r| {
                    Ok(TaskRetirement {
                        task_id: TaskId::from_raw(r.get::<_, String>(0)?),
                        completion: r.get(1)?,
                        checkout: r.get(2)?,
                        branch: r.get(3)?,
                        due_at: r.get(4)?,
                        phase: r.get(5)?,
                        retry_at: r.get(6)?,
                    })
                },
            )
            .optional()?)
    }

    pub(crate) fn retirement_progress(
        &self,
        intent: &TaskRetirement,
        phase: &str,
        reason: Option<&str>,
    ) -> StoreResult<bool> {
        Ok(self.conn.lock().expect("store mutex poisoned").execute(
            "UPDATE task_retirements SET phase=?5,reason=?6 WHERE task_id=?1 AND completion=?2 AND checkout=?3 AND branch=?4 AND due_at IS NOT NULL",
            params![intent.task_id.as_str(),intent.completion,intent.checkout,intent.branch,phase,reason])? == 1)
    }
}

/// Use current Session membership, never shell receipts or input ownership.
/// The caller records viewing and revokes cleanup in the same transaction.
fn hold_viewer_retirements(
    conn: &Connection,
    terminal: &TerminalId,
    absent_since: Option<i64>,
) -> StoreResult<()> {
    let mut query = conn.prepare(&format!(
        "WITH owned AS MATERIALIZED (
             SELECT task_id,cwd,primary_scope FROM agent_sessions WHERE held_terminal_id=?1
         )
         SELECT h.task_id FROM task_retirements h WHERE h.phase!='retired'
         AND (?2 IS NULL OR h.due_at < ?2+?3)
         AND h.task_id IN (SELECT tw.id FROM owned s JOIN tasks tw ON ({}))",
        super::task_work::session_membership("s")
    ))?;
    let tasks = query
        .query_map(params![terminal, absent_since, WORKSPACE_GRACE], |r| {
            Ok(TaskId::from_raw(r.get::<_, String>(0)?))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for task in tasks {
        hold_retirement(conn, &task, absent_since.is_none().then_some(VIEWED))?;
    }
    Ok(())
}

// A returning viewer resets checkout cleanup, including its retry deadline.
fn hold_retirement(conn: &Connection, task: &TaskId, reason: Option<&str>) -> StoreResult<()> {
    conn.execute(
        "UPDATE task_retirements SET due_at=NULL,phase='waiting',reason=?2,retry_at=NULL,failures=0
         WHERE task_id=?1 AND phase!='retired'
           AND (due_at IS NOT NULL OR phase!='waiting' OR reason IS NOT ?2
                OR retry_at IS NOT NULL OR failures!=0)",
        params![task.as_str(), reason],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SqliteStore, TaskId, TerminalId, ViewerHold, WORKSPACE_GRACE};
    use rusqlite::params;

    fn fixture() -> (tempfile::TempDir, SqliteStore, TaskId) {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("loopflow.db")).unwrap();
        let repo = store.ensure_repository("/repo").unwrap();
        let task = TaskId::new();
        store.conn.lock().unwrap().execute("INSERT INTO tasks(id,repository_id,issue_identifier,created_at,worktree,branch,planning_completed,planning_completed_at) VALUES(?1,?2,'SHELL-1',1,'/repo.task','task',1,'completion-1')", params![task.as_str(),repo.as_str()]).unwrap();
        (home, store, task)
    }

    fn terminal(store: &SqliteStore) -> TerminalId {
        let terminal = TerminalId::new();
        store.create_terminal(&terminal, "build").unwrap();
        terminal
    }

    fn session_terminal(store: &SqliteStore, task: &TaskId, session: &str) -> TerminalId {
        let terminal = terminal(store);
        store.test_session(session, &crate::session_record::new_artifact_key());
        store.conn.lock().unwrap().execute("UPDATE agent_sessions SET task_id=?1,interactive=1,held_terminal_id=?2 WHERE id=?3", params![task.as_str(),terminal,session]).unwrap();
        terminal
    }

    fn held(store: &SqliteStore, task: &TaskId) -> Vec<(ViewerHold, bool)> {
        store
            .task_viewer_holds(task)
            .unwrap()
            .into_iter()
            .map(|h| (h.hold, h.ended))
            .collect()
    }

    fn diagnostics(store: &SqliteStore, task: &TaskId) -> (Option<String>, u32) {
        store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT reason,failures FROM task_retirements WHERE task_id=?1",
                [task.as_str()],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap()
    }

    #[test]
    fn returning_viewers_reset_retirement_and_retry_without_a_sweep() {
        for hold in [ViewerHold::Viewed(1200), ViewerHold::Absent(1201)] {
            let (_home, store, task) = fixture();
            let viewer = session_terminal(&store, &task, "passive");
            let shell = crate::id::TerminalId::new();
            store.create_terminal(&shell, "build").unwrap();
            store
                .create_desktop_shell("explicit", &task, "/repo.task", None)
                .unwrap();
            store.bind_desktop_shell("explicit", &shell).unwrap();
            let intent = store
                .reconcile_task_retirement(&task, 100, None)
                .unwrap()
                .unwrap();
            store.retirement_progress(&intent, "closing", None).unwrap();
            store.close_desktop_shell("explicit").unwrap();
            store
                .fail_retirement(&task, 1100, "owner not settled")
                .unwrap();
            // The recorder may see only the departure after a brief return.
            store.record_viewer_hold(&viewer, Some(hold)).unwrap();
            let viewed: bool = store
                .conn
                .lock()
                .unwrap()
                .query_row(
                    "SELECT viewed FROM held_terminals WHERE id=?1",
                    [&viewer],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(viewed, matches!(hold, ViewerHold::Viewed(_)));
            let returned = store.task_retirement(&task).unwrap().unwrap();
            assert_eq!(returned.phase, "waiting");
            assert_eq!(
                (
                    returned.due_at,
                    returned.retry_at,
                    diagnostics(&store, &task).1
                ),
                (None, None, 0)
            );
            store
                .record_viewer_hold(&viewer, Some(ViewerHold::Absent(1201)))
                .unwrap();
            assert_eq!(
                store.task_retirement(&task).unwrap().unwrap().phase,
                "waiting"
            );
            // Only now does a later pass date the new deadline from departure.
            let next = store
                .reconcile_task_retirement(&task, 1500, Some(ViewerHold::Absent(1201)))
                .unwrap()
                .unwrap();
            assert_eq!(next.due_at, Some(1201 + WORKSPACE_GRACE));
        }
    }

    #[test]
    fn abandoned_checkout_remains_pending_after_its_last_conversation_ends() {
        let (_home, store, task) = fixture();
        let owner = session_terminal(&store, &task, "abandoned");
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET planning_completed=0,abandoned_at=1,
            checkout_machine_id=(SELECT id FROM machines WHERE is_local=1) WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        assert!(store.pending_retirements().unwrap().contains(&task));
        store
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE held_terminals SET ended_at=1 WHERE id=?1", [&owner])
            .unwrap();
        assert!(store.pending_retirements().unwrap().contains(&task));
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET abandoned_at=NULL WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        assert!(!store.pending_retirements().unwrap().contains(&task));
    }

    #[test]
    fn viewer_evidence_records_absence_and_follows_current_session_terminals() {
        let (_home, store, task) = fixture();
        let first = session_terminal(&store, &task, "first");
        let second = session_terminal(&store, &task, "second");
        let unrelated = terminal(&store);
        store
            .record_viewer_hold(&unrelated, Some(ViewerHold::Viewed(100)))
            .unwrap();
        assert!(held(&store, &task).is_empty());
        store
            .record_viewer_hold(&first, Some(ViewerHold::Viewed(100)))
            .unwrap();
        store
            .record_viewer_hold(&second, Some(ViewerHold::Viewed(100)))
            .unwrap();
        assert_eq!(held(&store, &task).len(), 2);
        // One viewer's exit leaves the other Session's viewer holding.
        store.record_viewer_hold(&first, None).unwrap();
        assert_eq!(held(&store, &task), vec![(ViewerHold::Viewed(100), false)]);
        // An ended terminal retains its dated evidence; it cannot hold indefinitely.
        let conn = store.conn.lock().unwrap();
        conn.execute(
            "UPDATE held_terminals SET ended_at=1 WHERE id=?1",
            [&second],
        )
        .unwrap();
        drop(conn);
        assert_eq!(held(&store, &task), vec![(ViewerHold::Viewed(100), true)]);
        store
            .record_viewer_hold(&second, Some(ViewerHold::Absent(110)))
            .unwrap();
        assert_eq!(held(&store, &task), vec![(ViewerHold::Absent(110), true)]);
        // A replacement terminal speaks for the Session; the old one no longer does.
        let replacement = terminal(&store);
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET held_terminal_id=?1 WHERE id='second'",
                [&replacement],
            )
            .unwrap();
        assert!(held(&store, &task).is_empty());
    }

    #[test]
    fn viewer_holds_do_not_scan_session_history_for_every_retirement() {
        let (_home, store, task) = fixture();
        let viewer = session_terminal(&store, &task, "viewed");
        store.reconcile_task_retirement(&task, 100, None).unwrap();
        let repo = store.ensure_repository("/repo").unwrap();
        for n in 0..32 {
            let other = TaskId::new();
            store.conn.lock().unwrap().execute(
                "INSERT INTO tasks(id,repository_id,issue_identifier,created_at,worktree,branch,planning_completed,planning_completed_at)
                 VALUES(?1,?2,?3,1,?4,?3,1,'completion-1')",
                params![other.as_str(), repo.as_str(), format!("OTHER-{n}"), format!("/repo.other-{n}")],
            ).unwrap();
            store.reconcile_task_retirement(&other, 100, None).unwrap();
            session_terminal(&store, &other, &format!("other-{n}"));
        }
        crate::performance::testing::observe(&store.conn.lock().unwrap());
        let (result, work) = crate::performance::testing::measure(|| {
            store.record_viewer_hold(&viewer, Some(ViewerHold::Viewed(1200)))
        });
        result.unwrap();
        assert_eq!(store.task_retirement(&task).unwrap().unwrap().due_at, None);
        assert!(
            work.vm_steps < 50_000,
            "viewer hold holds the write lock: {work:?}"
        );
    }

    #[test]
    fn workspace_retirement_holds_cancel_deadlines_and_work_does_not_restart_them() {
        let (_home, store, task) = fixture();
        let held = store
            .reconcile_task_retirement(&task, 100, Some(ViewerHold::Viewed(100)))
            .unwrap()
            .unwrap();
        assert_eq!(held.due_at, None);
        let left = store
            .reconcile_task_retirement(&task, 110, None)
            .unwrap()
            .unwrap();
        assert_eq!(left.due_at, Some(110 + WORKSPACE_GRACE));
        // Another cleanup pass does not restart the absence grace.
        assert_eq!(
            store
                .reconcile_task_retirement(&task, 230, None)
                .unwrap()
                .unwrap()
                .due_at,
            left.due_at
        );
        let returned = store
            .reconcile_task_retirement(&task, 900, Some(ViewerHold::Viewed(900)))
            .unwrap()
            .unwrap();
        assert_eq!(returned.due_at, None);
        assert!(diagnostics(&store, &task)
            .0
            .unwrap()
            .contains("open Task Session viewer"));
        assert_eq!(
            store
                .reconcile_task_retirement(&task, 920, None)
                .unwrap()
                .unwrap()
                .due_at,
            Some(920 + WORKSPACE_GRACE)
        );
    }

    #[test]
    fn disconnect_deadline_uses_observed_absence_not_later_cleanup_or_agent_exit() {
        let (_home, store, task) = fixture();
        store
            .reconcile_task_retirement(&task, 100, Some(ViewerHold::Viewed(100)))
            .unwrap();
        let lost = store
            .reconcile_task_retirement(&task, 400, Some(ViewerHold::Absent(110)))
            .unwrap()
            .unwrap();
        assert_eq!(lost.due_at, Some(1010));
        let ended = store
            .reconcile_task_retirement(&task, 900, Some(ViewerHold::Absent(110)))
            .unwrap()
            .unwrap();
        assert_eq!(ended.due_at, lost.due_at);
        // A short reconnect may begin and end between cleanup passes,
        // after retirement has already entered closing.
        store.retirement_progress(&lost, "closing", None).unwrap();
        let returned = store
            .reconcile_task_retirement(&task, 1310, Some(ViewerHold::Absent(1300)))
            .unwrap()
            .unwrap();
        assert_eq!(returned.due_at, Some(2200));
        assert_eq!(returned.phase, "waiting");
        store
            .reconcile_task_retirement(&task, 1950, Some(ViewerHold::Viewed(1950)))
            .unwrap();
        assert_eq!(
            store
                .reconcile_task_retirement(&task, 2050, Some(ViewerHold::Absent(2000)))
                .unwrap()
                .unwrap()
                .due_at,
            Some(2900)
        );
    }

    #[test]
    fn retry_backoff_is_bounded_and_returning_viewer_revokes_it() {
        let (_home, store, task) = fixture();
        let initial = store
            .reconcile_task_retirement(&task, 100, None)
            .unwrap()
            .unwrap();
        for (attempt, seconds) in [60, 120, 240, 480, 960, 1920, 3600, 3600]
            .into_iter()
            .enumerate()
        {
            store.fail_retirement(&task, 2000, "offline").unwrap();
            let failed = store
                .reconcile_task_retirement(&task, 2000, None)
                .unwrap()
                .unwrap();
            assert_eq!(failed.retry_at, Some(2000 + seconds));
            let (reason, failures) = diagnostics(&store, &task);
            assert_eq!(failures as usize, attempt + 1);
            assert_eq!(failed.due_at, initial.due_at);
            assert_eq!(reason.as_deref(), Some("offline"));
        }
        let returned = store
            .reconcile_task_retirement(&task, 2100, Some(ViewerHold::Viewed(2100)))
            .unwrap()
            .unwrap();
        assert_eq!(returned.retry_at, None);
        assert_eq!(diagnostics(&store, &task).1, 0);
        assert_eq!(returned.due_at, None);
    }

    #[test]
    fn workspace_retirement_reopen_and_checkout_replacement_revoke_queued_work() {
        let (_home, store, task) = fixture();
        let old = store
            .reconcile_task_retirement(&task, 100, None)
            .unwrap()
            .unwrap();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET planning_completed=0 WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        assert!(!store.retirement_progress(&old, "closing", None).unwrap());
        assert!(store
            .reconcile_task_retirement(&task, 200, None)
            .unwrap()
            .is_none());
        store.conn.lock().unwrap().execute("UPDATE tasks SET planning_completed=1,planning_completed_at='completion-2' WHERE id=?1", [task.as_str()]).unwrap();
        let next = store
            .reconcile_task_retirement(&task, 300, None)
            .unwrap()
            .unwrap();
        assert_eq!(next.due_at, Some(1200));
        assert!(!store.retirement_progress(&old, "retired", None).unwrap());
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET worktree='/repo.new' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
        assert!(!store.retirement_progress(&next, "closing", None).unwrap());
    }
}
