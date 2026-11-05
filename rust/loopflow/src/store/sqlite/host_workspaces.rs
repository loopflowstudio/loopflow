//! Native locators and creation receipts, never host liveness or layout authority.
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use rusqlite::{params, OptionalExtension};

use super::SqliteStore;
use crate::durable::TaskId;
use crate::store::StoreResult;
use crate::work::selection::Selection;

#[derive(Debug, Clone)]
pub struct HostWorkspace {
    pub id: String,
    pub repository_id: String,
    pub host: String,
    pub server: String,
    pub selection: Selection,
    pub copy_name: Option<String>,
    pub container_id: Option<String>,
    pub creation_pending: bool,
    pub updated_at: i64,
}

/// How far native creation got. A pending state whose reply was lost fences retries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostCreation {
    Pending,
    /// A first-start arrival showing only its Task view.
    Passive,
    ToolsPending,
    Complete,
}

impl ToSql for HostCreation {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(match self {
            Self::Pending => "pending",
            Self::Passive => "passive",
            Self::ToolsPending => "tools_pending",
            Self::Complete => "complete",
        }
        .into())
    }
}

impl FromSql for HostCreation {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        match value.as_str()? {
            "pending" => Ok(Self::Pending),
            "passive" => Ok(Self::Passive),
            "tools_pending" => Ok(Self::ToolsPending),
            "complete" => Ok(Self::Complete),
            _ => Err(FromSqlError::InvalidType),
        }
    }
}

#[derive(Debug)]
pub struct HostOccurrence {
    pub id: String,
    pub workspace_id: String,
    pub task_id: TaskId,
    pub destination_id: Option<String>,
    pub creation: HostCreation,
}

impl SqliteStore {
    pub fn task_host_workspaces(
        &self,
        task: &TaskId,
        host: &str,
        server: &str,
    ) -> StoreResult<Vec<HostWorkspace>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT DISTINCT h.id,h.repository_id,h.host,h.server,h.selection_json,h.copy_name,h.container_id,h.creation_pending,h.updated_at
             FROM host_workspaces h JOIN host_work_occurrences o ON o.workspace_id=h.id
             WHERE h.host=?1 AND h.server=?2 AND o.task_id=?3
             ORDER BY h.updated_at DESC",
        )?;
        let rows = statement.query_map(params![host, server, task.as_str()], |row| {
            Ok(map_workspace(row))
        })?;
        rows.map(|row| row?).collect()
    }
    /// Every recorded occurrence of a Task with its workspace, across hosts,
    /// servers and named copies.
    pub fn task_host_occurrences(
        &self,
        task: &TaskId,
    ) -> StoreResult<Vec<(HostWorkspace, HostOccurrence)>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT h.id,h.repository_id,h.host,h.server,h.selection_json,h.copy_name,h.container_id,h.creation_pending,h.updated_at,
                    o.id,o.destination_id,o.creation
             FROM host_work_occurrences o JOIN host_workspaces h ON h.id=o.workspace_id
             WHERE o.task_id=?1 ORDER BY o.id",
        )?;
        let rows = statement.query_map([task.as_str()], |row| {
            let occurrence = HostOccurrence {
                id: row.get(9)?,
                workspace_id: row.get(0)?,
                task_id: task.clone(),
                destination_id: row.get(10)?,
                creation: row.get(11)?,
            };
            Ok(map_workspace(row).map(|workspace| (workspace, occurrence)))
        })?;
        rows.map(|row| row?).collect()
    }

    pub fn host_workspace(&self, id: &str) -> StoreResult<Option<HostWorkspace>> {
        self.conn.lock().expect("store mutex poisoned").query_row(
            "SELECT id,repository_id,host,server,selection_json,copy_name,container_id,creation_pending,updated_at FROM host_workspaces WHERE id=?1",
            [id],
            |row| Ok(map_workspace(row)),
        ).optional()?.transpose()
    }

    pub fn save_host_workspace(&self, workspace: &HostWorkspace) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "INSERT INTO host_workspaces(id,repository_id,host,server,selection_json,copy_name,container_id,creation_pending,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(id) DO UPDATE SET container_id=excluded.container_id,creation_pending=excluded.creation_pending,updated_at=excluded.updated_at",
            params![workspace.id,workspace.repository_id,workspace.host,workspace.server,serde_json::to_string(&workspace.selection)?,workspace.copy_name,workspace.container_id,workspace.creation_pending,workspace.updated_at],
        )?;
        Ok(())
    }

    pub fn host_occurrence(&self, id: &str) -> StoreResult<Option<HostOccurrence>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT workspace_id,task_id,destination_id,creation FROM host_work_occurrences WHERE id=?1",
            [id],
            |row| Ok(HostOccurrence {
                id: id.into(),
                workspace_id: row.get(0)?,
                task_id: TaskId::from_raw(row.get::<_, String>(1)?),
                destination_id: row.get(2)?,
                creation: row.get(3)?,
            }),
        ).optional()?)
    }

    pub fn save_host_occurrence(&self, occurrence: &HostOccurrence) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "INSERT INTO host_work_occurrences(id,workspace_id,task_id,destination_id,creation) VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET destination_id=excluded.destination_id,creation=excluded.creation",
            params![occurrence.id,occurrence.workspace_id,occurrence.task_id.as_str(),occurrence.destination_id,occurrence.creation],
        )?;
        Ok(())
    }
}

fn map_workspace(row: &rusqlite::Row<'_>) -> StoreResult<HostWorkspace> {
    Ok(HostWorkspace {
        id: row.get(0)?,
        repository_id: row.get(1)?,
        host: row.get(2)?,
        server: row.get(3)?,
        selection: serde_json::from_str(&row.get::<_, String>(4)?)?,
        copy_name: row.get(5)?,
        container_id: row.get(6)?,
        creation_pending: row.get(7)?,
        updated_at: row.get(8)?,
    })
}
