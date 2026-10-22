use std::num::NonZeroU32;

use crate::id::LfProcessId;
use crate::process::{LfProcess, LfProcessCursor, LfProcessFilter, LfProcessPage};

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn process(&self, id: &LfProcessId) -> StoreResult<Option<LfProcess>> {
        let id = id.clone();
        run_sqlite(&self.sqlite, move |store| store.process(&id)).await
    }

    pub async fn resolve_process(&self, selector: &str) -> StoreResult<Option<LfProcess>> {
        let selector = selector.to_string();
        run_sqlite(&self.sqlite, move |store| store.resolve_process(&selector)).await
    }

    pub async fn processes(
        &self,
        filter: &LfProcessFilter,
        after: Option<&LfProcessCursor>,
        limit: NonZeroU32,
    ) -> StoreResult<LfProcessPage> {
        let filter = filter.clone();
        let after = after.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.processes(&filter, after.as_ref(), limit)
        })
        .await
    }
}
