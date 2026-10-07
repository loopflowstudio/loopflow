use std::num::NonZeroU32;

use crate::id::ProcessLfid;
use crate::process::{Process, ProcessCursor, ProcessFilter, ProcessPage};

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn process(&self, id: &ProcessLfid) -> StoreResult<Option<Process>> {
        let id = id.clone();
        run_sqlite(&self.sqlite, move |store| store.process(&id)).await
    }

    pub async fn resolve_process(&self, selector: &str) -> StoreResult<Option<Process>> {
        let selector = selector.to_string();
        run_sqlite(&self.sqlite, move |store| store.resolve_process(&selector)).await
    }

    pub async fn processes(
        &self,
        filter: &ProcessFilter,
        after: Option<&ProcessCursor>,
        limit: NonZeroU32,
    ) -> StoreResult<ProcessPage> {
        let filter = filter.clone();
        let after = after.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.processes(&filter, after.as_ref(), limit)
        })
        .await
    }
}
