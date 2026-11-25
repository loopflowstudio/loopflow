//! Context callbacks carry provider authority, independently of the attached client.
use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use super::SqliteStore;
use crate::process::SessionAttachment;
use crate::steer::{
    IdleCandidate, IdleEndpoints, Inbox, NativeBoundary, SessionSteerPage, SessionSteerStatus,
    Steer, SteerAdapter, SteerWake,
};
use crate::{StoreError, StoreResult};

impl SqliteStore {
    pub fn reserve_steer_adapter(
        &self,
        session: &str,
        expected: &SessionAttachment,
    ) -> StoreResult<SteerAdapter> {
        self.with_session_attachment(session, expected, || {
            let conn = self.conn.lock().expect("store mutex poisoned");
            conn.execute(
                "INSERT INTO steer_adapters(agent_process_id,capability,created_at)
                 VALUES(?1,?2,unixepoch()) ON CONFLICT DO NOTHING",
                params![expected.agent_process_id, uuid::Uuid::new_v4().to_string()],
            )?;
            Ok(SteerAdapter {
                session: session.into(),
                agent_process_id: expected.agent_process_id.clone(),
                capability: conn.query_row(
                    "SELECT capability FROM steer_adapters WHERE agent_process_id=?1",
                    [&expected.agent_process_id],
                    |row| row.get(0),
                )?,
            })
        })
    }

    /// The Session's selected native conversation is the one record callbacks
    /// follow. A provider start in the owned process selects; activation sets
    /// only a missing selection; every other boundary must match it.
    pub fn with_provider_context<T>(
        &self,
        adapter: &SteerAdapter,
        thread: &str,
        boundary: NativeBoundary<'_>,
        write: impl FnOnce() -> StoreResult<T>,
    ) -> StoreResult<T> {
        let _fence = self.lock_session_dispatch(&adapter.session)?;
        // A provider start that changes conversation also moves the resume
        // reference, on the account it already names. A launch whose provider
        // names the conversation (Antigravity) has published none yet, and a
        // held terminal leaves no lf process to publish it later: the start
        // does.
        let resume = match boundary {
            NativeBoundary::Select { .. } => self.resume_input(&adapter.session)?,
            _ => None,
        };
        // A turn in the selected, activated, already observed conversation
        // changes nothing; it takes no write lock. Any other answer goes
        // through the transaction, which checks again.
        if matches!(boundary, NativeBoundary::Turn) && !thread.is_empty() {
            let conn = self.conn.lock().expect("store mutex poisoned");
            let current: Option<bool> = conn
                .query_row(
                    "SELECT s.provider_thread IS ?4 AND a.activated_at IS NOT NULL FROM steer_adapters a
                 JOIN agent_sessions s ON s.agent_process_id=a.agent_process_id
                 WHERE s.id=?1 AND a.agent_process_id=?2 AND a.capability=?3
                 AND s.agent_ended_at IS NULL",
                    params![
                        adapter.session,
                        adapter.agent_process_id,
                        adapter.capability,
                        thread
                    ],
                    |row| row.get(0),
                )
                .optional()?;
            if current == Some(true)
                && super::provider_transitions::observed_in(&conn, &adapter.session, thread)?
            {
                drop(conn);
                return write();
            }
        }
        {
            let mut conn = self.conn.lock().expect("store mutex poisoned");
            let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
            let selected: Option<Option<String>> = tx
                .query_row(
                    "SELECT s.provider_thread FROM steer_adapters a
                 JOIN agent_sessions s ON s.agent_process_id=a.agent_process_id
                 WHERE s.id=?1 AND a.agent_process_id=?2 AND a.capability=?3
                 AND s.agent_ended_at IS NULL",
                    params![
                        adapter.session,
                        adapter.agent_process_id,
                        adapter.capability
                    ],
                    |row| row.get(0),
                )
                .optional()?;
            let selected = selected.ok_or_else(|| {
                StoreError::InvalidAuthority("Context adapter no longer owns this provider".into())
            })?;
            if thread.is_empty() {
                return Err(StoreError::InvalidAuthority(
                    "Context callback names no native conversation".into(),
                ));
            }
            match (boundary, selected.as_deref()) {
                (_, Some(current)) if current == thread => {}
                (NativeBoundary::Select { .. }, _) | (NativeBoundary::Activate { .. }, None) => {
                    tx.execute(
                        "UPDATE agent_sessions SET provider_thread=?2 WHERE id=?1",
                        params![adapter.session, thread],
                    )?;
                }
                (_, Some(current)) => {
                    return Err(StoreError::InvalidAuthority(format!(
                        "Context callback names native conversation {thread}; this Session selects {current}"
                    )));
                }
                (NativeBoundary::Turn, None) => {
                    return Err(StoreError::InvalidAuthority(format!(
                        "Context callback names native conversation {thread}; this Session selects none yet"
                    )));
                }
            }
            let source = match boundary {
                NativeBoundary::Select { source } | NativeBoundary::Activate { source } => source,
                _ => "observed",
            };
            let mut history = Vec::from_iter(super::provider_transitions::observe_in(
                &tx,
                &adapter.session,
                thread,
                source,
            )?);
            if let Some((session, input, reference)) = resume.filter(|(_, _, reference)| {
                reference
                    .as_ref()
                    .is_none_or(|reference| reference.agent_session.as_str() != thread)
            }) {
                history.extend(super::sessions::retain_history_in(
                    &tx,
                    &session,
                    &[crate::session_record::provider_session_observation(
                        &session,
                        &input,
                        &thread.into(),
                        reference.and_then(|reference| reference.account_id),
                    )],
                )?);
            }
            tx.execute(
                "UPDATE steer_adapters SET activated_at=COALESCE(activated_at,unixepoch()) WHERE agent_process_id=?1",
                [&adapter.agent_process_id],
            )?;
            tx.commit()?;
            drop(conn);
            self.append_history("provider transition", &history);
        }
        write()
    }

