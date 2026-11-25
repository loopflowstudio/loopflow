//! Session transactions share the invocation's SQLite transaction and fences.

use loopflow_model::id::AgentSessionId;
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};

use crate::session::{LfSession, PrimaryScope, TitleSource, WorkSource};
use crate::{StoreError, StoreResult};
use loopflow_model::durable::TaskId;

use super::SqliteStore;
use loopflow_model::id::{LfProcessId, TerminalId};

/// Caller observations acquired before a Session command enters storage.
#[derive(Debug, Clone)]
pub struct SessionCaller {
    pub agent: Option<crate::process::AgentCaller>,
    pub capture_key: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SessionProcessIdentity {
    pub id: String,
    pub pid: Option<u32>,
    pub os_started_at: Option<i64>,
    pub started_at: i64,
    /// The OS process of the lf holding the attachment; `None` when released.
    pub holder: Option<crate::process::AttachmentHolder>,
}

/// Explicit observed inputs for one pinned Session reading. SQL performs no OS acquisition.
#[derive(Debug)]
pub struct SessionProjection {
    pub now: i64,
    pub live_agents: String,
    pub evidence: std::collections::HashMap<String, crate::ProcessIdentityEvidence>,
    /// Whether the lf holding each agent's attachment runs, by agent.
    pub holders: std::collections::HashMap<String, crate::ProcessIdentityEvidence>,
    /// Each of `graph_runs`' results as this Machine observes it.
    pub graph_results: std::collections::HashMap<String, crate::graph_process::ExecutionResult>,
}

impl SessionProjection {
    fn evidence(
        &self,
        id: Option<&loopflow_model::id::AgentProcessId>,
    ) -> crate::ProcessIdentityEvidence {
        id.and_then(|id| self.evidence.get(id.as_str()))
            .copied()
            .unwrap_or(crate::ProcessIdentityEvidence::Unknown)
    }

    fn holder(
        &self,
        id: Option<&loopflow_model::id::AgentProcessId>,
    ) -> crate::ProcessIdentityEvidence {
        id.and_then(|id| self.holders.get(id.as_str()))
            .copied()
            .unwrap_or(crate::ProcessIdentityEvidence::Unknown)
    }

    fn graph_result(
        &self,
        run: &crate::graph_process::Execution,
    ) -> crate::graph_process::ExecutionResult {
        self.graph_results
            .get(run.id.as_str())
            .copied()
            .unwrap_or_else(|| run.result())
    }
}

/// What a Session reading must learn outside SQL, found from the agents that
/// have not ended. An ended agent is dead by its own row and appears nowhere here.
#[derive(Debug, PartialEq, Eq)]
pub struct SessionProjectionInputs {
    read_at: i64,
    session: Option<String>,
    /// The agents the OS must be asked about.
    pub identities: Vec<SessionProcessIdentity>,
    /// The Graph runs the OS must be asked about: every unended run when
    /// reading all Sessions, else the unended run whose step captured the one
    /// Session's input.
    pub graph_runs: Vec<crate::graph_process::Execution>,
    /// When a live agent's quiet time next makes its Session available with no write.
    pub deadlines: Vec<i64>,
}

pub(crate) fn session_projection_inputs_in(
    conn: &Connection,
    now: i64,
    session: Option<&str>,
) -> StoreResult<SessionProjectionInputs> {
    // Reading all Sessions starts from the unended agents, never from history.
    let (sessions, selected, graphs) = if session.is_some() {
        (
            "agent_sessions s",
            "s.id=?1",
            format!(
                "g.lf_process_id IN (SELECT {SESSION_GRAPH} FROM agent_sessions s WHERE s.id=?1)"
            ),
        )
    } else {
        (
            "agent_sessions s INDEXED BY agent_sessions_unended_agent",
            "?1 IS NULL AND s.agent_process_id IS NOT NULL",
            "?1 IS NULL".to_owned(),
        )
    };
    let mut query = conn.prepare(&format!(
        "SELECT s.agent_process_id,s.agent_pid,s.agent_os_started_at,s.agent_started_at,
         s.attached_lf_process_id IS NOT NULL,s.attached_pid,s.attached_os_started_at
         FROM {sessions} WHERE {selected} AND s.agent_ended_at IS NULL
         AND s.agent_started_at IS NOT NULL
         ORDER BY s.agent_process_id"
    ))?;
    let identities = query
        .query_map([session], |row| {
            Ok(SessionProcessIdentity {
                id: row.get(0)?,
                pid: row.get(1)?,
                os_started_at: row.get(2)?,
                started_at: row.get(3)?,
                holder: row
                    .get::<_, bool>(4)?
                    .then(|| -> rusqlite::Result<_> {
                        Ok(crate::process::AttachmentHolder {
                            pid: row.get(5)?,
                            os_started_at: row.get(6)?,
                        })
                    })
                    .transpose()?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut query = conn.prepare(&format!(
        "SELECT {} FROM graph_processes g WHERE g.ended_at IS NULL AND {graphs}
         ORDER BY g.lf_process_id",
        super::graph_inventory::run_columns("g")
    ))?;
    let graph_runs = query
        .query_map([session], super::graph_inventory::read_run)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut query = conn.prepare(&format!(
        "SELECT DISTINCT act.observed_at + ?2 FROM {sessions}
         JOIN session_activity act ON {}
         WHERE {selected} AND act.observed_at > ?3 - ?2 ORDER BY 1",
        current_agent_activity("s")
    ))?;
    let deadlines = query
        .query_map(
            params![session, crate::session::AVAILABILITY_QUIET_SECONDS, now],
            |row| row.get(0),
        )?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(SessionProjectionInputs {
        read_at: now,
        session: session.map(str::to_owned),
        identities,
        graph_runs,
        deadlines,
    })
}

impl SessionProjectionInputs {
    pub fn is_current(&self, conn: &Connection) -> StoreResult<bool> {
        Ok(session_projection_inputs_in(conn, self.read_at, self.session.as_deref())? == *self)
    }

    pub fn pin<'a>(
        &self,
        conn: &'a mut Connection,
    ) -> StoreResult<Option<rusqlite::Savepoint<'a>>> {
        let snapshot = conn.savepoint()?;
        if !self.is_current(&snapshot)? {
            return Ok(None);
        }
        Ok(Some(snapshot))
    }
}

impl SqliteStore {
    pub fn session_projection_inputs(
        &self,
        now: i64,
        session: Option<&str>,
    ) -> StoreResult<SessionProjectionInputs> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        session_projection_inputs_in(&snapshot, now, session)
    }
}

/// Why a held terminal cannot retire without losing work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalRetention {
    RunningGraph,
    /// No open Session names the terminal. This is unknown, not work: the row
    /// may be complete or absent, so it cannot show the terminal is safe to end.
    NoOpenSession,
    HeadlessRun,
    LiveTurn,
}

impl std::fmt::Display for TerminalRetention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            TerminalRetention::RunningGraph => "a running Graph",
            TerminalRetention::NoOpenSession => "no open Session",
            TerminalRetention::HeadlessRun => "a headless Run",
            TerminalRetention::LiveTurn => "a live turn",
        })
    }
}

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
/// Current provider work or a viewed terminal's working status holds Session `s`.
/// Terminal idle cannot settle owned work; unviewed status is only history.
const TURN_LIVE: &str = "EXISTS(SELECT 1 FROM session_activity act
    WHERE act.session_id=s.id AND act.agent_process_id IS s.agent_process_id
    AND s.agent_ended_at IS NULL
    AND ((act.attachment_token IS s.attachment_token
            AND (act.open_tools>0 OR act.pending_input>0 OR act.yielded=0))
        OR (act.program_status IS NOT NULL
            AND EXISTS(SELECT 1 FROM held_terminals t WHERE t.id=s.held_terminal_id AND t.viewed=1)
            AND NOT EXISTS(SELECT 1 FROM json_each(act.program_status,'$.records') r
                WHERE json_extract(r.value,'$.state') IN ('idle','blocked','done')))))";

/// SQL: which scope Session row `s` is primary for. A Task's primary is its
/// row in `task_primary_sessions`, which released lf also reads and writes;
/// LOO-877 moves it onto `primary_scope`.
pub(super) fn primary_scope_sql(s: &str) -> String {
    format!(
        "coalesce({s}.primary_scope,
            (SELECT 'task' FROM task_primary_sessions tp WHERE tp.session_id={s}.id))"
    )
}

