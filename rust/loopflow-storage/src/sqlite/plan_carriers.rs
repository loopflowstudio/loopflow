//! How the plan of a repository without Linear or Asana travels between
//! Machines. Every Machine holds the whole plan and records which record
//! changed last (`plan_changes`). A carrier (a hub, a Git remote) sends the
//! stamped records past its cursor (`plan_carriers.sent_seq`) and hands
//! arriving ones to the one import, where the newest stamp wins.
use std::collections::{BTreeSet, HashMap};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use super::planning_write;
use super::SqliteStore;
use crate::rows::now_unix;
use crate::work::edit::{StampedRecord, WorkEdit, WorkValue};
use crate::{StoreError, StoreResult};
use loopflow_model::durable::{MachineId, TaskId};

/// The person's names for their Machines, which an ownership steer shows.
pub type MachineNames = HashMap<MachineId, String>;

/// A way a repository's plan travels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Carrier {
    /// This Machine's ref on the repository's remote.
    Git,
    /// The Machine `lf machine hub` names.
    Hub,
}

impl Carrier {
    fn as_str(self) -> &'static str {
        match self {
            Self::Git => "git",
            Self::Hub => "hub",
        }
    }
}

/// Where one carrier stands for one repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanCarrier {
    /// The remote's name, or the hub's Machine id.
    pub peer: String,
    /// How far into this Machine's `plan_changes` the carrier has taken.
    pub sent_seq: i64,
    /// Whether this Machine has read the peer: the hub's own change watermark
    /// read to, or `0` for Git once a fetch was read (Git keeps per-Machine
    /// tips in `plan_refs`); `None` before the first read.
    pub seen_seq: Option<i64>,
    pub error: Option<String>,
    /// When the carrier stopped working, while it still doesn't.
    pub failing_since: Option<i64>,
    /// What its last read brought that could not be written here; `None`
    /// when it took everything.
    pub not_taken: Option<NotTaken>,
}

impl PlanCarrier {
    /// Whether this Machine has read this carrier's peer at least once.
    pub fn has_read(&self) -> bool {
        self.seen_seq.is_some()
    }
}

/// A record a carrier brought that this Machine could not write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub id: String,
    /// Why, and what to do about it when something can be done.
    pub reason: String,
}

/// What a carrier's read brought that this Machine could not write. As an
/// error it is the exchange's, never the carrier's own: the carrier worked.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("{count} not taken: {reason}")]
pub struct NotTaken {
    /// How many records.
    pub count: usize,
    /// Why the first was not, and what to do about it when something can be
    /// done.
    pub reason: String,
}

impl NotTaken {
    /// `None` when nothing was skipped. A record two refs brought counts once.
    pub fn of(skipped: &[Skipped]) -> Option<Self> {
        let first = skipped.first()?;
        let records = skipped
            .iter()
            .map(|record| record.id.as_str())
            .collect::<BTreeSet<_>>();
        Some(Self {
            count: records.len(),
            reason: first.reason.clone(),
        })
    }
}

fn set_not_taken(
    conn: &Connection,
    repo: &str,
    carrier: Carrier,
    skipped: &[Skipped],
) -> StoreResult<()> {
    let not_taken = NotTaken::of(skipped);
    conn.execute(
        "UPDATE plan_carriers SET not_taken=?3,not_taken_reason=?4 WHERE repo=?1 AND carrier=?2",
        params![
            repo,
            carrier.as_str(),
            not_taken.as_ref().map(|not_taken| not_taken.count as i64),
            not_taken.map(|not_taken| not_taken.reason),
        ],
    )?;
    Ok(())
}

/// Records changed after a cursor, each whole and stamped.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanPage {
    pub records: Vec<StampedRecord>,
    /// The cursor after this page.
    pub seq: i64,
    /// More changed records follow `seq`.
    pub more: bool,
}

/// What a carrier has yet to take.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unsent {
    pub count: usize,
    /// The first of them in change order, as (table, id).
    pub first: Vec<(String, String)>,
}

fn owner_repo(conn: &Connection, table: &str, id: &str) -> StoreResult<Option<String>> {
    let sql = match table {
        "waves" => "SELECT repo FROM waves WHERE id=?1",
        "projects" => "SELECT w.repo FROM projects p JOIN waves w ON w.id=p.wave_id WHERE p.id=?1",
        "tasks" => "SELECT r.repo FROM tasks t JOIN repository_plans r ON r.id=t.repository_id WHERE t.id=?1",
        "task_comments" => "SELECT r.repo FROM task_comments c JOIN tasks t ON t.id=c.task_id
             JOIN repository_plans r ON r.id=t.repository_id WHERE c.id=?1",
        _ => return Err(StoreError::InvalidData("unknown Work table".into())),
    };
    Ok(conn
        .prepare_cached(sql)?
        .query_row([id], |row| row.get(0))
        .optional()?)
}