    /// Native hooks report turn boundaries and work that outlives a reply.
    /// A retained provider capability cannot report for a replacement
    /// provider or a different native conversation.
    pub fn record_native_turn(
        &self,
        adapter: &SteerAdapter,
        thread: &str,
        yielded: bool,
        open_tools: Option<usize>,
    ) -> StoreResult<()> {
        self.with_provider_context(adapter, thread, NativeBoundary::Turn, || {
            let (native, previous_work, started_since_turn): (bool, i64, bool) =
                self.conn.lock().expect("store mutex poisoned").query_row(
                    "SELECT s.interactive, COALESCE(act.open_tools,0),
                            s.starts_since_turn>0 OR s.last_started_at IS NOT NULL
                     FROM agent_sessions s
                     LEFT JOIN session_activity act
                       ON act.agent_process_id=s.agent_process_id AND act.session_id=s.id
                     WHERE s.id=?2 AND s.agent_process_id=?1",
                    params![adapter.agent_process_id, adapter.session],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?;
            // A headless run reports through the stream lf owns. A person's
            // conversation reports here, Codex's included: its AgentProcess is
            // an app-server lf spawned, but a native client takes the turns.
            if !native {
                return Ok(());
            }
            let attachment = self.session_attachment(&adapter.session)?.ok_or_else(|| {
                StoreError::InvalidAuthority("Native provider has no attachment".into())
            })?;
            if yielded && started_since_turn {
                // A finished turn is a conversation that runs: the recovery
                // pass counts its starts again from none, and starts the
                // first death after it at once.
                self.conn.lock().expect("store mutex poisoned").execute(
                    "UPDATE agent_sessions SET starts_since_turn=0,last_started_at=NULL
                     WHERE id=?1 AND (starts_since_turn>0 OR last_started_at IS NOT NULL)",
                    [&adapter.session],
                )?;
            }
            self.record_session_activity(
                &adapter.session,
                &attachment,
                &crate::session::SessionActivity {
                    preview: None,
                    observed_at: time::OffsetDateTime::now_utc().unix_timestamp(),
                    // An unavailable task registry cannot settle known work.
                    open_tools: open_tools.unwrap_or(previous_work as usize),
                    pending_input: 0,
                    yielded,
                },
            )
        })
    }

    /// Record a person's conversation as idle at its provider process's first
    /// boundary, whatever the process before it on this Session was doing: a
    /// new provider runs no turn until one starts. True when this boundary was
    /// the first. A held terminal's status stream may already have moved the
    /// reading to this process; with no turn read yet it is still 0. A
    /// headless run reports through the stream lf owns.
    pub fn record_first_boundary(&self, adapter: &SteerAdapter) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // Only a boundary that will write takes the Machine's write lock: a
        // later boundary, a worker or a process with no steer capability reads.
        let first: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_sessions s
                WHERE s.id=?1 AND s.agent_process_id=?2 AND s.interactive=1
                  AND EXISTS(SELECT 1 FROM steer_adapters a
                    WHERE a.agent_process_id=?2 AND a.capability=?3)
                  AND NOT EXISTS(SELECT 1 FROM session_activity
                    WHERE session_id=?1 AND agent_process_id=?2 AND observed_at<>0))",
            params![
                adapter.session,
                adapter.agent_process_id,
                adapter.capability
            ],
            |row| row.get(0),
        )?;
        if !first {
            return Ok(false);
        }
        // The write repeats only what another connection can change between
        // the read and it: which process is current, and its own reading.
        let changed = conn.execute(
            "INSERT INTO session_activity(session_id,attachment_token,observed_at,open_tools,pending_input,yielded,agent_process_id)
             SELECT s.id,s.attachment_token,unixepoch(),0,0,1,s.agent_process_id FROM agent_sessions s
             WHERE s.id=?1 AND s.agent_process_id=?2
             ON CONFLICT(session_id) DO UPDATE SET agent_process_id=excluded.agent_process_id,
                yielded=1,open_tools=0,pending_input=0,observed_at=excluded.observed_at,
                program_status=CASE WHEN session_activity.agent_process_id IS excluded.agent_process_id THEN session_activity.program_status END,
                cpu_sampled_at=NULL,cpu_quiet_since=NULL,
                attachment_token=COALESCE(excluded.attachment_token,session_activity.attachment_token)
             WHERE session_activity.agent_process_id IS NOT excluded.agent_process_id OR session_activity.observed_at=0",
            params![adapter.session, adapter.agent_process_id],
        )?;
        Ok(changed > 0)
    }

    /// Save where an idle conversation takes messages: Claude's inbox key in
    /// `agent_inbox`, an OpenCode or Pi socket in `agent_socket`. Written only
    /// when it changes, which is the answer.
    pub fn record_agent_inbox(&self, adapter: &SteerAdapter, inbox: &Inbox) -> StoreResult<bool> {
        let (column, path) = match inbox {
            Inbox::Claude(path) => ("agent_inbox", path),
            Inbox::Socket(path) => ("agent_socket", path),
        };
        let conn = self.conn.lock().expect("store mutex poisoned");
        let changed = conn.execute(
            &format!(
                "UPDATE agent_sessions SET {column}=?3 WHERE id=?1
                    AND agent_process_id=?2 AND {column} IS NOT ?3 AND EXISTS(SELECT 1
                    FROM steer_adapters a WHERE a.agent_process_id=?2 AND a.capability=?4)"
            ),
            params![
                adapter.session,
                adapter.agent_process_id,
                path.to_string_lossy(),
                adapter.capability
            ],
        )?;
        Ok(changed > 0)
    }

    /// Idle managed conversations, restricted to unfinished agents and, when
    /// given, one Session. The harness chooses which endpoint it can use.
    pub fn idle_conversation_candidates(
        &self,
        session: Option<&str>,
    ) -> StoreResult<Vec<IdleCandidate>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT s.id,s.agent_process_id,a.capability,s.provider_thread,s.cwd,
                s.provider,s.agent_endpoint,s.agent_inbox,s.agent_socket,s.held_terminal_id,
                s.agent_pid,s.agent_os_started_at
             FROM agent_sessions s INDEXED BY agent_sessions_unended_agent
             JOIN steer_adapters a ON a.agent_process_id=s.agent_process_id
             JOIN session_activity act ON act.session_id=s.id AND act.agent_process_id=s.agent_process_id
             WHERE s.agent_process_id IS NOT NULL AND s.agent_ended_at IS NULL
               AND act.yielded=1 AND s.interactive=1
               AND (s.provider_thread IS NOT NULL OR s.held_terminal_id IS NOT NULL)
               AND (?1 IS NULL OR s.id=?1)",
        )?;
        let rows = statement.query_map([session], |row| {
            let path = |column| {
                row.get::<_, Option<String>>(column)
                    .map(|path| path.map(Into::into))
            };
            Ok(IdleCandidate {
                adapter: SteerAdapter {
                    session: row.get(0)?,
                    agent_process_id: row.get(1)?,
                    capability: row.get(2)?,
                },
                thread: row.get(3)?,
                cwd: row.get::<_, String>(4)?.into(),
                provider: row.get(5)?,
                endpoints: IdleEndpoints {
                    endpoint: path(6)?,
                    inbox: path(7)?,
                    socket: path(8)?,
                    held_terminal: row.get(9)?,
                },
                identity: row.get::<_, Option<u32>>(10)?.zip(row.get(11)?),
            })
        })?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// A woken conversation is in a turn until its next Stop says otherwise.
    pub fn record_woken(&self, adapter: &SteerAdapter) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE session_activity SET yielded=0,observed_at=unixepoch()
             WHERE session_id=?1 AND agent_process_id=?2",
            params![adapter.session, adapter.agent_process_id],
        )?;
        Ok(())
    }

    /// The Session's current input with the resume reference published for
    /// it, if any.
    fn resume_input(
        &self,
        session: &str,
    ) -> StoreResult<
        Option<(
            crate::session::LfSession,
            String,
            Option<crate::session_record::ProviderSessionRef>,
        )>,
    > {
        let Some(session) = self.session(session)? else {
            return Ok(None);
        };
        let Some(input) = session.artifact_key.clone() else {
            return Ok(None);
        };
        let reference = self.input_provider_session(&input)?;
        Ok(Some((session, input, reference)))
    }
}

