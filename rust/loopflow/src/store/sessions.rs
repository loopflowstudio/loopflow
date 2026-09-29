use crate::durable::{FlowSession, TaskId};
use crate::session::{AgentSession, TitleSource};

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub(crate) async fn session_summaries(
        &self,
        filter: &crate::session::SessionFilter,
    ) -> StoreResult<Vec<crate::session::SessionSummary>> {
        let filter = filter.clone();
        run_sqlite(&self.sqlite, move |store| store.session_summaries(&filter)).await
    }

    pub(crate) async fn input_final_answer(
        &self,
        input: &str,
    ) -> StoreResult<Option<crate::run_record::FinalAnswer>> {
        let input = input.to_owned();
        run_sqlite(&self.sqlite, move |store| store.input_final_answer(&input)).await
    }

    pub(crate) async fn historical_session_inputs(
        &self,
    ) -> StoreResult<Vec<(Option<AgentSession>, crate::session::SessionObservation)>> {
        run_sqlite(&self.sqlite, |store| store.historical_session_inputs()).await
    }

    pub(crate) async fn import_session(
        &self,
        session: AgentSession,
        review: Option<FlowSession>,
        history: Vec<crate::session::SessionObservation>,
        dry_run: bool,
    ) -> StoreResult<bool> {
        run_sqlite(&self.sqlite, move |store| {
            store.import_session(session, review.as_ref(), &history, dry_run)
        })
        .await
    }
    pub async fn agent_work(
        &self,
        exec: &crate::id::ExecId,
    ) -> StoreResult<Option<crate::session::RunWork>> {
        let exec = exec.clone();
        run_sqlite(&self.sqlite, move |store| store.agent_work(&exec)).await
    }
    pub async fn reserve_review_run(
        &self,
        expected: &FlowSession,
    ) -> StoreResult<(FlowSession, AgentSession)> {
        let expected = expected.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_review_run(&expected)
        })
        .await
    }
    pub async fn session(&self, id: &str) -> StoreResult<Option<AgentSession>> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.session(&id)).await
    }

    pub async fn session_for_artifact(&self, run_id: &str) -> StoreResult<Option<AgentSession>> {
        let run_id = run_id.to_owned();
        run_sqlite(&self.sqlite, move |store| {
            store.session_for_artifact(&run_id)
        })
        .await
    }

    pub async fn create_session(
        &self,
        session: AgentSession,
        review: Option<FlowSession>,
    ) -> StoreResult<AgentSession> {
        let caller = crate::journal::current_exec_id();
        run_sqlite(&self.sqlite, move |store| {
            store.create_session(session, review.as_ref(), caller.as_ref())
        })
        .await
    }

    pub async fn replace_session_input(
        &self,
        expected_capture: Option<i64>,
        session: AgentSession,
    ) -> StoreResult<AgentSession> {
        run_sqlite(&self.sqlite, move |store| {
            store.replace_session_input(expected_capture, session)
        })
        .await
    }

    pub async fn fill_run_provider(
        &self,
        run: &str,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let run = run.to_owned();
        let provider = provider.to_string();
        let model = model.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.fill_run_provider(&run, &provider, model.as_deref())
        })
        .await
    }

    pub async fn retarget_unpublished_run(
        &self,
        run: &str,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let run = run.to_owned();
        let provider = provider.to_string();
        let model = model.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.retarget_unpublished_run(&run, &provider, model.as_deref())
        })
        .await
    }

    pub async fn bind_session(
        &self,
        id: &str,
        expected_capture: Option<i64>,
        task: &TaskId,
    ) -> StoreResult<AgentSession> {
        let id = id.to_string();
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.bind_session(&id, expected_capture, &task)
        })
        .await
    }

    pub(crate) async fn conversation_history(
        &self,
        since: i64,
    ) -> StoreResult<Vec<crate::run_record::SessionHistory>> {
        run_sqlite(&self.sqlite, move |store| {
            store.conversation_history(None, None, None, None, since, true)
        })
        .await
    }

    pub async fn sessions(
        &self,
        filter: &crate::session::SessionFilter,
    ) -> StoreResult<Vec<AgentSession>> {
        let filter = filter.clone();
        run_sqlite(&self.sqlite, move |store| store.sessions(&filter)).await
    }

    pub async fn waiting_flow(&self, session_id: &str) -> StoreResult<Option<(String, String)>> {
        let session_id = session_id.to_string();
        run_sqlite(&self.sqlite, move |store| store.waiting_flow(&session_id)).await
    }

    pub async fn complete_session(
        &self,
        id: &str,
        expected_capture: Option<i64>,
    ) -> StoreResult<()> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.complete_session(&id, expected_capture)
        })
        .await
    }

    pub async fn session_inputs(&self, id: &str) -> StoreResult<Vec<String>> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.session_inputs(&id)).await
    }

    pub async fn rename_session(
        &self,
        id: &str,
        expected_capture: Option<i64>,
        title: &str,
        source: TitleSource,
    ) -> StoreResult<()> {
        let id = id.to_string();
        let title = title.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.rename_session(&id, expected_capture, &title, source)
        })
        .await
    }

    pub async fn ready_session(
        &self,
        id: &str,
        expected_capture: Option<i64>,
        summary: &str,
    ) -> StoreResult<()> {
        let id = id.to_string();
        let summary = summary.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.ready_session(&id, expected_capture, &summary)
        })
        .await
    }
}
