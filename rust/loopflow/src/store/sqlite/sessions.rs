//! Session transactions share the invocation's SQLite transaction and fences.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::durable::{FlowSession, RunId, TaskId};
use crate::engine::ExecutionCursor;
use crate::session::{AgentSession, SessionKind, TitleSource, WorkSource};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

const SESSION_SELECT: &str = "SELECT s.id,s.input_id,s.title,s.title_source,
    s.ready_summary,s.completed_at,s.created_at,s.kind,s.request,s.interactive,s.repo,
    s.task_id,s.wave_id,s.flow_session_id,s.work_source,s.bound_at,
    s.input_published,s.cwd,s.skill,s.provider,s.model,s.node,s.iterations,i.caller_input_id FROM agent_sessions s LEFT JOIN agent_session_inputs i ON i.input_id=s.input_id";

fn read_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoreResult<AgentSession>> {
    Ok((|| {
        Ok(AgentSession {
            caller_input_id: row
                .get::<_, Option<String>>(23)?
                .map(|id| RunId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            id: row.get(0)?,
            input_id: RunId::parse(&row.get::<_, String>(1)?).map_err(invalid)?,
            title: row.get(2)?,
            title_source: serde_json::from_value(serde_json::Value::String(row.get(3)?))?,
            ready_summary: row.get(4)?,
            completed_at: row.get(5)?,
            created_at: row.get(6)?,
            kind: serde_json::from_value(serde_json::Value::String(row.get(7)?))?,
            request: row.get(8)?,
            interactive: row.get(9)?,
            repo: row.get(10)?,
            task_id: row
                .get::<_, Option<String>>(11)?
                .map(|id| TaskId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            wave_id: row
                .get::<_, Option<String>>(12)?
                .map(|id| crate::id::WaveId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            flow_session_id: row.get(13)?,
            work_source: row
                .get::<_, Option<String>>(14)?
                .map(|source| serde_json::from_value(serde_json::Value::String(source)))
                .transpose()?,
            bound_at: row.get(15)?,
            input_published: row.get(16)?,
            cwd: row.get::<_, String>(17)?.into(),
            skill: row.get(18)?,
            provider: row.get(19)?,
            model: row.get(20)?,
            node: row.get(21)?,
            iterations: row
                .get::<_, Option<String>>(22)?
                .map(|value| serde_json::from_str(&value))
                .transpose()?,
        })
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

pub(super) fn session_in(conn: &Connection, id: &str) -> StoreResult<Option<AgentSession>> {
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
    /// Old rows are import evidence only. None means no established conversation;
    /// the importer must report that retained row, not silently omit it.
    pub(crate) fn historical_session_inputs(
        &self,
    ) -> StoreResult<Vec<(Option<AgentSession>, crate::session::SessionObservation)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare("SELECT coalesce(i.session_id,r.session_id,r.id),r.id,r.created_at,r.task_id,r.wave_id,
            json_object('id',r.id,'session_id',r.session_id,'invocation_id',r.invocation_id,
                'task_id',r.task_id,'wave_id',r.wave_id,'work_source',r.work_source,
                'created_at',r.created_at,'published',r.published,'cwd',r.cwd,'skill',r.skill,
                'node',r.node,'iterations',r.iterations,'attempt',r.attempt,'provider',r.provider,
                'model',r.model,'caller_run_id',r.caller_run_id,'outcome',r.outcome,'ended_at',r.ended_at)
            FROM runs r LEFT JOIN agent_session_inputs i ON i.input_id=r.id
            ORDER BY r.created_at,r.id")?;
        let rows = query.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
            ))
        })?;
        let mut inputs = Vec::new();
        for row in rows {
            let (session, input, observed_at, task, wave, raw) = row?;
            let evidence: serde_json::Value = serde_json::from_str(&raw)?;
            let agent = historical_agent_input(&conn, &evidence)?;
            if !agent && historical_operation_retained(&conn, &evidence)? {
                continue;
            }
            let session = if agent {
                Some(match session_in(&conn, &session)? {
                    Some(session) => session,
                    None => historical_conversation(&evidence)?,
                })
            } else {
                None
            };
            if session.as_ref().is_some_and(|session| {
                evidence["session_id"]
                    .as_str()
                    .is_some_and(|id| id != session.id)
            }) {
                return Err(invalid(format!(
                    "historical input {input} names a different Session"
                )));
            }
            inputs.push((session, crate::session::SessionObservation {
                input_id: RunId::parse(&input).map_err(invalid)?, source: "runs".into(), observed_at,
                task_id: task.map(|task| TaskId::parse(&task)).transpose().map_err(invalid)?,
                wave_id: wave.map(|wave| crate::id::WaveId::parse(&wave)).transpose().map_err(invalid)?,
                payload: serde_json::json!({"input_id":input,"source":"runs","evidence":evidence}),
            }));
        }
        Ok(inputs)
    }

    /// Restore historical conversation facts without borrowing the importing Exec's Work.
    pub(crate) fn resolve_history_input(&self, selector: &str) -> StoreResult<String> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE id=?1)",
            [selector],
            |row| row.get::<_, bool>(0),
        )? {
            return Ok(selector.to_string());
        }
        let mut query = conn.prepare("SELECT id FROM (
            SELECT input_id AS id FROM agent_session_inputs UNION SELECT caller_input_id FROM agent_session_inputs)
            WHERE substr(id,1,length(?1))=?1 OR (substr(id,1,4)='run_' AND substr(id,5,length(?1))=?1)
            ORDER BY id LIMIT 2")?;
        let ids = query
            .query_map([selector], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        match ids.as_slice() {
            [id] => Ok(id.clone()),
            [] => Err(StoreError::NotFound),
            _ => Err(invalid(format!("Input selector {selector:?} is ambiguous"))),
        }
    }

    /// Project retained input history, with its own attribution and chronology.
    /// A continuation never moves earlier usage to the conversation's new binding.
    pub(crate) fn conversation_snapshots(
        &self,
        wave: Option<&str>,
        project: Option<&str>,
        task: Option<&str>,
        caller: Option<&str>,
        since: i64,
        include_finished: bool,
    ) -> StoreResult<
        Vec<(
            Option<crate::durable::WorkRef>,
            crate::run_record::RunSnapshot,
        )>,
    > {
        self.conversation_inputs(wave, project, task, caller, (since, include_finished), None)
    }

    pub(crate) fn input_snapshot(
        &self,
        selector: &str,
    ) -> StoreResult<crate::run_record::RunSnapshot> {
        let selected = self.resolve_history_input(selector)?;
        let input = self
            .session(&selected)?
            .map(|session| session.input_id.to_string())
            .unwrap_or(selected);
        self.conversation_inputs(None, None, None, None, (0, false), Some(&input))?
            .pop()
            .map(|(_, snapshot)| snapshot)
            .ok_or(StoreError::NotFound)
    }

    fn conversation_inputs(
        &self,
        wave: Option<&str>,
        project: Option<&str>,
        task: Option<&str>,
        caller: Option<&str>,
        window: (i64, bool),
        input: Option<&str>,
    ) -> StoreResult<
        Vec<(
            Option<crate::durable::WorkRef>,
            crate::run_record::RunSnapshot,
        )>,
    > {
        let inputs = {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let mut query = conn.prepare("WITH inputs AS (
                SELECT i.input_id,i.session_id,i.caller_input_id,
                    COALESCE(m.observed_at,r.observed_at,s.created_at) AS started,
                    CASE WHEN m.seq IS NOT NULL THEN m.task_id WHEN r.seq IS NOT NULL THEN r.task_id ELSE s.task_id END AS task_id,
                    CASE WHEN m.seq IS NOT NULL THEN m.wave_id WHEN r.seq IS NOT NULL THEN r.wave_id ELSE s.wave_id END AS wave_id,
                    COALESCE(terminal.observed_at,json_extract(r.payload,'$.evidence.ended_at')) AS ended
                FROM agent_session_inputs i JOIN agent_sessions s ON s.id=i.session_id
                LEFT JOIN session_events m ON m.session_id=s.id AND m.kind='observed'
                    AND m.receipt_key=i.input_id||':manifest.json'
                LEFT JOIN session_events r ON r.session_id=s.id AND r.kind='observed'
                    AND r.receipt_key=i.input_id||':runs'
                LEFT JOIN session_events terminal ON terminal.session_id=s.id AND terminal.kind='observed'
                    AND terminal.receipt_key=i.input_id||':terminal.json'
                WHERE (?7 IS NULL OR i.input_id=?7) AND
                    (?7 IS NOT NULL OR m.seq IS NOT NULL OR json_extract(r.payload,'$.evidence.published')=1
                    OR (i.input_id=s.input_id AND s.input_published=1)))
                SELECT session_id,input_id,caller_input_id,started,task_id,wave_id,
                    (SELECT name FROM waves WHERE id=inputs.wave_id),
                    (SELECT p.project_slug FROM tasks t JOIN projects p ON p.id=t.project_id WHERE t.id=inputs.task_id),
                    (SELECT issue_identifier FROM tasks WHERE id=inputs.task_id)
                FROM inputs
                WHERE (?1 IS NULL OR wave_id IN (SELECT id FROM waves WHERE id=?1 OR name=?1))
                AND (?2 IS NULL OR task_id IN (SELECT t.id FROM tasks t JOIN projects p ON p.id=t.project_id
                    WHERE p.id=?2 OR p.project_slug=?2 OR p.external_project_id=?2))
                AND (?3 IS NULL OR task_id IN (SELECT id FROM tasks WHERE id=?3 OR issue_identifier=?3 OR external_issue_id=?3))
                AND (?4 IS NULL OR caller_input_id=?4 OR caller_input_id IN (SELECT input_id FROM agent_session_inputs WHERE session_id=?4))
                AND (started>=?5 OR (?6 AND ended>=?5))
                ORDER BY started DESC,input_id DESC")?;
            let rows = query.query_map(
                params![wave, project, task, caller, window.0, window.1, input],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        (
                            row.get::<_, Option<String>>(6)?,
                            row.get::<_, Option<String>>(7)?,
                            row.get::<_, Option<String>>(8)?,
                        ),
                    ))
                },
            )?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        inputs
            .into_iter()
            .map(|(session_id, input, caller, started, task, wave, names)| {
                let input = RunId::parse(&input).map_err(invalid)?;
                let session = self.session(&session_id)?.ok_or(StoreError::NotFound)?;
                let history = self.summary_for_input(&session_id, &input)?;
                let mut snapshot =
                    crate::run_record::conversation_snapshot(&session, &input, &history, names)
                        .map_err(invalid)?;
                snapshot.started = started;
                snapshot.parent_run_id = caller;
                let work = match (task, wave) {
                    (Some(task), _) => Some(crate::durable::WorkRef::Task(
                        TaskId::parse(&task).map_err(invalid)?,
                    )),
                    (None, Some(wave)) => Some(crate::durable::WorkRef::Wave(
                        crate::id::WaveId::parse(&wave).map_err(invalid)?,
                    )),
                    _ => None,
                };
                Ok((work, snapshot))
            })
            .collect()
    }

    pub(crate) fn import_session(
        &self,
        mut session: AgentSession,
        review: Option<&FlowSession>,
        history: &[crate::session::SessionObservation],
        dry_run: bool,
    ) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(flow) = review {
            super::flows::import_flow_in(&tx, flow)?;
        }
        resolve_ancestry_in(&tx, &mut session)?;
        if let Some(mut saved) = session_in(&tx, &session.id)? {
            // A later rename or closure is conversation state, never overwritten by import.
            if saved.title_source == TitleSource::Human {
                session.title = saved.title.clone();
                session.title_source = saved.title_source;
            }
            if session.ready_summary.is_none() {
                session.ready_summary = saved.ready_summary.clone();
            }
            if session.completed_at.is_none() {
                session.completed_at = saved.completed_at;
            }
            // File modification time was the only chronology for unopened conversations.
            if !session.input_published {
                session.created_at = saved.created_at;
                if session.completed_at.is_some() {
                    session.completed_at = saved.completed_at;
                }
            }
            // Captured names with generated provenance may have been enriched from their file.
            if session.title_source == TitleSource::Generated
                && saved.title_source == TitleSource::Generated
            {
                saved.title = session.title.clone();
            }
            if saved != session {
                return Err(invalid(format!(
                    "Session {} conflicts with its recorded input or answer",
                    session.id
                )));
            }
            let changed = retain_history_in(&tx, &session, history)?;
            if !dry_run {
                tx.commit()?;
            }
            return Ok(changed);
        }
        insert_session_in(&tx, &session)?;
        retain_history_in(&tx, &session, history)?;
        let pending = review.filter(|flow| flow.pending_session_id.as_deref() == Some(&session.id));
        if let Some(flow) = pending {
            let current = super::flows::flow_in(&tx, flow.id())?.ok_or(StoreError::NotFound)?;
            if !current.finished
                && current.cursor == flow.cursor
                && current.pending_session_id.is_none()
            {
                tx.execute(
                    "UPDATE flow_sessions SET pending_session_id=?2,current_run_id=?3 WHERE id=?1",
                    params![
                        session.flow_session_id,
                        session.id,
                        session.input_id.as_str()
                    ],
                )?;
            }
        }
        if !dry_run {
            tx.commit()?;
        }
        Ok(true)
    }

    pub fn reserve_review_run(
        &self,
        expected: &FlowSession,
    ) -> StoreResult<(FlowSession, AgentSession)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = super::flows::flow_in(&tx, expected.id())?.ok_or(StoreError::NotFound)?;
        if current != *expected || current.claim.is_some() || current.failure.is_some() {
            return Err(StoreError::InvalidAuthority(
                "review changed before input reservation".into(),
            ));
        }
        let id = review_id(expected)?;
        let mut session = session_in(&tx, &id)?.ok_or(StoreError::NotFound)?;
        if session.completed_at.is_some() {
            return Err(StoreError::InvalidAuthority("review is complete".into()));
        }
        if session.input_published {
            session.input_id = RunId::new();
            session.input_published = false;
            replace_input_in(&tx, &session)?;
            tx.execute(
                "UPDATE flow_sessions SET position_version=position_version+1 WHERE id=?1",
                [expected.id()],
            )?;
        }
        let flow = super::flows::flow_in(&tx, expected.id())?.ok_or(StoreError::NotFound)?;
        select_input_in(&tx, &flow, &session)?;
        let flow = super::flows::flow_in(&tx, expected.id())?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok((flow, session))
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
            AND claim_json IS NULL AND EXISTS(SELECT 1 FROM agent_sessions s WHERE s.id=?1 AND s.input_id=?2 AND s.input_published=0 AND s.completed_at IS NULL)",
            params![session_id, run_id.as_str(), i64::try_from(version).map_err(invalid)?])? != 1 {
            return Err(StoreError::InvalidAuthority("review Run reservation is stale".into()));
        }
        tx.execute(
            "UPDATE agent_sessions SET input_published=1, provider=?2, model=?3 WHERE input_id=?1",
            params![run_id.as_str(), provider, model],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn session(&self, id: &str) -> StoreResult<Option<AgentSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        session_in(&conn, id)
    }

    pub fn session_for_run(&self, run_id: &RunId) -> StoreResult<Option<AgentSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            &format!("{SESSION_SELECT} WHERE s.id=(SELECT session_id FROM agent_session_inputs WHERE input_id=?1)"),
            [run_id.as_str()],
            read_session,
        )
        .optional()?
        .transpose()
    }

    /// Admit the conversation and its captured input before provider effects.
    pub fn create_session(
        &self,
        session: AgentSession,
        review: Option<&FlowSession>,
        caller_exec: Option<&crate::id::ExecId>,
    ) -> StoreResult<AgentSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = session_in(&tx, &session.id)? {
            return Ok(existing);
        }
        if let Some(flow) = review {
            super::flows::insert_flow_in(&tx, flow)?;
        }
        let session = reserve_session_in(&tx, session, caller_exec)?;
        if session.kind == SessionKind::FlowReview {
            tx.execute(
                "UPDATE flow_sessions SET pending_session_id=?2,current_run_id=?3 WHERE id=?1",
                params![
                    session.flow_session_id,
                    session.id,
                    session.input_id.as_str()
                ],
            )?;
        }
        tx.commit()?;
        Ok(session)
    }

    /// Replace captured input under the existing conversation. Native history,
    /// title, feedback and assignment remain on their owners.
    pub fn replace_session_input(
        &self,
        expected_input: &RunId,
        session: AgentSession,
    ) -> StoreResult<AgentSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous = session_in(&tx, &session.id)?.ok_or(StoreError::NotFound)?;
        if previous.input_id != *expected_input
            || previous.completed_at.is_some()
            || previous.task_id != session.task_id
            || previous.wave_id != session.wave_id
            || previous.flow_session_id != session.flow_session_id
        {
            return Err(StoreError::InvalidAuthority(
                "conversation changed before input replacement".into(),
            ));
        }
        replace_input_in(&tx, &session)?;
        tx.execute("UPDATE flow_sessions SET current_run_id=?2 WHERE current_run_id=?1 AND state='current'",
            params![expected_input.as_str(),session.input_id.as_str()])?;
        let session = session_in(&tx, &session.id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(session)
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
            "UPDATE agent_sessions SET provider=?2, model=?3 WHERE input_id=?1 AND provider IS NULL",
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
            "UPDATE agent_sessions SET provider=?2, model=?3 WHERE input_id=?1 AND input_published=0",
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
    ) -> StoreResult<AgentSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session = session_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        if session.input_id != *expected_run {
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
            let wave = super::durable::task_wave_in(&tx, task)?;
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
    ) -> StoreResult<Vec<AgentSession>> {
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
            "UPDATE agent_sessions SET completed_at=?3 WHERE id=?1 AND input_id=?2
             AND completed_at IS NULL AND (kind='conversation' OR ready_summary IS NOT NULL)
             AND NOT EXISTS(SELECT 1 FROM tasks t WHERE t.id=agent_sessions.task_id
                 AND t.current_invocation_id=agent_sessions.flow_session_id)",
            params![id, expected_run.as_str(), crate::store::rows::now_unix()],
        )? != 1
        {
            return Err(StoreError::InvalidAuthority(
                "Session changed before completion".into(),
            ));
        }
        Ok(())
    }

    pub fn session_inputs(&self, id: &str) -> StoreResult<Vec<RunId>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT input_id FROM agent_session_inputs WHERE session_id=?1 ORDER BY input_id",
        )?;
        let rows = query.query_map([id], |row| row.get::<_, String>(0))?;
        rows.map(|row| RunId::parse(&row?).map_err(invalid))
            .collect()
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
        let session = session_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        if expected_run.is_some_and(|run| *run != session.input_id) {
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
            "UPDATE agent_sessions SET ready_summary=?3 WHERE id=?1 AND input_id=?2
             AND completed_at IS NULL AND (kind='ask' OR EXISTS(SELECT 1 FROM flow_sessions f WHERE f.id=agent_sessions.flow_session_id
                 AND agent_sessions.input_published=1 AND f.pending_session_id=?1 AND f.state='current'
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

fn historical_agent_input(conn: &Connection, row: &serde_json::Value) -> StoreResult<bool> {
    match row["provider"].as_str() {
        Some("loopflow") => return Ok(false),
        Some(_) => return Ok(true),
        None if row["session_id"].is_string() => return Ok(true),
        None => {}
    }
    let (Some(flow), Some(node)) = (row["invocation_id"].as_str(), row["node"].as_u64()) else {
        return Ok(false);
    };
    let Some(flow) = super::flows::flow_in(conn, flow)? else {
        return Ok(false);
    };
    let graph =
        crate::engine::flow_graph::FlowGraph::new(&flow.invocation.flow, &flow.invocation.steps);
    Ok(graph
        .node_at(u32::try_from(node).map_err(invalid)?)
        .is_some_and(|node| {
            matches!(
                node.kind,
                crate::engine::flow_graph::FlowNodeKind::Skill
                    | crate::engine::flow_graph::FlowNodeKind::Xor
            )
        }))
}

fn historical_operation_retained(conn: &Connection, row: &serde_json::Value) -> StoreResult<bool> {
    let saved: Option<String> = conn.query_row(
        "SELECT json_extract(e.payload,'$.sql') FROM flow_events e
         WHERE e.flow_id=?1 AND e.kind='operation_started' AND json_extract(e.payload,'$.legacy_run_id')=?2
         AND e.node=?3 AND e.iterations=?4 AND (?5 IS NULL OR EXISTS(
             SELECT 1 FROM flow_events c WHERE c.operation_start=e.seq AND c.kind='operation_completed'
             AND c.flow_id=e.flow_id AND c.node=e.node AND c.iterations=e.iterations
             AND c.outcome=?5 AND c.observed_at IS ?6))",
        params![row["invocation_id"].as_str(), row["id"].as_str(), row["node"].as_i64(),
            row["iterations"].as_str(), row["outcome"].as_str(), row["ended_at"].as_i64()],
        |r| r.get(0),
    ).optional()?.flatten();
    saved
        .map(|saved| Ok(serde_json::from_str::<serde_json::Value>(&saved)? == *row))
        .unwrap_or(Ok(false))
}

/// Sessionless agent rows used the input ID as their conversation selector.
/// Mechanical or unclassified rows remain on the legacy table until their
/// Flow/command evidence has a destination; they are never agent conversations.
fn historical_conversation(row: &serde_json::Value) -> StoreResult<AgentSession> {
    let input: String = serde_json::from_value(row["id"].clone())?;
    if !row["session_id"].is_null() {
        return Err(invalid(format!(
            "historical input {input} names an unavailable Session"
        )));
    }
    let skill: Option<String> = serde_json::from_value(row["skill"].clone())?;
    let iterations = row["iterations"]
        .as_str()
        .map(serde_json::from_str)
        .transpose()?;
    Ok(AgentSession {
        id: input.clone(),
        input_id: RunId::parse(&input).map_err(invalid)?,
        caller_input_id: row["caller_run_id"]
            .as_str()
            .map(RunId::parse)
            .transpose()
            .map_err(invalid)?,
        input_published: row["published"] == 1,
        cwd: serde_json::from_value(row["cwd"].clone())?,
        title: skill.clone().unwrap_or(input),
        title_source: TitleSource::Generated,
        skill,
        provider: serde_json::from_value(row["provider"].clone())?,
        model: serde_json::from_value(row["model"].clone())?,
        node: serde_json::from_value(row["node"].clone())?,
        iterations,
        task_id: row["task_id"]
            .as_str()
            .map(TaskId::parse)
            .transpose()
            .map_err(invalid)?,
        wave_id: row["wave_id"]
            .as_str()
            .map(crate::id::WaveId::parse)
            .transpose()
            .map_err(invalid)?,
        flow_session_id: serde_json::from_value(row["invocation_id"].clone())?,
        work_source: serde_json::from_value(row["work_source"].clone())?,
        bound_at: None,
        kind: SessionKind::Conversation,
        interactive: false,
        repo: None,
        request: None,
        ready_summary: None,
        completed_at: None,
        created_at: serde_json::from_value(row["created_at"].clone())?,
    })
}

pub(super) fn retain_history_in(
    conn: &Connection,
    session: &AgentSession,
    history: &[crate::session::SessionObservation],
) -> StoreResult<bool> {
    let mut changed = false;
    for observation in history {
        if observation.source == "runs" {
            let caller = observation.payload["evidence"]["caller_run_id"].as_str();
            let saved: Option<(String, Option<String>)> = conn
                .query_row(
                    "SELECT session_id,caller_input_id FROM agent_session_inputs WHERE input_id=?1",
                    [observation.input_id.as_str()],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;
            match saved {
                Some((owner, original_caller))
                    if owner != session.id || original_caller.as_deref() != caller =>
                {
                    return Err(invalid(format!(
                        "input {} has conflicting Session or caller evidence",
                        observation.input_id
                    )));
                }
                Some(_) => {}
                None => {
                    conn.execute("INSERT INTO agent_session_inputs(input_id,session_id,caller_input_id) VALUES(?1,?2,?3)",
                        params![observation.input_id.as_str(),session.id,caller])?;
                    changed = true;
                }
            }
        }
        let key = format!("{}:{}", observation.input_id, observation.source);
        let payload = serde_json::to_string(&observation.payload)?;
        let saved: Option<String> = conn.query_row(
            "SELECT payload FROM session_events WHERE session_id=?1 AND kind='observed' AND receipt_key=?2",
            params![session.id,key], |row| row.get(0)).optional()?;
        if let Some(saved) = saved {
            if saved != payload {
                return Err(invalid(format!(
                    "input {} has conflicting {} evidence",
                    observation.input_id, observation.source
                )));
            }
            continue;
        }
        conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,task_id,wave_id,observed_at,payload)
            VALUES(?1,'observed',?2,?3,?4,?5,?6)",
            params![session.id,key,observation.task_id.as_ref().map(TaskId::as_str),
                observation.wave_id.as_ref().map(crate::id::WaveId::as_str),observation.observed_at,payload])?;
        changed = true;
    }
    Ok(changed)
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
            "only a review reserves a review Session".into(),
        ));
    }
    let id = review_id(flow)?;
    let session = match session_in(conn, &id)? {
        Some(session) if session.completed_at.is_some() => {
            return Err(StoreError::InvalidAuthority(
                "completed review cannot be reopened by a cursor write".into(),
            ))
        }
        Some(session) => session,
        None => {
            let title = conn.query_row(
                "SELECT issue_title FROM tasks WHERE id=?1",
                [flow.task_id.as_ref().map(TaskId::as_str)],
                |row| row.get(0),
            )?;
            reserve_flow_conversation_in(conn, flow, id, SessionKind::FlowReview, title)?
        }
    };
    if conn.execute("UPDATE flow_sessions SET pending_session_id=?2 WHERE id=?1 AND state='current' AND position_version=?3 AND claim_json IS NULL",
        params![flow.id(),session.id,i64::try_from(flow.version).map_err(invalid)?])? != 1
    {
        return Err(StoreError::InvalidAuthority("review changed before reservation".into()));
    }
    select_input_in(conn, flow, &session)
}

