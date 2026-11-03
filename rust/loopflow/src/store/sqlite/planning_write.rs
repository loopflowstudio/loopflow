//! The planning write boundary shared by local operations and imports.
//! Callers own their transaction and local delivery evidence; this writer changes
//! only planning columns and captures local mutations in that same transaction.
use crate::planning_exchange::PlanningMutation;
use crate::store::{StoreError, StoreResult};
use crate::work::edit::{CommentContent, Disposition, WorkEdit, WorkValue};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy)]
pub(crate) enum WriteOrigin {
    Local,
    Import,
}

const WORK_TABLES: [&str; 4] = ["waves", "projects", "tasks", "task_comments"];

// SQL ownership is private to the writer. Git addresses fields by their typed
// variant and target, without another public object/kind model.
pub(crate) fn table(value: &WorkValue) -> &'static str {
    match value {
        WorkValue::WaveName(_)
        | WorkValue::WaveParent(_)
        | WorkValue::WaveProject(_)
        | WorkValue::HistoricalWaveWorkflow(_) => "waves",
        WorkValue::ProjectWave(_)
        | WorkValue::ProjectLinearId(_)
        | WorkValue::ProjectSlug(_)
        | WorkValue::ProjectName(_)
        | WorkValue::ProjectSummary(_)
        | WorkValue::ProjectDescription(_)
        | WorkValue::ProjectStatus(_)
        | WorkValue::ProjectRank(_)
        | WorkValue::ProjectInitiatives(_)
        | WorkValue::ProjectTeams(_)
        | WorkValue::HistoricalProjectWorkflow(_)
        | WorkValue::Krs(_)
        | WorkValue::MetricTargets(_) => "projects",
        WorkValue::TaskProject(_)
        | WorkValue::TaskLinearId(_)
        | WorkValue::TaskIdentifier(_)
        | WorkValue::TaskTitle(_)
        | WorkValue::TaskDescription(_)
        | WorkValue::TaskRank(_)
        | WorkValue::TaskAssignee(_)
        | WorkValue::HistoricalTaskPaused(_)
        | WorkValue::TaskShippedAt(_)
        | WorkValue::TaskDueDate(_)
        | WorkValue::TaskDeletedAt(_)
        | WorkValue::TaskUrl(_)
        | WorkValue::TaskBranch(_)
        | WorkValue::TaskTeam(_)
        | WorkValue::TaskRegressedIn(_)
        | WorkValue::Disposition(_) => "tasks",
        WorkValue::CommentTask(_) | WorkValue::CommentContent(_) => "task_comments",
    }
}

fn column(value: &WorkValue) -> &'static str {
    match value {
        WorkValue::WaveName(_) => "name",
        WorkValue::WaveParent(_) => "parent_wave_id",
        WorkValue::WaveProject(_) => "current_project_id",
        WorkValue::HistoricalWaveWorkflow(_) => "workflow",
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
        WorkValue::HistoricalTaskPaused(_) => "paused",
        WorkValue::TaskShippedAt(_) => "shipped_at",
        WorkValue::TaskDueDate(_) => "planning_due_date",
        WorkValue::TaskDeletedAt(_) => "planning_deleted_at",
        WorkValue::TaskUrl(_) => "planning_url",
        WorkValue::TaskBranch(_) => "planning_branch_name",
        WorkValue::TaskTeam(_) => "planning_team_id",
        WorkValue::TaskRegressedIn(_) => "regressed_in",
        WorkValue::CommentTask(_) => "task_id",
        WorkValue::HistoricalProjectWorkflow(_) => "workflow",
        WorkValue::Krs(_) => "krs",
        WorkValue::MetricTargets(_) => "metric_targets",
        WorkValue::Disposition(_) => "disposition",
        WorkValue::CommentContent(_) => "content",
    }
}

