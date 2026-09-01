use rusqlite::{params, Connection, OptionalExtension, Transaction, TransactionBehavior};
use time::OffsetDateTime;

use crate::child::ChildRef;
use crate::durable::{
    AbandonReceipt, Author, FlowPosition, Home, HomeId, Placement, ProjectId, RunId, Steer,
    SteerId, SteerReceipt, TaskId, ToolResponseId, ToolResponseReceipt, ToolResponseWrite, WorkRef,
    WorkStatus,
};
use crate::id::WaveId;
use crate::store::rows::now_unix;
use crate::store::{StoreError, StoreResult};
use crate::work::project::Project;
use crate::work::task::Task;

use super::SqliteStore;

impl SqliteStore {
    pub(crate) fn task_issue_identifier(
        &self,
        external_issue_id: &str,
    ) -> StoreResult<Option<String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT issue_identifier FROM tasks WHERE external_issue_id=?1",
            [external_issue_id],
            |row| row.get(0),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn home_by_id(&self, home_id: &HomeId) -> StoreResult<Option<Home>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        map_home_by_id(&conn, home_id)
    }

    pub fn local_home(&self) -> StoreResult<Home> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        map_local_home(&conn)
    }

    pub fn observe_home(&self, home_id: &HomeId, route: &str) -> StoreResult<Home> {
        let route = route.trim();
        if route.is_empty() {
            return Err(StoreError::InvalidData(
                "Home route cannot be empty".to_string(),
            ));
        }
        let conn = self.conn.lock().expect("store mutex poisoned");
        if map_home_by_id(&conn, home_id)?
            .is_some_and(|home| home.route == "local" && route != "local")
        {
            return Err(StoreError::InvalidData(format!(
                "cannot replace local Home {home_id} with remote route {route:?}"
            )));
        }
        let existing_id = conn
            .query_row("SELECT id FROM homes WHERE route=?1", [route], |row| {
                row.get::<_, String>(0)
            })
            .optional()?;
        if existing_id
            .as_deref()
            .is_some_and(|id| id != home_id.as_str())
        {
            return Err(StoreError::InvalidData(format!(
                "Home route {route:?} is already observed for {}",
                existing_id.expect("checked as present")
            )));
        }
        let now = now_unix();
        conn.execute(
            "INSERT INTO homes (id, route, created_at, observed_at)
             VALUES (?1, ?2, ?3, ?3)
             ON CONFLICT(id) DO UPDATE SET
                route=excluded.route, observed_at=excluded.observed_at",
            params![home_id.as_str(), route, now],
        )?;
        map_home_by_id(&conn, home_id)?.ok_or(StoreError::NotFound)
    }

    pub fn placement(&self, work: &WorkRef) -> StoreResult<Placement> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        placement_in(&conn, work)
    }

    pub fn set_work_enabled(&self, work: &WorkRef, enabled: bool) -> StoreResult<Placement> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        placement_in(&tx, work)?;
        let enabled = if enabled { 1_i64 } else { 0_i64 };
        match work {
            WorkRef::Wave(id) => tx.execute(
                "UPDATE work_placements SET enabled=?2 WHERE wave_id=?1",
                params![id.as_str(), enabled],
            )?,
            WorkRef::Project(id) => tx.execute(
                "UPDATE work_placements SET enabled=?2 WHERE project_id=?1",
                params![id.as_str(), enabled],
            )?,
            WorkRef::Task(id) => tx.execute(
                "UPDATE work_placements SET enabled=?2 WHERE task_id=?1",
                params![id.as_str(), enabled],
            )?,
        };
        let placement = placement_in(&tx, work)?;
        tx.commit()?;
        Ok(placement)
    }

    pub(crate) fn place_work(&self, work: &WorkRef, home_id: &HomeId) -> StoreResult<Placement> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_ready_work(&tx, work)?;
        tx.query_row(
            "SELECT 1 FROM homes WHERE id=?1",
            [home_id.as_str()],
            |_| Ok(()),
        )?;
        if let Some(current) = find_placement_in(&tx, work)? {
            if current.home_id == *home_id {
                tx.commit()?;
                return Ok(current);
            }
        }
        write_placement(&tx, work, home_id, now_unix())?;
        let placement = placement_in(&tx, work)?;
        tx.commit()?;
        Ok(placement)
    }

    pub fn set_flow_position(
        &self,
        work: &WorkRef,
        position: &FlowPosition,
    ) -> StoreResult<FlowPosition> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_ready_work(&tx, work)?;
        if &position.work != work {
            return Err(StoreError::InvalidAuthority(
                "flow position does not belong to this Work".to_string(),
            ));
        }
        if position.flow.trim().is_empty() || position.step.trim().is_empty() {
            return Err(StoreError::InvalidData(
                "flow and step cannot be empty".to_string(),
            ));
        }
        if position.human && position.node_id.is_none() {
            return Err(StoreError::InvalidData(
                "human flow positions require a stable node id".to_string(),
            ));
        }
        tx.execute(
            "INSERT INTO work_flow_positions (
                work_kind, work_id, flow, step, node_id, human, session_run_id,
                ready_summary, step_index, iteration, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
             ON CONFLICT(work_kind, work_id) DO UPDATE SET
                flow=excluded.flow, step=excluded.step, node_id=excluded.node_id,
                human=excluded.human, session_run_id=excluded.session_run_id,
                ready_summary=excluded.ready_summary,
                step_index=excluded.step_index, iteration=excluded.iteration,
                updated_at=excluded.updated_at",
            params![
                work.kind(),
                work.id(),
                position.flow,
                position.step,
                position.node_id,
                position.human,
                position.session_run_id.as_ref().map(RunId::as_str),
                position.ready_summary,
                i64::from(position.step_index),
                i64::from(position.iteration),
                position.updated_at.unix_timestamp()
            ],
        )?;
        tx.commit()?;
        Ok(position.clone())
    }

    pub fn flow_position(&self, work: &WorkRef) -> StoreResult<Option<FlowPosition>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let row = conn
            .query_row(
                "SELECT flow, step, node_id, human, session_run_id, ready_summary,
                        step_index, iteration, updated_at
                 FROM work_flow_positions WHERE work_kind=?1 AND work_id=?2",
                params![work.kind(), work.id()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, bool>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, i64>(8)?,
                    ))
                },
            )
            .optional()?;
        row.map(|row| decode_flow_position(work.clone(), row))
            .transpose()
    }

    pub fn human_flow_positions(&self) -> StoreResult<Vec<FlowPosition>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT work_kind, work_id, flow, step, node_id, human,
                    session_run_id, ready_summary, step_index, iteration, updated_at
             FROM work_flow_positions WHERE human=1 ORDER BY updated_at, work_kind, work_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, bool>(5)?,
                row.get::<_, Option<String>>(6)?,
                row.get::<_, Option<String>>(7)?,
                row.get::<_, i64>(8)?,
                row.get::<_, i64>(9)?,
                row.get::<_, i64>(10)?,
            ))
        })?;
        let mut positions = Vec::new();
        for row in rows {
            let (
                kind,
                id,
                flow,
                step,
                node_id,
                human,
                session_run_id,
                ready_summary,
                step_index,
                iteration,
                updated_at,
            ) = row?;
            let work = parse_work_ref(&kind, &id)?;
            positions.push(decode_flow_position(
                work,
                (
                    flow,
                    step,
                    node_id,
                    human,
                    session_run_id,
                    ready_summary,
                    step_index,
                    iteration,
                    updated_at,
                ),
            )?);
        }
        Ok(positions)
    }

    pub fn abandon(&self, work: &WorkRef, reason: &str) -> StoreResult<AbandonReceipt> {
        let reason = reason.trim();
        if reason.is_empty() {
            return Err(StoreError::InvalidData(
                "abandon reason cannot be empty".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let now = now_unix();
        let (table, id) = work_table(work);
        if tx.execute(
            &format!(
                "UPDATE {table} SET work_state='abandoned', work_terminal_at=?2
                 WHERE id=?1 AND work_state='ready'"
            ),
            params![id, now],
        )? != 1
        {
            return Err(StoreError::InvalidAuthority(format!(
                "{} {} is not ready",
                work.kind(),
                work.id()
            )));
        }
        tx.commit()?;
        Ok(AbandonReceipt {
            work: work.clone(),
            reason: reason.to_string(),
            abandoned_at: OffsetDateTime::from_unix_timestamp(now)
                .expect("current Unix timestamp must be valid"),
        })
    }

    pub fn work_status(&self, work: &WorkRef) -> StoreResult<WorkStatus> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        work_status_in(&conn, work)
    }

    pub fn work_for_child(&self, target: &ChildRef) -> StoreResult<WorkRef> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        work_for_child_in(&conn, target)
    }

    pub fn work_steers(&self, work: &WorkRef) -> StoreResult<Vec<Steer>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        work_steers_in(&conn, work)
    }

    pub(crate) fn work_steers_for_child(&self, target: &ChildRef) -> StoreResult<Vec<Steer>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let work = work_for_child_in(&conn, target)?;
        work_steers_in(&conn, &work)
    }

    pub fn append_steer(
        &self,
        work: &WorkRef,
        author: &Author,
        text: &str,
    ) -> StoreResult<SteerReceipt> {
        let text = text.trim();
        if text.is_empty() {
            return Err(StoreError::InvalidData(
                "Steer text cannot be empty".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let receipt = Self::append_steer_in(&tx, work, author, text)?;
        tx.commit()?;
        Ok(receipt)
    }

    /// Every durable Steer recorded at or after `since`, newest first.
    ///
    pub fn list_steers_since(&self, since: i64) -> StoreResult<Vec<Steer>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT id, work_kind, work_id, author_kind, author_run_id, text, issued_at
             FROM steers WHERE issued_at >= ?1 ORDER BY issued_at DESC, id DESC",
        )?;
        let rows = statement.query_map([since], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, i64>(6)?,
            ))
        })?;
        let mut steers = Vec::new();
        for row in rows {
            let (id, work_kind, work_id, author_kind, author_run_id, text, issued_at) = row?;
            let work = parse_work_ref(&work_kind, &work_id)?;
            steers.push(decode_steer(
                (id, author_kind, author_run_id, text, issued_at),
                work,
            )?);
        }
        Ok(steers)
    }

    pub(crate) fn append_steer_in(
        tx: &Transaction<'_>,
        work: &WorkRef,
        author: &Author,
        text: &str,
    ) -> StoreResult<SteerReceipt> {
        let text = text.trim();
        if text.is_empty() {
            return Err(StoreError::InvalidData(
                "Steer text cannot be empty".to_string(),
            ));
        }
        require_ready_work(tx, work)?;
        let sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM steers
             WHERE work_kind=?1 AND work_id=?2",
            params![work.kind(), work.id()],
            |row| row.get(0),
        )?;
        let steer = Steer {
            id: SteerId::new(),
            work: work.clone(),
            author: author.clone(),
            text: text.to_string(),
            issued_at: OffsetDateTime::now_utc(),
        };
        let (author_kind, author_run_id) = match &steer.author {
            Author::User => ("user", None),
            Author::Run(run_id) => ("run", Some(run_id.as_str())),
        };
        tx.execute(
            "INSERT INTO steers (
                id, work_kind, work_id, sequence, author_kind, author_run_id, text, issued_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                steer.id.as_str(),
                work.kind(),
                work.id(),
                sequence,
                author_kind,
                author_run_id,
                steer.text,
                steer.issued_at.unix_timestamp(),
            ],
        )?;
        Ok(SteerReceipt { steer })
    }

    pub fn write_tool_response(
        &self,
        work: &WorkRef,
        write: &ToolResponseWrite,
    ) -> StoreResult<(ToolResponseReceipt, bool)> {
        let choice = write.choice.trim();
        if choice.is_empty() {
            return Err(StoreError::InvalidData(
                "tool response choice cannot be empty".to_string(),
            ));
        }
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        require_ready_work(&tx, work)?;
        if let Some(existing) = tool_response_in(&tx, work, &write.request_id)? {
            if existing.choice != choice {
                return Err(StoreError::InvalidData(format!(
                    "tool response {} is already resolved as {:?}",
                    write.request_id, existing.choice
                )));
            }
            return Ok((existing, false));
        }
        let receipt = ToolResponseReceipt {
            id: ToolResponseId::new(),
            work: work.clone(),
            request_id: write.request_id.clone(),
            choice: choice.to_string(),
            responded_at: OffsetDateTime::now_utc(),
        };
        tx.execute(
            "INSERT INTO tool_responses (
                id, work_kind, work_id, request_id, choice, responded_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                receipt.id.as_str(),
                work.kind(),
                work.id(),
                receipt.request_id,
                receipt.choice,
                receipt.responded_at.unix_timestamp(),
            ],
        )?;
        tx.commit()?;
        Ok((receipt, true))
    }

    pub fn tool_response(
        &self,
        work: &WorkRef,
        request_id: &str,
    ) -> StoreResult<Option<ToolResponseReceipt>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        tool_response_in(&conn, work, request_id)
    }
}

