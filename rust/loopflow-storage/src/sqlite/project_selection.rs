//! Current Project selection has one owner: the Wave row. YAML is import evidence only.
use crate::work::edit::WorkValue as Edit;
use rusqlite::{params, Connection, OptionalExtension};

use super::planning_write;
use super::SqliteStore;
use crate::graph_process::{Execution, ExecutionResult};
use crate::work::project::Project;
use crate::{StoreError, StoreResult};
use loopflow_model::durable::ProjectId;
use loopflow_model::id::WaveId;

pub(super) fn read_in(conn: &Connection, wave: &WaveId) -> StoreResult<Option<String>> {
    Ok(conn.query_row(
        "SELECT COALESCE(p.external_project_id,p.id) FROM waves w LEFT JOIN projects p ON p.id=w.current_project_id WHERE w.id=?1",
        [wave], |row| row.get(0),
    )?)
}

pub fn read_project_binding(store: &SqliteStore, wave: &WaveId) -> StoreResult<Option<String>> {
    let conn = store.conn.lock().expect("store mutex poisoned");
    read_in(&conn, wave)
}

// Selection accepts exact saved identities; display names and slugs never select a plan.
fn resolve_project_id(conn: &Connection, wave: &WaveId, project: &str) -> StoreResult<ProjectId> {
    let canonical = loopflow_model::id::canonical_prefixed(
        &loopflow_model::id::canonical_uuid(project),
        "proj_",
    );
    let project = canonical.as_str();
    let id: Option<String> = conn
        .query_row(
            "SELECT id FROM projects WHERE wave_id=?1 AND (id=?2 OR external_project_id=?2)",
            params![wave, project],
            |row| row.get(0),
        )
        .optional()?;
    id.map(ProjectId::from_raw).ok_or_else(|| {
        StoreError::InvalidAuthority("Project ID has no saved record in this Wave".into())
    })
}

// Initial binding and one-time import share the same identity decision.
fn bind_in(conn: &Connection, wave: &WaveId, project: &str) -> StoreResult<ProjectId> {
    let id = resolve_project_id(conn, wave, project)?;
    let selected: Option<String> = conn.query_row(
        "SELECT current_project_id FROM waves WHERE id=?1",
        [wave],
        |row| row.get(0),
    )?;
    if selected
        .as_deref()
        .is_some_and(|selected| selected != id.as_str())
    {
        return Err(StoreError::InvalidAuthority(
            "Wave already has a different configured Project".into(),
        ));
    }
    planning_write::local(
        conn,
        &crate::work::edit::WorkEdit::for_target(
            wave.as_str(),
            &[Edit::WaveProject(Some(id.to_string()))],
        ),
    )?;
    Ok(id)
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
    let id = resolve_project_id(conn, wave, project)?;
    planning_write::local(
        conn,
        &crate::work::edit::WorkEdit::for_target(
            wave.as_str(),
            &[Edit::WaveProject(Some(id.to_string()))],
        ),
    )?;
    Ok(())
}

/// Callers hold the Wave guard through reconciliation; compare and replace in one transaction.
#[cfg(any(test, feature = "test-support"))]
pub fn write_project_binding(
    store: &SqliteStore,
    wave: &WaveId,
    expected: Option<&str>,
    project: &str,
    _guard: &crate::PlanningLocks,
) -> StoreResult<()> {
    let mut conn = store.conn.lock().expect("store mutex poisoned");
    let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
    write_in(&tx, wave, expected, project)?;
    tx.commit()?;
    Ok(())
}

