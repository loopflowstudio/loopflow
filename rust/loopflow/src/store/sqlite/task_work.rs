//! One association rule for CLI and Desktop. Membership is additive to binding;
//! it never rewrites event attribution or follows causal parent Execs.

use crate::durable::TaskId;
use crate::store::StoreResult;
use crate::task_work::{TaskSession, TaskWork};

use super::SqliteStore;

// Bound Flows inherit their Task's checkout instead of storing a second path.
const FLOW_CWD: &str = "COALESCE(af.cwd,(SELECT worktree FROM tasks WHERE id=af.task_id))";

fn tasks(selector: &str) -> String {
    format!("SELECT id,worktree FROM tasks WHERE id={selector} OR issue_identifier={selector} OR external_issue_id={selector}")
}

fn checkout(cwd: &str) -> String {
    // Component boundaries avoid /repo.task matching /repo.task-other. Paths
    // remain usable after checkout removal; SQL LIKE would treat '%' as syntax.
    format!("tw.worktree!='' AND ({cwd}=rtrim(tw.worktree,'/') OR instr({cwd},rtrim(tw.worktree,'/')||'/')=1)")
}

pub(super) fn flow_ids(selector: &str) -> String {
    format!(
        "SELECT af.id FROM flow_sessions af JOIN ({}) tw ON af.task_id=tw.id OR ({})",
        tasks(selector),
        checkout(FLOW_CWD)
    )
}

fn session_membership(session: &str) -> String {
    format!(
        "{session}.task_id=tw.id OR ({}) OR EXISTS(SELECT 1 FROM flow_sessions af
        WHERE af.id={session}.flow_session_id AND (af.task_id=tw.id OR ({})))",
        checkout(&format!("{session}.cwd")),
        checkout(FLOW_CWD)
    )
}

pub(super) fn session_ids(selector: &str) -> String {
    format!(
        "SELECT a.id FROM agent_sessions a JOIN ({}) tw ON ({})",
        tasks(selector),
        session_membership("a")
    )
}

pub(super) fn session_tasks(session: &str) -> String {
    format!(
        "SELECT tw.id FROM tasks tw WHERE {} ORDER BY tw.id",
        session_membership(session)
    )
}

fn exec_ids(selector: &str) -> String {
    format!("SELECT ae.id FROM execs ae JOIN ({}) tw ON ({})
        UNION SELECT se.exec_id FROM session_events se WHERE se.session_id IN ({}) AND se.exec_id IS NOT NULL
        UNION SELECT a.driver_exec_id FROM agent_sessions a WHERE a.id IN ({}) AND a.driver_exec_id IS NOT NULL
        UNION SELECT fe.exec_id FROM flow_events fe WHERE fe.flow_id IN ({}) AND fe.exec_id IS NOT NULL",
        tasks(selector), checkout("ae.cwd"), session_ids(selector), session_ids(selector), flow_ids(selector))
}