fn map_local_home(conn: &Connection) -> StoreResult<Home> {
    conn.query_row(
        "SELECT id, route, created_at, observed_at FROM homes WHERE route='local'",
        [],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        },
    )
    .map_err(StoreError::from)
    .and_then(|(id, route, created_at, observed_at)| {
        Ok(Home {
            id: HomeId::parse(&id).map_err(invalid_durable)?,
            route,
            created_at: OffsetDateTime::from_unix_timestamp(created_at).map_err(invalid_durable)?,
            observed_at: OffsetDateTime::from_unix_timestamp(observed_at)
                .map_err(invalid_durable)?,
        })
    })
}

fn map_home_by_id(conn: &Connection, home_id: &HomeId) -> StoreResult<Option<Home>> {
    conn.query_row(
        "SELECT id, route, created_at, observed_at FROM homes WHERE id=?1",
        [home_id.as_str()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, i64>(3)?,
            ))
        },
    )
    .optional()
    .map_err(StoreError::from)?
    .map(|(id, route, created_at, observed_at)| {
        Ok(Home {
            id: HomeId::parse(&id).map_err(invalid_durable)?,
            route,
            created_at: OffsetDateTime::from_unix_timestamp(created_at).map_err(invalid_durable)?,
            observed_at: OffsetDateTime::from_unix_timestamp(observed_at)
                .map_err(invalid_durable)?,
        })
    })
    .transpose()
}

