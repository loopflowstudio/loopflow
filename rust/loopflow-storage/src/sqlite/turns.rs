//! Provider turns in `usage.turns`: one row from dispatch to end. A turn's
//! start, origin, usage and completion each fill their own columns of that
//! row; the first reading of each is kept, and a different second one is a
//! recording gap.

use rusqlite::{params, OptionalExtension, Transaction};
use serde_json::Value;

use crate::session::{RecordedUsage, SessionTurnOrigin, Turn, TurnContext};
use crate::{StoreError, StoreResult};
use loopflow_model::chat::types::TurnUsage;
use loopflow_model::id::AgentSessionId;

use super::SqliteStore;

/// The columns [`read_turn`] reads, each named under `alias`.
pub(crate) fn turn_columns(alias: &str) -> String {
    [
        "id",
        "session_id",
        "provider_thread",
        "provider_turn",
        "captured_event",
        "lf_process_id",
        "agent_process_id",
        "task_id",
        "wave_id",
        "project_id",
        "repository",
        "skill",
        "graph",
        "step",
        "started_at",
        "ended_at",
        "status",
        "duration_ms",
        "error",
        "provider",
        "model",
        "usage_at",
        "usage_final",
        "usage_partial",
        "input_tokens",
        "cached_input_tokens",
        "cache_write_tokens",
        "output_tokens",
        "reasoning_tokens",
        "total_input_tokens",
        "peak_input_tokens",
        "context_window_tokens",
        "cost_usd",
        "first_request_tokens",
        "peak_request_tokens",
        "tool_results",
        "tool_output_tokens",
        "compactions",
        "compaction_tokens",
    ]
    .map(|column| format!("{alias}.{column}"))
    .join(",")
}

