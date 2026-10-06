//! Session transactions share the invocation's SQLite transaction and fences.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::durable::TaskId;
use crate::session::{AgentSession, PrimaryScope, TitleSource};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

/// The Flow whose driver recorded, as a step, the Exec that captured session
/// `s`'s current input. Sessions carry no Flow column of their own.
macro_rules! session_flow {
    () => {
        "(SELECT fs.flow_exec_id FROM session_events captured JOIN flow_exec_steps fs ON fs.exec_id=captured.exec_id WHERE captured.seq=s.current_capture)"
    };
}
/// That step's node and loop counts, as its driver recorded them.
macro_rules! session_step {
    ($column:literal) => {
        concat!("(SELECT fs.", $column, " FROM session_events captured JOIN flow_exec_steps fs ON fs.exec_id=captured.exec_id WHERE captured.seq=s.current_capture)")
    };
}
pub(super) const SESSION_FLOW: &str = session_flow!();

const SESSION_SELECT: &str = concat!("SELECT s.id,COALESCE(c.receipt_key,'') AS artifact_key,s.title,s.title_source,
    (SELECT json_extract(feedback.payload,'$.summary') FROM session_events feedback WHERE feedback.session_id=s.id AND feedback.kind='observed' AND feedback.receipt_key='legacy_review_feedback'),s.completed_at,s.created_at,s.request,s.interactive,s.repo,
    s.task_id,s.wave_id,", session_flow!(), ",s.work_source,s.bound_at,
    s.input_published,s.cwd,s.skill,s.provider,s.model,", session_step!("node"), ",", session_step!("iterations"), ",json_extract(c.payload,'$.caller_key'),s.current_capture FROM agent_sessions s LEFT JOIN session_events c ON c.seq=s.current_capture");

fn read_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoreResult<AgentSession>> {
    Ok((|| {
        Ok(AgentSession {
            captured: row.get(23)?,
            caller_artifact_key: row
                .get::<_, Option<String>>(22)?
                .map(|id| crate::session_record::parse_artifact_key(&id))
                .transpose()
                .map_err(invalid)?,
            id: row.get(0)?,
            artifact_key: crate::session_record::parse_artifact_key(&row.get::<_, String>(1)?)
                .map_err(invalid)?,
            title: row.get(2)?,
            title_source: serde_json::from_value(serde_json::Value::String(row.get(3)?))?,
            ready_summary: row.get(4)?,
            completed_at: row.get(5)?,
            created_at: row.get(6)?,
            request: row.get(7)?,
            interactive: row.get(8)?,
            repo: row.get(9)?,
            task_id: row
                .get::<_, Option<String>>(10)?
                .map(|id| TaskId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            wave_id: row
                .get::<_, Option<String>>(11)?
                .map(|id| crate::id::WaveId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            flow_id: row.get(12)?,
            work_source: row
                .get::<_, Option<String>>(13)?
                .map(|source| serde_json::from_value(serde_json::Value::String(source)))
                .transpose()?,
            bound_at: row.get(14)?,
            input_published: row.get(15)?,
            cwd: row.get::<_, String>(16)?.into(),
            skill: row.get(17)?,
            provider: row.get(18)?,
            model: row.get(19)?,
            node: row.get(20)?,
            iterations: row
                .get::<_, Option<String>>(21)?
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

/// Whether agent_sessions row `session` waits on a person at `now`: its
/// current driver's reading shows an unanswered question, or no unresolved
/// tool call and either an interactive turn handed back or a quiet stream.
/// No reading, or one from a driver that has let go, is not Waiting.
fn waiting_sql(session: &str, now: i64) -> String {
    format!(
        "EXISTS(SELECT 1 FROM session_activity act WHERE act.session_id={session}.id
            AND {session}.completed_at IS NULL AND act.driver_generation={session}.driver_generation
            AND (act.pending_input>0 OR (act.open_tools=0 AND (({session}.interactive=1 AND act.yielded=1)
                OR {now}-act.observed_at>={quiet}))))",
        quiet = crate::session::WAITING_QUIET_SECONDS
    )
}

fn inventory_query(
    filter: &crate::session::SessionFilter,
    select: &str,
    now: i64,
) -> StoreResult<(String, Vec<rusqlite::types::Value>)> {
    use rusqlite::types::Value;
    let mut sql = format!("{select} WHERE 1");
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
        sql.push_str(" AND s.completed_at IS NULL");
    }
    if filter.waiting {
        sql.push_str(&format!(" AND {}", waiting_sql("s", now)));
    }
    if let Some(repo) = &filter.repo {
        sql.push_str(&format!(
            " AND COALESCE(s.repo,(SELECT repo FROM waves WHERE id=s.wave_id))={}",
            bind(Value::Text(repo.clone()))
        ));
    }
    if let Some(task) = &filter.task {
        let task = bind(Value::Text(task.clone()));
        sql.push_str(&format!(
            " AND s.id IN ({})",
            super::task_work::session_ids(&task)
        ));
    }
    if filter.orphan {
        sql.push_str(&format!(
            " AND NOT EXISTS ({})",
            super::task_work::session_tasks("s")
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
    if let Some(after) = &filter.after {
        let after = bind(Value::Text(after.clone()));
        sql.push_str(&format!(" AND s.id>{after} ORDER BY s.id LIMIT {limit}"));
    } else {
        let offset = bind(Value::Integer(
            i64::try_from(filter.offset).map_err(invalid)?,
        ));
        sql.push_str(&format!(
            " ORDER BY s.title, s.id LIMIT {limit} OFFSET {offset}"
        ));
    }
    Ok((sql, values))
}

// Preserve the existing filters/order. Materialize only the selected metadata
// before joining Flow/Work labels; no request or historical payload is selected.
const SUMMARY_SELECT: &str = concat!("SELECT s.id,c.receipt_key AS artifact_key,s.title,s.title_source,
    (SELECT json_extract(feedback.payload,'$.summary') FROM session_events feedback WHERE feedback.session_id=s.id AND feedback.kind='observed' AND feedback.receipt_key='legacy_review_feedback'),s.completed_at,s.interactive,s.task_id,s.wave_id,
    ", session_flow!(), ",s.cwd,s.skill,s.provider,s.model,", session_step!("node"), ",", session_step!("iterations"), ",s.current_capture
    FROM agent_sessions s LEFT JOIN session_events c ON c.seq=s.current_capture");

const MEMBERSHIP_KIND: &str = "CASE WHEN json_valid(payload) THEN CASE WHEN json_extract(payload,'$.source')='manifest.json' AND json_extract(payload,'$.evidence.schema_version')=1 AND json_extract(payload,'$.evidence.artifact_key')=json_extract(payload,'$.input_id') AND receipt_key=json_extract(payload,'$.input_id')||':manifest.json' THEN json_extract(payload,'$.evidence.flow.kind') END END";

fn summary_query(page: &str, by_id: bool, now: i64) -> String {
    let waiting = waiting_sql("a", now);
    let order = if by_id { "s.id" } else { "s.title,s.id" };
    // A Flow is the driver Exec above the step that captured the current input.
    format!("WITH page AS MATERIALIZED ({page})
        SELECT s.*,driver.id,flow.flow,driver.outcome,driver.completed_at,step.started_at,
        (fs.seq=(SELECT MAX(later.seq) FROM flow_exec_steps later WHERE later.flow_exec_id=fs.flow_exec_id)),
        w.slug,t.issue_identifier,
        ((SELECT {MEMBERSHIP_KIND} FROM session_events INDEXED BY session_input_membership
         WHERE session_id=s.id AND captured_event=s.current_capture
         AND kind='observed' AND substr(receipt_key,-14)=':manifest.json')='independent'),
        (SELECT json_group_array(id) FROM ({})),
        a.primary_scope,
        (SELECT CASE WHEN json_valid(e.payload) THEN json_extract(e.payload,'$.outcome') END FROM session_events e INDEXED BY session_driver_exit
            WHERE e.session_id=s.id AND e.receipt_key='driver:'||(a.driver_generation-1)||':exit' AND e.kind='observed'),
        {waiting},
        COALESCE(t.work_state IN ('done','abandoned'),0),
        EXISTS(SELECT 1 FROM tasks p WHERE p.primary_session_id=s.id)
        FROM page s JOIN agent_sessions a ON a.id=s.id
        LEFT JOIN session_events captured ON captured.seq=s.current_capture
        LEFT JOIN flow_exec_steps fs ON fs.exec_id=captured.exec_id
        LEFT JOIN flow_execs flow ON flow.exec_id=fs.flow_exec_id
        LEFT JOIN execs step ON step.id=fs.exec_id
        LEFT JOIN execs driver ON driver.id=fs.flow_exec_id
        LEFT JOIN wave_addresses w ON w.id=s.wave_id
        LEFT JOIN tasks t ON t.id=s.task_id
        ORDER BY {order}", super::task_work::session_tasks("s"))
}

fn read_summary(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<StoreResult<crate::session::SessionSummary>> {
    Ok((|| {
        let task_id = row
            .get::<_, Option<String>>(7)?
            .map(|id| TaskId::parse(&id))
            .transpose()
            .map_err(invalid)?;
        let wave_id = row
            .get::<_, Option<String>>(8)?
            .map(|id| crate::id::WaveId::parse(&id))
            .transpose()
            .map_err(invalid)?;
        let flow = match row.get::<_, Option<String>>(17)? {
            Some(driver) => {
                let completed: Option<i64> = row.get(20)?;
                let name: String = row.get(18)?;
                Some(crate::session::FlowSummary {
                    id: driver,
                    name,
                    state: crate::session::FlowSummaryState::of_driver(
                        row.get::<_, Option<String>>(19)?.as_deref(),
                        completed,
                    ),
                    task_id: task_id.clone(),
                    wave_id: wave_id.clone(),
                    updated_at: match completed {
                        Some(completed) => completed,
                        None => row.get(21)?,
                    },
                })
            }
            None => None,
        };
        Ok(crate::session::SessionSummary {
            task_ids: serde_json::from_str(&row.get::<_, String>(26)?)?,
            primary_scope: row.get(27)?,
            driver_outcome: row.get(28)?,
            waiting: row.get(29)?,
            task_terminal: row.get(30)?,
            task_primary: row.get(31)?,
            captured: row.get(16)?,
            id: row.get(0)?,
            artifact_key: crate::session_record::parse_artifact_key(&row.get::<_, String>(1)?)
                .map_err(invalid)?,
            title: row.get(2)?,
            title_source: serde_json::from_value(serde_json::Value::String(row.get(3)?))?,
            ready_summary: row.get(4)?,
            completed_at: row.get(5)?,
            interactive: row.get(6)?,
            task_id,
            wave_id,
            flow_id: row.get(9)?,
            cwd: row.get::<_, String>(10)?.into(),
            skill: row.get(11)?,
            provider: row.get(12)?,
            model: row.get(13)?,
            node: row.get(14)?,
            iterations: row
                .get::<_, Option<String>>(15)?
                .map(|raw| serde_json::from_str(&raw))
                .transpose()?,
            flow,
            flow_step_latest: row.get::<_, Option<bool>>(22)?.unwrap_or(false),
            wave_name: row.get(23)?,
            task_identifier: row.get(24)?,
            independent: row.get::<_, Option<bool>>(25)?.unwrap_or(false),
        })
    })())
}

fn interactive_sessions_in<P: rusqlite::Params>(
    conn: &Connection,
    selection: &str,
    params: P,
) -> StoreResult<Vec<(AgentSession, Option<i64>)>> {
    let mut query = conn.prepare(&format!(
        "SELECT candidates.*, (
            SELECT MAX(json_extract(payload,'$.opened_at_ms'))
            FROM session_events WHERE session_id=candidates.id AND kind='observed'
            AND json_extract(payload,'$.type')='interactive_opened'
        ) AS opened_at FROM ({SESSION_SELECT}
        WHERE s.interactive=1 {selection}) AS candidates"
    ))?;
    let rows = query.query_map(params, |row| {
        Ok((read_session(row)?, row.get("opened_at")?))
    })?;
    rows.map(|row| {
        let (session, opened) = row?;
        Ok((session?, opened))
    })
    .collect()
}

fn task_primary_in(conn: &Connection, task: &TaskId) -> StoreResult<Option<AgentSession>> {
    conn.query_row(
        &format!(
            "{SESSION_SELECT} WHERE s.id=(SELECT primary_session_id FROM tasks WHERE id=?1)
             AND s.completed_at IS NULL"
        ),
        [task.as_str()],
        read_session,
    )
    .optional()?
    .transpose()
}

impl SqliteStore {
    pub(crate) fn resume_candidates(&self) -> StoreResult<Vec<(AgentSession, Option<i64>)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        interactive_sessions_in(&conn, "", [])
    }

    /// A Task's unfinished interactive conversations, each with its last opening.
    pub(crate) fn task_conversations(
        &self,
        task: &TaskId,
    ) -> StoreResult<Vec<(AgentSession, Option<i64>)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        interactive_sessions_in(
            &conn,
            &format!(
                "AND s.completed_at IS NULL AND s.id IN ({})",
                super::task_work::session_ids("?1")
            ),
            [task.as_str()],
        )
    }

    /// The conversation a Task's primary pointer names, while it is unfinished.
    pub(crate) fn task_primary(&self, task: &TaskId) -> StoreResult<Option<AgentSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        task_primary_in(&conn, task)
    }

    /// Name one of the Task's own unfinished interactive conversations its
    /// primary. The earlier choice stays an ordinary conversation of the Task.
    pub(crate) fn choose_task_primary(
        &self,
        task: &TaskId,
        session: &str,
    ) -> StoreResult<AgentSession> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let chosen = conn.execute(
            &format!(
                "UPDATE tasks SET primary_session_id=?2 WHERE id=?1 AND ?2 IN (
                    SELECT s.id FROM agent_sessions s WHERE s.interactive=1
                    AND s.completed_at IS NULL AND s.id IN ({}))",
                super::task_work::session_ids("?1")
            ),
            params![task.as_str(), session],
        )?;
        if chosen != 1 {
            return Err(StoreError::InvalidAuthority(format!(
                "Session {session} is not an unfinished interactive conversation of this Task"
            )));
        }
        task_primary_in(&conn, task)?
            .ok_or_else(|| invalid(format!("Session {session} disappeared while being chosen")))
    }

    /// Each row's Waiting is judged at `now`.
    pub(crate) fn session_summaries(
        &self,
        filter: &crate::session::SessionFilter,
        now: i64,
    ) -> StoreResult<Vec<crate::session::SessionSummary>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (page, values) = inventory_query(filter, SUMMARY_SELECT, now)?;
        let mut query = conn.prepare(&summary_query(&page, filter.after.is_some(), now))?;
        let rows = query.query_map(rusqlite::params_from_iter(values), read_summary)?;
        rows.map(|row| row?).collect()
    }

    pub(crate) fn session_summary(
        &self,
        id: &str,
        now: i64,
    ) -> StoreResult<Option<crate::session::SessionSummary>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            &summary_query(&format!("{SUMMARY_SELECT} WHERE s.id=?1"), false, now),
            [id],
            read_summary,
        )
        .optional()?
        .transpose()
    }

    /// Resolve a conversation or captured-input selector from its SQLite owner.
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
            SELECT receipt_key AS id FROM session_events WHERE kind='captured' UNION SELECT json_extract(payload,'$.caller_key') FROM session_events WHERE kind='captured')
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

    /// Sessions that recorded `thread` as their provider's own conversation id.
    pub(crate) fn sessions_for_provider_thread(&self, thread: &str) -> StoreResult<Vec<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT id FROM agent_sessions WHERE provider_thread=?1
             UNION SELECT session_id FROM session_events WHERE kind='observed'
                AND json_extract(payload,'$.evidence.provider_session_id')=?1
             ORDER BY 1",
        )?;
        let ids = query
            .query_map([thread], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    /// Project retained input history, with its own attribution and chronology.
    /// A continuation never moves earlier usage to the conversation's new binding.
    pub(crate) fn conversation_history(
        &self,
        wave: Option<&str>,
        project: Option<&str>,
        task: Option<&str>,
        caller: Option<&str>,
        since: i64,
        include_finished: bool,
    ) -> StoreResult<Vec<crate::session_record::SessionHistory>> {
        self.conversation_inputs(
            wave,
            project,
            task,
            caller,
            (since, include_finished, None),
            None,
        )
        .map(|(rows, _)| rows)
    }

    /// Select the recent presentation budget before history decoding. Every
    /// unterminated input is retained; explicit drills use conversation_history.
    pub(crate) fn recent_conversation_history(
        &self,
        wave: Option<&str>,
        project: Option<&str>,
        task: Option<&str>,
        since: i64,
        limit: usize,
    ) -> StoreResult<(Vec<crate::session_record::SessionHistory>, bool)> {
        let (rows, truncated) =
            self.conversation_inputs(wave, project, task, None, (since, false, Some(limit)), None)?;
        Ok((rows, truncated))
    }

    pub(crate) fn input_history(
        &self,
        selector: &str,
    ) -> StoreResult<crate::session_record::SessionHistory> {
        let selected = self.resolve_history_input(selector)?;
        let input = self
            .session(&selected)?
            .map(|session| session.artifact_key.to_string())
            .unwrap_or(selected);
        self.conversation_inputs(None, None, None, None, (0, false, None), Some(&input))?
            .0
            .pop()
            .ok_or(StoreError::NotFound)
    }

    fn conversation_inputs(
        &self,
        wave: Option<&str>,
        project: Option<&str>,
        task: Option<&str>,
        caller: Option<&str>,
        window: (i64, bool, Option<usize>),
        input: Option<&str>,
    ) -> StoreResult<(Vec<crate::session_record::SessionHistory>, bool)> {
        let inputs = {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let mut query = conn.prepare("WITH inputs AS (
                SELECT i.seq AS captured,i.receipt_key AS input_id,i.session_id,json_extract(i.payload,'$.caller_key') AS caller_input_id,
                    COALESCE(m.observed_at,i.observed_at) AS started,
                    CASE WHEN m.seq IS NOT NULL THEN m.task_id ELSE i.task_id END AS task_id,
                    CASE WHEN m.seq IS NOT NULL THEN m.wave_id ELSE i.wave_id END AS wave_id,
                    CASE WHEN terminal.seq IS NOT NULL AND NOT (
                        json_valid(terminal.payload) AND CASE WHEN json_valid(terminal.payload) THEN
                        COALESCE(json_type(terminal.payload,'$.evidence.outcome')='text',0)
                        AND unixepoch(json_extract(terminal.payload,'$.evidence.ended_at')) IS NOT NULL ELSE 0 END
                    ) THEN NULL WHEN EXISTS (
                        SELECT 1 FROM session_events origin WHERE origin.session_id=s.id
                        AND origin.captured_event=i.seq AND origin.kind='started' AND NOT EXISTS (
                            SELECT 1 FROM session_events done WHERE done.session_id=origin.session_id
                            AND done.provider_thread=origin.provider_thread AND done.provider_turn=origin.provider_turn
                            AND done.kind='completed')) THEN NULL ELSE
                        COALESCE(terminal.observed_at,(
                            SELECT MAX(done.observed_at) FROM session_events origin JOIN session_events done
                            ON done.session_id=origin.session_id AND done.provider_thread=origin.provider_thread
                            AND done.provider_turn=origin.provider_turn AND done.kind='completed'
                            WHERE origin.session_id=s.id AND origin.captured_event=i.seq AND origin.kind='started')) END AS ended, NULL AS thread, NULL AS turn
                FROM session_events i JOIN agent_sessions s ON s.id=i.session_id
                LEFT JOIN session_events m ON m.session_id=s.id AND m.kind='observed'
                    AND m.receipt_key=i.receipt_key||':manifest.json'
                LEFT JOIN session_events terminal ON terminal.session_id=s.id AND terminal.kind='observed'
                    AND terminal.receipt_key=i.receipt_key||':terminal.json'
                WHERE i.kind='captured' AND (?7 IS NULL OR i.receipt_key=?7) AND
                    (?7 IS NOT NULL OR m.seq IS NOT NULL
                    OR (i.receipt_key=(SELECT receipt_key FROM session_events WHERE seq=s.current_capture) AND s.input_published=1)
                    OR EXISTS(SELECT 1 FROM session_events origin WHERE origin.captured_event=i.seq AND origin.kind='started'))
                UNION ALL
                SELECT NULL,NULL,e.session_id,NULL,MIN(e.observed_at),origin.task_id,origin.wave_id,
                    MAX(CASE WHEN e.kind='completed' THEN e.observed_at END),e.provider_thread,e.provider_turn
                FROM session_events e LEFT JOIN session_events origin
                    ON origin.session_id=e.session_id AND origin.provider_thread=e.provider_thread
                    AND origin.provider_turn=e.provider_turn AND origin.kind='started'
                WHERE ?7 IS NULL AND e.kind IN ('started','usage','completed','output')
                    AND origin.captured_event IS NULL
                GROUP BY e.session_id,e.provider_thread,e.provider_turn)
                , eligible AS MATERIALIZED (
                SELECT *, SUM(ended IS NULL) OVER () AS unfinished,
                    ROW_NUMBER() OVER (PARTITION BY ended IS NULL ORDER BY started DESC,input_id DESC,session_id,thread,turn) AS ordinal,
                    COUNT(*) OVER () AS total
                FROM inputs
                WHERE (?1 IS NULL OR wave_id IN (SELECT id FROM wave_addresses WHERE id=?1 OR slug=?1)
                    OR EXISTS (SELECT 1 FROM session_events origin WHERE origin.session_id=inputs.session_id
                        AND inputs.captured IS NOT NULL AND origin.captured_event=inputs.captured AND origin.kind='started'
                        AND origin.wave_id IN (SELECT id FROM wave_addresses WHERE id=?1 OR slug=?1)))
                AND (?2 IS NULL OR task_id IN (SELECT t.id FROM tasks t JOIN projects p ON p.id=t.project_id
                    WHERE p.id=?2 OR p.project_slug=?2 OR p.external_project_id=?2)
                    OR EXISTS (SELECT 1 FROM session_events origin WHERE origin.session_id=inputs.session_id
                        AND inputs.captured IS NOT NULL AND origin.captured_event=inputs.captured AND origin.kind='started'
                        AND origin.task_id IN (SELECT t.id FROM tasks t JOIN projects p ON p.id=t.project_id
                            WHERE p.id=?2 OR p.project_slug=?2 OR p.external_project_id=?2)))
                AND (?3 IS NULL OR task_id IN (SELECT id FROM tasks WHERE id=?3 OR issue_identifier=?3 OR external_issue_id=?3)
                    OR EXISTS (SELECT 1 FROM session_events origin WHERE origin.session_id=inputs.session_id
                        AND inputs.captured IS NOT NULL AND origin.captured_event=inputs.captured AND origin.kind='started'
                        AND origin.task_id IN (SELECT id FROM tasks WHERE id=?3 OR issue_identifier=?3 OR external_issue_id=?3)))
                AND (?4 IS NULL OR caller_input_id=?4 OR caller_input_id IN (SELECT receipt_key FROM session_events WHERE kind='captured' AND session_id=?4))
                AND (started>=?5 OR (?6 AND ended>=?5)))
                SELECT eligible.session_id,eligible.input_id,eligible.caller_input_id,started,eligible.task_id,eligible.wave_id,
                    (SELECT slug FROM wave_addresses WHERE id=eligible.wave_id),
                    (SELECT issue_identifier FROM tasks WHERE id=eligible.task_id),
                    s.current_capture,s.cwd,s.repo,s.skill,s.provider,s.model,s.interactive,
                    total > MAX(?8,unfinished),COALESCE((SELECT json_extract(payload,'$.work_source') FROM session_events WHERE seq=eligible.captured),
                        CASE WHEN eligible.captured=s.current_capture AND s.bound_at IS NULL THEN s.work_source END),
                    eligible.captured,eligible.thread,eligible.turn
                FROM eligible JOIN agent_sessions s ON s.id=eligible.session_id
                WHERE (?8 IS NULL OR ended IS NULL OR ordinal<=MAX(?8-unfinished,0))
                ORDER BY started DESC,eligible.input_id DESC,eligible.session_id,eligible.thread,eligible.turn")?;
            let rows = query.query_map(
                params![
                    wave,
                    project,
                    task,
                    caller,
                    window.0,
                    window.1,
                    input,
                    window.2.map(|n| n as i64)
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        (
                            row.get::<_, Option<String>>(6)?,
                            row.get::<_, Option<String>>(7)?,
                        ),
                        row.get::<_, Option<i64>>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, Option<String>>(10)?,
                        row.get::<_, Option<String>>(11)?,
                        row.get::<_, Option<String>>(12)?,
                        row.get::<_, Option<String>>(13)?,
                        row.get::<_, bool>(14)?,
                        row.get::<_, Option<bool>>(15)?.unwrap_or(false),
                        row.get::<_, Option<String>>(16)?,
                        row.get::<_, Option<i64>>(17)?,
                        row.get::<_, Option<String>>(18)?,
                        row.get::<_, Option<String>>(19)?,
                    ))
                },
            )?;
            rows.collect::<Result<Vec<_>, _>>()?
        };
        let truncated = inputs.iter().any(|row| row.14);
        let scope = (wave, project, task);
        let histories = inputs
            .into_iter()
            .map(
                |(
                    session_id,
                    input,
                    caller,
                    started,
                    task,
                    wave,
                    names,
                    current_input,
                    cwd,
                    repo,
                    skill,
                    provider,
                    model,
                    interactive,
                    _,
                    work_source,
                    captured,
                    thread,
                    turn,
                )| {
                    let input = input
                        .as_deref()
                        .map(crate::session_record::parse_artifact_key)
                        .transpose()
                        .map_err(invalid)?;
                    let session = crate::session::HistoryCapture {
                        captured,
                        id: session_id.clone(),
                        current_capture: current_input,
                        caller_artifact_key: caller
                            .as_deref()
                            .map(crate::session_record::parse_artifact_key)
                            .transpose()
                            .map_err(invalid)?,
                        task_id: task
                            .as_deref()
                            .map(TaskId::parse)
                            .transpose()
                            .map_err(invalid)?,
                        wave_id: wave
                            .as_deref()
                            .map(crate::id::WaveId::parse)
                            .transpose()
                            .map_err(invalid)?,
                        observed_at: started,
                        cwd: cwd.into(),
                        repo,
                        skill,
                        provider,
                        model,
                        interactive,
                        work_source: work_source
                            .map(|source| serde_json::from_value(serde_json::Value::String(source)))
                            .transpose()?,
                    };
                    let history = match input.as_deref() {
                        Some(input) => {
                            self.summary_for_input_matching(&session_id, input, scope)?
                        }
                        None => self.orphaned_native_history(
                            &session_id,
                            scope,
                            thread.as_deref(),
                            turn.as_deref(),
                        )?,
                    };
                    let snapshot = crate::session_record::project_input_history(
                        &session,
                        input.as_deref(),
                        &history,
                        names,
                    )
                    .map_err(invalid)?;
                    Ok(snapshot)
                },
            )
            .collect::<StoreResult<Vec<_>>>()?;
        Ok((histories, truncated))
    }

    pub fn session(&self, id: &str) -> StoreResult<Option<AgentSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        session_in(&conn, id)
    }

    /// Resolve an artifact alias to its immutable captured-event sequence.
    pub fn captured_sequence(&self, artifact: &str) -> StoreResult<Option<i64>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=?1",
                [artifact],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn captured_artifact(&self, captured: i64) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT receipt_key FROM session_events WHERE seq=?1 AND kind='captured'",
                [captured],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn session_for_artifact(&self, artifact_key: &str) -> StoreResult<Option<AgentSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            &format!("{SESSION_SELECT} WHERE s.id=(SELECT session_id FROM session_events WHERE kind='captured' AND receipt_key=?1)"),
            [artifact_key],
            read_session,
        )
        .optional()?
        .transpose()
    }

    /// Admit the conversation and its captured input before provider effects.
    pub fn create_session(
        &self,
        session: AgentSession,
        caller_exec: Option<&crate::id::ExecId>,
    ) -> StoreResult<AgentSession> {
        let _admission = self.lock_checkout(&session.cwd)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = session_in(&tx, &session.id)? {
            return Ok(existing);
        }
        let session = reserve_session_in(&tx, session, caller_exec)?;
        tx.commit()?;
        Ok(session)
    }

    /// Find the scope's uncompleted primary, or admit `session` as it. When the
    /// current primary is `replacing`, complete it and admit its successor in
    /// the same transaction; a repeat naming a replaced predecessor finds the
    /// successor already admitted.
    pub fn ensure_primary_session(
        &self,
        scope: &PrimaryScope,
        replacing: Option<&str>,
        session: AgentSession,
        caller_exec: Option<&crate::id::ExecId>,
    ) -> StoreResult<AgentSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let marked = |kind: &str, column: &str, id: String| {
            tx.query_row(
                &format!(
                    "{SESSION_SELECT} WHERE s.primary_scope=?1 AND s.{column}=?2 \
                     AND s.completed_at IS NULL"
                ),
                params![kind, id],
                read_session,
            )
            .optional()?
            .transpose()
        };
        let current = match scope {
            PrimaryScope::Repository(repo) => marked("repository", "repo", repo.to_string())?,
            PrimaryScope::Wave(wave) => marked("wave", "wave_id", wave.to_string())?,
            PrimaryScope::Task(task) => task_primary_in(&tx, task)?,
        };
        match current {
            Some(current) if Some(current.id.as_str()) == replacing => {
                tx.execute(
                    "UPDATE agent_sessions SET completed_at=?2 WHERE id=?1",
                    params![current.id, crate::store::rows::now_unix()],
                )?;
            }
            Some(current) => return Ok(current),
            None => {}
        }
        let session = reserve_session_in(&tx, session, caller_exec)?;
        // A Task's primary stays an ordinary member conversation; its Task
        // names it. A repository's or Wave's is marked on its own row.
        match scope {
            PrimaryScope::Repository(_) => tx.execute(
                "UPDATE agent_sessions SET primary_scope='repository' WHERE id=?1",
                [&session.id],
            )?,
            PrimaryScope::Wave(_) => tx.execute(
                "UPDATE agent_sessions SET primary_scope='wave' WHERE id=?1",
                [&session.id],
            )?,
            PrimaryScope::Task(task) => tx.execute(
                "UPDATE tasks SET primary_session_id=?2 WHERE id=?1",
                params![task.as_str(), session.id],
            )?,
        };
        tx.commit()?;
        Ok(session)
    }

    /// Called under the Session launch lock after proving there is no live client.
    pub(crate) fn move_primary_workspace(
        &self,
        session: &AgentSession,
        cwd: &std::path::Path,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE agent_sessions SET cwd=?3 WHERE id=?1 AND current_capture IS ?2 AND primary_scope IS NOT NULL AND completed_at IS NULL AND task_id IS NULL",
            params![session.id, session.captured, cwd.to_string_lossy()],
        )?;
        if updated != 1 {
            return Err(StoreError::InvalidAuthority(
                "primary Session changed before workspace admission".into(),
            ));
        }
        Ok(())
    }

    /// The Task a Session is primary for, else the repository or Wave it is or
    /// was primary for.
    pub fn primary_scope(&self, id: &str) -> StoreResult<Option<PrimaryScope>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let task: Option<String> = conn
            .query_row(
                "SELECT id FROM tasks WHERE primary_session_id=?1",
                [id],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(task) = task {
            return Ok(Some(PrimaryScope::Task(
                TaskId::parse(&task).map_err(invalid)?,
            )));
        }
        let row: Option<(Option<String>, Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT primary_scope,wave_id,repo FROM agent_sessions WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        match row {
            Some((Some(kind), Some(wave), _)) if kind == "wave" => Ok(Some(PrimaryScope::Wave(
                crate::id::WaveId::parse(&wave).map_err(invalid)?,
            ))),
            Some((Some(kind), _, Some(repo))) if kind == "repository" => {
                Ok(Some(PrimaryScope::Repository(
                    crate::repository::CanonicalRepo::discover(std::path::Path::new(&repo))
                        .map_err(invalid)?,
                )))
            }
            Some((Some(kind), _, _)) => Err(invalid(format!(
                "Session {id} has unsupported primary scope {kind:?}"
            ))),
            _ => Ok(None),
        }
    }

    pub(crate) fn publish_capture(&self, id: &str, captured: Option<i64>) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute(
            "UPDATE agent_sessions SET input_published=1 WHERE id=?1 AND current_capture=?2
             AND input_published=0 AND completed_at IS NULL",
            params![id, captured],
        )? != 1
        {
            return Err(StoreError::InvalidAuthority(
                "conversation capture changed before publication".into(),
            ));
        }
        Ok(())
    }

    /// Replace captured input under the existing conversation. Native history,
    /// title, feedback and assignment remain on their owners.
    pub fn replace_session_input(
        &self,
        expected_input: Option<i64>,
        mut session: AgentSession,
    ) -> StoreResult<AgentSession> {
        let _admission = self.lock_checkout(&session.cwd)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous = session_in(&tx, &session.id)?.ok_or(StoreError::NotFound)?;
        if previous.captured != expected_input
            || previous.completed_at.is_some()
            || previous.task_id != session.task_id
            || previous.wave_id != session.wave_id
        {
            return Err(StoreError::InvalidAuthority(
                "conversation changed before input replacement".into(),
            ));
        }
        replace_input_in(
            &tx,
            &mut session,
            crate::journal::current_exec_id().as_ref(),
        )?;
        let session = session_in(&tx, &session.id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(session)
    }

    /// Choose the agent of an unpublished capture. A published capture keeps
    /// the provider it launched with.
    pub fn retarget_unpublished_capture(
        &self,
        artifact_key: &str,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE agent_sessions SET provider=?2, model=?3 WHERE current_capture=?1 AND input_published=0",
            params![capture_seq_in(&conn,artifact_key)?, provider, model],
        )?;
        Ok(())
    }

    /// Assign future work without changing earlier attribution. Closed Sessions bind too.
    pub fn bind_session(
        &self,
        id: &str,
        expected_capture: Option<i64>,
        task: &TaskId,
    ) -> StoreResult<AgentSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session = session_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        if session.captured != expected_capture {
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
            tx.execute(
                "UPDATE agent_sessions SET task_id=?2,wave_id=?3,work_source='bound',bound_at=?4 WHERE id=?1",
                params![id, task.as_str(), wave.as_str(), crate::store::rows::now_unix()],
            )?;
        }
        let bound = session_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(bound)
    }

    /// Sessions assigned to a Task after they began, oldest bind first.
    pub(crate) fn bound_sessions(&self) -> StoreResult<Vec<crate::session::SessionBind>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT s.id,s.bound_at,t.issue_identifier,(SELECT slug FROM wave_addresses WHERE id=s.wave_id)
             FROM agent_sessions s JOIN tasks t ON t.id=s.task_id
             WHERE s.bound_at IS NOT NULL ORDER BY s.bound_at,s.id",
        )?;
        let rows = query.query_map([], |row| {
            Ok(crate::session::SessionBind {
                session_id: row.get(0)?,
                at: row.get(1)?,
                task: row.get(2)?,
                wave: row.get(3)?,
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// Every open Session.
    pub fn sessions(
        &self,
        filter: &crate::session::SessionFilter,
    ) -> StoreResult<Vec<AgentSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let (sql, values) = inventory_query(filter, SESSION_SELECT, now)?;
        let mut query = conn.prepare(&sql)?;
        let rows = query.query_map(rusqlite::params_from_iter(values), read_session)?;
        rows.map(|row| row?).collect()
    }

    pub fn session_inputs(&self, id: &str) -> StoreResult<Vec<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT receipt_key FROM session_events WHERE kind='captured' AND session_id=?1 ORDER BY seq",
        )?;
        let rows = query.query_map([id], |row| row.get::<_, String>(0))?;
        rows.map(|row| crate::session_record::parse_artifact_key(&row?).map_err(invalid))
            .collect()
    }

    pub fn rename_session(&self, id: &str, title: &str, source: TitleSource) -> StoreResult<()> {
        if title.trim().is_empty() {
            return Err(invalid("Session title cannot be empty"));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_current_actor_in(&tx, id)?;
        session_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        tx.execute(
            "UPDATE agent_sessions SET title=?2, title_source=?3
             WHERE id=?1 AND (title_source='generated' OR ?3='human')",
            params![id, title.trim(), title_source(source)],
        )?;
        tx.commit()?;
        Ok(())
    }
}

pub(super) fn retain_history_in(
    conn: &Connection,
    session: &AgentSession,
    history: &[crate::session::SessionObservation],
) -> StoreResult<bool> {
    let mut changed = false;
    for observation in history {
        let captured: Option<(i64,String)> = conn.query_row(
            "SELECT seq,session_id FROM session_events WHERE kind='captured' AND receipt_key=?1",
            [&observation.artifact_key], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
        let (captured, owner) = captured.ok_or(StoreError::NotFound)?;
        if owner != session.id {
            return Err(invalid("artifact belongs to another conversation"));
        }
        let key = format!("{}:{}", observation.artifact_key, observation.source);
        let payload = serde_json::to_string(&observation.payload)?;
        let saved: Option<String> = conn.query_row(
            "SELECT payload FROM session_events WHERE session_id=?1 AND kind='observed' AND receipt_key=?2",
            params![session.id,key], |row| row.get(0)).optional()?;
        if let Some(saved) = saved {
            if saved != payload {
                return Err(invalid(format!(
                    "input {} has conflicting {} evidence",
                    observation.artifact_key, observation.source
                )));
            }
            continue;
        }
        conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,task_id,wave_id,observed_at,payload,captured_event)
            VALUES(?1,'observed',?2,?3,?4,?5,?6,?7)",
            params![session.id,key,observation.task_id.as_ref().map(TaskId::as_str),
                observation.wave_id.as_ref().map(crate::id::WaveId::as_str),observation.observed_at,payload,captured])?;
        changed = true;
    }
    Ok(changed)
}