fn placement_in(conn: &Connection, work: &WorkRef) -> StoreResult<Placement> {
    find_placement_in(conn, work)?.ok_or_else(|| {
        StoreError::InvalidData(format!(
            "{} {} has no Home placement",
            work.kind(),
            work.id()
        ))
    })
}

fn find_placement_in(conn: &Connection, work: &WorkRef) -> StoreResult<Option<Placement>> {
    let row = match work {
        WorkRef::Wave(id) => conn.query_row(
            "SELECT home_id, enabled, placed_at FROM work_placements WHERE wave_id=?1",
            [id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        ),
        WorkRef::Project(id) => conn.query_row(
            "SELECT home_id, enabled, placed_at FROM work_placements WHERE project_id=?1",
            [id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        ),
        WorkRef::Task(id) => conn.query_row(
            "SELECT home_id, enabled, placed_at FROM work_placements WHERE task_id=?1",
            [id.as_str()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        ),
    }
    .optional()?;
    row.map(|(home_id, enabled, placed_at)| {
        Ok(Placement {
            work: work.clone(),
            home_id: HomeId::parse(&home_id).map_err(invalid_durable)?,
            enabled,
            placed_at: OffsetDateTime::from_unix_timestamp(placed_at).map_err(invalid_durable)?,
        })
    })
    .transpose()
}

fn write_placement(
    tx: &Transaction<'_>,
    work: &WorkRef,
    home_id: &HomeId,
    placed_at: i64,
) -> StoreResult<()> {
    match work {
        WorkRef::Wave(id) => tx.execute(
            "INSERT INTO work_placements (wave_id, home_id, enabled, placed_at)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(wave_id) DO UPDATE SET
                home_id=excluded.home_id, placed_at=excluded.placed_at",
            params![id.as_str(), home_id.as_str(), placed_at],
        )?,
        WorkRef::Project(id) => tx.execute(
            "INSERT INTO work_placements (project_id, home_id, enabled, placed_at)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(project_id) DO UPDATE SET
                home_id=excluded.home_id, placed_at=excluded.placed_at",
            params![id.as_str(), home_id.as_str(), placed_at],
        )?,
        WorkRef::Task(id) => tx.execute(
            "INSERT INTO work_placements (task_id, home_id, enabled, placed_at)
             VALUES (?1, ?2, 1, ?3)
             ON CONFLICT(task_id) DO UPDATE SET
                home_id=excluded.home_id, placed_at=excluded.placed_at",
            params![id.as_str(), home_id.as_str(), placed_at],
        )?,
    };
    Ok(())
}

fn inherit_placement(
    tx: &Transaction<'_>,
    work: &WorkRef,
    parent: Option<&WorkRef>,
    placed_at: i64,
) -> StoreResult<()> {
    if find_placement_in(tx, work)?.is_some() {
        return Ok(());
    }
    let home_id = match parent {
        Some(parent) => placement_in(tx, parent)?.home_id,
        None => map_local_home(tx)?.id,
    };
    write_placement(tx, work, &home_id, placed_at)
}

pub(crate) fn work_status_in(conn: &Connection, work: &WorkRef) -> StoreResult<WorkStatus> {
    let (table, id) = work_table(work);
    let state: String = conn.query_row(
        &format!("SELECT work_state FROM {table} WHERE id=?1"),
        [id],
        |row| row.get(0),
    )?;
    match state.as_str() {
        "ready" => Ok(WorkStatus::Ready),
        "done" => Ok(WorkStatus::Done),
        "abandoned" => Ok(WorkStatus::Abandoned),
        other => Err(StoreError::InvalidData(format!(
            "invalid {} Work state {other:?}",
            work.kind()
        ))),
    }
}

fn require_ready_work(conn: &Connection, work: &WorkRef) -> StoreResult<()> {
    match work_status_in(conn, work)? {
        WorkStatus::Ready => Ok(()),
        status => Err(StoreError::InvalidAuthority(format!(
            "{} {} is {status}",
            work.kind(),
            work.id()
        ))),
    }
}

pub(crate) fn reopen_work_in(conn: &Connection, work: &WorkRef) -> StoreResult<()> {
    conn.execute(
        "DELETE FROM work_flow_positions WHERE work_kind=?1 AND work_id=?2",
        params![work.kind(), work.id()],
    )?;
    let (table, id) = work_table(work);
    if conn.execute(
        &format!(
            "UPDATE {table} SET work_state='ready', work_terminal_at=NULL
             WHERE id=?1 AND work_state IN ('done', 'abandoned')"
        ),
        [id],
    )? != 1
    {
        return Err(StoreError::InvalidAuthority(format!(
            "{} {} is not terminal",
            work.kind(),
            work.id()
        )));
    }
    Ok(())
}

fn work_table(work: &WorkRef) -> (&'static str, &str) {
    match work {
        WorkRef::Wave(id) => ("waves", id.as_str()),
        WorkRef::Project(id) => ("projects", id.as_str()),
        WorkRef::Task(id) => ("tasks", id.as_str()),
    }
}

fn parse_work_ref(kind: &str, id: &str) -> StoreResult<WorkRef> {
    match kind {
        "wave" => WaveId::parse(id)
            .map(WorkRef::Wave)
            .map_err(invalid_durable),
        "project" => ProjectId::parse(id)
            .map(WorkRef::Project)
            .map_err(invalid_durable),
        "task" => TaskId::parse(id)
            .map(WorkRef::Task)
            .map_err(invalid_durable),
        value => Err(StoreError::InvalidData(format!(
            "invalid Work kind: {value}"
        ))),
    }
}

type StoredFlowPosition = (
    String,
    String,
    Option<String>,
    bool,
    Option<String>,
    Option<String>,
    i64,
    i64,
    i64,
);

fn decode_flow_position(
    work: WorkRef,
    (
        flow,
        step,
        node_id,
        human,
        session_run_id,
        ready_summary,
        step_index,
        iteration,
        updated_at,
    ): StoredFlowPosition,
) -> StoreResult<FlowPosition> {
    Ok(FlowPosition {
        work,
        flow,
        step,
        node_id,
        human,
        session_run_id: session_run_id
            .map(|run_id| RunId::parse(&run_id).map_err(invalid_durable))
            .transpose()?,
        ready_summary,
        step_index: u32::try_from(step_index).map_err(invalid_durable)?,
        iteration: u32::try_from(iteration).map_err(invalid_durable)?,
        updated_at: OffsetDateTime::from_unix_timestamp(updated_at).map_err(invalid_durable)?,
    })
}

fn invalid_durable(error: impl std::fmt::Display) -> StoreError {
    StoreError::InvalidData(error.to_string())
}

pub(crate) fn create_wave_work(
    tx: &Transaction<'_>,
    wave_id: &WaveId,
    created_at: i64,
) -> StoreResult<()> {
    let work = WorkRef::Wave(wave_id.clone());
    inherit_placement(tx, &work, None, created_at)
}

pub(crate) fn create_project_work(tx: &Transaction<'_>, project: &Project) -> StoreResult<()> {
    let project_id = tx
        .query_row(
            "SELECT id FROM projects WHERE external_project_id=?1",
            [project.plan.id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .unwrap_or_else(|| ProjectId::new().to_string());
    tx.execute(
        "INSERT OR IGNORE INTO projects (
            id, wave_id, external_project_id, created_at
         ) VALUES (?1, ?2, ?3, ?4)",
        params![
            project_id,
            project.wave_id.as_str(),
            project.plan.id.as_str(),
            project.created_at.unix_timestamp(),
        ],
    )?;
    let work = WorkRef::Project(ProjectId::parse(&project_id).map_err(invalid_durable)?);
    let parent = WorkRef::Wave(project.wave_id.clone());
    inherit_placement(
        tx,
        &work,
        Some(&parent),
        project.created_at.unix_timestamp(),
    )?;
    Ok(())
}

pub(crate) fn create_task_work(tx: &Transaction<'_>, task: &Task) -> StoreResult<()> {
    let project_id = task.project_id.as_str().to_string();
    let task_id = tx
        .query_row(
            "SELECT id FROM tasks WHERE external_issue_id=?1",
            [task.plan.id.as_str()],
            |row| row.get::<_, String>(0),
        )
        .optional()?
        .unwrap_or_else(|| TaskId::new().to_string());
    tx.execute(
        "INSERT OR IGNORE INTO tasks (
            id, project_id, external_issue_id, issue_identifier, created_at
         ) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            task_id,
            project_id,
            task.plan.id.as_str(),
            task.plan.identifier,
            task.created_at.unix_timestamp(),
        ],
    )?;
    let work = WorkRef::Task(TaskId::parse(&task_id).map_err(invalid_durable)?);
    let parent = WorkRef::Project(ProjectId::parse(&project_id).map_err(invalid_durable)?);
    inherit_placement(tx, &work, Some(&parent), task.created_at.unix_timestamp())?;
    Ok(())
}

pub(crate) fn work_for_child_in(conn: &Connection, target: &ChildRef) -> StoreResult<WorkRef> {
    match target {
        ChildRef::Project(project_id) => {
            conn.query_row(
                "SELECT 1 FROM projects WHERE id=?1",
                [project_id.as_str()],
                |_| Ok(()),
            )?;
            Ok(WorkRef::Project(project_id.clone()))
        }
        ChildRef::Task(task_id) => {
            conn.query_row(
                "SELECT 1 FROM tasks WHERE id=?1",
                [task_id.as_str()],
                |_| Ok(()),
            )?;
            Ok(WorkRef::Task(task_id.clone()))
        }
    }
}

fn work_steers_in(conn: &Connection, work: &WorkRef) -> StoreResult<Vec<Steer>> {
    require_ready_work(conn, work)?;
    let mut statement = conn.prepare(
        "SELECT id, author_kind, author_run_id, text, issued_at
         FROM steers WHERE work_kind=?1 AND work_id=?2 ORDER BY sequence",
    )?;
    let rows = statement.query_map(params![work.kind(), work.id()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, i64>(4)?,
        ))
    })?;
    let mut steers = Vec::new();
    for row in rows {
        steers.push(decode_steer(row?, work.clone())?);
    }
    Ok(steers)
}

