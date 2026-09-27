use crate::durable::{FlowPosition, RunId};
use crate::session::{Run, Session, TitleSource};

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn reserve_review_run(
        &self,
        expected: &FlowPosition,
    ) -> StoreResult<(FlowPosition, Run)> {
        let expected = expected.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_review_run(&expected)
        })
        .await
    }
    pub async fn session(&self, id: &str) -> StoreResult<Option<(Session, Run)>> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.session(&id)).await
    }

    pub async fn session_for_run(&self, run_id: &RunId) -> StoreResult<Option<(Session, Run)>> {
        let run_id = run_id.clone();
        run_sqlite(&self.sqlite, move |store| store.session_for_run(&run_id)).await
    }

    pub async fn open_review_sessions(&self) -> StoreResult<Vec<(Session, Run)>> {
        run_sqlite(&self.sqlite, |store| store.open_review_sessions()).await
    }

    pub async fn session_run_ids(&self) -> StoreResult<Vec<RunId>> {
        run_sqlite(&self.sqlite, |store| store.session_run_ids()).await
    }

    pub async fn session_runs(&self, id: &str) -> StoreResult<Vec<Run>> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.session_runs(&id)).await
    }

    pub async fn rename_session(
        &self,
        id: &str,
        title: &str,
        source: TitleSource,
    ) -> StoreResult<()> {
        let id = id.to_string();
        let title = title.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.rename_session(&id, &title, source)
        })
        .await
    }

    pub async fn ready_session(
        &self,
        id: &str,
        expected_run: &RunId,
        summary: &str,
    ) -> StoreResult<()> {
        let id = id.to_string();
        let expected_run = expected_run.clone();
        let summary = summary.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.ready_session(&id, &expected_run, &summary)
        })
        .await
    }
}