/// Resolve ancestry and captured location on the conversation's admission transaction.
pub(super) fn reserve_session_in(
    conn: &Transaction<'_>,
    mut session: AgentSession,
    caller: Option<&crate::id::ExecId>,
) -> StoreResult<AgentSession> {
    resolve_ancestry_in(conn, &mut session)?;
    insert_session_in(conn, &mut session, caller)?;
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

/// A replaced provider keeps causal history, but cannot mutate its conversation.
fn require_current_actor_in(conn: &Connection, id: &str) -> StoreResult<()> {
    if let Some(caller) = crate::journal::agent_caller().filter(|caller| caller.session_id == id) {
        let current: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE id=?1
                AND provider_generation=?2 AND provider_exec_id=?3
                AND driver_exec_id IS NOT NULL)",
            params![id, caller.provider_generation, caller.origin_exec_id],
            |row| row.get(0),
        )?;
        if !current {
            return Err(StoreError::InvalidAuthority(
                "Session provider was replaced".into(),
            ));
        }
    }
    if let Some(key) = std::env::var_os(crate::session_record::CAPTURE_KEY_ENV) {
        let key = key
            .into_string()
            .map_err(|_| invalid("capture key is not valid UTF-8"))?;
        crate::session_record::parse_artifact_key(&key).map_err(invalid)?;
        let (owner, current): (String, bool) = conn
            .query_row(
                "SELECT s.id,e.seq IS s.current_capture FROM session_events e
                JOIN agent_sessions s ON s.id=e.session_id
                WHERE e.kind='captured' AND e.receipt_key=?1",
                [&key],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| invalid("capture does not belong to a recorded Session in this Home"))?;
        if crate::journal::agent_caller().is_some_and(|caller| caller.session_id != owner) {
            return Err(invalid("capture belongs to another Session"));
        }
        let stale = owner == id && !current;
        if stale {
            return Err(StoreError::InvalidAuthority(
                "Session input was replaced".into(),
            ));
        }
    }
    Ok(())
}

