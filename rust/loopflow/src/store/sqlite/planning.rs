use rusqlite::{params, Connection, OptionalExtension};

use crate::id::WaveId;
use crate::pm::{PmItem, PmProject, PmSnapshot};
use crate::store::sqlite::SqliteStore;
use crate::store::{PmSnapshotRow, PmTaskRecord, StoreError, StoreResult};

impl SqliteStore {
    pub fn put_pm_task(
        &self,
        repo: &str,
        provider: &str,
        record: &PmTaskRecord,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        if let Some(project) = &record.project {
            put_project(&tx, repo, provider, record.observed_at, project)?;
        }
        let mut item = record.item.clone();
        // Detail queries do not observe relative list order.
        if let Some(rank) = tx.query_row(
            "SELECT json_extract(body,'$.rank') FROM pm_items WHERE repo=?1 AND provider=?2 AND id=?3 AND project_id IS ?4",
            params![repo,provider,item.id,item.project_id], |row| row.get::<_,u32>(0),
        ).optional()? {
            item.rank = rank;
        }
        put_item(&tx, repo, provider, record.observed_at, &item)?;
        tx.execute("UPDATE pm_items SET needs_refresh=0 WHERE repo=?1 AND provider=?2 AND id=?3 AND observed_at=?4",
            params![repo,provider,record.item.id,record.observed_at])?;
        tx.commit()?;
        Ok(())
    }

    pub fn invalidate_pm_task(
        &self,
        repo: &str,
        provider: &str,
        selector: &str,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        // A null detail response invalidates cached admission, not provider history.
        // Only a complete detail observation can repair it; list omission is ambiguous.
        conn.execute("UPDATE pm_items SET needs_refresh=1 WHERE repo=?1 AND provider=?2 AND (id=?3 OR identifier=?3)",
            params![repo,provider,selector])?;
        Ok(())
    }

    pub fn pm_task(
        &self,
        repo: &str,
        provider: &str,
        selector: &str,
    ) -> StoreResult<Option<PmTaskRecord>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare(
            "SELECT i.body,p.body,i.observed_at FROM pm_items i LEFT JOIN pm_projects p
             ON p.repo=i.repo AND p.provider=i.provider AND p.id=i.project_id
             WHERE i.repo=?1 AND i.provider=?2 AND (i.id=?3 OR i.identifier=?3) AND i.needs_refresh=0
             AND NOT EXISTS(SELECT 1 FROM task_deletions d JOIN waves w ON w.id=d.wave_id
                            WHERE w.repo=i.repo AND d.issue_id=i.id)",
        )?;
        let rows = query
            .query_map(params![repo, provider, selector], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if rows.len() > 1 {
            return Err(StoreError::InvalidData(format!(
                "ambiguous planning selector {selector:?}; use a Task UUID"
            )));
        }
        let Some((item, project, observed_at)) = rows.into_iter().next() else {
            return Ok(None);
        };
        let mut item: PmItem = serde_json::from_str(&item)?;
        let project: Option<PmProject> = project
            .map(|body| serde_json::from_str(&body))
            .transpose()?;
        if let Some(project) = &project {
            item.project = Some(project.slug.clone());
        }
        Ok(Some(PmTaskRecord {
            item,
            project,
            observed_at,
        }))
    }

