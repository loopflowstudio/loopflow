//! The planning write boundary shared by local operations and imports.
//! Callers own their transaction and local delivery evidence; this writer changes
//! only planning columns and captures local mutations in that same transaction.
use crate::engine::planning_exchange::{PlanningKind, PlanningMutation, PlanningObject};
use crate::store::{StoreError, StoreResult};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Disposition {
    pub planning_state: Option<String>,
    pub planning_completed: i64,
    pub planning_completed_at: Option<String>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CommentContent {
    pub body: String,
    pub author: String,
    pub created_at: Option<String>,
}

macro_rules! planning_fields {
    ($(($kind:ident, $variant:ident, $column:literal, $value:ty)),* $(,)?) => {
        #[derive(Debug, Clone, PartialEq)]
        pub(crate) enum PlanningEdit {
            $($variant($value),)*
            Workflow(String),
            Krs(Vec<crate::pm::PmKr>),
            MetricTargets(Vec<crate::pm::ChapterMetricTarget>),
            Disposition(Disposition),
            CommentContent(CommentContent),
        }
        impl PlanningEdit {
            pub(crate) fn from_value(kind: PlanningKind, field: &str, value: Value) -> StoreResult<Self> {
                let edit=match (kind, field) {
                    $((PlanningKind::$kind, $column) => Self::$variant(serde_json::from_value(value)?),)*
                    (PlanningKind::Project, "workflow") => Self::Workflow(serde_json::from_value(value)?),
                    (PlanningKind::Project, "krs") => Self::Krs(serde_json::from_value(value)?),
                    (PlanningKind::Project, "metric_targets") => Self::MetricTargets(serde_json::from_value(value)?),
                    (PlanningKind::Task, "disposition") => Self::Disposition(serde_json::from_value(value)?),
                    (PlanningKind::Comment, "content") => Self::CommentContent(serde_json::from_value(value)?),
                    _ => return Err(StoreError::InvalidData("unknown planning field".into())),
                };
                edit.validate()?;
                Ok(edit)
            }
            pub(crate) fn kind(&self) -> PlanningKind {
                match self {
                    $(Self::$variant(_) => PlanningKind::$kind,)*
                    Self::Workflow(_) | Self::Krs(_) | Self::MetricTargets(_) => PlanningKind::Project,
                    Self::Disposition(_) => PlanningKind::Task,
                    Self::CommentContent(_) => PlanningKind::Comment,
                }
            }
            pub(crate) fn field(&self) -> &'static str {
                match self {
                    $(Self::$variant(_) => $column,)*
                    Self::Workflow(_) => "workflow", Self::Krs(_) => "krs",
                    Self::MetricTargets(_) => "metric_targets", Self::Disposition(_) => "disposition",
                    Self::CommentContent(_) => "content",
                }
            }
            pub(crate) fn value(&self) -> StoreResult<Value> {
                Ok(match self {
                    $(Self::$variant(v) => serde_json::to_value(v)?,)*
                    Self::Workflow(v) => serde_json::to_value(v)?,
                    Self::Krs(v) => serde_json::to_value(v)?,
                    Self::MetricTargets(v) => serde_json::to_value(v)?,
                    Self::Disposition(v) => serde_json::to_value(v)?,
                    Self::CommentContent(v) => serde_json::to_value(v)?,
                })
            }
        }
        pub(crate) fn fields(kind: PlanningKind) -> Vec<&'static str> {
            let mut fields = Vec::new();
            $(if kind == PlanningKind::$kind { fields.push($column); })*
            fields.extend(match kind {
                PlanningKind::Wave => &[][..],
                PlanningKind::Project => &["workflow", "krs", "metric_targets"],
                PlanningKind::Task => &["disposition"],
                PlanningKind::Comment => &["content"],
            });
            fields
        }
    }
}
planning_fields! {
    (Wave, WaveName, "name", String),
    (Wave, WaveParent, "parent_wave_id", Option<String>),
    (Wave, WaveProject, "current_project_id", Option<String>),
    (Project, ProjectWave, "wave_id", String),
    (Project, ProjectLinearId, "external_project_id", Option<String>),
    (Project, ProjectSlug, "project_slug", Option<String>),
    (Project, ProjectName, "project_name", Option<String>),
    (Project, ProjectSummary, "project_summary", String),
    (Project, ProjectStatus, "status", crate::pm::ProjectStatus),
    (Project, ProjectRank, "planning_rank", u32),
    (Project, ProjectInitiatives, "planning_initiatives", String),
    (Project, ProjectTeams, "planning_teams", String),
    (Task, TaskProject, "project_id", String),
    (Task, TaskLinearId, "external_issue_id", Option<String>),
    (Task, TaskIdentifier, "issue_identifier", String),
    (Task, TaskTitle, "issue_title", Option<String>),
    (Task, TaskDescription, "issue_description", Option<String>),
    (Task, TaskRank, "planning_rank", u32),
    (Task, TaskAssignee, "planning_assignee", Option<String>),
    (Task, TaskDueDate, "planning_due_date", Option<String>),
    (Task, TaskDeletedAt, "planning_deleted_at", Option<i64>),
    (Task, TaskUrl, "planning_url", Option<String>),
    (Task, TaskBranch, "planning_branch_name", Option<String>),
    (Task, TaskTeam, "planning_team_id", Option<String>),
    (Comment, CommentTask, "task_id", String),
}