fn invalid(message: impl ToString) -> StoreError {
    StoreError::InvalidData(message.to_string())
}

pub(super) fn read_steer(row: &rusqlite::Row<'_>) -> rusqlite::Result<Steer> {
    Ok(Steer {
        id: row.get(0)?,
        sequence: row.get(1)?,
        source: row.get(2)?,
        revision: row.get(3)?,
        author: serde_json::from_str(&row.get::<_, String>(4)?).map_err(|error| {
            rusqlite::Error::FromSqlConversionFailure(
                4,
                rusqlite::types::Type::Text,
                Box::new(error),
            )
        })?,
        text: row.get(5)?,
    })
}
pub(super) const COLUMNS: &str = "p.id,p.seq,p.source,p.revision,p.author,p.text";

/// Callers supply exactly one existing source owner; these columns are private,
/// never a wire target or a second Work hierarchy.
pub(super) fn save_in(
    conn: &Connection,
    owner: (&str, &str),
    source: &str,
    revision: &str,
    author: &loopflow_model::durable::Author,
    text: &str,
) -> StoreResult<Steer> {
    if text.trim().is_empty() {
        return Err(invalid("Steer text cannot be empty"));
    }
    let column = match owner.0 {
        "task" => "task_id",
        "wave" => "wave_id",
        "repository" => "repo",
        "session" => "session_id",
        _ => return Err(invalid("Unsupported steer owner")),
    };
    conn.execute(
        &format!(
            "INSERT INTO steers(id,{column},source,revision,author,text,created_at)
        VALUES(?1,?2,?3,?4,?5,?6,unixepoch()) ON CONFLICT(source,revision) DO NOTHING"
        ),
        params![
            uuid::Uuid::new_v4().to_string(),
            owner.1,
            source,
            revision,
            serde_json::to_string(author)?,
            text
        ],
    )?;
    // This transaction holds the write lock, so a hook that checks after this
    // touch waits for the commit before claiming pending steers.
    if let Some(home) = conn
        .path()
        .filter(|path| !path.is_empty())
        .and_then(|path| Path::new(path).parent())
    {
        let marker = match owner.0 {
            "task" => crate::steer_markers::task_marker(home, owner.1),
            "session" => crate::steer_markers::session_marker(home, owner.1),
            _ => crate::steer_markers::latest_marker(home),
        };
        crate::steer_markers::touch(&marker)
            .map_err(|error| invalid(format!("Mark saved steer for hooks: {error}")))?;
    }
    let result = conn.query_row(
        &format!("SELECT {COLUMNS} FROM steers p WHERE source=?1 AND revision=?2"),
        params![source, revision],
        read_steer,
    )?;
    let same_owner: bool = conn.query_row(
        &format!("SELECT {column} IS ?2 FROM steers WHERE id=?1"),
        params![result.id, owner.1],
        |row| row.get(0),
    )?;
    if !same_owner || result.text != text || result.author != *author {
        return Err(invalid(
            "Steer revision already has different contents or ownership",
        ));
    }
    Ok(result)
}