pub(crate) fn read(conn: &Connection, table: &str, id: &str) -> StoreResult<Vec<WorkValue>> {
    let mut values = match table {
        "waves" => read_record(conn, "SELECT name,parent_wave_id,current_project_id FROM waves WHERE id=?1", id, |row| {
                    Ok(vec![
                        WorkValue::WaveName(row.get(0)?),
                        WorkValue::WaveParent(row.get(1)?),
                        WorkValue::WaveProject(row.get(2)?),
                    ])
        }),
        "projects" => read_record(conn, "SELECT wave_id,external_project_id,project_slug,project_name,project_summary,description,status,planning_rank,planning_initiatives,planning_teams,krs,metric_targets FROM projects WHERE id=?1", id, |row| {
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
                        WorkValue::Krs(serde_json::from_str(&row.get::<_, String>(10)?)?),
                        WorkValue::MetricTargets(serde_json::from_str(&row.get::<_, String>(11)?)?),
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
    // Historical migrations seed their journal before later Task columns exist.
    if table == "tasks" {
        if let Some(paused) = optional_task_column(conn, id, "paused")? {
            values.push(WorkValue::HistoricalTaskPaused(paused));
        }
        if let Some(shipped_at) = optional_task_column(conn, id, "shipped_at")? {
            values.push(WorkValue::TaskShippedAt(shipped_at));
        }
        if let Some(regressed_in) = optional_task_column(conn, id, "regressed_in")? {
            values.push(WorkValue::TaskRegressedIn(regressed_in));
        }
    }
    for value in &values {
        value.validate()?;
    }
    Ok(values)
}

/// A `tasks` column's value, or `None` before the migration that adds it.
fn optional_task_column<T: rusqlite::types::FromSql>(
    conn: &Connection,
    id: &str,
    column: &str,
) -> StoreResult<Option<T>> {
    let exists: bool = conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM pragma_table_info('tasks') WHERE name=?1)",
        [column],
        |row| row.get(0),
    )?;
    if !exists {
        return Ok(None);
    }
    Ok(Some(conn.query_row(
        &format!("SELECT {column} FROM tasks WHERE id=?1"),
        [id],
        |row| row.get(0),
    )?))
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

fn write_record(
    conn: &Connection,
    table: &str,
    id: &str,
    edits: &[WorkEdit],
    origin: WriteOrigin,
) -> StoreResult<bool> {
    let before = read(conn, table, id)?;
    let mut columns = BTreeMap::new();
    for edit in edits {
        let edit = &edit.change;
        if matches!(
            edit,
            WorkValue::HistoricalProjectWorkflow(_) | WorkValue::HistoricalWaveWorkflow(_)
        ) {
            continue;
        }
        edit.validate()?;
        if let WorkValue::WaveProject(Some(project)) = edit {
            let owner: Option<String> = conn
                .query_row("SELECT wave_id FROM projects WHERE id=?1", [project], |r| {
                    r.get(0)
                })
                .optional()?;
            if owner.as_deref() != Some(id) {
                return Err(StoreError::InvalidAuthority(
                    "selected Project belongs to another Wave or is missing".into(),
                ));
            }
        }
        if before.contains(edit) {
            continue;
        }
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
        super::children::short_task_id(conn, &crate::durable::TaskId::from_raw(id))?
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
            &crate::durable::TaskId::from_raw(id),
            previous_identifier.as_deref(),
        )?;
    }
    if matches!(origin, WriteOrigin::Local) {
        // Initial rows and every subsequent write use this same typed snapshot.
        capture(conn, table, id)?;
    }
    Ok(changed)
}

pub(crate) fn capture(conn: &Connection, table: &str, target: &str) -> StoreResult<()> {
    for change in read(conn, table, target)? {
        let edit = WorkEdit {
            target: target.into(),
            change,
        };
        let saved: Option<String> = conn
            .query_row(
                "SELECT mutation FROM planning_peer_changes WHERE object_id=?1 AND field=?2
             AND id IN (SELECT id FROM planning_peer_observed)
             ORDER BY clock DESC,id DESC LIMIT 1",
                params![target, edit.change.field()],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(saved) = saved {
            if serde_json::from_str::<PlanningMutation>(&saved)?.edit == edit {
                continue;
            }
        }
        let parents = {
            let mut query = conn.prepare(
                "SELECT id FROM planning_peer_changes WHERE object_id=?1 AND field=?2
                 AND id IN (SELECT id FROM planning_peer_observed)",
            )?;
            let parents = query
                .query_map(params![target, edit.change.field()], |row| row.get(0))?
                .collect::<Result<BTreeSet<String>, _>>()?;
            parents
        };
        let clock = conn.query_row(
            "SELECT max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE(max(clock)+1,0)) FROM planning_peer_changes",
            [], |row| row.get(0),
        )?;
        let mutation = PlanningMutation {
            edit,
            clock,
            parents,
        };
        let id = uuid::Uuid::new_v4().to_string();
        retain(conn, &id, &mutation)?;
        observe(conn, &mutation.edit, std::iter::once(id.as_str()))?;
    }
    Ok(())
}

pub(crate) fn retain(conn: &Connection, id: &str, change: &PlanningMutation) -> StoreResult<()> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT mutation FROM planning_peer_changes WHERE id=?1",
            [id],
            |row| row.get(0),
        )
        .optional()?;
    if let Some(existing) = existing {
        if serde_json::from_str::<PlanningMutation>(&existing)? != *change {
            return Err(StoreError::InvalidData(
                "planning mutation identity was reused".into(),
            ));
        }
        return Ok(());
    }
    conn.execute(
        "INSERT INTO planning_peer_changes(id,record_table,mutation) VALUES(?1,?2,?3)",
        params![
            id,
            table(&change.edit.change),
            serde_json::to_string(change)?
        ],
    )?;
    Ok(())
}

