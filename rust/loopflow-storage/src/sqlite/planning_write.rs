//! The planning write boundary shared by local operations, tracker
//! observations and values arriving from other Machines. Callers own their
//! transaction; this writer changes planning columns, stamps each field a
//! save here changes, records which record changed last for carriers to send,
//! and queues each value a local write changed for the tracker.
use crate::work::edit::{
    CommentContent, Disposition, StampedRecord, StampedValue, WorkEdit, WorkValue,
};
use crate::{StoreError, StoreResult};
use rusqlite::{params, Connection};
use std::collections::{BTreeMap, BTreeSet, HashMap};

/// Who a planning write comes from, which decides what else it records.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteOrigin {
    /// A person or agent here: stamped, and queued for the tracker.
    Local,
    /// A fact the tracker reported: stamped, never echoed back to it.
    Tracker,
    /// Another Machine's value, which keeps the stamp it arrived with.
    Import,
}

pub(crate) const WORK_TABLES: [&str; 4] = ["waves", "projects", "tasks", "task_comments"];

// SQL ownership is private to the writer. Callers address fields by their
// typed variant and target, without another public object/kind model.
pub(crate) fn table(value: &WorkValue) -> &'static str {
    match value {
        WorkValue::WaveName(_) | WorkValue::WaveParent(_) | WorkValue::WaveRetired(_) => "waves",
        WorkValue::ProjectWave(_)
        | WorkValue::ProjectLinearId(_)
        | WorkValue::ProjectSlug(_)
        | WorkValue::ProjectName(_)
        | WorkValue::ProjectSummary(_)
        | WorkValue::ProjectDescription(_)
        | WorkValue::ProjectStatus(_)
        | WorkValue::ProjectRank(_)
        | WorkValue::ProjectInitiatives(_)
        | WorkValue::ProjectTeams(_) => "projects",
        WorkValue::TaskProject(_)
        | WorkValue::TaskLinearId(_)
        | WorkValue::TaskIdentifier(_)
        | WorkValue::TaskTitle(_)
        | WorkValue::TaskDescription(_)
        | WorkValue::TaskRank(_)
        | WorkValue::TaskAssignee(_)
        | WorkValue::TaskDueDate(_)
        | WorkValue::TaskDeletedAt(_)
        | WorkValue::TaskUrl(_)
        | WorkValue::TaskBranch(_)
        | WorkValue::TaskTeam(_)
        | WorkValue::Disposition(_) => "tasks",
        WorkValue::CommentTask(_) | WorkValue::CommentContent(_) => "task_comments",
    }
}

fn column(value: &WorkValue) -> &'static str {
    match value {
        WorkValue::WaveName(_) => "name",
        WorkValue::WaveParent(_) => "parent_wave_id",
        WorkValue::WaveRetired(_) => "retired_at",
        WorkValue::ProjectWave(_) => "wave_id",
        WorkValue::ProjectLinearId(_) => "external_project_id",
        WorkValue::ProjectSlug(_) => "project_slug",
        WorkValue::ProjectName(_) => "project_name",
        WorkValue::ProjectSummary(_) => "project_summary",
        WorkValue::ProjectDescription(_) => "description",
        WorkValue::ProjectStatus(_) => "status",
        WorkValue::ProjectRank(_) => "planning_rank",
        WorkValue::ProjectInitiatives(_) => "planning_initiatives",
        WorkValue::ProjectTeams(_) => "planning_teams",
        WorkValue::TaskProject(_) => "project_id",
        WorkValue::TaskLinearId(_) => "external_issue_id",
        WorkValue::TaskIdentifier(_) => "issue_identifier",
        WorkValue::TaskTitle(_) => "issue_title",
        WorkValue::TaskDescription(_) => "issue_description",
        WorkValue::TaskRank(_) => "planning_rank",
        WorkValue::TaskAssignee(_) => "planning_assignee",
        WorkValue::TaskDueDate(_) => "planning_due_date",
        WorkValue::TaskDeletedAt(_) => "planning_deleted_at",
        WorkValue::TaskUrl(_) => "planning_url",
        WorkValue::TaskBranch(_) => "planning_branch_name",
        WorkValue::TaskTeam(_) => "planning_team_id",
        WorkValue::CommentTask(_) => "task_id",
        WorkValue::Disposition(_) => "disposition",
        WorkValue::CommentContent(_) => "content",
    }
}

