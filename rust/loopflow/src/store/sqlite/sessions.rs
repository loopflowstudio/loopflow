//! Session transactions share the invocation's SQLite transaction and fences.

use crate::id::AgentSessionId;
use std::fs::File;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::durable::TaskId;
use crate::session::{LfSession, PrimaryScope, TitleSource};
use crate::store::{StoreError, StoreResult};

use super::SqliteStore;

/// The Graph whose process recorded, as a step, the Process that captured session
/// `s`'s current input. Sessions carry no Graph column of their own.
macro_rules! session_graph {
    () => {
        "(SELECT gs.graph_lf_process_id FROM session_events captured JOIN graph_process_steps gs ON gs.lf_process_id=captured.lf_process_id WHERE captured.seq=s.current_capture)"
    };
}
/// That step's node and loop counts, as its Graph process recorded them.
macro_rules! session_step {
    ($column:literal) => {
        concat!("(SELECT gs.", $column, " FROM session_events captured JOIN graph_process_steps gs ON gs.lf_process_id=captured.lf_process_id WHERE captured.seq=s.current_capture)")
    };
}
pub(super) const SESSION_GRAPH: &str = session_graph!();
/// Session `s`'s current AgentProcess last reported work in progress rather
/// than waiting for a person.
const TURN_RUNNING: &str = "EXISTS(SELECT 1 FROM session_activity act
    WHERE act.session_id=s.id AND act.agent_process_id IS s.agent_process_id
    AND NOT CASE WHEN act.program_status IS NOT NULL THEN
        EXISTS(SELECT 1 FROM json_each(act.program_status,'$.records') r
            WHERE json_extract(r.value,'$.state') IN ('idle','blocked','done'))
    ELSE act.pending_input>0 OR (act.open_tools=0 AND act.yielded=1) END)";