pub(super) fn reserve_flow_conversation_in(
    conn: &Transaction<'_>,
    flow: &FlowSession,
    id: String,
    kind: SessionKind,
    title: String,
) -> StoreResult<AgentSession> {
    reserve_session_in(
        conn,
        AgentSession {
            caller_input_id: None,
            id,
            input_id: RunId::new(),
            input_published: false,
            cwd: flow.cwd.clone(),
            skill: Some(flow.current().step),
            provider: None,
            model: None,
            node: None,
            iterations: None,
            task_id: flow.task_id.clone(),
            wave_id: flow.wave_id.clone(),
            flow_session_id: Some(flow.id().to_owned()),
            work_source: flow.declared_work().map(|_| WorkSource::Inherited),
            bound_at: None,
            kind,
            interactive: kind != SessionKind::Conversation,
            repo: None,
            title,
            title_source: TitleSource::Generated,
            request: None,
            ready_summary: None,
            completed_at: None,
            created_at: crate::store::rows::now_unix(),
        },
        None,
    )
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
        .ok_or_else(|| StoreError::InvalidAuthority("review has no published input".into()))?;
    if conn.execute(
        "UPDATE agent_sessions SET completed_at=?3 WHERE id=?1 AND input_id=?2
        AND completed_at IS NULL AND ready_summary IS ?4
        AND EXISTS(SELECT 1 FROM flow_sessions WHERE id=?5 AND current_run_id=?2 AND state='current')",
        params![id, run_id.as_str(), crate::store::rows::now_unix(), summary, expected.id()],
    )? != 1
    {
        return Err(StoreError::InvalidAuthority(
            "review changed before completion".into(),
        ));
    }
    Ok(())
}

