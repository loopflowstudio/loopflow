//! Captured Linear input for preservation and delayed-observation fixtures.
#![allow(dead_code)]
use loopflow::id::WaveId;
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::{PlanningLocks, Store, StoreResult};
use loopflow_storage::planning::ProjectInventory;
use std::sync::Arc;

pub fn task_history_inventory() -> ProjectInventory {
    serde_json::from_str(include_str!(
        "../../../../tests/fixtures/dto/task_history_planning.json"
    ))
    .unwrap()
}

/// One acquired Linear inventory. Input to ingestion, never a local Work reading.
#[derive(Debug, Clone, PartialEq)]
pub struct InventoryFixture {
    pub wave_id: WaveId,
    pub synced_at: i64,
    pub snapshot: ProjectInventory,
}

/// A planning observation; neither Project ownership nor execution is required.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IssueFixture {
    pub item: loopflow_storage::planning::TrackerItem,
    pub project: Option<loopflow_storage::planning::TrackerProject>,
    pub observed_at: i64,
}

#[allow(async_fn_in_trait)]
pub trait SeedLinear {
    async fn seed_inventory(
        &self,
        input: InventoryFixture,
        locks: Option<Arc<PlanningLocks>>,
    ) -> StoreResult<()>;
    async fn seed_issue(
        &self,
        repo: &str,
        tracker: &str,
        input: IssueFixture,
        locks: Option<Arc<PlanningLocks>>,
    ) -> StoreResult<()>;
}
impl SeedLinear for Store {
    async fn seed_inventory(
        &self,
        input: InventoryFixture,
        locks: Option<Arc<PlanningLocks>>,
    ) -> StoreResult<()> {
        loopflow::linear::events::accept_linear_inventory(
            self,
            input.wave_id,
            input.snapshot,
            input.synced_at,
            locks,
        )
        .await
    }
    async fn seed_issue(
        &self,
        repo: &str,
        tracker: &str,
        input: IssueFixture,
        locks: Option<Arc<PlanningLocks>>,
    ) -> StoreResult<()> {
        assert_eq!(tracker, "linear");
        loopflow::linear::events::accept_linear_issue(
            self,
            repo,
            input.item,
            input.project,
            input.observed_at,
            locks,
        )
        .await
    }
}
pub trait SeedLinearSqlite {
    fn seed_inventory(&self, input: &InventoryFixture) -> StoreResult<()>;
    fn seed_issue(&self, repo: &str, tracker: &str, input: &IssueFixture) -> StoreResult<()>;
}
impl SeedLinearSqlite for SqliteStore {
    fn seed_inventory(&self, input: &InventoryFixture) -> StoreResult<()> {
        loopflow::store::sqlite::linear_events::accept_linear_inventory_in(
            self,
            &input.wave_id,
            &input.snapshot,
            input.synced_at,
        )
    }
    fn seed_issue(&self, repo: &str, tracker: &str, input: &IssueFixture) -> StoreResult<()> {
        assert_eq!(tracker, "linear");
        loopflow::store::sqlite::linear_events::accept_linear_issue_in(
            self,
            repo,
            &input.item,
            input.project.as_ref(),
            input.observed_at,
            loopflow_storage::planning_sync::TrackerKind::Linear,
        )
    }
}
