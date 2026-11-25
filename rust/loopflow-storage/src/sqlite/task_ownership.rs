//! A Task's append-only ownership log and this Machine's reading of it.
//!
//! The later entry wins, in the order every Machine reads: the tracker's, or
//! without one the order each entry's writer gave it. There a claim is
//! first-wins: one whose writer had not seen the claim holding the Task loses
//! to it. Writes stay within one person's Machines: your Task's next entry is
//! written on its Machine when that is one of yours, otherwise on your Machine
//! for its Wave. Each entry names the Machine that wrote it. An entry another
//! Machine wrote never ends this Machine's own open claim; only a later entry
//! this Machine wrote does.
use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, TransactionBehavior};

use crate::{StoreError, StoreResult};
use loopflow_model::comment::{TaskComment, TaskCommentAuthor};
use loopflow_model::durable::{
    MachineId, OwnershipEntryId, OwnershipKind, ProjectId, RepositoryId, TaskId, TaskOwner,
    TaskOwnership, TaskOwnershipEntry,
};

use super::SqliteStore;

const MARKER: &str = "<!-- loopflow-ownership:";

fn read_kind(value: &str) -> StoreResult<OwnershipKind> {
    serde_json::from_value(value.into()).map_err(Into::into)
}

pub(super) fn log_in(conn: &Connection, task: &TaskId) -> StoreResult<Vec<TaskOwnershipEntry>> {
    Ok(logs_in(conn, std::slice::from_ref(task))?
        .remove(task)
        .unwrap_or_default())
}

/// Each Task's ownership log, in one statement.
fn logs_in(
    conn: &Connection,
    tasks: &[TaskId],
) -> StoreResult<HashMap<TaskId, Vec<TaskOwnershipEntry>>> {
    // Entries the tracker has not stamped yet share whole-second `at` values;
    // insertion order keeps a claim and the move after it in the order they happened.
    // Two Machines that gave their entries one order settle it by machine id.
    let mut statement = conn.prepare(
        "SELECT o.id,o.kind,coalesce(p.name,o.person),o.person_key,o.machine_id,o.machine_label,
             o.at,o.writer_machine_id,o.task_id,o.person_id,o.seen,
             CASE WHEN o.seen IS NOT NULL THEN o.ordered_at END
         FROM task_ownership o LEFT JOIN people p ON p.id=o.person_id
         WHERE o.task_id IN (SELECT value FROM json_each(?1))
         ORDER BY o.ordered_at IS NULL,o.ordered_at,
             CASE WHEN o.seen IS NOT NULL THEN o.writer_machine_id END,o.at,o.rowid",
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
                row.get::<_, Option<String>>(7)?,
                row.get::<_, String>(8)?,
                row.get::<_, Option<String>>(9)?,
                row.get::<_, Option<String>>(10)?,
                row.get::<_, Option<i64>>(11)?,
            ))
        },
    )?;
    let mut logs = HashMap::<TaskId, Vec<TaskOwnershipEntry>>::new();
    for row in rows {
        let (
            id,
            kind,
            person,
            person_key,
            machine,
            machine_label,
            at,
            writer,
            task,
            person_id,
            seen,
            order,
        ) = row?;
        let task = TaskId::from_raw(task);
        logs.entry(task.clone())
            .or_default()
            .push(TaskOwnershipEntry {
                id: OwnershipEntryId::from_raw(id),
                task_id: task,
                kind: read_kind(&kind)?,
                person,
                person_key,
                person_id,
                machine_id: machine.as_deref().map(MachineId::from_raw),
                machine_label,
                writer_machine_id: writer.as_deref().map(MachineId::from_raw),
                at,
                order,
                seen: seen.map(OwnershipEntryId::from_raw),
            });
    }
    Ok(logs)
}

/// Each listed Task's ownership as this Machine reads it, with Machine
/// labels as written; Tasks without an owner are absent.
pub(super) fn ownerships_in(
    conn: &Connection,
    tasks: &[TaskId],
) -> StoreResult<(MachineId, HashMap<TaskId, TaskOwnership>)> {
    let local = local_machine_in(conn)?;
    let mut owned = HashMap::new();
    for (task, log) in logs_in(conn, tasks)? {
        let ownership = ownership_from_log(log, &local);
        if ownership.owner.is_some() {
            owned.insert(task, ownership);
        }
    }
    Ok((local, owned))
}

/// Who holds a Task after an entry, with the Machine label as written.
fn holder_after(entry: &TaskOwnershipEntry) -> Option<TaskOwner> {
    (entry.kind != OwnershipKind::Unassign).then(|| entry.owner())
}

/// This Machine's reading of the ordered log.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Reading {
    owner: Option<TaskOwner>,
    superseded_by: Option<TaskOwner>,
    /// This Machine holds an open claim that the tracker order no longer shows,
    /// for a reason other than a later claim.
    reassert: bool,
}

/// The entries in effect, in order. A claim loses to the claim holding the
/// Task when its writer had not seen that claim; nothing later builds on it.
fn standing(rows: &[TaskOwnershipEntry]) -> Vec<&TaskOwnershipEntry> {
    let mut standing = Vec::<&TaskOwnershipEntry>::with_capacity(rows.len());
    for row in rows {
        let lost = match (standing.last(), &row.seen) {
            (Some(held), Some(seen)) => {
                row.kind == OwnershipKind::Claim && holder_after(held).is_some() && *seen != held.id
            }
            _ => false,
        };
        if !lost {
            standing.push(row);
        }
    }
    standing
}