/// Append capture evidence before publication; an existing artifact key must
/// retain its original conversation and caller.
pub(super) fn capture_seq_in(conn: &Connection, artifact: &str) -> StoreResult<i64> {
    conn.query_row(
        "SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=?1",
        [artifact],
        |row| row.get(0),
    )
    .optional()?
    .ok_or(StoreError::NotFound)
}

fn capture_in(
    conn: &Connection,
    session: &AgentSession,
    observed_at: i64,
    exec: Option<&crate::id::ExecId>,
) -> StoreResult<i64> {
    let saved: Option<(i64, String, Option<String>)> = conn
        .query_row(
            "SELECT seq,session_id,json_extract(payload,'$.caller_key') FROM session_events
         WHERE kind='captured' AND receipt_key=?1",
            [&session.artifact_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if let Some((seq, owner, caller)) = saved {
        if owner != session.id || caller != session.caller_artifact_key {
            return Err(invalid(
                "captured artifact has conflicting Session or caller evidence",
            ));
        }
        return Ok(seq);
    }
    let payload = serde_json::json!({"artifact_key":session.artifact_key,
        "caller_key":session.caller_artifact_key,"cwd":session.cwd,"skill":session.skill,
        "provider":session.provider,"model":session.model,"work_source":session.work_source});
    conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,exec_id,task_id,wave_id,observed_at,payload)
        VALUES(?1,'captured',?2,?3,?4,?5,?6,?7)",
        params![session.id,session.artifact_key,exec,session.task_id.as_ref().map(TaskId::as_str),
            session.wave_id.as_ref().map(crate::id::WaveId::as_str),observed_at,serde_json::to_string(&payload)?])?;
    Ok(conn.last_insert_rowid())
}