/// One turn from a row selected with [`turn_columns`].
pub(crate) fn read_turn(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoreResult<Turn>> {
    let count = |index: usize| -> rusqlite::Result<Option<u64>> {
        Ok(row.get::<_, Option<i64>>(index)?.map(|count| count as u64))
    };
    let usage_at: Option<i64> = row.get(21)?;
    let usage = match usage_at {
        Some(observed_at) => Some(RecordedUsage {
            observed_at,
            final_reading: row.get(22)?,
            partial: row.get(23)?,
            usage: TurnUsage {
                input_tokens: count(24)?,
                cache_read_tokens: count(25)?,
                cache_write_tokens: count(26)?,
                output_tokens: count(27)?,
                reasoning_tokens: count(28)?,
                total_input_tokens: count(29)?,
                peak_input_tokens: count(30)?,
                context_window_tokens: count(31)?,
                cost_usd: row.get(32)?,
                model: row.get(20)?,
            },
        }),
        None => None,
    };
    let context = match row.get::<_, Option<i64>>(35)? {
        Some(tool_results) => Some(TurnContext {
            first_request_tokens: count(33)?,
            peak_request_tokens: count(34)?,
            tool_results: tool_results as u64,
            tool_output_tokens: count(36)?.unwrap_or(0),
            compactions: count(37)?.unwrap_or(0),
            compaction_tokens: count(38)?,
        }),
        None => None,
    };
    let error = row
        .get::<_, Option<String>>(18)?
        .map(|error| serde_json::from_str(&error))
        .transpose();
    let error = match error {
        Ok(error) => error,
        Err(error) => return Ok(Err(error.into())),
    };
    Ok(Ok(Turn {
        id: row.get(0)?,
        session_id: row.get::<_, String>(1)?.into(),
        thread: row.get(2)?,
        turn: row.get(3)?,
        captured_event: row.get(4)?,
        lf_process_id: row.get(5)?,
        agent_process_id: row.get(6)?,
        task_id: row.get(7)?,
        wave_id: row.get(8)?,
        project_id: row.get(9)?,
        repository: row.get(10)?,
        skill: row.get(11)?,
        graph: row.get(12)?,
        step: row.get(13)?,
        started_at: row.get(14)?,
        ended_at: row.get(15)?,
        status: row.get(16)?,
        duration_ms: row.get(17)?,
        error,
        provider: row.get(19)?,
        usage,
        context,
    }))
}

/// The row of `turn`, inserted when it has none.
fn ensure_turn(
    tx: &Transaction<'_>,
    session: &str,
    thread: &AgentSessionId,
    turn: &str,
) -> StoreResult<i64> {
    tx.execute(
        "INSERT INTO turns(session_id,provider_thread,provider_turn) VALUES(?1,?2,?3)
         ON CONFLICT(session_id,provider_thread,provider_turn) DO NOTHING",
        params![session, thread, turn],
    )?;
    Ok(tx.query_row(
        "SELECT id FROM turns WHERE session_id=?1 AND provider_thread=?2 AND provider_turn=?3",
        params![session, thread, turn],
        |row| row.get(0),
    )?)
}

fn conflict(what: &str, session: &str, turn: &str) -> StoreError {
    StoreError::InvalidData(format!(
        "a second, different {what} for turn {turn} of Session {session}; the first is kept"
    ))
}

/// Write `usage` onto the turn `id`; a turn keeps the reading it has, and a
/// different one is a conflict. Returns whether it wrote.
pub(crate) fn write_usage_in(
    tx: &Transaction<'_>,
    id: i64,
    usage: &RecordedUsage,
    provider: Option<&str>,
) -> StoreResult<bool> {
    let kept: Option<Option<i64>> = tx
        .query_row("SELECT usage_at FROM turns WHERE id=?1", [id], |row| {
            row.get(0)
        })
        .optional()?;
    if kept.flatten().is_some() {
        let turn = tx.query_row(
            &format!("SELECT {} FROM turns t WHERE t.id=?1", turn_columns("t")),
            [id],
            read_turn,
        )??;
        let same = turn.usage.as_ref().is_some_and(|kept| {
            kept.usage == usage.usage
                && kept.partial == usage.partial
                && kept.final_reading == usage.final_reading
        });
        if same {
            return Ok(false);
        }
        return Err(conflict("usage reading", &turn.session_id, &turn.turn));
    }
    let count = |count: Option<u64>| count.map(|count| count as i64);
    let reading = &usage.usage;
    tx.execute(
        "UPDATE turns SET usage_at=?2,usage_final=?3,usage_partial=?4,input_tokens=?5,
            cached_input_tokens=?6,cache_write_tokens=?7,output_tokens=?8,reasoning_tokens=?9,
            total_input_tokens=?10,peak_input_tokens=?11,context_window_tokens=?12,cost_usd=?13,
            model=COALESCE(?14,model),provider=COALESCE(?15,provider)
         WHERE id=?1",
        params![
            id,
            usage.observed_at,
            usage.final_reading,
            usage.partial,
            count(reading.input_tokens),
            count(reading.cache_read_tokens),
            count(reading.cache_write_tokens),
            count(reading.output_tokens),
            count(reading.reasoning_tokens),
            count(reading.total_input_tokens),
            count(reading.peak_input_tokens),
            count(reading.context_window_tokens),
            reading.cost_usd,
            reading.model,
            provider,
        ],
    )?;
    Ok(true)
}

/// Write a completion `detail` observed at `at` onto the turn `id`. A turn
/// keeps its first end; another with a different status is a conflict.
pub(crate) fn write_completion_in(
    tx: &Transaction<'_>,
    id: i64,
    at: i64,
    detail: &Value,
) -> StoreResult<()> {
    let status = detail["status"].as_str();
    let kept: Option<(Option<String>, String, String)> = tx
        .query_row(
            "SELECT status,session_id,provider_turn FROM turns WHERE id=?1 AND ended_at IS NOT NULL",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?;
    if let Some((kept, session, turn)) = kept {
        if kept.as_deref() == status {
            return Ok(());
        }
        return Err(conflict("completion", &session, &turn));
    }
    let context: Option<TurnContext> = match detail.get("context") {
        Some(context) if !context.is_null() => Some(serde_json::from_value(context.clone())?),
        _ => None,
    };
    let error = [&detail["errors"], &detail["error"]]
        .into_iter()
        .find(|error| !error.is_null())
        .map(serde_json::to_string)
        .transpose()?;
    let count = |count: Option<u64>| count.map(|count| count as i64);
    tx.execute(
        "UPDATE turns SET ended_at=?2,status=?3,duration_ms=?4,error=?5,first_request_tokens=?6,
            peak_request_tokens=?7,tool_results=?8,tool_output_tokens=?9,compactions=?10,
            compaction_tokens=?11
         WHERE id=?1",
        params![
            id,
            at,
            status,
            detail["duration_ms"].as_i64(),
            error,
            context.as_ref().and_then(|c| count(c.first_request_tokens)),
            context.as_ref().and_then(|c| count(c.peak_request_tokens)),
            context.as_ref().map(|c| c.tool_results as i64),
            context.as_ref().map(|c| c.tool_output_tokens as i64),
            context.as_ref().map(|c| c.compactions as i64),
            context.as_ref().and_then(|c| count(c.compaction_tokens)),
        ],
    )?;
    Ok(())
}

impl SqliteStore {
    /// Record that `turn` of `thread` started at `at`.
    pub fn record_turn_start(&self, session: &str, thread: &AgentSessionId, turn: &str, at: i64) {
        self.write_usage("turn start", |tx| {
            tx.execute(
                "INSERT INTO turns(session_id,provider_thread,provider_turn,started_at)
                 VALUES(?1,?2,?3,?4)
                 ON CONFLICT(session_id,provider_thread,provider_turn)
                 DO UPDATE SET started_at=excluded.started_at WHERE started_at IS NULL",
                params![session, thread, turn, at],
            )?;
            Ok(())
        });
    }

    /// Freeze what `turn` was dispatched for, and start it. A turn acquires
    /// its origin once; a second origin must match every field, including an
    /// absent bind, or it is a gap and the first stays.
    pub fn record_session_turn_origin(
        &self,
        thread: &AgentSessionId,
        turn: &str,
        origin: &SessionTurnOrigin,
    ) -> StoreResult<()> {
        let now = time::OffsetDateTime::now_utc().unix_timestamp();
        self.write_usage("turn origin", |tx| {
            let id = ensure_turn(tx, &origin.session_id, thread, turn)?;
            tx.execute(
                "UPDATE turns SET started_at=?2 WHERE id=?1 AND started_at IS NULL",
                params![id, now],
            )?;
            let matches: Option<bool> = tx
                .query_row(
                    "SELECT lf_process_id IS ?2 AND agent_process_id IS ?3
                        AND task_id IS ?4 AND wave_id IS ?5 AND captured_event IS ?6
                     FROM turns WHERE id=?1 AND lf_process_id IS NOT NULL",
                    params![
                        id,
                        origin.lf_process_id,
                        origin.agent_process_id,
                        origin.task_id,
                        origin.wave_id,
                        origin.captured_event
                    ],
                    |row| row.get(0),
                )
                .optional()?;
            match matches {
                Some(true) => Ok(()),
                Some(false) => Err(StoreError::InvalidData(
                    "Native turn has different initiating evidence".into(),
                )),
                None => {
                    tx.execute(
                        "UPDATE turns SET lf_process_id=?2,agent_process_id=?3,task_id=?4,
                            wave_id=?5,captured_event=?6,project_id=?7,repository=?8,skill=?9,
                            graph=?10,step=?11,provider=COALESCE(provider,?12)
                         WHERE id=?1",
                        params![
                            id,
                            origin.lf_process_id,
                            origin.agent_process_id,
                            origin.task_id,
                            origin.wave_id,
                            origin.captured_event,
                            origin.project_id,
                            origin.repository,
                            origin.skill,
                            origin.graph,
                            origin.step,
                            origin.provider,
                        ],
                    )?;
                    Ok(())
                }
            }
        });
        Ok(())
    }

    /// Record `turn`'s usage as its provider read it, dated `at`, resolved to
    /// the turn's own counts. A turn keeps the reading it has; returns
    /// whether one was written.
    pub fn record_turn_usage(
        &self,
        session: &str,
        thread: &AgentSessionId,
        turn: &str,
        at: i64,
        reading: &Value,
    ) -> StoreResult<bool> {
        let usage = crate::session_record::resolve_usage(reading, None, at);
        Ok(self
            .write_usage("turn usage", |tx| {
                let usage = usage.as_ref().ok_or_else(|| {
                    StoreError::InvalidData(format!("unreadable usage reading for turn {turn}"))
                })?;
                let id = ensure_turn(tx, session, thread, turn)?;
                write_usage_in(tx, id, usage, reading["provider"].as_str())
            })
            .unwrap_or(false))
    }

    /// Record that `turn` ended at `at`, with the provider's `detail`: its
    /// status, duration, errors and the context the turn filled.
    pub fn record_turn_completion(
        &self,
        session: &str,
        thread: &AgentSessionId,
        turn: &str,
        at: i64,
        detail: &Value,
    ) -> StoreResult<()> {
        self.write_usage("turn completion", |tx| {
            let id = ensure_turn(tx, session, thread, turn)?;
            write_completion_in(tx, id, at, detail)
        });
        Ok(())
    }

    /// Every turn of `session`, in the order they were first recorded.
    pub fn session_turns(&self, session: &str) -> StoreResult<Vec<Turn>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(&format!(
            "SELECT {} FROM usage.turns t WHERE t.session_id=?1 ORDER BY t.id",
            turn_columns("t")
        ))?;
        let rows = query.query_map([session], read_turn)?;
        rows.map(|row| row?).collect()
    }
}

/// Threads converted per pass of [`convert_turn_rows`].
const CONVERT_BATCH: i64 = 1_000;

/// One `started`, `usage` or `completed` row an earlier lf kept, from either
/// `loopflow.db` or `history.db`.
#[derive(Debug)]
struct LegacyRow {
    turn: String,
    kind: String,
    observed_at: i64,
    lf_process_id: Option<String>,
    agent_process_id: Option<String>,
    task_id: Option<String>,
    wave_id: Option<String>,
    captured_event: Option<i64>,
    payload: Value,
}

/// Turn every `started`, `usage` and `completed` row an earlier lf kept,
/// in `loopflow.db` or `history.db`, into one `usage.turns` row per turn,
/// then delete the rows it converted; returns how many turns it wrote. A
/// turn that already has a row keeps what it has. Safe to run again: a pass
/// cut short converts the rest next time.
///
/// # Errors
/// When the store or a journal cannot be opened, read or written.
pub fn convert_turn_rows(database: &std::path::Path) -> StoreResult<u64> {
    let conn = rusqlite::Connection::open(database)?;
    conn.busy_timeout(std::time::Duration::from_secs(15))?;
    crate::history::attach(&conn, database);
    let history = crate::history::HistoryFile::beside(&crate::history::HISTORY, Some(database));
    let usage = crate::history::HistoryFile::beside(&crate::history::USAGE, Some(database));
    let mut converted = 0;
    loop {
        let threads: Vec<(String, String)> = conn
            .prepare(
                "SELECT session_id,provider_thread FROM session_events
                     WHERE kind IN ('started','usage','completed') AND provider_thread IS NOT NULL
                 UNION SELECT session_id,provider_thread FROM history.session_history
                     WHERE kind IN ('started','usage','completed') AND provider_thread IS NOT NULL
                 LIMIT ?1",
            )?
            .query_map([CONVERT_BATCH], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        if threads.is_empty() {
            return Ok(converted);
        }
        for (session, thread) in &threads {
            converted += convert_thread(&conn, &usage, session, thread)?;
        }
        let delete = "DELETE FROM session_history WHERE kind IN ('started','usage','completed')
             AND session_id=?1 AND provider_thread=?2";
        history
            .write("turn conversion", |tx| {
                for (session, thread) in &threads {
                    tx.execute(delete, params![session, thread])?;
                }
                Ok(())
            })
            .ok_or_else(|| StoreError::InvalidData("history.db is unavailable".into()))?;
        let tx =
            rusqlite::Transaction::new_unchecked(&conn, rusqlite::TransactionBehavior::Immediate)?;
        for (session, thread) in &threads {
            tx.execute(
                "DELETE FROM session_events WHERE kind IN ('started','usage','completed')
                 AND session_id=?1 AND provider_thread=?2",
                params![session, thread],
            )?;
        }
        tx.commit()?;
    }
}

/// Write the turns of one Session thread from its legacy rows.
fn convert_thread(
    conn: &rusqlite::Connection,
    usage: &crate::history::HistoryFile,
    session: &str,
    thread: &str,
) -> StoreResult<u64> {
    let mut rows: Vec<LegacyRow> = Vec::new();
    for table in ["session_events", "history.session_history"] {
        let mut query = conn.prepare(&format!(
            "SELECT COALESCE(provider_turn,''),kind,observed_at,lf_process_id,agent_process_id,
                    task_id,wave_id,captured_event,payload,seq
             FROM {table} WHERE session_id=?1 AND provider_thread=?2
               AND kind IN ('started','usage','completed') ORDER BY seq"
        ))?;
        let found = query.query_map(params![session, thread], |row| {
            Ok((
                LegacyRow {
                    turn: row.get(0)?,
                    kind: row.get(1)?,
                    observed_at: row.get(2)?,
                    lf_process_id: row.get(3)?,
                    agent_process_id: row.get(4)?,
                    task_id: row.get(5)?,
                    wave_id: row.get(6)?,
                    captured_event: row.get(7)?,
                    payload: Value::Null,
                },
                row.get::<_, String>(8)?,
            ))
        })?;
        for row in found {
            let (mut row, payload) = row?;
            row.payload = serde_json::from_str(&payload).unwrap_or(Value::Null);
            rows.push(row);
        }
    }
    // Rows of two files have no shared sequence; time orders the thread.
    rows.sort_by_key(|row| row.observed_at);
    let mut turns: Vec<&str> = Vec::new();
    for row in &rows {
        if !row.turn.is_empty() && !turns.contains(&row.turn.as_str()) {
            turns.push(&row.turn);
        }
    }
    let thread_id: AgentSessionId = thread.into();
    let mut prior: Option<Value> = None;
    let mut written = 0;
    for turn in turns {
        let of = |kind: &'static str| {
            rows.iter()
                .filter(move |row| row.turn == turn && row.kind == kind)
        };
        let started = of("started").next();
        let completed = of("completed").next();
        let readings: Vec<&LegacyRow> = of("usage").collect();
        let recorded = resolve_readings(&readings, prior.as_ref());
        // A completed Codex turn's last total is where the next one begins.
        prior = readings
            .iter()
            .rev()
            .find(|row| row.payload["total"].is_object())
            .map(|row| row.payload["total"].clone())
            .filter(|_| completed.is_some());
        let grouping = match started.and_then(|row| row.captured_event.map(|c| (c, row))) {
            Some((captured, row)) => {
                dispatch_grouping(conn, session, captured, row.task_id.as_deref())?
            }
            None => Default::default(),
        };
        let wrote = usage.write("turn conversion", |tx| {
            let id = ensure_turn(tx, session, &thread_id, turn)?;
            if let Some(row) = started {
                tx.execute(
                    "UPDATE turns SET started_at=COALESCE(started_at,?2),
                        lf_process_id=COALESCE(lf_process_id,?3),
                        agent_process_id=COALESCE(agent_process_id,?4),
                        task_id=COALESCE(task_id,?5),wave_id=COALESCE(wave_id,?6),
                        captured_event=COALESCE(captured_event,?7),
                        project_id=COALESCE(project_id,?8),repository=COALESCE(repository,?9),
                        skill=COALESCE(skill,?10),provider=COALESCE(provider,?11),
                        graph=COALESCE(graph,?12),step=COALESCE(step,?13)
                     WHERE id=?1",
                    params![
                        id,
                        row.observed_at,
                        row.lf_process_id,
                        row.agent_process_id,
                        row.task_id,
                        row.wave_id,
                        row.captured_event,
                        grouping.project_id,
                        grouping.repository,
                        grouping.skill,
                        grouping.provider,
                        grouping.graph,
                        grouping.step,
                    ],
                )?;
            }
            let kept: (bool, bool) = tx.query_row(
                "SELECT usage_at IS NOT NULL,ended_at IS NOT NULL FROM turns WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
            if let (false, Some((usage, provider))) = (kept.0, &recorded) {
                write_usage_in(tx, id, usage, provider.as_deref())?;
            }
            if let (false, Some(row)) = (kept.1, completed) {
                write_completion_in(tx, id, row.observed_at, &row.payload)?;
            }
            Ok(())
        });
        if wrote.is_none() {
            return Err(StoreError::InvalidData("usage.db is unavailable".into()));
        }
        written += 1;
    }
    Ok(written)
}

/// One turn's usage from the readings an earlier lf kept of it. Codex's are
/// cumulative, so the last stands; OpenCode's are one per model call, so each
/// call's last is summed.
fn resolve_readings(
    readings: &[&LegacyRow],
    prior: Option<&Value>,
) -> Option<(RecordedUsage, Option<String>)> {
    let last = readings.last()?;
    let provider = last.payload["provider"].as_str().map(str::to_owned);
    if !last.payload["message"]["tokens"].is_object() {
        let usage = crate::session_record::resolve_usage(&last.payload, prior, last.observed_at)?;
        return Some((usage, provider));
    }
    let mut calls: Vec<(&str, RecordedUsage)> = Vec::new();
    for row in readings {
        let Some(message) = row.payload["message"]["id"].as_str() else {
            continue;
        };
        let Some(usage) = crate::session_record::resolve_usage(&row.payload, None, row.observed_at)
        else {
            continue;
        };
        calls.retain(|(known, _)| *known != message);
        calls.push((message, usage));
    }
    let (_, first) = calls.first()?;
    let mut total = first.clone();
    let add = |a: Option<u64>, b: Option<u64>| match (a, b) {
        (Some(a), Some(b)) => a.checked_add(b),
        (a, b) => a.or(b),
    };
    for (_, call) in &calls[1..] {
        let (sum, next) = (&mut total.usage, &call.usage);
        sum.input_tokens = add(sum.input_tokens, next.input_tokens);
        sum.output_tokens = add(sum.output_tokens, next.output_tokens);
        sum.total_input_tokens = add(sum.total_input_tokens, next.total_input_tokens);
        sum.peak_input_tokens = sum.peak_input_tokens.max(next.peak_input_tokens);
        sum.reasoning_tokens = add(sum.reasoning_tokens, next.reasoning_tokens);
        sum.cache_read_tokens = add(sum.cache_read_tokens, next.cache_read_tokens);
        sum.cache_write_tokens = add(sum.cache_write_tokens, next.cache_write_tokens);
        sum.cost_usd = match (sum.cost_usd, next.cost_usd) {
            (Some(a), Some(b)) => Some(a + b),
            (a, b) => a.or(b),
        };
        sum.model = next.model.clone().or(sum.model.take());
        total.observed_at = call.observed_at;
    }
    Some((total, provider))
}

/// What a converted turn's input was dispatched for, read from the store.
#[derive(Debug, Default)]
struct Grouping {
    project_id: Option<String>,
    repository: Option<String>,
    skill: Option<String>,
    provider: Option<String>,
    graph: Option<String>,
    step: Option<String>,
}

fn dispatch_grouping(
    conn: &rusqlite::Connection,
    session: &str,
    captured: i64,
    task: Option<&str>,
) -> StoreResult<Grouping> {
    Ok(conn
        .query_row(
            "SELECT (SELECT project_id FROM tasks WHERE id=?3),s.repo,
                    COALESCE(json_extract(c.payload,'$.skill'),s.skill),
                    COALESCE(json_extract(c.payload,'$.provider'),s.provider),g.graph,
                    (SELECT json_extract(step.value,'$.label') FROM json_each(g.compiled,'$.steps') step
                     WHERE json_extract(step.value,'$.key')=gs.node)
             FROM agent_sessions s
             LEFT JOIN session_events c ON c.seq=?2 AND c.kind='captured'
             LEFT JOIN graph_process_steps gs ON gs.lf_process_id=c.lf_process_id
             LEFT JOIN graph_processes g ON g.lf_process_id=gs.graph_lf_process_id
             WHERE s.id=?1",
            params![session, captured, task],
            |row| {
                Ok(Grouping {
                    project_id: row.get(0)?,
                    repository: row.get(1)?,
                    skill: row.get(2)?,
                    provider: row.get(3)?,
                    graph: row.get(4)?,
                    step: row.get(5)?,
                })
            },
        )
        .optional()?
        .unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::sqlite::SqliteStore;

    fn store() -> (tempfile::TempDir, SqliteStore) {
        let home = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&home.path().join("loopflow.db")).unwrap();
        store.test_session("conversation", &crate::session_record::new_artifact_key());
        (home, store)
    }

    #[test]
    fn a_running_turn_is_read_from_its_one_row() {
        let (_home, store) = store();
        let thread = "thread".into();
        store.record_turn_start("conversation", &thread, "turn", 10);
        let running = store.session_turns("conversation").unwrap();
        assert_eq!(running.len(), 1);
        assert_eq!(
            (running[0].started_at, running[0].ended_at),
            (Some(10), None)
        );
        assert_eq!(
            store
                .recorded_turn_end("conversation", &thread, "turn")
                .unwrap(),
            None
        );

        store
            .record_turn_usage(
                "conversation",
                &thread,
                "turn",
                12,
                &json!({"provider":"claude","final":true,"usage":{"input_tokens":3,"output_tokens":4,"model":"opus"}}),
            )
            .unwrap();
        store
            .record_turn_completion(
                "conversation",
                &thread,
                "turn",
                12,
                &json!({"status":"completed","context":{"first_request_tokens":3,
                    "peak_request_tokens":3,"tool_results":1,"tool_output_tokens":5,
                    "compactions":0,"compaction_tokens":null}}),
            )
            .unwrap();
        let ended = store.session_turns("conversation").unwrap();
        assert_eq!(ended.len(), 1, "start, usage and end are one row");
        let turn = &ended[0];
        assert_eq!(
            (turn.ended_at, turn.status.as_deref()),
            (Some(12), Some("completed"))
        );
        let usage = turn.usage.as_ref().unwrap();
        assert_eq!(
            (usage.usage.input_tokens, usage.usage.output_tokens),
            (Some(3), Some(4))
        );
        assert_eq!(usage.usage.model.as_deref(), Some("opus"));
        assert_eq!(turn.provider.as_deref(), Some("claude"));
        assert_eq!(turn.context.as_ref().unwrap().tool_output_tokens, 5);
        // Nothing of the turn is in Session history.
        let history: i64 = store
            .conn
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM history.session_history", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(history, 0);
    }

    #[test]
    fn a_codex_reading_of_the_thread_is_kept_as_the_turns_own_counts() {
        let (_home, store) = store();
        let thread = "thread".into();
        store.record_turn_start("conversation", &thread, "second", 10);
        store
            .record_turn_usage(
                "conversation",
                &thread,
                "second",
                12,
                &json!({
                    "baseline":{"inputTokens":100,"cachedInputTokens":40,"outputTokens":10,"reasoningOutputTokens":2},
                    "total":{"inputTokens":160,"cachedInputTokens":70,"outputTokens":25,"reasoningOutputTokens":5},
                    "last":{"inputTokens":60,"cachedInputTokens":30,"outputTokens":15,"reasoningOutputTokens":3},
                    "modelContextWindow":1000,"peakInputTokens":60
                }),
            )
            .unwrap();
        let usage = store.session_turns("conversation").unwrap()[0]
            .usage
            .clone()
            .unwrap();
        assert_eq!(usage.usage.total_input_tokens, Some(60));
        assert_eq!(usage.usage.cache_read_tokens, Some(30));
        assert_eq!(usage.usage.input_tokens, Some(30));
        assert_eq!(usage.usage.output_tokens, Some(15));
        assert_eq!(usage.usage.reasoning_tokens, Some(3));
        assert_eq!(usage.usage.context_window_tokens, Some(1000));
        assert!(!usage.partial);
    }

    #[test]
    fn a_second_different_reading_is_a_gap_that_keeps_the_first() {
        let (_home, store) = store();
        let thread = "thread".into();
        let reading = |tokens: u64| json!({"final":true,"usage":{"output_tokens":tokens}});
        let write = |tokens| {
            store
                .record_turn_usage("conversation", &thread, "turn", 1, &reading(tokens))
                .unwrap()
        };
        assert!(write(5));
        assert!(!write(5));
        assert!(!store.usage.gap_open());
        assert!(!write(9));
        assert!(store.usage.gap_open());
        let kept = store.session_turns("conversation").unwrap()[0]
            .usage
            .clone()
            .unwrap();
        assert_eq!(kept.usage.output_tokens, Some(5));
    }

    #[test]
    fn a_locked_usage_file_is_a_gap_not_a_failed_turn() {
        let (home, store) = store();
        let thread = "thread".into();
        store.record_turn_start("conversation", &thread, "first", 1);
        let mut other = rusqlite::Connection::open(crate::history::usage_path(
            &home.path().join("loopflow.db"),
        ))
        .unwrap();
        let held = other
            .transaction_with_behavior(rusqlite::TransactionBehavior::Exclusive)
            .unwrap();
        store.record_turn_start("conversation", &thread, "missed", 2);
        assert!(store
            .record_turn_completion(
                "conversation",
                &thread,
                "first",
                3,
                &json!({"status":"completed"})
            )
            .is_ok());
        assert!(store.usage.gap_open());
        // The store itself keeps answering writes while usage is gapped.
        store
            .conn
            .lock()
            .unwrap()
            .execute("UPDATE store_revisions SET revision=revision+1", [])
            .unwrap();
        held.rollback().unwrap();

        store.record_turn_start("conversation", &thread, "after", 4);
        assert!(!store.usage.gap_open());
        let turns: Vec<String> = store
            .session_turns("conversation")
            .unwrap()
            .into_iter()
            .map(|turn| turn.turn)
            .collect();
        assert_eq!(turns, ["first", "after"]);
    }

    /// A released lf kept a turn as `started`, `usage` and `completed` rows,
    /// in `loopflow.db` or (on a branch build) `history.db`. Conversion makes
    /// each one turn and reads what the three rows said.
    #[test]
    fn earlier_turn_rows_become_one_turn_each() {
        let home = tempfile::tempdir().unwrap();
        let database = home.path().join("loopflow.db");
        // A history file from before turns moved out, which kept them.
        rusqlite::Connection::open(crate::history::history_path(&database))
            .unwrap()
            .execute_batch(
                "CREATE TABLE session_history (seq INTEGER PRIMARY KEY, session_id TEXT NOT NULL,
                    provider_thread TEXT, provider_turn TEXT,
                    kind TEXT NOT NULL CHECK(kind IN ('started','usage','completed','observed')),
                    receipt_key TEXT NOT NULL, lf_process_id TEXT, agent_process_id TEXT,
                    task_id TEXT, wave_id TEXT, captured_event INTEGER,
                    observed_at INTEGER NOT NULL, payload TEXT NOT NULL);
                 PRAGMA user_version=1;",
            )
            .unwrap();
        let store = crate::test_fixture::open(&database).unwrap();
        let input = crate::session_record::new_artifact_key();
        let session = store.test_session("conversation", &input);
        let at: i64 = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT observed_at FROM session_events WHERE seq=?1",
                [session.captured],
                |row| row.get(0),
            )
            .unwrap();
        let codex = |base: u64, total: u64| {
            json!({"total":{"inputTokens":total,"cachedInputTokens":0,"outputTokens":total/10,
                "reasoningOutputTokens":0},"last":{"inputTokens":total-base,"cachedInputTokens":0,
                "outputTokens":(total-base)/10,"reasoningOutputTokens":0}})
            .to_string()
        };
        let rows = [
            ("first", "started", at + 10, "{}".to_owned()),
            ("first", "usage", at + 20, codex(0, 100)),
            (
                "first",
                "completed",
                at + 30,
                r#"{"status":"completed"}"#.to_owned(),
            ),
            ("second", "started", at + 40, "{}".to_owned()),
            ("second", "usage", at + 50, codex(100, 160)),
            (
                "second",
                "completed",
                at + 60,
                r#"{"status":"failed"}"#.to_owned(),
            ),
        ];
        let insert = "INSERT INTO {t}(session_id,provider_thread,provider_turn,kind,receipt_key,
                captured_event,observed_at,payload) VALUES('conversation','thread',?1,?2,'',?3,?4,?5)";
        {
            let conn = store.conn.lock().unwrap();
            for (turn, kind, observed, payload) in &rows[..3] {
                conn.execute(
                    &insert.replace("{t}", "session_events"),
                    rusqlite::params![
                        turn,
                        kind,
                        (*kind == "started").then_some(session.captured),
                        observed,
                        payload
                    ],
                )
                .unwrap();
            }
        }
        for (turn, kind, observed, payload) in &rows[3..] {
            store
                .execute_history(
                    &insert.replace("{t}", "session_history"),
                    rusqlite::params![
                        turn,
                        kind,
                        (*kind == "started").then_some(session.captured),
                        observed,
                        payload
                    ],
                )
                .unwrap();
        }

        assert_eq!(super::convert_turn_rows(&database).unwrap(), 2);
        assert_eq!(super::convert_turn_rows(&database).unwrap(), 0);

        let turns = store.session_turns("conversation").unwrap();
        let read: Vec<_> = turns
            .iter()
            .map(|turn| {
                let usage = &turn.usage.as_ref().unwrap();
                (
                    turn.turn.as_str(),
                    turn.started_at,
                    turn.ended_at,
                    turn.status.as_deref(),
                    turn.captured_event,
                    usage.usage.total_input_tokens,
                    usage.usage.output_tokens,
                    usage.partial,
                )
            })
            .collect();
        assert_eq!(
            read,
            [
                (
                    "first",
                    Some(at + 10),
                    Some(at + 30),
                    Some("completed"),
                    session.captured,
                    Some(100),
                    Some(10),
                    false
                ),
                // The second counts from where the first left the thread.
                (
                    "second",
                    Some(at + 40),
                    Some(at + 60),
                    Some("failed"),
                    session.captured,
                    Some(60),
                    Some(6),
                    false
                ),
            ]
        );
        // The input reads its turns as the rows said: it ended with the last.
        let pending = store
            .pending_inputs(None, None, None, None, (0, true, None), Some(&input))
            .unwrap();
        assert_eq!(pending.inputs.len(), 1);
        assert_eq!(pending.inputs[0].latest, at + 60);
        assert_eq!(
            store
                .recorded_turn_end("conversation", &"thread".into(), "second")
                .unwrap(),
            Some((at + 60, Some("failed".to_owned())))
        );
        // What was converted is gone from both files.
        let conn = store.conn.lock().unwrap();
        let left = |sql: &str| -> i64 { conn.query_row(sql, [], |row| row.get(0)).unwrap() };
        assert_eq!(
            left("SELECT count(*) FROM session_events WHERE kind!='captured'"),
            0
        );
        assert_eq!(left("SELECT count(*) FROM history.session_history"), 0);
    }
}
