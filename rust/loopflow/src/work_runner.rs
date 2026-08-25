use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::durable::WorkRef;
use crate::store::{open_existing_store, SharedStore};

/// Run one Project planning controller or one legacy Task body.
pub async fn run_work(work: WorkRef) -> Result<()> {
    let store: SharedStore = Arc::new(
        open_existing_store()
            .await
            .ok_or_else(|| anyhow!("no Loopflow registry on this machine"))?,
    );
    match work {
        WorkRef::Project(id) => crate::project::runner::run(store, id).await,
        WorkRef::Task(id) => crate::task::runner::run(store, id).await,
        WorkRef::Wave(id) => anyhow::bail!("Wave {id} is hosted by its resident, not a child body"),
    }
}