/// Resolve ancestry and captured location on the conversation's admission transaction.
pub(super) fn reserve_session_in(
    conn: &Transaction<'_>,
    mut session: AgentSession,
    caller: Option<&crate::id::ExecId>,
) -> StoreResult<AgentSession> {
    if session.flow_session_id.is_none()
        && (session.work_source.is_none() || session.work_source == Some(WorkSource::Inherited))
    {
        if let Some(caller) = caller {
            if let Some(work) = super::execs::agent_work_in(conn, caller)? {
                session.task_id = work.task_id;
                session.wave_id = work.wave_id;
                session.work_source = Some(work.source);
            }
        }
    }
    if let Some(id) = &session.flow_session_id {
        let flow = super::flows::flow_in(conn, id)?.ok_or(StoreError::NotFound)?;
        if session.task_id.is_some() && session.task_id != flow.task_id {
            return Err(invalid("AgentSession and FlowSession Tasks disagree"));
        }
        session.task_id = flow.task_id;
        if let Some(wave) = flow.wave_id {
            if session.wave_id.as_ref().is_some_and(|given| given != &wave) {
                return Err(invalid("AgentSession and FlowSession Waves disagree"));
            }
            session.wave_id = Some(wave);
        }
        let (node, iterations) = flow.invocation.location(&flow.cursor).map_err(invalid)?;
        if session.node.is_some_and(|given| given != node)
            || session
                .iterations
                .as_ref()
                .is_some_and(|given| given != &iterations)
        {
            return Err(invalid(
                "AgentSession location differs from captured Flow cursor",
            ));
        }
        session.node = Some(node);
        session.iterations = Some(iterations);
    }
    resolve_ancestry_in(conn, &mut session)?;
    insert_session_in(conn, &session)?;
    Ok(session)
}

