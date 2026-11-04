//! Presentation occurrences. A host's inventory is evidence, never execution authority.
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};

use super::SqliteStore;
use crate::store::StoreResult;
use crate::work::selection::{Selection, WorkWorkspaceKey};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostWorkspace {
    pub id: String,
    pub repository_id: String,
    pub host: String,
    pub server: String,
    pub selection: Selection,
    pub copy_name: Option<String>,
    pub container_id: Option<String>,
    pub observation: String,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostOccurrence {
    pub id: String,
    pub workspace_id: String,
    pub work: WorkWorkspaceKey,
    pub destination_id: Option<String>,
    pub observation: String,
    pub updated_at: i64,
}

impl SqliteStore {
    pub fn work_host_workspaces(
        &self,
        work: &WorkWorkspaceKey,
        host: &str,
        server: &str,
    ) -> StoreResult<Vec<HostWorkspace>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut statement = conn.prepare(
            "SELECT DISTINCT h.id,h.repository_id,h.host,h.server,h.selection_json,h.copy_name,h.container_id,h.observation,h.updated_at
             FROM host_workspaces h JOIN host_work_occurrences o ON o.workspace_id=h.id
             WHERE h.repository_id=?1 AND h.host=?2 AND h.server=?3
               AND json_extract(o.work_json,'$.kind')=?4 AND json_extract(o.work_json,'$.id')=?5
             ORDER BY h.updated_at DESC",
        )?;
        let kind = serde_json::to_value(&work.kind)?;
        let rows = statement.query_map(
            params![work.repository, host, server, kind.as_str(), work.id],
            |row| Ok(map_workspace(row)),
        )?;
        rows.map(|row| row?).collect()
    }
    pub fn host_workspace(&self, id: &str) -> StoreResult<Option<HostWorkspace>> {
        self.conn.lock().expect("store mutex poisoned").query_row(
            "SELECT id,repository_id,host,server,selection_json,copy_name,container_id,observation,updated_at FROM host_workspaces WHERE id=?1",
            [id],
            |row| Ok(map_workspace(row)),
        ).optional()?.transpose()
    }

    pub fn save_host_workspace(&self, workspace: &HostWorkspace) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "INSERT INTO host_workspaces(id,repository_id,host,server,selection_json,copy_name,container_id,observation,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(id) DO UPDATE SET container_id=excluded.container_id,observation=excluded.observation,updated_at=excluded.updated_at",
            params![workspace.id,workspace.repository_id,workspace.host,workspace.server,serde_json::to_string(&workspace.selection)?,workspace.copy_name,workspace.container_id,workspace.observation,workspace.updated_at],
        )?;
        Ok(())
    }

    pub fn host_occurrence(&self, id: &str) -> StoreResult<Option<HostOccurrence>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.query_row(
            "SELECT workspace_id,work_json,destination_id,observation,updated_at FROM host_work_occurrences WHERE id=?1",
            [id],
            |row| Ok(map_occurrence(row, id)),
        ).optional()?.transpose()
    }

    pub fn save_host_occurrence(&self, occurrence: &HostOccurrence) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "INSERT INTO host_work_occurrences(id,workspace_id,work_json,destination_id,observation,updated_at) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET destination_id=excluded.destination_id,observation=excluded.observation,updated_at=excluded.updated_at",
            params![occurrence.id,occurrence.workspace_id,serde_json::to_string(&occurrence.work)?,occurrence.destination_id,occurrence.observation,occurrence.updated_at],
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
        observation: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn map_occurrence(row: &rusqlite::Row<'_>, id: &str) -> StoreResult<HostOccurrence> {
    Ok(HostOccurrence {
        id: id.into(),
        workspace_id: row.get(0)?,
        work: serde_json::from_str(&row.get::<_, String>(1)?)?,
        destination_id: row.get(2)?,
        observation: row.get(3)?,
        updated_at: row.get(4)?,
    })
}