pub(crate) fn read(conn: &Connection, table: &str, id: &str) -> StoreResult<Vec<WorkValue>> {
    let values = match table {
        "waves" => read_record(conn, "SELECT name,parent_wave_id,retired_at FROM waves WHERE id=?1", id, |row| {
                    let retired: Option<i64> = row.get(2)?;
                    Ok([
                        WorkValue::WaveName(row.get(0)?),
                        WorkValue::WaveParent(row.get(1)?),
                    ]
                    .into_iter()
                    .chain(retired.map(|at| WorkValue::WaveRetired(Some(at))))
                    .collect())
        }),
        "projects" => read_record(conn, "SELECT wave_id,external_project_id,project_slug,project_name,project_summary,description,status,planning_rank,planning_initiatives,planning_teams FROM projects WHERE id=?1", id, |row| {
                    Ok(vec![
                        WorkValue::ProjectWave(row.get(0)?),
                        WorkValue::ProjectLinearId(row.get(1)?),
                        WorkValue::ProjectSlug(row.get(2)?),
                        WorkValue::ProjectName(row.get(3)?),
                        WorkValue::ProjectSummary(row.get(4)?),
                        WorkValue::ProjectDescription(row.get(5)?),
                        WorkValue::ProjectStatus(serde_json::from_value(
                            row.get::<_, String>(6)?.into(),
                        )?),
                        WorkValue::ProjectRank(row.get(7)?),
                        WorkValue::ProjectInitiatives(serde_json::from_str(
                            &row.get::<_, String>(8)?,
                        )?),
                        WorkValue::ProjectTeams(serde_json::from_str(&row.get::<_, String>(9)?)?),
                    ])
        }),
        "tasks" => read_record(conn, "SELECT project_id,external_issue_id,issue_identifier,issue_title,issue_description,planning_rank,planning_assignee,planning_due_date,planning_deleted_at,planning_url,planning_branch_name,planning_team_id,planning_state,planning_completed,planning_completed_at FROM tasks WHERE id=?1", id, |row| {
                    Ok(vec![
                        WorkValue::TaskProject(row.get(0)?),
                        WorkValue::TaskLinearId(row.get(1)?),
                        WorkValue::TaskIdentifier(row.get(2)?),
                        WorkValue::TaskTitle(row.get(3)?),
                        WorkValue::TaskDescription(row.get(4)?),
                        WorkValue::TaskRank(row.get(5)?),
                        WorkValue::TaskAssignee(row.get(6)?),
                        WorkValue::TaskDueDate(row.get(7)?),
                        WorkValue::TaskDeletedAt(row.get(8)?),
                        WorkValue::TaskUrl(row.get(9)?),
                        WorkValue::TaskBranch(row.get(10)?),
                        WorkValue::TaskTeam(row.get(11)?),
                        WorkValue::Disposition(Disposition {
                            planning_state: row.get(12)?,
                            planning_completed: row.get(13)?,
                            planning_completed_at: row.get(14)?,
                        }),
                    ])
        }),
        "task_comments" => read_record(conn, "SELECT task_id,body,author,created_at FROM task_comments WHERE id=?1", id, |row| {
                    Ok(vec![
                        WorkValue::CommentTask(row.get(0)?),
                        WorkValue::CommentContent(CommentContent {
                            body: row.get(1)?,
                            author: serde_json::from_str(&row.get::<_, String>(2)?)?,
                            created_at: row.get(3)?,
                        }),
                    ])
        }),
        _ => Err(StoreError::InvalidData("unknown Work table".into())),
    }?;
    for value in &values {
        value.validate()?;
    }
    Ok(values)
}

