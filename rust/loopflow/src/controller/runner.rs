use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::durable::WorkRef;
use crate::store::{open_existing_store, SharedStore};

use super::WorkStartupAttempt;

/// Run one Project or Task end-to-end controller.
pub async fn run_work(work: WorkRef, startup: Option<WorkStartupAttempt>) -> Result<()> {
    let store: SharedStore = Arc::new(
        open_existing_store()
            .await
            .ok_or_else(|| anyhow!("no Loopflow registry on this machine"))?,
    );
    match work {
        WorkRef::Project(id) => crate::controller::project::run(store, id, startup).await,
        WorkRef::Task(id) => crate::controller::task::run(store, id, startup).await,
        WorkRef::Wave(id) => anyhow::bail!("Wave {id} is hosted by its resident, not a child body"),
    }
}