fn read_log(rows: &[TaskOwnershipEntry], local: &MachineId) -> Reading {
    let rows = standing(rows);
    let global = rows.last().and_then(|row| holder_after(row));
    let written_here = |row: &TaskOwnershipEntry| row.writer_machine_id.as_ref() == Some(local);
    let mut local_claim = None;
    for (index, row) in rows.iter().enumerate() {
        let holder = holder_after(row);
        if holder
            .as_ref()
            .and_then(|holder| holder.machine.id.as_ref())
            == Some(local)
        {
            local_claim = Some(index);
        } else if written_here(row) {
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
    let owner = holder_after(rows[index]);
    let global_is_local = global
        .as_ref()
        .and_then(|holder| holder.machine.id.as_ref())
        == Some(local);
    let later_claim = rows[index + 1..]
        .iter()
        .rev()
        .find(|row| row.kind == OwnershipKind::Claim && !written_here(row));
    let superseded_by = match later_claim {
        Some(claim) if !global_is_local => global.clone().or_else(|| holder_after(claim)),
        _ => None,
    };
    Reading {
        reassert: !global_is_local && superseded_by.is_none(),
        owner,
        superseded_by,
    }
}

pub(super) fn local_machine_in(conn: &Connection) -> StoreResult<MachineId> {
    let id: String = conn.query_row("SELECT id FROM machines WHERE is_local=1", [], |row| {
        row.get(0)
    })?;
    Ok(MachineId::from_raw(id))
}

/// The `people` id of the person this Machine belongs to, once known.
#[cfg(test)]
fn local_person_in(conn: &Connection) -> StoreResult<Option<String>> {
    Ok(conn.query_row(
        "SELECT person_id FROM machines WHERE is_local=1",
        [],
        |row| row.get(0),
    )?)
}

/// Know `id` as a person, named `name` when one is given. An id outside
/// the trackers' forms is not a person and is skipped.
fn name_person_in(conn: &Connection, id: &str, name: Option<&str>) -> StoreResult<()> {
    conn.execute(
        "INSERT OR IGNORE INTO people(id,name) VALUES(?1,?2)",
        params![id, name],
    )?;
    conn.execute(
        "UPDATE people SET name=?2 WHERE id=?1 AND ?2 IS NOT NULL AND name IS NOT ?2",
        params![id, name],
    )?;
    Ok(())
}

fn reading_in(conn: &Connection, task: &TaskId) -> StoreResult<Reading> {
    Ok(read_log(&log_in(conn, task)?, &local_machine_in(conn)?))
}

/// Apply observed display labels without changing the saved owner.
pub fn label_owner(owner: &mut TaskOwner, names: &HashMap<MachineId, String>) {
    if let Some(label) = owner.machine.id.as_ref().and_then(|id| names.get(id)) {
        owner.machine.label = Some(label.clone());
    }
}

pub(super) fn ownership_in(conn: &Connection, task: &TaskId) -> StoreResult<TaskOwnership> {
    Ok(ownership_from_log(
        log_in(conn, task)?,
        &local_machine_in(conn)?,
    ))
}

fn ownership_from_log(log: Vec<TaskOwnershipEntry>, local: &MachineId) -> TaskOwnership {
    let reading = read_log(&log, local);
    TaskOwnership {
        owner: reading.owner,
        superseded_by: reading.superseded_by,
        log,
    }
}

/// An unfinished Task a Machine owns: the owner as the log names them, and
/// when the latest entry naming the Machine was written.
#[derive(Debug)]
pub(super) struct HeldTask {
    pub(super) task: TaskId,
    pub(super) owner: TaskOwner,
    pub(super) since: Option<i64>,
}

/// The unfinished Tasks whose current owner is `machine`. Reads only the store.
pub(super) fn held_in(conn: &Connection, machine: &MachineId) -> StoreResult<Vec<HeldTask>> {
    let candidates = {
        let mut statement = conn.prepare(
            "SELECT DISTINCT o.task_id FROM task_ownership o JOIN tasks t ON t.id=o.task_id
             WHERE o.machine_id=?1 AND t.planning_state IS NOT 'completed' AND t.abandoned_at IS NULL
             ORDER BY o.task_id",
        )?;
        let rows = statement.query_map([machine.as_str()], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    let candidates: Vec<_> = candidates.into_iter().map(TaskId::from_raw).collect();
    let (_, mut ownerships) = ownerships_in(conn, &candidates)?;
    let mut held = Vec::new();
    for task in candidates {
        let Some(ownership) = ownerships.remove(&task) else {
            continue;
        };
        let since = ownership.since(machine);
        let Some(owner) = ownership
            .owner
            .filter(|owner| owner.machine.id.as_ref() == Some(machine))
        else {
            continue;
        };
        held.push(HeldTask { task, owner, since });
    }
    Ok(held)
}

pub(super) fn owned_in(conn: &Connection, task: &TaskId) -> StoreResult<bool> {
    Ok(reading_in(conn, task)?.owner.is_some())
}

/// Whether this Machine holds the Task by a claim nobody has claimed after.
fn claimed_here_in(conn: &Connection, task: &TaskId) -> StoreResult<bool> {
    let local = local_machine_in(conn)?;
    let reading = reading_in(conn, task)?;
    Ok(reading.superseded_by.is_none()
        && reading.owner.and_then(|holder| holder.machine.id) == Some(local))
}

/// This Machine, or one registered with `lf machine add`.
fn is_my_machine_in(
    conn: &Connection,
    me: &OwnershipFacts,
    machine: &MachineId,
) -> StoreResult<bool> {
    Ok(machine == &local_machine_in(conn)? || me.declared_machines.contains_key(machine))
}

fn is_mine_in(conn: &Connection, me: &OwnershipFacts, holder: &TaskOwner) -> StoreResult<bool> {
    let local = local_machine_in(conn)?;
    Ok(holds(&me.person, holder, |machine| {
        machine == &local || me.declared_machines.contains_key(machine)
    }))
}

/// Whether a holder is you, checked in order: your `people` id; else your
/// person key (Git `user.email`), even on a Machine you registered; else, for
/// a keyless entry written before keys, one of your Machines. A different id
/// alone doesn't mean someone else: connecting Linear moves this Machine from
/// `journal:<key>` to `linear:<viewer>`, and earlier claims keep the journal
/// id. Names are display only.
fn holds(person: &Person, holder: &TaskOwner, is_mine: impl Fn(&MachineId) -> bool) -> bool {
    if holder.person_id.is_some() && holder.person_id == person.id {
        return true;
    }
    if let Some(key) = &holder.person_key {
        return person.key.as_ref() == Some(key);
    }
    holder.machine.id.as_ref().is_some_and(is_mine)
}

/// You, as an ownership reading recognizes you: your person and your
/// Machines, this one and those registered with `lf machine add`. Read once
/// per repository, then [`You::holds`] recognizes your claims without a
/// statement each.
#[derive(Debug, Clone)]
pub struct You {
    pub person: Person,
    pub machines: HashSet<MachineId>,
}

impl You {
    pub fn holds(&self, holder: &TaskOwner) -> bool {
        holds(&self.person, holder, |machine| {
            self.machines.contains(machine)
        })
    }
}

/// What an ownership write needs from outside the store: you in the Task's
/// repository (Git `user.email` is your person key), whether the repository
/// has a remote tracker, the Machines you registered and your Machine
/// for the Task's Wave or repository. Reading them spawns `git` and loads
/// configuration files, so callers read them before they take the write lock.
#[derive(Debug, Clone)]
pub struct OwnershipFacts {
    pub scope: super::scope_defaults::TaskScope,
    pub person: Person,
    pub planning_connected: bool,
    pub declared_machines: HashMap<MachineId, String>,
    pub local_label: String,
    pub owning_machine: MachineId,
}

/// The person an entry names: their display name, stable key and `people` id.
#[derive(Debug, Clone, Default)]
pub struct Person {
    pub name: Option<String>,
    pub key: Option<String>,
    pub id: Option<String>,
}

impl From<TaskOwner> for Person {
    fn from(owner: TaskOwner) -> Self {
        Self {
            name: owner.person,
            key: owner.person_key,
            id: owner.person_id,
        }
    }
}

impl OwnershipFacts {
    fn claimant(&self) -> StoreResult<&Person> {
        if self.person.key.is_none() {
            return Err(StoreError::InvalidData(
                "claiming needs your Git user.email, which identifies you to teammates; run `git config --global user.email you@example.com`".into(),
            ));
        }
        Ok(&self.person)
    }

    pub fn planning_connected(&self) -> bool {
        self.planning_connected
    }
    pub fn matches_task_in(&self, conn: &Connection, task: &TaskId) -> StoreResult<bool> {
        Ok(super::scope_defaults::task_scope_in(conn, task)? == self.scope)
    }
    pub fn matches_new_task_in(
        &self,
        conn: &Connection,
        repository: &RepositoryId,
        project: Option<&ProjectId>,
    ) -> StoreResult<bool> {
        Ok(super::scope_defaults::new_task_scope_in(conn, repository, project)? == self.scope)
    }
}

/// The one Machine of yours that writes the Task's next entry: the owner's,
/// when you own it on one of your Machines, else your Machine for its Wave.
/// Someone else's Machine is never a writer for you.
pub(super) fn writer_in(
    conn: &Connection,
    task: &TaskId,
    me: &OwnershipFacts,
) -> StoreResult<(MachineId, bool)> {
    let reading = reading_in(conn, task)?;
    if let Some(holder) = &reading.owner {
        if let Some(machine) = &holder.machine.id {
            if is_mine_in(conn, me, holder)? && is_my_machine_in(conn, me, machine)? {
                return Ok((machine.clone(), true));
            }
        }
    }
    Ok((me.owning_machine.clone(), false))
}

fn require_writer_in(
    conn: &Connection,
    task: &TaskId,
    me: &OwnershipFacts,
) -> StoreResult<MachineId> {
    let local = local_machine_in(conn)?;
    let (writer, _) = writer_in(conn, task, me)?;
    // Until a move this Machine began arrives, its target has not taken the
    // Task: the next entry, which ends the move, is written here.
    if writer != local && super::task_moves::move_in_flight_in(conn, task)?.is_none() {
        return Err(StoreError::InvalidAuthority(format!(
            "Task {task} ownership is written on Machine {writer}; run the command there"
        )));
    }
    Ok(local)
}

/// A move and an unclaim end your own claim; someone else's stays theirs.
/// `None` when nobody owns the Task.
fn my_claim_in(
    conn: &Connection,
    task: &TaskId,
    me: &OwnershipFacts,
) -> StoreResult<Option<(MachineId, TaskOwner)>> {
    let local = require_writer_in(conn, task, me)?;
    let reading = reading_in(conn, task)?;
    match reading.owner {
        None => Ok(None),
        Some(holder) if !is_mine_in(conn, me, &holder)? => {
            Err(StoreError::InvalidAuthority(format!(
                "Task {task} is claimed by {}; claim it to take it over",
                holder.person.as_deref().unwrap_or("someone else")
            )))
        }
        Some(holder) => Ok(Some((local, holder))),
    }
}

/// A claim by the same person naming `target`: the Task moves there.
fn move_in(
    conn: &Connection,
    task: &TaskId,
    target: &MachineId,
    me: &OwnershipFacts,
) -> StoreResult<TaskOwnershipEntry> {
    let (local, person) = match my_claim_in(conn, task, me)? {
        Some((local, holder)) => (local, holder.into()),
        None => {
            let person = me.claimant()?.clone();
            if me.planning_connected {
                start_claimed_task(conn, task)?;
            }
            (me.scope.placement.local_machine.clone(), person)
        }
    };
    if *target == local {
        return Err(StoreError::InvalidData(format!(
            "Task {task} is already on this Machine"
        )));
    }
    append_in(
        conn,
        task,
        OwnershipKind::Claim,
        person,
        target,
        me.planning_connected,
    )
}

/// Published text names only what the entry carries: a public name, else the
/// Machine id. Local labels and hostnames never leave this Machine.
fn describe(entry: &TaskOwnershipEntry) -> String {
    let who =
        loopflow_model::durable::person_text(entry.person.as_deref(), entry.person_key.as_deref());
    let place = entry
        .machine_label
        .clone()
        .or(entry.machine_id.as_ref().map(|id| id.to_string()))
        .unwrap_or_else(|| "an unknown Machine".into());
    match entry.kind {
        OwnershipKind::Claim => format!("{who} claimed this on {place}"),
        OwnershipKind::Unassign => format!("{who} unassigned this on {place}"),
    }
}

/// The marked text an entry travels in; `ingest_ownership_marker` reads it.
pub fn marker(entry: &TaskOwnershipEntry) -> StoreResult<String> {
    // Escape angle brackets so a person name cannot close the HTML comment.
    let json = serde_json::to_string(entry)?
        .replace('<', "\\u003c")
        .replace('>', "\\u003e");
    Ok(format!("{MARKER}{json} -->"))
}

/// Append a local entry and deliver it as a marked Task comment. The comment
/// travels through the existing Linear delivery or to the hub.
fn append_in(
    conn: &Connection,
    task: &TaskId,
    kind: OwnershipKind,
    person: Person,
    machine: &MachineId,
    planning_connected: bool,
) -> StoreResult<TaskOwnershipEntry> {
    let local = local_machine_in(conn)?;
    if let Some(id) = &person.id {
        name_person_in(conn, id, person.name.as_deref())?;
    }
    // With a tracker its comment order decides, and no claim loses.
    let id = OwnershipEntryId::new();
    let (order, seen) = if planning_connected {
        (None, None)
    } else {
        let seen = standing(&log_in(conn, task)?)
            .last()
            .map_or_else(|| id.clone(), |held| held.id.clone());
        (Some(next_order_in(conn, task)?), Some(seen))
    };
    let entry = TaskOwnershipEntry {
        id,
        task_id: task.clone(),
        kind,
        person: person.name,
        person_key: person.key,
        person_id: person.id,
        machine_id: Some(machine.clone()),
        // Private Machines publish no label; teammates see the id.
        machine_label: if *machine == local {
            super::durable::local_public_name_in(conn)?
        } else {
            None
        },
        writer_machine_id: Some(local.clone()),
        at: time::OffsetDateTime::now_utc().unix_timestamp(),
        order,
        seen,
    };
    conn.execute(
        "INSERT INTO task_ownership(id,task_id,kind,person,person_key,machine_id,machine_label,
             at,ordered_at,writer_machine_id,written_here,person_id,seen)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,1,?11,?12)",
        params![
            entry.id.as_str(),
            task.as_str(),
            kind.as_str(),
            entry.person,
            entry.person_key,
            machine.as_str(),
            entry.machine_label,
            entry.at,
            entry.order,
            local.as_str(),
            entry.person_id,
            entry.seen.as_ref().map(OwnershipEntryId::as_str)
        ],
    )?;
    let created_at = match entry.order {
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
    // A move keeps the person, so planning's assignee and state stay.
    if planning_connected {
        match kind {
            OwnershipKind::Claim if *machine == local => {
                start_claimed_task(conn, task)?;
            }
            OwnershipKind::Unassign => {
                super::planning_write::local(
                    conn,
                    &crate::work::edit::WorkEdit::for_target(
                        task.as_str(),
                        &[crate::work::edit::WorkValue::TaskAssignee(None)],
                    ),
                )
                .map(|_| ())?;
            }
            OwnershipKind::Claim => {}
        }
    }
    Ok(entry)
}

fn start_claimed_task(conn: &Connection, task: &TaskId) -> StoreResult<()> {
    let state: Option<String> = conn.query_row(
        "SELECT planning_state FROM tasks WHERE id=?1",
        [task.as_str()],
        |row| row.get(0),
    )?;
    if matches!(
        state.as_deref(),
        None | Some("unstarted" | "backlog" | "triage")
    ) {
        super::task_state::decide_in(conn, task, "started")?;
    }
    Ok(())
}

/// Claim for this Machine when the Task is unowned or its owner's Machine is
/// unknown. Returns the new entry, or None when this Machine already owns it.
pub(super) fn claim_in(
    conn: &Connection,
    task: &TaskId,
    me: &OwnershipFacts,
) -> StoreResult<Option<TaskOwnershipEntry>> {
    let local = require_writer_in(conn, task, me)?;
    if claimed_here_in(conn, task)? {
        return Ok(None);
    }
    append_in(
        conn,
        task,
        OwnershipKind::Claim,
        me.claimant()?.clone(),
        &local,
        me.planning_connected,
    )
    .map(Some)
}

/// The order an entry written here carries to every Machine: this Machine's
/// plan clock, and after every entry it holds for the Task, whatever their
/// writers' clocks said.
fn next_order_in(conn: &Connection, task: &TaskId) -> StoreResult<i64> {
    let latest: i64 = conn.query_row(
        "SELECT COALESCE(max(ordered_at)+1,0) FROM task_ownership WHERE task_id=?1",
        [task.as_str()],
        |row| row.get(0),
    )?;
    Ok(super::planning_write::next_at(conn)?.max(latest))
}

/// The tracker's order for an entry its comment carries: the comment's time.
pub(super) fn provider_order(created_at: Option<&str>) -> Option<i64> {
    created_at
        .and_then(|at| {
            time::OffsetDateTime::parse(at, &time::format_description::well_known::Rfc3339).ok()
        })
        .map(|at| (at.unix_timestamp_nanos() / 1_000_000) as i64)
}

/// Parse a marked ownership comment into the log. Repeats are ignored. An
/// entry's own order is the one every Machine keeps; without one the
/// tracker's (`ordered_at`) is stamped once, the first time it is reported.
pub(super) fn ingest_marker_in(
    conn: &Connection,
    task: &TaskId,
    body: &str,
    ordered_at: Option<i64>,
    machine_names: &HashMap<MachineId, String>,
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
    if entry.task_id != *task
        || entry.machine_id.is_none()
        || entry.order.is_none() != entry.seen.is_none()
    {
        return Ok(false);
    }
    let ordered_at = entry.order.or(ordered_at);
    let held = claimed_here_in(conn, task)?;
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO task_ownership(id,task_id,kind,person,person_key,machine_id,
             machine_label,at,ordered_at,writer_machine_id,written_here,person_id,seen)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,0,?11,?12)",
        params![
            entry.id.as_str(),
            task.as_str(),
            entry.kind.as_str(),
            entry.person,
            entry.person_key,
            entry.machine_id.as_ref().map(MachineId::as_str),
            entry.machine_label,
            entry.at,
            ordered_at,
            entry.writer_machine_id.as_ref().map(MachineId::as_str),
            entry.person_id,
            entry.seen.as_ref().map(OwnershipEntryId::as_str)
        ],
    )? > 0;
    if let (true, Some(id)) = (inserted, &entry.person_id) {
        name_person_in(conn, id, entry.person.as_deref())?;
    }
    if !inserted {
        if let Some(ordered_at) = ordered_at {
            conn.execute(
                "UPDATE task_ownership SET ordered_at=?2 WHERE id=?1 AND ordered_at IS NULL",
                params![entry.id.as_str(), ordered_at],
            )?;
        }
    }
    if inserted && entry.kind == OwnershipKind::Claim {
        notify_superseded_in(conn, task, &entry, held, machine_names)?;
    }
    Ok(inserted)
}

/// What an arriving claim means for this Machine's own, told to live Sessions
/// as direction. A later claim that this Machine's open claim outlives cancels
/// nothing; the claimant decides what to do. A claim made first, which this
/// Machine's claim (`held` before the arrival) was written without seeing,
/// holds the Task: the run here is interrupted and its work stays where it is.
/// `machine_names` is observed above SQL, read before the write.
fn notify_superseded_in(
    conn: &Connection,
    task: &TaskId,
    entry: &TaskOwnershipEntry,
    held: bool,
    machine_names: &HashMap<MachineId, String>,
) -> StoreResult<()> {
    let reading = reading_in(conn, task)?;
    let local = local_machine_in(conn)?;
    let (text, lost) = match (reading.owner, reading.superseded_by) {
        (Some(mut owner), Some(mut later)) => {
            label_owner(&mut owner, machine_names);
            label_owner(&mut later, machine_names);
            let text = format!(
                "Also claimed later by {later}. {owner} still owns this Task on this Machine and \
                 nothing was cancelled. Keep going, or run `lf task unassign` or \
                 `lf task move <task> --machine <machine>` to end this claim."
            );
            (text, false)
        }
        (Some(mut first), None) if held && first.machine.id.as_ref() != Some(&local) => {
            label_owner(&mut first, machine_names);
            let text = format!(
                "{first} claimed this Task first, before this Machine had seen it, so the \
                 claim made here did not hold. The run here was stopped and its work stays \
                 in this checkout. Run `lf task claim <task>` to take the Task over."
            );
            (text, true)
        }
        _ => return Ok(()),
    };
    let steer = super::steers::save_in(
        conn,
        ("task", task.as_str()),
        &format!("ownership:{}", entry.id),
        "local",
        &loopflow_model::durable::Author::Named("Loopflow".into()),
        &text,
    )?;
    super::children::insert_task_event_in(
        conn,
        task,
        &crate::work::task::TaskEventKind::Steer { steer_id: steer.id },
    )?;
    if lost {
        super::children::insert_task_event_in(
            conn,
            task,
            &crate::work::task::TaskEventKind::Interrupt,
        )?;
    }
    Ok(())
}

/// After an inbound observation: an open local claim that the tracker order no
/// longer shows, for a reason other than a later claim, is claimed again.
pub(super) fn reassert_in(
    conn: &Connection,
    task: &TaskId,
    planning_connected: bool,
) -> StoreResult<Option<TaskOwnershipEntry>> {
    let reading = reading_in(conn, task)?;
    let (true, Some(holder)) = (reading.reassert, reading.owner) else {
        return Ok(None);
    };
    append_in(
        conn,
        task,
        OwnershipKind::Claim,
        holder.into(),
        &local_machine_in(conn)?,
        planning_connected,
    )
    .map(Some)
}

/// Repository evidence for the complete held set, acquired without configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeldPlanningScope {
    pub machine: MachineId,
    pub repositories: HashMap<TaskId, (RepositoryId, String)>,
}