fn insert_session_in(
    conn: &Connection,
    session: &mut AgentSession,
    exec: Option<&crate::id::ExecId>,
) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO agent_sessions(id,title,title_source,completed_at,
        created_at,request,interactive,repo,task_id,wave_id,work_source,bound_at,
        input_published,cwd,skill,provider,model)
        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17)",
        params![
            session.id,
            session.title,
            title_source(session.title_source),
            session.completed_at,
            session.created_at,
            session.request,
            session.interactive,
            session.repo,
            session.task_id.as_ref().map(TaskId::as_str),
            session.wave_id.as_ref().map(crate::id::WaveId::as_str),
            session
                .work_source
                .map(serde_json::to_value)
                .transpose()?
                .as_ref()
                .and_then(serde_json::Value::as_str),
            session.bound_at,
            session.input_published,
            session.cwd.to_string_lossy(),
            session.skill,
            session.provider,
            session.model
        ],
    )?;
    session.captured = Some(capture_in(conn, session, session.created_at, exec)?);
    conn.execute(
        "UPDATE agent_sessions SET current_capture=?2 WHERE id=?1",
        params![session.id, session.captured],
    )?;
    if let Some(summary) = &session.ready_summary {
        conn.execute(
            "INSERT INTO session_events(session_id,kind,receipt_key,task_id,wave_id,observed_at,payload,captured_event)
             VALUES(?1,'observed','legacy_review_feedback',?2,?3,?4,?5,?6)",
            params![session.id,session.task_id.as_ref().map(TaskId::as_str),session.wave_id.as_ref().map(crate::id::WaveId::as_str),crate::store::rows::now_unix(),
                serde_json::to_string(&serde_json::json!({"type":"legacy_review_feedback","summary":summary}))?,session.captured],
        )?;
    }
    Ok(())
}

