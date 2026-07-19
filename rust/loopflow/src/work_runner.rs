use std::sync::Arc;

use anyhow::{anyhow, Result};

use crate::durable::WorkRef;
use crate::store::{open_existing_store, SharedStore};

/// Run one Project or Task body under the exact ambient Run lease.
pub async fn run_work(work: WorkRef) -> Result<()> {
    let store: SharedStore = Arc::new(
        open_existing_store()
            .await
            .ok_or_else(|| anyhow!("no Loopflow registry on this machine"))?,
    );
    let lease = crate::ops::required_run_lease(&store).await?;
    if lease.work != work {
        anyhow::bail!(
            "ambient Run lease owns {} {}, not {} {}",
            lease.work.kind(),
            lease.work.id(),
            work.kind(),
            work.id(),
        );
    }
    match work {
        WorkRef::Project(id) => crate::project::runner::run(store, id, &lease).await,
        WorkRef::Task(id) => crate::task::runner::run(store, id, &lease).await,
        WorkRef::Wave(id) => anyhow::bail!("Wave {id} is hosted by its resident, not a child body"),
    }
}