fn resolve_ancestry_in(conn: &Connection, session: &mut AgentSession) -> StoreResult<()> {
    if let Some(task) = &session.task_id {
        let wave = super::durable::task_wave_in(conn, task)?;
        if session.wave_id.as_ref().is_some_and(|given| given != &wave) {
            return Err(invalid("AgentSession Task and Wave disagree"));
        }
        session.wave_id = Some(wave);
    }
    if session.repo.is_none() && session.cwd.is_dir() {
        session.repo = crate::repo::discover_repo_root(&session.cwd)
            .map_err(invalid)?
            .map(|root| {
                crate::repository::CanonicalRepo::discover(&root).map(|repo| repo.to_string())
            })
            .transpose()
            .map_err(invalid)?;
    }
    Ok(())
}

fn insert_session_in(conn: &Connection, session: &AgentSession) -> StoreResult<()> {
    conn.execute("INSERT INTO agent_sessions(id,input_id,title,title_source,ready_summary,completed_at,
        created_at,kind,request,interactive,repo,task_id,wave_id,flow_session_id,work_source,bound_at,
        input_published,cwd,skill,provider,model,node,iterations)
        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22,?23)",
        params![session.id,session.input_id.as_str(),session.title,title_source(session.title_source),
            session.ready_summary,session.completed_at,session.created_at,
            serde_json::to_value(session.kind)?.as_str(),session.request,session.interactive,session.repo,
            session.task_id.as_ref().map(TaskId::as_str),session.wave_id.as_ref().map(crate::id::WaveId::as_str),
            session.flow_session_id,session.work_source.map(serde_json::to_value).transpose()?.as_ref().and_then(serde_json::Value::as_str),
            session.bound_at,session.input_published,session.cwd.to_string_lossy(),session.skill,session.provider,session.model,
            session.node,session.iterations.as_ref().map(serde_json::to_string).transpose()?])?;
    conn.execute(
        "INSERT INTO agent_session_inputs(input_id,session_id,caller_input_id) VALUES(?1,?2,?3)",
        params![
            session.input_id.as_str(),
            session.id,
            session.caller_input_id.as_ref().map(RunId::as_str)
        ],
    )?;
    Ok(())
}

