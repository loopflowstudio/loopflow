//! A Task's append-only ownership log and this Machine's reading of it.
//!
//! The later entry wins, in provider order. Writes stay within one person's
//! Machines: your Task's next entry is written on its Machine when that is one
//! of yours, otherwise on your Machine for its Wave. An inbound entry never
//! ends this Machine's own open claim; only its own unclaim or handoff does.
use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::durable::{
    MachineId, MachineRef, OwnershipEntryId, OwnershipKind, TaskExecutionSource, TaskId, TaskOwner,
    TaskOwnership, TaskOwnershipEntry,
};
use crate::store::{StoreError, StoreResult};
use crate::work::task::comment::{TaskComment, TaskCommentAuthor};

use super::SqliteStore;

const MARKER: &str = "<!-- loopflow-ownership:";

#[derive(Debug, Clone)]
struct Row {
    entry: TaskOwnershipEntry,
    written_here: bool,
}

fn read_kind(value: &str) -> StoreResult<OwnershipKind> {
    serde_json::from_value(value.into()).map_err(Into::into)
}

fn log_in(conn: &Connection, task: &TaskId) -> StoreResult<Vec<Row>> {
    Ok(logs_in(conn, std::slice::from_ref(task))?
        .remove(task)
        .unwrap_or_default())
}

/// Each Task's ownership log, in one statement.
fn logs_in(conn: &Connection, tasks: &[TaskId]) -> StoreResult<HashMap<TaskId, Vec<Row>>> {
    // Entries the provider has not stamped yet share whole-second `at` values;
    // insertion order keeps a claim and its handoff in the order they happened.
    let mut statement = conn.prepare(
        "SELECT id,kind,person,person_key,machine_id,machine_label,at,written_here,task_id
         FROM task_ownership WHERE task_id IN (SELECT value FROM json_each(?1))
         ORDER BY ordered_at IS NULL,ordered_at,at,rowid",
    )?;
    let rows = statement.query_map(
        [super::children::ids_json(tasks.iter().map(TaskId::as_str))],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, Option<String>>(5)?,
                row.get::<_, i64>(6)?,
                row.get::<_, bool>(7)?,
                row.get::<_, String>(8)?,
            ))
        },
    )?;
    let mut logs = HashMap::<TaskId, Vec<Row>>::new();
    for row in rows {
        let (id, kind, person, person_key, machine, machine_label, at, written_here, task) = row?;
        let task = TaskId::from_raw(task);
        logs.entry(task.clone()).or_default().push(Row {
            entry: TaskOwnershipEntry {
                id: OwnershipEntryId::from_raw(id),
                task_id: task,
                kind: read_kind(&kind)?,
                person,
                person_key,
                machine_id: machine.as_deref().map(MachineId::from_raw),
                machine_label,
                at,
            },
            written_here,
        });
    }
    Ok(logs)
}

/// Each listed Task's current owner as this Machine reads it, with Machine
/// labels as best known here; Tasks without an owner are absent.
pub(super) fn owners_in(
    conn: &Connection,
    tasks: &[TaskId],
) -> StoreResult<HashMap<TaskId, TaskOwner>> {
    let local = local_machine_in(conn)?;
    let mut labels = conn.prepare("SELECT id,label FROM machines WHERE label IS NOT NULL")?;
    let labels = labels
        .query_map([], |row| {
            Ok((
                MachineId::from_raw(row.get::<_, String>(0)?),
                row.get::<_, String>(1)?,
            ))
        })?
        .collect::<rusqlite::Result<HashMap<_, _>>>()?;
    let mut owners = HashMap::new();
    for (task, rows) in logs_in(conn, tasks)? {
        let Some(mut owner) = read_log(&rows, &local).owner else {
            continue;
        };
        if let Some(machine) = owner.machine.id.as_ref() {
            owner.machine.label = if *machine == local {
                Some(local_label())
            } else {
                labels.get(machine).cloned().or(owner.machine.label.take())
            };
        }
        owners.insert(task, owner);
    }
    Ok(owners)
}

