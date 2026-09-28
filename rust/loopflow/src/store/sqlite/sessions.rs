//! Session transactions share the invocation's SQLite transaction and fences.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::durable::{FlowSession, RunId, TaskId};
use crate::engine::ExecutionCursor;
use crate::session::{AgentSession, Run, SessionKind, TitleSource, WorkSource};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

const SESSION_SELECT: &str = "SELECT s.id, s.current_run_id, s.title, s.title_source,
    s.ready_summary, s.completed_at, s.created_at, s.kind, s.request, s.interactive, s.repo,
    r.id, r.session_id, r.invocation_id, r.task_id, r.wave_id, r.work_source,
    r.created_at, r.published, r.cwd, r.skill, r.node, r.iterations, r.attempt,
    r.provider, r.model, r.caller_run_id, r.outcome, r.ended_at,
    s.task_id,s.wave_id,s.flow_session_id,s.work_source,s.bound_at
    FROM agent_sessions s JOIN runs r ON r.id=s.current_run_id AND r.session_id=s.id";

fn read_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoreResult<(AgentSession, Run)>> {
    let id: String = row.get(0)?;
    let run_id: String = row.get(1)?;
    let title = row.get(2)?;
    let source: String = row.get(3)?;
    let ready_summary = row.get(4)?;
    let completed_at = row.get(5)?;
    let created_at = row.get(6)?;
    let kind: String = row.get(7)?;
    let request = row.get(8)?;
    let run = super::runs::read_run(row, 11)?;
    Ok((|| {
        let run_id = RunId::parse(&run_id).map_err(invalid)?;
        Ok((
            AgentSession {
                id: id.clone(),
                current_run_id: run_id.clone(),
                task_id: row
                    .get::<_, Option<String>>(29)?
                    .map(|id| TaskId::parse(&id))
                    .transpose()
                    .map_err(invalid)?,
                wave_id: row
                    .get::<_, Option<String>>(30)?
                    .map(|id| crate::id::WaveId::parse(&id))
                    .transpose()
                    .map_err(invalid)?,
                flow_session_id: row.get(31)?,
                work_source: row
                    .get::<_, Option<String>>(32)?
                    .map(|source| serde_json::from_value(serde_json::Value::String(source)))
                    .transpose()?,
                bound_at: row.get(33)?,
                kind: match kind.as_str() {
                    "conversation" => SessionKind::Conversation,
                    "flow_review" => SessionKind::FlowReview,
                    "ask" => SessionKind::Ask,
                    _ => return Err(invalid("unknown Session kind")),
                },
                title,
                title_source: match source.as_str() {
                    "human" => TitleSource::Human,
                    "generated" => TitleSource::Generated,
                    _ => return Err(invalid("unknown Session title provenance")),
                },
                interactive: row.get(9)?,
                repo: row.get(10)?,
                request,
                ready_summary,
                completed_at,
                created_at,
            },
            run?,
        ))
    })())
}

fn invalid(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

fn title_source(source: TitleSource) -> &'static str {
    match source {
        TitleSource::Human => "human",
        TitleSource::Generated => "generated",
    }
}

pub(super) fn session_in(conn: &Connection, id: &str) -> StoreResult<Option<(AgentSession, Run)>> {
    conn.query_row(
        &format!("{SESSION_SELECT} WHERE s.id=?1"),
        [id],
        read_session,
    )
    .optional()?
    .transpose()
}