const SESSION_SELECT: &str = concat!("SELECT s.id,c.receipt_key AS artifact_key,s.title,s.title_source,
    (SELECT json_extract(feedback.payload,'$.summary') FROM session_events feedback WHERE feedback.session_id=s.id AND feedback.kind='observed' AND feedback.receipt_key='legacy_review_feedback') AS feedback_summary,s.completed_at,s.created_at,s.request,s.interactive,s.repo,
    s.task_id,s.wave_id,", session_graph!(), " AS graph_lf_process_id,s.work_source,s.bound_at,
    s.input_published,s.cwd,s.skill,s.provider,s.model,", session_step!("node"), " AS node,", session_step!("iterations"), " AS iterations,json_extract(c.payload,'$.caller_key') AS caller_artifact_key,s.current_capture FROM agent_sessions s LEFT JOIN session_events c ON c.seq=s.current_capture");

fn read_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoreResult<LfSession>> {
    Ok((|| {
        Ok(LfSession {
            captured: row.get(23)?,
            caller_artifact_key: row
                .get::<_, Option<String>>(22)?
                .map(|id| crate::session_record::parse_artifact_key(&id))
                .transpose()
                .map_err(invalid)?,
            id: row.get(0)?,
            artifact_key: row
                .get::<_, Option<String>>(1)?
                .map(|key| crate::session_record::parse_artifact_key(&key))
                .transpose()
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
            graph_lf_process_id: row.get(12)?,
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

pub(super) fn session_in(conn: &Connection, id: &str) -> StoreResult<Option<LfSession>> {
    let canonical = crate::id::canonical_session(id);
    let id = canonical.as_str();
    conn.query_row(
        &format!("{SESSION_SELECT} WHERE s.id=?1"),
        [id],
        read_session,
    )
    .optional()?
    .transpose()
}

/// Reported status wins for the current AgentProcess; otherwise use the
/// current attachment's input/hand-back/quiet reading. Filter before pagination.
fn available_sql(session: &str, now: i64) -> String {
    format!(
        "EXISTS(SELECT 1 FROM session_activity act
            JOIN processes p ON p.id={session}.agent_process_id
            WHERE act.session_id={session}.id
            AND {session}.completed_at IS NULL AND act.agent_process_id IS {session}.agent_process_id
            AND p.completed_at IS NULL
            AND CASE WHEN act.program_status IS NOT NULL THEN
                EXISTS(SELECT 1 FROM json_each(act.program_status,'$.records') r
                    WHERE json_extract(r.value,'$.state')='blocked'
                    OR ({session}.interactive=1 AND json_extract(r.value,'$.state')='idle'))
            ELSE act.attachment_token=p.attachment_token
                AND (act.pending_input>0 OR (act.open_tools=0 AND (({session}.interactive=1 AND act.yielded=1)
                    OR {now}-act.observed_at>={quiet}))) END)",
        quiet = crate::session::AVAILABILITY_QUIET_SECONDS
    )
}

fn inventory_query(
    filter: &crate::session::SessionFilter,
    select: &str,
    now: i64,
    live_agents: &str,
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
    if filter.available {
        sql.push_str(&format!(
            " AND {} AND s.agent_process_id IN (SELECT value FROM json_each({}))",
            available_sql("s", now),
            bind(Value::Text(live_agents.into()))
        ));
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
// before joining Graph/Work labels; no request or historical payload is selected.
const SUMMARY_SELECT: &str = concat!("SELECT s.id,c.receipt_key AS artifact_key,s.title,s.title_source,
    (SELECT json_extract(feedback.payload,'$.summary') FROM session_events feedback WHERE feedback.session_id=s.id AND feedback.kind='observed' AND feedback.receipt_key='legacy_review_feedback') AS feedback_summary,s.completed_at,s.interactive,s.task_id,s.wave_id,
    ", session_graph!(), " AS graph_lf_process_id,s.cwd,s.skill,s.provider,s.model,", session_step!("node"), " AS node,", session_step!("iterations"), " AS iterations,s.current_capture
    FROM agent_sessions s LEFT JOIN session_events c ON c.seq=s.current_capture");

const MEMBERSHIP_KIND: &str = "CASE WHEN json_valid(payload) THEN CASE WHEN json_extract(payload,'$.source')='manifest.json' AND json_extract(payload,'$.evidence.schema_version')=1 AND json_extract(payload,'$.evidence.artifact_key')=json_extract(payload,'$.input_id') AND receipt_key=json_extract(payload,'$.input_id')||':manifest.json' THEN json_extract(payload,'$.evidence.flow.kind') END END";

fn summary_query(page: &str, by_id: bool, now: i64) -> String {
    let available = available_sql("a", now);
    let order = if by_id { "s.id" } else { "s.title,s.id" };
    // A Graph is the Graph process above the step that captured the current input.
    format!("WITH page AS MATERIALIZED ({page})
        SELECT s.id,s.artifact_key,s.title,s.title_source,s.feedback_summary,s.completed_at,s.interactive,s.task_id,s.wave_id,s.graph_lf_process_id,s.cwd,s.skill,s.provider,s.model,s.node,s.iterations,s.current_capture,graph_process.id,graph.graph,graph_process.outcome,graph_process.completed_at,step.started_at,
        (gs.seq=(SELECT MAX(later.seq) FROM graph_process_steps later WHERE later.graph_lf_process_id=gs.graph_lf_process_id)),
        w.slug,
        ((SELECT {MEMBERSHIP_KIND} FROM session_events INDEXED BY session_input_membership
         WHERE session_id=s.id AND captured_event=s.current_capture
         AND kind='observed' AND substr(receipt_key,-14)=':manifest.json')='independent'),
        (SELECT json_group_array(id) FROM ({})),
        a.primary_scope,
        {available},
        COALESCE(({task_state}) IN ('done','abandoned'),0),
        EXISTS(SELECT 1 FROM tasks p WHERE p.primary_session_id=s.id),
        (SELECT act.program_status FROM session_activity act WHERE act.session_id=s.id AND act.agent_process_id IS a.agent_process_id),a.agent_process_id,p.completed_at,p.pid,p.os_started_at,(SELECT terminal_id FROM processes WHERE id=p.attached_lf_process_id AND completed_at IS NULL),
        (SELECT act.preview FROM session_activity act WHERE act.session_id=s.id AND act.agent_process_id IS a.agent_process_id
         AND json_extract(act.preview,'$.provider_thread')=a.provider_thread
         AND act.attachment_token=p.attachment_token AND p.attached_lf_process_id IS NOT NULL AND a.completed_at IS NULL),
        p.started_at,
        COALESCE((SELECT o.viewed=0 FROM processes o WHERE o.id=a.transport_lf_process_id AND o.completed_at IS NULL),0),
        (SELECT pin.account_id FROM provider_session_accounts pin WHERE pin.provider=s.provider AND pin.provider_session_id=a.provider_thread)
        FROM page s JOIN agent_sessions a ON a.id=s.id
        LEFT JOIN processes p ON p.id=a.agent_process_id
        LEFT JOIN session_events captured ON captured.seq=s.current_capture
        LEFT JOIN graph_process_steps gs ON gs.lf_process_id=captured.lf_process_id
        LEFT JOIN graph_processes graph ON graph.lf_process_id=gs.graph_lf_process_id
        LEFT JOIN processes step ON step.id=gs.lf_process_id
        LEFT JOIN processes graph_process ON graph_process.id=gs.graph_lf_process_id
        LEFT JOIN wave_addresses w ON w.id=s.wave_id
        LEFT JOIN tasks t ON t.id=s.task_id
        ORDER BY {order}", super::task_work::session_tasks("a"),
        task_state = super::durable::task_state_sql("t"))
}

fn read_summary(
    conn: &Connection,
    labels: &mut std::collections::HashMap<TaskId, Option<String>>,
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
        let graph = match row.get::<_, Option<String>>(17)? {
            Some(graph_process) => {
                let completed: Option<i64> = row.get(20)?;
                let name: String = row.get(18)?;
                Some(crate::session::GraphProcessSummary {
                    id: graph_process,
                    name,
                    state: crate::session::GraphProcessSummaryState::of_process(
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
        let agent_process_id: Option<crate::id::LfProcessId> = row.get(31)?;
        let agent_process_evidence = crate::journal::agent_process_evidence(
            row.get(32)?,
            row.get(33)?,
            row.get(34)?,
            row.get(37)?,
            crate::journal::process_identity_evidence,
        );
        let live = agent_process_evidence == crate::journal::ProcessIdentityEvidence::Live;
        let unviewed: bool = row.get(38)?;
        let task_identifier = task_id
            .as_ref()
            .map(|id| super::children::cached_short_task_id(conn, labels, id))
            .transpose()?
            .flatten();
        Ok(crate::session::SessionSummary {
            preview: row
                .get::<_, Option<String>>(36)?
                .map(|json| serde_json::from_str(&json))
                .transpose()?
                .filter(|_| live),
            // A terminal no viewer occupies is gone, whatever its launch recorded.
            terminal_id: row
                .get::<_, Option<String>>(35)?
                .filter(|_| live && !unviewed),
            unviewed,
            task_ids: serde_json::from_str(&row.get::<_, String>(25)?)?,
            primary_scope: row.get(26)?,
            agent_process_evidence,
            available: live && row.get(27)?,
            program_status: row
                .get::<_, Option<String>>(30)?
                .map(|json| serde_json::from_str(&json))
                .transpose()?
                .filter(|_| live),
            agent_process_id,
            task_terminal: row.get(28)?,
            task_primary: row.get(29)?,
            captured: row.get(16)?,
            id: row.get(0)?,
            artifact_key: row
                .get::<_, Option<String>>(1)?
                .map(|key| crate::session_record::parse_artifact_key(&key))
                .transpose()
                .map_err(invalid)?,
            title: row.get(2)?,
            title_source: serde_json::from_value(serde_json::Value::String(row.get(3)?))?,
            ready_summary: row.get(4)?,
            completed_at: row.get(5)?,
            interactive: row.get(6)?,
            task_id,
            wave_id,
            graph_lf_process_id: row.get(9)?,
            cwd: row.get::<_, String>(10)?.into(),
            skill: row.get(11)?,
            provider: row.get(12)?,
            account: row
                .get::<_, Option<String>>(39)?
                .map(|id| crate::store::ProviderAccountId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            model: row.get(13)?,
            node: row.get(14)?,
            iterations: row
                .get::<_, Option<String>>(15)?
                .map(|raw| serde_json::from_str(&raw))
                .transpose()?,
            graph,
            graph_step_latest: row.get::<_, Option<bool>>(22)?.unwrap_or(false),
            wave_name: row.get(23)?,
            task_identifier,
            independent: row.get::<_, Option<bool>>(24)?.unwrap_or(false),
        })
    })())
}

fn task_primary_in(conn: &Connection, task: &TaskId) -> StoreResult<Option<LfSession>> {
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

pub(super) fn primary_session_in(
    conn: &rusqlite::Connection,
    scope: &PrimaryScope,
) -> StoreResult<Option<LfSession>> {
    let marked = |kind: &str, column: &str, id: String| {
        conn.query_row(
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
    match scope {
        PrimaryScope::Repository(repo) => marked("repository", "repo", repo.to_string()),
        PrimaryScope::Wave(wave) => marked("wave", "wave_id", wave.to_string()),
        PrimaryScope::Task(task) => task_primary_in(conn, task),
    }
}

pub(super) fn session_summaries_in(
    conn: &Connection,
    filter: &crate::session::SessionFilter,
    now: i64,
    live_agents: &str,
) -> StoreResult<Vec<crate::session::SessionSummary>> {
    let mut labels = std::collections::HashMap::new();
    let (page, values) = inventory_query(filter, SUMMARY_SELECT, now, live_agents)?;
    let mut query = conn.prepare(&summary_query(&page, filter.after.is_some(), now))?;
    let rows = query.query_map(rusqlite::params_from_iter(values), |row| {
        read_summary(conn, &mut labels, row)
    })?;
    rows.map(|row| row?).collect()
}

pub(super) fn complete_primary_session_in(
    conn: &Connection,
    scope: &PrimaryScope,
) -> StoreResult<Option<LfSession>> {
    let current = primary_session_in(conn, scope)?;
    if let Some(current) = &current {
        conn.execute(
            "UPDATE agent_sessions SET completed_at=?2 WHERE id=?1",
            params![current.id, crate::store::rows::now_unix()],
        )?;
    }
    Ok(current)
}

fn ensure_primary_session_in(
    conn: &Connection,
    scope: &PrimaryScope,
    replacing: Option<&str>,
    session: LfSession,
    caller_process: Option<&crate::id::LfProcessId>,
) -> StoreResult<LfSession> {
    let current = primary_session_in(conn, scope)?;
    match current {
        Some(current) if Some(current.id.as_str()) == replacing => {
            conn.execute(
                "UPDATE agent_sessions SET completed_at=?2 WHERE id=?1",
                params![current.id, crate::store::rows::now_unix()],
            )?;
        }
        Some(current) => return Ok(current),
        None => {}
    }
    let session = reserve_session_in(conn, session, caller_process)?;
    // A Task's primary stays an ordinary member conversation; its Task
    // names it. A repository's or Wave's is marked on its own row.
    match scope {
        PrimaryScope::Repository(_) => conn.execute(
            "UPDATE agent_sessions SET primary_scope='repository' WHERE id=?1",
            [&session.id],
        )?,
        PrimaryScope::Wave(_) => conn.execute(
            "UPDATE agent_sessions SET primary_scope='wave' WHERE id=?1",
            [&session.id],
        )?,
        PrimaryScope::Task(task) => conn.execute(
            "UPDATE tasks SET primary_session_id=?2 WHERE id=?1",
            params![task.as_str(), session.id],
        )?,
    };
    Ok(session)
}

impl SqliteStore {
    pub(crate) fn bind_session_transport(
        &self,
        session: &str,
        attachment: &crate::process::SessionAttachment,
        owner: &crate::id::LfProcessId,
    ) -> StoreResult<()> {
        let canonical = crate::id::canonical_session(session);
        let session = canonical.as_str();
        self.with_session_attachment(session, attachment, || {
            let conn = self.conn.lock().expect("store mutex poisoned");
            conn.execute(
                "UPDATE agent_sessions SET transport_lf_process_id=?2 WHERE id=?1",
                params![session, owner.as_str()],
            )?;
            Ok(())
        })
    }

    pub(crate) fn session_transport(
        &self,
        session: &str,
    ) -> StoreResult<Option<crate::id::LfProcessId>> {
        let canonical = crate::id::canonical_session(session);
        let session = canonical.as_str();
        let conn = self.conn.lock().expect("store mutex poisoned");
        let id: Option<String> = conn
            .query_row(
                "SELECT s.transport_lf_process_id FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id WHERE s.id=?1 AND p.completed_at IS NULL",
                [session],
                |row| row.get(0),
            )
            .optional()?
            .flatten();
        id.map(|id| crate::id::LfProcessId::parse(&id).map_err(invalid))
            .transpose()
    }

    /// The Session an execution owner runs, for its viewers.
    pub(crate) fn transport_session(
        &self,
        owner: &crate::id::LfProcessId,
    ) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT id FROM agent_sessions WHERE transport_lf_process_id=?1",
                [owner.as_str()],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Unfinished interactive Sessions whose execution owner never recorded an
    /// exit, with that owner. Leaving or detaching records one; a restart,
    /// crash or kill does not. Liveness is the caller's observation.
    pub(crate) fn unexited_session_owners(
        &self,
    ) -> StoreResult<Vec<(String, crate::id::LfProcessId)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT s.id, s.transport_lf_process_id FROM agent_sessions s
             JOIN processes o ON o.id=s.transport_lf_process_id
             WHERE s.interactive=1 AND s.completed_at IS NULL AND o.completed_at IS NULL
             ORDER BY s.id",
        )?;
        let rows = query
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|(session, owner)| {
                Ok((
                    session,
                    crate::id::LfProcessId::parse(&owner).map_err(invalid)?,
                ))
            })
            .collect()
    }

    /// Whether the Session's current AgentProcess last reported work in progress,
    /// rather than waiting for a person, when it stopped.
    pub(crate) fn session_turn_was_running(&self, session: &str) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            &format!(
                "SELECT EXISTS(SELECT 1 FROM agent_sessions s WHERE s.id=?1 AND {TURN_RUNNING})"
            ),
            [session],
            |row| row.get(0),
        )?)
    }

    /// Record whether a viewer is attached to execution owner `owner`.
    pub(crate) fn record_owner_viewers(
        &self,
        owner: &crate::id::LfProcessId,
        viewed: bool,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE processes SET viewed=?2 WHERE id=?1",
            params![owner.as_str(), viewed],
        )?;
        Ok(())
    }

    /// Whether every Session execution owner `owner` holds is a standalone
    /// interactive conversation waiting for a person. A Graph step, a headless
    /// Run, or a turn in progress keeps it running.
    pub(crate) fn owner_waits_for_person(
        &self,
        owner: &crate::id::LfProcessId,
    ) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // A Graph started inside an owner, as from a terminal, holds its steps
        // there. CROSS JOIN seeds the walk from the few Graphs, not every Process.
        Ok(conn.query_row(
            &format!(
                "WITH RECURSIVE graph_ancestors(id) AS (
                     SELECT p.parent_lf_process_id FROM graph_processes f
                         CROSS JOIN processes p ON p.id=f.lf_process_id
                     UNION SELECT p.parent_lf_process_id FROM processes p
                         JOIN graph_ancestors a ON p.id=a.id)
                 SELECT ?1 NOT IN (SELECT id FROM graph_ancestors WHERE id IS NOT NULL)
                 AND EXISTS(SELECT 1 FROM agent_sessions s
                     WHERE s.transport_lf_process_id=?1 AND s.completed_at IS NULL)
                 AND NOT EXISTS(SELECT 1 FROM agent_sessions s
                     WHERE s.transport_lf_process_id=?1
                     AND (s.interactive=0 OR {SESSION_GRAPH} IS NOT NULL
                         OR (s.completed_at IS NULL AND {TURN_RUNNING})))"
            ),
            [owner.as_str()],
            |row| row.get(0),
        )?)
    }

    /// The unfinished Sessions execution owner `owner` holds, by id.
    pub(crate) fn owner_sessions(
        &self,
        owner: &crate::id::LfProcessId,
    ) -> StoreResult<Vec<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT id FROM agent_sessions
             WHERE transport_lf_process_id=?1 AND completed_at IS NULL ORDER BY id",
        )?;
        let rows = query
            .query_map([owner.as_str()], |row| row.get(0))?
            .collect::<rusqlite::Result<Vec<String>>>()?;
        Ok(rows)
    }

    /// Whether a viewer is attached to execution owner `owner`.
    pub(crate) fn owner_viewed(&self, owner: &crate::id::LfProcessId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT viewed FROM processes WHERE id=?1",
                [owner.as_str()],
                |row| row.get::<_, Option<bool>>(0),
            )
            .optional()?
            .flatten()
            .unwrap_or(false))
    }

    /// A Task's unfinished interactive conversations, each with its last opening.
    pub(crate) fn task_conversations(
        &self,
        task: &TaskId,
    ) -> StoreResult<Vec<(LfSession, Option<i64>)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT candidates.id,candidates.artifact_key,candidates.title,candidates.title_source,candidates.feedback_summary,candidates.completed_at,candidates.created_at,candidates.request,candidates.interactive,candidates.repo,candidates.task_id,candidates.wave_id,candidates.graph_lf_process_id,candidates.work_source,candidates.bound_at,candidates.input_published,candidates.cwd,candidates.skill,candidates.provider,candidates.model,candidates.node,candidates.iterations,candidates.caller_artifact_key,candidates.current_capture, (
                SELECT MAX(json_extract(payload,'$.opened_at_ms'))
                FROM session_events WHERE session_id=candidates.id AND kind='observed'
                AND json_extract(payload,'$.type')='interactive_opened'
            ) AS opened_at FROM ({SESSION_SELECT}
            WHERE s.interactive=1 AND s.completed_at IS NULL
            AND s.id IN ({})) AS candidates",
            super::task_work::session_ids("?1")
        ))?;
        let rows = query.query_map([task.as_str()], |row| {
            Ok((read_session(row)?, row.get("opened_at")?))
        })?;
        rows.map(|row| {
            let (session, opened) = row?;
            Ok((session?, opened))
        })
        .collect()
    }

    /// The unfinished interactive conversation last opened in a checkout.
    pub(crate) fn checkout_conversation(&self, root: &Path) -> StoreResult<Option<LfSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let root = root.to_string_lossy();
        conn.query_row(
            &format!(
                "{SESSION_SELECT}
                WHERE s.interactive=1 AND s.completed_at IS NULL
                AND (s.cwd=?1 OR substr(s.cwd,1,length(?1)+1)=?1||'/')
                ORDER BY COALESCE((
                    SELECT MAX(json_extract(payload,'$.opened_at_ms'))
                    FROM session_events WHERE session_id=s.id AND kind='observed'
                    AND json_extract(payload,'$.type')='interactive_opened'
                ), s.created_at*1000) DESC, s.id DESC LIMIT 1"
            ),
            [root.as_ref()],
            read_session,
        )
        .optional()?
        .transpose()
    }

    /// The conversation a Task's primary pointer names, while it is unfinished.
    pub(crate) fn task_primary(&self, task: &TaskId) -> StoreResult<Option<LfSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        task_primary_in(&conn, task)
    }

    /// Name one of the Task's own unfinished interactive conversations its
    /// primary. The earlier choice stays an ordinary conversation of the Task.
    pub(crate) fn choose_task_primary(
        &self,
        task: &TaskId,
        session: &str,
    ) -> StoreResult<LfSession> {
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

    // Available filters run before pagination; absence/unknown cannot occupy a page.
    fn live_agent_ids(&self, needed: bool) -> StoreResult<String> {
        let mut live = Vec::new();
        if needed {
            let identities = {
                let conn = self.conn.lock().expect("store mutex poisoned");
                let mut query = conn.prepare("SELECT id,pid,os_started_at FROM processes WHERE kind='agent' AND completed_at IS NULL")?;
                let rows = query.query_map([], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<u32>>(1)?,
                        row.get::<_, Option<i64>>(2)?,
                    ))
                })?;
                rows.collect::<rusqlite::Result<Vec<_>>>()?
            };
            for (id, pid, start) in identities {
                if crate::journal::agent_process_evidence(
                    None,
                    pid,
                    start,
                    None,
                    crate::journal::process_identity_evidence,
                ) == crate::journal::ProcessIdentityEvidence::Live
                {
                    live.push(id);
                }
            }
        }
        Ok(serde_json::to_string(&live)?)
    }

    /// Each row's Available is judged at `now`.
    pub(crate) fn session_summaries(
        &self,
        filter: &crate::session::SessionFilter,
        now: i64,
    ) -> StoreResult<Vec<crate::session::SessionSummary>> {
        let live_agents = self.live_agent_ids(filter.available)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        session_summaries_in(&conn, filter, now, &live_agents)
    }

    pub(crate) fn session_summary(
        &self,
        id: &str,
        now: i64,
    ) -> StoreResult<Option<crate::session::SessionSummary>> {
        let canonical = crate::id::canonical_session(id);
        let id = canonical.as_str();
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        let mut labels = std::collections::HashMap::new();
        conn.query_row(
            &summary_query(&format!("{SUMMARY_SELECT} WHERE s.id=?1"), false, now),
            [id],
            |row| read_summary(&conn, &mut labels, row),
        )
        .optional()?
        .transpose()
    }

    /// Resolve a conversation or captured-input selector from its SQLite owner.
    pub(crate) fn resolve_history_input(&self, selector: &str) -> StoreResult<String> {
        let canonical = crate::id::canonical_session(selector);
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE id=?1)",
            [&canonical],
            |row| row.get::<_, bool>(0),
        )? {
            return Ok(canonical);
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
    pub(crate) fn sessions_for_agent_session(
        &self,
        thread: &AgentSessionId,
    ) -> StoreResult<Vec<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // Only `provider-session:` observations carry the id; matching their
        // receipt first spares parsing millions of other payloads.
        let mut query = conn.prepare(
            "SELECT id FROM agent_sessions WHERE provider_thread=?1
             UNION SELECT session_id FROM session_events WHERE kind='observed'
                AND instr(receipt_key,':provider-session:')>0
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
            .and_then(|session| session.artifact_key)
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
        let task_id = task
            .map(|selector| self.resolve_task_id(selector, None))
            .transpose()?
            .flatten();
        let task = task_id.as_ref().map(TaskId::as_str).or(task);
        // A captured input cannot outlive its lf process. Terminal and provider
        // turn evidence describe outcomes; attachment exit events do not own life.
        let inputs = {
            let mut conn = self.conn.lock().expect("store mutex poisoned");
            let conn = conn.savepoint()?;
            let mut query = conn.prepare("WITH inputs AS (
                SELECT i.seq AS captured,i.receipt_key AS input_id,i.session_id,json_extract(i.payload,'$.caller_key') AS caller_input_id,
                    COALESCE(m.observed_at,i.observed_at) AS started,
                    CASE WHEN m.seq IS NOT NULL THEN m.task_id ELSE i.task_id END AS task_id,
                    CASE WHEN m.seq IS NOT NULL THEN m.wave_id ELSE i.wave_id END AS wave_id,
                    CASE WHEN attached.completed_at IS NOT NULL THEN attached.completed_at
                    WHEN terminal.seq IS NOT NULL AND NOT (
                        json_valid(terminal.payload) AND CASE WHEN json_valid(terminal.payload) THEN
                        COALESCE(json_type(terminal.payload,'$.evidence.outcome')='text',0)
                        AND unixepoch(json_extract(terminal.payload,'$.evidence.ended_at')) IS NOT NULL ELSE 0 END
                    ) THEN NULL WHEN EXISTS (
                        SELECT 1 FROM session_events origin WHERE origin.session_id=s.id
                        AND origin.captured_event=i.seq AND origin.kind='started' AND NOT EXISTS (
                            SELECT 1 FROM session_events done WHERE done.session_id=origin.session_id
                            AND done.provider_thread=origin.provider_thread AND done.provider_turn=origin.provider_turn
                            AND done.kind='completed')
                        ) THEN NULL ELSE
                        COALESCE(terminal.observed_at,(
                            SELECT MAX(done.observed_at) FROM session_events origin JOIN session_events done
                            ON done.session_id=origin.session_id AND done.provider_thread=origin.provider_thread
                            AND done.provider_turn=origin.provider_turn AND done.kind='completed'
                            WHERE origin.session_id=s.id AND origin.captured_event=i.seq AND origin.kind='started')) END AS ended, NULL AS thread, NULL AS turn
                FROM session_events i JOIN agent_sessions s ON s.id=i.session_id
                LEFT JOIN session_events latest_turn ON latest_turn.seq=(
                    SELECT origin.seq FROM session_events origin WHERE origin.session_id=i.session_id
                    AND origin.captured_event=i.seq AND origin.kind='started' ORDER BY origin.seq DESC LIMIT 1)
                LEFT JOIN processes attached ON attached.id=CASE WHEN latest_turn.seq IS NULL
                    THEN i.lf_process_id ELSE latest_turn.lf_process_id END
                LEFT JOIN session_events m ON m.session_id=s.id AND m.kind='observed'
                    AND m.receipt_key=i.receipt_key||':manifest.json'
                LEFT JOIN session_events terminal ON terminal.session_id=s.id AND terminal.kind='observed'
                    AND terminal.receipt_key=i.receipt_key||':terminal.json'
                WHERE i.kind='captured' AND (?7 IS NULL OR i.receipt_key=?7) AND
                    (?7 IS NOT NULL OR m.seq IS NOT NULL
                    OR (i.receipt_key=(SELECT receipt_key FROM session_events WHERE seq=s.current_capture) AND s.input_published=1)
                    OR EXISTS(SELECT 1 FROM session_events origin WHERE origin.session_id=i.session_id
                        AND origin.captured_event=i.seq AND origin.kind='started'))
                UNION ALL
                SELECT NULL,NULL,e.session_id,NULL,e.started,origin.task_id,origin.wave_id,
                    COALESCE(MAX(attached.completed_at),e.completed),e.provider_thread,e.provider_turn
                FROM (SELECT session_id,provider_thread,provider_turn,MIN(observed_at) AS started,
                        MAX(CASE WHEN kind='completed' THEN observed_at END) AS completed
                    FROM session_events WHERE ?7 IS NULL AND kind IN ('started','usage','completed','output')
                        AND captured_event IS NULL
                    GROUP BY session_id,provider_thread,provider_turn) e
                LEFT JOIN session_events origin
                    ON origin.session_id=e.session_id AND origin.provider_thread=e.provider_thread
                    AND origin.provider_turn=e.provider_turn AND origin.kind='started'
                LEFT JOIN processes attached ON attached.id=origin.lf_process_id
                WHERE origin.captured_event IS NULL
                GROUP BY e.session_id,e.provider_thread,e.provider_turn)
                , eligible AS MATERIALIZED (
                SELECT captured,input_id,session_id,caller_input_id,started,task_id,wave_id,ended,thread,turn, SUM(ended IS NULL) OVER () AS unfinished,
                    ROW_NUMBER() OVER (PARTITION BY ended IS NULL ORDER BY started DESC,input_id DESC,session_id,thread,turn) AS ordinal,
                    COUNT(*) OVER () AS total
                FROM inputs
                WHERE (?1 IS NULL OR wave_id IN (SELECT id FROM wave_addresses WHERE id=?9 OR slug=?1)
                    OR EXISTS (SELECT 1 FROM session_events origin WHERE origin.session_id=inputs.session_id
                        AND inputs.captured IS NOT NULL AND origin.captured_event=inputs.captured AND origin.kind='started'
                        AND origin.wave_id IN (SELECT id FROM wave_addresses WHERE id=?9 OR slug=?1)))
                AND (?2 IS NULL OR task_id IN (SELECT t.id FROM tasks t JOIN projects p ON p.id=t.project_id
                    WHERE p.id=?10 OR p.project_slug=?2 OR p.external_project_id=?11)
                    OR EXISTS (SELECT 1 FROM session_events origin WHERE origin.session_id=inputs.session_id
                        AND inputs.captured IS NOT NULL AND origin.captured_event=inputs.captured AND origin.kind='started'
                        AND origin.task_id IN (SELECT t.id FROM tasks t JOIN projects p ON p.id=t.project_id
                            WHERE p.id=?10 OR p.project_slug=?2 OR p.external_project_id=?11)))
                AND (?3 IS NULL OR task_id IN (SELECT id FROM tasks WHERE id=?3 OR issue_identifier=?3 OR external_issue_id=?3)
                    OR EXISTS (SELECT 1 FROM session_events origin WHERE origin.session_id=inputs.session_id
                        AND inputs.captured IS NOT NULL AND origin.captured_event=inputs.captured AND origin.kind='started'
                        AND origin.task_id IN (SELECT id FROM tasks WHERE id=?3 OR issue_identifier=?3 OR external_issue_id=?3)))
                AND (?4 IS NULL OR caller_input_id=?4 OR caller_input_id IN (SELECT receipt_key FROM session_events WHERE kind='captured' AND session_id=?12))
                AND (started>=?5 OR (?6 AND ended>=?5)))
                SELECT eligible.session_id,eligible.input_id,eligible.caller_input_id,started,eligible.task_id,eligible.wave_id,
                    (SELECT slug FROM wave_addresses WHERE id=eligible.wave_id),
                    s.current_capture,s.cwd,s.repo,s.skill,s.provider,s.model,s.interactive,
                    total > MAX(?8,unfinished),COALESCE((SELECT json_extract(payload,'$.work_source') FROM session_events WHERE seq=eligible.captured),
                        CASE WHEN eligible.captured=s.current_capture AND s.bound_at IS NULL THEN s.work_source END),
                    eligible.captured,eligible.thread,eligible.turn
                FROM eligible JOIN agent_sessions s ON s.id=eligible.session_id
                WHERE (?8 IS NULL OR ended IS NULL OR ordinal<=MAX(?8-unfinished,0))
                ORDER BY started DESC,eligible.input_id DESC,eligible.session_id,eligible.thread,eligible.turn")?;
            // Names, slugs and capture locators keep their bytes; identities are canonical.
            let mut rows = query.query(params![
                wave,
                project,
                task,
                caller,
                window.0,
                window.1,
                input,
                window.2.map(|n| n as i64),
                wave.map(crate::id::canonical_uuid),
                project.map(|project| crate::id::canonical_prefixed(project, "proj_")),
                project.map(crate::id::canonical_uuid),
                caller.map(crate::id::canonical_session),
            ])?;
            let mut inputs = Vec::new();
            let mut labels = std::collections::HashMap::new();
            while let Some(row) = rows.next()? {
                let task: Option<String> = row.get(4)?;
                let task_identifier = task
                    .as_ref()
                    .map(|id| {
                        super::children::cached_short_task_id(
                            &conn,
                            &mut labels,
                            &TaskId::from_raw(id),
                        )
                    })
                    .transpose()?
                    .flatten();
                inputs.push((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                    task,
                    row.get::<_, Option<String>>(5)?,
                    (row.get::<_, Option<String>>(6)?, task_identifier),
                    row.get::<_, Option<i64>>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, Option<String>>(10)?,
                    row.get::<_, Option<String>>(11)?,
                    row.get::<_, Option<String>>(12)?,
                    row.get::<_, bool>(13)?,
                    row.get::<_, Option<bool>>(14)?.unwrap_or(false),
                    row.get::<_, Option<String>>(15)?,
                    row.get::<_, Option<i64>>(16)?,
                    row.get::<_, Option<AgentSessionId>>(17)?,
                    row.get::<_, Option<String>>(18)?,
                ));
            }
            inputs
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
                        id: session_id.clone().into(),
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
                            thread.as_ref(),
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

    pub fn session(&self, id: &str) -> StoreResult<Option<LfSession>> {
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

    pub fn session_for_artifact(&self, artifact_key: &str) -> StoreResult<Option<LfSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            &format!("{SESSION_SELECT} WHERE s.id=(SELECT session_id FROM session_events WHERE kind='captured' AND receipt_key=?1)"),
            [artifact_key],
            read_session,
        )
        .optional()?
        .transpose()
    }

    fn lock_session_checkouts(&self, session: &LfSession) -> StoreResult<Vec<File>> {
        self.lock_task_checkouts(&[session.cwd.as_path()], session.task_id.as_ref())
    }

    /// Admit the conversation and its captured input before provider effects.
    pub fn create_session(
        &self,
        session: LfSession,
        caller_process: Option<&crate::id::LfProcessId>,
    ) -> StoreResult<LfSession> {
        let _admission = self.lock_session_checkouts(&session)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = session_in(&tx, &session.id)? {
            return Ok(existing);
        }
        let session = reserve_session_in(&tx, session, caller_process)?;
        tx.commit()?;
        Ok(session)
    }

    pub(crate) fn primary_session(&self, scope: &PrimaryScope) -> StoreResult<Option<LfSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        primary_session_in(&conn, scope)
    }

    /// Find the scope's uncompleted primary, or admit `session` as it. When the
    /// current primary is `replacing`, complete it and admit its successor in
    /// the same transaction; a repeat naming a replaced predecessor finds the
    /// successor already admitted.
    pub fn ensure_primary_session(
        &self,
        scope: &PrimaryScope,
        replacing: Option<&str>,
        session: LfSession,
        caller_process: Option<&crate::id::LfProcessId>,
    ) -> StoreResult<LfSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session = ensure_primary_session_in(&tx, scope, replacing, session, caller_process)?;
        tx.commit()?;
        Ok(session)
    }

    /// Called under the Session launch lock after proving there is no live client.
    pub(crate) fn move_primary_workspace(
        &self,
        session: &LfSession,
        cwd: &Path,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE agent_sessions SET cwd=?3 WHERE id=?1 AND current_capture IS ?2 AND completed_at IS NULL AND (primary_scope IS NOT NULL OR EXISTS(SELECT 1 FROM tasks WHERE primary_session_id=agent_sessions.id))",
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
                    crate::repository::CanonicalRepo::discover(Path::new(&repo))
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
             AND input_published=0",
            params![id, captured],
        )? != 1
        {
            return Err(StoreError::InvalidAuthority(
                "conversation capture changed before publication".into(),
            ));
        }
        Ok(())
    }

    /// Replace captured input, retaining the conversation's workspace, native
    /// history, title, feedback and assignment. Explicit opening may retry a
    /// retired conversation without making it primary again.
    pub fn replace_session_input(
        &self,
        expected_input: Option<i64>,
        mut session: LfSession,
    ) -> StoreResult<LfSession> {
        let stored = self.session(&session.id)?.ok_or(StoreError::NotFound)?;
        let _admission = self.lock_session_checkouts(&stored)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous = session_in(&tx, &session.id)?.ok_or(StoreError::NotFound)?;
        if session.task_id == previous.task_id && session.task_id.is_some() {
            session.wave_id = previous.wave_id.clone();
        }
        if previous.captured != expected_input
            || previous.completed_at != session.completed_at
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
            crate::journal::current_lf_process_id().as_ref(),
        )?;
        let session = session_in(&tx, &session.id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(session)
    }

    /// Reserve input and attachment together, including for an explicitly retried
    /// retired Session. Retain its retirement; a losing claimant changes neither.
    pub(crate) fn claim_session_input(
        &self,
        mut next: LfSession,
        expected: Option<&crate::process::SessionAttachment>,
        process: &crate::id::LfProcessId,
        close: impl FnOnce() -> StoreResult<bool>,
    ) -> StoreResult<(LfSession, crate::process::SessionAttachment)> {
        let _admission = self.lock_session_checkouts(&next)?;
        let session = next.id.clone();
        self.with_session_resume(&session, expected, close, |tx| {
            let previous = session_in(tx, &next.id)?.ok_or(StoreError::NotFound)?;
            if next.task_id == previous.task_id && next.task_id.is_some() {
                next.wave_id = previous.wave_id.clone();
            }
            if previous.captured != next.captured
                || previous.completed_at != next.completed_at
                || previous.cwd != next.cwd
                || previous.task_id != next.task_id
                || previous.wave_id != next.wave_id
            {
                return Err(StoreError::InvalidAuthority(
                    "conversation changed before input admission".into(),
                ));
            }
            replace_input_in(tx, &mut next, Some(process))?;
            let attachment = super::processes::attach_in(tx, &next.id, expected, process, true)?;
            let next = session_in(tx, &next.id)?.ok_or(StoreError::NotFound)?;
            Ok((next, attachment))
        })
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
    ) -> StoreResult<LfSession> {
        let before = self.session(id)?.ok_or(StoreError::NotFound)?;
        let _admission = self.lock_task_checkouts(&[&before.cwd], Some(task))?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session = session_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        if session.cwd != before.cwd {
            return Err(StoreError::InvalidAuthority(
                "Session workspace changed before binding".into(),
            ));
        }
        if session.captured != expected_capture {
            return Err(StoreError::InvalidAuthority(
                "Session changed before binding".into(),
            ));
        }
        if let Some(bound) = &session.task_id {
            if bound != task {
                let issue =
                    super::children::short_task_id(&tx, bound)?.ok_or(StoreError::NotFound)?;
                return Err(StoreError::InvalidAuthority(format!(
                    "Session {id} already has Task {issue}; binding is permanent"
                )));
            }
        } else {
            let wave = super::durable::task_wave_in(&tx, task)?;
            if session
                .wave_id
                .as_ref()
                .is_some_and(|existing| Some(existing) != wave.as_ref())
            {
                return Err(StoreError::InvalidAuthority(format!(
                    "Session {id} belongs to another Wave"
                )));
            }
            tx.execute(
                "UPDATE agent_sessions SET task_id=?2,wave_id=?3,work_source='bound',bound_at=?4 WHERE id=?1",
                params![id, task.as_str(), wave.as_ref().map(crate::id::WaveId::as_str), crate::store::rows::now_unix()],
            )?;
        }
        repair_session_title_in(&tx, id, None)?;
        let bound = session_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(bound)
    }

    /// Sessions assigned to a Task after they began, oldest bind first.
    pub(crate) fn bound_sessions(&self) -> StoreResult<Vec<crate::session::SessionBind>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let conn = conn.savepoint()?;
        let mut query = conn.prepare(
            "SELECT s.id,s.bound_at,t.id,(SELECT slug FROM wave_addresses WHERE id=s.wave_id)
             FROM agent_sessions s JOIN tasks t ON t.id=s.task_id
             WHERE s.bound_at IS NOT NULL ORDER BY s.bound_at,s.id",
        )?;
        let mut rows = query.query([])?;
        let mut binds = Vec::new();
        let mut labels = std::collections::HashMap::new();
        while let Some(row) = rows.next()? {
            let task = TaskId::from_raw(row.get::<_, String>(2)?);
            binds.push(crate::session::SessionBind {
                session_id: row.get(0)?,
                at: row.get(1)?,
                task: super::children::cached_short_task_id(&conn, &mut labels, &task)?
                    .ok_or(StoreError::NotFound)?,
                wave: row.get(3)?,
            });
        }
        Ok(binds)
    }

    /// Every open Session.
    pub fn sessions(&self, filter: &crate::session::SessionFilter) -> StoreResult<Vec<LfSession>> {
        let live_agents = self.live_agent_ids(filter.available)?;
        let conn = self.conn.lock().expect("store mutex poisoned");
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        let (sql, values) = inventory_query(filter, SESSION_SELECT, now, &live_agents)?;
        let mut query = conn.prepare(&sql)?;
        let rows = query.query_map(rusqlite::params_from_iter(values), read_session)?;
        rows.map(|row| row?).collect()
    }

    pub fn session_inputs(&self, id: &str) -> StoreResult<Vec<String>> {
        let canonical = crate::id::canonical_session(id);
        let id = canonical.as_str();
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT receipt_key FROM session_events WHERE kind='captured' AND session_id=?1 ORDER BY seq",
        )?;
        let rows = query.query_map([id], |row| row.get::<_, String>(0))?;
        rows.map(|row| crate::session_record::parse_artifact_key(&row?).map_err(invalid))
            .collect()
    }

    pub fn rename_session(&self, id: &str, title: &str, source: TitleSource) -> StoreResult<()> {
        let canonical = crate::id::canonical_session(id);
        let id = canonical.as_str();
        if title.trim().is_empty() {
            return Err(invalid("Session title cannot be empty"));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session = session_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        rename_session_in(&tx, &session, title, source, None)?;
        tx.commit()?;
        Ok(())
    }

    /// The Session continues on `machine`: complete it here, naming where it went.
    pub(crate) fn complete_moved_session(
        &self,
        id: &str,
        machine: &crate::durable::MachineId,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE agent_sessions SET completed_at=COALESCE(completed_at,?3),moved_to_machine_id=?2 WHERE id=?1",
            params![id, machine.as_str(), crate::store::rows::now_unix()],
        )?;
        if updated != 1 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// The Machine a Session here moved to; `None` when it never left.
    pub fn session_moved_to(&self, id: &str) -> StoreResult<Option<crate::durable::MachineId>> {
        let canonical = crate::id::canonical_session(id);
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT moved_to_machine_id FROM agent_sessions WHERE id=?1",
            [canonical.as_str()],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
        .map(|machine| crate::durable::MachineId::parse(&machine).map_err(invalid))
        .transpose()
    }

    /// Where a Task's conversation went when it moved without its Task: the
    /// latest of its Sessions that moved, while none is open here.
    pub fn task_conversation_moved_to(
        &self,
        task: &TaskId,
    ) -> StoreResult<Option<(String, crate::durable::MachineId)>> {
        let ids = super::task_work::session_ids("?1");
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            &format!(
                "SELECT id,moved_to_machine_id FROM agent_sessions
                 WHERE id IN ({ids}) AND moved_to_machine_id IS NOT NULL
                 AND NOT EXISTS(SELECT 1 FROM agent_sessions WHERE id IN ({ids}) AND completed_at IS NULL)
                 ORDER BY completed_at DESC LIMIT 1"
            ),
            [task.as_str()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
        )
        .optional()?
        .map(|(id, machine)| {
            Ok((
                id,
                crate::durable::MachineId::parse(&machine).map_err(invalid)?,
            ))
        })
        .transpose()
    }

    /// `repository` or `wave` for a scope's ongoing conversation.
    pub(crate) fn session_primary_scope(&self, id: &str) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT primary_scope FROM agent_sessions WHERE id=?1",
                [id],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten())
    }

    /// A Session that moved away comes back: open again, in `cwd`.
    pub(crate) fn reopen_moved_session(&self, id: &str, cwd: &Path) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE agent_sessions SET completed_at=NULL,moved_to_machine_id=NULL,cwd=?2 WHERE id=?1",
            params![id, cwd.to_string_lossy()],
        )?;
        Ok(())
    }
}