pub(crate) fn observe<'a>(
    conn: &Connection,
    edit: &WorkEdit,
    ids: impl IntoIterator<Item = &'a str>,
) -> StoreResult<()> {
    conn.execute(
        "DELETE FROM planning_peer_observed WHERE id IN
         (SELECT id FROM planning_peer_changes WHERE object_id=?1 AND field=?2)",
        params![edit.target, edit.change.field()],
    )?;
    for id in ids {
        conn.execute(
            "INSERT OR IGNORE INTO planning_peer_observed(id) VALUES(?1)",
            [id],
        )?;
    }
    Ok(())
}

pub(crate) fn ensure_record(
    conn: &Connection,
    repo: &str,
    table: &str,
    id: &str,
    edits: &[WorkEdit],
) -> StoreResult<bool> {
    if table == "waves" {
        super::repositories::ensure_repository_in(conn, repo)?;
    }
    let exists = conn.query_row(
        &format!("SELECT EXISTS(SELECT 1 FROM {} WHERE id=?1)", table),
        [id],
        |r| r.get::<_, bool>(0),
    )?;
    if exists {
        return Ok(false);
    }
    let missing = || StoreError::InvalidData("required planning identity is missing".into());
    match table {
        "waves" => {
            crate::id::WaveId::parse(id).map_err(|e| StoreError::InvalidData(e.to_string()))?;
            let name = edits
                .iter()
                .rev()
                .find_map(|edit| match &edit.change {
                    WorkValue::WaveName(name) => Some(name),
                    _ => None,
                })
                .ok_or_else(missing)?;
            let parent = edits
                .iter()
                .rev()
                .find_map(|edit| match &edit.change {
                    WorkValue::WaveParent(parent) => Some(parent),
                    _ => None,
                })
                .and_then(Option::as_deref);
            conn.execute("INSERT INTO waves(id,name,repo,created_at,parent_wave_id) VALUES(?1,?2,?3,unixepoch(),?4)",params![id,name,repo,parent])?;
        }
        "projects" => {
            let wave = edits
                .iter()
                .rev()
                .find_map(|edit| match &edit.change {
                    WorkValue::ProjectWave(wave) => Some(wave),
                    _ => None,
                })
                .ok_or_else(missing)?;
            conn.execute(
                "INSERT INTO projects(id,wave_id,created_at,updated_at,description) VALUES(?1,?2,unixepoch(),unixepoch(),'')",
                params![id, wave],
            )?;
        }
        "tasks" => {
            let project = edits
                .iter()
                .rev()
                .find_map(|edit| match &edit.change {
                    WorkValue::TaskProject(project) => Some(project),
                    _ => None,
                })
                .ok_or_else(missing)?;
            let identifier = edits
                .iter()
                .rev()
                .find_map(|edit| match &edit.change {
                    WorkValue::TaskIdentifier(identifier) => Some(identifier),
                    _ => None,
                })
                .ok_or_else(missing)?;
            let repository = super::repositories::ensure_repository_in(conn, repo)?;
            conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,created_at,updated_at,workspace_slug,repository_id) VALUES(?1,?2,?3,unixepoch(),unixepoch(),'',?4)",params![id,project,identifier,repository.as_str()])?;
        }
        "task_comments" => {
            let task = edits
                .iter()
                .rev()
                .find_map(|edit| match &edit.change {
                    WorkValue::CommentTask(task) => Some(task),
                    _ => None,
                })
                .ok_or_else(missing)?;
            let content = edits
                .iter()
                .rev()
                .find_map(|edit| match &edit.change {
                    WorkValue::CommentContent(content) => Some(content),
                    _ => None,
                })
                .ok_or_else(missing)?;
            conn.execute("INSERT INTO task_comments(id,task_id,body,author,created_at) VALUES(?1,?2,?3,?4,?5)",params![id,task,content.body,serde_json::to_string(&content.author)?,content.created_at])?;
        }
        _ => return Err(StoreError::InvalidData("unknown Work table".into())),
    }
    Ok(true)
}

