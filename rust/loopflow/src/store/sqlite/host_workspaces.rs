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
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let snapshot = conn.savepoint()?;
        let ids = {
            let mut statement = snapshot.prepare("SELECT DISTINCT h.id FROM host_workspaces h JOIN host_work_occurrences o ON o.workspace_id=h.id WHERE h.repository_id=?1 AND h.host=?2 AND h.server=?3 AND json_extract(o.work_json,'$.kind')=?4 AND json_extract(o.work_json,'$.id')=?5 ORDER BY h.updated_at DESC")?;
            let kind = serde_json::to_value(&work.kind)?;
            let rows = statement
                .query_map(
                    params![work.repository, host, server, kind.as_str(), work.id],
                    |row| row.get::<_, String>(0),
                )?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };
        ids.iter()
            .filter_map(|id| read_workspace(&snapshot, id).transpose())
            .collect()
    }
    pub fn host_workspace(&self, id: &str) -> StoreResult<Option<HostWorkspace>> {
        read_workspace(&self.conn.lock().expect("store mutex poisoned"), id)
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
        let row = conn.query_row(
            "SELECT workspace_id,work_json,destination_id,observation,updated_at FROM host_work_occurrences WHERE id=?1",
            [id],
            |row| Ok((row.get(0)?, row.get::<_, String>(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        ).optional()?;
        row.map(
            |(workspace_id, work, destination_id, observation, updated_at)| {
                Ok(HostOccurrence {
                    id: id.into(),
                    workspace_id,
                    work: serde_json::from_str(&work)?,
                    destination_id,
                    observation,
                    updated_at,
                })
            },
        )
        .transpose()
    }

    pub fn save_host_occurrence(&self, occurrence: &HostOccurrence) -> StoreResult<()> {
        self.conn.lock().expect("store mutex poisoned").execute(
            "INSERT INTO host_work_occurrences(id,workspace_id,work_json,destination_id,observation,updated_at) VALUES(?1,?2,?3,?4,?5,?6) ON CONFLICT(id) DO UPDATE SET destination_id=excluded.destination_id,observation=excluded.observation,updated_at=excluded.updated_at",
            params![occurrence.id,occurrence.workspace_id,serde_json::to_string(&occurrence.work)?,occurrence.destination_id,occurrence.observation,occurrence.updated_at],
        )?;
        Ok(())
    }
}

fn read_workspace(conn: &rusqlite::Connection, id: &str) -> StoreResult<Option<HostWorkspace>> {
    let row = conn.query_row(
            "SELECT id,repository_id,host,server,selection_json,copy_name,container_id,observation,updated_at FROM host_workspaces WHERE id=?1", [id],
            |row| Ok((row.get::<_,String>(0)?,row.get(1)?,row.get(2)?,row.get(3)?,row.get::<_,String>(4)?,row.get(5)?,row.get(6)?,row.get(7)?,row.get(8)?)),
        ).optional()?;
    row.map(
        |(
            id,
            repository_id,
            host,
            server,
            selection,
            copy_name,
            container_id,
            observation,
            updated_at,
        )| {
            Ok(HostWorkspace {
                id,
                repository_id,
                host,
                server,
                selection: serde_json::from_str(&selection)?,
                copy_name,
                container_id,
                observation,
                updated_at,
            })
        },
    )
    .transpose()
}
