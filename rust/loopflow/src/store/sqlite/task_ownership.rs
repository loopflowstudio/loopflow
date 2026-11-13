//! A Task's append-only ownership log and this Machine's reading of it.
//!
//! The later entry wins, in provider order. Writes stay within one person's
//! Machines: your Task's next entry is written on its Machine when that is one
//! of yours, otherwise on your Machine for its Wave. Each entry names the
//! Machine that wrote it. An entry another Machine wrote never ends this
//! Machine's own open claim; only a later entry this Machine wrote does.
use std::collections::HashMap;

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::durable::{
    MachineId, MachineRef, MachineSetting, OwnershipEntryId, OwnershipKind, OwningMachineSource,
    ProjectId, RepositoryId, TaskExecutionSource, TaskId, TaskOwner, TaskOwnership,
    TaskOwnershipEntry,
};
use crate::store::{StoreError, StoreResult};
use crate::work::task::comment::{TaskComment, TaskCommentAuthor};

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
    // Entries the provider has not stamped yet share whole-second `at` values;
    // insertion order keeps a claim and the move after it in the order they happened.
    let mut statement = conn.prepare(
        "SELECT o.id,o.kind,coalesce(p.name,o.person),o.person_key,o.machine_id,o.machine_label,
             o.at,o.writer_machine_id,o.task_id,o.person_id
         FROM task_ownership o LEFT JOIN people p ON p.id=o.person_id
         WHERE o.task_id IN (SELECT value FROM json_each(?1))
         ORDER BY o.ordered_at IS NULL,o.ordered_at,o.at,o.rowid",
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
            ))
        },
    )?;
    let mut logs = HashMap::<TaskId, Vec<TaskOwnershipEntry>>::new();
    for row in rows {
        let (id, kind, person, person_key, machine, machine_label, at, writer, task, person_id) =
            row?;
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
    let labels = machine_names_in(conn)?;
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
    (entry.kind != OwnershipKind::Unassign).then(|| entry.owner())
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

fn read_log(rows: &[TaskOwnershipEntry], local: &MachineId) -> Reading {
    let global = rows.iter().fold(None, |_, row| holder_after(row));
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
    let owner = holder_after(&rows[index]);
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
fn local_person_in(conn: &Connection) -> StoreResult<Option<String>> {
    Ok(conn.query_row(
        "SELECT person_id FROM machines WHERE is_local=1",
        [],
        |row| row.get(0),
    )?)
}

/// Know `id` as a person, named `name` when one is given. An id outside
/// the planning providers' forms is not a person and is skipped.
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

fn reading_in(conn: &Connection, task: &TaskId) -> StoreResult<(Vec<TaskOwnershipEntry>, Reading)> {
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

/// The Machines you registered with `lf machine add`, each with the name you
/// gave it. Reading them loads configuration files: read before the write lock.
pub(super) fn machine_names_in(conn: &Connection) -> StoreResult<HashMap<MachineId, String>> {
    Ok(crate::config::placement::machine_declarations(conn)
        .map_err(|error| StoreError::InvalidData(error.to_string()))?
        .into_iter()
        .map(|(name, declaration)| (declaration.id, name))
        .collect())
}

/// A Machine's best known label: this Machine's hostname, then the name you
/// registered, then the label its claimant wrote.
fn machine_ref(
    machine: Option<&MachineId>,
    written: Option<&str>,
    local: &MachineId,
    names: &HashMap<MachineId, String>,
) -> MachineRef {
    let label = match machine {
        Some(machine) if machine == local => Some(local_label()),
        Some(machine) => names.get(machine).cloned().or(written.map(Into::into)),
        None => written.map(Into::into),
    };
    MachineRef {
        id: machine.cloned(),
        label,
    }
}

/// An owner with their Machine's label as best known here.
fn resolved(
    owner: Option<TaskOwner>,
    local: &MachineId,
    names: &HashMap<MachineId, String>,
) -> Option<TaskOwner> {
    owner.map(|mut owner| {
        owner.machine = machine_ref(
            owner.machine.id.as_ref(),
            owner.machine.label.as_deref(),
            local,
            names,
        );
        owner
    })
}

pub(super) fn ownership_in(conn: &Connection, task: &TaskId) -> StoreResult<TaskOwnership> {
    let names = machine_names_in(conn)?;
    let local = local_machine_in(conn)?;
    let (rows, reading) = reading_in(conn, task)?;
    let log = rows
        .into_iter()
        .map(|row| {
            let mut entry = row;
            entry.machine_label = machine_ref(
                entry.machine_id.as_ref(),
                entry.machine_label.as_deref(),
                &local,
                &names,
            )
            .label;
            entry
        })
        .collect();
    Ok(TaskOwnership {
        owner: resolved(reading.owner, &local, &names),
        superseded_by: resolved(reading.superseded_by, &local, &names),
        log,
    })
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
             WHERE o.machine_id=?1 AND t.planning_completed=0 AND t.abandoned_at IS NULL
             ORDER BY o.task_id",
        )?;
        let rows = statement.query_map([machine.as_str()], |row| row.get::<_, String>(0))?;
        rows.collect::<Result<Vec<_>, _>>()?
    };
    let mut held = Vec::new();
    for task in candidates {
        let task = TaskId::from_raw(task);
        let (rows, reading) = reading_in(conn, &task)?;
        let Some(owner) = reading.owner else {
            continue;
        };
        if owner.machine.id.as_ref() != Some(machine) {
            continue;
        }
        let since = rows
            .iter()
            .rev()
            .find(|row| row.machine_id.as_ref() == Some(machine))
            .map(|row| row.at);
        held.push(HeldTask { task, owner, since });
    }
    Ok(held)
}

pub(super) fn owned_in(conn: &Connection, task: &TaskId) -> StoreResult<bool> {
    Ok(reading_in(conn, task)?.1.owner.is_some())
}

/// This Machine, or one registered with `lf machine add`.
fn is_my_machine_in(
    conn: &Connection,
    me: &OwnershipFacts,
    machine: &MachineId,
) -> StoreResult<bool> {
    Ok(machine == &local_machine_in(conn)? || me.declared_machines.contains_key(machine))
}

/// Whether a holder is you. A holder and you both named by a `people` id are
/// the same person exactly when the ids match. Otherwise a holder with a person key (Git `user.email`) is
/// you exactly when the key is yours, even on a Machine you registered. A
/// keyless entry, written before keys or recorded before ownership existed, is
/// yours when it holds one of your Machines. Names are display only.
fn is_mine_in(conn: &Connection, me: &OwnershipFacts, holder: &TaskOwner) -> StoreResult<bool> {
    if let (Some(id), Some(mine)) = (&holder.person_id, &me.person.id) {
        return Ok(id == mine);
    }
    if let Some(key) = &holder.person_key {
        return Ok(me.person.key.as_ref() == Some(key));
    }
    match &holder.machine.id {
        Some(machine) => is_my_machine_in(conn, me, machine),
        None => Ok(false),
    }
}

/// What an ownership write needs from outside the store: you in the Task's
/// repository (Git `user.email` is your person key), whether the repository
/// has a remote planning provider, the Machines you registered and your Machine
/// for the Task's Wave or repository. Reading them spawns `git` and loads
/// configuration files, so callers read them before they take the write lock.
#[derive(Debug, Clone)]
pub(crate) struct OwnershipFacts {
    person: Person,
    planning_connected: bool,
    declared_machines: HashMap<MachineId, String>,
    owning_machine: (MachineId, OwningMachineSource),
}

/// The person an entry names: their display name, stable key and `people` id.
#[derive(Debug, Clone, Default)]
struct Person {
    name: Option<String>,
    key: Option<String>,
    id: Option<String>,
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
    fn in_scope(
        conn: &Connection,
        repo: &str,
        wave: Option<String>,
        repository: String,
    ) -> StoreResult<Self> {
        Ok(Self {
            person: Person {
                name: crate::config::participant_name()
                    .map_err(|error| StoreError::InvalidData(error.to_string()))?,
                key: crate::config::participant_email(std::path::Path::new(repo)),
                id: local_person_in(conn)?,
            },
            planning_connected: crate::ops::planning_client::connected(repo),
            declared_machines: machine_names_in(conn)?,
            owning_machine: super::scope_defaults::owning_machine_in(
                conn,
                wave,
                Some(repository),
                MachineSetting::Tasks,
            )?,
        })
    }

    pub(crate) fn planning_connected(&self) -> bool {
        self.planning_connected
    }

    /// You in the Task's repository.
    pub(crate) fn of_task_in(conn: &Connection, task: &TaskId) -> StoreResult<Self> {
        let (repo, wave, repository): (String, Option<String>, String) = conn
            .query_row(
                "SELECT w.repo,p.wave_id,t.repository_id FROM tasks t
                 JOIN repository_plans w ON w.id=t.repository_id
                 LEFT JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
                [task.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        Self::in_scope(conn, &repo, wave, repository)
    }

    /// You in the repository and Project of a Task about to be created.
    pub(crate) fn of_new_task_in(
        conn: &Connection,
        repository: &RepositoryId,
        project: Option<&ProjectId>,
    ) -> StoreResult<Self> {
        let (repo, wave): (String, Option<String>) = conn
            .query_row(
                "SELECT repo,(SELECT wave_id FROM projects WHERE id=?2)
                 FROM repository_plans WHERE id=?1",
                params![repository.as_str(), project.map(ProjectId::as_str)],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        Self::in_scope(conn, &repo, wave, repository.to_string())
    }
}

/// Whether the Task's repository has a remote planning provider. Loading
/// planning configuration takes milliseconds: read it before the write lock.
pub(super) fn planning_connected_in(conn: &Connection, task: &TaskId) -> StoreResult<bool> {
    let repo: String = conn
        .query_row(
            "SELECT w.repo FROM tasks t JOIN repository_plans w ON w.id=t.repository_id WHERE t.id=?1",
            [task.as_str()],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(StoreError::NotFound)?;
    Ok(crate::ops::planning_client::connected(&repo))
}

/// The planning connection of each open Task a Machine holds, read before the
/// write that hands them back.
pub(super) fn held_planning_in(
    conn: &Connection,
    machine: &MachineId,
) -> StoreResult<HashMap<TaskId, bool>> {
    held_in(conn, machine)?
        .into_iter()
        .map(|held| Ok((held.task.clone(), planning_connected_in(conn, &held.task)?)))
        .collect()
}

/// The one Machine of yours that writes the Task's next entry: the owner's,
/// when you own it on one of your Machines, else your Machine for its Wave.
/// Someone else's Machine is never a writer for you.
pub(super) fn writer_in(
    conn: &Connection,
    task: &TaskId,
    me: &OwnershipFacts,
) -> StoreResult<(MachineId, TaskExecutionSource)> {
    let (_, reading) = reading_in(conn, task)?;
    if let Some(holder) = &reading.owner {
        if let Some(machine) = &holder.machine.id {
            if is_mine_in(conn, me, holder)? && is_my_machine_in(conn, me, machine)? {
                return Ok((machine.clone(), TaskExecutionSource::Owner));
            }
        }
    }
    let (machine, source) = me.owning_machine.clone();
    Ok((machine, TaskExecutionSource::OwningMachine(source)))
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
    let (_, reading) = reading_in(conn, task)?;
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
    let Some((local, holder)) = my_claim_in(conn, task, me)? else {
        return Err(StoreError::InvalidAuthority(format!(
            "Task {task} is unowned"
        )));
    };
    if *target == local {
        return Err(StoreError::InvalidData(format!(
            "Task {task} is already owned on this Machine"
        )));
    }
    append_in(
        conn,
        task,
        OwnershipKind::Claim,
        holder.into(),
        target,
        me.planning_connected,
    )
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
        OwnershipKind::Unassign => format!("{who} unassigned this on {place}"),
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
    person: Person,
    machine: &MachineId,
    planning_connected: bool,
) -> StoreResult<TaskOwnershipEntry> {
    let local = local_machine_in(conn)?;
    if let Some(id) = &person.id {
        name_person_in(conn, id, person.name.as_deref())?;
    }
    let entry = TaskOwnershipEntry {
        id: OwnershipEntryId::new(),
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
    };
    let ordered_at = journal_order_in(conn, task, planning_connected)?;
    conn.execute(
        "INSERT INTO task_ownership(id,task_id,kind,person,person_key,machine_id,machine_label,
             at,ordered_at,writer_machine_id,written_here,person_id)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,1,?11)",
        params![
            entry.id.as_str(),
            task.as_str(),
            kind.as_str(),
            entry.person,
            entry.person_key,
            machine.as_str(),
            entry.machine_label,
            entry.at,
            ordered_at,
            local.as_str(),
            entry.person_id
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
    // A move keeps the person, so planning's assignee and state stay.
    if planning_connected {
        match kind {
            OwnershipKind::Claim if *machine == local => {
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
            OwnershipKind::Unassign => {
                super::planning_changes::PlanningChanges::Task(task)
                    .save(conn, &[crate::work::edit::WorkValue::TaskAssignee(None)])?;
            }
            OwnershipKind::Claim => {}
        }
    }
    Ok(entry)
}

/// Claim for this Machine when the Task is unowned or its owner's Machine is
/// unknown. Returns the new entry, or None when this Machine already owns it.
pub(super) fn claim_in(
    conn: &Connection,
    task: &TaskId,
    me: &OwnershipFacts,
) -> StoreResult<Option<TaskOwnershipEntry>> {
    let local = require_writer_in(conn, task, me)?;
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
    let key = me.person.key.clone().ok_or_else(|| {
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
        Person {
            key: Some(key),
            ..me.person.clone()
        },
        &local,
        me.planning_connected,
    )
    .map(Some)
}

/// Without a remote provider, the planning journal is the provider and an
/// entry this Machine takes in is ordered when it does: after every entry it
/// has seen, whatever their clocks said.
fn journal_order_in(conn: &Connection, task: &TaskId, remote: bool) -> StoreResult<Option<i64>> {
    if remote {
        return Ok(None);
    }
    Ok(Some(conn.query_row(
        "SELECT max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE(max(ordered_at)+1,0))
         FROM task_ownership WHERE task_id=?1",
        [task.as_str()],
        |row| row.get(0),
    )?))
}

/// The provider's order for an entry its comment carries: the comment's time.
pub(super) fn provider_order(created_at: Option<&str>) -> Option<i64> {
    created_at
        .and_then(|at| {
            time::OffsetDateTime::parse(at, &time::format_description::well_known::Rfc3339).ok()
        })
        .map(|at| (at.unix_timestamp_nanos() / 1_000_000) as i64)
}

/// Parse a marked ownership comment into the log. Repeats are ignored; the
/// provider's order is stamped once, the first time it is reported.
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
    if entry.task_id != *task || entry.machine_id.is_none() {
        return Ok(false);
    }
    let inserted = conn.execute(
        "INSERT OR IGNORE INTO task_ownership(id,task_id,kind,person,person_key,machine_id,
             machine_label,at,ordered_at,writer_machine_id,written_here,person_id)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,0,?11)",
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
            entry.person_id
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
        notify_superseded_in(conn, task, &entry, machine_names)?;
    }
    Ok(inserted)
}

/// A later claim that this Machine's open claim outlives reaches live Sessions
/// as direction. Nothing is cancelled; the claimant decides what to do.
/// `machine_names` comes from [`machine_names_in`], read before the write.
fn notify_superseded_in(
    conn: &Connection,
    task: &TaskId,
    entry: &TaskOwnershipEntry,
    machine_names: &HashMap<MachineId, String>,
) -> StoreResult<()> {
    let (_, reading) = reading_in(conn, task)?;
    let local = local_machine_in(conn)?;
    let (Some(owner), Some(later)) = (
        resolved(reading.owner, &local, machine_names),
        resolved(reading.superseded_by, &local, machine_names),
    ) else {
        return Ok(());
    };
    let text = format!(
        "Also claimed later by {later}. {owner} still owns this Task on this Machine and \
         nothing was cancelled. Keep going, or run `lf task unassign` or \
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
    planning_connected: bool,
) -> StoreResult<Option<TaskOwnershipEntry>> {
    let (_, reading) = reading_in(conn, task)?;
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

/// Move each open Task an ended Machine still owns to this Machine, in its
/// owner's name: the claim survives and the log keeps the Machine as history.
/// `planning` comes from [`held_planning_in`]. Returns `None` without writing
/// if a new claim needs planning facts; roll back and read them before retrying.
pub(super) fn hand_back_in(
    conn: &Connection,
    machine: &MachineId,
    planning: &HashMap<TaskId, bool>,
) -> StoreResult<Option<Vec<TaskId>>> {
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
    /// provider knows them; `name` is how they are shown.
    pub(crate) fn record_local_person(&self, id: &str, name: Option<&str>) -> StoreResult<()> {
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
        let me = OwnershipFacts::of_task_in(&conn, task)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        super::children::task_on(&tx, task)?.ok_or(StoreError::NotFound)?;
        let entry = claim_in(&tx, task, &me)?;
        tx.commit()?;
        Ok(entry)
    }

    /// Move your claim to `target`; the Task's primary Session here is
    /// completed, and the target admits its own on first connect.
    pub fn move_claim(&self, task: &TaskId, target: &MachineId) -> StoreResult<TaskOwnershipEntry> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let me = OwnershipFacts::of_task_in(&conn, task)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let entry = move_in(&tx, task, target, &me)?;
        super::sessions::complete_primary_session_in(
            &tx,
            &crate::session::PrimaryScope::Task(task.clone()),
        )?;
        tx.commit()?;
        Ok(entry)
    }

    /// Begin moving the Task to `target`: claim it here when unowned, write
    /// the move's claim naming `target` and record the move as leaving, in one
    /// transaction. Written before the move stops anything, so nothing here
    /// starts the Task again; its conversation travels with it, and the move
    /// completes it once taken.
    pub fn begin_task_move(
        &self,
        task: &TaskId,
        target: &MachineId,
    ) -> StoreResult<TaskOwnershipEntry> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let me = OwnershipFacts::of_task_in(&conn, task)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if reading_in(&tx, task)?.1.owner.is_none() {
            claim_in(&tx, task, &me)?;
        }
        let entry = move_in(&tx, task, target, &me)?;
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
                crate::store::rows::now_unix(),
                entry.id.as_str()
            ],
        )?;
        tx.commit()?;
        Ok(entry)
    }

    /// Take back `sent`, the move claim this Machine wrote, when the move it
    /// began could not leave; the move is no longer leaving. Nothing may follow
    /// the claim in the log: until it is received, its target has not taken
    /// the Task. The log keeps both entries.
    pub(crate) fn hand_back(
        &self,
        task: &TaskId,
        sent: &TaskOwnershipEntry,
    ) -> StoreResult<TaskOwnershipEntry> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let planning_connected = planning_connected_in(&conn, task)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (rows, _) = reading_in(&tx, task)?;
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
            planning_connected,
        )?;
        tx.execute(
            "DELETE FROM task_moves WHERE task_id=?1 AND state='leaving'",
            [task.as_str()],
        )?;
        tx.commit()?;
        Ok(entry)
    }

    /// Take in an entry another Machine wrote, before planning delivers its
    /// comment. In the planning journal it follows every entry seen here, so
    /// the claim that arrives with a move outranks this Machine's earlier ones.
    pub(crate) fn ingest_ownership_marker(&self, task: &TaskId, marker: &str) -> StoreResult<bool> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let planning_connected = planning_connected_in(&conn, task)?;
        let machine_names = machine_names_in(&conn)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let order = journal_order_in(&tx, task, planning_connected)?;
        let inserted = ingest_marker_in(&tx, task, marker, order, &machine_names)?;
        tx.commit()?;
        Ok(inserted)
    }

    /// End your claim. An unowned Task is already unclaimed and writes nothing.
    pub fn unassign_task(&self, task: &TaskId) -> StoreResult<Option<TaskOwnershipEntry>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let me = OwnershipFacts::of_task_in(&conn, task)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let Some((local, holder)) = my_claim_in(&tx, task, &me)? else {
            return Ok(None);
        };
        let person = Person {
            name: me.person.name,
            key: me.person.key.or(holder.person_key),
            id: me.person.id.or(holder.person_id),
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
        Ok(Some(entry))
    }

    /// Queue the claiming person, or nobody, as Linear assignee unless Linear already shows it.
    pub(crate) fn assign_task_owner(
        &self,
        task: &TaskId,
        assignee: Option<&str>,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let current: Option<String> = tx.query_row(
            "SELECT planning_assignee FROM tasks WHERE id=?1",
            [task.as_str()],
            |row| row.get(0),
        )?;
        if current.as_deref() != assignee {
            super::planning_changes::PlanningChanges::Task(task).save(
                &tx,
                &[crate::work::edit::WorkValue::TaskAssignee(
                    assignee.map(str::to_owned),
                )],
            )?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{local_label, log_in, read_log, Reading};
    use crate::durable::{
        MachineId, MachineRef, OwnershipEntryId, OwnershipKind, TaskId, TaskOwner,
        TaskOwnershipEntry,
    };
    use crate::store::sqlite::task_moves::MoveStage;
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
        // Linear stamps provider order later; until then a claim and the move
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
        use crate::store::migrations::{apply_before_migration, migration_section_sql};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "task_machine_local");
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
        use crate::store::migrations::{apply_before_migration, migration_section_sql};

        let conn = rusqlite::Connection::open_in_memory().unwrap();
        apply_before_migration(&conn, "task_machine_local");
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

    #[test]
    fn a_claim_names_its_person_once_and_the_latest_name_shows() {
        let (_dir, store, task) = store_with_unowned_task();
        store
            .record_local_person("linear:lee", Some("Lee"))
            .unwrap();
        let claim = store.claim_task(&task).unwrap().unwrap();
        assert_eq!(claim.person_id.as_deref(), Some("linear:lee"));
        store
            .record_local_person("linear:lee", Some("Lee Park"))
            .unwrap();
        let ownership = store.task_ownership(&task).unwrap();
        assert_eq!(ownership.log[0].person.as_deref(), Some("Lee Park"));
        assert_eq!(
            ownership.owner.unwrap().person_id.as_deref(),
            Some("linear:lee")
        );
    }

    #[test]
    fn unassigning_an_unowned_task_writes_nothing() {
        let (_dir, store, task) = store_with_unowned_task();
        assert_eq!(store.unassign_task(&task).unwrap(), None);
        assert!(store.task_ownership(&task).unwrap().log.is_empty());
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
    fn a_move_does_not_publish_the_targets_local_label() {
        let (_dir, store, task) = store_with_unowned_task();
        store.claim_task(&task).unwrap().unwrap();
        let mini = MachineId::new();
        store
            .add_machine(&mini, "ssh://mini.example", "kims-basement-mini")
            .unwrap();
        let entry = store.move_claim(&task, &mini).unwrap();
        assert_eq!(entry.machine_label, None);
        let bodies = published_bodies(&store, &task);
        assert!(bodies.iter().any(|body| body.contains(mini.as_str())));
        assert!(bodies
            .iter()
            .all(|body| !body.contains("kims-basement-mini")));
    }

    #[test]
    fn a_move_leaves_from_its_claim_until_it_is_sent_or_handed_back() {
        let (_dir, store, task) = store_with_unowned_task();
        let (laptop, mini) = (store.local_machine().unwrap().id, MachineId::new());
        store
            .add_machine(&mini, "ssh://mini.example", "mini")
            .unwrap();

        // An unowned Task is claimed here, then its claim moves to mini.
        let claim = store.begin_task_move(&task, &mini).unwrap();
        let ownership = store.task_ownership(&task).unwrap();
        assert_eq!(ownership.log.len(), 2);
        assert_eq!(ownership.owner.unwrap().machine.id, Some(mini.clone()));
        store
            .record_move_stopped(&task, "{\"step\":\"implement\"}")
            .unwrap();
        let leaving = store.task_move_in_flight(&task).unwrap().unwrap();
        assert_eq!(leaving.to_machine_id, mini);
        assert_eq!(
            leaving.stage,
            MoveStage::Leaving {
                claim_id: claim.id.clone(),
                stopped: Some("{\"step\":\"implement\"}".into())
            }
        );

        // Handed back, nothing is leaving and the laptop owns it again.
        store.hand_back(&task, &claim).unwrap();
        assert_eq!(store.task_move_in_flight(&task).unwrap(), None);
        let owner = store.task_ownership(&task).unwrap().owner.unwrap();
        assert_eq!(owner.machine.id, Some(laptop.clone()));

        // Sent, the move is no longer leaving.
        store.begin_task_move(&task, &mini).unwrap();
        store.record_move_sent(&task, "{}").unwrap();
        let sent = store.task_move_in_flight(&task).unwrap().unwrap();
        assert!(matches!(sent.stage, MoveStage::Sent { .. }));

        // Claimed back here before it arrives, the move is over; its row stays.
        store.claim_task(&task).unwrap().unwrap();
        assert_eq!(store.task_move_in_flight(&task).unwrap(), None);
        let moves: i64 = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT count(*) FROM task_moves WHERE task_id=?1 AND state='sent'",
                [task.as_str()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(moves, 1);
    }
}