pub(super) fn replace_input_in(conn: &Transaction<'_>, session: &AgentSession) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO agent_session_inputs(input_id,session_id,caller_input_id) VALUES(?1,?2,?3)",
        params![
            session.input_id.as_str(),
            session.id,
            session.caller_input_id.as_ref().map(RunId::as_str)
        ],
    )?;
    conn.execute("UPDATE agent_sessions SET input_id=?2,input_published=?3,cwd=?4,skill=?5,provider=?6,model=?7
        WHERE id=?1 AND completed_at IS NULL",
        params![session.id,session.input_id.as_str(),session.input_published,session.cwd.to_string_lossy(),
            session.skill,session.provider,session.model])?;
    Ok(())
}

pub(super) fn select_input_in(
    conn: &Transaction<'_>,
    flow: &FlowSession,
    session: &AgentSession,
) -> StoreResult<()> {
    let (node, iterations) = flow.invocation.location(&flow.cursor).map_err(invalid)?;
    if session.flow_session_id.as_deref() != Some(flow.id())
        || session.node != Some(node)
        || session.iterations.as_ref() != Some(&iterations)
    {
        return Err(StoreError::InvalidAuthority(
            "conversation does not belong to this captured Flow boundary".into(),
        ));
    }
    if conn.execute("UPDATE flow_sessions SET current_run_id=?3 WHERE id=?1 AND position_version=?2 AND state='current'",
        params![flow.id(),i64::try_from(flow.version).map_err(invalid)?,session.input_id.as_str()])? != 1
    {
        return Err(StoreError::InvalidAuthority("Flow changed before conversation selection".into()));
    }
    Ok(())
}
