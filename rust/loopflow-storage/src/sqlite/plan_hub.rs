//! The plan of a repository without Linear or Asana, kept on one hub Machine.
//! Both ends speak `WorkEdit`s and share every id. Every Machine records which
//! owner changed last (`plan_changes`), so any of them can answer as a hub;
//! a Machine whose plan is on a hub also queues each local save (`hub_sends`)
//! and remembers how far it has read the hub (`hub_plans`).
use std::collections::{BTreeSet, HashMap};

use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

use super::planning_write::{self, WriteOrigin};
use super::SqliteStore;
use crate::rows::now_unix;
use crate::work::edit::{WorkEdit, WorkValue};
use crate::{StoreError, StoreResult};
use loopflow_model::durable::{MachineId, TaskId};

/// The person's names for their Machines, which an ownership steer shows.
pub type MachineNames = HashMap<MachineId, String>;

/// Where this Machine stands with its hub for one repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HubPlan {
    pub hub_machine_id: String,
    /// The hub's change watermark this Machine has read to; `None` before
    /// the first read.
    pub seq: Option<i64>,
    pub error: Option<String>,
    /// When the hub stopped answering, while it still doesn't.
    pub failing_since: Option<i64>,
}

/// A value saved here that the hub has not confirmed.
#[derive(Debug, Clone, PartialEq)]
pub struct HubSend {
    pub seq: i64,
    /// `waves`, `projects`, `tasks` or `task_comments`.
    pub owner_table: String,
    pub edit: WorkEdit,
    pub sent_at: Option<i64>,
}

impl HubSend {
    /// The field without its record prefix: `title`, `name`.
    pub fn field(&self) -> &'static str {
        let field = self.edit.change.field();
        ["task_", "wave_", "project_", "comment_"]
            .iter()
            .find_map(|prefix| field.strip_prefix(prefix))
            .unwrap_or(field)
    }

    /// `queued`, or `sent` while its answer is outstanding.
    pub fn phase(&self) -> &'static str {
        if self.sent_at.is_some() {
            "sent"
        } else {
            "queued"
        }
    }
}

/// Owners changed after a watermark, as every current value of each.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanPage {
    pub edits: Vec<WorkEdit>,
    /// The watermark after this page.
    pub seq: i64,
    /// More changed owners follow `seq`.
    pub more: bool,
}

// ---- The writer's side ----

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

/// The writer changed `table`/`id`: it is the newest change of its owner,
/// and `sent` (a local save's values) goes to the hub when the repository's
/// plan is there.
pub(crate) fn saved(
    conn: &Connection,
    table: &str,
    id: &str,
    sent: &[&WorkValue],
) -> StoreResult<()> {
    let Some(repo) = owner_repo(conn, table, id)? else {
        return Ok(());
    };
    conn.prepare_cached(
        "INSERT OR REPLACE INTO plan_changes(repo,owner_table,owner_id) VALUES(?1,?2,?3)",
    )?
    .execute(params![repo, table, id])?;
    if sent.is_empty() {
        return Ok(());
    }
    let on_hub: bool = conn
        .prepare_cached("SELECT EXISTS(SELECT 1 FROM hub_plans WHERE repo=?1)")?
        .query_row([&repo], |row| row.get(0))?;
    if on_hub {
        queue(conn, &repo, table, id, sent)?;
    }
    Ok(())
}

fn queue(
    conn: &Connection,
    repo: &str,
    table: &str,
    id: &str,
    values: &[&WorkValue],
) -> StoreResult<()> {
    let mut insert = conn.prepare_cached(
        "INSERT OR REPLACE INTO hub_sends(repo,owner_table,target,field,edit_json)
         VALUES(?1,?2,?3,?4,?5)",
    )?;
    for value in values {
        let edit = WorkEdit {
            target: id.into(),
            change: (*value).clone(),
        };
        insert.execute(params![
            repo,
            table,
            id,
            value.field(),
            serde_json::to_string(&edit)?
        ])?;
    }
    Ok(())
}

fn hub_plan_in(conn: &Connection, repo: &str) -> StoreResult<Option<HubPlan>> {
    Ok(conn
        .query_row(
            "SELECT hub_machine_id,seq,error,failing_since FROM hub_plans WHERE repo=?1",
            [repo],
            |row| {
                Ok(HubPlan {
                    hub_machine_id: row.get(0)?,
                    seq: row.get(1)?,
                    error: row.get(2)?,
                    failing_since: row.get(3)?,
                })
            },
        )
        .optional()?)
}