impl PlanningEdit {
    fn validate(&self) -> StoreResult<()> {
        match self {
            Self::Disposition(v) if !matches!(v.planning_completed, 0 | 1) => {
                return Err(StoreError::InvalidData("invalid completion value".into()))
            }
            Self::CommentContent(v) => {
                serde_json::from_str::<crate::ops::pm::TaskCommentAuthor>(&v.author)?;
            }
            Self::ProjectInitiatives(v) | Self::ProjectTeams(v) => {
                serde_json::from_str::<Vec<String>>(v)?;
            }
            Self::Workflow(v) => crate::pm::ProjectContent {
                workflow: v.clone(),
                krs: Vec::new(),
                metric_targets: Vec::new(),
            }
            .validate()
            .map_err(|e| StoreError::InvalidData(e.to_string()))?,
            Self::Krs(v) => crate::pm::ProjectContent {
                workflow: String::new(),
                krs: v.clone(),
                metric_targets: Vec::new(),
            }
            .validate()
            .map_err(|e| StoreError::InvalidData(e.to_string()))?,
            Self::MetricTargets(v) => crate::pm::ProjectContent {
                workflow: String::new(),
                krs: Vec::new(),
                metric_targets: v.clone(),
            }
            .validate()
            .map_err(|e| StoreError::InvalidData(e.to_string()))?,
            _ => {}
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum WriteOrigin {
    Local,
    Import,
}

pub(crate) fn table(kind: PlanningKind) -> &'static str {
    match kind {
        PlanningKind::Wave => "waves",
        PlanningKind::Project => "projects",
        PlanningKind::Task => "tasks",
        PlanningKind::Comment => "task_comments",
    }
}

pub(crate) fn read(conn: &Connection, object: &PlanningObject) -> StoreResult<Vec<PlanningEdit>> {
    let mut fields = fields(object.kind);
    let content = if object.kind == PlanningKind::Project {
        fields.retain(|field| !matches!(*field, "workflow" | "krs" | "metric_targets"));
        Some(super::project_content::read_content(
            conn,
            &crate::durable::ProjectId::from_raw(&object.id),
        )?)
    } else {
        None
    };
    let expressions: Vec<_> = fields.iter().map(|field| match *field {
        "disposition" => "json_object('planning_state',planning_state,'planning_completed',planning_completed,'planning_completed_at',planning_completed_at)".into(),
        "content" => "json_object('body',body,'author',author,'created_at',created_at)".into(),
        field => format!("json_quote({field})"),
    }).collect();
    let sql = format!(
        "SELECT {} FROM {} WHERE id=?1",
        expressions.join(","),
        table(object.kind)
    );
    let values = conn.query_row(&sql, [&object.id], |row| {
        (0..fields.len())
            .map(|i| row.get::<_, String>(i))
            .collect::<Result<Vec<_>, _>>()
    })?;
    let mut edits: Vec<_> = fields
        .into_iter()
        .zip(values)
        .map(|(field, value)| {
            PlanningEdit::from_value(object.kind, field, serde_json::from_str(&value)?)
        })
        .collect::<StoreResult<_>>()?;
    if let Some(content) = content {
        edits.extend([
            PlanningEdit::Workflow(content.workflow),
            PlanningEdit::Krs(content.krs),
            PlanningEdit::MetricTargets(content.metric_targets),
        ]);
    }
    Ok(edits)
}

pub(crate) fn write(
    conn: &Connection,
    object: &PlanningObject,
    edits: &[PlanningEdit],
    origin: WriteOrigin,
) -> StoreResult<bool> {
    let before: BTreeMap<_, _> = read(conn, object)?
        .into_iter()
        .map(|edit| Ok((edit.field(), edit.value()?)))
        .collect::<StoreResult<_>>()?;
    let mut columns = BTreeMap::new();
    let mut content = None;
    let mut changed = Vec::new();
    for edit in edits {
        edit.validate()?;
        if edit.kind() != object.kind {
            return Err(StoreError::InvalidData(
                "planning field belongs to another object kind".into(),
            ));
        }
        if let PlanningEdit::WaveProject(Some(project)) = edit {
            let owner: Option<String> = conn
                .query_row("SELECT wave_id FROM projects WHERE id=?1", [project], |r| {
                    r.get(0)
                })
                .optional()?;
            if owner.as_deref() != Some(object.id.as_str()) {
                return Err(StoreError::InvalidAuthority(
                    "selected Project belongs to another Wave or is missing".into(),
                ));
            }
        }
        let value = edit.value()?;
        if before.get(edit.field()) == Some(&value) {
            continue;
        }
        match edit {
            PlanningEdit::Workflow(_) | PlanningEdit::Krs(_) | PlanningEdit::MetricTargets(_) => {
                if content.is_none() {
                    content = Some(super::project_content::read_content(
                        conn,
                        &crate::durable::ProjectId::from_raw(&object.id),
                    )?);
                }
                let content = content.as_mut().expect("content loaded");
                match edit {
                    PlanningEdit::Workflow(v) => content.workflow = v.clone(),
                    PlanningEdit::Krs(v) => content.krs = v.clone(),
                    PlanningEdit::MetricTargets(v) => content.metric_targets = v.clone(),
                    _ => unreachable!(),
                }
            }
            PlanningEdit::Disposition(v) => {
                if !matches!(v.planning_completed, 0 | 1) {
                    return Err(StoreError::InvalidData("invalid completion value".into()));
                }
                for (key, value) in value.as_object().expect("typed disposition").iter() {
                    columns.insert(key.clone(), value.clone());
                }
            }
            PlanningEdit::CommentContent(v) => {
                serde_json::from_str::<crate::ops::pm::TaskCommentAuthor>(&v.author)?;
                for (key, value) in value.as_object().expect("typed comment").iter() {
                    columns.insert(key.clone(), value.clone());
                }
            }
            _ => {
                columns.insert(edit.field().into(), value.clone());
            }
        }
        changed.push(edit);
    }
    if let Some(content) = content {
        content
            .validate()
            .map_err(|e| StoreError::InvalidData(e.to_string()))?;
        columns.insert(
            "project_prompt_context".into(),
            Value::String(crate::pm::render_project_content(&content)),
        );
        columns.insert("workflow".into(), content.workflow.into());
    }
    if !columns.is_empty() {
        let assignments = columns
            .keys()
            .map(|key| format!("{key}=json_extract(?2,'$.{key}')"))
            .collect::<Vec<_>>()
            .join(",");
        let revision = match object.kind {
            PlanningKind::Task => ",planning_revision=planning_revision+1,updated_at=unixepoch()",
            PlanningKind::Project => ",updated_at=unixepoch()",
            _ => "",
        };
        conn.execute(
            &format!(
                "UPDATE {} SET {assignments}{revision} WHERE id=?1",
                table(object.kind)
            ),
            params![object.id, serde_json::to_string(&columns)?],
        )?;
    }
    if matches!(origin, WriteOrigin::Local) {
        // Initial rows and every subsequent write use this same typed snapshot.
        capture(conn, object)?;
    }
    Ok(!changed.is_empty())
}

pub(crate) fn capture(conn: &Connection, object: &PlanningObject) -> StoreResult<()> {
    for edit in read(conn, object)? {
        let value = edit.value()?;
        let saved: Option<String> = conn.query_row("SELECT value FROM planning_peer_changes WHERE kind=?1 AND object_id=?2 AND field=?3 AND id IN (SELECT id FROM planning_peer_observed WHERE object_id=?2) ORDER BY clock DESC,id DESC LIMIT 1",params![object.kind.as_str(),object.id,edit.field()],|row|row.get(0)).optional()?;
        if saved
            .as_deref()
            .map(serde_json::from_str::<Value>)
            .transpose()?
            .as_ref()
            == Some(&value)
        {
            continue;
        }
        let parents: BTreeSet<String> = {
            let mut q=conn.prepare("SELECT id FROM planning_peer_observed WHERE object_id=?1 AND id IN(SELECT id FROM planning_peer_changes WHERE kind=?2 AND field=?3)")?;
            let rows = q.query_map(
                params![object.id, object.kind.as_str(), edit.field()],
                |r| r.get(0),
            )?;
            rows.collect::<Result<_, _>>()?
        };
        let clock: i64=conn.query_row("SELECT max(CAST(unixepoch('subsec')*1000 AS INTEGER),COALESCE(max(clock)+1,0)) FROM planning_peer_changes",[],|r|r.get(0))?;
        let change = PlanningMutation {
            object: object.clone(),
            field: edit.field().into(),
            value,
            clock,
            parents,
        };
        let id = uuid::Uuid::new_v4().to_string();
        retain(conn, &id, &change)?;
        observe(conn, object, edit.field(), std::iter::once(id.as_str()))?;
    }
    Ok(())
}

pub(crate) fn retain(conn: &Connection, id: &str, change: &PlanningMutation) -> StoreResult<()> {
    let existing:Option<(String,String,String,String,i64,String)>=conn.query_row("SELECT kind,object_id,field,value,clock,parents FROM planning_peer_changes WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?,r.get(5)?))).optional()?;
    if let Some((kind, object, field, value, clock, parents)) = existing {
        if kind != change.object.kind.as_str()
            || object != change.object.id
            || field != change.field
            || serde_json::from_str::<Value>(&value)? != change.value
            || clock != change.clock
            || serde_json::from_str::<BTreeSet<String>>(&parents)? != change.parents
        {
            return Err(StoreError::InvalidData(
                "planning mutation identity was reused".into(),
            ));
        }
        return Ok(());
    }
    conn.execute("INSERT OR IGNORE INTO planning_peer_changes(id,kind,object_id,field,value,clock,parents) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![id,change.object.kind.as_str(),change.object.id,change.field,serde_json::to_string(&change.value)?,change.clock,serde_json::to_string(&change.parents)?])?;
    Ok(())
}

pub(crate) fn observe<'a>(
    conn: &Connection,
    object: &PlanningObject,
    field: &str,
    ids: impl IntoIterator<Item = &'a str>,
) -> StoreResult<()> {
    conn.execute("DELETE FROM planning_peer_observed WHERE object_id=?1 AND id IN (SELECT id FROM planning_peer_changes WHERE kind=?2 AND field=?3)",params![object.id,object.kind.as_str(),field])?;
    for id in ids {
        conn.execute(
            "INSERT OR IGNORE INTO planning_peer_observed(object_id,id) VALUES(?1,?2)",
            params![object.id, id],
        )?;
    }
    Ok(())
}

pub(crate) fn ensure_record(
    conn: &Connection,
    repo: &str,
    object: &PlanningObject,
    edits: &[PlanningEdit],
) -> StoreResult<()> {
    let exists = conn.query_row(
        &format!(
            "SELECT EXISTS(SELECT 1 FROM {} WHERE id=?1)",
            table(object.kind)
        ),
        [&object.id],
        |r| r.get::<_, bool>(0),
    )?;
    if exists {
        return Ok(());
    }
    let fields = edits
        .iter()
        .map(|e| Ok((e.field(), e.value()?)))
        .collect::<StoreResult<BTreeMap<_, _>>>()?;
    let required = |field: &str| {
        fields
            .get(field)
            .and_then(Value::as_str)
            .ok_or_else(|| StoreError::InvalidData("required planning identity is missing".into()))
    };
    match object.kind {
        PlanningKind::Wave => {
            crate::id::WaveId::parse(&object.id)
                .map_err(|e| StoreError::InvalidData(e.to_string()))?;
            conn.execute("INSERT INTO waves(id,name,repo,created_at,parent_wave_id) VALUES(?1,?2,?3,unixepoch(),?4)",params![object.id,required("name")?,repo,fields.get("parent_wave_id").and_then(Value::as_str)])?;
        }
        PlanningKind::Project => {
            conn.execute(
                "INSERT INTO projects(id,wave_id,created_at,updated_at,project_prompt_context) VALUES(?1,?2,unixepoch(),unixepoch(),'')",
                params![object.id, required("wave_id")?],
            )?;
        }
        PlanningKind::Task => {
            conn.execute("INSERT INTO tasks(id,project_id,issue_identifier,created_at,updated_at,workspace_slug) VALUES(?1,?2,?3,unixepoch(),unixepoch(),'')",params![object.id,required("project_id")?,required("issue_identifier")?])?;
        }
        PlanningKind::Comment => {
            let c: &Value = &fields["content"];
            conn.execute("INSERT INTO task_comments(id,task_id,body,author,created_at) VALUES(?1,?2,?3,?4,?5)",params![object.id,required("task_id")?,c["body"].as_str(),c["author"].as_str(),c["created_at"].as_str()])?;
        }
    }
    Ok(())
}

/// Seed released planning once, in the migration's transaction.
pub(crate) fn seed(conn: &Connection) -> StoreResult<()> {
    for kind in [
        PlanningKind::Wave,
        PlanningKind::Project,
        PlanningKind::Task,
        PlanningKind::Comment,
    ] {
        let mut q = conn.prepare(&format!("SELECT id FROM {}", table(kind)))?;
        let ids = q
            .query_map([], |r| r.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        for id in ids {
            capture(conn, &PlanningObject { kind, id })?;
        }
    }
    Ok(())
}

/// Save local planning in the caller's transaction, including its causal identity.
pub(crate) fn local(
    conn: &Connection,
    kind: PlanningKind,
    id: &str,
    edits: &[PlanningEdit],
) -> StoreResult<bool> {
    write(
        conn,
        &PlanningObject {
            kind,
            id: id.into(),
        },
        edits,
        WriteOrigin::Local,
    )
}

/// Create the planning record before attaching any machine-local execution data.
pub(crate) fn create(
    conn: &Connection,
    repo: &str,
    kind: PlanningKind,
    id: &str,
    edits: &[PlanningEdit],
) -> StoreResult<()> {
    let object = PlanningObject {
        kind,
        id: id.into(),
    };
    ensure_record(conn, repo, &object, edits)?;
    write(conn, &object, edits, WriteOrigin::Local)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{create, local, PlanningEdit as Edit};
    use crate::durable::{ProjectId, TaskId};
    use crate::engine::planning_exchange::PlanningKind;
    use crate::engine::planning_git::PlanningDestination;
    use crate::id::WaveId;
    use crate::ops::pm::{TaskComment, TaskCommentAuthor};
    use crate::planning::NewTask;
    use crate::pm::PmItemUpdate;
    use crate::store::sqlite::SqliteStore;
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
                PlanningKind::Project,
                project.as_str(),
                &[
                    Edit::ProjectWave(wave.id().to_string()),
                    Edit::ProjectSlug(Some("plan".into())),
                    Edit::ProjectName(Some("Plan".into())),
                ],
            )
            .unwrap();
            local(
                &tx,
                PlanningKind::Wave,
                wave.id().as_str(),
                &[Edit::WaveProject(Some(project.to_string()))],
            )
            .unwrap();
            tx.commit().unwrap();
        }
        let id = TaskId::new();
        store
            .create_task(&NewTask {
                id: id.clone(),
                project_id: project,
                title: "Original".into(),
                description: "Brief".into(),
                due_date: None,
            })
            .unwrap();
        id
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
            store.use_peer_planning("/repo", Some(&route.id())).unwrap();
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
            conn.execute("INSERT INTO task_workflows(task_id,graph,node,updated_at) VALUES(?1,?2,'review',1)",rusqlite::params![task.as_str(),r#"{"name":"review","nodes":[{"name":"review","skill":"review","description":null}],"edges":[]}"#]).unwrap();
        }
        let original = a.task(&task).unwrap().unwrap();
        a.edit_task(
            &task,
            original.plan.revision,
            &PmItemUpdate {
                name: Some("Changed".into()),
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
        let received = b.task(&task).unwrap().unwrap();
        assert_eq!(received.plan.title, "Changed");
        assert_eq!(received.worktree.unwrap().to_str(), Some("/retained"));
        assert_eq!(received.branch, "local-branch");
        let conn = b.conn.lock().unwrap();
        assert_eq!(
            conn.query_row(
                "SELECT node FROM task_workflows WHERE task_id=?1",
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
                "SELECT body FROM task_comments WHERE id='comment-proof'",
                [],
                |r| r.get::<_, String>(0)
            )
            .unwrap(),
            "Keep this finding"
        );
        assert_eq!(
            conn.query_row("SELECT count(*) FROM task_state_deliveries", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0
        );
        drop(conn);
        let before = a.export_peer_planning("/repo", &route.id()).unwrap();
        a.conn.lock().unwrap().execute_batch("CREATE TRIGGER fail_capture BEFORE INSERT ON planning_peer_changes BEGIN SELECT RAISE(ABORT,'injected journal failure'); END;").unwrap();
        let current = a.task(&task).unwrap().unwrap();
        let pending = a.pending_task_changes(&task).unwrap();
        assert!(a
            .edit_task(
                &task,
                current.plan.revision,
                &PmItemUpdate {
                    name: Some("Must roll back".into()),
                    ..Default::default()
                }
            )
            .is_err());
        assert_eq!(
            a.task(&task).unwrap().unwrap().plan.title,
            current.plan.title
        );
        assert_eq!(a.pending_task_changes(&task).unwrap(), pending);
        assert_eq!(
            a.export_peer_planning("/repo", &route.id()).unwrap(),
            before
        );
    }

    #[test]
    fn rejected_selection_retains_history_without_becoming_an_observed_parent() {
        use crate::engine::planning_exchange::{PlanningMutation, PlanningObject};
        use serde_json::json;
        let home = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&home.path().join("db")).unwrap();
        let task = plan(&store);
        let wave = store.task(&task).unwrap().unwrap().wave_id;
        let object = PlanningObject {
            kind: PlanningKind::Wave,
            id: wave.to_string(),
        };
        let route =
            PlanningDestination::new("/synthetic.git", "refs/loopflow/planning/shared/test")
                .unwrap();
        store.bind_peer_planning("/repo", &route).unwrap();
        store.use_peer_planning("/repo", Some(&route.id())).unwrap();
        let mut incoming = store.export_peer_planning("/repo", &route.id()).unwrap();
        let prior = incoming
            .winners()
            .find(|(_, c)| c.object == object && c.field == "current_project_id")
            .unwrap()
            .1
            .value
            .clone();
        for (id, field, value) in [
            ("rejected", "current_project_id", json!("project_missing")),
            ("rename", "name", json!("Renamed")),
        ] {
            let parents = incoming
                .heads()
                .filter(|(_, c)| c.object == object && c.field == field)
                .map(|(id, _)| id.to_owned())
                .collect();
            let clock = incoming.changes.values().map(|c| c.clock).max().unwrap() + 1;
            incoming.changes.insert(
                id.into(),
                PlanningMutation {
                    object: object.clone(),
                    field: field.into(),
                    value,
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
        assert_eq!(
            store.peer_planning_status("/repo").unwrap()[0]
                .conflicts
                .len(),
            1
        );
        let mut conn = store.conn.lock().unwrap();
        let values = super::read(&conn, &object).unwrap();
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
            PlanningKind::Wave,
            wave.as_str(),
            &[Edit::WaveName("Local edit".into())],
        )
        .unwrap();
        tx.commit().unwrap();
        drop(conn);
        let local = store.export_peer_planning("/repo", &route.id()).unwrap();
        let (_, name) = local
            .winners()
            .find(|(_, c)| c.object == object && c.field == "name")
            .unwrap();
        assert!(name.parents.contains("rename"));
        assert!(!local
            .changes
            .values()
            .any(|c| c.parents.contains("rejected")));
        let before = local.clone();
        let mut reused = incoming;
        reused.changes.get_mut("rename").unwrap().value = json!("Reused identity");
        assert!(store
            .import_peer_planning("/repo", &route.id(), "invalid", &reused)
            .is_err());
        assert_eq!(
            store.export_peer_planning("/repo", &route.id()).unwrap(),
            before
        );
    }
}