pub(super) fn replace_input_in(
    conn: &Transaction<'_>,
    session: &mut AgentSession,
    exec: Option<&crate::id::ExecId>,
) -> StoreResult<()> {
    if conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM session_events WHERE kind='captured' AND receipt_key=?1)",
        [&session.artifact_key],
        |row| row.get::<_, bool>(0),
    )? {
        return Err(StoreError::InvalidAuthority(
            "replacement must append a new captured event".into(),
        ));
    }
    session.captured = Some(capture_in(
        conn,
        session,
        crate::store::rows::now_unix(),
        exec,
    )?);
    conn.execute("UPDATE agent_sessions SET current_capture=?2,input_published=?3,cwd=?4,skill=?5,provider=?6,model=?7
        WHERE id=?1 AND completed_at IS NULL",
        params![session.id,session.captured,session.input_published,session.cwd.to_string_lossy(),
            session.skill,session.provider,session.model])?;
    Ok(())
}

#[cfg(test)]
pub(crate) fn test_capture(conn: &Connection, session: &str, artifact: &str) {
    conn.execute(
        "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
        VALUES(?1,'captured',?2,1,json_object('artifact_key',?2))",
        params![session, artifact],
    )
    .unwrap();
    conn.execute(
        "UPDATE agent_sessions SET current_capture=?2 WHERE id=?1",
        params![session, conn.last_insert_rowid()],
    )
    .unwrap();
}

