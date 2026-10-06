//! Current Project selection has one owner: the Wave row. YAML is import evidence only.
use rusqlite::{params, Connection, OptionalExtension};

use super::SqliteStore;
use crate::id::WaveId;
use crate::store::{PlanningLocks, StoreError, StoreResult};

pub(super) fn read_in(conn: &Connection, wave: &WaveId) -> StoreResult<Option<String>> {
    Ok(conn.query_row(
        "SELECT p.external_project_id FROM waves w LEFT JOIN projects p ON p.id=w.current_project_id WHERE w.id=?1",
        [wave], |row| row.get(0),
    )?)
}

pub(crate) fn read_project_binding(
    store: &SqliteStore,
    wave: &WaveId,
) -> StoreResult<Option<String>> {
    let conn = store.conn.lock().expect("store mutex poisoned");
    read_in(&conn, wave)
}

fn write_in(
    conn: &Connection,
    wave: &WaveId,
    expected: Option<&str>,
    project: &str,
) -> StoreResult<()> {
    if read_in(conn, wave)?.as_deref() != expected {
        return Err(StoreError::InvalidAuthority(
            "Project selection changed; preserve the intervening decision".into(),
        ));
    }
    let id: Option<String> = conn
        .query_row(
            "SELECT id FROM projects WHERE wave_id=?1 AND external_project_id=?2",
            params![wave, project],
            |row| row.get(0),
        )
        .optional()?;
    let id = id.ok_or_else(|| {
        StoreError::InvalidAuthority(
            "Project selection requires accepted facts owned by this Wave".into(),
        )
    })?;
    conn.execute(
        "UPDATE waves SET current_project_id=?2 WHERE id=?1 AND current_project_id IS NOT ?2",
        params![wave, id],
    )?;
    Ok(())
}

/// Callers hold the Wave guard through reconciliation; compare and replace in one transaction.
pub(crate) fn write_project_binding(
    store: &SqliteStore,
    wave: &WaveId,
    expected: Option<&str>,
    project: &str,
    _guard: &PlanningLocks,
) -> StoreResult<()> {
    let mut conn = store.conn.lock().expect("store mutex poisoned");
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    write_in(&tx, wave, expected, project)?;
    tx.commit()?;
    Ok(())
}

impl SqliteStore {
    pub(crate) fn finish_project_creation(
        &self,
        wave: &WaveId,
        expected: Option<&str>,
        project: &str,
        _guard: &PlanningLocks,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        write_in(&tx, wave, expected, project)?;
        let updated = tx.execute("UPDATE project_transitions SET settled_at=COALESCE(settled_at,unixepoch()) WHERE wave_id=?1 AND successor_id=?2 AND predecessor_id IS NULL AND reset_name IS NULL", params![wave,project])?;
        if updated != 1 {
            return Err(StoreError::InvalidAuthority(
                "Project creation reservation is missing".into(),
            ));
        }
        tx.commit()?;
        Ok(())
    }

