use crate::durable::{FlowSession, RunId, TaskId};
use crate::session::{AgentSession, Run, TitleSource};

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn reserve_review_run(
        &self,
        expected: &FlowSession,
    ) -> StoreResult<(FlowSession, Run)> {
        let expected = expected.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_review_run(&expected)
        })
        .await
    }
    pub async fn session(&self, id: &str) -> StoreResult<Option<(AgentSession, Run)>> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.session(&id)).await
    }

    pub async fn session_for_run(
        &self,
        run_id: &RunId,
    ) -> StoreResult<Option<(AgentSession, Run)>> {
        let run_id = run_id.clone();
        run_sqlite(&self.sqlite, move |store| store.session_for_run(&run_id)).await
    }

    pub async fn run(&self, id: &RunId) -> StoreResult<Option<Run>> {
        let id = id.clone();
        run_sqlite(&self.sqlite, move |store| store.run(&id)).await
    }

    pub async fn create_run(&self, run: Run) -> StoreResult<Run> {
        run_sqlite(&self.sqlite, move |store| store.create_run(run)).await
    }

    pub async fn create_session(
        &self,
        session: AgentSession,
        run: Run,
        review: Option<FlowSession>,
    ) -> StoreResult<(AgentSession, Run)> {
        run_sqlite(&self.sqlite, move |store| {
            store.create_session(session, run, review.as_ref())
        })
        .await
    }

    pub async fn replace_session_run(&self, expected_run: &RunId, run: Run) -> StoreResult<Run> {
        let expected_run = expected_run.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.replace_session_run(&expected_run, run)
        })
        .await
    }

    pub async fn fill_run_provider(
        &self,
        run: &RunId,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let run = run.clone();
        let provider = provider.to_string();
        let model = model.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.fill_run_provider(&run, &provider, model.as_deref())
        })
        .await
    }

    pub async fn retarget_unpublished_run(
        &self,
        run: &RunId,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let run = run.clone();
        let provider = provider.to_string();
        let model = model.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.retarget_unpublished_run(&run, &provider, model.as_deref())
        })
        .await
    }

    pub async fn bind_session(&self, id: &str, task: &TaskId) -> StoreResult<(AgentSession, Run)> {
        let id = id.to_string();
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| store.bind_session(&id, &task)).await
    }

    pub async fn runs(
        &self,
        wave: Option<&str>,
        project: Option<&str>,
        task: Option<&str>,
        caller: Option<&str>,
        since: i64,
    ) -> StoreResult<Vec<super::sqlite::ListedRun>> {
        let [wave, project, task, caller] =
            [wave, project, task, caller].map(|name| name.map(str::to_string));
        run_sqlite(&self.sqlite, move |store| {
            store.runs(
                wave.as_deref(),
                project.as_deref(),
                task.as_deref(),
                caller.as_deref(),
                since,
            )
        })
        .await
    }

    pub async fn sessions(
        &self,
        filter: &crate::session::SessionFilter,
    ) -> StoreResult<Vec<(AgentSession, Run)>> {
        let filter = filter.clone();
        run_sqlite(&self.sqlite, move |store| store.sessions(&filter)).await
    }

    pub async fn waiting_flow(&self, session_id: &str) -> StoreResult<Option<(String, String)>> {
        let session_id = session_id.to_string();
        run_sqlite(&self.sqlite, move |store| store.waiting_flow(&session_id)).await
    }

    pub async fn complete_session(&self, id: &str, expected_run: &RunId) -> StoreResult<()> {
        let id = id.to_string();
        let expected_run = expected_run.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.complete_session(&id, &expected_run)
        })
        .await
    }

    pub async fn session_runs(&self, id: &str) -> StoreResult<Vec<Run>> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.session_runs(&id)).await
    }

    pub async fn rename_session(
        &self,
        id: &str,
        expected_run: Option<&RunId>,
        title: &str,
        source: TitleSource,
    ) -> StoreResult<()> {
        let id = id.to_string();
        let expected_run = expected_run.cloned();
        let title = title.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.rename_session(&id, expected_run.as_ref(), &title, source)
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