impl SqliteStore {
    pub(crate) fn session_task_ids(&self, session: &str) -> StoreResult<Vec<TaskId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT tw.id FROM tasks tw JOIN agent_sessions a ON a.id=?1 WHERE {} ORDER BY tw.id",
            session_membership("a")
        ))?;
        let rows = query.query_map([session], |row| row.get::<_, String>(0))?;
        rows.map(|row| {
            TaskId::parse(&row?)
                .map_err(|error| crate::store::StoreError::InvalidData(error.to_string()))
        })
        .collect()
    }

    /// Read all three owners in one SQLite snapshot, including closed history.
    pub fn task_work(&self, task: &TaskId) -> StoreResult<TaskWork> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let sessions = tx
            .prepare(&format!(
                "SELECT s.id,s.title,s.kind,s.interactive,s.flow_session_id,s.completed_at,
            COALESCE(s.flow_session_id=(SELECT current_invocation_id FROM tasks WHERE id=?1),0)
            FROM agent_sessions s WHERE s.id IN ({}) ORDER BY s.created_at,s.id",
                session_ids("?1")
            ))?
            .query_and_then([task.as_str()], |row| {
                Ok(TaskSession {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    kind: serde_json::from_value(serde_json::Value::String(row.get(2)?))?,
                    interactive: row.get(3)?,
                    flow_session_id: row.get(4)?,
                    completed_at: row.get(5)?,
                    managed: row.get(6)?,
                })
            })?
            .collect::<StoreResult<Vec<_>>>()?;
        let mut flows = tx
            .prepare(&format!(
                "SELECT {},{} {} WHERE f.id IN ({}) ORDER BY f.id",
                super::flows::FLOW_METADATA_COLUMNS,
                super::flow_inventory::INVENTORY_EXTRA,
                super::flow_inventory::INVENTORY_FROM,
                flow_ids("?1")
            ))?
            .query_map([task.as_str()], super::flow_inventory::read_entry)?
            .collect::<rusqlite::Result<StoreResult<Vec<_>>>>()??;
        for flow in &mut flows {
            flow.managed &= flow.summary.task_id.as_ref() == Some(task);
        }
        let execs = tx
            .prepare(&format!(
                "{} WHERE e.id IN ({}) ORDER BY e.started_at DESC,e.id",
                super::execs::EXEC_SELECT,
                exec_ids("?1")
            ))?
            .query_map([task.as_str()], super::execs::read_exec)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        tx.commit()?;
        Ok(TaskWork {
            sessions,
            flows,
            execs,
        })
    }

    /// Exact Flow membership is used only to exempt the completing worker from
    /// its own completion gate. Causal ancestry does not establish membership.
    pub(crate) fn flow_exec_ids(&self, flow: &str) -> StoreResult<Vec<crate::id::ExecId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare("SELECT exec_id FROM flow_events WHERE flow_id=?1 AND exec_id IS NOT NULL
            UNION SELECT e.exec_id FROM session_events e JOIN agent_sessions s ON s.id=e.session_id
                WHERE s.flow_session_id=?1 AND e.exec_id IS NOT NULL
            UNION SELECT driver_exec_id FROM agent_sessions WHERE flow_session_id=?1 AND driver_exec_id IS NOT NULL")?;
        let rows = query.query_map([flow], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub(crate) fn session_has_pending_turn(&self, session: &str) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM session_events start WHERE start.session_id=?1
            AND start.kind='started' AND NOT EXISTS(SELECT 1 FROM session_events done
                WHERE done.session_id=start.session_id AND done.kind='completed'
                AND done.provider_thread=start.provider_thread AND done.provider_turn=start.provider_turn))",
            [session], |row| row.get(0))?)
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use rusqlite::params;

    use crate::durable::{FlowFilter, ProjectId, TaskId};
    use crate::exec::{ExecFilter, ExecWorkFilter};
    use crate::id::{ExecId, TraceId, WaveId};
    use crate::session::SessionFilter;
    use crate::store::sqlite::SqliteStore;

    #[test]
    fn task_work_includes_checkout_binding_and_mechanical_history_without_granting_ownership() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let task = TaskId::new();
        let wave = WaveId::new();
        let project = ProjectId::new();
        let mechanical = ExecId::new();
        let bound_exec = ExecId::new();
        let sibling = ExecId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
                [&wave],
            )
            .unwrap();
            conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", params![project.as_str(), wave]).unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at) VALUES(?1,?2,'issue','PROOF-1','/missing/task%_',1)", params![task.as_str(), project.as_str()]).unwrap();
            for (id, cwd, bound, complete) in [
                ("manual", "/missing/task%_/src", false, false),
                ("ask", "/missing/task%_", false, false),
                ("history", "/elsewhere", true, true),
                ("sibling", "/missing/task%_-other", false, false),
                ("wildcard", "/missing/taskAB", false, false),
            ] {
                conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id,kind,completed_at)
                    VALUES(?1,?1,'human',1,0,?2,?3,?4,?5,?6)", params![id,cwd,bound.then(|| task.as_str()),bound.then_some(wave.as_str()),if id=="ask" {"ask"} else {"conversation"},complete.then_some(2)]).unwrap();
            }
            for id in ["manual", "ask", "history", "sibling", "wildcard"] {
                super::super::sessions::test_capture(
                    &conn,
                    id,
                    &crate::session_record::new_artifact_key(),
                );
            }
            for (id, cwd, bound) in [
                ("managed", "/elsewhere", true),
                ("independent", "/missing/task%_/sub", false),
                ("unrelated", "/other", false),
            ] {
                conn.execute("INSERT INTO flow_sessions(id,task_id,wave_id,cwd,invocation_json,step_index,iteration,position_version,worker_generation,updated_at,state)
                    VALUES(?1,?2,?3,?4,?5,0,0,1,0,1,'current')", params![id,bound.then(||task.as_str()),bound.then_some(wave.as_str()),(!bound).then_some(cwd),serde_json::json!({"id": id,"flow": id,"steps": []}).to_string()]).unwrap();
            }
            conn.execute(
                "UPDATE tasks SET current_invocation_id='managed' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
            for (id, cwd) in [
                (&mechanical, "/missing/task%_"),
                (&bound_exec, "/elsewhere"),
                (&sibling, "/missing/task%_-other"),
            ] {
                conn.execute("INSERT INTO execs(id,trace_id,cwd,started_at,completed_at,outcome) VALUES(?1,?2,?3,1,2,'succeeded')", params![id, TraceId::new(), cwd]).unwrap();
            }
            // Binding includes earlier Execs in the inventory without assigning
            // their earlier performed work or usage to this Task.
            conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,exec_id,observed_at,payload) VALUES('history','started','before-bind',?1,1,'{}')", [&bound_exec]).unwrap();
            conn.execute(
                "UPDATE execs SET parent_exec_id=?1 WHERE id=?2",
                params![mechanical, sibling],
            )
            .unwrap();
        }
        let work = store.task_work(&task).unwrap();
        assert_eq!(
            work.sessions
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            ["ask", "history", "manual"]
        );
        assert_eq!(
            work.flows
                .iter()
                .map(|f| (f.summary.id.as_str(), f.managed))
                .collect::<Vec<_>>(),
            [("independent", false), ("managed", true)]
        );
        assert_eq!(work.execs.len(), 2);
        assert!(work.execs.iter().any(|exec| exec.id == mechanical));
        assert!(work.execs.iter().any(|exec| exec.id == bound_exec));
        assert_eq!(
            store.session_task_ids("manual").unwrap(),
            std::slice::from_ref(&task)
        );
        assert!(store.session_task_ids("sibling").unwrap().is_empty());
        let summaries = store
            .session_summaries(&SessionFilter {
                task: Some("PROOF-1".into()),
                interactive: None,
                history: true,
                ..Default::default()
            })
            .unwrap();
        assert_eq!(summaries.len(), work.sessions.len());
        assert!(summaries
            .iter()
            .all(|session| session.task_ids == std::slice::from_ref(&task)));
        let flows = store
            .flow_inventory(
                &FlowFilter {
                    task_id: Some(task.clone()),
                    ..Default::default()
                },
                None,
                NonZeroU32::new(100).unwrap(),
            )
            .unwrap();
        assert_eq!(flows.entries, work.flows);
        assert!(store
            .execs(
                &ExecFilter {
                    performed_work: Some(ExecWorkFilter::Task(task.clone())),
                    ..Default::default()
                },
                None,
                NonZeroU32::new(100).unwrap()
            )
            .unwrap()
            .entries
            .is_empty());

        let child = TaskId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at) VALUES(?1,?2,'child','PROOF-2','/missing/task%_/child',1)", params![child.as_str(), project.as_str()]).unwrap();
            conn.execute("INSERT INTO flow_sessions(id,task_id,wave_id,invocation_json,step_index,iteration,position_version,worker_generation,updated_at,state) VALUES('child-flow',?1,?2,?3,0,0,1,0,1,'current')", params![child.as_str(), wave, serde_json::json!({"id":"child-flow","flow":"child","steps":[]}).to_string()]).unwrap();
            conn.execute(
                "UPDATE tasks SET current_invocation_id='child-flow' WHERE id=?1",
                [child.as_str()],
            )
            .unwrap();
            conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,flow_session_id,task_id,wave_id) VALUES('child-session','Child','human',1,0,'/elsewhere','child-flow',?1,?2)", params![child.as_str(), wave]).unwrap();
        }
        let work = store.task_work(&task).unwrap();
        assert!(work
            .flows
            .iter()
            .any(|flow| flow.summary.id == "child-flow" && !flow.managed));
        assert!(work
            .sessions
            .iter()
            .any(|session| session.id == "child-session" && !session.managed));
        let memberships = store.session_task_ids("child-session").unwrap();
        assert!(memberships.contains(&task) && memberships.contains(&child));
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("UPDATE tasks SET worktree='' WHERE id=?1", [task.as_str()])
                .unwrap();
        }
        let work = store.task_work(&task).unwrap();
        assert_eq!(
            work.sessions
                .iter()
                .map(|session| session.id.as_str())
                .collect::<Vec<_>>(),
            ["history"]
        );
        assert_eq!(
            work.flows
                .iter()
                .map(|flow| flow.summary.id.as_str())
                .collect::<Vec<_>>(),
            ["managed"]
        );
    }
}