fn read_record(
    conn: &Connection,
    sql: &str,
    id: &str,
    decode: impl FnOnce(&rusqlite::Row<'_>) -> StoreResult<Vec<WorkValue>>,
) -> StoreResult<Vec<WorkValue>> {
    let mut query = conn.prepare(sql)?;
    let mut rows = query.query([id])?;
    decode(rows.next()?.ok_or(StoreError::NotFound)?)
}

/// `queue`: whether a `Local` write's changes are actions. A record created in
/// this write is not edited by it: its tracker learns it through `create`.
fn write_record(
    conn: &Connection,
    table: &str,
    id: &str,
    edits: &[WorkEdit],
    origin: WriteOrigin,
    queue: bool,
) -> StoreResult<bool> {
    let before = read(conn, table, id)?;
    let mut columns = BTreeMap::new();
    let mut changes = Vec::new();
    for edit in edits {
        let edit = &edit.change;
        edit.validate()?;
        if before.contains(edit) {
            continue;
        }
        changes.push(edit);
        let value = edit.value()?;
        match edit {
            WorkValue::Disposition(_) | WorkValue::CommentContent(_) => {
                columns.extend(value.as_object().expect("typed fields").clone());
            }
            _ => {
                columns.insert(column(edit).into(), value.clone());
            }
        }
    }
    let previous_identifier = if table == "tasks" && columns.contains_key("issue_identifier") {
        super::children::short_task_id(conn, &loopflow_model::durable::TaskId::from_raw(id))?
    } else {
        None
    };
    let changed = !columns.is_empty();
    if changed {
        let assignments = columns
            .keys()
            .map(|key| format!("{key}=json_extract(?2,'$.{key}')"))
            .collect::<Vec<_>>()
            .join(",");
        let revision = match table {
            "tasks" => ",planning_revision=planning_revision+1,updated_at=unixepoch()",
            "projects" => ",updated_at=unixepoch()",
            _ => "",
        };
        conn.execute(
            &format!("UPDATE {} SET {assignments}{revision} WHERE id=?1", table),
            params![id, serde_json::to_string(&columns)?],
        )?;
    }
    if table == "tasks" && columns.contains_key("issue_identifier") {
        super::sessions::repair_task_titles_in(
            conn,
            &loopflow_model::durable::TaskId::from_raw(id),
            previous_identifier.as_deref(),
        )?;
    }
    if origin != WriteOrigin::Import {
        // A record created here is new in every field it was given.
        let written: Vec<&str> = if queue {
            changes.iter().map(|change| change.field()).collect()
        } else {
            edits.iter().map(|edit| edit.change.field()).collect()
        };
        if !written.is_empty() {
            let (at, machine) = (next_at(conn)?, local_machine(conn)?);
            for field in written {
                stamp(conn, id, field, at, &machine)?;
            }
            super::plan_carriers::changed(conn, table, id)?;
        }
    }
    if origin == WriteOrigin::Local && queue {
        for change in changes {
            super::tracker_actions::enqueue_local(conn, table, id, change)?;
        }
    }
    Ok(changed)
}

// ---- Stamps ----

#[cfg(test)]
thread_local! {
    /// The wall clock a test sets instead of waiting for one.
    pub(crate) static CLOCK_MS: std::cell::Cell<Option<i64>> = const { std::cell::Cell::new(None) };
}

fn wall_clock_ms() -> i64 {
    #[cfg(test)]
    if let Some(at) = CLOCK_MS.with(std::cell::Cell::get) {
        return at;
    }
    (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as i64
}

/// The next stamp a save here takes: the wall clock, or one past the highest
/// stamp this Machine has written or taken in when the clock is behind it.
pub(crate) fn next_at(conn: &Connection) -> StoreResult<i64> {
    let highest: i64 = conn
        .prepare_cached("SELECT COALESCE(MAX(at),0) FROM plan_stamps")?
        .query_row([], |row| row.get(0))?;
    Ok(wall_clock_ms().max(highest + 1))
}

fn local_machine(conn: &Connection) -> StoreResult<String> {
    Ok(conn
        .prepare_cached("SELECT id FROM machines WHERE is_local=1")?
        .query_row([], |row| row.get(0))?)
}

fn stamp(conn: &Connection, id: &str, field: &str, at: i64, machine: &str) -> StoreResult<()> {
    conn.prepare_cached(
        "INSERT INTO plan_stamps(owner_id,field,at,machine_id) VALUES(?1,?2,?3,?4)
         ON CONFLICT(owner_id,field) DO UPDATE SET at=excluded.at,machine_id=excluded.machine_id",
    )?
    .execute(params![id, field, at, machine])?;
    Ok(())
}

fn stamps(conn: &Connection, id: &str) -> StoreResult<HashMap<String, (i64, String)>> {
    let mut query =
        conn.prepare_cached("SELECT field,at,machine_id FROM plan_stamps WHERE owner_id=?1")?;
    let held = query
        .query_map([id], |row| Ok((row.get(0)?, (row.get(1)?, row.get(2)?))))?
        .collect::<Result<_, _>>()?;
    Ok(held)
}

/// A record as it travels: every field it has, each with the stamp held for
/// it. A field never stamped reads as `(0, this Machine)`.
pub(crate) fn stamped(conn: &Connection, table: &str, id: &str) -> StoreResult<StampedRecord> {
    let values = read(conn, table, id)?;
    let mut held = stamps(conn, id)?;
    let local = local_machine(conn)?;
    Ok(StampedRecord {
        table: table.to_owned(),
        id: id.to_owned(),
        fields: values
            .into_iter()
            .map(|value| {
                let (at, machine) = held
                    .remove(value.field())
                    .unwrap_or_else(|| (0, local.clone()));
                StampedValue { value, at, machine }
            })
            .collect(),
    })
}

/// The one way values from another Machine come in. A field is written only
/// when its stamp is newer than the one held; a record this Machine lacks is
/// created from all of its fields. Returns what was written. Applying the
/// same records again writes nothing and changes no `plan_changes` row.
pub(crate) fn apply(
    conn: &Connection,
    repo: &str,
    records: &[StampedRecord],
) -> StoreResult<Vec<WorkEdit>> {
    let local = local_machine(conn)?;
    let mut won = Vec::new();
    for record in records {
        if !WORK_TABLES.contains(&record.table.as_str()) {
            return Err(StoreError::InvalidData("unknown Work table".into()));
        }
        let mut held = None;
        for field in &record.fields {
            if table(&field.value) != record.table {
                return Err(StoreError::InvalidData(
                    "a record carries another table's field".into(),
                ));
            }
            let edit = WorkEdit {
                target: record.id.clone(),
                change: field.value.clone(),
            }
            .canonicalized()?;
            let (exists, held) = match &held {
                Some(held) => held,
                None => {
                    let exists = conn.query_row(
                        &format!("SELECT EXISTS(SELECT 1 FROM {} WHERE id=?1)", record.table),
                        [&edit.target],
                        |row| row.get::<_, bool>(0),
                    )?;
                    held.insert((exists, stamps(conn, &edit.target)?))
                }
            };
            let mine = held
                .get(edit.change.field())
                .map_or((0, local.as_str()), |(at, machine)| (*at, machine.as_str()));
            if !*exists || (field.at, field.machine.as_str()) > mine {
                won.push((edit, field.at, field.machine.as_str()));
            }
        }
    }
    let edits: Vec<WorkEdit> = won.iter().map(|(edit, ..)| edit.clone()).collect();
    create(conn, repo, &edits, WriteOrigin::Import)?;
    let mut changed = BTreeSet::new();
    for (edit, at, machine) in &won {
        stamp(conn, &edit.target, edit.change.field(), *at, machine)?;
        changed.insert((table(&edit.change), edit.target.as_str()));
    }
    for (table, id) in changed {
        super::plan_carriers::changed(conn, table, id)?;
    }
    Ok(edits)
}

/// Registration is acquired by the application before the command transaction.
fn registered_repository_in(
    conn: &Connection,
    repo: &str,
) -> StoreResult<loopflow_model::durable::RepositoryId> {
    super::repositories::repository_id_in(conn, repo)?.ok_or(StoreError::NotFound)
}

pub(crate) fn ensure_record(
    conn: &Connection,
    repo: &str,
    table: &str,
    id: &str,
    edits: &[WorkEdit],
) -> StoreResult<bool> {
    let exists = conn.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM {} WHERE id=?1)", table),
        [id],
        |r| r.get::<_, bool>(0),
    )?;
    if exists {
        return Ok(false);
    }
    let missing = || StoreError::InvalidData("required planning identity is missing".into());
    // The value the edits leave a field with.
    macro_rules! last {
        ($field:ident) => {
            edits.iter().rev().find_map(|edit| match &edit.change {
                WorkValue::$field(value) => Some(value),
                _ => None,
            })
        };
    }
    match table {
        "waves" => {
            loopflow_model::id::WaveId::parse(id)
                .map_err(|e| StoreError::InvalidData(e.to_string()))?;
            registered_repository_in(conn, repo)?;
            let name = last!(WaveName).ok_or_else(missing)?;
            let parent = last!(WaveParent).and_then(Option::as_deref);
            conn.execute("INSERT INTO waves(id,name,repo,created_at,parent_wave_id) VALUES(?1,?2,?3,unixepoch(),?4)",params![id,name,repo,parent])?;
        }
        "projects" => {
            let wave = last!(ProjectWave).ok_or_else(missing)?;
            conn.execute(
                "INSERT INTO projects(id,wave_id,created_at,updated_at,description) VALUES(?1,?2,unixepoch(),unixepoch(),'')",
                params![id, wave],
            )?;
        }
        "tasks" => {
            let project = last!(TaskProject).ok_or_else(missing)?;
            let identifier = last!(TaskIdentifier).ok_or_else(missing)?;
            let repository = registered_repository_in(conn, repo)?;
            conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,created_at,updated_at,workspace_slug,repository_id) VALUES(?1,?2,?3,unixepoch(),unixepoch(),'',?4)",params![id,project,identifier,repository.as_str()])?;
        }
        "task_comments" => {
            let task = last!(CommentTask).ok_or_else(missing)?;
            let content = last!(CommentContent).ok_or_else(missing)?;
            conn.execute("INSERT INTO task_comments(id,task_id,body,author,created_at) VALUES(?1,?2,?3,?4,?5)",params![id,task,content.body,serde_json::to_string(&content.author)?,content.created_at])?;
        }
        _ => return Err(StoreError::InvalidData("unknown Work table".into())),
    }
    Ok(true)
}