/// Who holds a Task after an entry, with the Machine label as written.
fn holder_after(entry: &TaskOwnershipEntry) -> Option<TaskOwner> {
    (entry.kind != OwnershipKind::Unclaim).then(|| entry.owner())
}

/// This Machine's reading of the ordered log.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Reading {
    owner: Option<TaskOwner>,
    superseded_by: Option<TaskOwner>,
    /// This Machine holds an open claim that the provider order no longer shows,
    /// for a reason other than a later claim.
    reassert: bool,
}

fn read_log(rows: &[Row], local: &MachineId) -> Reading {
    let global = rows.iter().fold(None, |_, row| holder_after(&row.entry));
    let mut local_claim = None;
    for (index, row) in rows.iter().enumerate() {
        let holder = holder_after(&row.entry);
        if holder
            .as_ref()
            .and_then(|holder| holder.machine.id.as_ref())
            == Some(local)
        {
            local_claim = Some(index);
        } else if row.written_here
            && matches!(
                row.entry.kind,
                OwnershipKind::Unclaim | OwnershipKind::Handoff
            )
        {
            local_claim = None;
        }
    }
    let Some(index) = local_claim else {
        return Reading {
            owner: global,
            superseded_by: None,
            reassert: false,
        };
    };
    let owner = holder_after(&rows[index].entry);
    let global_is_local = global
        .as_ref()
        .and_then(|holder| holder.machine.id.as_ref())
        == Some(local);
    let later_claim = rows[index + 1..]
        .iter()
        .rev()
        .find(|row| row.entry.kind == OwnershipKind::Claim && !row.written_here);
    let superseded_by = match later_claim {
        Some(claim) if !global_is_local => global.clone().or_else(|| holder_after(&claim.entry)),
        _ => None,
    };
    Reading {
        reassert: !global_is_local && superseded_by.is_none(),
        owner,
        superseded_by,
    }
}

pub(super) fn local_machine_in(conn: &Connection) -> StoreResult<MachineId> {
    Ok(super::durable::map_local_machine(conn)?.id)
}

fn reading_in(conn: &Connection, task: &TaskId) -> StoreResult<(Vec<Row>, Reading)> {
    let rows = log_in(conn, task)?;
    let reading = read_log(&rows, &local_machine_in(conn)?);
    Ok((rows, reading))
}

/// The local Machine has no registered label; its short hostname names it
/// here. It is never published: only a public name leaves this Machine.
pub(crate) fn local_label() -> String {
    let host = gethostname::gethostname().to_string_lossy().into_owned();
    host.split('.').next().unwrap_or(&host).to_string()
}