impl SqliteStore {
    // ---- A Machine whose plan is on a hub ----

    pub fn hub_plan(&self, repo: &str) -> StoreResult<Option<HubPlan>> {
        hub_plan_in(&self.conn.lock().expect("store mutex poisoned"), repo)
    }

    /// Keep `repo`'s plan on `hub` from now on. A repository new to this hub
    /// starts over at first contact: its next read is the hub's whole plan.
    pub fn select_hub(&self, repo: &str, hub: &str) -> StoreResult<HubPlan> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        if let Some(plan) = hub_plan_in(&conn, repo)?.filter(|plan| plan.hub_machine_id == hub) {
            return Ok(plan);
        }
        conn.execute(
            "INSERT INTO hub_plans(repo,hub_machine_id) VALUES(?1,?2)
             ON CONFLICT(repo) DO UPDATE SET hub_machine_id=excluded.hub_machine_id,seq=NULL,
                error=NULL,failing_since=NULL",
            params![repo, hub],
        )?;
        hub_plan_in(&conn, repo)?.ok_or(StoreError::NotFound)
    }

    /// Keep every plan on this Machine again: nothing queues for a hub.
    /// Returns how many values the hub never took.
    pub fn forget_hub_plans(&self) -> StoreResult<usize> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let unsent = tx.execute("DELETE FROM hub_sends", [])?;
        tx.execute("DELETE FROM hub_plans", [])?;
        tx.commit()?;
        Ok(unsent)
    }

    /// What `repo` has yet to send to its hub, oldest first.
    pub fn open_hub_sends(&self, repo: &str) -> StoreResult<Vec<HubSend>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT seq,owner_table,edit_json,sent_at FROM hub_sends WHERE repo=?1 ORDER BY seq",
        )?;
        let sends = query
            .query_and_then([repo], |row| -> StoreResult<HubSend> {
                Ok(HubSend {
                    seq: row.get(0)?,
                    owner_table: row.get(1)?,
                    edit: serde_json::from_str(&row.get::<_, String>(2)?)?,
                    sent_at: row.get(3)?,
                })
            })?
            .collect();
        sends
    }

    pub fn mark_hub_sent(&self, seqs: &[i64]) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut update = conn.prepare("UPDATE hub_sends SET sent_at=?2 WHERE seq=?1")?;
        let now = now_unix();
        for seq in seqs {
            update.execute(params![seq, now])?;
        }
        Ok(())
    }

    /// The hub wrote these sends. A field saved again since has a newer row,
    /// which stays open.
    pub fn confirm_hub_sends(&self, seqs: &[i64]) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        {
            let mut delete = tx.prepare("DELETE FROM hub_sends WHERE seq=?1")?;
            for seq in seqs {
                delete.execute([seq])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    /// The hub answered (`None`) or did not, and why.
    pub fn record_hub_exchange(&self, repo: &str, error: Option<&str>) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        match error {
            None => conn.execute(
                "UPDATE hub_plans SET error=NULL,failing_since=NULL WHERE repo=?1",
                [repo],
            )?,
            Some(error) => conn.execute(
                "UPDATE hub_plans SET error=?2,failing_since=COALESCE(failing_since,?3) WHERE repo=?1",
                params![repo, error, now_unix()],
            )?,
        };
        Ok(())
    }

    /// Write what the hub holds for `repo`, read through `seq`. A field with
    /// an open send keeps its local value: that value is on its way. At first
    /// contact, every owner here the hub did not return is queued whole, so
    /// two plans become one.
    pub fn import_hub_plan(
        &self,
        repo: &str,
        edits: &[WorkEdit],
        seq: i64,
        names: &MachineNames,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let plan = hub_plan_in(&tx, repo)?.ok_or(StoreError::NotFound)?;
        let open = {
            let mut query = tx.prepare("SELECT target,field FROM hub_sends WHERE repo=?1")?;
            let open = query
                .query_map([repo], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })?
                .collect::<Result<BTreeSet<_>, _>>()?;
            open
        };
        let kept = edits
            .iter()
            .filter(|edit| !open.contains(&(edit.target.clone(), edit.change.field().to_owned())))
            .cloned()
            .collect::<Vec<_>>();
        let arriving = new_comments(&tx, &kept)?;
        planning_write::create(&tx, repo, &kept, WriteOrigin::Import)?;
        comments_arrived(&tx, &kept, &arriving, names)?;
        if plan.seq.is_none() {
            let on_hub = edits
                .iter()
                .map(|edit| edit.target.as_str())
                .collect::<BTreeSet<_>>();
            let owners = {
                let mut query = tx.prepare(
                    "SELECT owner_table,owner_id FROM plan_changes WHERE repo=?1 ORDER BY seq",
                )?;
                let owners = query
                    .query_map([repo], |row| {
                        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                owners
            };
            for (table, id) in owners {
                if on_hub.contains(id.as_str()) {
                    continue;
                }
                let values = match planning_write::read(&tx, &table, &id) {
                    Err(StoreError::NotFound) => continue,
                    values => values?,
                };
                queue(&tx, repo, &table, &id, &values.iter().collect::<Vec<_>>())?;
            }
        }
        tx.execute(
            "UPDATE hub_plans SET seq=?2 WHERE repo=?1",
            params![repo, seq],
        )?;
        tx.commit()?;
        Ok(())
    }

    // ---- The hub ----

    /// Every current value of each owner in `repo` changed after `since`
    /// (all of them for `None`), in change order, until about `budget` bytes.
    pub fn plan_changes_since(
        &self,
        repo: &str,
        since: Option<i64>,
        budget: usize,
    ) -> StoreResult<PlanPage> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let since = since.unwrap_or(0);
        let mut query = conn.prepare(
            "SELECT seq,owner_table,owner_id FROM plan_changes WHERE repo=?1 AND seq>?2 ORDER BY seq",
        )?;
        let mut rows = query.query(params![repo, since])?;
        let mut page = PlanPage {
            edits: Vec::new(),
            seq: since,
            more: false,
        };
        let mut bytes = 0;
        while let Some(row) = rows.next()? {
            if bytes >= budget {
                page.more = true;
                break;
            }
            let (seq, table, id): (i64, String, String) = (row.get(0)?, row.get(1)?, row.get(2)?);
            page.seq = seq;
            // An owner removed since its change has nothing to send.
            let values = match planning_write::read(&conn, &table, &id) {
                Err(StoreError::NotFound) => continue,
                values => values?,
            };
            for change in values {
                let edit = WorkEdit {
                    target: id.clone(),
                    change,
                };
                bytes += serde_json::to_vec(&edit)?.len();
                page.edits.push(edit);
            }
        }
        Ok(page)
    }

    /// Write another Machine's saves as this Machine's own, in one
    /// transaction. Applying the same edits again changes nothing.
    pub fn apply_hub_edits(
        &self,
        repo: &str,
        edits: &[WorkEdit],
        names: &MachineNames,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let arriving = new_comments(&tx, edits)?;
        planning_write::create(&tx, repo, edits, WriteOrigin::Local)?;
        comments_arrived(&tx, edits, &arriving, names)?;
        tx.commit()?;
        Ok(())
    }
}

