//! A Task's append-only ownership log and this Machine's reading of it.
//!
//! The later entry wins, in provider order. Exactly one Machine may append the
//! next entry: the owner's Machine, or the configured owning Machine while the
//! Task is unowned or its owner's Machine is unknown. An inbound entry never
//! ends this Machine's own open claim; only its own release or handoff does.
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use crate::durable::{
    MachineId, MachineRef, OwnershipEntryId, OwnershipKind, OwningMachineSource,
    TaskExecutionSource, TaskId, TaskOwner, TaskOwnership, TaskOwnershipEntry,
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
    let mut statement = conn.prepare(
        "SELECT id,kind,person,machine_id,at,written_here FROM task_ownership WHERE task_id=?1
         ORDER BY ordered_at IS NULL,ordered_at,at,id",
    )?;
    let rows = statement.query_map([task.as_str()], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, bool>(5)?,
        ))
    })?;
    rows.map(|row| {
        let (id, kind, person, machine, at, written_here) = row?;
        Ok(Row {
            entry: TaskOwnershipEntry {
                id: OwnershipEntryId::from_raw(id),
                task_id: task.clone(),
                kind: read_kind(&kind)?,
                person,
                machine_id: machine.as_deref().map(MachineId::from_raw),
                at,
            },
            written_here,
        })
    })
    .collect()
}

type Holder = (Option<String>, Option<MachineId>);

fn holder_after(entry: &TaskOwnershipEntry) -> Option<Holder> {
    match entry.kind {
        OwnershipKind::Release => None,
        _ => Some((entry.person.clone(), entry.machine_id.clone())),
    }
}

/// This Machine's reading of the ordered log.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Reading {
    owner: Option<Holder>,
    superseded_by: Option<Holder>,
    /// This Machine holds an open claim that the provider order no longer shows,
    /// for a reason other than a later claim.
    reassert: bool,
}