impl SqliteStore {
    pub fn bind_project(&self, wave: &WaveId, project: &str) -> StoreResult<Project> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let project = bind_in(&tx, wave, project)?;
        let candidate = super::children::project_in(&tx, &project)?;
        match readiness_in(&tx, wave, Execution::result)?.state {
            ProjectReadinessState::Unavailable | ProjectReadinessState::Terminal => return Err(StoreError::InvalidAuthority(
                "Project is terminal or has unresolved provider evidence; selection is unchanged".into())),
            _ => {}
        }
        tx.commit()?;
        Ok(candidate)
    }

    pub fn ensure_project(&self, wave: &WaveId, name: &str) -> StoreResult<ProjectId> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let selected = read_in(&tx, wave)?;
        let pending: Option<(String, Option<String>, Option<String>)> = tx
            .query_row(
                "SELECT successor_id,predecessor_id,reset_name FROM project_transitions
             WHERE wave_id=?1 AND settled_at IS NULL",
                [wave],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        if pending.as_ref().is_some_and(|(id, predecessor, reset)| {
            predecessor.is_some()
                || reset.is_some()
                || selected.as_ref().is_some_and(|selected| selected != id)
        }) {
            return Err(StoreError::InvalidAuthority(
                "an unfinished Project transition owns selection; resume its original request"
                    .into(),
            ));
        }
        let now = crate::rows::now_unix();
        let id = if selected.is_some() {
            if readiness_in(&tx, wave, Execution::result)?.state
                == ProjectReadinessState::Unavailable
            {
                return Err(StoreError::InvalidAuthority(
                    "selected Project has unresolved provider evidence".into(),
                ));
            }
            tx.query_row(
                "SELECT current_project_id FROM waves WHERE id=?1",
                [wave],
                |row| row.get::<_, String>(0),
            )?
        } else {
            let id = match &pending {
                Some((id, _, _)) => {
                    let uuid = uuid::Uuid::parse_str(id.strip_prefix("proj_").unwrap_or(id))
                        .map_err(|error| StoreError::InvalidData(error.to_string()))?;
                    ProjectId::from_raw(format!("proj_{}", uuid.simple()))
                }
                None => ProjectId::new(),
            };
            // A retained mapping may already own an interrupted creation's identity.
            let existing: Option<String> = tx.query_row(
                "SELECT id FROM projects WHERE wave_id=?1 AND (id=?2 OR external_project_id=?3)",
                params![wave,id.as_str(),pending.as_ref().map(|p| &p.0)], |row| row.get(0),
            ).optional()?;
            if let Some(existing) = existing {
                existing
            } else {
                planning_write::create(
                    &tx,
                    "",
                    &crate::work::edit::WorkEdit::for_target(
                        id.as_str(),
                        &[
                            Edit::ProjectWave(wave.to_string()),
                            Edit::ProjectSlug(Some(name.into())),
                            Edit::ProjectName(Some(name.into())),
                            Edit::ProjectStatus(crate::work::project::ProjectStatus::Started),
                        ],
                    ),
                    crate::sqlite::planning_write::WriteOrigin::Local,
                )?;
                if pending.is_none() {
                    tx.execute("INSERT INTO project_transitions(wave_id,successor_id,created_at,local_plan_json)
                        VALUES(?1,?2,?3,?4)",params![wave,id.as_str(),now,serde_json::json!({"name":name}).to_string()])?;
                }
                id.to_string()
            }
        };
        let id = ProjectId::from_raw(id);
        let project = super::children::project_in(&tx, &id)?;
        if !matches!(
            project.status,
            crate::work::project::ProjectStatus::Backlog
                | crate::work::project::ProjectStatus::Planned
                | crate::work::project::ProjectStatus::Started
        ) {
            return Err(StoreError::InvalidAuthority(
                "terminal or paused Project history cannot be activated".into(),
            ));
        }
        planning_write::local(
            &tx,
            &crate::work::edit::WorkEdit::for_target(
                id.as_str(),
                &[Edit::ProjectStatus(
                    crate::work::project::ProjectStatus::Started,
                )],
            ),
        )?;
        write_in(&tx, wave, selected.as_deref(), id.as_str())?;
        if readiness_in(&tx, wave, Execution::result)?.state == ProjectReadinessState::Unavailable {
            return Err(StoreError::InvalidAuthority(
                "Project has unresolved provider evidence; activation is unchanged".into(),
            ));
        }
        tx.execute("UPDATE project_transitions SET settled_at=COALESCE(settled_at,?3)
            WHERE wave_id=?1 AND successor_id=?2 AND predecessor_id IS NULL AND reset_name IS NULL AND settled_at IS NULL",
            params![wave,pending.as_ref().map_or(id.as_str(),|p|p.0.as_str()),now])?;
        tx.commit()?;
        Ok(id)
    }

    /// The lf activating `wave`'s Project, as it starts: a new activation
    /// replaces the last one's end.
    pub fn record_project_activation(
        &self,
        wave: &WaveId,
        process: &loopflow_model::id::LfProcessId,
        start: &crate::graph_process::ExecutionStart,
    ) -> StoreResult<()> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        conn.execute(
            "UPDATE waves SET project_activation_lf_process_id=?2,project_activation_started_at=?3,
                project_activation_pid=?4,project_activation_os_started_at=?5,
                project_activation_ended_at=NULL,project_activation_exit_code=NULL,
                project_activation_error=NULL
             WHERE id=?1",
            params![
                wave,
                process,
                start.started_at,
                start.pid,
                start.os_started_at
            ],
        )?;
        Ok(())
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ProjectReadinessState {
    Unconfigured,
    Unavailable,
    Inactive,
    Ready,
    Terminal,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ProjectReadiness {
    pub state: ProjectReadinessState,
    pub project_id: Option<String>,
    pub observed_at: Option<i64>,
    pub pending_successor: Option<String>,
    pub activation: Option<ProjectActivation>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ProjectActivation {
    pub lf_process_id: String,
    pub completed_at: Option<i64>,
    pub outcome: Option<String>,
    pub error: Option<String>,
}

impl ProjectActivation {
    fn of(run: Execution, result: ExecutionResult) -> Self {
        Self {
            lf_process_id: run.id.as_str().to_owned(),
            completed_at: run.ended_at,
            outcome: result.outcome().map(str::to_owned),
            error: run.error,
        }
    }
}

/// How a reader sees a recorded run's result; liveness lives above the store.
pub type ObserveResult = fn(&Execution) -> ExecutionResult;

impl SqliteStore {
    pub fn selected_project(&self, wave: &WaveId) -> StoreResult<Option<Project>> {
        let mut conn = self.conn.lock().expect("store mutex poisoned");
        let tx = conn.savepoint()?;
        let readiness = readiness_in(&tx, wave, Execution::result)?;
        match readiness.state {
            ProjectReadinessState::Unconfigured => return Ok(None),
            ProjectReadinessState::Unavailable => {
                return Err(StoreError::InvalidData(format!(
                    "Wave {wave} selected Project is unavailable; inspect its saved fields and retained provider evidence"
                )));
            }
            _ => {}
        }
        let id: String = tx.query_row(
            "SELECT current_project_id FROM waves WHERE id=?1",
            [wave],
            |row| row.get(0),
        )?;
        let project = super::children::project_in(&tx, &ProjectId::from_raw(id))?;
        tx.commit()?;
        Ok(Some(project))
    }

    /// `wave`'s Project readiness, its activation's result read by `observe`.
    pub fn project_readiness(
        &self,
        wave: &WaveId,
        observe: ObserveResult,
    ) -> StoreResult<ProjectReadiness> {
        let conn = self.conn.lock().expect("store mutex poisoned");
        readiness_in(&conn, wave, observe)
    }
}

// Saved planning supplies values and age; retained provider evidence can invalidate
// selection, but absent inventory does not erase an owned Project. A peer can
// supply an accepted entity without claiming a complete provider membership list.
fn readiness_in(
    conn: &Connection,
    wave: &WaveId,
    observe: ObserveResult,
) -> StoreResult<ProjectReadiness> {
    Ok(conn.query_row(
            "SELECT p.id, p.pm_snapshot_synced_at,
             CASE WHEN w.current_project_id IS NULL THEN 'unconfigured'
                  WHEN p.id IS NULL OR p.project_name IS NULL OR p.project_slug IS NULL OR p.description IS NULL THEN 'unavailable'
                  WHEN f.id IS NOT NULL AND NOT json_valid(f.initiative_ids) THEN 'unavailable'
                  WHEN f.id IS NOT NULL AND (f.archived OR f.membership_unresolved OR p.pm_snapshot_synced_at IS NOT f.observed_at
                    OR (s.wave_id IS NOT NULL AND NOT EXISTS(SELECT 1 FROM json_each(f.initiative_ids) WHERE value=s.initiative)))
                    THEN 'unavailable'
                  WHEN p.status IN ('completed','canceled') THEN 'terminal'
                  WHEN p.status='started' THEN 'ready' ELSE 'inactive' END,
             (SELECT successor_id FROM project_transitions WHERE wave_id=w.id AND settled_at IS NULL),
             w.project_activation_lf_process_id,w.project_activation_started_at,
             w.project_activation_ended_at,w.project_activation_pid,
             w.project_activation_os_started_at,w.project_activation_exit_code,
             w.project_activation_error
             FROM waves w LEFT JOIN projects p ON p.id=w.current_project_id
             LEFT JOIN tracker_wave_sync s ON s.wave_id=w.id
             LEFT JOIN tracker_projects f ON f.repo=w.repo AND f.tracker=COALESCE(s.tracker,'linear') AND f.id=p.external_project_id
             WHERE w.id=?1",
            [wave], |row| {
                let state: String = row.get(2)?;
                let process: Option<loopflow_model::id::LfProcessId> = row.get(4)?;
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
                    activation: process.map(|id| {
                        Ok::<_, rusqlite::Error>(Execution {
                            id,
                            started_at: row.get::<_, Option<i64>>(5)?.unwrap_or_default(),
                            ended_at: row.get(6)?,
                            pid: row.get(7)?,
                            os_started_at: row.get(8)?,
                            exit_code: row.get(9)?,
                            error: row.get(10)?,
                        })
                    }).transpose()?.map(|run| {
                        let result = observe(&run);
                        ProjectActivation::of(run, result)
                    }),
                })
            },
        )?)
}

#[cfg(test)]
mod tests {
    use super::{read_project_binding, write_project_binding, ProjectReadinessState};
    use crate::work::wave::Wave;
    use crate::PlanningLocks;
    use loopflow_model::id::WaveId;

    #[test]
    fn project_creation_rolls_back_and_reuses_retained_identity() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("loopflow.db")).unwrap();
        let wave = Wave::new(WaveId::new(), "product".into(), "/repo".into());
        crate::test_fixture::wave(&store, &wave);
        let reserved = "11111111-1111-4111-8111-111111111111";
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO project_transitions(wave_id,successor_id,created_at) VALUES(?1,?2,1)",
                rusqlite::params![wave.id(), reserved],
            )
            .unwrap();
        store.conn.lock().unwrap().execute_batch("CREATE TRIGGER reject_selection BEFORE UPDATE OF current_project_id ON waves BEGIN SELECT RAISE(ABORT,'fixture failure'); END;").unwrap();
        let before = store.revisions().unwrap();
        assert!(store.ensure_project(wave.id(), "Original name").is_err());
        assert!(store.list_projects(Some(wave.id())).unwrap().is_empty());
        assert_eq!(store.revisions().unwrap(), before);
        store
            .conn
            .lock()
            .unwrap()
            .execute_batch("DROP TRIGGER reject_selection")
            .unwrap();
        let id = store.ensure_project(wave.id(), "Original name").unwrap();
        assert_eq!(id.as_str(), "proj_11111111111141118111111111111111");
        store
            .edit_project(&id, Some("Later edit"), None, None)
            .unwrap();
        let changes = store.pending_project_changes(&id).unwrap();
        let before = store.revisions().unwrap();
        assert_eq!(
            store.ensure_project(wave.id(), "Original name").unwrap(),
            id
        );
        assert_eq!(store.revisions().unwrap(), before);
        assert_eq!(store.pending_project_changes(&id).unwrap(), changes);
        assert_eq!(store.project(&id).unwrap().unwrap().name, "Later edit");
        let receipt: (String,bool) = store.conn.lock().unwrap().query_row("SELECT successor_id,settled_at IS NOT NULL FROM project_transitions WHERE wave_id=?1",[wave.id()],|row|Ok((row.get(0)?,row.get(1)?))).unwrap();
        assert_eq!(receipt, (reserved.into(), true));
        assert!(store.project(&id).unwrap().unwrap().tracker_id.is_none());
    }

    #[test]
    fn only_a_committed_selection_wakes_readers() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("loopflow.db")).unwrap();
        let wave = Wave::new(WaveId::new(), "a".into(), "/repo".into());
        crate::test_fixture::wave(&store, &wave);
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
        let before = store.revisions().unwrap();
        assert_eq!(
            store
                .project_readiness(wave.id(), crate::graph_process::Execution::result)
                .unwrap()
                .state,
            ProjectReadinessState::Unconfigured
        );
        assert_eq!(store.revisions().unwrap(), before);
        write_project_binding(&store, wave.id(), None, first, &guard).unwrap();
        assert!(store.revisions().unwrap().planning > before.planning);
        write_project_binding(&store, wave.id(), Some(first), second, &guard).unwrap();
        let selected = store.revisions().unwrap();
        assert!(write_project_binding(&store, wave.id(), Some(first), first, &guard).is_err());
        assert_eq!(
            read_project_binding(&store, wave.id()).unwrap().as_deref(),
            Some(second)
        );
        assert_eq!(store.revisions().unwrap(), selected);
        // No accepted provider facts: a selected ID alone cannot prove readiness.
        assert_eq!(
            store
                .project_readiness(wave.id(), crate::graph_process::Execution::result)
                .unwrap()
                .state,
            ProjectReadinessState::Unavailable
        );
    }
}