    pub(crate) fn record_project_activation(
        &self,
        wave: &WaveId,
        exec: Option<&crate::id::ExecId>,
    ) -> StoreResult<()> {
        let Some(exec) = exec else {
            return Ok(());
        };
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE waves SET project_activation_exec_id=?2 WHERE id=?1",
            params![wave, exec],
        )?;
        Ok(())
    }

    pub(crate) fn project_binding_imported(&self, wave: &WaveId) -> StoreResult<bool> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM project_binding_imports WHERE wave_id=?1)",
            [wave],
            |row| row.get(0),
        )?)
    }

    /// Preserve original bytes and select their exact accepted identity atomically.
    pub(crate) fn import_project_binding(
        &self,
        wave: &WaveId,
        original: Option<&str>,
        project: Option<&str>,
        _guard: &PlanningLocks,
    ) -> StoreResult<()> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let imported: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM project_binding_imports WHERE wave_id=?1)",
            [wave],
            |row| row.get(0),
        )?;
        if !imported {
            if let Some(project) = project {
                let selected = read_in(&tx, wave)?;
                if selected.as_deref().is_some_and(|id| id != project) {
                    return Err(StoreError::InvalidAuthority(
                        "YAML import conflicts with the Wave's selected Project".into(),
                    ));
                }
                write_in(&tx, wave, selected.as_deref(), project)?;
            }
            tx.execute("INSERT INTO project_binding_imports(wave_id,original_yaml,imported_at) VALUES(?1,?2,unixepoch())", params![wave, original])?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectReadinessState {
    Unconfigured,
    Unavailable,
    Inactive,
    Ready,
    Terminal,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectReadiness {
    pub state: ProjectReadinessState,
    pub project_id: Option<String>,
    pub observed_at: Option<i64>,
    pub pending_successor: Option<String>,
    pub activation: Option<ProjectActivation>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectActivation {
    pub exec_id: String,
    pub completed_at: Option<i64>,
    pub outcome: Option<String>,
    pub error: Option<String>,
}

impl SqliteStore {
    /// Partial ingestion can confirm one Project before a full inventory exists.
    pub(crate) fn accepted_projects(
        &self,
        wave: &WaveId,
    ) -> StoreResult<Vec<crate::pm::PmProject>> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        let mut query = conn.prepare("SELECT f.body FROM projects p JOIN waves w ON w.id=p.wave_id
            JOIN pm_projects f ON f.repo=w.repo AND f.provider='linear' AND f.id=p.external_project_id
            JOIN pm_wave_projects m ON m.wave_id=w.id AND m.project_id=f.id
            LEFT JOIN pm_wave_sync s ON s.wave_id=w.id
            WHERE w.id=?1 AND f.archived=0 AND f.membership_unresolved=0
              AND p.pm_snapshot_synced_at=f.observed_at
              AND (s.wave_id IS NULL OR EXISTS(SELECT 1 FROM json_each(f.body,'$.initiative_ids') WHERE value=s.initiative))
            ORDER BY m.position,f.id")?;
        let rows = query.query_map([wave], |row| row.get::<_, String>(0))?;
        rows.map(|row| Ok(serde_json::from_str(&row?)?)).collect()
    }

    pub(crate) fn project_readiness(&self, wave: &WaveId) -> StoreResult<ProjectReadiness> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        Ok(conn.query_row(
            "SELECT p.external_project_id, f.observed_at,
             CASE WHEN w.current_project_id IS NULL THEN 'unconfigured'
                  WHEN f.id IS NULL OR f.archived OR f.membership_unresolved OR p.pm_snapshot_synced_at!=f.observed_at
                    OR NOT EXISTS(SELECT 1 FROM pm_wave_projects m WHERE m.wave_id=w.id AND m.project_id=f.id)
                    OR (s.wave_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM json_each(f.body,'$.initiative_ids') WHERE value=s.initiative))
                    THEN 'unavailable'
                  WHEN p.status IN ('completed','canceled') THEN 'terminal'
                  WHEN p.status='started' THEN 'ready' ELSE 'inactive' END,
             (SELECT successor_id FROM project_transitions WHERE wave_id=w.id AND settled_at IS NULL),
             e.id,e.completed_at,e.outcome,e.error
             FROM waves w LEFT JOIN projects p ON p.id=w.current_project_id
             LEFT JOIN pm_wave_sync s ON s.wave_id=w.id
             LEFT JOIN pm_projects f ON f.repo=w.repo AND f.provider=COALESCE(s.provider,'linear') AND f.id=p.external_project_id
             LEFT JOIN execs e ON e.id=w.project_activation_exec_id WHERE w.id=?1",
            [wave], |row| {
                let state: String = row.get(2)?;
                let exec: Option<String> = row.get(4)?;
                Ok(ProjectReadiness {
                    project_id: row.get(0)?, observed_at: row.get(1)?,
                    state: match state.as_str() {
                        "unconfigured" => ProjectReadinessState::Unconfigured,
                        "unavailable" => ProjectReadinessState::Unavailable,
                        "terminal" => ProjectReadinessState::Terminal,
                        "ready" => ProjectReadinessState::Ready,
                        _ => ProjectReadinessState::Inactive,
                    },
                    pending_successor: row.get(3)?,
                    activation: exec.map(|exec_id| Ok::<_,rusqlite::Error>(ProjectActivation {
                        exec_id, completed_at: row.get(5)?, outcome: row.get(6)?, error: row.get(7)?,
                    })).transpose()?,
                })
            },
        )?)
    }
}

#[cfg(test)]
mod tests {
    use super::{read_project_binding, write_project_binding, ProjectReadinessState};
    use crate::id::WaveId;
    use crate::store::{sqlite::SqliteStore, PlanningLocks};
    use crate::work::wave::Wave;

    #[test]
    fn selection_import_is_once_and_only_committed_selection_wakes_readers() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("loopflow.db")).unwrap();
        let wave = Wave::new(WaveId::new(), "a".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        let guard = PlanningLocks::new(tempfile::tempfile().unwrap());
        {
            let conn = store.conn.lock().unwrap();
            for (id, provider) in [
                ("one", "11111111-1111-4111-8111-111111111111"),
                ("two", "22222222-2222-4222-8222-222222222222"),
            ] {
                conn.execute("INSERT INTO projects(id,wave_id,external_project_id,created_at) VALUES(?1,?2,?3,1)", rusqlite::params![id,wave.id(),provider]).unwrap();
            }
        }
        let first = "11111111-1111-4111-8111-111111111111";
        let second = "22222222-2222-4222-8222-222222222222";
        let original = format!("# preserve me\npm:\n  linear_project: {first}\n");
        let before = store.revisions().unwrap();
        assert_eq!(
            store.project_readiness(wave.id()).unwrap().state,
            ProjectReadinessState::Unconfigured
        );
        assert_eq!(store.revisions().unwrap(), before);
        store
            .import_project_binding(wave.id(), Some(&original), Some(first), &guard)
            .unwrap();
        assert!(store.revisions().unwrap().planning > before.planning);
        write_project_binding(&store, wave.id(), Some(first), second, &guard).unwrap();
        let selected = store.revisions().unwrap();
        store
            .import_project_binding(wave.id(), Some(&original), Some(first), &guard)
            .unwrap();
        assert!(write_project_binding(&store, wave.id(), Some(first), first, &guard).is_err());
        assert_eq!(
            read_project_binding(&store, wave.id()).unwrap().as_deref(),
            Some(second)
        );
        assert_eq!(store.revisions().unwrap(), selected);
        // No accepted provider facts: a selected ID alone cannot prove readiness.
        assert_eq!(
            store.project_readiness(wave.id()).unwrap().state,
            ProjectReadinessState::Unavailable
        );
        let saved: String = store
            .conn
            .lock()
            .unwrap()
            .query_row(
                "SELECT original_yaml FROM project_binding_imports WHERE wave_id=?1",
                [wave.id()],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(saved, original);
    }
}

#[cfg(test)]
mod activation_tests {
    use crate::id::{ExecId, TraceId, WaveId};
    use crate::store::sqlite::SqliteStore;
    use crate::work::wave::Wave;

    #[test]
    fn readiness_retains_exact_unknown_and_failed_activation_outcomes() {
        let dir = tempfile::tempdir().unwrap();
        let store = SqliteStore::open_ephemeral(&dir.path().join("loopflow.db")).unwrap();
        let wave = Wave::new(WaveId::new(), "a".into(), "/repo".into());
        store.create_wave(&wave).unwrap();
        let exec = ExecId::new();
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO execs(id,trace_id,cwd,started_at) VALUES(?1,?2,'/repo',1)",
                rusqlite::params![exec, TraceId::new()],
            )
            .unwrap();
        store
            .record_project_activation(wave.id(), Some(&exec))
            .unwrap();
        let pending = store
            .project_readiness(wave.id())
            .unwrap()
            .activation
            .unwrap();
        assert_eq!(pending.exec_id, exec.as_str());
        assert!(pending.completed_at.is_none());
        assert!(pending.outcome.is_none());
        store.conn.lock().unwrap().execute("UPDATE execs SET completed_at=2,outcome='failed',error='provider unavailable' WHERE id=?1", [&exec]).unwrap();
        let failed = store
            .project_readiness(wave.id())
            .unwrap()
            .activation
            .unwrap();
        assert_eq!(failed.exec_id, exec.as_str());
        assert_eq!(failed.outcome.as_deref(), Some("failed"));
        assert_eq!(failed.error.as_deref(), Some("provider unavailable"));
    }
}