/// The steers that apply to Session `?1`, as CTE `applicable`.
fn scopes() -> String {
    // Membership is derived here; receipts alone retain delivery state.
    // Reuse Task inventory's association rule: the Session's recorded Task.
    // UNION terminates even in malformed cyclic ancestry.
    format!("WITH RECURSIVE member_tasks(id) AS MATERIALIZED (
        SELECT s.task_id FROM agent_sessions s WHERE s.id=?1 AND s.task_id IS NOT NULL AND {}
        ), conducted_tasks(id) AS MATERIALIZED (
        SELECT id FROM tasks WHERE parent_task_id IN member_tasks
        ), member_waves(id) AS (
        SELECT wave_id FROM agent_sessions WHERE id=?1
        UNION SELECT pr.wave_id FROM projects pr JOIN tasks t ON t.project_id=pr.id WHERE t.id IN member_tasks
        UNION SELECT w.parent_wave_id FROM waves w JOIN member_waves m ON w.id=m.id WHERE w.parent_wave_id IS NOT NULL
        ), applicable(id) AS (
        SELECT p.id FROM steers p WHERE
        -- A notice (`notice:<Task>:<key>`) is owned by the Task it is about
        -- and read by the conversations that conduct it, never by its own.
        (p.session_id=?1 OR (p.task_id IN member_tasks AND p.source NOT LIKE 'notice:%')
         OR (p.task_id IN conducted_tasks AND p.source LIKE 'notice:%')
         OR p.wave_id IN member_waves
         OR p.repo=(SELECT repo FROM agent_sessions WHERE id=?1))
        AND NOT EXISTS(SELECT 1 FROM steers newer WHERE newer.source=p.source AND newer.seq>p.seq)
        -- A Task update (`update:<writer Session or ->:<key>`) is for the
        -- Task's interactive conversations, never the Session that caused it.
        AND (p.source NOT LIKE 'update:%' AND p.source NOT LIKE 'notice:%'
         OR ((SELECT interactive FROM agent_sessions WHERE id=?1)
             AND p.source NOT LIKE 'update:'||?1||':%'))
        -- A shared steer saved before this Session started and already read
        -- by another Session of its kind was read there; a later Session
        -- starts past it. A Task conversation's reading leaves the next
        -- worker its own copy, and a worker's leaves the conversation one.
        AND (p.session_id=?1 OR p.created_at>=(SELECT created_at FROM agent_sessions WHERE id=?1)
         OR NOT EXISTS(SELECT 1 FROM steer_delivered d JOIN agent_sessions o ON o.id=d.session_id
            WHERE d.steer_id=p.id AND d.session_id!=?1 AND d.delivered_at IS NOT NULL
            AND o.interactive=(SELECT interactive FROM agent_sessions WHERE id=?1)))
        )",
        super::task_work::counts_for_task("s"))
}

