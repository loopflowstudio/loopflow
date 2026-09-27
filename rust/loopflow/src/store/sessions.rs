use crate::durable::{FlowPosition, RunId, TaskId};
use crate::engine::invocation::QueuedInvocation;
use crate::engine::ExecutionCursor;
use crate::session::{Run, Session, TitleSource};

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn position_runs(
        &self,
        invocation: &str,
        node: u32,
        iterations: &[Vec<u32>],
    ) -> StoreResult<Vec<Run>> {
        let invocation = invocation.to_owned();
        let iterations = iterations.to_vec();
        run_sqlite(&self.sqlite, move |store| {
            store.position_runs(&invocation, node, &iterations)
        })
        .await
    }

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

    pub async fn run(&self, id: &RunId) -> StoreResult<Option<Run>> {
        let id = id.clone();
        run_sqlite(&self.sqlite, move |store| store.run(&id)).await
    }

    pub async fn create_session(
        &self,
        session: Session,
        run: Run,
        review: Option<(QueuedInvocation, ExecutionCursor)>,
    ) -> StoreResult<(Session, Run)> {
        run_sqlite(&self.sqlite, move |store| {
            let review = review
                .as_ref()
                .map(|(invocation, cursor)| (invocation, cursor));
            store.create_session(session, run, review)
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

    pub async fn bind_session(&self, id: &str, task: &TaskId) -> StoreResult<(Session, Run)> {
        let id = id.to_string();
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| store.bind_session(&id, &task)).await
    }

    pub async fn task_runs(&self, task: &TaskId) -> StoreResult<Vec<Run>> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| store.task_runs(&task)).await
    }

    pub async fn open_sessions(&self) -> StoreResult<Vec<(Session, Run)>> {
        run_sqlite(&self.sqlite, |store| store.open_sessions()).await
    }

    pub async fn waiting_flow(&self, session_id: &str) -> StoreResult<Option<(String, String)>> {
        let session_id = session_id.to_string();
        run_sqlite(&self.sqlite, move |store| store.waiting_flow(&session_id)).await
    }

    pub async fn end_flow(&self, invocation: &str) -> StoreResult<()> {
        let invocation = invocation.to_string();
        run_sqlite(&self.sqlite, move |store| store.end_flow(&invocation)).await
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
