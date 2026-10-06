//! One association rule for CLI and Desktop. Membership is additive to binding;
//! it never rewrites event attribution or follows causal parent Execs.

use std::collections::HashMap;

use crate::durable::TaskId;
use crate::engine::workflow::{WorkflowDefinition, END, START};
use crate::exec::Exec;
use crate::id::ExecId;
use crate::ops::workflow::{
    Workflow, WorkflowActor, WorkflowMove, WorkflowMoveKind, WorkflowPosition,
};
use crate::store::StoreResult;
use crate::task_work::{TaskSession, TaskWork};

use super::SqliteStore;

fn tasks(selector: &str) -> String {
    format!("SELECT id,worktree FROM tasks WHERE id={selector} OR issue_identifier={selector} OR external_issue_id={selector}")
}

fn checkout(cwd: &str) -> String {
    // Component boundaries avoid /repo.task matching /repo.task-other. Paths
    // remain usable after checkout removal; SQL LIKE would treat '%' as syntax.
    format!("tw.worktree!='' AND ({cwd}=rtrim(tw.worktree,'/') OR instr({cwd},rtrim(tw.worktree,'/')||'/')=1)")
}

fn session_membership(session: &str) -> String {
    format!(
        "NOT EXISTS(SELECT 1 FROM agent_sessions scoped WHERE scoped.id={session}.id
         AND scoped.primary_scope IS NOT NULL)
         AND ({session}.task_id=tw.id OR ({}))",
        checkout(&format!("{session}.cwd")),
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
        UNION SELECT a.driver_exec_id FROM agent_sessions a WHERE a.id IN ({}) AND a.driver_exec_id IS NOT NULL",
        tasks(selector), checkout("ae.cwd"), session_ids(selector), session_ids(selector))
}

pub(super) fn flows_of_task(
    conn: &rusqlite::Connection,
    task: &TaskId,
) -> StoreResult<Vec<crate::ops::flow_run::FlowExec>> {
    super::flow_inventory::flows_in(
        conn,
        &format!("e.id IN ({})", exec_ids("?1")),
        &[&task.as_str()],
    )
}

/// The Task's workflow row: its definition, the node it waits at or left,
/// and the edge it is on with the Exec carrying it.
type WorkflowRow = (WorkflowDefinition, String, Option<(u32, crate::exec::Exec)>);

fn workflow_row(conn: &rusqlite::Connection, task: &TaskId) -> StoreResult<Option<WorkflowRow>> {
    use rusqlite::OptionalExtension;
    let Some((graph, node, edge, exec)) = conn
        .query_row(
            "SELECT graph,node,edge,exec_id FROM task_workflows WHERE task_id=?1",
            [task.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<u32>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()?
    else {
        return Ok(None);
    };
    let edge = match edge.zip(exec) {
        Some((edge, exec)) => Some((
            edge,
            conn.query_row(
                &format!("{} WHERE e.id=?1", super::execs::EXEC_SELECT),
                [exec],
                super::execs::read_exec,
            )?,
        )),
        None => None,
    };
    Ok(Some((serde_json::from_str(&graph)?, node, edge)))
}

fn workflow_moves(conn: &rusqlite::Connection, task: &TaskId) -> StoreResult<Vec<WorkflowMove>> {
    conn.prepare(
        "SELECT m.workflow,m.kind,m.from_node,m.to_node,m.edge,m.exec_id,e.caller_session_id,m.note,m.at
         FROM task_workflow_moves m LEFT JOIN execs e ON e.id=m.exec_id
         WHERE m.task_id=?1 ORDER BY m.seq",
    )?
    .query_and_then([task.as_str()], |row| {
        let kind: String = row.get(1)?;
        let kind = WorkflowMoveKind::parse(&kind).ok_or_else(|| {
            crate::store::StoreError::InvalidData(format!("unknown workflow move {kind:?}"))
        })?;
        // An arrival is the edge's own; any other move is its caller's.
        let session_id = row
            .get::<_, Option<String>>(6)?
            .filter(|_| kind != WorkflowMoveKind::Arrived);
        Ok(WorkflowMove {
            workflow: row.get(0)?,
            kind,
            from: row.get(2)?,
            to: row.get(3)?,
            edge: row.get(4)?,
            exec_id: row.get(5)?,
            actor: match (kind, &session_id) {
                (WorkflowMoveKind::Arrived, _) => WorkflowActor::Edge,
                (_, Some(_)) => WorkflowActor::Conversation,
                (_, None) => WorkflowActor::Person,
            },
            session_id,
            note: row.get(7)?,
            at: row.get(8)?,
        })
    })?
    .collect()
}

#[allow(clippy::too_many_arguments)]
fn append_workflow_move(
    conn: &rusqlite::Connection,
    task: &TaskId,
    workflow: &str,
    kind: WorkflowMoveKind,
    (from, to): (&str, &str),
    edge: Option<u32>,
    by: Option<&ExecId>,
    note: Option<&str>,
) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO task_workflow_moves(task_id,workflow,kind,from_node,to_node,edge,exec_id,note,at)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)",
        rusqlite::params![
            task.as_str(),
            workflow,
            kind.as_str(),
            from,
            to,
            edge,
            by,
            note,
            crate::store::rows::now_unix()
        ],
    )?;
    Ok(())
}