/// `table`/`id` is now the record of its repository that changed last, by a
/// save here or an arriving stamp that won.
pub(crate) fn changed(conn: &Connection, table: &str, id: &str) -> StoreResult<()> {
    let Some(repo) = owner_repo(conn, table, id)? else {
        return Ok(());
    };
    conn.prepare_cached(
        "INSERT OR REPLACE INTO plan_changes(repo,owner_table,owner_id) VALUES(?1,?2,?3)",
    )?
    .execute(params![repo, table, id])?;
    Ok(())
}

fn carrier_in(conn: &Connection, repo: &str, carrier: Carrier) -> StoreResult<Option<PlanCarrier>> {
    Ok(conn
        .query_row(
            "SELECT peer,sent_seq,seen_seq,error,failing_since,not_taken,not_taken_reason
             FROM plan_carriers
             WHERE repo=?1 AND carrier=?2",
            params![repo, carrier.as_str()],
            |row| {
                Ok(PlanCarrier {
                    peer: row.get(0)?,
                    sent_seq: row.get(1)?,
                    seen_seq: row.get(2)?,
                    error: row.get(3)?,
                    failing_since: row.get(4)?,
                    not_taken: row.get::<_, Option<i64>>(5)?.zip(row.get(6)?).map(
                        |(count, reason)| NotTaken {
                            count: count as usize,
                            reason,
                        },
                    ),
                })
            },
        )
        .optional()?)
}

fn unsent_count(conn: &Connection, repo: &str, sent_seq: i64) -> StoreResult<usize> {
    Ok(conn
        .prepare_cached("SELECT COUNT(*) FROM plan_changes WHERE repo=?1 AND seq>?2")?
        .query_row(params![repo, sent_seq], |row| row.get::<_, i64>(0))? as usize)
}

/// What a record cannot be written without: a comment's Task, a Task's
/// Project and its parent in the same repository, a Project's Wave, a Wave's parent.
fn needs(conn: &Connection, table: &str, id: &str) -> StoreResult<Vec<(String, String)>> {
    let sql = match table {
        "waves" => "SELECT 'waves',parent_wave_id FROM waves WHERE id=?1",
        "projects" => "SELECT 'waves',wave_id FROM projects WHERE id=?1",
        "tasks" => {
            "SELECT 'projects',project_id FROM tasks WHERE id=?1
             UNION ALL SELECT 'tasks',p.id FROM tasks t JOIN tasks p ON p.id=t.parent_task_id
                 AND p.repository_id=t.repository_id WHERE t.id=?1"
        }
        _ => "SELECT 'tasks',task_id FROM task_comments WHERE id=?1",
    };
    let mut query = conn.prepare_cached(sql)?;
    let mut needed = Vec::new();
    for row in query.query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))? {
        let (table, id): (String, Option<String>) = row?;
        needed.extend(id.map(|id| (table, id)));
    }
    Ok(needed)
}

/// Add `table`/`id` to `page` after everything it needs, each record once,
/// so a page can be applied on its own. Returns the bytes added.
fn push_record(
    conn: &Connection,
    page: &mut Vec<StampedRecord>,
    held: &mut BTreeSet<String>,
    table: &str,
    id: &str,
) -> StoreResult<usize> {
    if !held.insert(id.to_owned()) {
        return Ok(0);
    }
    let mut bytes = 0;
    for (table, id) in needs(conn, table, id)? {
        bytes += push_record(conn, page, held, &table, &id)?;
    }
    // A record removed since its change has nothing to send.
    let record = match planning_write::stamped(conn, table, id) {
        Err(StoreError::NotFound) => return Ok(bytes),
        record => record?,
    };
    bytes += serde_json::to_vec(&record)?.len();
    page.push(record);
    Ok(bytes)
}

impl SqliteStore {
    // ---- A carrier's cursor ----

    pub fn plan_carrier(&self, repo: &str, carrier: Carrier) -> StoreResult<Option<PlanCarrier>> {
        carrier_in(
            &self.conn.lock().expect("store mutex poisoned"),
            repo,
            carrier,
        )
    }