fn read_log(rows: &[Row], local: &MachineId) -> Reading {
    let global = rows.iter().fold(None, |_, row| holder_after(&row.entry));
    let mut local_claim = None;
    for (index, row) in rows.iter().enumerate() {
        let holder = holder_after(&row.entry);
        if holder.as_ref().and_then(|(_, machine)| machine.as_ref()) == Some(local) {
            local_claim = Some(index);
        } else if row.written_here
            && matches!(
                row.entry.kind,
                OwnershipKind::Release | OwnershipKind::Handoff
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
    let global_is_local = global.as_ref().and_then(|(_, machine)| machine.as_ref()) == Some(local);
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

fn local_machine_in(conn: &Connection) -> StoreResult<MachineId> {
    Ok(super::durable::map_local_machine(conn)?.id)
}

fn reading_in(conn: &Connection, task: &TaskId) -> StoreResult<(Vec<Row>, Reading)> {
    let rows = log_in(conn, task)?;
    let reading = read_log(&rows, &local_machine_in(conn)?);
    Ok((rows, reading))
}

/// The local Machine has no registered label; its short hostname names it.
fn local_label() -> String {
    let host = gethostname::gethostname().to_string_lossy().into_owned();
    host.split('.').next().unwrap_or(&host).to_string()
}

fn machine_ref_in(conn: &Connection, machine: Option<&MachineId>) -> StoreResult<MachineRef> {
    let Some(machine) = machine else {
        return Ok(MachineRef {
            id: None,
            label: None,
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
    };
    Ok(MachineRef {
        id: Some(machine.clone()),
        label,
    })
}

fn owner_in(conn: &Connection, holder: Option<Holder>) -> StoreResult<Option<TaskOwner>> {
    holder
        .map(|(person, machine)| {
            Ok(TaskOwner {
                person,
                machine: machine_ref_in(conn, machine.as_ref())?,
            })
        })
        .transpose()
}

pub(super) fn ownership_in(conn: &Connection, task: &TaskId) -> StoreResult<TaskOwnership> {
    let (rows, reading) = reading_in(conn, task)?;
    Ok(TaskOwnership {
        owner: owner_in(conn, reading.owner)?,
        superseded_by: owner_in(conn, reading.superseded_by)?,
        log: rows.into_iter().map(|row| row.entry).collect(),
    })
}

pub(super) fn owned_in(conn: &Connection, task: &TaskId) -> StoreResult<bool> {
    Ok(reading_in(conn, task)?.1.owner.is_some())
}

/// The configured owning Machine: the Task's Wave or a parent Wave's entry,
/// then `machines.default`, then this Machine.
pub(super) fn owning_machine_in(
    conn: &Connection,
    task: &TaskId,
) -> StoreResult<(MachineId, OwningMachineSource)> {
    let (repo, wave): (String, Option<String>) = conn.query_row(
        "SELECT w.repo,p.wave_id FROM tasks t JOIN repository_plans w ON w.id=t.repository_id
         LEFT JOIN projects p ON p.id=t.project_id WHERE t.id=?1",
        [task.as_str()],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    resolve_owning_machine(conn, &repo, wave)
}

fn resolve_owning_machine(
    conn: &Connection,
    repo: &str,
    mut wave: Option<String>,
) -> StoreResult<(MachineId, OwningMachineSource)> {
    let machines = crate::config::load_repo_config(std::path::Path::new(repo))
        .map_err(|error| StoreError::InvalidData(error.to_string()))?
        .and_then(|config| config.machines)
        .unwrap_or_default();
    let mut visited = std::collections::HashSet::new();
    while let Some(id) = wave.filter(|id| visited.insert(id.clone())) {
        let (name, parent): (String, Option<String>) = conn.query_row(
            "SELECT name,parent_wave_id FROM waves WHERE id=?1",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if let Some(machine) = machines.waves.get(&name) {
            return Ok((machine.clone(), OwningMachineSource::Wave(name)));
        }
        wave = parent;
    }
    match machines.default {
        Some(machine) => Ok((machine, OwningMachineSource::Default)),
        None => Ok((local_machine_in(conn)?, OwningMachineSource::ThisMachine)),
    }
}

/// The one Machine that may append the Task's next entry.
pub(super) fn writer_in(
    conn: &Connection,
    task: &TaskId,
) -> StoreResult<(MachineId, TaskExecutionSource)> {
    let (_, reading) = reading_in(conn, task)?;
    if let Some((_, Some(machine))) = reading.owner {
        return Ok((machine, TaskExecutionSource::Owner));
    }
    let (machine, source) = owning_machine_in(conn, task)?;
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

fn person() -> StoreResult<Option<String>> {
    crate::config::participant_name().map_err(|error| StoreError::InvalidData(error.to_string()))
}

fn describe_in(conn: &Connection, entry: &TaskOwnershipEntry) -> StoreResult<String> {
    let who = entry.person.as_deref().unwrap_or("Someone");
    let machine = machine_ref_in(conn, entry.machine_id.as_ref())?;
    let place = machine
        .label
        .or(machine.id.map(|id| id.to_string()))
        .unwrap_or_else(|| "an unknown Machine".into());
    Ok(match entry.kind {
        OwnershipKind::Claim => format!("{who} claimed this on {place}"),
        OwnershipKind::Handoff => format!("{who} handed this off to {place}"),
        OwnershipKind::Release => format!("{who} released this on {place}"),
        OwnershipKind::Recorded => format!("{who} was recorded on {place}"),
    })
}

fn marker(entry: &TaskOwnershipEntry) -> StoreResult<String> {
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
    machine: &MachineId,
) -> StoreResult<TaskOwnershipEntry> {
    let entry = TaskOwnershipEntry {
        id: OwnershipEntryId::new(),
        task_id: task.clone(),
        kind,
        person,
        machine_id: Some(machine.clone()),
        at: time::OffsetDateTime::now_utc().unix_timestamp(),
    };
    let repo: String = conn.query_row(
        "SELECT w.repo FROM tasks t JOIN repository_plans w ON w.id=t.repository_id WHERE t.id=?1",
        [task.as_str()],
        |row| row.get(0),
    )?;
    let linear = crate::ops::linear_observe::connected(&repo);
    // Without Linear, the planning journal is the provider and this write is its
    // order: after every entry this Machine has seen, whatever their clocks said.
    let ordered_at: Option<i64> = if linear {
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
        "INSERT INTO task_ownership(id,task_id,kind,person,machine_id,at,ordered_at,written_here)
         VALUES(?1,?2,?3,?4,?5,?6,?7,1)",
        params![
            entry.id.as_str(),
            task.as_str(),
            kind.as_str(),
            entry.person,
            machine.as_str(),
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
            body: format!("{}\n\n{}", describe_in(conn, &entry)?, marker(&entry)?),
            author: TaskCommentAuthor::Integration,
            created_at: Some(created_at),
        },
    )?;
    if linear {
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
            OwnershipKind::Release => {
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
    match reading.owner {
        Some((_, Some(machine))) if machine == local && reading.superseded_by.is_none() => Ok(None),
        _ => append_in(conn, task, OwnershipKind::Claim, person()?, &local).map(Some),
    }
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
        "INSERT OR IGNORE INTO task_ownership(id,task_id,kind,person,machine_id,at,ordered_at,written_here)
         VALUES(?1,?2,?3,?4,?5,?6,?7,0)",
        params![
            entry.id.as_str(),
            task.as_str(),
            entry.kind.as_str(),
            entry.person,
            entry.machine_id.as_ref().map(MachineId::as_str),
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
         nothing was cancelled. Keep going, or run `lf task release` or \
         `lf task handoff` to end this claim."
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
    if !reading.reassert {
        return Ok(None);
    }
    let person = reading.owner.and_then(|(person, _)| person);
    append_in(
        conn,
        task,
        OwnershipKind::Claim,
        person,
        &local_machine_in(conn)?,
    )
    .map(Some)
}

impl SqliteStore {
    /// The Machine that owns a Wave's unowned Tasks, from repository configuration.
    pub fn wave_owning_machine(
        &self,
        wave: &crate::id::WaveId,
    ) -> StoreResult<(MachineId, OwningMachineSource)> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let repo: String = snapshot.query_row(
            "SELECT repo FROM waves WHERE id=?1",
            [wave.as_str()],
            |row| row.get(0),
        )?;
        resolve_owning_machine(&snapshot, &repo, Some(wave.to_string()))
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
            && reading.owner.and_then(|(_, machine)| machine) == Some(local))
    }

    pub fn claim_task(&self, task: &TaskId) -> StoreResult<Option<TaskOwnershipEntry>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        super::children::task_on(&tx, task)?.ok_or(StoreError::NotFound)?;
        let entry = claim_in(&tx, task)?;
        tx.commit()?;
        Ok(entry)
    }

    pub fn handoff_task(
        &self,
        task: &TaskId,
        target: &MachineId,
    ) -> StoreResult<TaskOwnershipEntry> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let local = require_writer_in(&tx, task)?;
        let (_, reading) = reading_in(&tx, task)?;
        let Some((person, _)) = reading.owner else {
            return Err(StoreError::InvalidAuthority(format!(
                "Task {task} is unowned; claim it before handing it off"
            )));
        };
        if *target == local {
            return Err(StoreError::InvalidData(format!(
                "Task {task} is already owned on this Machine"
            )));
        }
        let entry = append_in(&tx, task, OwnershipKind::Handoff, person, target)?;
        tx.commit()?;
        Ok(entry)
    }

    pub fn release_task(&self, task: &TaskId) -> StoreResult<TaskOwnershipEntry> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let local = require_writer_in(&tx, task)?;
        if reading_in(&tx, task)?.1.owner.is_none() {
            return Err(StoreError::InvalidAuthority(format!(
                "Task {task} is already unowned"
            )));
        }
        let entry = append_in(&tx, task, OwnershipKind::Release, person()?, &local)?;
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
    use super::{read_log, Reading, Row};
    use crate::durable::{MachineId, OwnershipEntryId, OwnershipKind, TaskId, TaskOwnershipEntry};

    fn row(kind: OwnershipKind, person: &str, machine: &MachineId, here: bool) -> Row {
        Row {
            entry: TaskOwnershipEntry {
                id: OwnershipEntryId::new(),
                task_id: TaskId::new(),
                kind,
                person: Some(person.into()),
                machine_id: Some(machine.clone()),
                at: 1,
            },
            written_here: here,
        }
    }

    fn holder(person: &str, machine: &MachineId) -> Option<(Option<String>, Option<MachineId>)> {
        Some((Some(person.into()), Some(machine.clone())))
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
    fn only_the_local_release_or_handoff_ends_the_local_claim() {
        let (laptop, mini) = (MachineId::new(), MachineId::new());
        let mut rows = vec![
            row(OwnershipKind::Claim, "Jack", &laptop, true),
            row(OwnershipKind::Release, "Kim", &mini, false),
        ];
        let reading = read_log(&rows, &laptop);
        assert_eq!(reading.owner, holder("Jack", &laptop));
        assert!(reading.reassert);
        rows.push(row(OwnershipKind::Handoff, "Jack", &mini, true));
        assert_eq!(read_log(&rows, &laptop).owner, holder("Jack", &mini));
        rows.push(row(OwnershipKind::Release, "Jack", &mini, false));
        assert_eq!(read_log(&rows, &laptop).owner, None);
    }

    #[test]
    fn a_recorded_entry_with_an_unknown_machine_is_owned_without_a_machine() {
        let laptop = MachineId::new();
        let mut recorded = row(OwnershipKind::Recorded, "", &laptop, true);
        recorded.entry.person = None;
        recorded.entry.machine_id = None;
        assert_eq!(read_log(&[recorded], &laptop).owner, Some((None, None)));
    }
}