/// How a Task reaches `end`.
#[derive(Debug, Clone)]
pub(crate) enum EndMove {
    /// The edge the calling process carried it along succeeded.
    Arrive,
    /// It leaves by an edge that runs nothing.
    Choose { workflow: Workflow, edge: u32 },
    /// It is put there. A Task with no Workflow is put at the end of one
    /// with nothing between.
    Set,
}

fn choose_edge_in(
    tx: &rusqlite::Transaction<'_>,
    task: &TaskId,
    workflow: &Workflow,
    edge: u32,
    by: Option<&ExecId>,
    note: Option<&str>,
) -> StoreResult<bool> {
    let chosen = &workflow.definition.edges[edge as usize];
    let stopped = match &workflow.position {
        WorkflowPosition::Node { .. } => None,
        WorkflowPosition::Edge { exec_id, .. } => Some(exec_id.as_str()),
    };
    let carried = chosen.flow.is_some();
    let left = tx.execute(
        "UPDATE task_workflows SET node=?2,edge=?3,exec_id=?4,updated_at=?5
         WHERE task_id=?1 AND node=?6 AND exec_id IS ?7",
        rusqlite::params![
            task.as_str(),
            if carried { &chosen.from } else { &chosen.to },
            carried.then_some(edge),
            by.filter(|_| carried),
            crate::store::rows::now_unix(),
            chosen.from,
            stopped
        ],
    )?;
    if left == 0 {
        return Ok(false);
    }
    append_workflow_move(
        tx,
        task,
        &workflow.definition.name,
        WorkflowMoveKind::Chose,
        (&chosen.from, &chosen.to),
        Some(edge),
        by,
        note,
    )?;
    Ok(true)
}

fn arrive_in(tx: &rusqlite::Transaction<'_>, task: &TaskId, by: &ExecId) -> StoreResult<()> {
    let Some((definition, _, Some((edge, carrier)))) = workflow_row(tx, task)? else {
        return Ok(());
    };
    if &carrier.id != by {
        return Ok(());
    }
    let arrived = &definition.edges[edge as usize];
    tx.execute(
        "UPDATE task_workflows SET node=?2,edge=NULL,exec_id=NULL,updated_at=?3 WHERE task_id=?1",
        rusqlite::params![task.as_str(), arrived.to, crate::store::rows::now_unix()],
    )?;
    append_workflow_move(
        tx,
        task,
        &definition.name,
        WorkflowMoveKind::Arrived,
        (&arrived.from, &arrived.to),
        Some(edge),
        Some(by),
        None,
    )
}

fn set_node_in(
    tx: &rusqlite::Transaction<'_>,
    task: &TaskId,
    node: &str,
    by: Option<&ExecId>,
    note: Option<&str>,
) -> StoreResult<bool> {
    let Some((definition, from, edge)) = workflow_row(tx, task)? else {
        return Ok(false);
    };
    tx.execute(
        "UPDATE task_workflows SET node=?2,edge=NULL,exec_id=NULL,updated_at=?3 WHERE task_id=?1",
        rusqlite::params![task.as_str(), node, crate::store::rows::now_unix()],
    )?;
    append_workflow_move(
        tx,
        task,
        &definition.name,
        WorkflowMoveKind::Set,
        (&from, node),
        edge.map(|(edge, _)| edge),
        by,
        note,
    )?;
    Ok(true)
}