    /// Carry `repo`'s plan to `peer` from now on. A peer new to this
    /// repository is sent the whole plan and read from its start.
    pub fn select_carrier(
        &self,
        repo: &str,
        carrier: Carrier,
        peer: &str,
    ) -> StoreResult<PlanCarrier> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if let Some(held) = carrier_in(&conn, repo, carrier)?.filter(|held| held.peer == peer) {
            return Ok(held);
        }
        conn.execute(
            "INSERT INTO plan_carriers(repo,carrier,peer,sent_seq) VALUES(?1,?2,?3,0)
             ON CONFLICT(repo,carrier) DO UPDATE SET peer=excluded.peer,sent_seq=0,seen_seq=NULL,
                error=NULL,failing_since=NULL,not_taken=NULL,not_taken_reason=NULL",
            params![repo, carrier.as_str(), peer],
        )?;
        carrier_in(&conn, repo, carrier)?.ok_or(StoreError::NotFound)
    }

    /// Stop carrying every repository's plan by `carrier`. The plan stays.
    pub fn forget_carrier(&self, carrier: Carrier) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "DELETE FROM plan_carriers WHERE carrier=?1",
            [carrier.as_str()],
        )?;
        Ok(())
    }

    /// What `carrier` has yet to take of `repo`, naming at most `limit`.
    pub fn plan_unsent(&self, repo: &str, carrier: Carrier, limit: usize) -> StoreResult<Unsent> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let sent_seq = carrier_in(&snapshot, repo, carrier)?.map_or(0, |held| held.sent_seq);
        let mut query = snapshot.prepare_cached(
            "SELECT owner_table,owner_id FROM plan_changes WHERE repo=?1 AND seq>?2
             ORDER BY seq LIMIT ?3",
        )?;
        let first = query
            .query_map(params![repo, sent_seq, limit as i64], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })?
            .collect::<Result<_, _>>()?;
        Ok(Unsent {
            count: unsent_count(&snapshot, repo, sent_seq)?,
            first,
        })
    }

    /// `carrier`'s peer holds every record of `repo` through `seq`.
    pub fn plan_sent(&self, repo: &str, carrier: Carrier, seq: i64) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE plan_carriers SET sent_seq=MAX(sent_seq,?3) WHERE repo=?1 AND carrier=?2",
            params![repo, carrier.as_str(), seq],
        )?;
        Ok(())
    }

    /// `carrier` has fetched and read `repo`'s plan at least once. Only
    /// writes when `seen_seq` is still `NULL`, so repeated exchanges do not
    /// advance the row's revision.
    pub fn plan_read(&self, repo: &str, carrier: Carrier) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE plan_carriers SET seen_seq=0
             WHERE repo=?1 AND carrier=?2 AND seen_seq IS NULL",
            params![repo, carrier.as_str()],
        )?;
        Ok(())
    }

    /// The carrier worked (`None`) or did not, and why.
    pub fn record_carrier_exchange(
        &self,
        repo: &str,
        carrier: Carrier,
        error: Option<&str>,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        match error {
            None => conn.execute(
                "UPDATE plan_carriers SET error=NULL,failing_since=NULL
                 WHERE repo=?1 AND carrier=?2",
                params![repo, carrier.as_str()],
            )?,
            Some(error) => conn.execute(
                "UPDATE plan_carriers SET error=?3,failing_since=COALESCE(failing_since,?4)
                 WHERE repo=?1 AND carrier=?2",
                params![repo, carrier.as_str(), error, now_unix()],
            )?,
        };
        Ok(())
    }

    // ---- Other Machines' plan refs ----

    /// The commit of each other Machine's plan ref this store last took in.
    pub fn plan_ref_tips(&self, repo: &str) -> StoreResult<HashMap<String, String>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query =
            conn.prepare_cached("SELECT machine_id,tip FROM plan_refs WHERE repo=?1")?;
        let tips = query
            .query_map([repo], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;
        Ok(tips)
    }

    /// What Git's read of the other Machines' refs could not write here;
    /// nothing, when it took them all.
    pub fn record_not_taken(
        &self,
        repo: &str,
        carrier: Carrier,
        skipped: &[Skipped],
    ) -> StoreResult<()> {
        set_not_taken(
            &self.conn.lock().expect("store mutex poisoned"),
            repo,
            carrier,
            skipped,
        )
    }

    /// Every record of `machine`'s plan ref at `tip` has been taken in.
    pub fn record_plan_ref(&self, repo: &str, machine: &str, tip: &str) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "INSERT INTO plan_refs(repo,machine_id,tip) VALUES(?1,?2,?3)
             ON CONFLICT(repo,machine_id) DO UPDATE SET tip=excluded.tip",
            params![repo, machine, tip],
        )?;
        Ok(())
    }

    // ---- Records out and in ----

    /// The records of `repo` changed after `since`, in change order, each
    /// behind the records it needs, until about `budget` bytes.
    pub fn plan_records_since(
        &self,
        repo: &str,
        since: i64,
        budget: usize,
    ) -> StoreResult<PlanPage> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let mut query = snapshot.prepare_cached(
            "SELECT seq,owner_table,owner_id FROM plan_changes WHERE repo=?1 AND seq>?2 ORDER BY seq",
        )?;
        let mut rows = query.query(params![repo, since])?;
        let mut page = PlanPage {
            records: Vec::new(),
            seq: since,
            more: false,
        };
        let mut held = BTreeSet::new();
        let mut bytes = 0;
        while let Some(row) = rows.next()? {
            if bytes >= budget {
                page.more = true;
                break;
            }
            let (seq, table, id): (i64, String, String) = (row.get(0)?, row.get(1)?, row.get(2)?);
            page.seq = seq;
            bytes += push_record(&snapshot, &mut page.records, &mut held, &table, &id)?;
        }
        Ok(page)
    }

    /// Take in records another Machine holds, in one transaction: the newest
    /// stamp of each field wins. `from` names the carrier that read them from
    /// its peer and the peer's watermark they reach; records that peer just
    /// gave are not sent back to it unless something else here is unsent.
    ///
    /// Returns the records that could not be written here. With any, the
    /// carrier in `from` keeps the watermark it held, so its next read brings
    /// them again, and its `not_taken` says so; a read that takes everything
    /// clears it.
    pub fn apply_plan_records(
        &self,
        repo: &str,
        records: &[StampedRecord],
        from: Option<(Carrier, i64)>,
        names: &MachineNames,
    ) -> StoreResult<Vec<Skipped>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let mut tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let caught_up = match from {
            Some((carrier, _)) => {
                let held = carrier_in(&tx, repo, carrier)?.ok_or(StoreError::NotFound)?;
                unsent_count(&tx, repo, held.sent_seq)? == 0
            }
            None => false,
        };
        let arriving = new_comments(&tx, records)?;
        let (written, skipped) = apply_each(&mut tx, repo, records, names)?;
        comments_arrived(&tx, &written, &arriving, names)?;
        if let Some((carrier, seen_seq)) = from {
            if skipped.is_empty() {
                tx.execute(
                    "UPDATE plan_carriers SET seen_seq=?3 WHERE repo=?1 AND carrier=?2",
                    params![repo, carrier.as_str(), seen_seq],
                )?;
            }
            set_not_taken(&tx, repo, carrier, &skipped)?;
            if caught_up {
                tx.execute(
                    "UPDATE plan_carriers SET sent_seq=
                        (SELECT COALESCE(MAX(seq),0) FROM plan_changes WHERE repo=?1)
                     WHERE repo=?1 AND carrier=?2 AND sent_seq<
                        (SELECT COALESCE(MAX(seq),0) FROM plan_changes WHERE repo=?1)",
                    params![repo, carrier.as_str()],
                )?;
            }
        }
        tx.commit()?;
        Ok(skipped)
    }
}