fn inventory_query(
    filter: &crate::session::SessionFilter,
) -> StoreResult<(String, Vec<rusqlite::types::Value>)> {
    use rusqlite::types::Value;
    let mut sql = format!("{SESSION_SELECT} WHERE 1");
    let mut values = Vec::new();
    let mut bind = |value| {
        values.push(value);
        format!("?{}", values.len())
    };
    if let Some(interactive) = filter.interactive {
        sql.push_str(&format!(
            " AND s.interactive={}",
            bind(Value::Integer(i64::from(interactive)))
        ));
    }
    if !filter.history {
        sql.push_str(
            " AND s.completed_at IS NULL AND (s.kind!='flow_review' OR EXISTS(
            SELECT 1 FROM flow_sessions f WHERE f.pending_session_id=s.id AND f.state='current'))",
        );
    }
    if let Some(repo) = &filter.repo {
        sql.push_str(&format!(" AND s.repo={}", bind(Value::Text(repo.clone()))));
    }
    if let Some(task) = &filter.task {
        let task = bind(Value::Text(task.clone()));
        sql.push_str(&format!(
            " AND s.task_id IN (SELECT id FROM tasks
            WHERE id={task} OR issue_identifier={task} OR external_issue_id={task})"
        ));
    }
    if let Some(search) = &filter.search {
        let search = bind(Value::Text(search.clone()));
        sql.push_str(&format!(
            " AND (instr(lower(s.title),lower({search}))>0 OR instr(s.id,{search})>0)"
        ));
    }
    let limit = if filter.limit == 0 {
        -1
    } else {
        i64::try_from(filter.limit).map_err(invalid)?
    };
    let limit = bind(Value::Integer(limit));
    let offset = bind(Value::Integer(
        i64::try_from(filter.offset).map_err(invalid)?,
    ));
    sql.push_str(&format!(
        " ORDER BY s.title, s.id LIMIT {limit} OFFSET {offset}"
    ));
    Ok((sql, values))
}

