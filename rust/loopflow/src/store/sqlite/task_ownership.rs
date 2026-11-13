//! A Task's append-only ownership log and this Machine's reading of it.
//!
//! The later entry wins, in provider order. Writes stay within one person's
//! Machines: your Task's next entry is written on its Machine when that is one
//! of yours, otherwise on your Machine for its Wave. An inbound entry never
//! ends this Machine's own open claim; only its own unclaim or handoff does.
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
                OwnershipKind::Unassign | OwnershipKind::Handoff
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
    let id: String = conn.query_row("SELECT id FROM machines WHERE is_local=1", [], |row| {
        row.get(0)
    })?;
    Ok(MachineId::from_raw(id))
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
            let mut entry = row.entry;
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
            .find(|row| row.entry.machine_id.as_ref() == Some(machine))
            .map(|row| row.entry.at);
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

/// Whether a holder is you. A holder with a person key (Git `user.email`) is
/// you exactly when the key is yours, even on a Machine you registered. A
/// keyless entry, written before keys or recorded before ownership existed, is
/// yours when it holds one of your Machines. Names are display only.
fn is_mine_in(conn: &Connection, me: &OwnershipFacts, holder: &TaskOwner) -> StoreResult<bool> {
    if let Some(key) = &holder.person_key {
        return Ok(me.key.as_ref() == Some(key));
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
    key: Option<String>,
    name: Option<String>,
    planning_connected: bool,
    declared_machines: HashMap<MachineId, String>,
    owning_machine: (MachineId, OwningMachineSource),
}

impl OwnershipFacts {
    fn in_scope(
        conn: &Connection,
        repo: &str,
        wave: Option<String>,
        repository: String,
    ) -> StoreResult<Self> {
        Ok(Self {
            key: crate::config::participant_email(std::path::Path::new(repo)),
            name: crate::config::participant_name()
                .map_err(|error| StoreError::InvalidData(error.to_string()))?,
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
    if writer != local {
        return Err(StoreError::InvalidAuthority(format!(
            "Task {task} ownership is written on Machine {writer}; run the command there"
        )));
    }
    Ok(local)
}

/// Handoff and unclaim end your own claim; someone else's stays theirs.
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

fn handoff_in(
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
        OwnershipKind::Handoff,
        holder.person,
        holder.person_key,
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
        OwnershipKind::Handoff => format!("{who} handed this off to {place}"),
        OwnershipKind::Unassign => format!("{who} unassigned this on {place}"),
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
    planning_connected: bool,
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
    let ordered_at = journal_order_in(conn, task, planning_connected)?;
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
    if planning_connected {
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
            OwnershipKind::Unassign => {
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
    let key = me.key.clone().ok_or_else(|| {
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
        me.name.clone(),
        Some(key),
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
    if entry.task_id != *task || entry.kind == OwnershipKind::Recorded || entry.machine_id.is_none()
    {
        return Ok(false);
    }
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
        holder.person,
        holder.person_key,
        &local_machine_in(conn)?,
        planning_connected,
    )
    .map(Some)
}

/// Hand each open Task an ended Machine still owns to this Machine, in its
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
            OwnershipKind::Handoff,
            owner.person,
            owner.person_key,
            &local,
            planning_connected,
        )?;
        tasks.push(task);
    }
    Ok(Some(tasks))
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
        let me = OwnershipFacts::of_task_in(&conn, task)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        super::children::task_on(&tx, task)?.ok_or(StoreError::NotFound)?;
        let entry = claim_in(&tx, task, &me)?;
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
        let me = OwnershipFacts::of_task_in(&conn, task)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let entry = handoff_in(&tx, task, target, &me)?;
        super::sessions::complete_primary_session_in(
            &tx,
            &crate::session::PrimaryScope::Task(task.clone()),
        )?;
        tx.commit()?;
        Ok(entry)
    }

    /// A move's handoff, written before the move stops anything: the Task's
    /// conversation travels with it, and the move completes it once taken.
    pub(crate) fn handoff_moving_task(
        &self,
        task: &TaskId,
        target: &MachineId,
    ) -> StoreResult<TaskOwnershipEntry> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let me = OwnershipFacts::of_task_in(&conn, task)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let entry = handoff_in(&tx, task, target, &me)?;
        tx.commit()?;
        Ok(entry)
    }

    /// Take back `sent`, the handoff this Machine just wrote, when the move it
    /// began could not leave. Nothing may follow it in the log: until it is
    /// received, its target has not taken the Task. The log keeps both entries.
    pub(crate) fn hand_back(
        &self,
        task: &TaskId,
        sent: &TaskOwnershipEntry,
    ) -> StoreResult<TaskOwnershipEntry> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let planning_connected = planning_connected_in(&conn, task)?;
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let (rows, _) = reading_in(&tx, task)?;
        if rows.last().map(|row| &row.entry.id) != Some(&sent.id) {
            return Err(StoreError::InvalidAuthority(format!(
                "Task {task} changed hands after its handoff {}; it stays where that left it",
                sent.id
            )));
        }
        let entry = append_in(
            &tx,
            task,
            OwnershipKind::Handoff,
            sent.person.clone(),
            sent.person_key.clone(),
            &local_machine_in(&tx)?,
            planning_connected,
        )?;
        tx.commit()?;
        Ok(entry)
    }

    /// Take in an entry another Machine wrote, before planning delivers its
    /// comment. In the planning journal it follows every entry seen here, so
    /// a handoff that arrives with a move outranks this Machine's earlier ones.
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
        let key = me.key.or(holder.person_key);
        let entry = append_in(
            &tx,
            task,
            OwnershipKind::Unassign,
            me.name,
            key,
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
            row(OwnershipKind::Unassign, "Kim", &mini, false),
        ];
        let reading = read_log(&rows, &laptop);
        assert_eq!(reading.owner, holder("Jack", &laptop));
        assert!(reading.reassert);
        rows.push(row(OwnershipKind::Handoff, "Jack", &mini, true));
        assert_eq!(read_log(&rows, &laptop).owner, holder("Jack", &mini));
        rows.push(row(OwnershipKind::Unassign, "Jack", &mini, false));
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

    /// A laptop and mini sharing one Task's planning, each with its own store
    /// and checkout; `to_laptop`/`to_mini` exchange the planning journal.
    struct Pair {
        laptop: SqliteStore,
        mini: SqliteStore,
        laptop_repo: loopflow_test_support::TestRepo,
        mini_repo: loopflow_test_support::TestRepo,
        destination: crate::planning_git::PlanningDestination,
        task: TaskId,
        _dir: tempfile::TempDir,
    }

    impl Pair {
        /// mini creates the Task; the laptop has not seen it yet.
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let laptop = SqliteStore::open_ephemeral(&dir.path().join("laptop.db")).unwrap();
            let mini = SqliteStore::open_ephemeral(&dir.path().join("mini.db")).unwrap();
            let (laptop_repo, mini_repo) = (
                loopflow_test_support::TestRepo::new(),
                loopflow_test_support::TestRepo::new(),
            );
            let repository = crate::durable::RepositoryId::new();
            let destination = crate::planning_git::PlanningDestination::new(
                "/unused/fixture",
                "refs/loopflow/planning/shared/owners",
            )
            .unwrap();
            for (store, repo) in [(&laptop, &laptop_repo), (&mini, &mini_repo)] {
                let path = Self::path(repo);
                store.bind_repository(&path, &repository).unwrap();
                store.bind_peer_planning(&path, &destination).unwrap();
            }
            let path = Self::path(&mini_repo);
            let project = mini.ensure_wave_project(&path, "owners").unwrap();
            let task = mini
                .create_task(&NewTask {
                    repository_id: mini.ensure_repository(&path).unwrap(),
                    id: TaskId::new(),
                    project_id: Some(project.id),
                    title: "Ownership demo".into(),
                    description: String::new(),
                    due_date: None,
                })
                .unwrap()
                .id;
            Self {
                laptop,
                mini,
                laptop_repo,
                mini_repo,
                destination,
                task,
                _dir: dir,
            }
        }

        fn path(repo: &loopflow_test_support::TestRepo) -> String {
            repo.path()
                .canonicalize()
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned()
        }

        fn exchange(
            &self,
            from: (&SqliteStore, &loopflow_test_support::TestRepo),
            to: (&SqliteStore, &loopflow_test_support::TestRepo),
        ) {
            let journal = from
                .0
                .export_peer_planning(&Self::path(from.1), &self.destination.id())
                .unwrap();
            to.0.import_peer_planning(
                &Self::path(to.1),
                &self.destination.id(),
                &uuid::Uuid::new_v4().to_string(),
                &journal,
            )
            .unwrap();
        }

        fn to_laptop(&self) {
            self.exchange(
                (&self.mini, &self.mini_repo),
                (&self.laptop, &self.laptop_repo),
            );
        }

        fn to_mini(&self) {
            self.exchange(
                (&self.laptop, &self.laptop_repo),
                (&self.mini, &self.mini_repo),
            );
        }
    }

    #[test]
    fn a_double_claim_keeps_the_first_claimant_and_names_the_later_claim() {
        let pair = Pair::new();
        let task = &pair.task;
        pair.to_laptop();
        let mini = pair.mini.local_machine().unwrap().id;
        let laptop = pair.laptop.local_machine().unwrap().id;
        // No owning Machine is configured, so each Machine may claim for itself.
        pair.mini.claim_task(task).unwrap().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(1100));
        pair.laptop.claim_task(task).unwrap().unwrap();
        pair.to_mini();
        pair.to_laptop();
        let first = pair.mini.task_ownership(task).unwrap();
        assert_eq!(first.owner.unwrap().machine.id, Some(mini.clone()));
        assert_eq!(
            first.superseded_by.unwrap().machine.id,
            Some(laptop.clone())
        );
        assert_eq!(
            pair.mini.task_execution_route(task).unwrap().machine_id,
            mini
        );
        assert!(pair
            .mini
            .task_steers(task)
            .unwrap()
            .iter()
            .any(|steer| steer.text.starts_with("Also claimed later by")));
        let second = pair.laptop.task_ownership(task).unwrap();
        assert_eq!(second.owner.unwrap().machine.id, Some(laptop));
        assert!(second.superseded_by.is_none());
        // Exchange settles: neither Machine re-claims a log that shows a later claim.
        pair.to_mini();
        pair.to_laptop();
        assert_eq!(pair.mini.task_ownership(task).unwrap().log.len(), 2);
    }

    #[test]
    fn the_owner_reclaims_after_a_stale_unclaim_elsewhere() {
        use crate::work::task::comment::{TaskComment, TaskCommentAuthor};
        let pair = Pair::new();
        let task = &pair.task;
        pair.to_laptop();
        let mini = pair.mini.local_machine().unwrap().id;
        pair.laptop.claim_task(task).unwrap().unwrap();
        pair.to_mini();
        // A stale reading on mini treats the Task as its own and unclaims it there,
        // delivering the unclaim as mini would: a marked comment in the journal.
        let later = time::OffsetDateTime::now_utc() + time::Duration::seconds(5);
        let entry = serde_json::json!({
            "id": format!("own_{}", uuid::Uuid::new_v4().simple()),
            "task_id": task.as_str(), "kind": "unclaim", "person": "Kim",
            "machine_id": mini.as_str(),
            "at": later.unix_timestamp(),
        });
        pair.mini
            .append_task_comment(
                task,
                &TaskComment {
                    id: uuid::Uuid::new_v4().to_string(),
                    body: format!("Kim unclaimed this\n\n<!-- loopflow-ownership:{entry} -->"),
                    author: TaskCommentAuthor::Integration,
                    created_at: Some(
                        later
                            .format(&time::format_description::well_known::Rfc3339)
                            .unwrap(),
                    ),
                },
            )
            .unwrap();
        pair.to_laptop();
        // The laptop's claim stays open locally and it claims again in the journal.
        let ownership = pair.laptop.task_ownership(task).unwrap();
        assert_eq!(
            ownership.owner.unwrap().machine.id,
            Some(pair.laptop.local_machine().unwrap().id)
        );
        assert_eq!(ownership.log.last().unwrap().kind, OwnershipKind::Claim);
    }

    #[test]
    fn another_persons_claim_never_routes_work_to_their_machine() {
        use crate::durable::{OwningMachineSource, TaskExecutionSource};
        let pair = Pair::new();
        let task = &pair.task;
        let mini = pair.mini.local_machine().unwrap().id;
        let laptop = pair.laptop.local_machine().unwrap().id;
        // Kim claims on mini, a Machine this laptop also has registered. Her
        // person key, not the Machine, says whose claim it is.
        assert!(std::process::Command::new("git")
            .args(["config", "user.email", "kim@example.com"])
            .current_dir(pair.mini_repo.path())
            .status()
            .unwrap()
            .success());
        pair.mini.claim_task(task).unwrap().unwrap();
        pair.to_laptop();
        assert_eq!(
            pair.laptop
                .task_ownership(task)
                .unwrap()
                .owner
                .unwrap()
                .person_key
                .as_deref(),
            Some("kim@example.com")
        );
        // Someone else's claim stays theirs: no route to mini, no handoff or unassign.
        let route = pair.laptop.task_execution_route(task).unwrap();
        assert_eq!(
            (route.machine_id, route.source),
            (
                laptop.clone(),
                TaskExecutionSource::OwningMachine(OwningMachineSource::ThisMachine)
            )
        );
        assert!(pair.laptop.unassign_task(task).is_err());
        assert!(pair.laptop.handoff_task(task, &mini).is_err());
        // Claiming writes here; the later claim wins.
        pair.laptop.claim_task(task).unwrap().unwrap();
        pair.to_mini();
        // Kim's claim stays Kim's on mini, which now sees the later claim.
        let later = pair
            .mini
            .task_ownership(task)
            .unwrap()
            .superseded_by
            .unwrap();
        assert_eq!(later.machine.id, Some(laptop));
    }
}