const SESSION_SELECT: &str = concat!("SELECT s.id,c.receipt_key AS artifact_key,s.title,s.title_source,
    s.completed_at,s.created_at,s.request,s.interactive,s.repo,
    s.task_id,s.wave_id,", session_graph!(), " AS graph_lf_process_id,s.work_source,s.bound_at,
    s.input_published,s.cwd,s.skill,s.provider,s.model,", session_step!("node"), " AS node,", session_step!("iterations"), " AS iterations,json_extract(c.payload,'$.caller_key') AS caller_artifact_key,s.current_capture,s.opened_at FROM agent_sessions s LEFT JOIN session_events c ON c.seq=s.current_capture");

fn read_session(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoreResult<LfSession>> {
    Ok((|| {
        Ok(LfSession {
            captured: row.get(22)?,
            caller_artifact_key: row
                .get::<_, Option<String>>(21)?
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
            completed_at: row.get(4)?,
            created_at: row.get(5)?,
            request: row.get(6)?,
            interactive: row.get(7)?,
            repo: row.get(8)?,
            task_id: row
                .get::<_, Option<String>>(9)?
                .map(|id| TaskId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            wave_id: row
                .get::<_, Option<String>>(10)?
                .map(|id| loopflow_model::id::WaveId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            graph_lf_process_id: row.get(11)?,
            work_source: row
                .get::<_, Option<String>>(12)?
                .map(|source| serde_json::from_value(serde_json::Value::String(source)))
                .transpose()?,
            bound_at: row.get(13)?,
            input_published: row.get(14)?,
            cwd: row.get::<_, String>(15)?.into(),
            skill: row.get(16)?,
            provider: row.get(17)?,
            model: row.get(18)?,
            node: row.get(19)?,
            iterations: row
                .get::<_, Option<String>>(20)?
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
    let canonical = loopflow_model::id::canonical_session(id);
    let id = canonical.as_str();
    conn.query_row(
        &format!("{SESSION_SELECT} WHERE s.id=?1"),
        [id],
        read_session,
    )
    .optional()?
    .transpose()
}

/// Activity row `act` counts toward Session `session`'s availability: the
/// Session is open and the reading is its current agent's, which has not ended.
/// The quiet deadlines join on this too, so the rule and the time its answer
/// next changes read the same rows.
fn current_agent_activity(session: &str) -> String {
    format!(
        "act.session_id={session}.id AND {session}.completed_at IS NULL
            AND act.agent_process_id IS {session}.agent_process_id
            AND {session}.agent_ended_at IS NULL"
    )
}

/// Reported status wins for the current AgentProcess; otherwise use the
/// current attachment's input/hand-back/quiet reading. Filter before pagination.
fn available_sql(session: &str, now: i64) -> String {
    format!(
        "EXISTS(SELECT 1 FROM session_activity act
            WHERE {current}
            AND CASE WHEN act.program_status IS NOT NULL THEN
                EXISTS(SELECT 1 FROM json_each(act.program_status,'$.records') r
                    WHERE json_extract(r.value,'$.state')='blocked'
                    OR ({session}.interactive=1 AND (json_extract(r.value,'$.state')='idle'
                        OR (json_extract(r.value,'$.state')='done' AND NOT EXISTS(
                            SELECT 1 FROM json_each(act.program_status,'$.records') other
                            WHERE json_extract(other.value,'$.state')='working')))))
            ELSE act.attachment_token={session}.attachment_token
                AND (act.pending_input>0 OR (act.open_tools=0 AND (({session}.interactive=1 AND act.yielded=1)
                    OR {now}-act.observed_at>={quiet}))) END)",
        current = current_agent_activity(session),
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
    let mut sql = format!("{select} WHERE s.completed_at IS NULL");
    let mut values = Vec::new();
    let mut bind = |value| {
        values.push(value);
        format!("?{}", values.len())
    };
    if !filter.headless {
        sql.push_str(" AND s.interactive=1");
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
        let value = bind(Value::Text(task.clone()));
        if TaskId::parse(task).is_ok() {
            sql.push_str(&format!(
                " AND {}",
                super::task_work::session_in_task("s", &value)
            ));
        } else {
            sql.push_str(&format!(
                " AND {} AND s.task_id IN (SELECT id FROM tasks WHERE id={value} OR issue_identifier={value} OR external_issue_id={value})",
                super::task_work::counts_for_task("s")
            ));
        }
    }
    if let Some(wave) = &filter.wave {
        let wave = bind(Value::Text(wave.to_string()));
        sql.push_str(&format!(
            " AND (s.wave_id={wave} OR ({} AND s.task_id IN (
                SELECT t.id FROM projects p JOIN tasks t ON t.project_id=p.id WHERE p.wave_id={wave})))",
            super::task_work::counts_for_task("s")
        ));
    }
    if let Some(primary) = filter.primary {
        use crate::session::SessionPrimary;
        let scope = primary_scope_sql("s");
        sql.push_str(&match primary {
            SessionPrimary::All => format!(" AND {scope} IS NOT NULL"),
            SessionPrimary::Task => format!(" AND {scope}='task'"),
            SessionPrimary::Wave => " AND s.primary_scope='wave'".into(),
            SessionPrimary::Repository => " AND s.primary_scope='repository'".into(),
        });
    }
    if filter.orphan {
        sql.push_str(
            " AND s.wave_id IS NULL AND (s.task_id IS NULL OR s.primary_scope IS NOT NULL)",
        );
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
        sql.push_str(&format!(" ORDER BY s.title, s.id LIMIT {limit}"));
    }
    Ok((sql, values))
}

// Preserve the existing filters/order. Materialize only the selected metadata
// before joining Graph/Work labels; no request or historical payload is selected.
const SUMMARY_SELECT: &str = concat!(
    "SELECT s.id,c.receipt_key AS artifact_key,s.title,s.title_source,
    s.completed_at,s.interactive,s.task_id,s.wave_id,
    ",
    session_graph!(),
    " AS graph_lf_process_id,s.cwd,s.skill,s.provider,s.model,",
    session_step!("node"),
    " AS node,",
    session_step!("iterations"),
    " AS iterations,s.current_capture
    FROM agent_sessions s LEFT JOIN session_events c ON c.seq=s.current_capture"
);

fn summary_query(page: &str, by_id: bool, now: i64) -> String {
    let available = available_sql("a", now);
    let order = if by_id { "s.id" } else { "s.title,s.id" };
    // A Graph is the Graph process above the step that captured the current input.
    format!("WITH page AS MATERIALIZED ({page})
        SELECT s.id,s.artifact_key,s.title,s.title_source,s.completed_at,s.interactive,s.task_id,s.wave_id,s.graph_lf_process_id,s.cwd,s.skill,s.provider,s.model,s.node,s.iterations,s.current_capture,graph.lf_process_id,graph.graph,graph.started_at,graph.ended_at,COALESCE(gs.started_at,graph.started_at),
        (gs.seq=(SELECT MAX(later.seq) FROM graph_process_steps later WHERE later.graph_lf_process_id=gs.graph_lf_process_id)),
        w.slug,
        COALESCE(captured.independent=1,0),
        {scope},
        {available},
        COALESCE(({task_state}) IN ('done','abandoned'),0),
        (SELECT act.program_status FROM session_activity act WHERE act.session_id=s.id AND act.agent_process_id IS a.agent_process_id),a.agent_process_id,CASE WHEN a.attached_lf_process_id IS NOT NULL THEN a.attached_terminal_id END,
        (SELECT act.preview FROM session_activity act WHERE act.session_id=s.id AND act.agent_process_id IS a.agent_process_id
         AND json_extract(act.preview,'$.provider_thread')=a.provider_thread
         AND act.attachment_token=a.attachment_token AND a.attached_lf_process_id IS NOT NULL AND a.completed_at IS NULL),
        COALESCE((SELECT t.viewed IS NOT 1 FROM held_terminals t WHERE t.id=a.held_terminal_id AND t.ended_at IS NULL),0),
        (SELECT pin.account_id FROM provider_session_accounts pin WHERE pin.provider=s.provider AND pin.provider_session_id=a.provider_thread),
        COALESCE(captured.provider_thread IS NOT NULL,0),
        EXISTS(SELECT 1 FROM held_terminals t WHERE t.id=a.held_terminal_id AND t.ended_at IS NULL),
        graph.pid,graph.os_started_at,graph.exit_code,graph.error,a.agent_ended_at IS NOT NULL,
        a.agent_ended_at,a.agent_exit_code,
        a.starts_since_turn,a.last_started_at,a.start_failure,
        EXISTS(SELECT 1 FROM task_checkouts pc WHERE pc.task_id=t.id AND {here}),
        a.agent_pid IS NULL AND a.agent_ended_at IS NULL
         AND EXISTS(SELECT 1 FROM held_terminals t WHERE t.id=a.held_terminal_id AND t.ended_at IS NULL)
        FROM page s JOIN agent_sessions a ON a.id=s.id
        LEFT JOIN session_events captured ON captured.seq=s.current_capture
        LEFT JOIN graph_process_steps gs ON gs.lf_process_id=captured.lf_process_id
        LEFT JOIN graph_processes graph ON graph.lf_process_id=gs.graph_lf_process_id
        LEFT JOIN wave_addresses w ON w.id=s.wave_id
        LEFT JOIN tasks t ON t.id=s.task_id
        ORDER BY {order}",
        scope = primary_scope_sql("a"),
        task_state = super::durable::task_state_sql("t"),
        here = super::session_recovery::task_conversation_here_sql("a", "t", "pc"))
}

fn read_summary(
    conn: &Connection,
    labels: &mut std::collections::HashMap<TaskId, Option<String>>,
    row: &rusqlite::Row<'_>,
    projection: &SessionProjection,
) -> rusqlite::Result<StoreResult<crate::session::SessionSummary>> {
    Ok((|| {
        let task_id = row
            .get::<_, Option<String>>(6)?
            .map(|id| TaskId::parse(&id))
            .transpose()
            .map_err(invalid)?;
        let wave_id = row
            .get::<_, Option<String>>(7)?
            .map(|id| loopflow_model::id::WaveId::parse(&id))
            .transpose()
            .map_err(invalid)?;
        let graph = match row.get::<_, Option<loopflow_model::id::LfProcessId>>(16)? {
            Some(id) => {
                let run = crate::graph_process::Execution {
                    id,
                    started_at: row.get::<_, Option<i64>>(18)?.unwrap_or_default(),
                    ended_at: row.get(19)?,
                    pid: row.get(35)?,
                    os_started_at: row.get(36)?,
                    exit_code: row.get(37)?,
                    error: row.get(38)?,
                };
                Some(crate::session::GraphProcessSummary {
                    id: run.id.to_string(),
                    name: row.get(17)?,
                    state: crate::session::GraphProcessSummaryState::of_run(
                        projection.graph_result(&run),
                    ),
                    task_id: task_id.clone(),
                    wave_id: wave_id.clone(),
                    updated_at: match run.ended_at {
                        Some(ended) => ended,
                        None => row.get::<_, Option<i64>>(20)?.unwrap_or(run.started_at),
                    },
                })
            }
            None => None,
        };
        let agent_process_id: Option<loopflow_model::id::AgentProcessId> = row.get(28)?;
        // An agent whose end is recorded is dead by its row; the OS is asked about the rest.
        let agent_process_evidence = if row.get(39)? {
            crate::ProcessIdentityEvidence::Dead
        } else {
            projection.evidence(agent_process_id.as_ref())
        };
        let live = agent_process_evidence == crate::ProcessIdentityEvidence::Live;
        let unviewed: bool = row.get(31)?;
        let task_identifier = task_id
            .as_ref()
            .map(|id| super::children::cached_short_task_id(conn, labels, id))
            .transpose()?
            .flatten();
        Ok(crate::session::SessionSummary {
            transition: super::provider_transitions::latest_in(conn, &row.get::<_, String>(0)?)?,
            has_provider_history: row.get(33)?,
            preview: row
                .get::<_, Option<String>>(30)?
                .map(|json| serde_json::from_str(&json))
                .transpose()?
                .filter(|_| live),
            // A terminal no viewer occupies is gone, whatever its launch recorded.
            terminal_id: row.get::<_, Option<String>>(29)?.filter(|_| {
                live && !unviewed
                    && projection.holder(agent_process_id.as_ref())
                        != crate::ProcessIdentityEvidence::Dead
            }),
            unviewed,
            held: live && row.get(34)?,
            agent_end: row
                .get::<_, Option<i64>>(40)?
                .map(|at| {
                    row.get(41)
                        .map(|exit_code| crate::session::AgentEnd { at, exit_code })
                })
                .transpose()?,
            starts_since_turn: row.get(42)?,
            last_started_at: row.get(43)?,
            start_failure: row.get(44)?,
            primary_scope: row.get(24)?,
            agent_process_evidence,
            available: live && row.get(25)?,
            program_status: row
                .get::<_, Option<String>>(27)?
                .map(|json| serde_json::from_str(&json))
                .transpose()?
                .filter(|_| live),
            agent_process_id,
            task_terminal: row.get(26)?,
            in_task_checkout: row.get(45)?,
            launching: row.get(46)?,
            captured: row.get(15)?,
            id: row.get(0)?,
            artifact_key: row
                .get::<_, Option<String>>(1)?
                .map(|key| crate::session_record::parse_artifact_key(&key))
                .transpose()
                .map_err(invalid)?,
            title: row.get(2)?,
            title_source: serde_json::from_value(serde_json::Value::String(row.get(3)?))?,
            completed_at: row.get(4)?,
            interactive: row.get(5)?,
            task_id,
            wave_id,
            graph_lf_process_id: row.get(8)?,
            cwd: row.get::<_, String>(9)?.into(),
            skill: row.get(10)?,
            provider: row.get(11)?,
            account: row
                .get::<_, Option<String>>(32)?
                .map(|id| crate::ProviderAccountId::parse(&id))
                .transpose()
                .map_err(invalid)?,
            model: row.get(12)?,
            node: row.get(13)?,
            iterations: row
                .get::<_, Option<String>>(14)?
                .map(|raw| serde_json::from_str(&raw))
                .transpose()?,
            graph,
            graph_step_latest: row.get::<_, Option<bool>>(21)?.unwrap_or(false),
            wave_name: row.get(22)?,
            task_identifier,
            independent: row.get::<_, Option<bool>>(23)?.unwrap_or(false),
        })
    })())
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
        PrimaryScope::Task(task) => conn
            .query_row(
                &format!(
                    "{SESSION_SELECT} WHERE s.id=(SELECT session_id FROM task_primary_sessions WHERE task_id=?1)
                     AND s.completed_at IS NULL"
                ),
                [task.as_str()],
                read_session,
            )
            .optional()?
            .transpose(),
    }
}

pub(super) fn claim_session_input_in(
    tx: &Transaction<'_>,
    mut next: LfSession,
    expected: Option<&crate::process::SessionAttachment>,
    process: &LfProcessId,
    start: Option<&crate::process::AttachingLf>,
) -> StoreResult<(LfSession, crate::process::SessionAttachment)> {
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
    let attachment = super::processes::attach_in(tx, &next.id, expected, process, start)?;
    let next = session_in(tx, &next.id)?.ok_or(StoreError::NotFound)?;
    Ok((next, attachment))
}

pub(super) fn session_summaries_in(
    conn: &Connection,
    filter: &crate::session::SessionFilter,
    projection: &SessionProjection,
) -> StoreResult<Vec<crate::session::SessionSummary>> {
    let now = projection.now;
    let live_agents = &projection.live_agents;
    let mut labels = std::collections::HashMap::new();
    let (page, values) = inventory_query(filter, SUMMARY_SELECT, now, live_agents)?;
    let mut query = conn.prepare(&summary_query(&page, filter.after.is_some(), now))?;
    let rows = query.query_map(rusqlite::params_from_iter(values), |row| {
        read_summary(conn, &mut labels, row, projection)
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
            params![current.id, crate::rows::now_unix()],
        )?;
    }
    Ok(current)
}

/// A Task's primary when it has one, else `session`: chosen when it is
/// already one of the Task's conversations, admitted as new otherwise. A
/// conversation that began before its checkout became the Task's takes the
/// Task now, by the rule a new Session would.
pub(super) fn admit_task_primary_in(
    conn: &Connection,
    task: &TaskId,
    session: &LfSession,
    caller_process: Option<&loopflow_model::id::LfProcessId>,
) -> StoreResult<LfSession> {
    let scope = PrimaryScope::Task(task.clone());
    if primary_session_in(conn, &scope)?.is_none() {
        if let Some(current) = session_in(conn, session.id.as_str())? {
            if current.task_id.is_none()
                && super::task_work::starting_task_in(conn, None, Some(&current.cwd), None)?
                    .as_ref()
                    == Some(task)
            {
                bind_session_in(conn, &current, task)?;
            }
            choose_task_primary_in(conn, task, session.id.as_str())?;
        }
    }
    ensure_primary_session_in(conn, &scope, session.clone(), caller_process, None)
}

fn ensure_primary_session_in(
    conn: &Connection,
    scope: &PrimaryScope,
    session: LfSession,
    caller_process: Option<&loopflow_model::id::LfProcessId>,
    person_start_at: Option<i64>,
) -> StoreResult<LfSession> {
    if let Some(current) = primary_session_in(conn, scope)? {
        return Ok(current);
    }
    let session = reserve_session_in(conn, session, caller_process)?;
    if let Some(at) = person_start_at {
        conn.execute(
            "UPDATE agent_sessions SET last_started_at=?2 WHERE id=?1",
            params![&session.id, at],
        )?;
    }
    let marked = match scope {
        PrimaryScope::Repository(_) => conn.execute(
            "UPDATE agent_sessions SET primary_scope='repository' WHERE id=?1",
            [&session.id],
        )?,
        PrimaryScope::Wave(_) => conn.execute(
            "UPDATE agent_sessions SET primary_scope='wave' WHERE id=?1",
            [&session.id],
        )?,
        PrimaryScope::Task(task) => mark_task_primary_in(conn, task, &session.id, "")?,
    };
    if marked != 1 {
        return Err(StoreError::InvalidData(format!(
            "Session {} did not become its scope's primary",
            session.id
        )));
    }
    Ok(session)
}

#[derive(Clone, Copy, Debug)]
pub enum HistorySelection<'a> {
    All,
    Local,
    Input(&'a str),
}

/// The inputs a history read selected, before their history is decoded.
#[derive(Debug)]
pub struct PendingHistory {
    /// Wave, Project and resolved Task the read is scoped to.
    scope: (Option<String>, Option<String>, Option<String>),
    pub inputs: Vec<PendingInput>,
    pub truncated: bool,
}

/// One selected input: a captured input, or a native turn no input captured.
#[derive(Debug, Clone)]
pub struct PendingInput {
    /// The Machine whose copy holds this input. A copied native turn keeps
    /// its row from this snapshot until the projection needs it.
    copy: Option<(loopflow_model::durable::MachineId, Option<String>)>,
    pub session_id: String,
    input: Option<String>,
    caller: Option<String>,
    started: i64,
    task: Option<String>,
    wave: Option<String>,
    names: (Option<String>, Option<String>),
    current_input: Option<i64>,
    cwd: String,
    repo: Option<String>,
    skill: Option<String>,
    provider: Option<String>,
    model: Option<String>,
    interactive: bool,
    truncated: bool,
    work_source: Option<String>,
    captured: Option<i64>,
    thread: Option<AgentSessionId>,
    turn: Option<String>,
    skill_version: Option<String>,
    /// The latest `observed_at` of any of its history rows. Every time its
    /// decoded history reports is a row's `observed_at` or a payload time its
    /// writer also stored as that row's `observed_at`, so none is later.
    pub latest: i64,
}

impl PendingInput {
    pub fn origin(&self) -> Option<&loopflow_model::durable::MachineId> {
        self.copy.as_ref().map(|(origin, _)| origin)
    }
}

/// Saved evidence for one selected input, before application projection.
#[derive(Debug)]
pub struct HistoryInput {
    pub session: crate::session::HistoryCapture,
    pub artifact_key: Option<String>,
    pub events: Vec<crate::session::SessionEvent>,
    pub turns: Vec<crate::session::Turn>,
    pub names: (Option<String>, Option<String>),
}

/// The caller reads and validates this Session inside the same transaction.
pub(super) fn bind_session_in(
    conn: &Connection,
    session: &LfSession,
    task: &TaskId,
) -> StoreResult<()> {
    let id = session.id.as_str();
    if let Some(bound) = &session.task_id {
        if bound != task {
            let issue = super::children::short_task_id(conn, bound)?.ok_or(StoreError::NotFound)?;
            return Err(StoreError::InvalidAuthority(format!(
                "Session {id} already has Task {issue}; binding is permanent"
            )));
        }
    } else {
        let wave = super::durable::task_wave_in(conn, task)?;
        if session
            .wave_id
            .as_ref()
            .is_some_and(|existing| Some(existing) != wave.as_ref())
        {
            return Err(StoreError::InvalidAuthority(format!(
                "Session {id} belongs to another Wave"
            )));
        }
        conn.execute(
                "UPDATE agent_sessions SET task_id=?2,wave_id=?3,work_source='bound',bound_at=?4 WHERE id=?1",
                params![id, task.as_str(), wave.as_ref().map(loopflow_model::id::WaveId::as_str), crate::rows::now_unix()],
            )?;
    }
    repair_session_title_in(conn, id, None)
}

pub(super) fn choose_task_primary_in(
    conn: &Connection,
    task: &TaskId,
    session: &str,
) -> StoreResult<()> {
    let chosen = mark_task_primary_in(
        conn,
        task,
        session,
        "AND interactive=1 AND completed_at IS NULL",
    )?;
    if chosen != 1 {
        return Err(StoreError::InvalidAuthority(format!(
            "Session {session} is not an unfinished interactive conversation of this Task"
        )));
    }
    Ok(())
}

/// Move the Task's primary mark to `session`, one of its conversations that
/// meets `condition`. Answers how many Sessions took the mark.
fn mark_task_primary_in(
    conn: &Connection,
    task: &TaskId,
    session: &str,
    condition: &str,
) -> StoreResult<usize> {
    // REPLACE also clears a row an older lf left naming this Session for
    // another Task.
    Ok(conn.execute(
        &format!(
            "INSERT OR REPLACE INTO task_primary_sessions(task_id,session_id)
             SELECT ?1,?2 WHERE EXISTS(SELECT 1 FROM agent_sessions
                 WHERE id=?2 AND task_id=?1 AND {} {condition})",
            super::task_work::counts_for_task("agent_sessions")
        ),
        params![task.as_str(), session],
    )?)
}

impl SqliteStore {
    /// Record that `terminal` hosts the Session's agent. A host reopens a
    /// retired conversation; its next exit retires it again.
    pub fn bind_session_terminal(
        &self,
        session: &str,
        attachment: &crate::process::SessionAttachment,
        terminal: &TerminalId,
    ) -> StoreResult<()> {
        let canonical = loopflow_model::id::canonical_session(session);
        let session = canonical.as_str();
        self.with_session_attachment(session, attachment, || {
            let conn = self.conn.lock().expect("store mutex poisoned");
            conn.execute(
                "UPDATE agent_sessions SET transport_lf_process_id=NULL,held_terminal_id=?2,completed_at=NULL WHERE id=?1",
                params![session, terminal],
            )?;
            Ok(())
        })
    }

    /// The running held terminal that hosts the Session's running agent.
    pub fn session_terminal(&self, session: &str) -> StoreResult<Option<TerminalId>> {
        let canonical = loopflow_model::id::canonical_session(session);
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT t.id FROM agent_sessions s
                 JOIN held_terminals t ON t.id=s.held_terminal_id
                 WHERE s.id=?1 AND s.agent_started_at IS NOT NULL AND s.agent_ended_at IS NULL
                 AND t.ended_at IS NULL",
                [canonical.as_str()],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// The Session a held terminal runs, for its viewers.
    pub fn terminal_session(&self, terminal: &TerminalId) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT id FROM agent_sessions WHERE held_terminal_id=?1",
                [terminal],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// The Session lf process `process` ran an agent for.
    pub fn process_session(&self, process: &LfProcessId) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT id FROM agent_sessions WHERE attached_lf_process_id=?1
                 UNION ALL SELECT session_id FROM session_events WHERE lf_process_id=?1
                 UNION ALL SELECT session_id FROM history.session_history WHERE lf_process_id=?1
                 UNION ALL SELECT session_id FROM history.turns WHERE lf_process_id=?1
                 LIMIT 1",
                [process],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// What keeps held terminal `terminal` running. None means its standalone
    /// interactive Sessions hold no work; finished Graphs hold nothing.
    pub fn terminal_retention(
        &self,
        terminal: &TerminalId,
    ) -> StoreResult<Option<TerminalRetention>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // A running Graph started inside the terminal, or a Session's Graph
        // shown in it, holds its steps there.
        Ok(conn.query_row(
            &format!(
                "SELECT EXISTS(SELECT 1 FROM graph_processes g INDEXED BY graph_processes_unended
                         WHERE g.held_terminal_id=?1 AND g.ended_at IS NULL)
                     OR EXISTS(SELECT 1 FROM agent_sessions s
                         JOIN graph_processes g ON g.lf_process_id={SESSION_GRAPH}
                         WHERE s.held_terminal_id=?1 AND g.ended_at IS NULL)
                     AS running_graph,
                 NOT EXISTS(SELECT 1 FROM agent_sessions s
                     WHERE s.held_terminal_id=?1 AND s.completed_at IS NULL)
                     AS no_open_session,
                 EXISTS(SELECT 1 FROM agent_sessions s
                     WHERE s.held_terminal_id=?1 AND s.interactive=0)
                     AS headless_run,
                 EXISTS(SELECT 1 FROM agent_sessions s
                     WHERE s.held_terminal_id=?1
                         AND s.completed_at IS NULL AND {TURN_LIVE})
                     AS live_turn"
            ),
            [terminal],
            |row| {
                Ok(if row.get("running_graph")? {
                    Some(TerminalRetention::RunningGraph)
                } else if row.get("no_open_session")? {
                    Some(TerminalRetention::NoOpenSession)
                } else if row.get("headless_run")? {
                    Some(TerminalRetention::HeadlessRun)
                } else if row.get("live_turn")? {
                    Some(TerminalRetention::LiveTurn)
                } else {
                    None
                })
            },
        )?)
    }

    /// A Task's unfinished interactive conversations, each with its last opening.
    pub fn task_conversations(&self, task: &TaskId) -> StoreResult<Vec<(LfSession, Option<i64>)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "{SESSION_SELECT} WHERE s.interactive=1 AND s.completed_at IS NULL AND {}",
            super::task_work::session_in_task("s", "?1")
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
    pub fn checkout_conversation(&self, root: &Path) -> StoreResult<Option<LfSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let root = root.to_string_lossy();
        conn.query_row(
            &format!(
                "{SESSION_SELECT}
                WHERE s.interactive=1 AND s.completed_at IS NULL
                AND (s.cwd=?1 OR substr(s.cwd,1,length(?1)+1)=?1||'/')
                ORDER BY COALESCE(s.opened_at, s.created_at*1000) DESC, s.id DESC LIMIT 1"
            ),
            [root.as_ref()],
            read_session,
        )
        .optional()?
        .transpose()
    }

    /// A person opened the interactive conversation `session` at `at_ms`.
    pub fn record_session_opened(&self, session: &str, at_ms: i64) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE agent_sessions SET opened_at=?2 WHERE id=?1 AND interactive=1",
            params![session, at_ms],
        )?;
        Ok(())
    }

    /// The Task's primary conversation, while it is unfinished.
    pub fn task_primary(&self, task: &TaskId) -> StoreResult<Option<LfSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        primary_session_in(&conn, &PrimaryScope::Task(task.clone()))
    }

    /// Name one of the Task's own unfinished interactive conversations its
    /// primary. The earlier choice stays an ordinary conversation of the Task.
    pub fn choose_task_primary(&self, task: &TaskId, session: &str) -> StoreResult<LfSession> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        choose_task_primary_in(&conn, task, session)?;
        primary_session_in(&conn, &PrimaryScope::Task(task.clone()))?
            .ok_or_else(|| invalid(format!("Session {session} disappeared while being chosen")))
    }

    /// Each row's Available is judged at `now`.
    pub fn projected_session_summaries(
        &self,
        filter: &crate::session::SessionFilter,
        inputs: &SessionProjectionInputs,
        projection: &SessionProjection,
    ) -> StoreResult<Option<Vec<crate::session::SessionSummary>>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let Some(snapshot) = inputs.pin(&mut conn)? else {
            return Ok(None);
        };
        Ok(Some(session_summaries_in(&snapshot, filter, projection)?))
    }

    /// Whether `session`'s current AgentProcess has reported what it is doing:
    /// a hook, its live stream or its terminal's status.
    pub fn session_reports_status(&self, session: &str) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM session_activity act
                JOIN agent_sessions s ON s.id=act.session_id
                WHERE s.id=?1 AND act.agent_process_id IS s.agent_process_id)",
            [session],
            |row| row.get(0),
        )?)
    }

    pub fn projected_session_summary(
        &self,
        id: &str,
        inputs: &SessionProjectionInputs,
        projection: &SessionProjection,
    ) -> StoreResult<Option<Option<crate::session::SessionSummary>>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let Some(conn) = inputs.pin(&mut conn)? else {
            return Ok(None);
        };
        let mut labels = std::collections::HashMap::new();
        Ok(Some(
            conn.query_row(
                &summary_query(
                    &format!("{SUMMARY_SELECT} WHERE s.id=?1"),
                    false,
                    projection.now,
                ),
                [id],
                |row| read_summary(&conn, &mut labels, row, projection),
            )
            .optional()?
            .transpose()?,
        ))
    }

    /// Resolve a conversation or captured-input selector from its SQLite owner.
    pub fn resolve_history_input(&self, selector: &str) -> StoreResult<String> {
        let canonical = loopflow_model::id::canonical_session(selector);
        let conn = self.conn.lock().expect("store mutex poisoned");
        if conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE id=?1)",
            [&canonical],
            |row| row.get::<_, bool>(0),
        )? {
            return Ok(canonical);
        }
        // A prefix is the range [prefix, prefix+U+10FFFF) in BINARY order, so
        // each candidate is searched in `session_capture_key` or
        // `session_capture_caller` rather than read from every capture.
        let caller = "CASE WHEN json_valid(payload) THEN json_extract(payload,'$.caller_key') END";
        let mut query = conn.prepare(&format!("
            SELECT receipt_key AS id FROM session_events WHERE kind='captured' AND receipt_key>=?1 AND receipt_key<?2
            UNION SELECT receipt_key FROM session_events WHERE kind='captured' AND receipt_key>=?3 AND receipt_key<?4
            UNION SELECT {caller} FROM session_events WHERE kind='captured' AND {caller}>=?1 AND {caller}<?2
            UNION SELECT {caller} FROM session_events WHERE kind='captured' AND {caller}>=?3 AND {caller}<?4
            ORDER BY id LIMIT 2"))?;
        let run = format!("run_{selector}");
        let ids = query
            .query_map(
                params![
                    selector,
                    format!("{selector}\u{10FFFF}"),
                    run,
                    format!("{run}\u{10FFFF}")
                ],
                |row| row.get::<_, String>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        match ids.as_slice() {
            [id] => Ok(id.clone()),
            [] => Err(StoreError::NotFound),
            _ => Err(invalid(format!("Input selector {selector:?} is ambiguous"))),
        }
    }

    /// Sessions that recorded `thread` as their provider's own conversation id.
    pub fn sessions_for_agent_session(&self, thread: &AgentSessionId) -> StoreResult<Vec<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // An input's captured row names the conversation that resumes it.
        let mut query = conn.prepare(
            "SELECT id FROM agent_sessions WHERE provider_thread=?1
             UNION SELECT session_id FROM session_events WHERE kind='captured' AND provider_thread=?1
             ORDER BY 1",
        )?;
        let ids = query
            .query_map([thread], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        Ok(ids)
    }

    /// The inputs a conversation history read selects, newest first, before
    /// any history is decoded.
    pub fn pending_conversation_history(
        &self,
        wave: Option<&str>,
        project: Option<&str>,
        task: Option<&str>,
        since: i64,
        include_finished: bool,
    ) -> StoreResult<PendingHistory> {
        self.pending_inputs(
            wave,
            project,
            task,
            None,
            (since, include_finished, None),
            HistorySelection::All,
        )
    }

    pub fn pending_inputs(
        &self,
        wave: Option<&str>,
        project: Option<&str>,
        task: Option<&str>,
        caller: Option<&str>,
        window: (i64, bool, Option<usize>),
        selection: HistorySelection<'_>,
    ) -> StoreResult<PendingHistory> {
        let input = match selection {
            HistorySelection::Input(input) => Some(input),
            _ => None,
        };
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
            // `windowed` holds every capture that could start or end in the
            // window, read by range from the window indexes; the exact filters
            // below decide. A Task drill's window is the Task: its arm reads every
            // row naming the resolved Task id (capture, manifest, turn origin) and turns the time arms off. With no lower bound the
            // capture and turn arms already read everything, so the others stay
            // off. Each arm is one source of `started` or `ended`: a new source
            // below needs its own arm here, or inputs it places in the window
            // silently drop out of every read.
            // Partial index conditions repeat verbatim so the planner can use
            // them, and `CROSS JOIN` keeps the window as the outer loop. A `+`
            // on an arm's output stops SQLite merging the UNION in index order,
            // which it would get by scanning the whole index instead of the range.
            let mut query = conn.prepare(&format!("WITH windowed(seq) AS (
                SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=?7
                UNION SELECT seq FROM session_events WHERE ?7 IS NULL AND task_id=?3 AND kind='captured'
                UNION SELECT captured_event FROM history.session_history
                    WHERE ?7 IS NULL AND task_id=?3
                UNION SELECT captured_event FROM history.turns WHERE ?7 IS NULL AND task_id=?3
                UNION SELECT +seq FROM session_events WHERE ?7 IS NULL AND ?3 IS NULL AND kind='captured' AND observed_at>=?5
                UNION SELECT +o.captured_event FROM history.session_history o
                    WHERE ?7 IS NULL AND ?3 IS NULL AND ?5>0 AND o.kind IN ('manifest','terminal')
                    AND o.observed_at>=?5 AND (?6 OR o.kind='manifest')
                UNION SELECT +captured_event FROM history.turns
                    WHERE ?7 IS NULL AND ?3 IS NULL AND ?5>0 AND ?6 AND ended_at>=?5 AND +captured_event IS NOT NULL
                UNION SELECT +ran.seq FROM agent_sessions exited INDEXED BY agent_sessions_agent_ended
                    CROSS JOIN session_events ran ON ran.lf_process_id=exited.agent_launcher_lf_process_id
                    WHERE ?7 IS NULL AND ?3 IS NULL AND ?5>0 AND ?6 AND exited.agent_ended_at>=?5 AND ran.kind='captured'
                UNION SELECT +ran.captured_event FROM agent_sessions exited INDEXED BY agent_sessions_agent_ended
                    CROSS JOIN history.turns ran ON ran.lf_process_id=exited.agent_launcher_lf_process_id
                    WHERE ?7 IS NULL AND ?3 IS NULL AND ?5>0 AND ?6 AND exited.agent_ended_at>=?5),
            -- Turns no input started: one that started or ended in the window,
            -- or whose launcher's agent ended in it, though it started earlier.
            turns(id) AS (
                SELECT id FROM history.turns
                    WHERE ?7 IS NULL AND ?3 IS NULL AND started_at>=?5 AND +captured_event IS NULL
                UNION SELECT id FROM history.turns
                    WHERE ?7 IS NULL AND ?3 IS NULL AND ended_at>=?5 AND +captured_event IS NULL
                UNION SELECT +ran.id FROM agent_sessions exited INDEXED BY agent_sessions_agent_ended
                    CROSS JOIN history.turns ran ON ran.lf_process_id=exited.agent_launcher_lf_process_id
                    WHERE ?7 IS NULL AND ?3 IS NULL AND ?5>0 AND ?6 AND exited.agent_ended_at>=?5 AND ran.captured_event IS NULL
                UNION SELECT id FROM history.turns
                    WHERE ?7 IS NULL AND task_id=?3 AND captured_event IS NULL),
            copies AS NOT MATERIALIZED ({}),
            -- Copied rows that start or end an input in the window, name the Task, or are the input.
            copied_window(record_id) AS (
                SELECT record_id FROM machine_record_inputs WHERE ?13 AND ?4 IS NULL AND ?3 IS NULL AND started>=?5
                UNION SELECT rowid FROM machine_records INDEXED BY machine_record_turn_end WHERE kind='turn' AND ?13 AND ?4 IS NULL AND ?3 IS NULL AND ?6
                    AND ?5>0 AND json_extract(row,'$.ended_at')>=?5
                UNION SELECT record_id FROM machine_record_inputs WHERE ?13 AND ?4 IS NULL AND task_id=?3
                UNION SELECT rowid FROM machine_records INDEXED BY machine_record_capture_key
                    WHERE kind='session_event' AND json_extract(row,'$.kind')='captured' AND json_extract(row,'$.receipt_key')=?7),
            -- A copied capture, found from itself, its manifest or its turns.
            -- The local input keeps its copies out; among copies the lowest origin wins.
            copied_captures AS MATERIALIZED (
                SELECT cap.rowid AS record_id,cap.origin_machine_id AS origin,
                    json_extract(cap.row,'$.session_id') AS session_id,json_extract(cap.row,'$.seq') AS seq,
                    json_extract(cap.row,'$.receipt_key') AS input_id,cap.row
                FROM (SELECT DISTINCT r.origin_machine_id AS origin,
                        CAST(CASE r.kind WHEN 'session_event' THEN json_extract(r.row,'$.seq')
                            ELSE json_extract(r.row,'$.captured_event') END AS TEXT) AS key
                    FROM copied_window w CROSS JOIN machine_records r ON r.rowid=w.record_id) k
                CROSS JOIN machine_records cap ON cap.origin_machine_id=k.origin AND cap.kind='session_event' AND cap.key=k.key
                WHERE json_extract(cap.row,'$.kind')='captured'
                  AND NOT EXISTS (SELECT 1 FROM session_events l WHERE l.kind='captured' AND l.receipt_key=json_extract(cap.row,'$.receipt_key'))
                  AND cap.origin_machine_id=(SELECT o.origin_machine_id FROM machine_records o
                      WHERE o.kind='session_event' AND json_extract(o.row,'$.kind')='captured'
                        AND json_extract(o.row,'$.receipt_key')=json_extract(cap.row,'$.receipt_key')
                      ORDER BY o.origin_machine_id LIMIT 1)),
            -- Each copied input's turns and observations, read once.
            copied_turns AS MATERIALIZED (
                SELECT c.record_id,SUM(json_extract(t.row,'$.ended_at') IS NULL) AS open,
                    MAX(json_extract(t.row,'$.ended_at')) AS last_end,
                    MAX(MAX(coalesce(json_extract(t.row,'$.started_at'),0),coalesce(json_extract(t.row,'$.usage.observed_at'),0),
                        coalesce(json_extract(t.row,'$.ended_at'),0))) AS latest
                FROM copied_captures c CROSS JOIN machine_records t ON t.origin_machine_id=c.origin AND t.kind='turn'
                    AND json_extract(t.row,'$.session_id')=c.session_id
                WHERE json_extract(t.row,'$.captured_event')=c.seq GROUP BY c.record_id),
            copied_observations AS MATERIALIZED (
                SELECT c.record_id,MAX(json_extract(h.row,'$.observed_at')) AS latest
                FROM copied_captures c CROSS JOIN machine_records h ON h.origin_machine_id=c.origin AND h.kind='history'
                    AND json_extract(h.row,'$.session_id')=c.session_id
                WHERE json_extract(h.row,'$.captured_event')=c.seq GROUP BY c.record_id),
            -- The same reading as `inputs`, from the origin's copied Session, manifest,
            -- terminal receipt and turns.
            copied_inputs AS (
                SELECT c.origin,c.seq AS captured,c.input_id,c.session_id,
                    json_extract(json_extract(c.row,'$.payload'),'$.caller_key') AS caller_input_id,
                    coalesce(json_extract(m.row,'$.observed_at'),json_extract(c.row,'$.observed_at')) AS started,
                    CASE WHEN m.rowid IS NOT NULL THEN json_extract(m.row,'$.task_id') ELSE json_extract(c.row,'$.task_id') END AS task_id,
                    CASE WHEN m.rowid IS NOT NULL THEN json_extract(m.row,'$.wave_id') ELSE json_extract(c.row,'$.wave_id') END AS wave_id,
                    CASE WHEN json_extract(c.row,'$.lf_process_id') IS NOT NULL
                        AND (json_extract(s.row,'$.agent_launcher_lf_process_id') IS NOT json_extract(c.row,'$.lf_process_id')
                            OR json_extract(s.row,'$.agent_ended_at') IS NOT NULL)
                    THEN CASE WHEN json_extract(s.row,'$.agent_launcher_lf_process_id') IS json_extract(c.row,'$.lf_process_id')
                        THEN json_extract(s.row,'$.agent_ended_at') ELSE json_extract(s.row,'$.agent_started_at') END
                    WHEN t.open>0 THEN NULL
                    ELSE coalesce(json_extract(term.row,'$.observed_at'),t.last_end) END AS ended,
                    MAX(json_extract(c.row,'$.observed_at'),coalesce(h.latest,0),coalesce(t.latest,0)) AS latest,
                    c.record_id
                FROM copied_captures c
                LEFT JOIN copied_turns t ON t.record_id=c.record_id
                LEFT JOIN copied_observations h ON h.record_id=c.record_id
                LEFT JOIN machine_records s ON s.origin_machine_id=c.origin AND s.kind='session' AND s.key=c.session_id
                LEFT JOIN machine_records m ON m.origin_machine_id=c.origin AND m.kind='history'
                    AND m.key=json_array(c.session_id,c.input_id||':manifest.json')
                LEFT JOIN machine_records term ON term.origin_machine_id=c.origin AND term.kind='history'
                    AND term.key=json_array(c.session_id,c.input_id||':terminal.json')
                WHERE m.rowid IS NOT NULL OR t.record_id IS NOT NULL
                    OR (json_extract(s.row,'$.current_capture')=c.seq AND json_extract(s.row,'$.input_published')=1)),
            inputs AS (
                SELECT i.seq AS captured,i.receipt_key AS input_id,i.session_id,json_extract(i.payload,'$.caller_key') AS caller_input_id,
                    COALESCE(m.observed_at,i.observed_at) AS started,
                    CASE WHEN m.seq IS NOT NULL THEN m.task_id ELSE i.task_id END AS task_id,
                    CASE WHEN m.seq IS NOT NULL THEN m.wave_id ELSE i.wave_id END AS wave_id,
                    -- An input ends when its lf's agent does: the Session's
                    -- current launcher's at that agent's end, an earlier
                    -- launcher's by the time the next agent started.
                    CASE WHEN COALESCE(latest_turn.lf_process_id,i.lf_process_id) IS NOT NULL
                        AND (s.agent_launcher_lf_process_id IS NOT COALESCE(latest_turn.lf_process_id,i.lf_process_id) OR s.agent_ended_at IS NOT NULL)
                    THEN CASE WHEN s.agent_launcher_lf_process_id IS COALESCE(latest_turn.lf_process_id,i.lf_process_id)
                        THEN s.agent_ended_at ELSE s.agent_started_at END
                    WHEN terminal.seq IS NOT NULL AND NOT (
                        json_valid(terminal.payload) AND CASE WHEN json_valid(terminal.payload) THEN
                        COALESCE(json_type(terminal.payload,'$.evidence.outcome')='text',0)
                        AND unixepoch(json_extract(terminal.payload,'$.evidence.ended_at')) IS NOT NULL ELSE 0 END
                    ) THEN NULL WHEN EXISTS (
                        SELECT 1 FROM history.turns origin WHERE origin.session_id=s.id
                        AND origin.captured_event=i.seq AND origin.ended_at IS NULL
                        ) THEN NULL ELSE
                        COALESCE(terminal.observed_at,(
                            SELECT MAX(origin.ended_at) FROM history.turns origin
                            WHERE origin.session_id=s.id AND origin.captured_event=i.seq)) END AS ended, NULL AS thread, NULL AS turn,
                    MAX(i.observed_at,COALESCE(m.observed_at,0),
                        COALESCE((SELECT MAX(o.observed_at) FROM history.session_history o WHERE o.captured_event=i.seq),0),
                        COALESCE((SELECT MAX(MAX(COALESCE(origin.started_at,0),COALESCE(origin.usage_at,0),
                            COALESCE(origin.ended_at,0))) FROM history.turns origin
                            WHERE origin.session_id=s.id AND origin.captured_event=i.seq),0)) AS latest
                FROM windowed CROSS JOIN session_events i ON i.seq=windowed.seq JOIN agent_sessions s ON s.id=i.session_id
                LEFT JOIN history.turns latest_turn ON latest_turn.id=(
                    SELECT origin.id FROM history.turns origin WHERE origin.session_id=i.session_id
                    AND origin.captured_event=i.seq ORDER BY origin.id DESC LIMIT 1)
                LEFT JOIN history.session_history m ON m.session_id=s.id
                    AND m.receipt_key=i.receipt_key||':manifest.json'
                LEFT JOIN history.session_history terminal ON terminal.session_id=s.id
                    AND terminal.receipt_key=i.receipt_key||':terminal.json'
                WHERE i.kind='captured'
                    -- `eligible` keeps only these when finished inputs are excluded;
                    -- filtering here spares the per-input subqueries on older ones.
                    AND (?6 OR COALESCE(m.observed_at,i.observed_at)>=?5) AND
                    (?7 IS NOT NULL OR m.seq IS NOT NULL
                    OR (i.receipt_key=(SELECT receipt_key FROM session_events WHERE seq=s.current_capture) AND s.input_published=1)
                    OR EXISTS(SELECT 1 FROM history.turns origin WHERE origin.session_id=i.session_id
                        AND origin.captured_event=i.seq))
                UNION ALL
                SELECT NULL,NULL,t.session_id,NULL,COALESCE(t.started_at,t.usage_at,t.ended_at),t.task_id,t.wave_id,
                    COALESCE(CASE WHEN t.lf_process_id IS NULL THEN NULL
                        WHEN ts.agent_launcher_lf_process_id IS t.lf_process_id THEN ts.agent_ended_at
                        ELSE ts.agent_started_at END,t.ended_at),t.provider_thread,t.provider_turn,
                    MAX(COALESCE(t.started_at,0),COALESCE(t.usage_at,0),COALESCE(t.ended_at,0))
                FROM turns CROSS JOIN history.turns t ON t.id=turns.id
                LEFT JOIN agent_sessions ts ON ts.id=t.session_id
                WHERE ts.id IS NOT NULL AND t.captured_event IS NULL AND (?6 OR COALESCE(t.started_at,t.usage_at,t.ended_at)>=?5)
                    AND (NOT ?13 OR t.lf_process_id IS NOT NULL OR NOT EXISTS (SELECT 1 FROM copies c
                        WHERE c.key=json_array(t.session_id,t.provider_thread,t.provider_turn))))
                , scoped AS (
                SELECT captured,input_id,session_id,caller_input_id,started,task_id,wave_id,ended,thread,turn,latest,NULL AS copied_record
                FROM inputs
                WHERE (?1 IS NULL OR wave_id IN (SELECT id FROM wave_addresses WHERE id=?9 OR slug=?1)
                    OR EXISTS (SELECT 1 FROM history.turns origin WHERE origin.session_id=inputs.session_id
                        AND inputs.captured IS NOT NULL AND origin.captured_event=inputs.captured
                        AND origin.wave_id IN (SELECT id FROM wave_addresses WHERE id=?9 OR slug=?1)))
                AND (?2 IS NULL OR task_id IN (SELECT t.id FROM projects p CROSS JOIN tasks t ON t.project_id=p.id
                    WHERE p.id=?10 OR p.project_slug=?2 OR p.external_project_id=?11)
                    OR EXISTS (SELECT 1 FROM history.turns origin WHERE origin.session_id=inputs.session_id
                        AND inputs.captured IS NOT NULL AND origin.captured_event=inputs.captured
                        AND origin.task_id IN (SELECT t.id FROM projects p CROSS JOIN tasks t ON t.project_id=p.id
                            WHERE p.id=?10 OR p.project_slug=?2 OR p.external_project_id=?11)))
                AND (?3 IS NULL OR task_id=?3
                    OR EXISTS (SELECT 1 FROM history.turns origin WHERE origin.session_id=inputs.session_id
                        AND inputs.captured IS NOT NULL AND origin.captured_event=inputs.captured
                        AND origin.task_id=?3))
                AND (?4 IS NULL OR caller_input_id=?4 OR caller_input_id IN (SELECT receipt_key FROM session_events WHERE kind='captured' AND session_id=?12))
                AND (started>=?5 OR (?6 AND ended>=?5))
                UNION ALL
                SELECT NULL,NULL,c.session_id,NULL,c.started,c.task_id,c.wave_id,
                    json_extract(c.row,'$.ended_at'),json_extract(c.row,'$.thread'),json_extract(c.row,'$.turn'),
                    MAX(c.started,coalesce(json_extract(c.row,'$.ended_at'),0),
                        coalesce(json_extract(c.row,'$.usage.observed_at'),0)),c.record_id
                FROM copied_window w CROSS JOIN copies c ON c.record_id=w.record_id
                WHERE ?7 IS NULL AND ?4 IS NULL AND c.kind='turn' AND json_extract(c.row,'$.captured_event') IS NULL
                  AND (c.started>=?5 OR (?6 AND json_extract(c.row,'$.ended_at')>=?5))
                  AND (?1 IS NULL OR c.scope_wave IN (SELECT id FROM wave_addresses WHERE id=?9 OR slug=?1))
                  AND (?2 IS NULL OR c.project_id=?10 OR c.project_slug=?2 OR c.external_project_id=?11)
                UNION ALL
                SELECT captured,input_id,session_id,caller_input_id,started,task_id,wave_id,ended,NULL,NULL,latest,record_id
                FROM copied_inputs ci
                WHERE (started>=?5 OR (?6 AND ended>=?5))
                AND (?1 IS NULL OR wave_id IN (SELECT id FROM wave_addresses WHERE id=?9 OR slug=?1)
                    OR EXISTS (SELECT 1 FROM machine_record_inputs t WHERE t.origin_machine_id=ci.origin AND t.kind='turn'
                        AND t.session_id=ci.session_id AND json_extract(t.row,'$.captured_event')=ci.captured
                        AND t.scope_wave IN (SELECT id FROM wave_addresses WHERE id=?9 OR slug=?1)))
                AND (?2 IS NULL OR task_id IN (SELECT t.id FROM projects p CROSS JOIN tasks t ON t.project_id=p.id
                    WHERE p.id=?10 OR p.project_slug=?2 OR p.external_project_id=?11)
                    OR EXISTS (SELECT 1 FROM machine_record_inputs t WHERE t.origin_machine_id=ci.origin AND t.kind='turn'
                        AND t.session_id=ci.session_id AND json_extract(t.row,'$.captured_event')=ci.captured
                        AND (t.project_id=?10 OR t.project_slug=?2 OR t.external_project_id=?11)))
                AND (?3 IS NULL OR task_id=?3
                    OR EXISTS (SELECT 1 FROM machine_record_inputs t WHERE t.origin_machine_id=ci.origin AND t.kind='turn'
                        AND t.session_id=ci.session_id AND json_extract(t.row,'$.captured_event')=ci.captured
                        AND t.task_id=?3))),
                eligible AS MATERIALIZED (
                SELECT captured,input_id,session_id,caller_input_id,started,task_id,wave_id,ended,thread,turn,latest,copied_record, SUM(ended IS NULL) OVER () AS unfinished,
                    ROW_NUMBER() OVER (PARTITION BY ended IS NULL ORDER BY started DESC,input_id DESC,session_id,thread,turn) AS ordinal,
                    COUNT(*) OVER () AS total FROM scoped)
                SELECT eligible.session_id,eligible.input_id,eligible.caller_input_id,started,eligible.task_id,eligible.wave_id,
                    (SELECT slug FROM wave_addresses WHERE id=eligible.wave_id),
                    coalesce(s.current_capture,json_extract(cs.row,'$.current_capture')),
                    coalesce(s.cwd,json_extract(cs.row,'$.cwd'),''),coalesce(s.repo,json_extract(cs.row,'$.repo')),
                    coalesce(s.skill,json_extract(cs.row,'$.skill')),coalesce(s.provider,json_extract(cs.row,'$.provider')),
                    coalesce(s.model,json_extract(cs.row,'$.model')),coalesce(s.interactive,json_extract(cs.row,'$.interactive'),0),
                    total > MAX(?8,unfinished),CASE WHEN copy.rowid IS NULL THEN COALESCE((SELECT json_extract(payload,'$.work_source') FROM session_events WHERE seq=eligible.captured),
                        CASE WHEN eligible.captured=s.current_capture AND s.bound_at IS NULL THEN s.work_source END)
                    WHEN copy.kind='session_event' THEN COALESCE(json_extract(json_extract(copy.row,'$.payload'),'$.work_source'),
                        CASE WHEN eligible.captured=json_extract(cs.row,'$.current_capture') AND json_extract(cs.row,'$.bound_at') IS NULL
                        THEN json_extract(cs.row,'$.work_source') END) END,
                    eligible.captured,eligible.thread,eligible.turn,eligible.latest,
                    CASE WHEN eligible.captured=coalesce(s.current_capture,json_extract(cs.row,'$.current_capture'))
                        THEN coalesce(s.skill_version,json_extract(cs.row,'$.skill_version')) END,
                    copy.origin_machine_id,CASE WHEN copy.kind='turn' THEN copy.row END
                FROM eligible LEFT JOIN agent_sessions s ON eligible.copied_record IS NULL AND s.id=eligible.session_id
                LEFT JOIN machine_records copy ON copy.rowid=eligible.copied_record
                LEFT JOIN machine_records cs ON cs.origin_machine_id=copy.origin_machine_id AND cs.kind='session' AND cs.key=eligible.session_id
                WHERE (?8 IS NULL OR ended IS NULL OR ordinal<=MAX(?8-unfinished,0))
                ORDER BY started DESC,eligible.input_id DESC,eligible.session_id,eligible.thread,eligible.turn",super::machine_records::COLLECTED_TURNS))?;
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
                wave.map(loopflow_model::id::canonical_uuid),
                project.map(|project| loopflow_model::id::canonical_prefixed(project, "proj_")),
                project.map(loopflow_model::id::canonical_uuid),
                caller.map(loopflow_model::id::canonical_session),
                matches!(selection, HistorySelection::All),
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
                inputs.push(PendingInput {
                    copy: row
                        .get::<_, Option<String>>(21)?
                        .map(|origin| -> rusqlite::Result<_> {
                            Ok((
                                loopflow_model::durable::MachineId::from_raw(origin),
                                row.get(22)?,
                            ))
                        })
                        .transpose()?,
                    session_id: row.get(0)?,
                    input: row.get(1)?,
                    caller: row.get(2)?,
                    started: row.get(3)?,
                    task,
                    wave: row.get(5)?,
                    names: (row.get(6)?, task_identifier),
                    current_input: row.get(7)?,
                    cwd: row.get(8)?,
                    repo: row.get(9)?,
                    skill: row.get(10)?,
                    provider: row.get(11)?,
                    model: row.get(12)?,
                    interactive: row.get(13)?,
                    truncated: row.get::<_, Option<bool>>(14)?.unwrap_or(false),
                    work_source: row.get(15)?,
                    captured: row.get(16)?,
                    thread: row.get(17)?,
                    turn: row.get(18)?,
                    latest: row.get(19)?,
                    skill_version: row.get(20)?,
                });
            }
            inputs
        };
        Ok(PendingHistory {
            truncated: inputs.iter().any(|input| input.truncated),
            scope: (
                wave.map(str::to_owned),
                project.map(str::to_owned),
                task.map(str::to_owned),
            ),
            inputs,
        })
    }

    /// Read one selected input's saved metadata and scoped events.
    pub fn read_history_input(
        &self,
        pending: &PendingHistory,
        input: &PendingInput,
    ) -> StoreResult<HistoryInput> {
        let scope = (
            pending.scope.0.as_deref(),
            pending.scope.1.as_deref(),
            pending.scope.2.as_deref(),
        );
        let PendingInput {
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
            truncated: _,
            work_source,
            captured,
            thread,
            turn,
            skill_version,
            latest: _,
            copy,
        } = input.clone();
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
                .map(loopflow_model::id::WaveId::parse)
                .transpose()
                .map_err(invalid)?,
            observed_at: started,
            cwd: cwd.into(),
            repo,
            skill,
            skill_version,
            provider,
            model,
            interactive,
            work_source: work_source
                .map(|source| serde_json::from_value(serde_json::Value::String(source)))
                .transpose()?,
        };
        let origin = copy.as_ref().map(|(origin, _)| origin);
        let history = match input.as_deref() {
            Some(input) => self.summary_for_input_matching(&session_id, input, scope, origin)?,
            None => Vec::new(),
        };
        let turns = match &copy {
            Some((_, Some(row))) => vec![serde_json::from_str(row)?],
            Some((origin, None)) => self.copied_input_turns(
                origin,
                &session_id,
                captured.ok_or_else(|| invalid("a copied input has no capture"))?,
                scope,
            )?,
            None => self.turns_matching(
                &session_id,
                input.as_deref(),
                (thread.as_ref(), turn.as_deref()),
                scope,
            )?,
        };
        Ok(HistoryInput {
            session,
            artifact_key: input,
            events: history,
            turns,
            names,
        })
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

    /// Record which skill text the launch of `artifact_key` ran, while that
    /// input is still its Session's current one.
    pub fn record_skill_version(&self, artifact_key: &str, version: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE agent_sessions SET skill_version=?2
             WHERE id=(SELECT session_id FROM session_events WHERE kind='captured' AND receipt_key=?1)
               AND current_capture=(SELECT seq FROM session_events WHERE kind='captured' AND receipt_key=?1)",
            params![artifact_key, version],
        )?;
        Ok(())
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

    /// Admit the conversation and its captured input before provider effects.
    pub fn create_session_prepared(
        &self,
        session: LfSession,
        caller_process: Option<&loopflow_model::id::LfProcessId>,
    ) -> StoreResult<LfSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = session_in(&tx, &session.id)? {
            return Ok(existing);
        }
        let session = reserve_session_in(&tx, session, caller_process)?;
        tx.commit()?;
        Ok(session)
    }

    pub fn primary_session(&self, scope: &PrimaryScope) -> StoreResult<Option<LfSession>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        primary_session_in(&conn, scope)
    }

    /// Find the scope's uncompleted primary, or admit the prepared `session` as it.
    pub fn ensure_primary_session(
        &self,
        scope: &PrimaryScope,
        session: LfSession,
        caller_process: Option<&loopflow_model::id::LfProcessId>,
        person_start_at: Option<i64>,
    ) -> StoreResult<LfSession> {
        // Nearly every call finds the primary it asks for; that takes no write lock.
        if let Some(current) = self.primary_session(scope)? {
            return Ok(current);
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session =
            ensure_primary_session_in(&tx, scope, session, caller_process, person_start_at)?;
        tx.commit()?;
        Ok(session)
    }

    /// Called under the Session launch lock after proving there is no live client.
    pub fn admit_session_workspace(
        &self,
        session: &LfSession,
        cwd: &Path,
    ) -> StoreResult<LfSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Binding may have selected a directory while an opening prepared the
        // old scope's workspace. Its explicit choice wins over that preparation.
        let updated = tx.execute(
            &format!("UPDATE agent_sessions SET cwd=CASE WHEN work_source='bound' THEN cwd ELSE ?3 END WHERE id=?1 AND current_capture IS ?2 AND completed_at IS NULL AND {} IS NOT NULL", primary_scope_sql("agent_sessions")),
            params![session.id, session.captured, cwd.to_string_lossy()],
        )?;
        if updated != 1 {
            return Err(StoreError::InvalidAuthority(
                "Session changed before workspace admission".into(),
            ));
        }
        let admitted = session_in(&tx, &session.id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(admitted)
    }

    /// The repository, Wave or Task a Session is or was primary for.
    pub fn primary_scope(&self, id: &str) -> StoreResult<Option<PrimaryScope>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let row: Option<(Option<String>, Option<String>)> = conn
            .query_row(
                "SELECT s.primary_scope, CASE s.primary_scope
                     WHEN 'wave' THEN s.wave_id WHEN 'repository' THEN s.repo END
                 FROM agent_sessions s WHERE s.id=?1 AND s.primary_scope IS NOT NULL
                 UNION ALL SELECT 'task', tp.task_id FROM task_primary_sessions tp
                 WHERE tp.session_id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((Some(kind), scope)) = row else {
            return Ok(None);
        };
        match (kind.as_str(), scope) {
            ("task", Some(task)) => Ok(Some(PrimaryScope::Task(
                TaskId::parse(&task).map_err(invalid)?,
            ))),
            ("wave", Some(wave)) => Ok(Some(PrimaryScope::Wave(
                loopflow_model::id::WaveId::parse(&wave).map_err(invalid)?,
            ))),
            ("repository", Some(repo)) => Ok(Some(PrimaryScope::Repository(
                crate::address::CanonicalRepo::from_canonical_path(repo.into()),
            ))),
            _ => Err(invalid(format!(
                "Session {id} has unsupported primary scope {kind:?}"
            ))),
        }
    }

    pub fn publish_capture(&self, id: &str, captured: Option<i64>) -> StoreResult<()> {
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
    pub fn replace_session_input_prepared(
        &self,
        expected_input: Option<i64>,
        mut session: LfSession,
        creation: Option<&str>,
        caller: Option<&loopflow_model::id::LfProcessId>,
    ) -> StoreResult<LfSession> {
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
        replace_input_in(&tx, &mut session, caller)?;
        let session = session_in(&tx, &session.id)?.ok_or(StoreError::NotFound)?;
        if let Some(id) = creation {
            let changed = tx.execute("UPDATE creation_receipts SET milestone='admitted',updated_at=?3 WHERE request_id=?1 AND artifact_key=?2 AND milestone='stopping'",
                params![id,session.artifact_key,crate::rows::now_unix()])?;
            if changed != 1 {
                return Err(StoreError::InvalidData(
                    "new thread receipt does not match its reserved input".into(),
                ));
            }
        }
        tx.commit()?;
        Ok(session)
    }

    /// Assign future work without changing earlier attribution. Closed Sessions bind too.
    pub fn bind_session_prepared(
        &self,
        id: &str,
        expected_capture: Option<i64>,
        task: &TaskId,
        cwd: &Path,
        caller: &SessionCaller,
    ) -> StoreResult<LfSession> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current = session_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        if current.cwd != cwd || current.captured != expected_capture {
            return Err(StoreError::InvalidAuthority(
                "Session changed before binding".into(),
            ));
        }
        require_current_actor_in(&tx, id, caller)?;
        bind_session_in(&tx, &current, task)?;
        let bound = session_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        tx.commit()?;
        Ok(bound)
    }

    /// Sessions assigned to a Task after they began, oldest bind first.
    pub fn bound_sessions(&self) -> StoreResult<Vec<crate::session::SessionBind>> {
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
    pub fn projected_sessions(
        &self,
        filter: &crate::session::SessionFilter,
        inputs: Option<&SessionProjectionInputs>,
        now: i64,
        live_agents: &str,
    ) -> StoreResult<Option<Vec<LfSession>>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        if let Some(inputs) = inputs {
            if !inputs.is_current(&snapshot)? {
                return Ok(None);
            }
        }
        let (sql, values) = inventory_query(filter, SESSION_SELECT, now, live_agents)?;
        let mut query = snapshot.prepare(&sql)?;
        let rows = query.query_map(rusqlite::params_from_iter(values), read_session)?;
        Ok(Some(rows.map(|row| row?).collect::<StoreResult<_>>()?))
    }

    pub fn session_inputs(&self, id: &str) -> StoreResult<Vec<String>> {
        let canonical = loopflow_model::id::canonical_session(id);
        let id = canonical.as_str();
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT receipt_key FROM session_events WHERE kind='captured' AND session_id=?1 ORDER BY seq",
        )?;
        let rows = query.query_map([id], |row| row.get::<_, String>(0))?;
        rows.map(|row| crate::session_record::parse_artifact_key(&row?).map_err(invalid))
            .collect()
    }

    pub fn rename_session_prepared(
        &self,
        id: &str,
        title: &str,
        source: TitleSource,
        caller: &SessionCaller,
    ) -> StoreResult<()> {
        let canonical = loopflow_model::id::canonical_session(id);
        let id = canonical.as_str();
        if title.trim().is_empty() {
            return Err(invalid("Session title cannot be empty"));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let session = session_in(&tx, id)?.ok_or(StoreError::NotFound)?;
        require_current_actor_in(&tx, id, caller)?;
        rename_session_in(&tx, &session, title, source, None)?;
        tx.commit()?;
        Ok(())
    }

    /// The Session continues on `machine`: complete it here, naming where it went.
    pub fn complete_moved_session(
        &self,
        id: &str,
        machine: &loopflow_model::durable::MachineId,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE agent_sessions SET completed_at=COALESCE(completed_at,?3),moved_to_machine_id=?2 WHERE id=?1",
            params![id, machine.as_str(), crate::rows::now_unix()],
        )?;
        if updated != 1 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// `task`'s conversation left with it: complete it here, bound to the Task
    /// as it arrives there, rather than naming a Machine. It resumes wherever
    /// the Task runs, even after its checkout here is gone.
    pub fn complete_task_conversation(&self, id: &str, task: &TaskId) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let updated = conn.execute(
            "UPDATE agent_sessions SET completed_at=COALESCE(completed_at,?3),moved_to_machine_id=NULL,
                wave_id=CASE WHEN task_id IS NULL THEN (SELECT p.wave_id FROM tasks t
                    LEFT JOIN projects p ON p.id=t.project_id WHERE t.id=?2) ELSE wave_id END,
                task_id=COALESCE(task_id,?2)
             WHERE id=?1",
            params![id, task.as_str(), crate::rows::now_unix()],
        )?;
        if updated != 1 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }

    /// The Machine a Session here moved to; `None` when it never left.
    pub fn session_moved_to(
        &self,
        id: &str,
    ) -> StoreResult<Option<loopflow_model::durable::MachineId>> {
        let canonical = loopflow_model::id::canonical_session(id);
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT moved_to_machine_id FROM agent_sessions WHERE id=?1",
            [canonical.as_str()],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
        .map(|machine| loopflow_model::durable::MachineId::parse(&machine).map_err(invalid))
        .transpose()
    }

    /// `repository` or `wave` for a scope's ongoing conversation.
    pub fn session_primary_scope(&self, id: &str) -> StoreResult<Option<String>> {
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

    /// The Session continues in `cwd`; its recorded directory is gone.
    pub fn relocate_session(&self, id: &str, cwd: &Path) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE agent_sessions SET cwd=?2 WHERE id=?1",
            params![id, cwd.to_string_lossy()],
        )?;
        Ok(())
    }

    /// A Session that moved away comes back: open again, in `cwd`.
    pub fn reopen_moved_session(&self, id: &str, cwd: &Path) -> StoreResult<()> {
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
                return Ok(crate::session::task_session_title(
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
        repair_session_title_in(conn, &id, previous)?;
    }
    Ok(())
}

/// Keep `observations` of `session`'s inputs. A resume reference and an
/// independent launch are state of their input, kept on its captured row;
/// every other observation is history, returned for the caller to append
/// once this transaction commits.
pub(super) fn retain_history_in(
    conn: &Connection,
    session: &LfSession,
    observations: &[crate::session::SessionObservation],
) -> StoreResult<Vec<crate::history::Row>> {
    let mut history = Vec::new();
    for observation in observations {
        let captured: Option<(i64,String)> = conn.query_row(
            "SELECT seq,session_id FROM session_events WHERE kind='captured' AND receipt_key=?1",
            [&observation.artifact_key], |row| Ok((row.get(0)?,row.get(1)?))).optional()?;
        let (captured, owner) = captured.ok_or(StoreError::NotFound)?;
        if owner != session.id.as_str() {
            return Err(invalid("artifact belongs to another conversation"));
        }
        let evidence = &observation.payload["evidence"];
        if observation.source.starts_with("provider-session:") {
            let reference: crate::session_record::ProviderSessionRef =
                serde_json::from_value(evidence.clone())?;
            reference.validate().map_err(invalid)?;
            if Some(captured) == session.captured {
                history.extend(super::provider_transitions::observe_in(
                    conn,
                    &session.id,
                    reference.agent_session.as_str(),
                    "observed",
                )?);
            }
            conn.execute(
                "UPDATE session_events SET provider_thread=?2,provider_account=?3 WHERE seq=?1",
                params![
                    captured,
                    reference.agent_session,
                    reference.account_id.as_ref().map(|id| id.as_str())
                ],
            )?;
            continue;
        }
        if observation.source == "manifest.json" && evidence["flow"]["kind"] == "independent" {
            conn.execute(
                "UPDATE session_events SET independent=1 WHERE seq=?1",
                [captured],
            )?;
        }
        let mut row = crate::history::Row::observed(
            &session.id,
            format!("{}:{}", observation.artifact_key, observation.source),
            Some(captured),
            observation.observed_at,
            serde_json::to_string(&observation.payload)?,
        );
        row.task_id = observation
            .task_id
            .as_ref()
            .map(|id| id.as_str().to_owned());
        row.wave_id = observation
            .wave_id
            .as_ref()
            .map(|id| id.as_str().to_owned());
        history.push(row);
    }
    Ok(history)
}

/// Resolve ancestry on the conversation's admission transaction.
pub(super) fn reserve_session_in(
    conn: &Connection,
    mut session: LfSession,
    caller: Option<&loopflow_model::id::LfProcessId>,
) -> StoreResult<LfSession> {
    resolve_ancestry_in(conn, &mut session)?;
    insert_session_in(conn, &mut session, caller)?;
    Ok(session)
}

/// A Session that names no Task or Wave takes its Task by the rule a Graph run
/// does ([`super::task_work::starting_task_in`]), recorded now so a later move
/// or a reused path changes nothing; the Task of the Session that started it
/// arrives named, as declared. A primary for a repository or Wave declares
/// its scope.
fn resolve_ancestry_in(conn: &Connection, session: &mut LfSession) -> StoreResult<()> {
    if session.task_id.is_none()
        && session.wave_id.is_none()
        && matches!(session.work_source, None | Some(WorkSource::Checkout))
    {
        session.task_id = super::task_work::starting_task_in(conn, None, Some(&session.cwd), None)?;
        if session.task_id.is_some() {
            session.work_source = Some(WorkSource::Checkout);
        }
    }
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
    Ok(())
}

/// A replaced provider keeps causal history, but cannot mutate its conversation.
pub(super) fn require_current_actor_in(
    conn: &Connection,
    id: &str,
    observed: &SessionCaller,
) -> StoreResult<()> {
    if let Some(caller) = observed
        .agent
        .as_ref()
        .filter(|caller| caller.session_id == id)
    {
        let current: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE id=?1
                AND agent_process_id=?2 AND attached_lf_process_id IS NOT NULL)",
            params![id, caller.agent_process_id],
            |row| row.get(0),
        )?;
        if !current {
            return Err(StoreError::InvalidAuthority(
                "Session provider was replaced".into(),
            ));
        }
    }
    if let Some(key) = &observed.capture_key {
        crate::session_record::parse_artifact_key(key).map_err(invalid)?;
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
        if observed
            .agent
            .as_ref()
            .is_some_and(|caller| caller.session_id != owner)
        {
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
fn capture_in(
    conn: &Connection,
    session: &LfSession,
    observed_at: i64,
    process: Option<&loopflow_model::id::LfProcessId>,
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
            wave.as_ref().map(loopflow_model::id::WaveId::as_str),observed_at,serde_json::to_string(&payload)?])?;
    Ok(conn.last_insert_rowid())
}