impl SqliteStore {
    pub fn reserve_review_run(&self, expected: &FlowSession) -> StoreResult<(FlowSession, Run)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = super::flows::flow_in(&tx, expected.id())?.ok_or(StoreError::NotFound)?;
        if current != *expected || current.claim.is_some() || current.failure.is_some() {
            return Err(StoreError::InvalidAuthority(
                "review changed before Run reservation".into(),
            ));
        }
        let id = review_id(expected)?;
        let (session, run) = session_in(&tx, &id)?.ok_or(StoreError::NotFound)?;
        if session.completed_at.is_some() {
            return Err(StoreError::InvalidAuthority("review is complete".into()));
        }
        if run.published {
            let replacement = RunId::new();
            super::runs::insert_run_in(
                &tx,
                Run {
                    id: replacement.clone(),
                    node: None,
                    iterations: None,
                    attempt: None,
                    cwd: expected.cwd.clone(),
                    published: false,
                    created_at: crate::store::rows::now_unix(),
                    work_source: Some(WorkSource::Inherited),
                    ..run
                },
            )?;
            tx.execute(
                "UPDATE agent_sessions SET current_run_id=?2 WHERE id=?1",
                params![id, replacement.as_str()],
            )?;
            tx.execute(
                "UPDATE flow_sessions SET position_version=position_version+1 WHERE id=?1",
                [expected.id()],
            )?;
        }
        let flow = super::flows::flow_in(&tx, expected.id())?.ok_or(StoreError::NotFound)?;
        let (_, run) = session_in(&tx, &id)?.ok_or(StoreError::NotFound)?;
        super::runs::select_attempt_in(&tx, flow.id(), flow.version, &run.id)?;
        let flow = super::flows::flow_in(&tx, expected.id())?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok((flow, run))
    }

    pub(crate) fn publish_review_run(
        &self,
        session_id: &str,
        run_id: &RunId,
        version: u64,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute("UPDATE flow_sessions SET position_version=position_version+1
            WHERE state='current' AND pending_session_id=?1 AND position_version=?3
            AND claim_json IS NULL AND EXISTS(SELECT 1 FROM agent_sessions s JOIN runs r ON r.id=s.current_run_id
                WHERE s.id=?1 AND r.id=?2 AND r.published=0 AND s.completed_at IS NULL)",
            params![session_id, run_id.as_str(), i64::try_from(version).map_err(invalid)?])? != 1 {
            return Err(StoreError::InvalidAuthority("review Run reservation is stale".into()));
        }
        tx.execute(
            "UPDATE runs SET published=1, provider=?2, model=?3 WHERE id=?1",
            params![run_id.as_str(), provider, model],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn session(&self, id: &str) -> StoreResult<Option<(AgentSession, Run)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        session_in(&conn, id)
    }

    pub fn session_for_run(&self, run_id: &RunId) -> StoreResult<Option<(AgentSession, Run)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            &format!("{SESSION_SELECT} WHERE s.id=(SELECT session_id FROM runs WHERE id=?1)"),
            [run_id.as_str()],
            read_session,
        )
        .optional()?
        .transpose()
    }

    /// Reserve a Session and its first Run together; reserving an id again
    /// returns the first one. `review` is the saved Flow a review waits in;
    /// the import stores a Flow it meets for the first time here.
    pub fn create_session(
        &self,
        mut session: AgentSession,
        mut run: Run,
        review: Option<&FlowSession>,
        caller_exec: Option<&crate::id::ExecId>,
    ) -> StoreResult<(AgentSession, Run)> {
        if run.session_id.as_deref() != Some(session.id.as_str())
            || run.id != session.current_run_id
            || review.is_some() != (session.kind == SessionKind::FlowReview)
            || review.map(FlowSession::id) != run.invocation_id.as_deref()
        {
            return Err(invalid(
                "a Session is reserved with its first Run, a review with its invocation",
            ));
        }
        if session.repo.is_none() && run.cwd.is_dir() {
            session.repo = crate::repo::discover_repo_root(&run.cwd)
                .map_err(invalid)?
                .map(|root| {
                    crate::repository::CanonicalRepo::discover(&root).map(|repo| repo.to_string())
                })
                .transpose()
                .map_err(invalid)?;
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = session_in(&tx, &session.id)? {
            return Ok(existing);
        }
        if let Some(flow) = review {
            super::flows::insert_flow_in(&tx, flow)?;
        }
        tx.execute(
            "INSERT INTO agent_sessions(id,current_run_id,kind,title,title_source,request,created_at,
                ready_summary,completed_at,interactive,repo)
            VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
            params![
                session.id,
                run.id.as_str(),
                match session.kind {
                    SessionKind::Conversation => "conversation",
                    SessionKind::Ask => "ask",
                    SessionKind::FlowReview => "flow_review",
                },
                session.title,
                title_source(session.title_source),
                session.request,
                session.created_at,
                session.ready_summary,
                session.completed_at,
                session.interactive,
                session.repo
            ],
        )?;
        super::runs::inherit_agent_work_in(&tx, &mut run, caller_exec)?;
        let run = super::runs::insert_run_in(&tx, run)?;
        if session
            .task_id
            .as_ref()
            .is_some_and(|id| Some(id) != run.task_id.as_ref())
            || session
                .wave_id
                .as_ref()
                .is_some_and(|id| Some(id) != run.wave_id.as_ref())
            || session
                .flow_session_id
                .as_ref()
                .is_some_and(|id| Some(id) != run.invocation_id.as_ref())
        {
            return Err(invalid(
                "AgentSession admission ancestry disagrees with its work",
            ));
        }
        tx.execute(
            "UPDATE flow_sessions SET pending_session_id=?2, current_run_id=?3 WHERE id=?1",
            params![run.invocation_id, session.id, run.id.as_str()],
        )?;
        let created = session_in(&tx, &session.id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(created)
    }

    /// Append an attempt to a Session. Title and feedback stay on the Session.
    /// A Task review appends its attempts through `reserve_review_run`.
    pub fn replace_session_run(&self, expected_run: &RunId, run: Run) -> StoreResult<Run> {
        let id = run
            .session_id
            .clone()
            .ok_or_else(|| invalid("a replacement Run belongs to its Session"))?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let run = super::runs::insert_run_in(&tx, run)?;
        if tx.execute(
            "UPDATE agent_sessions SET current_run_id=?3 WHERE id=?1 AND current_run_id=?2
             AND completed_at IS NULL AND NOT EXISTS(SELECT 1 FROM runs r JOIN tasks t ON t.id=r.task_id
                 WHERE r.id=?2 AND t.current_invocation_id=r.invocation_id)",
            params![id, expected_run.as_str(), run.id.as_str()],
        )? != 1
        {
            return Err(StoreError::InvalidAuthority(
                "Session changed before its Run was replaced".into(),
            ));
        }
        tx.execute(
            "UPDATE flow_sessions SET current_run_id=?2
             WHERE pending_session_id=?1 AND state='current'",
            params![id, run.id.as_str()],
        )?;
        tx.commit()?;
        Ok(run)
    }

    /// A review Run stored before providers were: take it from launch evidence.
    pub fn fill_run_provider(
        &self,
        run: &RunId,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE runs SET provider=?2, model=?3 WHERE id=?1 AND provider IS NULL",
            params![run.as_str(), provider, model],
        )?;
        Ok(())
    }

    /// Choose the agent of a Run that has not launched. A published Run keeps
    /// the provider it launched with.
    pub fn retarget_unpublished_run(
        &self,
        run: &RunId,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE runs SET provider=?2, model=?3 WHERE id=?1 AND published=0",
            params![run.as_str(), provider, model],
        )?;
        Ok(())
    }

    /// Assign future work without changing earlier attribution. Closed Sessions bind too.
    pub fn bind_session(
        &self,
        id: &str,
        expected_run: &RunId,
        task: &TaskId,
    ) -> StoreResult<(AgentSession, Run)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (session, _) = session_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        if session.current_run_id != *expected_run {
            return Err(StoreError::InvalidAuthority(
                "Session changed before binding".into(),
            ));
        }
        if let Some(bound) = &session.task_id {
            if bound != task {
                let issue: String = tx.query_row(
                    "SELECT issue_identifier FROM tasks WHERE id=?1",
                    [bound.as_str()],
                    |row| row.get(0),
                )?;
                return Err(StoreError::InvalidAuthority(format!(
                    "Session {id} already has Task {issue}; binding is permanent"
                )));
            }
        } else {
            let wave = super::runs::task_wave_in(&tx, task)?;
            if session
                .wave_id
                .as_ref()
                .is_some_and(|existing| *existing != wave)
            {
                return Err(StoreError::InvalidAuthority(format!(
                    "Session {id} belongs to another Wave"
                )));
            }
            if let Some(flow) = &session.flow_session_id {
                let compatible: bool = tx.query_row(
                    "SELECT task_id IS ?2 FROM flow_sessions WHERE id=?1",
                    params![flow, task.as_str()],
                    |row| row.get(0),
                )?;
                if !compatible {
                    return Err(StoreError::InvalidAuthority(
                        "AgentSession and FlowSession nullable Tasks disagree".into(),
                    ));
                }
            }
            tx.execute(
                "UPDATE agent_sessions SET task_id=?2,wave_id=?3,work_source='bound',bound_at=?4 WHERE id=?1",
                params![id, task.as_str(), wave.as_str(), crate::store::rows::now_unix()],
            )?;
        }
        let bound = session_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(bound)
    }

    /// Every open Session. A review is open while its invocation waits on it.
    pub fn sessions(
        &self,
        filter: &crate::session::SessionFilter,
    ) -> StoreResult<Vec<(AgentSession, Run)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (sql, values) = inventory_query(filter)?;
        let mut query = conn.prepare(&sql)?;
        let rows = query.query_map(rusqlite::params_from_iter(values), read_session)?;
        rows.map(|row| row?).collect()
    }

    /// The Flow and cursor node of the saved Flow waiting on this review. A
    /// Task's own Flow waits through its position instead.
    pub fn waiting_flow(&self, session_id: &str) -> StoreResult<Option<(String, String)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT json_extract(invocation_json,'$.flow'), review_json FROM flow_sessions
             WHERE pending_session_id=?1 AND state='current'
             AND NOT EXISTS(SELECT 1 FROM tasks WHERE current_invocation_id=flow_sessions.id)",
            [session_id],
            |row| Ok((row.get(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .map(|(flow, cursor)| {
            let cursor: ExecutionCursor = serde_json::from_str(&cursor)?;
            Ok((flow, cursor.node_key()))
        })
        .transpose()
    }

    /// Completion closes the Session; its Runs and provider history remain.
    /// An Ask or review closes only with the feedback its caller waits for.
    /// A Task review closes inside its invocation's transaction instead.
    pub fn complete_session(&self, id: &str, expected_run: &RunId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute(
            "UPDATE agent_sessions SET completed_at=?3 WHERE id=?1 AND current_run_id=?2
             AND completed_at IS NULL AND (kind='conversation' OR ready_summary IS NOT NULL)
             AND NOT EXISTS(SELECT 1 FROM runs r JOIN tasks t ON t.id=r.task_id
                 WHERE r.id=?2 AND t.current_invocation_id=r.invocation_id)",
            params![id, expected_run.as_str(), crate::store::rows::now_unix()],
        )? != 1
        {
            return Err(StoreError::InvalidAuthority(
                "Session changed before completion".into(),
            ));
        }
        Ok(())
    }

    pub fn session_runs(&self, id: &str) -> StoreResult<Vec<Run>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT {} FROM runs WHERE session_id=?1 ORDER BY created_at, id",
            super::runs::RUN_COLUMNS
        ))?;
        let rows = query.query_map([id], |row| super::runs::read_run(row, 0))?;
        rows.map(|row| row?).collect()
    }

    pub fn rename_session(
        &self,
        id: &str,
        expected_run: Option<&RunId>,
        title: &str,
        source: TitleSource,
    ) -> StoreResult<()> {
        if title.trim().is_empty() {
            return Err(invalid("Session title cannot be empty"));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (session, _) = session_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        if expected_run.is_some_and(|run| *run != session.current_run_id) {
            return Err(StoreError::InvalidAuthority(
                "Session changed before rename".into(),
            ));
        }
        tx.execute(
            "UPDATE agent_sessions SET title=?2, title_source=?3
             WHERE id=?1 AND (title_source='generated' OR ?3='human')",
            params![id, title.trim(), title_source(source)],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn ready_session(&self, id: &str, expected_run: &RunId, summary: &str) -> StoreResult<()> {
        if summary.trim().is_empty() {
            return Err(invalid("ready summary cannot be empty"));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute(
            "UPDATE agent_sessions SET ready_summary=?3 WHERE id=?1 AND current_run_id=?2
             AND completed_at IS NULL AND (kind='ask' OR EXISTS(SELECT 1 FROM runs r
                 JOIN flow_sessions f ON f.id=r.invocation_id
                 WHERE r.id=?2 AND r.published=1 AND f.pending_session_id=?1 AND f.state='current'
                 AND f.claim_json IS NULL))",
            params![id, expected_run.as_str(), summary.trim()],
        )? != 1
        {
            return Err(StoreError::InvalidAuthority("Session is stale".into()));
        }
        // Readiness invalidates an in-flight completion's snapshot as well.
        tx.execute(
            "UPDATE flow_sessions SET position_version=position_version+1
            WHERE pending_session_id=?1 AND state='current'",
            [id],
        )?;
        tx.commit()?;
        Ok(())
    }
}

pub(super) fn review_id(flow: &FlowSession) -> StoreResult<String> {
    let task = flow
        .task_id
        .as_ref()
        .ok_or_else(|| invalid("review belongs to no Task"))?;
    let step = flow
        .current_checked()
        .ok_or_else(|| invalid("review has no captured step"))?;
    let node = step
        .policy
        .id
        .ok_or_else(|| invalid("review has no captured node"))?;
    Ok(format!(
        "{}:{}:{}:{}:{}",
        task, flow.invocation.id, step.flow, node, flow.cursor.iteration
    ))
}

/// Store the review Session a Task's Flow parks at, with its first Run
/// reserved. Parking at the same review again finds the Session it left.
pub(super) fn reserve_task_review_in(
    conn: &Transaction<'_>,
    flow: &FlowSession,
) -> StoreResult<()> {
    if !flow.is_human() {
        return Err(StoreError::InvalidAuthority(
            "only a review position reserves a review Session".into(),
        ));
    }
    let id = review_id(flow)?;
    match session_in(conn, &id)? {
        Some((session, _)) if session.completed_at.is_some() => {
            return Err(StoreError::InvalidAuthority(
                "completed review cannot be reopened by a cursor write".into(),
            ));
        }
        Some(_) => {}
        None => {
            let run_id = RunId::new();
            let title: String = conn.query_row(
                "SELECT issue_title FROM tasks WHERE id=?1",
                [flow.task_id.as_ref().map(TaskId::as_str)],
                |row| row.get(0),
            )?;
            conn.execute(
                "INSERT INTO agent_sessions(id,current_run_id,kind,title,title_source,created_at,repo)
                VALUES(?1,?2,'flow_review',?3,'generated',?4,
                    (SELECT w.repo FROM waves w JOIN projects p ON p.wave_id=w.id
                        JOIN tasks t ON t.project_id=p.id WHERE t.id=?5))",
                params![
                    id,
                    run_id.as_str(),
                    title,
                    crate::store::rows::now_unix(),
                    flow.task_id.as_ref().map(TaskId::as_str)
                ],
            )?;
            super::runs::insert_run_in(
                conn,
                Run {
                    id: run_id,
                    session_id: Some(id.clone()),
                    invocation_id: Some(flow.id().to_owned()),
                    node: None,
                    iterations: None,
                    attempt: None,
                    task_id: flow.task_id.clone(),
                    wave_id: flow.wave_id.clone(),
                    work_source: Some(WorkSource::Inherited),
                    created_at: crate::store::rows::now_unix(),
                    published: false,
                    cwd: flow.cwd.clone(),
                    skill: Some(flow.current().step),
                    provider: None,
                    model: None,
                    caller_run_id: None,
                    ended: None,
                },
            )?;
        }
    }
    if conn.execute(
        "UPDATE flow_sessions SET pending_session_id=?2
         WHERE id=?1 AND state='current' AND position_version=?3 AND claim_json IS NULL",
        params![flow.id(), id, i64::try_from(flow.version).map_err(invalid)?],
    )? != 1
    {
        return Err(StoreError::InvalidAuthority(
            "review position changed before its Session was reserved".into(),
        ));
    }
    let (_, run) = session_in(conn, &id)?.ok_or(StoreError::NotFound)?;
    super::runs::select_attempt_in(conn, flow.id(), flow.version, &run.id)?;
    Ok(())
}

pub(super) fn complete_review_in(conn: &Connection, expected: &FlowSession) -> StoreResult<()> {
    let id = review_id(expected)?;
    let summary = expected
        .ready_summary
        .as_deref()
        .filter(|summary| !summary.trim().is_empty())
        .ok_or_else(|| StoreError::InvalidAuthority("review is not ready".into()))?;
    let run_id = expected
        .session_run_id()
        .ok_or_else(|| StoreError::InvalidAuthority("review has no published Run".into()))?;
    super::runs::require_attempt_in(conn, expected.id(), run_id)?;
    if conn.execute(
        "UPDATE agent_sessions SET completed_at=?3 WHERE id=?1 AND current_run_id=?2
        AND completed_at IS NULL AND ready_summary IS ?4",
        params![id, run_id.as_str(), crate::store::rows::now_unix(), summary],
    )? != 1
    {
        return Err(StoreError::InvalidAuthority(
            "review changed before completion".into(),
        ));
    }
    Ok(())
}