pub(super) fn held_planning_scope_in(
    conn: &Connection,
    machine: &MachineId,
) -> StoreResult<HeldPlanningScope> {
    let held = held_in(conn, machine)?;
    let mut query = conn.prepare(
        "SELECT t.id,t.repository_id,r.repo FROM json_each(?1) ids
         CROSS JOIN tasks t ON t.id=ids.value
         CROSS JOIN repository_plans r ON r.id=t.repository_id",
    )?;
    let repositories = query
        .query_map(
            [super::children::ids_json(
                held.iter().map(|held| held.task.as_str()),
            )],
            |row| {
                Ok((
                    TaskId::from_raw(row.get::<_, String>(0)?),
                    (
                        RepositoryId::from_raw(row.get::<_, String>(1)?),
                        row.get(2)?,
                    ),
                ))
            },
        )?
        .collect::<Result<HashMap<_, _>, _>>()?;
    Ok(HeldPlanningScope {
        machine: machine.clone(),
        repositories,
    })
}

impl SqliteStore {
    pub fn held_planning_scope(&self, machine: &MachineId) -> StoreResult<HeldPlanningScope> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        held_planning_scope_in(&snapshot, machine)
    }
}

/// Move each open Task an ended Machine still owns to this Machine, in its
/// owner's name: the claim survives and the log keeps the Machine as history.
/// `planning` was observed for the complete saved `scope`. Returns `None` without writing
/// if a new claim needs planning facts; roll back and read them before retrying.
pub(super) fn hand_back_in(
    conn: &Connection,
    machine: &MachineId,
    scope: &HeldPlanningScope,
    planning: &HashMap<TaskId, bool>,
) -> StoreResult<Option<Vec<TaskId>>> {
    if scope.machine != *machine || held_planning_scope_in(conn, machine)? != *scope {
        return Ok(None);
    }
    let local = local_machine_in(conn)?;
    let held = held_in(conn, machine)?;
    if held.iter().any(|held| !planning.contains_key(&held.task)) {
        return Ok(None);
    }
    let mut tasks = Vec::with_capacity(held.len());
    for HeldTask { task, owner, .. } in held {
        let planning_connected = planning[&task];
        append_in(
            conn,
            &task,
            OwnershipKind::Claim,
            owner.into(),
            &local,
            planning_connected,
        )?;
        tasks.push(task);
    }
    Ok(Some(tasks))
}

