//! Captured Linear input for preservation and delayed-observation fixtures.
#![allow(dead_code)]
use loopflow::id::WaveId;
use loopflow::pm::linear::LinearIssue;
use loopflow::pm::linear::LinearProject;
use loopflow::store::sqlite::SqliteStore;
use loopflow::store::{PlanningLocks, Store, StoreResult};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Inventory {
    pub projects: Vec<LinearProject>,
    pub items: Vec<LinearIssue>,
}

/// One acquired Linear inventory. Input to ingestion, never a local Work reading.
#[derive(Debug, Clone, PartialEq)]
pub struct InventoryFixture {
    pub wave_id: WaveId,
    pub provider: String,
    pub initiative: String,
    pub synced_at: i64,
    pub snapshot: Inventory,
}

/// A planning observation; neither Project ownership nor execution is required.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct IssueFixture {
    pub item: loopflow::pm::linear::LinearIssue,
    pub project: Option<loopflow::pm::linear::LinearProject>,
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
        provider: &str,
        input: IssueFixture,
        confirmed: Option<(WaveId, String)>,
        locks: Option<Arc<PlanningLocks>>,
    ) -> StoreResult<()>;
}
impl SeedLinear for Store {
    async fn seed_inventory(
        &self,
        input: InventoryFixture,
        locks: Option<Arc<PlanningLocks>>,
    ) -> StoreResult<()> {
        assert_eq!(input.provider, "linear");
        loopflow::pm::linear::acceptance::accept_linear_inventory(
            self,
            input.wave_id,
            input.initiative,
            input.snapshot.projects,
            input.snapshot.items,
            input.synced_at,
            locks,
        )
        .await
    }
    async fn seed_issue(
        &self,
        repo: &str,
        provider: &str,
        input: IssueFixture,
        confirmed: Option<(WaveId, String)>,
        locks: Option<Arc<PlanningLocks>>,
    ) -> StoreResult<()> {
        assert_eq!(provider, "linear");
        loopflow::pm::linear::acceptance::accept_linear_issue(
            self,
            repo,
            input.item,
            input.project,
            input.observed_at,
            confirmed,
            locks,
        )
        .await
    }
}
pub trait SeedLinearSqlite {
    fn seed_inventory(&self, input: &InventoryFixture) -> StoreResult<()>;
    fn seed_issue(
        &self,
        repo: &str,
        provider: &str,
        input: &IssueFixture,
        confirmed: Option<(&WaveId, &str)>,
    ) -> StoreResult<()>;
}
impl SeedLinearSqlite for SqliteStore {
    fn seed_inventory(&self, input: &InventoryFixture) -> StoreResult<()> {
        assert_eq!(input.provider, "linear");
        loopflow::pm::linear::acceptance::accept_linear_inventory_in(
            self,
            &input.wave_id,
            &input.initiative,
            &input.snapshot.projects,
            &input.snapshot.items,
            input.synced_at,
        )
    }
    fn seed_issue(
        &self,
        repo: &str,
        provider: &str,
        input: &IssueFixture,
        confirmed: Option<(&WaveId, &str)>,
    ) -> StoreResult<()> {
        assert_eq!(provider, "linear");
        loopflow::pm::linear::acceptance::accept_linear_issue_in(
            self,
            repo,
            &input.item,
            input.project.as_ref(),
            input.observed_at,
            confirmed,
        )
    }
}