/// A Machine's best known label: this Machine's hostname, then a label you
/// registered, then the label its claimant wrote.
fn machine_ref_in(
    conn: &Connection,
    machine: Option<&MachineId>,
    written: Option<&str>,
) -> StoreResult<MachineRef> {
    let Some(machine) = machine else {
        return Ok(MachineRef {
            id: None,
            label: written.map(Into::into),
        });
    };
    let label = if *machine == local_machine_in(conn)? {
        Some(local_label())
    } else {
        conn.query_row(
            "SELECT label FROM machines WHERE id=?1",
            [machine.as_str()],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten()
        .or(written.map(Into::into))
    };
    Ok(MachineRef {
        id: Some(machine.clone()),
        label,
    })
}

/// An owner with their Machine's label as best known here.
fn resolved_in(conn: &Connection, owner: Option<TaskOwner>) -> StoreResult<Option<TaskOwner>> {
    owner
        .map(|mut owner| {
            owner.machine = machine_ref_in(
                conn,
                owner.machine.id.as_ref(),
                owner.machine.label.as_deref(),
            )?;
            Ok(owner)
        })
        .transpose()
}

pub(super) fn ownership_in(conn: &Connection, task: &TaskId) -> StoreResult<TaskOwnership> {
    let (rows, reading) = reading_in(conn, task)?;
    let mut log = Vec::with_capacity(rows.len());
    for row in rows {
        let mut entry = row.entry;
        entry.machine_label = machine_ref_in(
            conn,
            entry.machine_id.as_ref(),
            entry.machine_label.as_deref(),
        )?
        .label;
        log.push(entry);
    }
    Ok(TaskOwnership {
        owner: resolved_in(conn, reading.owner)?,
        superseded_by: resolved_in(conn, reading.superseded_by)?,
        log,
    })
}

pub(super) fn owned_in(conn: &Connection, task: &TaskId) -> StoreResult<bool> {
    Ok(reading_in(conn, task)?.1.owner.is_some())
}

/// This Machine, or one registered with `lf machine add`.
fn is_my_machine_in(conn: &Connection, machine: &MachineId) -> StoreResult<bool> {
    Ok(conn
        .query_row(
            "SELECT 1 FROM machines WHERE id=?1 AND (route='local' OR label IS NOT NULL)",
            [machine.as_str()],
            |_| Ok(()),
        )
        .optional()?
        .is_some())
}

/// Whether a holder is you. A holder with a person key (Git `user.email`) is
/// you exactly when the key is yours, even on a Machine you registered. A
/// keyless entry, written before keys or recorded before ownership existed, is
/// yours when it holds one of your Machines. Names are display only.
fn is_mine_in(conn: &Connection, task: &TaskId, holder: &TaskOwner) -> StoreResult<bool> {
    if let Some(key) = &holder.person_key {
        return Ok(my_key_in(conn, task)?.as_ref() == Some(key));
    }
    match &holder.machine.id {
        Some(machine) => is_my_machine_in(conn, machine),
        None => Ok(false),
    }
}

fn task_repo_in(conn: &Connection, task: &TaskId) -> StoreResult<String> {
    conn.query_row(
        "SELECT w.repo FROM tasks t JOIN repository_plans w ON w.id=t.repository_id WHERE t.id=?1",
        [task.as_str()],
        |row| row.get(0),
    )
    .map_err(Into::into)
}

/// Your person key in the Task's repository: its Git `user.email`.
fn my_key_in(conn: &Connection, task: &TaskId) -> StoreResult<Option<String>> {
    Ok(crate::config::participant_email(std::path::Path::new(
        &task_repo_in(conn, task)?,
    )))
}

/// The one Machine of yours that writes the Task's next entry: the owner's,
/// when you own it on one of your Machines, else your Machine for its Wave.
/// Someone else's Machine is never a writer for you.
pub(super) fn writer_in(
    conn: &Connection,
    task: &TaskId,
) -> StoreResult<(MachineId, TaskExecutionSource)> {
    let (_, reading) = reading_in(conn, task)?;
    if let Some(holder) = &reading.owner {
        if let Some(machine) = &holder.machine.id {
            if is_mine_in(conn, task, holder)? && is_my_machine_in(conn, machine)? {
                return Ok((machine.clone(), TaskExecutionSource::Owner));
            }
        }
    }
    let (wave, repository): (Option<String>, Option<String>) = conn.query_row(
        "SELECT p.wave_id,t.repository_id FROM tasks t LEFT JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
        [task.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    let (machine, source) = super::scope_defaults::owning_machine_in(
        conn,
        wave,
        repository,
        crate::durable::MachineSetting::Tasks,
    )?;
    Ok((machine, TaskExecutionSource::OwningMachine(source)))
}

fn require_writer_in(conn: &Connection, task: &TaskId) -> StoreResult<MachineId> {
    let local = local_machine_in(conn)?;
    let (writer, _) = writer_in(conn, task)?;
    if writer != local {
        return Err(StoreError::InvalidAuthority(format!(
            "Task {task} ownership is written on Machine {writer}; run the command there"
        )));
    }
    Ok(local)
}

/// Handoff and unclaim end your own claim; someone else's stays theirs.
fn require_my_claim_in(conn: &Connection, task: &TaskId) -> StoreResult<(MachineId, TaskOwner)> {
    let local = require_writer_in(conn, task)?;
    let (_, reading) = reading_in(conn, task)?;
    match reading.owner {
        None => Err(StoreError::InvalidAuthority(format!(
            "Task {task} is unowned"
        ))),
        Some(holder) if !is_mine_in(conn, task, &holder)? => {
            Err(StoreError::InvalidAuthority(format!(
                "Task {task} is claimed by {}; claim it to take it over",
                holder.person.as_deref().unwrap_or("someone else")
            )))
        }
        Some(holder) => Ok((local, holder)),
    }
}

fn person() -> StoreResult<Option<String>> {
    crate::config::participant_name().map_err(|error| StoreError::InvalidData(error.to_string()))
}

/// Published text names only what the entry carries: a public name, else the
/// Machine id. Local labels and hostnames never leave this Machine.
fn describe(entry: &TaskOwnershipEntry) -> String {
    let who = crate::durable::person_text(entry.person.as_deref(), entry.person_key.as_deref());
    let place = entry
        .machine_label
        .clone()
        .or(entry.machine_id.as_ref().map(|id| id.to_string()))
        .unwrap_or_else(|| "an unknown Machine".into());
    match entry.kind {
        OwnershipKind::Claim => format!("{who} claimed this on {place}"),
        OwnershipKind::Handoff => format!("{who} handed this off to {place}"),
        OwnershipKind::Unclaim => format!("{who} unclaimed this on {place}"),
        OwnershipKind::Recorded => format!("{who} was recorded on {place}"),
    }
}

/// The marked text an entry travels in; `ingest_ownership_marker` reads it.
pub(crate) fn marker(entry: &TaskOwnershipEntry) -> StoreResult<String> {
    // Escape angle brackets so a person name cannot close the HTML comment.
    let json = serde_json::to_string(entry)?
        .replace('<', "\\u003c")
        .replace('>', "\\u003e");
    Ok(format!("{MARKER}{json} -->"))
}

/// Append a local entry and deliver it as a marked Task comment. The comment
/// travels through the existing Linear delivery and the Git planning journal.
fn append_in(
    conn: &Connection,
    task: &TaskId,
    kind: OwnershipKind,
    person: Option<String>,
    person_key: Option<String>,
    machine: &MachineId,
) -> StoreResult<TaskOwnershipEntry> {
    let entry = TaskOwnershipEntry {
        id: OwnershipEntryId::new(),
        task_id: task.clone(),
        kind,
        person,
        person_key,
        machine_id: Some(machine.clone()),
        // Private Machines publish no label; teammates see the id.
        machine_label: if *machine == local_machine_in(conn)? {
            super::durable::local_public_name_in(conn)?
        } else {
            None
        },
        at: time::OffsetDateTime::now_utc().unix_timestamp(),
    };
    let repo = task_repo_in(conn, task)?;
    let remote = crate::ops::planning_client::connected(&repo);
    // Without a remote provider, the planning journal is the provider and this write is its
    // order: after every entry this Machine has seen, whatever their clocks said.
    let ordered_at: Option<i64> = if remote {
        None
    } else {
        Some(conn.query_row(
            "SELECT max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE(max(ordered_at)+1,0))
             FROM task_ownership WHERE task_id=?1",
            [task.as_str()],
            |row| row.get(0),
        )?)
    };
    conn.execute(
        "INSERT INTO task_ownership(id,task_id,kind,person,person_key,machine_id,machine_label,
             at,ordered_at,written_here)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,1)",
        params![
            entry.id.as_str(),
            task.as_str(),
            kind.as_str(),
            entry.person,
            entry.person_key,
            machine.as_str(),
            entry.machine_label,
            entry.at,
            ordered_at
        ],
    )?;
    // Peers order the entry by its comment's time; in the journal that is our order.
    let created_at = match ordered_at {
        Some(ms) => time::OffsetDateTime::from_unix_timestamp_nanos(i128::from(ms) * 1_000_000)
            .map_err(|error| StoreError::InvalidData(error.to_string()))?,
        None => time::OffsetDateTime::now_utc(),
    }
    .format(&time::format_description::well_known::Rfc3339)
    .map_err(|error| StoreError::InvalidData(error.to_string()))?;
    super::task_comments::insert_authored_comment(
        conn,
        task,
        &TaskComment {
            id: uuid::Uuid::new_v4().to_string(),
            body: format!("{}\n\n{}", describe(&entry), marker(&entry)?),
            author: TaskCommentAuthor::Integration,
            created_at: Some(created_at),
        },
    )?;
    if remote {
        match kind {
            OwnershipKind::Claim => {
                let state: Option<String> = conn.query_row(
                    "SELECT planning_state FROM tasks WHERE id=?1",
                    [task.as_str()],
                    |row| row.get(0),
                )?;
                if matches!(
                    state.as_deref(),
                    None | Some("unstarted" | "backlog" | "triage")
                ) {
                    super::task_state_delivery::queue_in(conn, task, "started")?;
                }
            }
            OwnershipKind::Unclaim => {
                super::planning_changes::PlanningChanges::Task(task)
                    .save(conn, &[crate::work::edit::WorkValue::TaskAssignee(None)])?;
            }
            OwnershipKind::Handoff | OwnershipKind::Recorded => {}
        }
    }
    Ok(entry)
}

/// Claim for this Machine when the Task is unowned or its owner's Machine is
/// unknown. Returns the new entry, or None when this Machine already owns it.
pub(super) fn claim_in(
    conn: &Connection,
    task: &TaskId,
) -> StoreResult<Option<TaskOwnershipEntry>> {
    let local = require_writer_in(conn, task)?;
    let (_, reading) = reading_in(conn, task)?;
    let owned_here = reading.superseded_by.is_none()
        && reading
            .owner
            .as_ref()
            .and_then(|owner| owner.machine.id.as_ref())
            == Some(&local);
    if owned_here {
        return Ok(None);
    }
    let key = my_key_in(conn, task)?.ok_or_else(|| {
        StoreError::InvalidData(
            "claiming needs your Git user.email, which identifies you to teammates; \
             run `git config --global user.email you@example.com`"
                .into(),
        )
    })?;
    append_in(
        conn,
        task,
        OwnershipKind::Claim,
        person()?,
        Some(key),
        &local,
    )
    .map(Some)
}

/// Parse a marked ownership comment into the log. Repeats are ignored; the
/// provider's order is stamped once, the first time it is reported.
pub(super) fn ingest_marker_in(
    conn: &Connection,
    task: &TaskId,
    body: &str,
    ordered_at: Option<&str>,
) -> StoreResult<bool> {
    let Some(start) = body.find(MARKER) else {
        return Ok(false);
    };
    let rest = &body[start + MARKER.len()..];
    let Some(end) = rest.find("-->") else {
        return Ok(false);
    };
    let Ok(entry) = serde_json::from_str::<TaskOwnershipEntry>(rest[..end].trim()) else {
        return Ok(false);
    };
    if entry.task_id != *task || entry.kind == OwnershipKind::Recorded || entry.machine_id.is_none()
    {
        return Ok(false);
    }
    let ordered_at = ordered_at
        .and_then(|at| {
            time::OffsetDateTime::parse(at, &time::format_description::well_known::Rfc3339).ok()
        })
        .map(|at| (at.unix_timestamp_nanos() / 1_000_000) as i64);
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO task_ownership(id,task_id,kind,person,person_key,machine_id,
             machine_label,at,ordered_at,written_here)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,0)",
        params![
            entry.id.as_str(),
            task.as_str(),
            entry.kind.as_str(),
            entry.person,
            entry.person_key,
            entry.machine_id.as_ref().map(MachineId::as_str),
            entry.machine_label,
            entry.at,
            ordered_at
        ],
    )? > 0;
    if !inserted {
        if let Some(ordered_at) = ordered_at {
            conn.execute(
                "UPDATE task_ownership SET ordered_at=?2 WHERE id=?1 AND ordered_at IS NULL",
                params![entry.id.as_str(), ordered_at],
            )?;
        }
    }
    if inserted && entry.kind == OwnershipKind::Claim {
        notify_superseded_in(conn, task, &entry)?;
    }
    Ok(inserted)
}