pub(crate) fn grouped(edits: &[WorkEdit]) -> BTreeMap<(&str, &str), Vec<WorkEdit>> {
    let mut records = BTreeMap::<_, Vec<_>>::new();
    for edit in edits {
        records
            .entry((table(&edit.change), edit.target.as_str()))
            .or_default()
            .push(edit.clone());
    }
    records
}

fn canonicalized(edits: &[WorkEdit]) -> StoreResult<Vec<WorkEdit>> {
    edits.iter().map(WorkEdit::canonicalized).collect()
}

pub(crate) fn write(
    conn: &Connection,
    edits: &[WorkEdit],
    origin: WriteOrigin,
) -> StoreResult<bool> {
    let edits = canonicalized(edits)?;
    let mut changed = false;
    for ((table, id), values) in grouped(&edits) {
        changed |= write_record(conn, table, id, &values, origin, true)?;
    }
    Ok(changed)
}

/// Save local Work in the caller's transaction, with the tracker actions it
/// asks for.
pub(crate) fn local(conn: &Connection, edits: &[WorkEdit]) -> StoreResult<bool> {
    write(conn, edits, WriteOrigin::Local)
}

/// Save what the tracker holds, which it needs no action to learn.
pub(crate) fn tracker(conn: &Connection, edits: &[WorkEdit]) -> StoreResult<bool> {
    write(conn, edits, WriteOrigin::Tracker)
}