fn rename_session_in(
    conn: &Connection,
    session: &LfSession,
    title: &str,
    source: TitleSource,
    previous: Option<&str>,
) -> StoreResult<()> {
    require_current_actor_in(conn, &session.id)?;
    let title = generated_title_in(conn, session, title, source, previous)?;
    conn.execute(
        "UPDATE agent_sessions SET title=?2,title_source=?3 WHERE id=?1 AND (title_source='generated' OR ?3='human')",
        params![session.id, title, title_source(source)],
    )?;
    Ok(())
}

fn generated_title_in(
    conn: &Connection,
    session: &LfSession,
    title: &str,
    source: TitleSource,
    previous: Option<&str>,
) -> StoreResult<String> {
    if source == TitleSource::Generated {
        if let Some(id) = &session.task_id {
            if let Some(label) = super::children::short_task_id(conn, id)? {
                return Ok(crate::naming::task_session_title(
                    title, id, &label, previous,
                ));
            }
        }
    }
    Ok(title.trim().to_string())
}

pub(super) fn repair_session_title_in(
    conn: &Connection,
    id: &str,
    previous: Option<&str>,
) -> StoreResult<()> {
    let session = session_in(conn, id)?.ok_or(StoreError::NotFound)?;
    if session.title_source == TitleSource::Generated {
        rename_session_in(
            conn,
            &session,
            &session.title,
            TitleSource::Generated,
            previous,
        )?;
    }
    Ok(())
}