    pub fn put_pm_snapshot(&self, snapshot: &PmSnapshotRow) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction()?;
        let repo: String = tx.query_row(
            "SELECT repo FROM waves WHERE id=?1",
            [snapshot.wave_id.as_str()],
            |row| row.get(0),
        )?;
        let previous: Option<i64> = tx
            .query_row(
                "SELECT synced_at FROM pm_wave_sync WHERE wave_id=?1",
                [snapshot.wave_id.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        if previous.is_some_and(|time| time > snapshot.synced_at) {
            return Ok(());
        }
        for project in &snapshot.snapshot.projects {
            put_project(&tx, &repo, &snapshot.provider, snapshot.synced_at, project)?;
        }
        for item in &snapshot.snapshot.items {
            put_item(&tx, &repo, &snapshot.provider, snapshot.synced_at, item)?;
        }
        tx.execute(
            "INSERT INTO pm_wave_sync(wave_id,provider,initiative,synced_at) VALUES(?1,?2,?3,?4)
             ON CONFLICT(wave_id) DO UPDATE SET provider=excluded.provider,
             initiative=excluded.initiative,synced_at=excluded.synced_at",
            params![
                snapshot.wave_id,
                snapshot.provider,
                snapshot.initiative,
                snapshot.synced_at
            ],
        )?;
        tx.execute(
            "DELETE FROM pm_wave_projects WHERE wave_id=?1",
            [snapshot.wave_id.as_str()],
        )?;
        for (position, project) in snapshot.snapshot.projects.iter().enumerate() {
            tx.execute(
                "INSERT INTO pm_wave_projects(wave_id,project_id,position) VALUES(?1,?2,?3)",
                params![snapshot.wave_id, project.id, position as i64],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn pm_snapshot(&self, wave_id: &WaveId) -> StoreResult<Option<PmSnapshotRow>> {
        let mut connection = self.conn.lock().expect("store mutex poisoned");
        let transaction = connection.transaction()?;
        let conn = &transaction;
        let metadata = conn
            .query_row(
                "SELECT w.repo,s.provider,s.initiative,s.synced_at FROM pm_wave_sync s
             JOIN waves w ON w.id=s.wave_id WHERE s.wave_id=?1",
                [wave_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?;
        let Some((repo, provider, initiative, synced_at)) = metadata else {
            return Ok(None);
        };
        let mut query = conn.prepare(
            "SELECT p.body FROM pm_wave_projects m JOIN pm_projects p ON p.id=m.project_id
             WHERE m.wave_id=?1 AND p.repo=?2 AND p.provider=?3 ORDER BY m.position",
        )?;
        let projects = query
            .query_map(params![wave_id, repo, provider], |row| {
                row.get::<_, String>(0)
            })?
            .map(|row| Ok(serde_json::from_str::<PmProject>(&row?)?))
            .collect::<StoreResult<Vec<_>>>()?;
        let mut query = conn.prepare(
            "SELECT i.body FROM pm_items i JOIN pm_wave_projects m ON m.project_id=i.project_id
             WHERE m.wave_id=?1 AND i.repo=?2 AND i.provider=?3 AND i.needs_refresh=0
             AND NOT EXISTS(SELECT 1 FROM task_deletions d JOIN waves w ON w.id=d.wave_id
                            WHERE w.repo=i.repo AND d.issue_id=i.id)
             ORDER BY m.position,json_extract(i.body,'$.rank'),i.id",
        )?;
        let mut items = query
            .query_map(params![wave_id, repo, provider], |row| {
                row.get::<_, String>(0)
            })?
            .map(|row| Ok(serde_json::from_str::<PmItem>(&row?)?))
            .collect::<StoreResult<Vec<_>>>()?;
        for item in &mut items {
            if let Some(project) = projects
                .iter()
                .find(|project| Some(project.id.as_str()) == item.project_id.as_deref())
            {
                item.project = Some(project.slug.clone());
            }
        }
        Ok(Some(PmSnapshotRow {
            wave_id: wave_id.clone(),
            provider,
            initiative,
            synced_at,
            snapshot: PmSnapshot { projects, items },
        }))
    }
}

fn put_project(
    conn: &Connection,
    repo: &str,
    provider: &str,
    observed_at: i64,
    project: &PmProject,
) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO pm_projects(repo,provider,id,observed_at,body) VALUES(?1,?2,?3,?4,?5)
         ON CONFLICT(repo,provider,id) DO UPDATE SET observed_at=excluded.observed_at,body=excluded.body
         WHERE excluded.observed_at >= pm_projects.observed_at",
        params![repo,provider,project.id,observed_at,serde_json::to_string(project)?],
    )?;
    Ok(())
}

fn put_item(
    conn: &Connection,
    repo: &str,
    provider: &str,
    observed_at: i64,
    item: &PmItem,
) -> StoreResult<()> {
    conn.execute(
        "INSERT INTO pm_items(repo,provider,id,identifier,project_id,observed_at,body) VALUES(?1,?2,?3,?4,?5,?6,?7)
         ON CONFLICT(repo,provider,id) DO UPDATE SET identifier=excluded.identifier,
         project_id=excluded.project_id,observed_at=excluded.observed_at,body=excluded.body
         WHERE excluded.observed_at >= pm_items.observed_at",
        params![repo,provider,item.id,item.identifier,item.project_id,observed_at,serde_json::to_string(item)?],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use rusqlite::{params, Connection};
    use serde_json::json;

    use crate::id::WaveId;
    use crate::store::{open_ephemeral_store, StorageConfig};

    #[tokio::test]
    async fn migration_preserves_planning_identity_and_removes_snapshot_storage() {
        let directory = tempfile::tempdir().unwrap();
        let database = directory.path().join("planning.db");
        let wave = WaveId::new();
        let snapshot = json!({"projects":[{
            "id":"project", "slug":"chapter", "name":"Chapter", "summary":"Proof",
            "metric_targets":[], "flows":{"recommended":null}, "krs":[],
            "initiative_ids":["initiative"], "team_ids":["team"]
        }],"items":[{
            "id":"issue", "identifier":"FIX-1", "url":null, "name":"Retained title",
            "description":"Retained notes", "rank":2, "completed":false, "state":"unstarted",
            "project_id":"project", "project":"chapter", "team_id":"team", "assignee":null
        }]});
        {
            let conn = Connection::open(&database).unwrap();
            crate::store::migrations::apply_sqlite(&conn).unwrap();
            conn.execute(
                "INSERT INTO waves(id,name,repo,created_at) VALUES(?1,'product','/repo',1)",
                [wave.as_str()],
            )
            .unwrap();
            conn.execute("INSERT INTO pm_snapshots(wave_id,provider,initiative,synced_at,payload) VALUES(?1,'linear','initiative',42,?2)",params![wave,snapshot.to_string()]).unwrap();
        }
        let store = open_ephemeral_store(&StorageConfig::sqlite(database.clone()))
            .await
            .unwrap();
        let detail = store
            .pm_task("/repo", "linear", "fix-1")
            .await
            .unwrap()
            .unwrap();
        let list = store.pm_snapshot(&wave).await.unwrap().unwrap();
        assert_eq!(detail.observed_at, 42);
        assert_eq!(detail.item, list.snapshot.items[0]);
        assert_eq!(detail.item.name, "Retained title");
        assert_eq!(detail.item.description, "Retained notes");
        assert_eq!(detail.project.as_ref(), Some(&list.snapshot.projects[0]));
        assert_eq!(store.get_wave(&wave).await.unwrap().unwrap().id(), &wave);
        assert!(store.list_tasks(None).await.unwrap().is_empty());
        assert!(!Connection::open(database)
            .unwrap()
            .prepare("SELECT payload FROM pm_snapshots")
            .is_ok());

        // A delayed list cannot resurrect a confirmed removal in either reader.
        store
            .confirm_task_deletion(&wave, "issue", "FIX-1")
            .await
            .unwrap();
        store.put_pm_snapshot(list).await.unwrap();
        assert!(store
            .pm_task("/repo", "linear", "FIX-1")
            .await
            .unwrap()
            .is_none());
        assert!(store
            .pm_snapshot(&wave)
            .await
            .unwrap()
            .unwrap()
            .snapshot
            .items
            .is_empty());
    }
}