/// Why `record` could not be written here. A Wave whose name an active Wave
/// here holds names the Machine it came from and the command that lets both
/// arrive; anything else gives the store's reason.
fn why_not_taken(
    conn: &Connection,
    repo: &str,
    record: &StampedRecord,
    error: &StoreError,
    names: &MachineNames,
) -> StoreResult<String> {
    let named = record.fields.iter().find_map(|field| match &field.value {
        WorkValue::WaveName(name) => Some((name, &field.machine)),
        _ => None,
    });
    if let Some((name, machine)) = named {
        let held: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM waves
             WHERE repo=?1 AND name=?2 AND id<>?3 AND retired_at IS NULL)",
            params![repo, name, record.id],
            |row| row.get(0),
        )?;
        if held {
            let from = names
                .iter()
                .find(|(id, _)| id.as_str() == machine)
                .map_or(machine, |(_, name)| name);
            return Ok(format!(
                "wave {name} from {from} has the name of a Wave here; \
                 `lf wave rename {name} <new name>` on either Machine lets both arrive"
            ));
        }
    }
    let kind = match record.table.as_str() {
        "waves" => "wave",
        "projects" => "project",
        "tasks" => "task",
        _ => "comment",
    };
    Ok(format!(
        "{kind} {} could not be written here: {error}",
        record.id
    ))
}

/// Write `records` together; when one cannot be written here (a Wave whose
/// name another Wave already holds, a comment whose Task is missing), take
/// them a record at a time, parents first, skip and log what fails, so the
/// rest of the plan still arrives. Apply a Wave with its incoming Project records.
fn apply_each(
    tx: &mut rusqlite::Transaction<'_>,
    repo: &str,
    records: &[StampedRecord],
    names: &MachineNames,
) -> StoreResult<(Vec<WorkEdit>, Vec<Skipped>)> {
    let together = tx.savepoint()?;
    if let Ok(written) = planning_write::apply(&together, repo, records) {
        together.commit()?;
        return Ok((written, Vec::new()));
    }
    drop(together);
    let wave_of = |record: &StampedRecord| {
        record.fields.iter().find_map(|field| match &field.value {
            WorkValue::ProjectWave(wave) => Some(wave.clone()),
            _ => None,
        })
    };
    let arriving_waves = records
        .iter()
        .filter(|record| record.table == "waves")
        .map(|record| record.id.as_str())
        .collect::<BTreeSet<_>>();
    let mut units = Vec::<Vec<StampedRecord>>::new();
    for table in planning_write::WORK_TABLES {
        for record in records.iter().filter(|record| record.table == table) {
            match table {
                "waves" => units.push(
                    std::iter::once(record)
                        .chain(records.iter().filter(|project| {
                            project.table == "projects"
                                && wave_of(project).as_deref() == Some(record.id.as_str())
                        }))
                        .cloned()
                        .collect(),
                ),
                "projects"
                    if wave_of(record)
                        .is_some_and(|wave| arriving_waves.contains(wave.as_str())) => {}
                _ => units.push(vec![record.clone()]),
            }
        }
    }
    let (mut written, mut skipped) = (Vec::new(), Vec::new());
    for unit in units {
        let one = tx.savepoint()?;
        match planning_write::apply(&one, repo, &unit) {
            Ok(edits) => {
                one.commit()?;
                written.extend(edits);
            }
            Err(error) => {
                drop(one);
                tracing::warn!(id = %unit[0].id, %error, "skipped a plan record that cannot be written here");
                let reason = why_not_taken(tx, repo, &unit[0], &error, names)?;
                skipped.extend(unit.into_iter().map(|record| Skipped {
                    id: record.id,
                    reason: reason.clone(),
                }));
            }
        }
    }
    Ok((written, skipped))
}