pub(super) fn repair_task_titles_in(
    conn: &Connection,
    task: &TaskId,
    previous: Option<&str>,
) -> StoreResult<()> {
    let mut query = conn.prepare("SELECT id FROM agent_sessions WHERE task_id=?1 AND title_source='generated' AND completed_at IS NULL")?;
    let sessions = query
        .query_map([task.as_str()], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for id in sessions {
        match repair_session_title_in(conn, &id, previous) {
            Err(StoreError::InvalidAuthority(_)) => {} // A stale provider cannot repair its replaced conversation.
            result => result?,
        }
    }
    Ok(())
}

pub(super) fn retain_history_in(
    conn: &Connection,
    session: &LfSession,
    history: &[crate::session::SessionObservation],
) -> StoreResult<bool> {
    let mut changed = false;
    for observation in history {
        let captured: Option<(i64,String)> = conn.query_row(
            "SELECT seq,session_id FROM session_events WHERE kind='captured' AND receipt_key=?1",
            [&observation.artifact_key], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
        let (captured, owner) = captured.ok_or(StoreError::NotFound)?;
        if owner != session.id.as_str() {
            return Err(invalid("artifact belongs to another conversation"));
        }
        let key = format!("{}:{}", observation.artifact_key, observation.source);
        let payload = serde_json::to_string(&observation.payload)?;
        let saved: Option<String> = conn.query_row(
            "SELECT payload FROM session_events WHERE session_id=?1 AND kind='observed' AND receipt_key=?2",
            params![session.id,key], |row| row.get(0)).optional()?;
        if let Some(saved) = saved {
            if saved == payload {
                continue;
            }
            return Err(invalid(format!(
                "input {} has conflicting {} evidence",
                observation.artifact_key, observation.source
            )));
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
    conn: &Connection,
    mut session: LfSession,
    caller: Option<&crate::id::LfProcessId>,
) -> StoreResult<LfSession> {
    resolve_ancestry_in(conn, &mut session)?;
    insert_session_in(conn, &mut session, caller)?;
    Ok(session)
}

fn resolve_ancestry_in(conn: &Connection, session: &mut LfSession) -> StoreResult<()> {
    if let Some(task) = &session.task_id {
        let wave = super::durable::task_wave_in(conn, task)?;
        if session
            .wave_id
            .as_ref()
            .is_some_and(|given| Some(given) != wave.as_ref())
        {
            return Err(invalid("Session Task and Wave disagree"));
        }
        session.wave_id = wave;
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
            "SELECT EXISTS(SELECT 1 FROM agent_sessions s JOIN processes p ON p.id=s.agent_process_id WHERE s.id=?1
                AND p.id=?2 AND p.parent_lf_process_id=?3
                AND p.attached_lf_process_id IS NOT NULL)",
            params![id, caller.agent_process_id, caller.origin_lf_process_id],
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
            .ok_or_else(|| {
                invalid("capture does not belong to a recorded Session in this Machine")
            })?;
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
    session: &LfSession,
    observed_at: i64,
    process: Option<&crate::id::LfProcessId>,
) -> StoreResult<i64> {
    let key = session.input_key().map_err(invalid)?;
    let saved: Option<(i64, String, Option<String>)> = conn
        .query_row(
            "SELECT seq,session_id,json_extract(payload,'$.caller_key') FROM session_events
         WHERE kind='captured' AND receipt_key=?1",
            [key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if let Some((seq, owner, caller)) = saved {
        if owner != session.id.as_str() || caller != session.caller_artifact_key {
            return Err(invalid(
                "captured artifact has conflicting Session or caller evidence",
            ));
        }
        return Ok(seq);
    }
    let wave = match &session.task_id {
        Some(task) => super::durable::task_wave_in(conn, task)?,
        None => session.wave_id.clone(),
    };
    let payload = serde_json::json!({"artifact_key":session.artifact_key,
        "caller_key":session.caller_artifact_key,"cwd":session.cwd,"skill":session.skill,
        "provider":session.provider,"model":session.model,"work_source":session.work_source});
    conn.execute("INSERT INTO session_events(session_id,kind,receipt_key,lf_process_id,task_id,wave_id,observed_at,payload)
        VALUES(?1,'captured',?2,?3,?4,?5,?6,?7)",
        params![session.id,session.artifact_key,process,session.task_id.as_ref().map(TaskId::as_str),
            wave.as_ref().map(crate::id::WaveId::as_str),observed_at,serde_json::to_string(&payload)?])?;
    Ok(conn.last_insert_rowid())
}

fn insert_session_in(
    conn: &Connection,
    session: &mut LfSession,
    process: Option<&crate::id::LfProcessId>,
) -> StoreResult<()> {
    session.title = generated_title_in(conn, session, &session.title, session.title_source, None)?;
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
    session.captured = session
        .artifact_key
        .as_ref()
        .map(|_| capture_in(conn, session, session.created_at, process))
        .transpose()?;
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
    session: &mut LfSession,
    process: Option<&crate::id::LfProcessId>,
) -> StoreResult<()> {
    // Workspace admission owns location; replacing input cannot move Task membership.
    session.cwd = conn
        .query_row(
            "SELECT cwd FROM agent_sessions WHERE id=?1",
            [&session.id],
            |row| row.get::<_, String>(0),
        )?
        .into();
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
        process,
    )?);
    // A selected AgentSession belongs to its provider; another provider starts
    // a fresh one. Earlier selections remain in session_events.
    conn.execute("UPDATE agent_sessions SET current_capture=?2,input_published=?3,skill=?4,provider=?5,model=?6,
        request=?7,provider_thread=CASE WHEN provider IS ?5 THEN provider_thread END
        WHERE id=?1",
        params![session.id,session.captured,session.input_published,
            session.skill,session.provider,session.model,session.request])?;
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
    pub(crate) fn test_session(&self, id: &str, artifact: &str) -> LfSession {
        self.create_session(
            LfSession {
                captured: None,
                id: id.into(),
                artifact_key: Some(artifact.into()),
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
                graph_lf_process_id: None,
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

    use crate::session::{GraphProcessSummaryState, SessionFilter};

    #[test]
    fn input_replacement_retains_workspace_and_task_membership() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let session = store.test_session("moving", &crate::session_record::new_artifact_key());
        let wave = crate::id::WaveId::new();
        let project = crate::work::project::ProjectId::new();
        let task = crate::durable::TaskId::new();
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'proof','/repo',1)",
                [&wave],
            )
            .unwrap();
            conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,'project',1)", params![project.as_str(), wave]).unwrap();
            conn.execute("INSERT INTO repository_plans(id,repo,selected) SELECT 'repo_'||lower(hex(randomblob(16))),w.repo,1 FROM waves w WHERE NOT EXISTS(SELECT 1 FROM repository_plans r WHERE r.repo=w.repo AND r.selected=1) GROUP BY w.repo", []).unwrap();
            conn.execute("INSERT INTO tasks(repository_id,id,project_id,external_issue_id,issue_identifier,worktree,created_at,checkout_machine_id) VALUES((SELECT r.id FROM projects p JOIN waves w ON w.id=p.wave_id JOIN repository_plans r ON r.repo=w.repo AND r.selected=1 WHERE p.id=?2),?1,?2,'issue','PROOF-1',?3,1,(SELECT id FROM machines WHERE route='local'))", params![task.as_str(), project.as_str(), session.cwd.to_str().unwrap()]).unwrap();
        }
        assert_eq!(store.task_work(&task).unwrap().sessions.len(), 1);
        let exclusion = store.lock_checkout(&session.cwd).unwrap();
        let mut next = session.clone();
        next.cwd = home.path().join("destination");
        next.artifact_key = Some(crate::session_record::new_artifact_key());
        assert!(store
            .replace_session_input(session.captured, next.clone())
            .is_err());
        let retained = store.task_work(&task).unwrap().sessions;
        drop(exclusion);
        assert_eq!(
            retained.len(),
            1,
            "input replacement removed Task work while its source checkout was excluded"
        );
        let replacement = store.replace_session_input(session.captured, next).unwrap();
        assert_eq!(replacement.cwd, session.cwd);
        assert_eq!(replacement.task_id, None);
        assert_eq!(store.task_work(&task).unwrap().sessions.len(), 1);
        assert_ne!(replacement.captured, session.captured);
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
    fn retrying_retired_input_preserves_completion_and_rejects_stale_snapshots() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let first = store.test_session("retired", &crate::session_record::new_artifact_key());
        let mut next = first.clone();
        next.artifact_key = Some(crate::session_record::new_artifact_key());
        next.input_published = false;
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE agent_sessions SET completed_at=10 WHERE id='retired'",
                [],
            )
            .unwrap();
        let before = store.session_history("retired", 0, 100).unwrap();
        assert!(store
            .replace_session_input(first.captured, next.clone())
            .is_err());
        assert_eq!(store.session_history("retired", 0, 100).unwrap(), before);

        next.completed_at = Some(10);
        let next = store.replace_session_input(first.captured, next).unwrap();
        store.publish_capture(&next.id, next.captured).unwrap();
        let retained = store.session("retired").unwrap().unwrap();
        assert_eq!(retained.completed_at, Some(10));
        assert!(retained.input_published);
        assert_ne!(retained.captured, first.captured);
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
        next.artifact_key = Some(crate::session_record::new_artifact_key());
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
    fn session_metadata_reads_its_graph_from_the_process_that_captured_its_input() {
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("store.db")).unwrap();
        let input = crate::session_record::new_artifact_key();
        let graph_process =
            store.test_graph("retained", "/unavailable", &[("implement", None)], None);
        {
            let conn = store.conn.lock().unwrap();
            conn.execute(
                "INSERT INTO agent_sessions(id,title,title_source,created_at,
                interactive,input_published,cwd,request)
                VALUES('session','Session','human',1,1,1,'/unavailable',?1)",
                params!["large request".repeat(1000)],
            )
            .unwrap();
            // The step Process captured this conversation's input.
            conn.execute(
                "INSERT INTO session_events(session_id,kind,receipt_key,lf_process_id,observed_at,payload)
                 VALUES('session','captured',?1,(SELECT id FROM processes WHERE parent_lf_process_id=?2),1,
                    json_object('artifact_key',?1))",
                params![input, graph_process],
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
        // Listing reads the Graph from Processes and survives unreadable detail.
        let rows = store
            .session_summaries(&SessionFilter::default(), 0)
            .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(
            rows[0].graph_lf_process_id.as_deref(),
            Some(graph_process.as_str())
        );
        let graph = rows[0].graph.as_ref().unwrap();
        assert_eq!(
            (graph.name.as_str(), graph.state),
            ("retained", GraphProcessSummaryState::Current)
        );
        assert!(rows[0].graph_step_latest);
        assert_eq!(
            store
                .session("session")
                .unwrap()
                .unwrap()
                .graph_lf_process_id
                .as_deref(),
            Some(graph_process.as_str())
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
                "UPDATE processes SET outcome='succeeded',completed_at=2 WHERE id=?1",
                params![graph_process],
            )
            .unwrap();
        let graph = store
            .session_summary("session", 0)
            .unwrap()
            .unwrap()
            .graph
            .unwrap();
        assert_eq!(graph.state, GraphProcessSummaryState::Completed);
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
        filter.after = Some(first[0].id.to_string());
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