impl SqliteStore {
    /// This Machine belongs to the person `id` names, as their planning
    /// tracker knows them; `name` is how they are shown.
    pub fn record_local_person(&self, id: &str, name: Option<&str>) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        name_person_in(&tx, id, name)?;
        tx.execute(
            "UPDATE machines SET person_id=?1 WHERE is_local=1 AND person_id IS NOT ?1
             AND EXISTS(SELECT 1 FROM people WHERE id=?1)",
            [id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn saved_task_ownership(&self, task: &TaskId) -> StoreResult<(MachineId, TaskOwnership)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        Ok((local_machine_in(&snapshot)?, ownership_in(&snapshot, task)?))
    }

    pub fn task_owned(&self, task: &TaskId) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        owned_in(&snapshot, task)
    }

    /// Whether this Machine holds an open claim that Linear should show.
    pub fn task_claimed_here(&self, task: &TaskId) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        claimed_here_in(&snapshot, task)
    }

    pub fn claim_task_prepared(
        &self,
        task: &TaskId,
        me: &OwnershipFacts,
    ) -> StoreResult<Option<Option<TaskOwnershipEntry>>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !me.matches_task_in(&tx, task)? {
            return Ok(None);
        }
        super::children::task_on(&tx, task)?.ok_or(StoreError::NotFound)?;
        let entry = claim_in(&tx, task, me)?;
        tx.commit()?;
        Ok(Some(entry))
    }

    /// Move your claim to `target`; the Task's primary Session here is
    /// completed, and the target admits its own on first connect.
    pub fn move_claim_prepared(
        &self,
        task: &TaskId,
        target: &MachineId,
        me: &OwnershipFacts,
    ) -> StoreResult<Option<TaskOwnershipEntry>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !me.matches_task_in(&tx, task)? {
            return Ok(None);
        }
        let entry = move_in(&tx, task, target, me)?;
        super::sessions::complete_primary_session_in(
            &tx,
            &crate::session::PrimaryScope::Task(task.clone()),
        )?;
        tx.commit()?;
        Ok(Some(entry))
    }

    /// Begin moving the Task to `target`: write its claim naming the target
    /// and record the move as leaving, in one
    /// transaction. Written before the move stops anything, so nothing here
    /// starts the Task again; its conversation travels with it, and the move
    /// completes it once taken.
    pub fn begin_task_move_prepared(
        &self,
        task: &TaskId,
        target: &MachineId,
        me: &OwnershipFacts,
    ) -> StoreResult<Option<TaskOwnershipEntry>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !me.matches_task_in(&tx, task)? {
            return Ok(None);
        }
        let entry = move_in(&tx, task, target, me)?;
        tx.execute(
            "DELETE FROM task_moves WHERE task_id=?1 AND state IN ('leaving','sent')",
            [task.as_str()],
        )?;
        tx.execute(
            "INSERT INTO task_moves(task_id,from_machine_id,to_machine_id,state,at,claim_id)
             VALUES(?1,?2,?3,'leaving',?4,?5)",
            params![
                task.as_str(),
                local_machine_in(&tx)?.as_str(),
                target.as_str(),
                crate::rows::now_unix(),
                entry.id.as_str()
            ],
        )?;
        tx.commit()?;
        Ok(Some(entry))
    }

    /// Take back `sent`, the move claim this Machine wrote, when the move it
    /// began could not leave; the move is no longer leaving. Nothing may follow
    /// the claim in the log: until it is received, its target has not taken
    /// the Task. The log keeps both entries.
    pub fn hand_back_prepared(
        &self,
        task: &TaskId,
        sent: &TaskOwnershipEntry,
        me: &OwnershipFacts,
    ) -> StoreResult<Option<TaskOwnershipEntry>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !me.matches_task_in(&tx, task)? {
            return Ok(None);
        }
        let rows = log_in(&tx, task)?;
        if rows.last().map(|row| &row.id) != Some(&sent.id) {
            return Err(StoreError::InvalidAuthority(format!(
                "Task {task} changed hands after its move {}; it stays where that left it",
                sent.id
            )));
        }
        let entry = append_in(
            &tx,
            task,
            OwnershipKind::Claim,
            sent.owner().into(),
            &local_machine_in(&tx)?,
            me.planning_connected,
        )?;
        tx.execute(
            "DELETE FROM task_moves WHERE task_id=?1 AND state='leaving'",
            [task.as_str()],
        )?;
        tx.commit()?;
        Ok(Some(entry))
    }

    /// Take in an entry another Machine wrote, before planning delivers its
    /// comment. Without a tracker it carries its own order, which its writer
    /// gave it after every entry it held for the Task.
    pub fn ingest_ownership_marker_prepared(
        &self,
        task: &TaskId,
        marker: &str,
        me: &OwnershipFacts,
    ) -> StoreResult<Option<bool>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !me.matches_task_in(&tx, task)? {
            return Ok(None);
        }
        let mut names = me.declared_machines.clone();
        names.insert(
            me.scope.placement.local_machine.clone(),
            me.local_label.clone(),
        );
        let inserted = ingest_marker_in(&tx, task, marker, None, &names)?;
        tx.commit()?;
        Ok(Some(inserted))
    }

    /// End your claim. An unowned Task is already unclaimed and writes nothing.
    pub fn unassign_task_prepared(
        &self,
        task: &TaskId,
        me: &OwnershipFacts,
    ) -> StoreResult<Option<Option<TaskOwnershipEntry>>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if !me.matches_task_in(&tx, task)? {
            return Ok(None);
        }
        let Some((local, holder)) = my_claim_in(&tx, task, me)? else {
            return Ok(Some(None));
        };
        let person = Person {
            name: me.person.name.clone(),
            key: me.person.key.clone().or(holder.person_key),
            id: me.person.id.clone().or(holder.person_id),
        };
        let entry = append_in(
            &tx,
            task,
            OwnershipKind::Unassign,
            person,
            &local,
            me.planning_connected,
        )?;
        tx.commit()?;
        Ok(Some(Some(entry)))
    }

    /// Queue the claiming person, or nobody, as Linear assignee unless Linear already shows it.
    pub fn assign_task_owner(&self, task: &TaskId, assignee: Option<&str>) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: Option<String> = tx.query_row(
            "SELECT planning_assignee FROM tasks WHERE id=?1",
            [task.as_str()],
            |row| row.get(0),
        )?;
        if current.as_deref() != assignee {
            super::planning_write::local(
                &tx,
                &crate::work::edit::WorkEdit::for_target(
                    task.as_str(),
                    &[crate::work::edit::WorkValue::TaskAssignee(
                        assignee.map(str::to_owned),
                    )],
                ),
            )
            .map(|_| ())?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{log_in, read_log, Reading};
    use crate::test_fixture::unowned_task as store_with_unowned_task;
    use loopflow_model::durable::{
        MachineId, MachineRef, OwnershipEntryId, OwnershipKind, TaskId, TaskOwner,
        TaskOwnershipEntry,
    };

    fn row(
        kind: OwnershipKind,
        person: &str,
        machine: &MachineId,
        writer: &MachineId,
    ) -> TaskOwnershipEntry {
        TaskOwnershipEntry {
            id: OwnershipEntryId::new(),
            task_id: TaskId::new(),
            kind,
            person: Some(person.into()),
            person_key: None,
            person_id: None,
            machine_id: Some(machine.clone()),
            machine_label: None,
            writer_machine_id: Some(writer.clone()),
            at: 1,
            order: None,
            seen: None,
        }
    }

    fn holder(person: &str, machine: &MachineId) -> Option<TaskOwner> {
        Some(TaskOwner {
            person: Some(person.into()),
            person_key: None,
            person_id: None,
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
            row(OwnershipKind::Claim, "Jack", &laptop, &laptop),
            row(OwnershipKind::Claim, "Kim", &mini, &mini),
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
    fn only_a_later_entry_written_here_ends_the_local_claim() {
        let (laptop, mini) = (MachineId::new(), MachineId::new());
        let mut rows = vec![
            row(OwnershipKind::Claim, "Jack", &laptop, &laptop),
            row(OwnershipKind::Unassign, "Kim", &mini, &mini),
        ];
        let reading = read_log(&rows, &laptop);
        assert_eq!(reading.owner, holder("Jack", &laptop));
        assert!(reading.reassert);
        // The move: the same person's claim naming mini, written here.
        rows.push(row(OwnershipKind::Claim, "Jack", &mini, &laptop));
        assert_eq!(read_log(&rows, &laptop).owner, holder("Jack", &mini));
        // Every Machine stores the same rows and reads the same owner.
        assert_eq!(read_log(&rows, &mini).owner, holder("Jack", &mini));
        rows.push(row(OwnershipKind::Unassign, "Jack", &mini, &mini));
        assert_eq!(read_log(&rows, &laptop).owner, None);
    }

    #[test]
    fn unstamped_entries_in_one_second_keep_their_written_order() {
        let (_dir, store, task) = store_with_unowned_task();
        let conn = store.conn.lock().unwrap();
        // Linear stamps tracker order later; until then a claim and the move
        // after it share a whole-second `at`, and the move's id may sort first.
        // An earlier lf wrote the move as `handoff`, which reads as a claim.
        for (id, kind) in [("own_zzzz", "claim"), ("own_aaaa", "handoff")] {
            conn.execute(
                "INSERT INTO task_ownership(id,task_id,kind,person,machine_id,at,ordered_at,written_here)
                 VALUES(?1,?2,?3,'Jack',?4,1,NULL,1)",
                rusqlite::params![id, task.as_str(), kind, MachineId::new().as_str()],
            )
            .unwrap();
        }
        let entries: Vec<(String, OwnershipKind)> = log_in(&conn, &task)
            .unwrap()
            .iter()
            .map(|row| (row.id.to_string(), row.kind))
            .collect();
        assert_eq!(
            entries,
            [
                ("own_zzzz".to_string(), OwnershipKind::Claim),
                ("own_aaaa".to_string(), OwnershipKind::Claim)
            ]
        );
    }

    #[test]
    fn a_recorded_entry_with_an_unknown_machine_is_owned_without_a_machine() {
        let laptop = MachineId::new();
        // Written as `recorded` before ownership travelled; it reads as a claim.
        let mut recorded = row(OwnershipKind::Claim, "", &laptop, &laptop);
        recorded.person = None;
        recorded.machine_id = None;
        assert_eq!(
            read_log(&[recorded], &laptop).owner,
            Some(TaskOwner {
                person: None,
                person_key: None,
                person_id: None,
                machine: MachineRef {
                    id: None,
                    label: None
                },
            })
        );
    }

    #[test]
    fn entries_a_released_lf_wrote_here_name_this_machine_as_their_writer() {
        use crate::migration_runner::{apply_before_migration, migration_section_sql};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "task_machine_local",
        );
        let laptop = super::local_machine_in(&conn).unwrap();
        let mini = MachineId::new();
        for (id, machine, here) in [("own_here", &laptop, 1), ("own_there", &mini, 0)] {
            conn.execute(
                "INSERT INTO task_ownership(id,task_id,kind,person,machine_id,at,written_here)
                 VALUES(?1,'task_1','claim','Jack',?2,1,?3)",
                rusqlite::params![id, machine.as_str(), here],
            )
            .unwrap();
        }
        conn.execute_batch(&migration_section_sql("task_machine_local"))
            .unwrap();
        let writers: Vec<(String, Option<String>)> = conn
            .prepare("SELECT id,writer_machine_id FROM task_ownership ORDER BY id")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        assert_eq!(
            writers,
            [
                ("own_here".to_string(), Some(laptop.to_string())),
                ("own_there".to_string(), None)
            ]
        );
        // Still append-only, writer included.
        assert!(conn
            .execute("UPDATE task_ownership SET writer_machine_id=NULL", [])
            .is_err());
    }

    #[test]
    fn the_planning_journal_user_names_this_machines_person() {
        use crate::migration_runner::{apply_before_migration, migration_section_sql};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(
            &crate::test_fixture::observers(),
            &crate::test_fixture::build(),
            &conn,
            "task_machine_local",
        );
        let key = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO planning_user(singleton,user_key) VALUES(1,?1)",
            [&key],
        )
        .unwrap();
        conn.execute_batch(&migration_section_sql("task_machine_local"))
            .unwrap();
        assert_eq!(
            super::local_person_in(&conn).unwrap(),
            Some(format!("journal:{key}"))
        );
    }
}
