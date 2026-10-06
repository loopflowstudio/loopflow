use std::sync::Arc;

use crate::id::WaveId;

use super::{run_planning_write, run_sqlite, PlanningLocks, Store, StoreResult};

/// Recovery evidence for one explicit Project creation or rotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectTransition {
    pub wave_id: WaveId,
    pub successor_id: String,
    pub predecessor_id: Option<String>,
    pub reset_name: Option<String>,
    pub created_at: i64,
    pub settled_at: Option<i64>,
}

impl Store {
    pub(crate) async fn pending_project_transition(
        &self,
        wave: &WaveId,
    ) -> StoreResult<Option<ProjectTransition>> {
        let wave = wave.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.pending_project_transition(&wave)
        })
        .await
    }

    pub(crate) async fn reserve_project_transition(
        &self,
        transition: ProjectTransition,
        acquisition: Arc<PlanningLocks>,
    ) -> StoreResult<()> {
        run_planning_write(&self.sqlite, Some(acquisition), move |store| {
            store.reserve_project_transition(&transition)
        })
        .await
    }

    pub(crate) async fn settle_project_transition(
        &self,
        wave: &WaveId,
        successor: &str,
        acquisition: Arc<PlanningLocks>,
    ) -> StoreResult<()> {
        let wave = wave.clone();
        let successor = successor.to_owned();
        run_planning_write(&self.sqlite, Some(acquisition), move |store| {
            store.settle_project_transition(&wave, &successor)
        })
        .await
    }
}