/// Seed released planning once, in the migration's transaction.
pub(crate) fn seed(conn: &Connection) -> StoreResult<()> {
    for table in WORK_TABLES {
        let mut query = conn.prepare(&format!("SELECT id FROM {table}"))?;
        let ids = query
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for id in ids {
            capture(conn, table, &id)?;
        }
    }
    Ok(())
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

pub(crate) fn write(
    conn: &Connection,
    edits: &[WorkEdit],
    origin: WriteOrigin,
) -> StoreResult<bool> {
    let edits = edits
        .iter()
        .map(WorkEdit::canonicalized)
        .collect::<StoreResult<Vec<_>>>()?;
    let mut changed = false;
    for ((table, id), values) in grouped(&edits) {
        changed |= write_record(conn, table, id, &values, origin)?;
    }
    Ok(changed)
}

/// Save local Work in the caller's transaction, including its causal identity.
pub(crate) fn local(conn: &Connection, edits: &[WorkEdit]) -> StoreResult<bool> {
    write(conn, edits, WriteOrigin::Local)
}

/// Create shared records before attaching any machine-local execution data.
pub(crate) fn create(conn: &Connection, repo: &str, edits: &[WorkEdit]) -> StoreResult<()> {
    let edits = edits
        .iter()
        .map(WorkEdit::canonicalized)
        .collect::<StoreResult<Vec<_>>>()?;
    let records = grouped(&edits);
    // Foreign-key parents exist before children even when the caller's edits
    // arrive in a different order. Capturing fields still uses the common writer.
    for table in WORK_TABLES {
        for (&(record_table, id), values) in &records {
            if record_table == table {
                ensure_record(conn, repo, table, id, values)?;
            }
        }
    }
    write(conn, &edits, WriteOrigin::Local)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{create, local};
    use crate::durable::{ProjectId, TaskId};
    use crate::id::WaveId;
    use crate::planning_git::PlanningDestination;
    use crate::store::sqlite::SqliteStore;
    use crate::work::edit::{WorkEdit, WorkValue, WorkValue as Edit};
    use crate::work::task::comment::{TaskComment, TaskCommentAuthor};
    use crate::work::task::NewTask;
    use crate::work::task::TaskUpdate;
    use crate::work::wave::Wave;

    fn plan(store: &SqliteStore) -> TaskId {
        let wave = Wave::new(WaveId::new(), "planning".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        let project = ProjectId::new();
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
            )
            .unwrap();
            local(
                &tx,
                &crate::work::edit::WorkEdit::for_target(
                    wave.id().as_str(),
                    &[Edit::WaveProject(Some(project.to_string()))],
                ),
            )
            .unwrap();
            tx.commit().unwrap();
        }
        let id = TaskId::new();
        store
            .create_task(&NewTask {
                repository_id: store.ensure_repository("/repo").unwrap(),
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
    fn standalone_task_exchange_preserves_membership_comments_and_ownership() {
        let left = tempfile::tempdir().unwrap();
        let right = tempfile::tempdir().unwrap();
        let a = SqliteStore::open_ephemeral(&left.path().join("db")).unwrap();
        let b = SqliteStore::open_ephemeral(&right.path().join("db")).unwrap();
        let repo = tempfile::tempdir().unwrap();
        let canonical = repo.path().canonicalize().unwrap();
        let path = canonical.to_str().unwrap();
        let route =
            PlanningDestination::new("/synthetic.git", "refs/loopflow/planning/shared/test")
                .unwrap();
        a.bind_peer_planning(path, &route).unwrap();
        b.bind_peer_planning(path, &route).unwrap();
        let task = a
            .create_task(&NewTask {
                id: TaskId::new(),
                repository_id: a.ensure_repository(path).unwrap(),
                project_id: None,
                title: "Before filing".into(),
                description: "Keep the same Work".into(),
                due_date: None,
            })
            .unwrap();
        a.append_task_comment(
            &task.id,
            &crate::work::task::comment::TaskComment {
                id: "standalone-comment".into(),
                body: "Retain direction".into(),
                author: crate::work::task::comment::TaskCommentAuthor::Integration,
                created_at: Some("2026-10-17T00:00:00Z".into()),
            },
        )
        .unwrap();
        let claim = a.claim_task(&task.id).unwrap().unwrap();
        let journal = a.export_peer_planning(path, &route.id()).unwrap();
        b.import_peer_planning(path, &route.id(), "first", &journal)
            .unwrap();
        let imported = b.task(&task.id).unwrap().unwrap();
        assert!(imported.project_id.is_none());
        assert_ne!(imported.repository_id, task.repository_id);
        assert_eq!(
            b.repository_path(&imported.repository_id)
                .unwrap()
                .as_deref(),
            Some(path)
        );
        // The claim travels as a marked comment; the other Machine follows it.
        let ownership = b.task_ownership(&task.id).unwrap();
        assert_eq!(ownership.log, vec![claim.clone()]);
        assert_eq!(
            ownership.owner.unwrap().machine.id,
            Some(a.local_machine().unwrap().id)
        );
        let project = b.ensure_wave_project(path, "product").unwrap();
        b.move_task(&task.id, None, Some(&project.id)).unwrap();
        let journal = b.export_peer_planning(path, &route.id()).unwrap();
        a.import_peer_planning(path, &route.id(), "attached", &journal)
            .unwrap();
        assert_eq!(
            a.task(&task.id).unwrap().unwrap().project_id,
            Some(project.id.clone())
        );
        a.move_task(&task.id, Some(&project.id), None).unwrap();
        a.delete_task(&task.id).unwrap();
        let journal = a.export_peer_planning(path, &route.id()).unwrap();
        b.import_peer_planning(path, &route.id(), "detached", &journal)
            .unwrap();
        assert!(b.task(&task.id).unwrap().unwrap().project_id.is_none());
        assert!(b.list_tasks(None).unwrap().is_empty());
        let conn = b.conn.lock().unwrap();
        let body: String = conn
            .query_row(
                "SELECT body FROM task_comments WHERE id='standalone-comment'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(body, "Retain direction");
    }

    #[test]
    fn structured_project_fields_survive_restart_and_peer_import_without_markdown() {
        let left = tempfile::tempdir().unwrap();
        let right = tempfile::tempdir().unwrap();
        let path = left.path().join("db");
        let a = SqliteStore::open_ephemeral(&path).unwrap();
        let task = plan(&a);
        let project = a.task(&task).unwrap().unwrap().project_id;
        let mut saved = a.project(project.as_ref().unwrap()).unwrap().unwrap();
        // These strings would be interpreted as Markdown by the retired storage path.
        saved.description = "Authored purpose\nworkflow: literal prose".into();

        saved.krs = vec![crate::work::project::ProjectKR {
            text: "First line\n## Not a section\n- [ ] not another KR".into(),
            holds: true,
        }];
        saved.metric_targets = vec![crate::work::wave::metrics::MetricTarget {
            metric_id: "recovery".into(),
            target: crate::work::wave::metrics::Threshold::AtLeast { value: 1.0 },
        }];
        a.insert_project(&saved).unwrap();
        drop(a);
        let a = SqliteStore::open_ephemeral(&path).unwrap();
        assert_eq!(
            a.project(project.as_ref().unwrap()).unwrap().unwrap(),
            saved
        );
        let b = SqliteStore::open_ephemeral(&right.path().join("db")).unwrap();
        let route =
            PlanningDestination::new("/synthetic.git", "refs/loopflow/planning/shared/test")
                .unwrap();
        for store in [&a, &b] {
            store.bind_peer_planning("/repo", &route).unwrap();
        }
        {
            let conn = a.conn.lock().unwrap();
            local(
                &conn,
                &crate::work::edit::WorkEdit::for_target(
                    project.as_ref().unwrap().as_str(),
                    &[
                        Edit::ProjectTeams(vec!["team-1".into(), "team-2".into()]),
                        Edit::ProjectInitiatives(vec!["initiative-1".into()]),
                    ],
                ),
            )
            .unwrap();
        }
        let journal = a.export_peer_planning("/repo", &route.id()).unwrap();
        b.import_peer_planning("/repo", &route.id(), "revision", &journal)
            .unwrap();
        let received = b.project(project.as_ref().unwrap()).unwrap().unwrap();
        assert_eq!(received.description, saved.description);
        assert_eq!(received.krs, saved.krs);
        assert_eq!(received.metric_targets, saved.metric_targets);

        let conn = b.conn.lock().unwrap();
        let (teams, initiatives): (String, String) = conn
            .query_row(
                "SELECT planning_teams,planning_initiatives FROM projects WHERE id=?1",
                [project.as_ref().unwrap().as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&teams).unwrap(),
            ["team-1", "team-2"]
        );
        assert_eq!(
            serde_json::from_str::<Vec<String>>(&initiatives).unwrap(),
            ["initiative-1"]
        );
        drop(conn);
        let current = a.project(project.as_ref().unwrap()).unwrap().unwrap();
        let pending = a
            .pending_project_changes(project.as_ref().unwrap())
            .unwrap();
        a.conn.lock().unwrap().execute_batch("CREATE TRIGGER fail_project_capture BEFORE INSERT ON planning_peer_changes BEGIN SELECT RAISE(ABORT,'injected failure'); END;").unwrap();
        assert!(a
            .edit_project(project.as_ref().unwrap(), Some("Renamed"), None, None)
            .is_err());
        assert_eq!(
            a.project(project.as_ref().unwrap()).unwrap().unwrap(),
            current
        );
        assert_eq!(
            a.pending_project_changes(project.as_ref().unwrap())
                .unwrap(),
            pending
        );
    }

    #[test]
    fn edits_imports_and_failed_saves_share_atomic_planning_without_execution() {
        let left = tempfile::tempdir().unwrap();
        let right = tempfile::tempdir().unwrap();
        let a = SqliteStore::open_ephemeral(&left.path().join("db")).unwrap();
        let b = SqliteStore::open_ephemeral(&right.path().join("db")).unwrap();
        let task = plan(&a);
        let route =
            PlanningDestination::new("/synthetic.git", "refs/loopflow/planning/shared/test")
                .unwrap();
        for store in [&a, &b] {
            store.bind_peer_planning("/repo", &route).unwrap();
        }
        let base = a.export_peer_planning("/repo", &route.id()).unwrap();
        b.import_peer_planning("/repo", &route.id(), "base", &base)
            .unwrap();
        {
            let conn = b.conn.lock().unwrap();
            conn.execute(
                "UPDATE tasks SET worktree='/retained',branch='local-branch' WHERE id=?1",
                [task.as_str()],
            )
            .unwrap();
            conn.execute(
                "INSERT INTO task_graphs(task_id,graph,step,updated_at) VALUES(?1,?2,'review',1)",
                rusqlite::params![task.as_str(), r#"{"name":"review","steps":[]}"#],
            )
            .unwrap();
        }
        let local_wave = b.task(&task).unwrap().unwrap().wave_id;
        {
            let conn = b.conn.lock().unwrap();
            conn.execute_batch("INSERT INTO processes(id,trace_id,command,cwd,started_at) VALUES('11111111-1111-4111-8111-111111111111','trace','lf run code','/retained/task',2);").unwrap();
            conn.execute_batch(
                &include_str!("../../../tests/fixtures/migrations/planning_local_sessions.sql")
                    .replace("task-proof", task.as_str())
                    .replace("wave-proof", local_wave.as_ref().unwrap().as_str()),
            )
            .unwrap();
        }
        let execution = || {
            let conn = b.conn.lock().unwrap();
            [
                "processes",
                "agent_sessions",
                "session_events",
                "session_activity",
                "task_graphs",
                "task_state_deliveries",
            ]
            .into_iter()
            .map(|table| {
                let mut query = conn
                    .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
                    .unwrap();
                let columns = query.column_count();
                query
                    .query_map([], |row| {
                        (0..columns)
                            .map(|i| row.get::<_, rusqlite::types::Value>(i))
                            .collect::<rusqlite::Result<Vec<_>>>()
                    })
                    .unwrap()
                    .collect::<rusqlite::Result<Vec<_>>>()
                    .unwrap()
            })
            .collect::<Vec<_>>()
        };
        let retained_execution = execution();
        let original = a.task(&task).unwrap().unwrap();
        a.edit_task(
            &task,
            original.revision,
            &TaskUpdate {
                title: Some("Changed".into()),
                ..Default::default()
            },
        )
        .unwrap();
        a.append_task_comment(
            &task,
            &TaskComment {
                id: "comment-proof".into(),
                body: "Keep this finding".into(),
                author: TaskCommentAuthor::Person {
                    name: Some("Maya".into()),
                },
                created_at: Some("2026-10-09T10:00:00Z".into()),
            },
        )
        .unwrap();
        {
            let mut conn = a.conn.lock().unwrap();
            let tx = conn.transaction().unwrap();
            super::super::task_state_delivery::queue_in(&tx, &task, "completed").unwrap();
            tx.commit().unwrap();
        }
        let snapshot = a.export_peer_planning("/repo", &route.id()).unwrap();
        assert_eq!(
            snapshot,
            a.export_peer_planning("/repo", &route.id()).unwrap()
        );
        b.import_peer_planning("/repo", &route.id(), "next", &snapshot)
            .unwrap();
        let revision = b.revisions().unwrap();
        b.import_peer_planning("/repo", &route.id(), "next", &snapshot)
            .unwrap();
        assert_eq!(revision, b.revisions().unwrap());
        assert_eq!(execution(), retained_execution);
        let received = b.task(&task).unwrap().unwrap();
        assert_eq!(received.title, "Changed");
        assert_eq!(received.worktree.unwrap().to_str(), Some("/retained"));
        assert_eq!(received.branch, "local-branch");
        let conn = b.conn.lock().unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT step FROM task_graphs WHERE task_id=?1",
                [task.as_str()],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "review"
        );
        assert!(conn
            .query_row(
                "SELECT planning_completed FROM tasks WHERE id=?1",
                [task.as_str()],
                |r| r.get::<_, bool>(0)
            )
            .unwrap());
        assert_eq!(
            conn.query_row(
                "SELECT body,author FROM task_comments WHERE id='comment-proof'",
                [],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?))
            )
            .map(|(body, author)| (
                body,
                serde_json::from_str::<TaskCommentAuthor>(&author).unwrap()
            ))
            .unwrap(),
            (
                "Keep this finding".into(),
                TaskCommentAuthor::Person {
                    name: Some("Maya".into())
                }
            )
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM task_state_deliveries", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        drop(conn);
        let before = a.export_peer_planning("/repo", &route.id()).unwrap();
        a.conn.lock().unwrap().execute_batch("CREATE TRIGGER fail_capture BEFORE INSERT ON planning_peer_changes BEGIN SELECT RAISE(ABORT,'injected journal failure'); END;").unwrap();
        let current = a.task(&task).unwrap().unwrap();
        let pending = a.pending_task_changes(&task).unwrap();
        assert!(a
            .edit_task(
                &task,
                current.revision,
                &TaskUpdate {
                    title: Some("Must roll back".into()),
                    ..Default::default()
                }
            )
            .is_err());
        assert_eq!(a.task(&task).unwrap().unwrap().title, current.title);
        assert_eq!(a.pending_task_changes(&task).unwrap(), pending);
        assert_eq!(
            a.export_peer_planning("/repo", &route.id()).unwrap(),
            before
        );
    }

    #[test]
    fn rejected_selection_retains_history_without_becoming_an_observed_parent() {
        use crate::planning_exchange::PlanningMutation;
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let task = plan(&store);
        let wave = store.task(&task).unwrap().unwrap().wave_id;
        let route =
            PlanningDestination::new("/synthetic.git", "refs/loopflow/planning/shared/test")
                .unwrap();
        store.bind_peer_planning("/repo", &route).unwrap();
        let mut incoming = store.export_peer_planning("/repo", &route.id()).unwrap();
        let prior = incoming
            .winners()
            .find(|(_, c)| {
                c.edit.target == wave.as_ref().unwrap().as_str()
                    && c.edit.change.field() == "wave_project"
            })
            .unwrap()
            .1
            .edit
            .change
            .value()
            .unwrap();
        for (id, value) in [
            (
                "rejected",
                WorkValue::WaveProject(Some("project_missing".into())),
            ),
            ("rename", WorkValue::WaveName("Renamed".into())),
        ] {
            let field = value.field();
            let parents = incoming
                .heads()
                .filter(|(_, c)| {
                    c.edit.target == wave.as_ref().unwrap().as_str()
                        && c.edit.change.field() == field
                })
                .map(|(id, _)| id.to_owned())
                .collect();
            let clock = incoming.changes.values().map(|c| c.clock).max().unwrap() + 1;
            incoming.changes.insert(
                id.into(),
                PlanningMutation {
                    edit: WorkEdit {
                        target: wave.as_ref().unwrap().to_string(),
                        change: value,
                    },
                    clock,
                    parents,
                },
            );
        }
        store
            .import_peer_planning("/repo", &route.id(), "rejected", &incoming)
            .unwrap();
        let exported = store.export_peer_planning("/repo", &route.id()).unwrap();
        assert_eq!(exported.changes["rejected"], incoming.changes["rejected"]);
        let status = store.peer_planning_status("/repo", None).unwrap().remove(0);
        assert_eq!(status.conflicts.len(), 1);
        let journal = store
            .peer_planning_journal("/repo", None)
            .unwrap()
            .remove(0);
        assert_eq!(journal.changes, exported.changes);
        assert!(journal.candidates.contains("rejected"));
        let mut conn = store.conn.lock().unwrap();
        let values = super::read(&conn, "waves", wave.as_ref().unwrap().as_str()).unwrap();
        assert!(values.contains(&Edit::WaveName("Renamed".into())));
        assert!(values.contains(&Edit::WaveProject(serde_json::from_value(prior).unwrap())));
        assert_eq!(
            conn.query_row(
                "SELECT count(*) FROM planning_peer_observed WHERE id='rejected'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            0
        );
        let tx = conn.transaction().unwrap();
        local(
            &tx,
            &crate::work::edit::WorkEdit::for_target(
                wave.as_ref().unwrap().as_str(),
                &[Edit::WaveName("Local edit".into())],
            ),
        )
        .unwrap();
        tx.commit().unwrap();
        drop(conn);
        let local = store.export_peer_planning("/repo", &route.id()).unwrap();
        let (_, name) = local
            .winners()
            .find(|(_, c)| {
                c.edit.target == wave.as_ref().unwrap().as_str()
                    && c.edit.change.field() == "wave_name"
            })
            .unwrap();
        assert!(name.parents.contains("rename"));
        assert!(!local
            .changes
            .values()
            .any(|c| c.parents.contains("rejected")));
        let before = local.clone();
        let mut reused = incoming;
        reused.changes.get_mut("rename").unwrap().edit.change =
            WorkValue::WaveName("Reused identity".into());
        assert!(store
            .import_peer_planning("/repo", &route.id(), "invalid", &reused)
            .is_err());
        assert_eq!(
            store.export_peer_planning("/repo", &route.id()).unwrap(),
            before
        );
    }
}