/// A claim whose hook never stamped it lapses after this long, and the steer
/// is printed again: a hook has ten seconds to run.
const CLAIM_SECS: i64 = 15;

/// The applicable steers Session `?1` has not read: no row, or a claim that
/// lapsed unstamped.
fn unread() -> String {
    format!(
        "{} SELECT {COLUMNS} FROM applicable a JOIN steers p ON p.id=a.id
        WHERE NOT EXISTS(SELECT 1 FROM steer_delivered d WHERE d.session_id=?1 AND d.steer_id=a.id
            AND (d.delivered_at IS NOT NULL OR d.claimed_at>unixepoch()-{CLAIM_SECS}))",
        scopes()
    )
}

/// The unread steers that name Session `?1`: its own, or its Task's when it
/// is the Task's primary conversation. The server starts a stopped Session
/// for these, and its start takes its unread steers when one exists.
fn naming() -> String {
    format!(
        "{} AND (p.session_id=?1 OR p.source NOT LIKE 'update:%'
            AND p.task_id IN (SELECT task_id FROM task_primary_sessions WHERE session_id=?1))",
        unread()
    )
}

fn pending_count(conn: &Connection, session: &str) -> StoreResult<i64> {
    Ok(conn.query_row(
        &format!("SELECT count(*) FROM ({})", unread()),
        [session],
        |row| row.get(0),
    )?)
}