fn insert_session_in(
    conn: &Connection,
    session: &mut LfSession,
    process: Option<&loopflow_model::id::LfProcessId>,
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
            session
                .wave_id
                .as_ref()
                .map(loopflow_model::id::WaveId::as_str),
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
    Ok(())
}

pub(super) fn replace_input_in(
    conn: &Transaction<'_>,
    session: &mut LfSession,
    process: Option<&loopflow_model::id::LfProcessId>,
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
    session.captured = Some(capture_in(conn, session, crate::rows::now_unix(), process)?);
    // A selected AgentSession belongs to its provider; another provider starts
    // a fresh one. Earlier selections remain in session_events.
    // The version belongs to the input it launched with; the next launch records its own.
    conn.execute("UPDATE agent_sessions SET current_capture=?2,input_published=?3,skill=?4,skill_version=NULL,provider=?5,model=?6,
        request=?7,provider_thread=CASE WHEN provider IS ?5 THEN provider_thread END
        WHERE id=?1",
        params![session.id,session.captured,session.input_published,
            session.skill,session.provider,session.model,session.request])?;
    Ok(())
}

#[cfg(any(test, feature = "test-support"))]
pub fn test_capture(conn: &Connection, session: &str, artifact: &str) {
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

#[cfg(any(test, feature = "test-support"))]
impl SqliteStore {
    pub fn test_session(&self, id: &str, artifact: &str) -> LfSession {
        self.test_session_at(id, artifact, 1)
    }

    /// A Session whose input was captured at `created_at`.
    pub fn test_session_at(&self, id: &str, artifact: &str, created_at: i64) -> LfSession {
        self.create_session_prepared(
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
                completed_at: None,
                created_at,
            },
            None,
        )
        .unwrap()
    }
}

