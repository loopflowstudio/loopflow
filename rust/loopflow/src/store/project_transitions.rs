use crate::id::WaveId;

use super::{run_sqlite, Store, StoreResult};

/// Recovery evidence for one explicit Project creation or rotation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ProjectTransition {
    pub wave_id: WaveId,
    pub successor_id: String,
    pub predecessor_id: Option<String>,
    pub reset_name: Option<String>,
    pub create_successor: Option<bool>,
    pub created_at: i64,
    pub settled_at: Option<i64>,
}

impl Store {
    pub(crate) async fn project_transition(
        &self,
        wave: &WaveId,
        successor: &str,
    ) -> StoreResult<Option<ProjectTransition>> {
        let wave = wave.clone();
        let successor = successor.to_owned();
        run_sqlite(&self.sqlite, move |store| {
            store.project_transition(&wave, &successor)
        })
        .await
    }

    pub(crate) async fn project_transition_items(
        &self,
        wave: &WaveId,
        successor: &str,
    ) -> StoreResult<Vec<String>> {
        let wave = wave.clone();
        let successor = successor.to_owned();
        run_sqlite(&self.sqlite, move |store| {
            store.project_transition_items(&wave, &successor)
        })
        .await
    }
}
