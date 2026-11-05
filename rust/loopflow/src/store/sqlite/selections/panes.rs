use super::{invalid, SqliteStore};
use crate::durable::RepositoryId;
use crate::store::{StoreError, StoreResult};
use crate::work::selection::{
    PaneIntent, PaneTree, WorkPaneLayout, WorkWorkspaceKey, WorkWorkspaceKind,
};
use rusqlite::{params, Connection, OptionalExtension, TransactionBehavior};

fn kind_name(kind: &WorkWorkspaceKind) -> &'static str {
    match kind {
        WorkWorkspaceKind::Repository => "repository",
        WorkWorkspaceKind::Wave => "wave",
        WorkWorkspaceKind::Task => "task",
    }
}

// Pane persistence needs Work ownership, not selection membership or Session activity.
fn validate_owner(conn: &Connection, work: &WorkWorkspaceKey) -> StoreResult<()> {
    RepositoryId::parse(&work.repository).map_err(invalid)?;
    let sql: String = match work.kind {
        WorkWorkspaceKind::Repository =>
            "SELECT EXISTS(SELECT 1 FROM repository_plans WHERE id=?1 AND id=?2)".into(),
        WorkWorkspaceKind::Wave =>
            "SELECT EXISTS(SELECT 1 FROM wave_addresses w JOIN repository_plans r ON r.repo=w.repo WHERE r.id=?1 AND w.id=?2 AND w.retired_at IS NULL)".into(),
        WorkWorkspaceKind::Task =>
            format!("SELECT EXISTS(SELECT 1 FROM tasks t LEFT JOIN projects p ON p.id=t.project_id LEFT JOIN waves w ON w.id=p.wave_id WHERE t.repository_id=?1 AND t.id=?2 AND w.retired_at IS NULL AND {})", super::super::children::TASK_VISIBLE),
    };
    let belongs: bool =
        conn.query_row(&sql, params![work.repository, work.id], |row| row.get(0))?;
    if belongs {
        Ok(())
    } else {
        Err(StoreError::NotFound)
    }
}

pub(super) fn layout_in(
    conn: &Connection,
    work: &WorkWorkspaceKey,
) -> StoreResult<Option<WorkPaneLayout>> {
    let json: Option<String> = conn.query_row(
        "SELECT layout_json FROM work_pane_layouts WHERE repository_id=?1 AND kind=?2 AND work_id=?3",
        params![work.repository,kind_name(&work.kind),work.id], |row| row.get(0),
    ).optional()?;
    json.map(|json| serde_json::from_str(&json).map_err(StoreError::from))
        .transpose()
}

fn validate_sessions(
    conn: &Connection,
    work: &WorkWorkspaceKey,
    tree: &PaneTree,
) -> StoreResult<()> {
    for (_, content) in tree.panes() {
        let PaneIntent::Session {
            session_id: Some(session_id),
            ..
        } = content
        else {
            continue;
        };
        let scope = match work.kind {
            WorkWorkspaceKind::Repository => "s.primary_scope='repository'".to_string(),
            WorkWorkspaceKind::Wave => "s.primary_scope='wave' AND s.wave_id=?3".to_string(),
            WorkWorkspaceKind::Task => {
                format!("s.id IN ({})", super::super::task_work::session_ids("?3"))
            }
        };
        let belongs: bool = conn.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM agent_sessions s JOIN repository_plans r ON r.repo=coalesce(s.repo,(SELECT repo FROM waves WHERE id=s.wave_id)) WHERE r.id=?1 AND s.id=?2 AND ({scope}) AND ?3 IS NOT NULL)"),
            params![work.repository,session_id,work.id], |row| row.get(0),
        )?;
        if !belongs {
            return Err(invalid("Session is unavailable in this repository"));
        }
    }
    Ok(())
}

impl SqliteStore {
    pub fn pane_layout(&self, work: &WorkWorkspaceKey) -> StoreResult<Option<WorkPaneLayout>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        validate_owner(&snapshot, work)?;
        layout_in(&snapshot, work)
    }

    /// Zero creates intent; subsequent writes require the last observed revision.
    pub fn save_pane_layout(&self, layout: &WorkPaneLayout) -> StoreResult<WorkPaneLayout> {
        layout.validate().map_err(invalid)?;
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
        validate_owner(&tx, &layout.work)?;
        let prior = layout_in(&tx, &layout.work)?;
        if prior.as_ref().map_or(0, |prior| prior.revision) != layout.revision {
            return Err(invalid("pane layout changed; read it again before editing"));
        }
        validate_sessions(&tx, &layout.work, &layout.tree)?;
        let mut saved = layout.clone();
        saved.revision += 1;
        tx.execute("INSERT INTO work_pane_layouts(repository_id,kind,work_id,layout_json) VALUES(?1,?2,?3,?4) ON CONFLICT(repository_id,kind,work_id) DO UPDATE SET layout_json=excluded.layout_json",
            params![saved.work.repository,kind_name(&saved.work.kind),saved.work.id,serde_json::to_string(&saved)?])?;
        tx.commit()?;
        Ok(saved)
    }
}
