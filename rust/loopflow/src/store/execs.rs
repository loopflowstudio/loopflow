use std::num::NonZeroU32;

use crate::exec::{Exec, ExecCursor, ExecFilter, ExecPage};
use crate::id::ExecId;

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn exec(&self, id: &ExecId) -> StoreResult<Option<Exec>> {
        let id = id.clone();
        run_sqlite(&self.sqlite, move |store| store.exec(&id)).await
    }

    pub async fn resolve_exec(&self, selector: &str) -> StoreResult<Option<Exec>> {
        let selector = selector.to_string();
        run_sqlite(&self.sqlite, move |store| store.resolve_exec(&selector)).await
    }

    pub async fn execs(
        &self,
        filter: &ExecFilter,
        after: Option<&ExecCursor>,
        limit: NonZeroU32,
    ) -> StoreResult<ExecPage> {
        let filter = filter.clone();
        let after = after.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.execs(&filter, after.as_ref(), limit)
        })
        .await
    }
}