#[cfg(test)]
mod metadata_tests {

    #[test]
    fn resume_reopens_the_conversation_last_opened_in_a_checkout() {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("loopflow.db")).unwrap();
        for id in ["first", "second"] {
            store.test_session(id, &crate::session_record::new_artifact_key());
        }
        let reopened = |store: &crate::sqlite::SqliteStore| {
            store
                .checkout_conversation(std::path::Path::new("/fixture"))
                .unwrap()
                .unwrap()
                .id
        };
        store.record_session_opened("first", 5_000).unwrap();
        store.record_session_opened("second", 3_000).unwrap();
        assert_eq!(reopened(&store).as_str(), "first");
        store.record_session_opened("second", 9_000).unwrap();
        assert_eq!(reopened(&store).as_str(), "second");
    }

    #[test]
    fn an_upgrade_keeps_each_conversations_last_opening() {
        use crate::migration_runner::{apply_before_migration, migration_section_sql};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "history_apart_from_state",
        );
        conn.execute_batch(
            "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd)
             VALUES('opened','Opened','human',1,0,'/repo'),('never','Never','human',1,0,'/repo');
             INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES
             ('opened','observed','a',1,'{\"type\":\"interactive_opened\",\"opened_at_ms\":4000}'),
             ('opened','observed','b',2,'{\"type\":\"interactive_opened\",\"opened_at_ms\":7000}');",
        )
        .unwrap();
        conn.execute_batch(&migration_section_sql("history_apart_from_state"))
            .unwrap();
        let opened: Vec<Option<i64>> = conn
            .prepare("SELECT opened_at FROM agent_sessions ORDER BY id DESC")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(opened, [Some(7_000), None]);
    }

    #[test]
    fn an_upgrade_moves_resume_references_and_the_latest_transition_onto_their_owners() {
        use crate::migration_runner::{apply_before_migration, migration_section_sql};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "history_apart_from_state",
        );
        let reference = |thread: &str, account: &str| {
            format!(
                r#"{{"source":"provider-session","evidence":{{"schema_version":1,"provider_session_id":"{thread}","account_id":{account}}}}}"#
            )
        };
        let insert = |session: &str, kind: &str, receipt: &str, payload: &str| {
            conn.execute(
                "INSERT INTO session_events(session_id,kind,receipt_key,observed_at,payload) VALUES(?1,?2,?3,7,?4)",
                rusqlite::params![session, kind, receipt, payload],
            )
            .unwrap();
        };
        conn.execute_batch(
            "INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd)
             VALUES('current','Current','human',1,0,'/repo'),('legacy','Legacy','human',1,0,'/repo');",
        )
        .unwrap();
        insert("current", "captured", "first", "{}");
        insert("current", "captured", "second", "{}");
        insert(
            "current",
            "observed",
            "first:provider-session:a",
            &reference("old", "\"work\""),
        );
        insert(
            "current",
            "observed",
            "first:provider-session:b",
            &reference("new", "null"),
        );
        insert(
            "current",
            "observed",
            "second:manifest.json",
            r#"{"source":"manifest.json","evidence":{"flow":{"kind":"independent"}}}"#,
        );
        insert(
            "legacy",
            "observed",
            "agent-session:1",
            r#"{"type":"agent_session_selected","agent_process_id":"44444444-4444-4444-8444-444444444444","provider_thread":"earlier","next_provider_thread":"fork","source":"fork"}"#,
        );
        insert(
            "current",
            "observed",
            "agent-session:2",
            r#"{"type":"agent_session_selected","agent_process_id":"44444444-4444-4444-8444-444444444444","provider_thread":null,"next_provider_thread":"old","source":"new"}"#,
        );
        insert(
            "current",
            "observed",
            "provider-transition:3",
            r#"{"type":"provider_transition","source":"observed","transition":{"kind":"resumed","from_provider_session":"old","to_provider_session":"new","from_agent_process":null,"to_agent_process":"55555555-5555-4555-8555-555555555555","original_running":null,"seed":null,"observed_at":9}}"#,
        );
        conn.execute_batch(&migration_section_sql("history_apart_from_state"))
            .unwrap();

        type Input = (String, Option<String>, Option<String>, Option<i64>);
        let inputs: Vec<Input> = conn
            .prepare("SELECT receipt_key,provider_thread,provider_account,independent FROM session_events WHERE kind='captured' ORDER BY seq")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            inputs,
            [
                ("first".into(), Some("new".into()), None, None),
                ("second".into(), None, None, Some(1)),
            ]
        );
        let transition = |session: &str| -> super::super::provider_transitions::ProviderTransition {
            let json: String = conn
                .query_row(
                    "SELECT transition FROM agent_sessions WHERE id=?1",
                    [session],
                    |row| row.get(0),
                )
                .unwrap();
            serde_json::from_str(&json).unwrap()
        };
        assert_eq!(transition("current").to_provider_session, "new");
        let legacy = transition("legacy");
        assert_eq!(
            legacy.kind,
            super::super::provider_transitions::TransitionKind::Forked
        );
        assert_eq!(legacy.from_provider_session.as_deref(), Some("earlier"));
        assert_eq!(legacy.seed.as_deref(), Some("native"));
        assert_eq!(legacy.observed_at, 7);
    }

    #[test]
    fn released_lf_sees_the_primary_this_build_chose() {
        use crate::migration_runner::{apply_before_migration, migration_section_sql};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "task_facts_once",
        );
        let task = loopflow_model::durable::TaskId::new();
        conn.execute_batch(&format!(
            "INSERT INTO repository_plans(id,repo,selected) VALUES('repo_1','/repo',1);
             INSERT INTO tasks(id,repository_id,issue_identifier,created_at)
             VALUES('{task}','repo_1','FIX-1',1);
             INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,starts_since_turn,last_started_at,start_failure)
             VALUES('chosen','Chosen','human',1,0,'/elsewhere',3,100,'provider unavailable');
             INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd,task_id)
             VALUES('other','Other','human',1,0,'/elsewhere','{task}');
             INSERT INTO task_primary_sessions(task_id,session_id) VALUES('{task}','chosen');"
        ))
        .unwrap();
        // As the runner migrates: the rebuild's DROP would otherwise cascade.
        conn.pragma_update(None, "foreign_keys", "OFF").unwrap();
        conn.execute_batch(&migration_section_sql("task_facts_once"))
            .unwrap();
        // What this build reads: each Session's scope, and its Task.
        let scopes = || {
            conn.prepare(&format!(
                "SELECT s.id,{},s.task_id FROM agent_sessions s ORDER BY s.id",
                super::primary_scope_sql("s")
            ))
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<rusqlite::Result<Vec<(String, Option<String>, Option<String>)>>>()
            .unwrap()
        };
        // What released lf reads: the Task's row.
        let released = || -> String {
            conn.query_row(
                "SELECT session_id FROM task_primary_sessions WHERE task_id=?1",
                [task.as_str()],
                |row| row.get(0),
            )
            .unwrap()
        };
        let owned = Some(task.to_string());
        assert_eq!(
            scopes(),
            [
                ("chosen".into(), Some("task".into()), owned.clone()),
                ("other".into(), None, owned.clone()),
            ]
        );
        let recovery: (u32, i64, String) = conn
            .query_row(
                "SELECT starts_since_turn,last_started_at,start_failure FROM agent_sessions WHERE id='chosen'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(recovery, (3, 100, "provider unavailable".to_owned()));

        super::choose_task_primary_in(&conn, &task, "other").unwrap();
        assert_eq!(released(), "other");
        assert_eq!(
            scopes(),
            [
                ("chosen".into(), None, owned.clone()),
                ("other".into(), Some("task".into()), owned),
            ]
        );
        let marked: u32 = conn
            .query_row(
                "SELECT count(*) FROM agent_sessions WHERE primary_scope IS NOT NULL",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            marked, 0,
            "released lf would count a marked Session as no Task's"
        );
    }

    #[test]
    fn a_released_tasks_primary_session_stays_its_conversation_here() {
        use crate::migration_runner::{apply_before_migration, migration_section_sql};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "task_machine_local",
        );
        conn.execute_batch(
            "INSERT INTO repository_plans(id,repo,selected) VALUES('repo_1','/repo',1);
             INSERT INTO agent_sessions(id,title,title_source,created_at,input_published,cwd)
             VALUES('primary','Primary','human',1,0,'/repo');
             INSERT INTO tasks(id,repository_id,issue_identifier,created_at,primary_session_id)
             VALUES('task_1','repo_1','FIX-1',1,'primary');
             INSERT INTO tasks(id,repository_id,issue_identifier,created_at)
             VALUES('task_2','repo_1','FIX-2',1);",
        )
        .unwrap();
        conn.execute_batch(&migration_section_sql("task_machine_local"))
            .unwrap();
        let primaries = || {
            let mut query = conn
                .prepare("SELECT task_id,session_id FROM task_primary_sessions")
                .unwrap();
            query
                .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                .unwrap()
                .collect::<rusqlite::Result<Vec<(String, String)>>>()
                .unwrap()
        };
        assert_eq!(primaries(), [("task_1".into(), "primary".into())]);
        conn.execute_batch(
            "PRAGMA foreign_keys=ON; DELETE FROM agent_sessions WHERE id='primary';",
        )
        .unwrap();
        assert_eq!(primaries(), []);
    }
}
