//! Session transactions share the invocation's SQLite transaction and fences.

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::durable::{FlowSession, TaskId};
use crate::engine::ExecutionCursor;
use crate::session::{AgentSession, PrimaryScope, SessionKind, TitleSource, WorkSource};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

const SESSION_SELECT: &str = "SELECT s.id,COALESCE(c.receipt_key,'') AS artifact_key,s.title,s.title_source,
    s.ready_summary,s.completed_at,s.created_at,s.kind,s.request,s.interactive,s.repo,
    s.task_id,s.wave_id,s.flow_session_id,s.work_source,s.bound_at,
    s.input_published,s.cwd,s.skill,s.provider,s.model,s.node,s.iterations,json_extract(c.payload,'$.caller_key'),s.current_capture FROM agent_sessions s LEFT JOIN session_events c ON c.seq=s.current_capture";

fn read_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoreResult<AgentSession>> {
    Ok((|| {
        Ok(AgentSession {
            captured: row.get(24)?,
            caller_artifact_key: row
                .get::<_, Option<String>>(23)?
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
    select: &str,
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
        sql.push_str(
            " AND s.completed_at IS NULL AND (s.kind!='flow_review' OR EXISTS(
            SELECT 1 FROM flow_sessions f WHERE f.pending_session_id=s.id AND f.state='current' AND NOT EXISTS(SELECT 1 FROM tasks t WHERE t.id=s.task_id AND t.work_state IN ('done','abandoned'))))",
        );
    }
    if let Some(repo) = &filter.repo {
        sql.push_str(&format!(" AND s.repo={}", bind(Value::Text(repo.clone()))));
    }
    if let Some(task) = &filter.task {
        let task = bind(Value::Text(task.clone()));
        sql.push_str(&format!(
            " AND s.id IN ({})",
            super::task_work::session_ids(&task)
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
const SUMMARY_SELECT: &str = "SELECT s.id,c.receipt_key AS artifact_key,s.title,s.title_source,
    s.ready_summary,s.completed_at,s.kind,s.interactive,s.task_id,s.wave_id,
    s.flow_session_id,s.cwd,s.skill,s.provider,s.model,s.node,s.iterations,s.current_capture
    FROM agent_sessions s LEFT JOIN session_events c ON c.seq=s.current_capture";

const MEMBERSHIP_KIND: &str = "CASE WHEN json_valid(payload) THEN CASE WHEN json_extract(payload,'$.source')='manifest.json' AND json_extract(payload,'$.evidence.schema_version')=1 AND json_extract(payload,'$.evidence.artifact_key')=json_extract(payload,'$.input_id') AND receipt_key=json_extract(payload,'$.input_id')||':manifest.json' THEN json_extract(payload,'$.evidence.flow.kind') END END";

fn summary_query(page: &str, by_id: bool) -> String {
    let order = if by_id { "s.id" } else { "s.title,s.id" };
    format!("WITH page AS MATERIALIZED ({page}),
        flows AS MATERIALIZED (SELECT {} FROM flow_sessions f INDEXED BY flow_metadata
            WHERE f.id IN (SELECT flow_session_id FROM page))
        SELECT s.*,f.id,f.name,f.state,f.current_capture,f.pending_session_id,f.task_id,f.wave_id,f.updated_at,
        w.slug,t.issue_identifier,
        COALESCE(t.current_invocation_id=s.flow_session_id,0),h.id,h.route,
        ((SELECT {MEMBERSHIP_KIND} FROM session_events INDEXED BY session_input_membership
         WHERE session_id=s.id AND captured_event=s.current_capture
         AND kind='observed' AND substr(receipt_key,-14)=':manifest.json')='independent'),
        (SELECT json_group_array(id) FROM ({})),
        a.primary_scope,
        (SELECT CASE WHEN json_valid(e.payload) THEN json_extract(e.payload,'$.outcome') END FROM session_events e INDEXED BY session_driver_exit
            WHERE e.session_id=s.id AND e.receipt_key='driver:'||(a.driver_generation-1)||':exit' AND e.kind='observed'),
        (SELECT COALESCE(CASE WHEN json_valid(done.payload) THEN json_extract(done.payload,'$.status') END,'running') FROM session_events start
            LEFT JOIN session_events done INDEXED BY session_turn_attention ON done.session_id=start.session_id
                AND done.provider_thread=start.provider_thread AND done.provider_turn=start.provider_turn AND done.kind='completed'
            WHERE start.session_id=s.id AND start.captured_event=s.current_capture AND start.kind='started'
            ORDER BY start.seq DESC LIMIT 1),
        COALESCE(t.work_state IN ('done','abandoned'),0)
        FROM page s JOIN agent_sessions a ON a.id=s.id
        LEFT JOIN flows f ON f.id=s.flow_session_id
        LEFT JOIN wave_addresses w ON w.id=s.wave_id
        LEFT JOIN tasks t ON t.id=s.task_id
        LEFT JOIN work_placements p ON p.task_id=t.id AND COALESCE(t.current_invocation_id=s.flow_session_id,0)
        LEFT JOIN homes h ON h.id=p.home_id
        ORDER BY {order}", super::flows::FLOW_METADATA_COLUMNS, super::task_work::session_tasks("s"))
}

fn read_summary(
    row: &rusqlite::Row<'_>,
) -> rusqlite::Result<StoreResult<crate::session::SessionSummary>> {
    Ok((|| {
        Ok(crate::session::SessionSummary {
            task_ids: serde_json::from_str(&row.get::<_, String>(32)?)?,
            primary_scope: row.get(33)?,
            driver_outcome: row.get(34)?,
            latest_turn: row.get(35)?,
            task_terminal: row.get(36)?,
            captured: row.get(17)?,
            id: row.get(0)?,
            artifact_key: crate::session_record::parse_artifact_key(&row.get::<_, String>(1)?)
                .map_err(invalid)?,
            title: row.get(2)?,
            title_source: serde_json::from_value(serde_json::Value::String(row.get(3)?))?,
            ready_summary: row.get(4)?,
            completed_at: row.get(5)?,
            kind: serde_json::from_value(serde_json::Value::String(row.get(6)?))?,
            interactive: row.get(7)?,
            task_id: row
                .get::<_, Option<String>>(8)?
                .map(|id| TaskId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            wave_id: row
                .get::<_, Option<String>>(9)?
                .map(|id| crate::id::WaveId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            flow_session_id: row.get(10)?,
            cwd: row.get::<_, String>(11)?.into(),
            skill: row.get(12)?,
            provider: row.get(13)?,
            model: row.get(14)?,
            node: row.get(15)?,
            iterations: row
                .get::<_, Option<String>>(16)?
                .map(|raw| serde_json::from_str(&raw))
                .transpose()?,
            flow: super::flows::read_flow_summary(row, 18)?,
            wave_name: row.get(26)?,
            task_identifier: row.get(27)?,
            managed: row.get(28)?,
            home_id: row
                .get::<_, Option<String>>(29)?
                .map(|id| crate::durable::HomeId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            home_route: row.get(30)?,
            independent: row.get::<_, Option<bool>>(31)?.unwrap_or(false),
        })
    })())
}

impl SqliteStore {
    pub(crate) fn session_summaries(
        &self,
        filter: &crate::session::SessionFilter,
    ) -> StoreResult<Vec<crate::session::SessionSummary>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let (page, values) = inventory_query(filter, SUMMARY_SELECT)?;
        let mut query = conn.prepare(&summary_query(&page, filter.after.is_some()))?;
        let rows = query.query_map(rusqlite::params_from_iter(values), read_summary)?;
        rows.map(|row| row?).collect()
    }

    pub(crate) fn session_summary(
        &self,
        id: &str,
    ) -> StoreResult<Option<crate::session::SessionSummary>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            &summary_query(&format!("{SUMMARY_SELECT} WHERE s.id=?1"), false),
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

    pub fn reserve_review_run(
        &self,
        expected: &FlowSession,
    ) -> StoreResult<(FlowSession, AgentSession)> {
        let _admission = self.lock_checkout(&expected.cwd)?;
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
            session.artifact_key = crate::session_record::new_artifact_key();
            session.input_published = false;
            replace_input_in(
                &tx,
                &mut session,
                crate::journal::current_exec_id().as_ref(),
            )?;
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

    pub(crate) fn publish_review_capture(
        &self,
        session_id: &str,
        captured: i64,
        version: u64,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute("UPDATE flow_sessions SET position_version=position_version+1
            WHERE state='current' AND pending_session_id=?1 AND position_version=?3
            AND claim_json IS NULL AND EXISTS(SELECT 1 FROM agent_sessions s WHERE s.id=?1 AND s.current_capture=?2 AND s.input_published=0 AND s.completed_at IS NULL)",
            params![session_id, captured, i64::try_from(version).map_err(invalid)?])? != 1 {
            return Err(StoreError::InvalidAuthority("review Run reservation is stale".into()));
        }
        tx.execute(
            "UPDATE agent_sessions SET input_published=1, provider=?2, model=?3 WHERE current_capture=?1",
            params![captured, provider, model],
        )?;
        tx.commit()?;
        Ok(())
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

    pub fn session_for_artifact(&self, run_id: &str) -> StoreResult<Option<AgentSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            &format!("{SESSION_SELECT} WHERE s.id=(SELECT session_id FROM session_events WHERE kind='captured' AND receipt_key=?1)"),
            [run_id],
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
        let _admission = self.lock_checkout(&session.cwd)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = session_in(&tx, &session.id)? {
            return Ok(existing);
        }
        if let Some(flow) = review {
            super::flows::insert_flow_in(&tx, flow, None)?;
        }
        let session = reserve_session_in(&tx, session, caller_exec)?;
        if session.kind == SessionKind::FlowReview {
            tx.execute(
                "UPDATE flow_sessions SET pending_session_id=?2,current_capture=?3 WHERE id=?1",
                params![session.flow_session_id, session.id, session.captured],
            )?;
        }
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
        let (kind, column, id) = match scope {
            PrimaryScope::Repository(repo) => ("repository", "repo", repo.to_string()),
            PrimaryScope::Wave(wave) => ("wave", "wave_id", wave.to_string()),
        };
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = tx
            .query_row(
                &format!(
                    "{SESSION_SELECT} WHERE s.primary_scope=?1 AND s.{column}=?2 \
                     AND s.completed_at IS NULL"
                ),
                params![kind, id],
                read_session,
            )
            .optional()?
            .transpose()?;
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
        tx.execute(
            "UPDATE agent_sessions SET primary_scope=?2 WHERE id=?1",
            params![session.id, kind],
        )?;
        tx.commit()?;
        Ok(session)
    }

    /// The scope a Session is or was primary for.
    pub fn primary_scope(&self, id: &str) -> StoreResult<Option<PrimaryScope>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
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
             AND input_published=0 AND completed_at IS NULL AND (flow_session_id IS NULL OR kind='flow_review')",
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
            || previous.flow_session_id != session.flow_session_id
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
        tx.execute("UPDATE flow_sessions SET current_capture=?2 WHERE current_capture=?1 AND state='current'",
            params![previous.captured,session.captured])?;
        let session = session_in(&tx, &session.id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(session)
    }

    /// A review Run stored before providers were: take it from launch evidence.
    pub fn fill_run_provider(
        &self,
        run: &str,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE agent_sessions SET provider=?2, model=?3 WHERE current_capture=?1 AND provider IS NULL",
            params![capture_seq_in(&conn,run)?, provider, model],
        )?;
        Ok(())
    }

    /// Choose the agent of a Run that has not launched. A published Run keeps
    /// the provider it launched with.
    pub fn retarget_unpublished_run(
        &self,
        run: &str,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE agent_sessions SET provider=?2, model=?3 WHERE current_capture=?1 AND input_published=0",
            params![capture_seq_in(&conn,run)?, provider, model],
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
        let (sql, values) = inventory_query(filter, SESSION_SELECT)?;
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
    /// A review closes only with the feedback its caller waits for.
    /// A Task review closes inside its invocation's transaction instead.
    pub fn complete_session(&self, id: &str, expected_capture: Option<i64>) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.execute(
            "UPDATE agent_sessions SET completed_at=?3 WHERE id=?1 AND current_capture=?2
             AND completed_at IS NULL AND (kind='conversation' OR ready_summary IS NOT NULL)
             AND NOT EXISTS(SELECT 1 FROM tasks m WHERE m.id=agent_sessions.task_id
                 AND m.current_invocation_id=agent_sessions.flow_session_id)",
            params![id, expected_capture, crate::store::rows::now_unix()],
        )? != 1
        {
            return Err(StoreError::InvalidAuthority(
                "Session changed before completion".into(),
            ));
        }
        Ok(())
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

    pub fn rename_session(
        &self,
        id: &str,
        expected_capture: Option<i64>,
        title: &str,
        source: TitleSource,
    ) -> StoreResult<()> {
        if title.trim().is_empty() {
            return Err(invalid("Session title cannot be empty"));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session = session_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        if expected_capture.is_some_and(|event| Some(event) != session.captured) {
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

    pub fn ready_session(
        &self,
        id: &str,
        expected_capture: Option<i64>,
        summary: &str,
    ) -> StoreResult<()> {
        if summary.trim().is_empty() {
            return Err(invalid("ready summary cannot be empty"));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if tx.execute(
            "UPDATE agent_sessions SET ready_summary=?3 WHERE id=?1 AND current_capture=?2
             AND completed_at IS NULL AND (EXISTS(SELECT 1 FROM flow_sessions f WHERE f.id=agent_sessions.flow_session_id
                 AND agent_sessions.input_published=1 AND f.pending_session_id=?1 AND f.state='current'
                 AND f.claim_json IS NULL))",
            params![id, expected_capture, summary.trim()],
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

pub(super) fn review_id(flow: &FlowSession) -> StoreResult<String> {
    let task = flow
        .task_id
        .as_ref()
        .ok_or_else(|| invalid("review belongs to no Task"))?;
    let step = flow
        .current_checked()
        .ok_or_else(|| invalid("review has no captured step"))?;
    let node = step
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
            reserve_flow_conversation_in(conn, flow, id, SessionKind::FlowReview, title, None)?
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
    exec: Option<&crate::id::ExecId>,
) -> StoreResult<AgentSession> {
    reserve_session_in(
        conn,
        AgentSession {
            captured: None,
            caller_artifact_key: None,
            id,
            artifact_key: crate::session_record::new_artifact_key(),
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
        exec,
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
        .review_artifact_key()
        .ok_or_else(|| StoreError::InvalidAuthority("review has no published input".into()))?;
    if conn.execute(
        "UPDATE agent_sessions SET completed_at=?3 WHERE id=?1 AND current_capture=?2
        AND completed_at IS NULL AND ready_summary IS ?4
        AND EXISTS(SELECT 1 FROM flow_sessions WHERE id=?5 AND current_capture=?2 AND state='current')",
        params![id, capture_seq_in(conn,run_id)?, crate::store::rows::now_unix(), summary, expected.id()],
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
        "provider":session.provider,"model":session.model,"flow_session_id":session.flow_session_id,
        "node":session.node,"iterations":session.iterations,"work_source":session.work_source});
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
    conn.execute("INSERT INTO agent_sessions(id,title,title_source,ready_summary,completed_at,
        created_at,kind,request,interactive,repo,task_id,wave_id,flow_session_id,work_source,bound_at,
        input_published,cwd,skill,provider,model,node,iterations)
        VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19,?20,?21,?22)",
        params![session.id,session.title,title_source(session.title_source),session.ready_summary,
            session.completed_at,session.created_at,serde_json::to_value(session.kind)?.as_str(),
            session.request,session.interactive,session.repo,session.task_id.as_ref().map(TaskId::as_str),
            session.wave_id.as_ref().map(crate::id::WaveId::as_str),session.flow_session_id,
            session.work_source.map(serde_json::to_value).transpose()?.as_ref().and_then(serde_json::Value::as_str),
            session.bound_at,session.input_published,session.cwd.to_string_lossy(),session.skill,
            session.provider,session.model,session.node,session.iterations.as_ref().map(serde_json::to_string).transpose()?])?;
    session.captured = Some(capture_in(conn, session, session.created_at, exec)?);
    conn.execute(
        "UPDATE agent_sessions SET current_capture=?2 WHERE id=?1",
        params![session.id, session.captured],
    )?;
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
    if conn.execute("UPDATE flow_sessions SET current_capture=?3 WHERE id=?1 AND position_version=?2 AND state='current'",
        params![flow.id(),i64::try_from(flow.version).map_err(invalid)?,session.captured])? != 1
    {
        return Err(StoreError::InvalidAuthority("Flow changed before conversation selection".into()));
    }
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
                flow_session_id: None,
                work_source: None,
                bound_at: None,
                kind: SessionKind::Conversation,
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
    fn session_metadata_dense_review_pages_use_indexed_membership() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let conn = store.conn.lock().unwrap();
        let indexes: Vec<String> = conn
            .prepare(
                "SELECT sql FROM sqlite_master WHERE name IN
            ('flow_metadata','flow_pending_review','session_input_membership') ORDER BY name",
            )
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        assert_eq!(indexes.len(), 3);
        conn.execute_batch(
            "DROP INDEX flow_metadata; DROP INDEX flow_pending_review;
            DROP INDEX session_input_membership;",
        )
        .unwrap();
        conn.execute_batch("BEGIN;
            WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<5000)
            INSERT INTO flow_sessions(id,invocation_json,cwd,step_index,iteration,
                position_version,worker_generation,updated_at,state,pending_session_id)
            SELECT printf('f%05d',i),json_object('id',printf('f%05d',i),'flow','review','steps',hex(zeroblob(1024))),
                '/repo',0,0,1,0,1,'current',NULL FROM n;
            WITH RECURSIVE n(i) AS (VALUES(1) UNION ALL SELECT i+1 FROM n WHERE i<20000)
            INSERT INTO agent_sessions(id,title,title_source,created_at,kind,
                interactive,input_published,cwd,repo,flow_session_id)
            SELECT printf('s%05d',i),printf('Review %05d',i),
                'human',1,'flow_review',1,1,'/repo','/repo',printf('f%05d',(i-1)%5000+1) FROM n;
            INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload)
            SELECT id,'captured',printf('run_%032x',cast(substr(id,2) AS integer)),1,'{}' FROM agent_sessions;
            UPDATE agent_sessions SET current_capture=(SELECT seq FROM session_events WHERE session_id=agent_sessions.id);
            UPDATE flow_sessions SET pending_session_id='s'||substr(id,2);
            COMMIT;").unwrap();
        for sql in indexes {
            conn.execute_batch(&sql).unwrap();
        }
        let mut cases = Vec::new();
        for (offset, search) in [(0, None), (4500, None), (5100, None), (0, Some("absent"))] {
            let filter = SessionFilter {
                offset,
                limit: 100,
                search: search.map(str::to_owned),
                ..SessionFilter::default()
            };
            let (page, values) = super::inventory_query(&filter, super::SUMMARY_SELECT).unwrap();
            let sql = super::summary_query(&page, filter.after.is_some());
            let plan: Vec<String> = conn
                .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))
                .unwrap()
                .query_map(rusqlite::params_from_iter(&values), |row| row.get(3))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            assert!(
                plan.iter()
                    .any(|line| line.contains("SEARCH f USING COVERING INDEX flow_pending_review")),
                "{plan:?}"
            );
            for index in ["session_driver_exit", "session_turn_attention"] {
                assert!(plan.iter().any(|line| line.contains(index)), "{plan:?}");
            }
            let started = std::time::Instant::now();
            let ids: Vec<String> = conn
                .prepare(&sql)
                .unwrap()
                .query_map(rusqlite::params_from_iter(&values), |row| row.get(0))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            assert_eq!(
                ids.len(),
                if search.is_some() || offset > 5000 {
                    0
                } else {
                    100
                }
            );
            println!("bundled SQLite {} offset={offset} search={search:?} indexed_ms={} rows={} plan={plan:?}",
                rusqlite::version(), started.elapsed().as_secs_f64()*1000.0, ids.len());
            cases.push((sql, values, ids));
        }
        conn.execute_batch("DROP INDEX flow_pending_review")
            .unwrap();
        for (sql, values, expected) in cases {
            let started = std::time::Instant::now();
            let ids: Vec<String> = conn
                .prepare(&sql)
                .unwrap()
                .query_map(rusqlite::params_from_iter(values), |row| row.get(0))
                .unwrap()
                .map(Result::unwrap)
                .collect();
            assert_eq!(
                ids, expected,
                "Index must preserve historical/review row selection"
            );
            println!(
                "unindexed_ms={} rows={}",
                started.elapsed().as_secs_f64() * 1000.0,
                ids.len()
            );
        }
    }

    #[test]
    fn session_metadata_survives_unreadable_detail_without_weakening_exact_reads() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let input = crate::session_record::new_artifact_key();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO flow_sessions(id,invocation_json,cwd,step_index,iteration,
                position_version,worker_generation,updated_at,state,current_capture)
                VALUES('flow',?1,'/unavailable',0,0,1,0,1,'current',NULL)",
                params![
                    json!({"id":"flow","flow":"retained","steps":"invalid capture"}).to_string()
                ],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO agent_sessions(id,title,title_source,created_at,kind,
                interactive,input_published,cwd,flow_session_id,request)
                VALUES('session','Session','human',1,'conversation',1,1,'/unavailable','flow',?1)",
                params!["large request".repeat(1000)],
            )
            .unwrap();
            super::test_capture(&conn, "session", &input);
            conn.execute("UPDATE flow_sessions SET current_capture=(SELECT current_capture FROM agent_sessions WHERE id='session') WHERE id='flow'", []).unwrap();
            conn.execute(
                "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload,captured_event)
                VALUES('session','observed',?1,1,'not JSON',(SELECT current_capture FROM agent_sessions WHERE id='session'))",
                [format!("{input}:events.jsonl:0")],
            )
            .unwrap();
        }
        let rows = store.session_summaries(&SessionFilter::default()).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].flow.as_ref().unwrap().name.as_deref(),
            Some("retained")
        );
        assert_eq!(rows[0].node, None);
        assert!(
            store.flow("flow").is_err(),
            "An exact action still validates the capture"
        );
        assert!(
            store.input_events(&input).is_err(),
            "Exact history still reports corrupt payload"
        );
        {
            let conn = store.conn.lock().unwrap();
            conn.execute("UPDATE flow_sessions SET invocation_json=?1,state='completed',ended_at=2 WHERE id='flow'",
                [json!({"id":"flow","flow":"renamed","steps":"invalid capture"}).to_string()]).unwrap();
        }
        let flow = store
            .session_summary("session")
            .unwrap()
            .unwrap()
            .flow
            .unwrap();
        assert_eq!(flow.name.as_deref(), Some("renamed"));
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
                .session_summary("imported")
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
                .session_summary("imported")
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
                .session_summary("imported")
                .unwrap()
                .unwrap()
                .independent
        );
    }

    #[test]
    fn session_metadata_filters_before_decoding_and_preserves_explicit_membership() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        {
            let conn = store.conn.lock().unwrap();
            for (id, title, interactive, repo, iterations, membership) in [
                ("a", "Alpha%", true, "/repo", None, Some("independent")),
                ("b", "Beta", true, "/repo", None, None),
                ("headless", "Headless", false, "/repo", None, None),
                (
                    "foreign",
                    "Foreign",
                    true,
                    "/other",
                    Some("bad metadata"),
                    None,
                ),
                ("later", "Zulu", true, "/repo", Some("bad metadata"), None),
            ] {
                let input = crate::session_record::new_artifact_key();
                conn.execute(
                    "INSERT INTO agent_sessions(id,title,title_source,created_at,kind,
                    interactive,input_published,cwd,repo,iterations)
                    VALUES(?1,?2,'human',1,'conversation',?3,1,'/unavailable',?4,?5)",
                    params![id, title, interactive, repo, iterations],
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
        let rows = store.session_summaries(&filter).unwrap();
        assert_eq!(
            rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert!(rows[0].independent);
        assert!(!rows[1].independent);
        filter.search = Some("%".into());
        assert_eq!(
            store.session_summaries(&filter).unwrap().len(),
            1,
            "Contains search is literal"
        );
        filter.search = Some("ALPHA".into());
        assert_eq!(store.session_summaries(&filter).unwrap()[0].id, "a");
        filter.search = None;
        filter.interactive = Some(false);
        assert_eq!(store.session_summaries(&filter).unwrap()[0].id, "headless");
        filter.interactive = Some(true);
        filter.offset = 1;
        filter.limit = 1;
        assert_eq!(store.session_summaries(&filter).unwrap()[0].id, "b");
        filter.after = Some(String::new());
        let first = store.session_summaries(&filter).unwrap();
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
        let second = store.session_summaries(&filter).unwrap();
        assert_eq!(second[0].id, "b");
        assert_eq!(second[0].title, "000 renamed");
        filter.after = Some(String::new());
        filter.limit = 2;
        assert_eq!(
            store
                .session_summaries(&filter)
                .unwrap()
                .iter()
                .map(|row| row.id.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"],
            "renaming must not reorder the ID page used to build its cursor"
        );
    }
}