/// The comments in `records` this Machine does not have yet.
fn new_comments(conn: &Connection, records: &[StampedRecord]) -> StoreResult<BTreeSet<String>> {
    let mut exists =
        conn.prepare_cached("SELECT EXISTS(SELECT 1 FROM task_comments WHERE id=?1)")?;
    let mut new = BTreeSet::new();
    for record in records {
        if record.table == "task_comments"
            && !exists.query_row([&record.id], |row| row.get::<_, bool>(0))?
        {
            new.insert(record.id.clone());
        }
    }
    Ok(new)
}

/// What a comment from another Machine carries besides its row: an
/// ownership entry, which joins the Task's log, and a person's direction,
/// which steers the Task as a comment made here would. An open claim here
/// that the arrived log no longer shows is claimed again.
fn comments_arrived(
    tx: &rusqlite::Transaction<'_>,
    edits: &[WorkEdit],
    arriving: &BTreeSet<String>,
    names: &MachineNames,
) -> StoreResult<()> {
    let comments = edits
        .iter()
        .filter(|edit| matches!(edit.change, WorkValue::CommentContent(_)))
        .map(|edit| edit.target.as_str())
        .collect::<BTreeSet<_>>();
    let mut touched = Vec::<TaskId>::new();
    for id in comments {
        let (task, body, author, created_at): (String, String, String, Option<String>) = tx
            .query_row(
                "SELECT task_id,body,author,created_at FROM task_comments WHERE id=?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )?;
        let task = TaskId::from_raw(task);
        super::task_ownership::ingest_marker_in(
            tx,
            &task,
            &body,
            super::task_ownership::provider_order(created_at.as_deref()),
            names,
        )?;
        if arriving.contains(id) {
            let comment = loopflow_model::comment::TaskComment {
                id: id.to_owned(),
                author: serde_json::from_str(&author)?,
                body,
                created_at,
            };
            super::task_comments::comment_steer_in(tx, &task, &comment)?;
        }
        if !touched.contains(&task) {
            touched.push(task);
        }
    }
    for task in &touched {
        // A repository whose plan travels by carriers has no tracker.
        super::task_ownership::reassert_in(tx, task, false)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::Carrier;
    use crate::sqlite::planning_write::{self, CLOCK_MS};
    use crate::sqlite::SqliteStore;
    use crate::work::edit::{WorkEdit, WorkValue};
    use crate::work::task::{NewTask, TaskUpdate};
    use crate::work::wave::Wave;
    use loopflow_model::durable::TaskId;
    use loopflow_model::id::WaveId;

    const REPO: &str = "/repo";

    fn machine(dir: &tempfile::TempDir, name: &str) -> SqliteStore {
        let store = crate::test_fixture::open(&dir.path().join(name)).unwrap();
        crate::test_fixture::repository(&store, REPO);
        store
    }

    /// The wall clock every later save in this test reads.
    fn at(ms: i64) {
        CLOCK_MS.with(|clock| clock.set(Some(ms)));
    }

    fn exchange(laptop: &SqliteStore, hub: &SqliteStore) {
        crate::test_fixture::exchange(laptop, REPO, hub, REPO);
    }

    fn unsent(store: &SqliteStore) -> usize {
        store.plan_unsent(REPO, Carrier::Hub, 0).unwrap().count
    }

    fn rename_wave(store: &SqliteStore, wave: &WaveId, name: &str) {
        let mut conn = store.conn.lock().unwrap();
        let tx = conn.transaction().unwrap();
        planning_write::local(
            &tx,
            &WorkEdit::for_target(wave.as_str(), &[WorkValue::WaveName(name.into())]),
        )
        .unwrap();
        tx.commit().unwrap();
    }

    fn task(store: &SqliteStore, title: &str) -> (WaveId, TaskId) {
        let wave = Wave::new(WaveId::new(), "plan".into(), REPO.into());
        crate::test_fixture::wave(store, &wave);
        let project = store.seed_project(wave.id()).unwrap().id;
        let task = TaskId::new();
        store
            .create_task(&NewTask {
                parent_task_id: None,
                repository_id: crate::test_fixture::repository(store, REPO),
                id: task.clone(),
                project_id: Some(project),
                title: title.into(),
                description: "Brief".into(),
                due_date: None,
            })
            .unwrap();
        (wave.id().clone(), task)
    }

    fn retitle(store: &SqliteStore, task: &TaskId, title: &str) {
        let revision = store.task(task).unwrap().unwrap().revision;
        store
            .edit_task(
                task,
                revision,
                &TaskUpdate {
                    title: Some(title.into()),
                    ..Default::default()
                },
            )
            .unwrap();
    }

    fn title(store: &SqliteStore, task: &TaskId) -> String {
        store.task(task).unwrap().unwrap().title
    }

    #[test]
    fn a_plan_made_on_a_laptop_reaches_the_hub_and_the_hub_s_changes_come_back() {
        let dir = tempfile::tempdir().unwrap();
        let (laptop, hub) = (machine(&dir, "laptop"), machine(&dir, "hub"));
        // Made before the laptop had a hub: the first exchange sends it whole.
        let (wave, task) = task(&laptop, "hub smoke");
        exchange(&laptop, &hub);
        assert_eq!(unsent(&laptop), 0);
        let held = hub.task(&task).unwrap().unwrap();
        assert_eq!(held.title, "hub smoke");
        assert_eq!(
            held.project_id,
            laptop.task(&task).unwrap().unwrap().project_id
        );

        rename_wave(&hub, &wave, "payments");
        retitle(&laptop, &task, "two");
        assert_eq!(unsent(&laptop), 1);
        exchange(&laptop, &hub);
        assert_eq!(laptop.get_wave(&wave).unwrap().unwrap().name(), "payments");
        assert_eq!(title(&hub, &task), "two");
        // What the hub gave is not owed back to it.
        assert_eq!(unsent(&laptop), 0);
    }

    #[test]
    fn a_follow_up_reaches_another_machine_under_its_parent() {
        let dir = tempfile::tempdir().unwrap();
        let (laptop, hub) = (machine(&dir, "laptop"), machine(&dir, "hub"));
        let (_, parent) = task(&laptop, "parent");
        let filed = laptop.task(&parent).unwrap().unwrap();
        let child = TaskId::new();
        laptop
            .create_task(&NewTask {
                parent_task_id: Some(parent.clone()),
                repository_id: filed.repository_id.clone(),
                id: child.clone(),
                project_id: filed.project_id.clone(),
                title: "follow-up".into(),
                description: "Brief".into(),
                due_date: None,
            })
            .unwrap();
        // Only the child changed last, and a page still holds its parent first.
        retitle(&laptop, &child, "follow-up, renamed");
        let page = laptop.plan_records_since(REPO, 0, usize::MAX).unwrap();
        let at = |id: &TaskId| {
            page.records
                .iter()
                .position(|record| record.id == id.as_str())
                .unwrap()
        };
        assert!(at(&parent) < at(&child));

        exchange(&laptop, &hub);
        let children = hub.task_children(&parent).unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].id, child);
        assert_eq!(
            hub.task(&child).unwrap().unwrap().parent_task_id,
            Some(parent)
        );
    }

    fn claim(store: &SqliteStore, task: &TaskId) -> loopflow_model::durable::MachineId {
        let facts = crate::test_fixture::ownership(store, store.task_scope(task).unwrap());
        store
            .claim_task_prepared(task, &facts)
            .unwrap()
            .unwrap()
            .unwrap()
            .machine_id
            .unwrap()
    }

    fn owner(store: &SqliteStore, task: &TaskId) -> Option<loopflow_model::durable::MachineId> {
        let (_, ownership) = store.saved_task_ownership(task).unwrap();
        ownership.owner.and_then(|owner| owner.machine.id)
    }

    fn interrupted(store: &SqliteStore, task: &TaskId) -> bool {
        store
            .task_events_after(task, 0)
            .unwrap()
            .iter()
            .any(|event| matches!(event.kind, crate::work::task::TaskEventKind::Interrupt))
    }

    #[test]
    fn of_two_claims_that_never_saw_each_other_the_first_holds_on_both_machines() {
        let dir = tempfile::tempdir().unwrap();
        let (laptop, hub) = (machine(&dir, "laptop"), machine(&dir, "hub"));
        at(1_000);
        let (_, task) = task(&laptop, "claimed twice");
        exchange(&laptop, &hub);

        at(2_000);
        let first = claim(&hub, &task);
        at(3_000);
        let second = claim(&laptop, &task);
        assert_eq!(owner(&laptop, &task), Some(second.clone()));
        exchange(&laptop, &hub);
        assert_eq!(owner(&laptop, &task), Some(first.clone()));
        assert_eq!(owner(&hub, &task), Some(first.clone()));
        // The run that lost is stopped; the one that holds the Task is not.
        assert!(interrupted(&laptop, &task));
        assert!(!interrupted(&hub, &task));
        // Both entries stay in the log, in one order.
        let log = |store: &SqliteStore| store.saved_task_ownership(&task).unwrap().1.log;
        assert_eq!(log(&laptop), log(&hub));
        assert_eq!(log(&hub).len(), 2);

        // A claim made after seeing the first takes the Task over, as before.
        at(4_000);
        assert_eq!(claim(&laptop, &task), second);
        exchange(&laptop, &hub);
        assert_eq!(owner(&laptop, &task), Some(second.clone()));
        let (_, held) = hub.saved_task_ownership(&task).unwrap();
        assert_eq!(
            held.superseded_by.and_then(|owner| owner.machine.id),
            Some(second)
        );
        assert!(!interrupted(&hub, &task));
    }

    #[test]
    fn the_lower_order_holds_though_a_slow_clock_made_its_claim_later() {
        let dir = tempfile::tempdir().unwrap();
        let (laptop, hub) = (machine(&dir, "laptop"), machine(&dir, "hub"));
        at(1_000);
        let (_, task) = task(&laptop, "claimed twice");
        exchange(&laptop, &hub);
        // The laptop's clock is behind: its claim is made later and sorts first.
        at(3_000);
        claim(&hub, &task);
        at(2_000);
        let fast = claim(&laptop, &task);
        exchange(&laptop, &hub);
        assert_eq!(owner(&laptop, &task), Some(fast.clone()));
        assert_eq!(owner(&hub, &task), Some(fast));
        assert!(interrupted(&hub, &task));
        assert!(!interrupted(&laptop, &task));
    }

    #[test]
    fn the_later_edit_wins_on_both_machines_whichever_arrives_first() {
        let dir = tempfile::tempdir().unwrap();
        let (laptop, hub) = (machine(&dir, "laptop"), machine(&dir, "hub"));
        at(1_000);
        let (_, task) = task(&laptop, "zero");
        exchange(&laptop, &hub);

        // The hub's edit is later, the laptop's reaches the hub after it.
        at(2_000);
        retitle(&laptop, &task, "offline");
        at(3_000);
        retitle(&hub, &task, "later");
        exchange(&laptop, &hub);
        assert_eq!(title(&laptop, &task), "later");
        assert_eq!(title(&hub, &task), "later");

        // The laptop's edit is later, and the hub already holds its own.
        at(4_000);
        retitle(&hub, &task, "earlier");
        at(5_000);
        retitle(&laptop, &task, "latest");
        exchange(&laptop, &hub);
        assert_eq!(title(&laptop, &task), "latest");
        assert_eq!(title(&hub, &task), "latest");
    }

    #[test]
    fn an_edit_made_after_seeing_another_beats_it_whatever_the_clock_says() {
        let dir = tempfile::tempdir().unwrap();
        let (laptop, hub) = (machine(&dir, "laptop"), machine(&dir, "hub"));
        at(1_000);
        let (_, task) = task(&laptop, "zero");
        exchange(&laptop, &hub);
        // The hub's clock runs an hour fast.
        at(3_600_000);
        retitle(&hub, &task, "fast clock");
        exchange(&laptop, &hub);
        at(2_000);
        retitle(&laptop, &task, "seen it");
        exchange(&laptop, &hub);
        assert_eq!(title(&laptop, &task), "seen it");
        assert_eq!(title(&hub, &task), "seen it");
    }

    #[test]
    fn a_stale_laptop_meeting_the_hub_for_the_first_time_takes_the_newer_value() {
        let dir = tempfile::tempdir().unwrap();
        let (laptop, hub) = (machine(&dir, "laptop"), machine(&dir, "hub"));
        at(1_000);
        let (_, task) = task(&laptop, "stale");
        // The hub got the Task some other way, then changed it.
        let page = laptop.plan_records_since(REPO, 0, usize::MAX).unwrap();
        hub.apply_plan_records(REPO, &page.records, None, &Default::default())
            .unwrap();
        at(2_000);
        retitle(&hub, &task, "newer");
        exchange(&laptop, &hub);
        assert_eq!(title(&laptop, &task), "newer");
        assert_eq!(title(&hub, &task), "newer");
    }

    #[test]
    fn an_arrival_that_changes_nothing_ends_the_exchange() {
        let dir = tempfile::tempdir().unwrap();
        let (laptop, hub) = (machine(&dir, "laptop"), machine(&dir, "hub"));
        let (_, task) = task(&laptop, "once");
        exchange(&laptop, &hub);
        exchange(&laptop, &hub);
        let (revisions, cursor) = (
            hub.revisions().unwrap(),
            hub.plan_records_since(REPO, 0, usize::MAX).unwrap().seq,
        );
        // A reply lost after the hub wrote it: the record goes again.
        let again = laptop.plan_records_since(REPO, 0, usize::MAX).unwrap();
        assert!(again
            .records
            .iter()
            .any(|record| record.id == task.as_str()));
        hub.apply_plan_records(REPO, &again.records, None, &Default::default())
            .unwrap();
        assert_eq!(hub.revisions().unwrap(), revisions);
        let after = hub.plan_records_since(REPO, cursor, usize::MAX).unwrap();
        assert!(after.records.is_empty());
        assert_eq!(unsent(&laptop), 0);
    }

    #[test]
    fn every_page_applies_on_its_own_though_a_parent_changed_after_its_child() {
        let dir = tempfile::tempdir().unwrap();
        let (laptop, hub) = (machine(&dir, "laptop"), machine(&dir, "hub"));
        let (wave, task) = task(&laptop, "child first");
        rename_wave(&laptop, &wave, "renamed later");
        let mut since = 0;
        loop {
            // One changed record a page.
            let page = laptop.plan_records_since(REPO, since, 1).unwrap();
            hub.apply_plan_records(REPO, &page.records, None, &Default::default())
                .unwrap();
            since = page.seq;
            if !page.more {
                break;
            }
        }
        assert_eq!(title(&hub, &task), "child first");
        assert_eq!(
            hub.get_wave(&wave).unwrap().unwrap().name(),
            "renamed later"
        );
    }

    #[test]
    fn a_wave_named_like_one_held_here_is_shown_not_taken_until_one_is_renamed() {
        let dir = tempfile::tempdir().unwrap();
        let (laptop, hub) = (machine(&dir, "laptop"), machine(&dir, "hub"));
        // Each Machine made a Wave called "plan" before seeing the other's.
        let (_, theirs) = task(&hub, "in the hub's plan");
        let (wave, mine) = task(&laptop, "in the laptop's plan");
        let other = crate::test_fixture::project(&laptop, REPO, "other");
        let apart = TaskId::new();
        laptop
            .create_task(&NewTask {
                repository_id: crate::test_fixture::repository(&laptop, REPO),
                id: apart.clone(),
                project_id: Some(other.id),
                title: "in another Wave".into(),
                description: "Brief".into(),
                due_date: None,
                parent_task_id: None,
            })
            .unwrap();
        let not_taken = |store: &SqliteStore| {
            let held = store.plan_carrier(REPO, Carrier::Hub).unwrap().unwrap();
            (held.not_taken, held.seen_seq)
        };
        exchange(&laptop, &hub);
        assert_eq!(title(&hub, &apart), "in another Wave");
        assert!(hub.task(&mine).unwrap().is_none());
        assert!(laptop.task(&theirs).unwrap().is_none());
        // The hub's Wave, its Project and its Task, with what to do; and the
        // hub is read from the same place next time.
        let (held, seen) = not_taken(&laptop);
        let held = held.unwrap();
        assert_eq!(held.count, 3);
        assert!(
            held.reason.starts_with("wave plan from ")
                && held.reason.contains("`lf wave rename plan <new name>`"),
            "{held}"
        );
        assert_eq!(seen, None);
        // Later changes still travel, and the line stays.
        retitle(&laptop, &apart, "still arriving");
        exchange(&laptop, &hub);
        assert_eq!(title(&hub, &apart), "still arriving");
        assert!(not_taken(&laptop).0.is_some());

        // One of the two is renamed: the next read writes what waited.
        rename_wave(&laptop, &wave, "laptop-plan");
        exchange(&laptop, &hub);
        assert_eq!(title(&laptop, &theirs), "in the hub's plan");
        let (held, seen) = not_taken(&laptop);
        assert_eq!(held, None);
        assert!(seen.is_some());
        // The hub took the laptop's Wave with its new name.
        assert_eq!(title(&hub, &mine), "in the laptop's plan");
    }

    #[test]
    fn a_record_travels_as_json_and_back() {
        let dir = tempfile::tempdir().unwrap();
        let laptop = machine(&dir, "laptop");
        at(1_000);
        task(&laptop, "wire");
        let page = laptop.plan_records_since(REPO, 0, usize::MAX).unwrap();
        let wire = serde_json::to_string(&page.records).unwrap();
        assert!(
            wire.contains(r#"{"field":"task_title","value":"wire","at":10"#),
            "{wire}"
        );
        let back: Vec<crate::work::edit::StampedRecord> = serde_json::from_str(&wire).unwrap();
        assert_eq!(back, page.records);
    }

    #[test]
    fn plan_read_marks_seen_seq_once_and_resets_when_the_peer_changes() {
        let dir = tempfile::tempdir().unwrap();
        let store = machine(&dir, "laptop");
        let initial = store.select_carrier(REPO, Carrier::Git, "origin").unwrap();
        assert!(!initial.has_read());
        assert_eq!(initial.seen_seq, None);

        store.plan_read(REPO, Carrier::Git).unwrap();
        let marked = store.plan_carrier(REPO, Carrier::Git).unwrap().unwrap();
        assert!(marked.has_read());
        assert_eq!(marked.seen_seq, Some(0));

        // A second mark on an already-read carrier is a no-op and advances no revision.
        let revisions = store.revisions().unwrap();
        store.plan_read(REPO, Carrier::Git).unwrap();
        assert_eq!(store.revisions().unwrap(), revisions);

        // Switching the peer resets the read mark so the new peer is read once.
        let switched = store
            .select_carrier(REPO, Carrier::Git, "upstream")
            .unwrap();
        assert!(!switched.has_read());
        assert_eq!(switched.seen_seq, None);
    }
}