fn stands_at_end(conn: &rusqlite::Connection, task: &TaskId) -> StoreResult<bool> {
    Ok(conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM task_workflows WHERE task_id=?1 AND node=?2 AND edge IS NULL)",
        [task.as_str(), END],
        |row| row.get(0),
    )?)
}

/// Make `how` for the Task. Returns whether this move put it at `end`: false
/// when it already stood there, or when the move no longer applied.
pub(super) fn reach_end_in(
    tx: &rusqlite::Transaction<'_>,
    task: &TaskId,
    how: &EndMove,
    by: Option<&ExecId>,
    note: Option<&str>,
) -> StoreResult<bool> {
    if stands_at_end(tx, task)? {
        return Ok(false);
    }
    match how {
        EndMove::Arrive => {
            if let Some(by) = by {
                arrive_in(tx, task, by)?;
            }
        }
        EndMove::Choose { workflow, edge } => {
            choose_edge_in(tx, task, workflow, *edge, by, note)?;
        }
        EndMove::Set => {
            if !set_node_in(tx, task, END, by, note)? {
                let unplanned = crate::engine::workflow::unplanned();
                tx.execute(
                    "INSERT INTO task_workflows(task_id,graph,node,updated_at) VALUES(?1,?2,?3,?4)",
                    rusqlite::params![
                        task.as_str(),
                        serde_json::to_string(&unplanned)?,
                        END,
                        crate::store::rows::now_unix()
                    ],
                )?;
                append_workflow_move(
                    tx,
                    task,
                    &unplanned.name,
                    WorkflowMoveKind::Set,
                    (START, END),
                    None,
                    by,
                    note,
                )?;
            }
        }
    }
    stands_at_end(tx, task)
}

/// Every unfinished Exec paired with each Task it belongs to: the same
/// membership as `exec_ids`, reached from the few unfinished Execs instead of
/// from every Session a Task has, and for all Tasks in one statement. The
/// index and join order are pinned: left to itself the planner scans every
/// Exec and every event that names one.
fn open_exec_tasks() -> String {
    let session = session_membership("a");
    format!(
        "WITH open AS MATERIALIZED (SELECT id,cwd FROM execs INDEXED BY execs_unfinished
            WHERE completed_at IS NULL),
        sessions AS (SELECT DISTINCT se.exec_id AS exec,se.session_id AS session
            FROM open CROSS JOIN session_events se ON se.exec_id=open.id
            UNION SELECT a.driver_exec_id,a.id FROM open CROSS JOIN agent_sessions a ON a.driver_exec_id=open.id)
        SELECT open.id,tw.id FROM open JOIN tasks tw ON {}
        UNION SELECT s.exec,tw.id FROM sessions s JOIN agent_sessions a ON a.id=s.session
            JOIN tasks tw ON ({session})",
        checkout("open.cwd")
    )
}

/// The unfinished Execs of every Task, read once for a reading of many Tasks.
/// Unfinished Execs are few; a checkout's Exec history grows without bound.
#[derive(Debug)]
pub(crate) struct OpenExecs {
    by_task: HashMap<String, Vec<Exec>>,
}