/// The comments in `edits` this Machine does not have yet.
fn new_comments(conn: &Connection, edits: &[WorkEdit]) -> StoreResult<BTreeSet<String>> {
    let mut exists =
        conn.prepare_cached("SELECT EXISTS(SELECT 1 FROM task_comments WHERE id=?1)")?;
    let mut new = BTreeSet::new();
    for edit in edits {
        if matches!(edit.change, WorkValue::CommentTask(_))
            && !exists.query_row([&edit.target], |row| row.get::<_, bool>(0))?
        {
            new.insert(edit.target.clone());
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
        // A repository whose plan is on a hub has no tracker.
        super::task_ownership::reassert_in(tx, task, false)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::sqlite::planning_write;
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

    /// One `lf task sync`: read what the hub changed, then send what is open.
    fn exchange(laptop: &SqliteStore, hub: &SqliteStore) {
        let since = laptop.select_hub(REPO, "hub").unwrap().seq;
        let page = hub.plan_changes_since(REPO, since, usize::MAX).unwrap();
        laptop
            .import_hub_plan(REPO, &page.edits, page.seq, &Default::default())
            .unwrap();
        let sends = laptop.open_hub_sends(REPO).unwrap();
        let edits = sends
            .iter()
            .map(|send| send.edit.clone())
            .collect::<Vec<_>>();
        hub.apply_hub_edits(REPO, &edits, &Default::default())
            .unwrap();
        laptop
            .confirm_hub_sends(&sends.iter().map(|send| send.seq).collect::<Vec<_>>())
            .unwrap();
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

    #[test]
    fn a_plan_made_on_a_laptop_reaches_the_hub_and_the_hub_s_changes_come_back() {
        let dir = tempfile::tempdir().unwrap();
        let (laptop, hub) = (machine(&dir, "laptop"), machine(&dir, "hub"));
        // Made before the laptop had a hub: first contact sends it whole.
        let wave = Wave::new(WaveId::new(), "billing".into(), REPO.into());
        crate::test_fixture::wave(&laptop, &wave);
        let project = laptop.ensure_project(wave.id(), "Invoices").unwrap();
        exchange(&laptop, &hub);
        assert_eq!(hub.get_wave(wave.id()).unwrap().unwrap().name(), "billing");

        let task = TaskId::new();
        laptop
            .create_task(&NewTask {
                repository_id: crate::test_fixture::repository(&laptop, REPO),
                id: task.clone(),
                project_id: Some(project.clone()),
                title: "hub smoke".into(),
                description: "Brief".into(),
                due_date: None,
            })
            .unwrap();
        assert!(laptop
            .open_hub_sends(REPO)
            .unwrap()
            .iter()
            .any(|send| send.edit.change == WorkValue::TaskTitle(Some("hub smoke".into()))));
        exchange(&laptop, &hub);
        assert!(laptop.open_hub_sends(REPO).unwrap().is_empty());
        let held = hub.task(&task).unwrap().unwrap();
        assert_eq!(held.title, "hub smoke");
        assert_eq!(held.project_id, Some(project));

        rename_wave(&hub, wave.id(), "payments");
        retitle(&laptop, &task, "two");
        exchange(&laptop, &hub);
        assert_eq!(
            laptop.get_wave(wave.id()).unwrap().unwrap().name(),
            "payments"
        );
        assert_eq!(hub.task(&task).unwrap().unwrap().title, "two");
    }

    #[test]
    fn an_edit_saved_offline_wins_over_the_hub_s_older_value_when_it_arrives() {
        let dir = tempfile::tempdir().unwrap();
        let (laptop, hub) = (machine(&dir, "laptop"), machine(&dir, "hub"));
        let (_, _, task) = {
            let wave = Wave::new(WaveId::new(), "plan".into(), REPO.into());
            crate::test_fixture::wave(&laptop, &wave);
            let project = laptop.ensure_project(wave.id(), "Plan").unwrap();
            let task = TaskId::new();
            laptop
                .create_task(&NewTask {
                    repository_id: crate::test_fixture::repository(&laptop, REPO),
                    id: task.clone(),
                    project_id: Some(project.clone()),
                    title: "zero".into(),
                    description: String::new(),
                    due_date: None,
                })
                .unwrap();
            (wave, project, task)
        };
        exchange(&laptop, &hub);
        retitle(&hub, &task, "one");
        // Offline: saved here, open, not yet sent.
        retitle(&laptop, &task, "two");
        let open = laptop.open_hub_sends(REPO).unwrap();
        assert_eq!(open.len(), 1);
        exchange(&laptop, &hub);
        assert_eq!(laptop.task(&task).unwrap().unwrap().title, "two");
        assert_eq!(hub.task(&task).unwrap().unwrap().title, "two");

        // A reply lost after the hub wrote it: the send goes again, harmlessly.
        let revisions = hub.revisions().unwrap();
        hub.apply_hub_edits(REPO, &[open[0].edit.clone()], &Default::default())
            .unwrap();
        assert_eq!(hub.revisions().unwrap(), revisions);
    }

    #[test]
    fn a_machine_without_a_hub_queues_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let store = machine(&dir, "alone");
        let wave = Wave::new(WaveId::new(), "solo".into(), REPO.into());
        crate::test_fixture::wave(&store, &wave);
        rename_wave(&store, wave.id(), "renamed");
        assert!(store.open_hub_sends(REPO).unwrap().is_empty());
        let page = store.plan_changes_since(REPO, None, usize::MAX).unwrap();
        assert!(page.edits.contains(&WorkEdit {
            target: wave.id().to_string(),
            change: WorkValue::WaveName("renamed".into()),
        }));
    }
}