fn tool_response_in(
    conn: &Connection,
    work: &WorkRef,
    request_id: &str,
) -> StoreResult<Option<ToolResponseReceipt>> {
    let row = conn
        .query_row(
            "SELECT id, choice, responded_at FROM tool_responses
             WHERE work_kind=?1 AND work_id=?2 AND request_id=?3",
            params![work.kind(), work.id(), request_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            },
        )
        .optional()?;
    let Some((id, choice, responded_at)) = row else {
        return Ok(None);
    };
    Ok(Some(ToolResponseReceipt {
        id: ToolResponseId::parse(&id).map_err(|error| {
            StoreError::InvalidData(format!("invalid stored ToolResponse id: {error}"))
        })?,
        work: work.clone(),
        request_id: request_id.to_string(),
        choice,
        responded_at: OffsetDateTime::from_unix_timestamp(responded_at).map_err(|error| {
            StoreError::InvalidData(format!("invalid Decision timestamp: {error}"))
        })?,
    }))
}

type SteerFields = (String, String, Option<String>, String, i64);

fn decode_steer(fields: SteerFields, work: WorkRef) -> StoreResult<Steer> {
    let (id, author_kind, author_run_id, text, issued_at) = fields;
    let author = match (author_kind.as_str(), author_run_id) {
        ("user", None) => Author::User,
        ("run", Some(id)) => Author::Run(RunId::parse(&id).map_err(invalid_durable)?),
        _ => {
            return Err(StoreError::InvalidData(
                "stored Steer author is inconsistent".to_string(),
            ))
        }
    };
    Ok(Steer {
        id: SteerId::parse(&id).map_err(invalid_durable)?,
        work,
        author,
        text,
        issued_at: OffsetDateTime::from_unix_timestamp(issued_at).map_err(|error| {
            StoreError::InvalidData(format!("invalid Steer timestamp: {error}"))
        })?,
    })
}