fn members(
    tx: &rusqlite::Transaction<'_>,
    task: &TaskId,
) -> StoreResult<(Vec<TaskSession>, Vec<crate::durable::FlowInventoryEntry>)> {
    let sessions = tx
        .prepare(&format!(
            "SELECT s.id,s.title,s.interactive,{},s.completed_at
            FROM agent_sessions s WHERE s.id IN ({}) ORDER BY s.created_at,s.id",
            super::sessions::SESSION_FLOW,
            session_ids("?1")
        ))?
        .query_and_then([task.as_str()], |row| {
            Ok(TaskSession {
                id: row.get(0)?,
                title: row.get(1)?,
                interactive: row.get(2)?,
                flow_id: row.get(3)?,
                completed_at: row.get(4)?,
            })
        })?
        .collect::<StoreResult<Vec<_>>>()?;
    let flows = flows_of_task(tx, task)?
        .iter()
        .map(|flow| super::flow_inventory::entry_in(tx, flow))
        .collect::<StoreResult<Vec<_>>>()?;
    Ok((sessions, flows))
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
        let (sessions, flows) = members(&tx, task)?;
        let execs = tx
            .prepare(&format!(
                "{} WHERE e.id IN ({}) ORDER BY e.started_at DESC,e.id",
                super::execs::EXEC_SELECT,
                exec_ids("?1")
            ))?
            .query_map([task.as_str()], super::execs::read_exec)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let workflow = workflow_row(&tx, task)?;
        let history = workflow_moves(&tx, task)?;
        tx.commit()?;
        drop(conn);
        let workflow = workflow.map(|row| self.read_workflow(row, history));
        Ok(TaskWork {
            sessions,
            flows,
            execs,
            workflow,
        })
    }

    pub(crate) fn open_execs(&self) -> StoreResult<OpenExecs> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let execs = tx
            .prepare(&format!(
                "{} INDEXED BY execs_unfinished WHERE e.completed_at IS NULL
                ORDER BY e.started_at DESC,e.id",
                super::execs::EXEC_SELECT
            ))?
            .query_map([], super::execs::read_exec)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut tasks: HashMap<ExecId, Vec<String>> = HashMap::new();
        let mut query = tx.prepare(&open_exec_tasks())?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            tasks.entry(row.get(0)?).or_default().push(row.get(1)?);
        }
        drop(rows);
        drop(query);
        tx.commit()?;
        let mut by_task: HashMap<String, Vec<Exec>> = HashMap::new();
        // Each Task's Execs keep the newest-first order they were read in.
        for exec in execs {
            for task in tasks.remove(&exec.id).unwrap_or_default() {
                by_task.entry(task).or_default().push(exec.clone());
            }
        }
        Ok(OpenExecs { by_task })
    }

    /// Sessions and Flows in full, with only the Execs still unfinished.
    /// Completion and recovery ask nothing of finished execution.
    pub(crate) fn task_open_work(&self, task: &TaskId, open: &OpenExecs) -> StoreResult<TaskWork> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let (sessions, flows) = members(&tx, task)?;
        let workflow = workflow_row(&tx, task)?;
        let history = workflow_moves(&tx, task)?;
        tx.commit()?;
        drop(conn);
        let workflow = workflow.map(|row| self.read_workflow(row, history));
        Ok(TaskWork {
            workflow,
            sessions,
            flows,
            execs: open.by_task.get(task.as_str()).cloned().unwrap_or_default(),
        })
    }

    /// Whether an Exec with no recorded exit may still have its process.
    /// Unknown is not stopped.
    pub(crate) fn exec_may_run(&self, exec: &crate::exec::Exec) -> bool {
        crate::journal::exec_process_evidence(self, &exec.id)
            != crate::journal::ProcessIdentityEvidence::Dead
    }

    /// The Task's Workflow with its history.
    pub(crate) fn workflow(&self, task: &TaskId) -> StoreResult<Option<Workflow>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let Some(row) = workflow_row(&conn, task)? else {
            return Ok(None);
        };
        let history = workflow_moves(&conn, task)?;
        drop(conn);
        Ok(Some(self.read_workflow(row, history)))
    }

    /// Whether the carrying Flow still runs is its Exec's, read here.
    fn read_workflow(
        &self,
        (definition, node, edge): WorkflowRow,
        history: Vec<WorkflowMove>,
    ) -> Workflow {
        let position = match edge {
            None => WorkflowPosition::Node { node },
            Some((edge, exec)) => WorkflowPosition::Edge {
                edge,
                exec_id: exec.id.to_string(),
                running: exec.completed_at.is_none() && self.exec_may_run(&exec),
            },
        };
        Workflow::new(definition, position, history)
    }

    /// Take up `definition` for the Task as it is defined now, at `start`.
    /// It replaces the Task's earlier Workflow; the history stays.
    pub(crate) fn take_up_workflow(
        &self,
        task: &TaskId,
        definition: &WorkflowDefinition,
        by: &ExecId,
        note: Option<&str>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR REPLACE INTO task_workflows(task_id,graph,node,updated_at) VALUES(?1,?2,?3,?4)",
            rusqlite::params![
                task.as_str(),
                serde_json::to_string(definition)?,
                START,
                crate::store::rows::now_unix()
            ],
        )?;
        append_workflow_move(
            &tx,
            task,
            &definition.name,
            WorkflowMoveKind::TookUp,
            (START, START),
            None,
            Some(by),
            note,
        )?;
        Ok(tx.commit()?)
    }

    /// Leave by `edge`, one of `workflow.outgoing`. An edge with a Flow puts
    /// the Task on it, carried by `by`; one that runs nothing puts it at its
    /// target. Returns false, writing nothing, when the Task is no longer
    /// where `workflow` read it, so two choosers cannot both leave.
    pub(crate) fn choose_workflow_edge(
        &self,
        task: &TaskId,
        workflow: &Workflow,
        edge: u32,
        by: &ExecId,
        note: Option<&str>,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let left = choose_edge_in(&tx, task, workflow, edge, Some(by), note)?;
        tx.commit()?;
        Ok(left)
    }

    /// The edge `by` carried the Task along succeeded: put the Task at its
    /// target. A Task no longer on that edge stays where it was put.
    pub(crate) fn arrive_workflow_edge(&self, task: &TaskId, by: &ExecId) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        arrive_in(&tx, task, by)?;
        Ok(tx.commit()?)
    }

    /// Put the Task at `node` of its Workflow, running nothing. Returns
    /// false when the Task has no Workflow.
    pub(crate) fn set_workflow_node(
        &self,
        task: &TaskId,
        node: &str,
        by: &ExecId,
        note: Option<&str>,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let set = set_node_in(&tx, task, node, Some(by), note)?;
        tx.commit()?;
        Ok(set)
    }

    /// The node `by`'s edge enters, while the Task is still on that edge.
    pub(crate) fn workflow_edge_target(
        &self,
        task: &TaskId,
        by: &ExecId,
    ) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(match workflow_row(&conn, task)? {
            Some((definition, _, Some((edge, carrier)))) if &carrier.id == by => {
                Some(definition.edges[edge as usize].to.clone())
            }
            _ => None,
        })
    }

    /// Every Flow among the Task's work, oldest first.
    pub(crate) fn task_flows(
        &self,
        task: &TaskId,
    ) -> StoreResult<Vec<crate::ops::flow_run::FlowExec>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        flows_of_task(&conn, task)
    }

    /// A turn runs in a local provider process, so one that began before this
    /// machine booted can no longer finish; its missing completion stays in history.
    pub(crate) fn session_has_pending_turn(&self, session: &str) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row("SELECT EXISTS(SELECT 1 FROM session_events start WHERE start.session_id=?1
            AND start.kind='started' AND (?2 IS NULL OR start.observed_at>=?2)
            AND NOT EXISTS(SELECT 1 FROM session_events retired WHERE retired.session_id=start.session_id AND retired.receipt_key='task_restart:stopped')
            AND NOT EXISTS(SELECT 1 FROM session_events done
                WHERE done.session_id=start.session_id AND done.kind='completed'
                AND done.provider_thread=start.provider_thread AND done.provider_turn=start.provider_turn))",
            rusqlite::params![session, crate::journal::machine_booted_at()], |row| row.get(0))?)
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
    use crate::task_work::TaskWork;

    #[tokio::test]
    async fn task_and_orphan_filters_resolve_aliases_before_pagination() {
        let repo = loopflow_test_support::TestRepo::new();
        let other = loopflow_test_support::TestRepo::new();
        let dir = tempfile::tempdir().unwrap();
        let alias = dir.path().join("alias");
        std::os::unix::fs::symlink(repo.path(), &alias).unwrap();
        std::fs::create_dir(repo.path().join("sub")).unwrap();
        let store = std::sync::Arc::new(
            crate::store::open_ephemeral_store(&crate::store::StorageConfig::sqlite(
                dir.path().join("db"),
            ))
            .await
            .unwrap(),
        );
        let home = store.local_home().await.unwrap().id;
        let task = TaskId::new();
        let wave = WaveId::new();
        let project = ProjectId::new();
        {
            let conn = store.sqlite.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
                [&wave],
            )
            .unwrap();
            conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", params![project.as_str(),wave]).unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at) VALUES(?1,?2,'issue','PROOF-1',?3,1)",params![task.as_str(),project.as_str(),repo.path().to_str().unwrap()]).unwrap();
            conn.execute(
                "INSERT INTO work_placements(task_id,home_id,placed_at) VALUES(?1,?2,1)",
                params![task.as_str(), home.as_str()],
            )
            .unwrap();
            for (id, path, scope) in [
                ("a-alias", alias.join("sub"), None),
                ("b-subdir", repo.path().join("sub"), None),
                ("c-repo", repo.path().to_path_buf(), Some("repository")),
                ("d-orphan", other.path().to_path_buf(), None),
            ] {
                conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,primary_scope,interactive) VALUES(?1,?1,'human',1,0,?2,?3,1)",params![id,path.to_str().unwrap(),scope]).unwrap();
                super::super::sessions::test_capture(
                    &conn,
                    id,
                    &crate::session_record::new_artifact_key(),
                );
            }
        }
        let mut filter = SessionFilter {
            task: Some("PROOF-1".into()),
            limit: 1,
            after: Some(String::new()),
            ..Default::default()
        };
        for expected in ["a-alias", "b-subdir"] {
            let page = crate::ops::human_session::list(&store, &filter)
                .await
                .unwrap();
            assert_eq!(page.len(), 1);
            assert_eq!(page[0].id, expected);
            assert_eq!(page[0].task_ids, vec![task.clone()]);
            filter.after = Some(page[0].id.clone());
        }
        filter.task = None;
        filter.orphan = true;
        filter.after = Some(String::new());
        for expected in ["c-repo", "d-orphan"] {
            let page = crate::ops::human_session::list(&store, &filter)
                .await
                .unwrap();
            assert_eq!(page.len(), 1);
            assert_eq!(page[0].id, expected);
            assert!(page[0].task_ids.is_empty());
            filter.after = Some(page[0].id.clone());
        }
    }

    #[test]
    fn scoped_sessions_stay_out_of_tasks_and_orphan_pages_include_only_unassociated_sessions() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let task = TaskId::new();
        let wave = WaveId::new();
        let project = ProjectId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
                [&wave],
            )
            .unwrap();
            conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", params![project.as_str(),wave]).unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at) VALUES(?1,?2,'issue','PROOF-1','/repo/task',1)",params![task.as_str(),project.as_str()]).unwrap();
            for (id, scope, wave_id, cwd) in [
                ("a-task", None, None, "/repo/task/sub"),
                ("b-repo", Some("repository"), None, "/repo/task"),
                ("c-wave", Some("wave"), Some(wave.as_str()), "/repo/task"),
                ("d-orphan", None, None, "/repo/other"),
                (
                    "e-wave-attribution",
                    None,
                    Some(wave.as_str()),
                    "/repo/task",
                ),
            ] {
                conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,primary_scope,wave_id,interactive) VALUES(?1,?1,'human',1,0,?2,?3,?4,1)",params![id,cwd,scope,wave_id]).unwrap();
                super::super::sessions::test_capture(
                    &conn,
                    id,
                    &crate::session_record::new_artifact_key(),
                );
            }
        }
        assert_eq!(
            store.session_task_ids("a-task").unwrap(),
            vec![task.clone()]
        );
        assert_eq!(
            store.session_task_ids("e-wave-attribution").unwrap(),
            vec![task]
        );
        for id in ["b-repo", "c-wave", "d-orphan"] {
            assert!(store.session_task_ids(id).unwrap().is_empty());
        }
        let mut filter = SessionFilter {
            orphan: true,
            limit: 1,
            after: Some(String::new()),
            ..Default::default()
        };
        for expected in ["b-repo", "c-wave", "d-orphan"] {
            let page = store.session_summaries(&filter, 0).unwrap();
            assert_eq!(page.len(), 1);
            assert_eq!(page[0].id, expected);
            filter.after = Some(page[0].id.clone());
        }
        assert!(store.session_summaries(&filter, 0).unwrap().is_empty());
    }

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
                ("conversation", "/missing/task%_", false, false),
                ("history", "/elsewhere", true, true),
                ("sibling", "/missing/task%_-other", false, false),
                ("wildcard", "/missing/taskAB", false, false),
            ] {
                conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id,completed_at)
                    VALUES(?1,?1,'human',1,0,?2,?3,?4,?5)", params![id,cwd,bound.then(|| task.as_str()),bound.then_some(wave.as_str()),complete.then_some(2)]).unwrap();
            }
            for id in ["manual", "conversation", "history", "sibling", "wildcard"] {
                super::super::sessions::test_capture(
                    &conn,
                    id,
                    &crate::session_record::new_artifact_key(),
                );
            }
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
        // A Flow is the Task's work when its Execs ran in the checkout.
        let independent = store.test_flow(
            "independent",
            "/missing/task%_/sub",
            &[("implement", None)],
            None,
        );
        store.test_flow("unrelated", "/other", &[("implement", None)], None);
        let work = store.task_work(&task).unwrap();
        assert_eq!(
            work.sessions
                .iter()
                .map(|s| s.id.as_str())
                .collect::<Vec<_>>(),
            ["conversation", "history", "manual"]
        );
        assert_eq!(
            work.flows
                .iter()
                .map(|f| f.summary.id.as_str())
                .collect::<Vec<_>>(),
            [independent.as_str()]
        );
        assert_eq!(work.flows[0].summary.task_id.as_ref(), Some(&task));
        assert_eq!(work.execs.len(), 4);
        // The open reading agrees with the full one about unfinished Execs.
        let initial_open = store
            .task_open_work(&task, &store.open_execs().unwrap())
            .unwrap();
        assert_eq!(initial_open.execs.len(), 2);
        assert!(initial_open
            .execs
            .iter()
            .all(|exec| exec.completed_at.is_none()));
        let unfinished = ExecId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO execs(id,trace_id,cwd,started_at) VALUES(?1,?2,'/elsewhere',1)",
                params![unfinished, TraceId::new()],
            )
            .unwrap();
            conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,exec_id,observed_at,payload) VALUES('history','started','open',?1,1,'{}')", [&unfinished]).unwrap();
        }
        let open = store
            .task_open_work(&task, &store.open_execs().unwrap())
            .unwrap();
        assert_eq!(open.sessions, store.task_work(&task).unwrap().sessions);
        assert_eq!(open.execs.len(), 3);
        assert!(open.execs.iter().any(|exec| exec.id == unfinished));
        // With every Exec unfinished, each membership path agrees: checkout,
        // Session event and driver.
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch(
                "CREATE TEMP TABLE finished AS SELECT id,completed_at FROM execs;
             UPDATE execs SET completed_at=NULL;",
            )
            .unwrap();
        let ids = |work: TaskWork| {
            work.execs
                .into_iter()
                .map(|exec| exec.id)
                .collect::<Vec<_>>()
        };
        let all = ids(store.task_work(&task).unwrap());
        assert_eq!(all.len(), 5);
        assert_eq!(
            ids(store
                .task_open_work(&task, &store.open_execs().unwrap())
                .unwrap()),
            all
        );
        store.conn.lock().unwrap().execute_batch(
            "UPDATE execs SET completed_at=(SELECT completed_at FROM finished WHERE finished.id=execs.id);
             DROP TABLE finished;",
        )
        .unwrap();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("DELETE FROM session_events WHERE receipt_key='open'", [])
                .unwrap();
            conn.execute("DELETE FROM execs WHERE id=?1", [&unfinished])
                .unwrap();
        }
        assert!(work.execs.iter().any(|exec| exec.id == mechanical));
        assert!(work.execs.iter().any(|exec| exec.id == bound_exec));
        assert_eq!(
            store.session_task_ids("manual").unwrap(),
            std::slice::from_ref(&task)
        );
        assert!(store.session_task_ids("sibling").unwrap().is_empty());
        let summaries = store
            .session_summaries(
                &SessionFilter {
                    task: Some("PROOF-1".into()),
                    interactive: None,
                    history: true,
                    ..Default::default()
                },
                0,
            )
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
        // The Flow's step is the only work performed in the checkout.
        let performed = store
            .execs(
                &ExecFilter {
                    performed_work: Some(ExecWorkFilter::Task(task.clone())),
                    ..Default::default()
                },
                None,
                NonZeroU32::new(100).unwrap(),
            )
            .unwrap();
        let (flow, _) = store.flow_exec(independent.as_str()).unwrap().unwrap();
        assert_eq!(
            performed
                .entries
                .iter()
                .map(|exec| &exec.id)
                .collect::<Vec<_>>(),
            [&flow.steps[0].exec.id]
        );

        let child = TaskId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("INSERT INTO tasks(id,project_id,external_issue_id,issue_identifier,worktree,created_at) VALUES(?1,?2,'child','PROOF-2','/missing/task%_/child',1)", params![child.as_str(), project.as_str()]).unwrap();
            conn.execute("INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id,wave_id) VALUES('child-session','Child','human',1,0,'/missing/task%_/child',?1,?2)", params![child.as_str(), wave]).unwrap();
        }
        // A nested checkout's work is also its enclosing Task's.
        let child_flow = store.test_flow(
            "child",
            "/missing/task%_/child",
            &[("implement", None)],
            None,
        );
        let work = store.task_work(&task).unwrap();
        assert!(work
            .flows
            .iter()
            .any(|flow| flow.summary.id == child_flow.as_str()));
        assert!(work
            .sessions
            .iter()
            .any(|session| session.id == "child-session"));
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
        assert!(work.flows.is_empty());
    }
}