/// A later claim that this Machine's open claim outlives reaches live Sessions
/// as direction. Nothing is cancelled; the claimant decides what to do.
fn notify_superseded_in(
    conn: &Connection,
    task: &TaskId,
    entry: &TaskOwnershipEntry,
) -> StoreResult<()> {
    let ownership = ownership_in(conn, task)?;
    let (Some(owner), Some(later)) = (&ownership.owner, &ownership.superseded_by) else {
        return Ok(());
    };
    let text = format!(
        "Also claimed later by {later}. {owner} still owns this Task on this Machine and \
         nothing was cancelled. Keep going, or run `lf task unclaim` or \
         `lf task move <task> --machine <machine>` to end this claim."
    );
    let steer = super::steers::save_in(
        conn,
        ("task", task.as_str()),
        &format!("ownership:{}", entry.id),
        "local",
        &crate::durable::Author::Named("Loopflow".into()),
        &text,
    )?;
    super::children::insert_task_event_in(
        conn,
        task,
        &crate::work::task::TaskEventKind::Steer { steer_id: steer.id },
    )?;
    Ok(())
}

/// After an inbound observation: an open local claim that the provider order no
/// longer shows, for a reason other than a later claim, is claimed again.
pub(super) fn reassert_in(
    conn: &Connection,
    task: &TaskId,
) -> StoreResult<Option<TaskOwnershipEntry>> {
    let (_, reading) = reading_in(conn, task)?;
    let (true, Some(holder)) = (reading.reassert, reading.owner) else {
        return Ok(None);
    };
    append_in(
        conn,
        task,
        OwnershipKind::Claim,
        holder.person,
        holder.person_key,
        &local_machine_in(conn)?,
    )
    .map(Some)
}