#[cfg(test)]
mod activation_tests {

    use crate::work::wave::Wave;
    use loopflow_model::id::{LfProcessId, TraceId, WaveId};

    #[test]
    fn an_activation_reads_pending_until_its_lf_writes_its_end() {
        let dir = tempfile::tempdir().unwrap();
        let store = crate::test_fixture::open(&dir.path().join("loopflow.db")).unwrap();
        let wave = Wave::new(WaveId::new(), "a".into(), "/repo".into());
        crate::test_fixture::wave(&store, &wave);
        let process = LfProcessId::new();
        // The journal's row for the activating lf, which `waves` still
        // references until its rebuild drops the foreign key.
        store
            .conn
            .lock()
            .unwrap()
            .execute(
                "INSERT INTO processes(id,trace_id,cwd,started_at) VALUES(?1,?2,'/repo',1)",
                rusqlite::params![process, TraceId::new()],
            )
            .unwrap();
        store
            .record_project_activation(
                wave.id(),
                &process,
                &crate::graph_process::ExecutionStart::reserved(1),
            )
            .unwrap();
        let pending = store
            .project_readiness(wave.id(), crate::graph_process::Execution::result)
            .unwrap()
            .activation
            .unwrap();
        assert_eq!(pending.lf_process_id, process.as_str());
        assert!(pending.completed_at.is_none());
        assert!(pending.outcome.is_none());
        store
            .finish_run(
                crate::graph_process::RunOwner::ProjectActivation,
                &process,
                2,
                Some(1),
                Some("provider unavailable"),
            )
            .unwrap();
        let failed = store
            .project_readiness(wave.id(), crate::graph_process::Execution::result)
            .unwrap()
            .activation
            .unwrap();
        assert_eq!(failed.lf_process_id, process.as_str());
        assert_eq!(failed.outcome.as_deref(), Some("failed"));
        assert_eq!(failed.error.as_deref(), Some("provider unavailable"));
    }
}