/// Create shared records before attaching any machine-local execution data.
pub(crate) fn create(
    conn: &Connection,
    repo: &str,
    edits: &[WorkEdit],
    origin: WriteOrigin,
) -> StoreResult<()> {
    let edits = canonicalized(edits)?;
    let records = grouped(&edits);
    // Foreign-key parents exist before children even when the caller's edits
    // arrive in a different order. Fields still go through the common writer.
    let mut created = BTreeSet::new();
    for table in WORK_TABLES {
        for (&(record_table, id), values) in &records {
            if record_table == table && ensure_record(conn, repo, table, id, values)? {
                created.insert((table, id));
                // A new comment is a change to its Task, though the insert
                // already holds it. A new Task or Project is a `create`.
                if origin == WriteOrigin::Local && table == "task_comments" {
                    for value in values {
                        super::tracker_actions::enqueue_local(conn, table, id, &value.change)?;
                    }
                }
            }
        }
    }
    for (&(table, id), values) in &records {
        write_record(
            conn,
            table,
            id,
            values,
            origin,
            !created.contains(&(table, id)),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{create, local};
    use crate::session::{LfSession, TitleSource};
    use crate::sqlite::{SessionCaller, SqliteStore};
    use crate::work::edit::{Disposition, WorkValue as Edit};
    use crate::work::task::NewTask;
    use crate::work::wave::Wave;
    use loopflow_model::durable::{ProjectId, TaskId};
    use loopflow_model::id::WaveId;

    fn plan(store: &SqliteStore) -> TaskId {
        let wave = Wave::new(WaveId::new(), "planning".into(), "/repo".into());
        crate::test_fixture::wave(store, &wave);
        let project = ProjectId::for_wave(wave.id());
        {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            create(
                &tx,
                "/repo",
                &crate::work::edit::WorkEdit::for_target(
                    project.as_str(),
                    &[
                        Edit::ProjectWave(wave.id().to_string()),
                        Edit::ProjectSlug(Some("plan".into())),
                        Edit::ProjectName(Some("Plan".into())),
                    ],
                ),
                super::WriteOrigin::Local,
            )
            .unwrap();
            tx.commit().unwrap();
        }
        let id = TaskId::new();
        store
            .create_task(&NewTask {
                repository_id: crate::test_fixture::repository(store, "/repo"),
                id: id.clone(),
                project_id: Some(project),
                title: "Original".into(),
                description: "Brief".into(),
                due_date: None,
            })
            .unwrap();
        id
    }

    #[test]
    fn local_task_fields_survive_restart() {
        let (_home, store, id) = crate::test_fixture::store_with_task();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "UPDATE tasks SET planning_due_date='2026-10-15' WHERE id=?1",
                [id.as_str()],
            )
            .unwrap();
        let before = store.task(&id).unwrap().unwrap();
        let path = std::path::PathBuf::from(store.conn.lock().unwrap().path().unwrap());
        {
            let mut conn = store.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            local(
                &tx,
                &crate::work::edit::WorkEdit::for_target(
                    id.as_str(),
                    &[
                        Edit::TaskRank(7),
                        Edit::TaskAssignee(Some("assignee-1".into())),
                        Edit::Disposition(Disposition {
                            planning_state: Some("completed".into()),
                            planning_completed: 1,
                            planning_completed_at: Some("2026-10-10T12:00:00Z".into()),
                        }),
                    ],
                ),
            )
            .unwrap();
            tx.commit().unwrap();
        }
        drop(store);
        let store = crate::test_fixture::open(&path).unwrap();
        let saved = store.task(&id).unwrap().unwrap();
        assert_eq!(saved.rank, 7);
        assert_eq!(saved.assignee.as_deref(), Some("assignee-1"));
        assert_eq!(saved.state.as_deref(), Some("completed"));
        assert_eq!(saved.completed_at.as_deref(), Some("2026-10-10T12:00:00Z"));
        assert_eq!(saved.worktree, before.worktree);
        assert_eq!(saved.branch, before.branch);
        assert_eq!(saved.due_date, before.due_date);
        assert_eq!(saved.rank, 7);
    }

    #[test]
    fn local_identifier_repair_uses_saved_titles_and_attachments() {
        let (_home, store, task) = crate::test_fixture::store_with_task();
        let template = crate::test_fixture::unpublished_conversation(Some(task.clone()), None, 1);
        for (id, title_source) in [
            ("editor", TitleSource::Generated),
            ("other", TitleSource::Generated),
            ("human", TitleSource::Human),
        ] {
            store
                .create_session_prepared(
                    LfSession {
                        id: id.into(),
                        title: "Saved title".into(),
                        title_source,
                        artifact_key: Some(crate::session_record::new_artifact_key()),
                        ..template.clone()
                    },
                    None,
                )
                .unwrap();
        }
        let process = loopflow_model::id::LfProcessId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,started_at) VALUES(?1,'saved',1)",
                [&process],
            )
            .unwrap();
        let caller = SessionCaller {
            agent: None,
            capture_key: None,
        };
        let old = store
            .attach_session("editor", None, &process, None, &caller)
            .unwrap();
        store
            .attach_session("editor", Some(&old), &process, None, &caller)
            .unwrap();
        let mut conn = store.conn.lock().unwrap();
        let tx = conn.transaction().unwrap();
        local(
            &tx,
            &crate::work::edit::WorkEdit::for_target(
                task.as_str(),
                &[Edit::TaskIdentifier("ENG-123".into())],
            ),
        )
        .unwrap();
        tx.commit().unwrap();
        drop(conn);
        for id in ["editor", "other"] {
            assert_eq!(
                store.session(id).unwrap().unwrap().title,
                "ENG-123 Saved title"
            );
        }
        assert_eq!(
            store.session("human").unwrap().unwrap().title,
            "Saved title"
        );
    }

    #[test]
    fn structured_project_fields_survive_restart_without_markdown() {
        let left = tempfile::tempdir().unwrap();
        let path = left.path().join("db");
        let a = crate::test_fixture::open(&path).unwrap();
        let task = plan(&a);
        let project = a.task(&task).unwrap().unwrap().project_id;
        let mut saved = a.project(project.as_ref().unwrap()).unwrap().unwrap();
        // These strings would be interpreted as Markdown by the retired storage path.
        saved.description = "Authored purpose\nworkflow: literal prose\n## KRs\n- [ ] prose".into();
        a.insert_project(&saved).unwrap();
        drop(a);
        let a = crate::test_fixture::open(&path).unwrap();
        assert_eq!(
            a.project(project.as_ref().unwrap()).unwrap().unwrap(),
            saved
        );
    }
}