#[cfg(test)]
impl SqliteStore {
    pub(crate) fn test_session(&self, id: &str, artifact: &str) -> AgentSession {
        self.create_session(
            AgentSession {
                captured: None,
                id: id.into(),
                artifact_key: artifact.into(),
                caller_artifact_key: None,
                input_published: true,
                cwd: "/fixture".into(),
                skill: None,
                provider: None,
                model: None,
                node: None,
                iterations: None,
                task_id: None,
                wave_id: None,
                flow_id: None,
                work_source: None,
                bound_at: None,
                interactive: true,
                repo: None,
                title: "Retained".into(),
                title_source: TitleSource::Human,
                request: None,
                ready_summary: None,
                completed_at: None,
                created_at: 1,
            },
            None,
        )
        .unwrap()
    }
}

#[cfg(test)]
mod metadata_tests {
    use rusqlite::params;
    use serde_json::json;

    use super::SqliteStore;

    use crate::session::{FlowSummaryState, SessionFilter};

    #[test]
    fn resume_candidates_keep_completed_conversations_and_original_opening_times() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let session =
            store.test_session("conversation", &crate::session_record::new_artifact_key());
        let background =
            store.test_session("background", &crate::session_record::new_artifact_key());
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "UPDATE agent_sessions SET completed_at=10 WHERE id=?1",
                [&session.id],
            )
            .unwrap();
            conn.execute(
                "UPDATE agent_sessions SET interactive=0 WHERE id=?1",
                [&background.id],
            )
            .unwrap();
        }
        for (source, opened) in [("first", 30_000), ("recovered-earlier", 20_000)] {
            store
                .retain_session_observation(
                    &session,
                    &crate::session::SessionObservation {
                        artifact_key: session.artifact_key.clone(),
                        source: source.into(),
                        observed_at: 999_999,
                        task_id: None,
                        wave_id: None,
                        payload: json!({"type":"interactive_opened", "opened_at_ms":opened}),
                    },
                )
                .unwrap();
        }
        let candidates = store.resume_candidates().unwrap();
        assert_eq!(candidates.len(), 1);
        assert_eq!(candidates[0].0.id, session.id);
        assert_eq!(candidates[0].1, Some(30_000));
    }

    #[test]
    fn retained_sessions_without_repo_use_their_recorded_wave() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        store.test_session("retained", &crate::session_record::new_artifact_key());
        let wave = crate::id::WaveId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'Product','/repo',1)",
                [wave.as_str()],
            )
            .unwrap();
            conn.execute(
                "UPDATE agent_sessions SET wave_id=?1,repo=NULL WHERE id='retained'",
                [wave.as_str()],
            )
            .unwrap();
        }
        let filter = SessionFilter {
            repo: Some("/repo".into()),
            ..SessionFilter::default()
        };
        assert_eq!(
            store.session_summaries(&filter, 0).unwrap()[0].id,
            "retained"
        );
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET repo='/other' WHERE id='retained'",
                [],
            )
            .unwrap();
        assert!(store.session_summaries(&filter, 0).unwrap().is_empty());
    }

    #[test]
    fn captured_reservation_survives_interruption_and_fences_replacement() {
        let home = tempfile::tempdir().unwrap();
        let path = home.path().join("store.db");
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let old_key = "run_00000000000000000000000000000001";
        let first = store.test_session("conversation", old_key);
        let history = store.session_history("conversation", 0, 100).unwrap();
        assert_eq!(
            history.first().unwrap().kind,
            crate::session::SessionEventKind::Captured
        );
        assert_eq!(history.first().unwrap().seq, first.captured.unwrap());
        let mut next = first.clone();
        next.artifact_key = crate::session_record::new_artifact_key();
        next.input_published = false;
        let next = store.replace_session_input(first.captured, next).unwrap();
        assert_ne!(next.captured, first.captured);
        drop(store);
        let store = SqliteStore::open_ephemeral(&path).unwrap();
        let retained = store.session("conversation").unwrap().unwrap();
        assert_eq!(retained, next);
        assert!(!retained.input_published);
        assert_eq!(store.captured_sequence(old_key).unwrap(), first.captured);
        assert_eq!(
            store.session_for_artifact(old_key).unwrap().unwrap().id,
            first.id
        );
        assert!(store.publish_capture(&first.id, first.captured).is_err());
        assert!(store
            .replace_session_input(first.captured, first.clone())
            .is_err());
        store
            .publish_capture(&retained.id, retained.captured)
            .unwrap();
        assert!(store
            .publish_capture(&retained.id, retained.captured)
            .is_err());
        assert_eq!(
            store.session_history("conversation", 0, 100).unwrap().len(),
            2
        );
        assert!(
            store
                .session("conversation")
                .unwrap()
                .unwrap()
                .input_published
        );
    }

    #[test]
    fn session_metadata_reads_its_flow_from_the_exec_that_captured_its_input() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let input = crate::session_record::new_artifact_key();
        let driver = store.test_flow("retained", "/unavailable", &[("implement", None)], None);
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO agent_sessions(id,title,title_source,created_at,
                interactive,input_published,cwd,request)
                VALUES('session','Session','human',1,1,1,'/unavailable',?1)",
                params!["large request".repeat(1000)],
            )
            .unwrap();
            // The step Exec captured this conversation's input.
            conn.execute(
                "INSERT INTO session_events(session_id,kind,receipt_key,exec_id,observed_at,payload)
                 VALUES('session','captured',?1,(SELECT id FROM execs WHERE parent_exec_id=?2),1,
                    json_object('artifact_key',?1))",
                params![input, driver],
            )
            .unwrap();
            conn.execute(
                "UPDATE agent_sessions SET current_capture=?1 WHERE id='session'",
                [conn.last_insert_rowid()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
                VALUES('session','observed',?1,1,'not JSON',(SELECT current_capture FROM agent_sessions WHERE id='session'))",
                [format!("{input}:events.jsonl:0")],
            )
            .unwrap();
        }
        // Listing reads the Flow from Execs and survives unreadable detail.
        let rows = store
            .session_summaries(&SessionFilter::default(), 0)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].flow_id.as_deref(), Some(driver.as_str()));
        let flow = rows[0].flow.as_ref().unwrap();
        assert_eq!(
            (flow.name.as_str(), flow.state),
            ("retained", FlowSummaryState::Current)
        );
        assert!(rows[0].flow_step_latest);
        assert_eq!(
            store
                .session("session")
                .unwrap()
                .unwrap()
                .flow_id
                .as_deref(),
            Some(driver.as_str())
        );
        assert!(
            store.input_events(&input).is_err(),
            "Exact history still reports corrupt payload"
        );
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE execs SET outcome='succeeded',completed_at=2 WHERE id=?1",
                params![driver],
            )
            .unwrap();
        let flow = store
            .session_summary("session", 0)
            .unwrap()
            .unwrap()
            .flow
            .unwrap();
        assert_eq!(flow.state, FlowSummaryState::Completed);
    }

    #[test]
    fn session_metadata_receipts_remain_idempotent_and_conflicts_do_not_relabel() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let input = crate::session_record::new_artifact_key();
        store.test_session("imported", &input);
        let session = store.session("imported").unwrap().unwrap();
        let mut observation = crate::session::SessionObservation {
            artifact_key: input.clone(),
            source: "manifest.json".into(),
            observed_at: 1,
            task_id: None,
            wave_id: None,
            payload: json!({"input_id":input.as_str(),"source":"manifest.json","evidence":{
                "artifact_key":input.as_str(),"schema_version":1,"flow":{"kind":"independent"}}}),
        };
        assert!(
            !store
                .session_summary("imported", 0)
                .unwrap()
                .unwrap()
                .independent
        );
        for _ in 0..2 {
            store
                .retain_session_observation(&session, &observation)
                .unwrap();
        }
        assert!(
            store
                .session_summary("imported", 0)
                .unwrap()
                .unwrap()
                .independent
        );
        assert_eq!(store.session_history("imported", 0, 0).unwrap().len(), 2);
        observation.payload["evidence"]["flow"] = serde_json::Value::Null;
        assert!(store
            .retain_session_observation(&session, &observation)
            .is_err());
        assert!(
            store
                .session_summary("imported", 0)
                .unwrap()
                .unwrap()
                .independent
        );
    }

    #[test]
    fn session_metadata_filters_and_preserves_explicit_membership() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        {
            let conn = store.conn.lock().unwrap();
            for (id, title, interactive, repo, membership) in [
                ("a", "Alpha%", true, "/repo", Some("independent")),
                ("b", "Beta", true, "/repo", None),
                ("headless", "Headless", false, "/repo", None),
                ("foreign", "Foreign", true, "/other", None),
                ("later", "Zulu", true, "/repo", None),
            ] {
                let input = crate::session_record::new_artifact_key();
                conn.execute(
                    "INSERT INTO agent_sessions(id,title,title_source,created_at,
                    interactive,input_published,cwd,repo)
                    VALUES(?1,?2,'human',1,?3,1,'/unavailable',?4)",
                    params![id, title, interactive, repo],
                )
                .unwrap();
                super::test_capture(&conn, id, &input);
                if let Some(kind) = membership {
                    let payload = json!({"input_id":input.as_str(),"source":"manifest.json", "evidence":{
                        "artifact_key":input.as_str(),"schema_version":1,"flow":{"kind":kind}}});
                    conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
                        VALUES(?1,'observed',?2,1,?3,(SELECT current_capture FROM agent_sessions WHERE id=?1))",
                        params![id,format!("{input}:manifest.json"),payload.to_string()]).unwrap();
                }
            }
        }
        let mut filter = SessionFilter {
            repo: Some("/repo".into()),
            limit: 2,
            ..SessionFilter::default()
        };
        let rows = store.session_summaries(&filter, 0).unwrap();
        assert_eq!(
            rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert!(rows[0].independent);
        assert!(!rows[1].independent);
        filter.search = Some("%".into());
        assert_eq!(
            store.session_summaries(&filter, 0).unwrap().len(),
            1,
            "Contains search is literal"
        );
        filter.search = Some("ALPHA".into());
        assert_eq!(store.session_summaries(&filter, 0).unwrap()[0].id, "a");
        filter.search = None;
        filter.interactive = Some(false);
        assert_eq!(
            store.session_summaries(&filter, 0).unwrap()[0].id,
            "headless"
        );
        filter.interactive = Some(true);
        filter.offset = 1;
        filter.limit = 1;
        assert_eq!(store.session_summaries(&filter, 0).unwrap()[0].id, "b");
        filter.after = Some(String::new());
        let first = store.session_summaries(&filter, 0).unwrap();
        assert_eq!(first[0].id, "a");
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET title='000 renamed' WHERE id='b'",
                [],
            )
            .unwrap();
        filter.after = Some(first[0].id.clone());
        let second = store.session_summaries(&filter, 0).unwrap();
        assert_eq!(second[0].id, "b");
        assert_eq!(second[0].title, "000 renamed");
        filter.after = Some(String::new());
        filter.limit = 2;
        assert_eq!(
            store
                .session_summaries(&filter, 0)
                .unwrap()
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"],
            "renaming must not reorder the ID page used to build its cursor"
        );
    }
}