impl SqliteStore {
    pub fn save_wave_steer(
        &self,
        wave: &loopflow_model::id::WaveId,
        author: &str,
        text: &str,
    ) -> StoreResult<Steer> {
        self.save_scope_steer(("wave", wave.as_str()), author, text)
    }

    pub fn save_repository_steer(
        &self,
        repo: &crate::address::CanonicalRepo,
        author: &str,
        text: &str,
    ) -> StoreResult<Steer> {
        self.save_scope_steer(("repository", &repo.to_string()), author, text)
    }

    pub fn save_session_steer(
        &self,
        session: &str,
        author: &str,
        text: &str,
    ) -> StoreResult<Steer> {
        self.save_scope_steer(("session", session), author, text)
    }

    /// The steer `source` at `revision` derived, whichever scope owns it.
    pub fn steer_by_source(&self, source: &str, revision: &str) -> StoreResult<Option<Steer>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut stmt = conn.prepare(&format!(
            "SELECT {COLUMNS} FROM steers p WHERE p.source=?1 AND p.revision=?2"
        ))?;
        let steer = stmt
            .query_row(params![source, revision], read_steer)
            .optional()?;
        Ok(steer)
    }

    fn save_scope_steer(
        &self,
        owner: (&str, &str),
        author: &str,
        text: &str,
    ) -> StoreResult<Steer> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let steer = save_in(
            &tx,
            owner,
            &uuid::Uuid::new_v4().to_string(),
            "1",
            &loopflow_model::durable::Author::Named(author.into()),
            text,
        )?;
        // Reads and callbacks reconcile their own Session, including steers
        // saved before it existed. Publishing need not scan unrelated Sessions.
        tx.commit()?;
        Ok(steer)
    }

    /// Whether `session` has a steer to read. Hooks and the server ask often;
    /// nearly always the answer is no, and it takes no write lock.
    pub fn has_undelivered_steers(&self, session: &str) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(
            conn.query_row(&format!("SELECT EXISTS({})", unread()), [session], |row| {
                row.get(0)
            })?,
        )
    }

    /// Every steer that applies to the Session or that it read, oldest first,
    /// with when it read each.
    pub fn session_steers(
        &self,
        session: &str,
        after: i64,
        limit: usize,
    ) -> StoreResult<SessionSteerPage> {
        let limit = limit.clamp(1, 100);
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let exists: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_sessions WHERE id=?1)",
            [session],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(StoreError::NotFound);
        }
        let mut items = tx
            .prepare(&format!(
                "{} SELECT {COLUMNS},d.delivered_at FROM (
                    SELECT id FROM applicable
                    UNION SELECT steer_id FROM steer_delivered WHERE session_id=?1) known
                JOIN steers p ON p.id=known.id
                LEFT JOIN steer_delivered d ON d.session_id=?1 AND d.steer_id=p.id
                WHERE p.seq>?2 ORDER BY p.seq LIMIT ?3",
                scopes()
            ))?
            .query_map(params![session, after, (limit + 1) as i64], |row| {
                Ok(SessionSteerStatus {
                    steer: read_steer(row)?,
                    delivered_at: row.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let next_after = if items.len() > limit {
            items.truncate(limit);
            items.last().map(|item| item.steer.sequence)
        } else {
            None
        };
        Ok(SessionSteerPage {
            items,
            next_after,
            pending: pending_count(&tx, session)?,
        })
    }

    pub fn session_steer_source(
        &self,
        session: &str,
        id: &loopflow_model::id::SteerId,
    ) -> StoreResult<Steer> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            &format!(
                "{} SELECT {COLUMNS} FROM steers p WHERE p.id=?2 AND (p.id IN applicable
                OR EXISTS(SELECT 1 FROM steer_delivered d WHERE d.session_id=?1 AND d.steer_id=p.id))",
                scopes()
            ),
            params![session, id],
            read_steer,
        )
        .optional()?
        .ok_or(StoreError::NotFound)
    }

    /// What the Task looked like when its updates were last written.
    pub fn task_update_reading(
        &self,
        task: &loopflow_model::durable::TaskId,
    ) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn
            .query_row(
                "SELECT observation FROM task_update_readings WHERE task_id=?1",
                [task.as_str()],
                |row| row.get(0),
            )
            .optional()?)
    }

    /// Save the Task's new reading and the steers that report what moved
    /// since `expected`, each as `(source, text)`: updates for its own
    /// conversations and a notice for the ones that conduct it. Nothing when another
    /// reader wrote first: its steers already say it.
    pub fn save_task_update(
        &self,
        task: &loopflow_model::durable::TaskId,
        expected: Option<&str>,
        observation: &str,
        updates: &[(String, String)],
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: Option<String> = tx
            .query_row(
                "SELECT observation FROM task_update_readings WHERE task_id=?1",
                [task.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        if current.as_deref() != expected {
            return Ok(());
        }
        tx.execute(
            "INSERT INTO task_update_readings(task_id,observation) VALUES(?1,?2)
            ON CONFLICT(task_id) DO UPDATE SET observation=excluded.observation",
            params![task.as_str(), observation],
        )?;
        for (source, text) in updates {
            save_in(
                &tx,
                ("task", task.as_str()),
                source,
                "1",
                &loopflow_model::durable::Author::Named("Loopflow".into()),
                text,
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    /// A conversation that just started has its Tasks' history in its prompt
    /// and reads its children's standing itself: the updates and notices
    /// written so far are not news to it.
    pub fn baseline_task_updates(&self, session: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let pending = format!(
            "{} AND (p.source LIKE 'update:%' OR p.source LIKE 'notice:%')",
            unread()
        );
        let any: bool = conn.query_row(&format!("SELECT EXISTS({pending})"), [session], |row| {
            row.get(0)
        })?;
        if any {
            conn.execute(
                &format!(
                    "INSERT INTO steer_delivered(session_id,steer_id,claimed_at,delivered_at)
                    SELECT ?1,id,unixepoch(),unixepoch() FROM ({pending}) WHERE true
                    ON CONFLICT(session_id,steer_id) DO UPDATE SET delivered_at=unixepoch()"
                ),
                [session],
            )?;
        }
        Ok(())
    }

    /// Claim up to eight of the Session's unread steers for the caller to
    /// print, oldest first, with how many more wait behind them. A second hook
    /// at the same moment finds them claimed. The caller stamps them with
    /// [`Self::deliver_session_steers`] once printed; an unstamped claim
    /// lapses.
    pub fn claim_session_steers(
        &self,
        adapter: &SteerAdapter,
        provider_thread: &str,
    ) -> StoreResult<(Vec<Steer>, usize)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let owns: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM agent_sessions s
                JOIN steer_adapters a ON a.agent_process_id=s.agent_process_id
                WHERE s.id=?1 AND s.agent_process_id=?2 AND a.capability=?3 AND s.provider_thread=?4
                AND s.agent_ended_at IS NULL)",
            params![
                adapter.session,
                adapter.agent_process_id,
                adapter.capability,
                provider_thread
            ],
            |row| row.get(0),
        )?;
        if !owns {
            return Err(StoreError::InvalidAuthority(
                "Context adapter no longer owns this provider conversation".into(),
            ));
        }
        let steers = tx
            .prepare(&format!("{} ORDER BY p.seq LIMIT 8", unread()))?
            .query_map([&adapter.session], read_steer)?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for steer in &steers {
            tx.execute(
                "INSERT INTO steer_delivered(session_id,steer_id,claimed_at) VALUES(?1,?2,unixepoch())
                ON CONFLICT(session_id,steer_id) DO UPDATE SET claimed_at=unixepoch()",
                params![adapter.session, steer.id],
            )?;
        }
        let remaining = pending_count(&tx, &adapter.session)?;
        tx.commit()?;
        Ok((steers, remaining as usize))
    }

    /// Save `text` as a steer for `session` and take every steer it has not
    /// read, oldest first and the new one last, as read by the caller's own
    /// turn. One write, so no hook or wake sees them as news in between.
    pub fn take_session_steers(
        &self,
        session: &str,
        author: &str,
        text: &str,
    ) -> StoreResult<Vec<Steer>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        save_in(
            &tx,
            ("session", session),
            &uuid::Uuid::new_v4().to_string(),
            "1",
            &loopflow_model::durable::Author::Named(author.into()),
            text,
        )?;
        let steers = take_unread_in(&tx, session)?;
        tx.commit()?;
        Ok(steers)
    }

    /// The first input of a stopped conversation starting again: every steer
    /// it has not read, oldest first, taken as read, when one of them names
    /// it (its own, or its Task's for the Task's primary conversation).
    /// Otherwise nothing, and its hooks hear the rest at its boundaries.
    pub fn take_steers_to_start(&self, session: &str) -> StoreResult<Vec<Steer>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        // Nearly every start has none; asking takes no write lock.
        let named: bool =
            conn.query_row(&format!("SELECT EXISTS({})", naming()), [session], |row| {
                row.get(0)
            })?;
        if !named {
            return Ok(Vec::new());
        }
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let steers = take_unread_in(&tx, session)?;
        tx.commit()?;
        Ok(steers)
    }

    /// The steers saved since the server last read, each read once: those
    /// that name an interactive Session which has not read them, by the rule
    /// its start takes them with (`take_steers_to_start`), with that
    /// Session's unfinished AgentProcess identity.
    pub fn read_steer_wakes(&self) -> StoreResult<Vec<SteerWake>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let (after, newest): (i64, i64) = conn.query_row(
            "SELECT r.seq,COALESCE((SELECT MAX(seq) FROM steers),r.seq)
             FROM steer_wake_reading r WHERE r.id=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if newest <= after {
            return Ok(Vec::new());
        }
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        // Candidates: the one Session each new steer could name. `naming`
        // decides, so a start never finds nothing to take.
        let candidates = tx
            .prepare(
                "SELECT p.id,s.id,s.agent_pid,s.agent_os_started_at FROM steers p
                 LEFT JOIN task_primary_sessions t ON t.task_id=p.task_id
                 JOIN agent_sessions s ON s.id=COALESCE(p.session_id,t.session_id)
                 WHERE p.seq>?1 AND p.seq<=?2 AND s.interactive=1
                 ORDER BY p.seq",
            )?
            .query_map(params![after, newest], |row| {
                Ok(SteerWake {
                    steer: row.get(0)?,
                    session: row.get(1)?,
                    identity: row
                        .get::<_, Option<u32>>(2)?
                        .zip(row.get::<_, Option<i64>>(3)?),
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut names = tx.prepare(&format!("SELECT EXISTS({} AND p.id=?2)", naming()))?;
        let mut wakes = Vec::new();
        for wake in candidates {
            if names.query_row(params![wake.session, wake.steer], |row| row.get(0))? {
                wakes.push(wake);
            }
        }
        drop(names);
        tx.execute("UPDATE steer_wake_reading SET seq=?1 WHERE id=1", [newest])?;
        tx.commit()?;
        Ok(wakes)
    }

    /// The caller printed these claimed steers: the Session has read them.
    pub fn deliver_session_steers(
        &self,
        session: &str,
        steers: &[loopflow_model::id::SteerId],
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        for steer in steers {
            tx.execute(
                "UPDATE steer_delivered SET delivered_at=unixepoch()
                WHERE session_id=?1 AND steer_id=?2 AND delivered_at IS NULL",
                params![session, steer],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

/// Take every steer `session` has not read as read by the caller, oldest first.
fn take_unread_in(tx: &Connection, session: &str) -> StoreResult<Vec<Steer>> {
    let steers = tx
        .prepare(&format!("{} ORDER BY p.seq", unread()))?
        .query_map([session], read_steer)?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    for read in &steers {
        tx.execute(
            "INSERT INTO steer_delivered(session_id,steer_id,claimed_at,delivered_at)
            VALUES(?1,?2,unixepoch(),unixepoch())
            ON CONFLICT(session_id,steer_id) DO UPDATE
            SET claimed_at=unixepoch(),delivered_at=unixepoch()",
            params![session, read.id],
        )?;
    }
    Ok(steers)
}

#[cfg(test)]
#[path = "steers_tests.rs"]
mod tests;