#[cfg(test)]
mod durable_store_tests {
    use crate::durable::{Author, FlowPosition, RunId, WorkRef};
    use crate::id::WaveId;
    use crate::store::sqlite::SqliteStore;

    /// A registered Wave is the cheapest real Work and needs no PM binding.
    fn _store_with_wave() -> (tempfile::TempDir, SqliteStore, WorkRef) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("loopflow.db");
        let store = SqliteStore::new(&path).expect("open a fresh store");
        let wave_id = WaveId::new();
        let conn = rusqlite::Connection::open(&path).unwrap();
        conn.execute(
            "INSERT INTO waves (id, name, repo, created_at, parent_wave_id)
             VALUES (?1, 'probe', '/repo', 1700000000, NULL)",
            [wave_id.as_str()],
        )
        .unwrap();
        drop(conn);
        // Reach the crate-private spine builder the public upsert path calls.
        {
            let mut raw = rusqlite::Connection::open(&path).unwrap();
            let tx = raw.transaction().unwrap();
            super::create_wave_work(&tx, &wave_id, 1_700_000_000).unwrap();
            tx.commit().unwrap();
        }
        let work = WorkRef::Wave(wave_id);
        (dir, store, work)
    }

    fn store_with_wave() -> (tempfile::TempDir, SqliteStore, WorkRef) {
        _store_with_wave()
    }

    #[test]
    fn activity_steers_are_ordered_work_facts() {
        let (dir, store, work) = store_with_wave();
        let first = store
            .append_steer(&work, &Author::User, "first direction")
            .unwrap();
        let second = store
            .append_steer(&work, &Author::User, "second direction")
            .unwrap();
        let conn = rusqlite::Connection::open(dir.path().join("loopflow.db")).unwrap();
        conn.execute(
            "UPDATE steers SET issued_at=1700000001 WHERE id=?1",
            [first.steer.id.as_str()],
        )
        .unwrap();
        conn.execute(
            "UPDATE steers SET issued_at=1700000021 WHERE id=?1",
            [second.steer.id.as_str()],
        )
        .unwrap();

        let steers = store.list_steers_since(0).unwrap();

        assert_eq!(
            steers
                .iter()
                .map(|steer| steer.text.as_str())
                .collect::<Vec<_>>(),
            ["second direction", "first direction"]
        );
        assert!(steers.iter().all(|steer| steer.work == work));
        assert_eq!(
            store
                .list_steers_since(1_700_000_010)
                .unwrap()
                .into_iter()
                .map(|steer| steer.id)
                .collect::<Vec<_>>(),
            [second.steer.id]
        );
    }

    #[test]
    fn human_session_runtime_survives_a_store_round_trip() {
        let (_dir, store, work) = store_with_wave();
        let run_id = RunId::new();
        let position = FlowPosition {
            work: work.clone(),
            flow: "review".to_string(),
            step: "review-design".to_string(),
            node_id: Some("human_review".to_string()),
            human: true,
            session_run_id: Some(run_id.clone()),
            ready_summary: Some("Ready for review".to_string()),
            step_index: 1,
            iteration: 2,
            updated_at: time::OffsetDateTime::now_utc(),
        };

        store.set_flow_position(&work, &position).unwrap();

        let stored = store.flow_position(&work).unwrap().unwrap();
        assert_eq!(stored.session_run_id, Some(run_id));
        assert_eq!(stored.ready_summary.as_deref(), Some("Ready for review"));
    }
}