/// Hand each open Task an ended Machine still owns to this Machine, in its
/// owner's name: the claim survives and the log keeps the Machine as history.
pub(super) fn hand_back_in(conn: &Connection, machine: &MachineId) -> StoreResult<Vec<TaskId>> {
    let local = local_machine_in(conn)?;
    let held = super::rentals::tasks_held_in(conn, machine)?;
    for task in &held {
        let (person, person_key) = reading_in(conn, task)?
            .1
            .owner
            .map_or((None, None), |owner| (owner.person, owner.person_key));
        append_in(
            conn,
            task,
            OwnershipKind::Handoff,
            person,
            person_key,
            &local,
        )?;
    }
    Ok(held)
}

impl SqliteStore {
    pub fn task_ownership(&self, task: &TaskId) -> StoreResult<TaskOwnership> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        ownership_in(&snapshot, task)
    }

    pub fn task_owned(&self, task: &TaskId) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        owned_in(&snapshot, task)
    }

    /// Whether this Machine holds an open claim that Linear should show.
    pub(crate) fn task_claimed_here(&self, task: &TaskId) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let local = local_machine_in(&snapshot)?;
        let (_, reading) = reading_in(&snapshot, task)?;
        Ok(reading.superseded_by.is_none()
            && reading.owner.and_then(|holder| holder.machine.id) == Some(local))
    }

    pub fn claim_task(&self, task: &TaskId) -> StoreResult<Option<TaskOwnershipEntry>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        super::children::task_on(&tx, task)?.ok_or(StoreError::NotFound)?;
        let entry = claim_in(&tx, task)?;
        tx.commit()?;
        Ok(entry)
    }

    /// Move your claim to another Machine. The Task's primary Session here is
    /// completed; the target admits its own on first connect.
    pub fn handoff_task(
        &self,
        task: &TaskId,
        target: &MachineId,
    ) -> StoreResult<TaskOwnershipEntry> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (local, holder) = require_my_claim_in(&tx, task)?;
        if *target == local {
            return Err(StoreError::InvalidData(format!(
                "Task {task} is already owned on this Machine"
            )));
        }
        let entry = append_in(
            &tx,
            task,
            OwnershipKind::Handoff,
            holder.person,
            holder.person_key,
            target,
        )?;
        super::sessions::complete_primary_session_in(
            &tx,
            &crate::session::PrimaryScope::Task(task.clone()),
        )?;
        tx.commit()?;
        Ok(entry)
    }

    /// Take in an entry another Machine wrote, before planning delivers its comment.
    pub(crate) fn ingest_ownership_marker(&self, task: &TaskId, marker: &str) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let inserted = ingest_marker_in(&tx, task, marker, None)?;
        tx.commit()?;
        Ok(inserted)
    }

    pub fn unclaim_task(&self, task: &TaskId) -> StoreResult<TaskOwnershipEntry> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (local, holder) = require_my_claim_in(&tx, task)?;
        let key = my_key_in(&tx, task)?.or(holder.person_key);
        let entry = append_in(&tx, task, OwnershipKind::Unclaim, person()?, key, &local)?;
        tx.commit()?;
        Ok(entry)
    }

    /// Queue the claiming person as Linear assignee unless Linear already shows it.
    pub(crate) fn assign_task_owner(&self, task: &TaskId, assignee: &str) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: Option<String> = tx.query_row(
            "SELECT planning_assignee FROM tasks WHERE id=?1",
            [task.as_str()],
            |row| row.get(0),
        )?;
        if current.as_deref() != Some(assignee) {
            super::planning_changes::PlanningChanges::Task(task).save(
                &tx,
                &[crate::work::edit::WorkValue::TaskAssignee(Some(
                    assignee.into(),
                ))],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{local_label, log_in, read_log, Reading, Row};
    use crate::durable::{
        MachineId, MachineRef, OwnershipEntryId, OwnershipKind, TaskId, TaskOwner,
        TaskOwnershipEntry,
    };
    use crate::store::sqlite::SqliteStore;
    use crate::work::task::NewTask;

    fn published_bodies(store: &SqliteStore, task: &TaskId) -> Vec<String> {
        let conn = store.conn.lock().unwrap();
        let mut statement = conn
            .prepare("SELECT body FROM task_comments WHERE task_id=?1")
            .unwrap();
        statement
            .query_map([task.as_str()], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    fn store_with_unowned_task() -> (tempfile::TempDir, SqliteStore, TaskId) {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("db")).unwrap();
        let repo = dir.path().canonicalize().unwrap();
        let task = store
            .create_task(&NewTask {
                id: TaskId::new(),
                repository_id: store.ensure_repository(repo.to_str().unwrap()).unwrap(),
                project_id: None,
                title: "Private Machines".into(),
                description: String::new(),
                due_date: None,
            })
            .unwrap();
        (dir, store, task.id)
    }

    fn row(kind: OwnershipKind, person: &str, machine: &MachineId, here: bool) -> Row {
        Row {
            entry: TaskOwnershipEntry {
                id: OwnershipEntryId::new(),
                task_id: TaskId::new(),
                kind,
                person: Some(person.into()),
                person_key: None,
                machine_id: Some(machine.clone()),
                machine_label: None,
                at: 1,
            },
            written_here: here,
        }
    }

    fn holder(person: &str, machine: &MachineId) -> Option<TaskOwner> {
        Some(TaskOwner {
            person: Some(person.into()),
            person_key: None,
            machine: MachineRef {
                id: Some(machine.clone()),
                label: None,
            },
        })
    }

    #[test]
    fn a_later_claim_wins_elsewhere_but_never_displaces_the_local_claim() {
        let (laptop, mini) = (MachineId::new(), MachineId::new());
        let rows = [
            row(OwnershipKind::Claim, "Jack", &laptop, true),
            row(OwnershipKind::Claim, "Kim", &mini, false),
        ];
        assert_eq!(
            read_log(&rows, &laptop),
            Reading {
                owner: holder("Jack", &laptop),
                superseded_by: holder("Kim", &mini),
                reassert: false,
            }
        );
        let elsewhere = MachineId::new();
        assert_eq!(read_log(&rows, &elsewhere).owner, holder("Kim", &mini));
    }

    #[test]
    fn only_the_local_unclaim_or_handoff_ends_the_local_claim() {
        let (laptop, mini) = (MachineId::new(), MachineId::new());
        let mut rows = vec![
            row(OwnershipKind::Claim, "Jack", &laptop, true),
            row(OwnershipKind::Unclaim, "Kim", &mini, false),
        ];
        let reading = read_log(&rows, &laptop);
        assert_eq!(reading.owner, holder("Jack", &laptop));
        assert!(reading.reassert);
        rows.push(row(OwnershipKind::Handoff, "Jack", &mini, true));
        assert_eq!(read_log(&rows, &laptop).owner, holder("Jack", &mini));
        rows.push(row(OwnershipKind::Unclaim, "Jack", &mini, false));
        assert_eq!(read_log(&rows, &laptop).owner, None);
    }

    #[test]
    fn unstamped_entries_in_one_second_keep_their_written_order() {
        let (_dir, store, task) = store_with_unowned_task();
        let conn = store.conn.lock().unwrap();
        // Linear stamps provider order later; until then a claim and its handoff
        // share a whole-second `at`, and the handoff's id may sort first.
        for (id, kind) in [("own_zzzz", "claim"), ("own_aaaa", "handoff")] {
            conn.execute(
                "INSERT INTO task_ownership(id,task_id,kind,person,machine_id,at,ordered_at,written_here)
                 VALUES(?1,?2,?3,'Jack',?4,1,NULL,1)",
                rusqlite::params![id, task.as_str(), kind, MachineId::new().as_str()],
            )
            .unwrap();
        }
        let kinds: Vec<OwnershipKind> = log_in(&conn, &task)
            .unwrap()
            .iter()
            .map(|row| row.entry.kind)
            .collect();
        assert_eq!(kinds, [OwnershipKind::Claim, OwnershipKind::Handoff]);
    }

    #[test]
    fn a_recorded_entry_with_an_unknown_machine_is_owned_without_a_machine() {
        let laptop = MachineId::new();
        let mut recorded = row(OwnershipKind::Recorded, "", &laptop, true);
        recorded.entry.person = None;
        recorded.entry.machine_id = None;
        assert_eq!(
            read_log(&[recorded], &laptop).owner,
            Some(TaskOwner {
                person: None,
                person_key: None,
                machine: MachineRef {
                    id: None,
                    label: None
                },
            })
        );
    }

    #[test]
    fn a_private_machine_publishes_its_id_but_not_its_hostname() {
        let (_dir, store, task) = store_with_unowned_task();
        store.claim_task(&task).unwrap().unwrap();
        let local = store.local_machine().unwrap().id;
        let bodies = published_bodies(&store, &task);
        assert_eq!(bodies.len(), 1);
        assert!(bodies[0].contains(local.as_str()));
        assert!(!bodies[0].contains(&local_label()));
    }

    #[test]
    fn a_handoff_does_not_publish_the_targets_local_label() {
        let (_dir, store, task) = store_with_unowned_task();
        store.claim_task(&task).unwrap().unwrap();
        let mini = MachineId::new();
        store
            .add_machine(&mini, "ssh://mini.example", "kims-basement-mini")
            .unwrap();
        let entry = store.handoff_task(&task, &mini).unwrap();
        assert_eq!(entry.machine_label, None);
        let bodies = published_bodies(&store, &task);
        assert!(bodies.iter().any(|body| body.contains(mini.as_str())));
        assert!(bodies
            .iter()
            .all(|body| !body.contains("kims-basement-mini")));
    }
}
