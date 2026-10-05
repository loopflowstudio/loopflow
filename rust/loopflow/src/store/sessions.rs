use crate::durable::{FlowSession, TaskId};
use crate::session::{AgentSession, TitleSource};

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub(crate) async fn resume_candidates(&self) -> StoreResult<Vec<(AgentSession, Option<i64>)>> {
        run_sqlite(&self.sqlite, |store| store.resume_candidates()).await
    }
    pub(crate) async fn session_summaries(
        &self,
        filter: &crate::session::SessionFilter,
    ) -> StoreResult<Vec<crate::session::SessionSummary>> {
        let filter = filter.clone();
        run_sqlite(&self.sqlite, move |store| store.session_summaries(&filter)).await
    }

    pub async fn reserve_review_capture(
        &self,
        expected: &FlowSession,
    ) -> StoreResult<(FlowSession, AgentSession)> {
        let expected = expected.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.reserve_review_capture(&expected)
        })
        .await
    }
    pub async fn session(&self, id: &str) -> StoreResult<Option<AgentSession>> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.session(&id)).await
    }

    pub async fn session_for_artifact(
        &self,
        artifact_key: &str,
    ) -> StoreResult<Option<AgentSession>> {
        let artifact_key = artifact_key.to_owned();
        run_sqlite(&self.sqlite, move |store| {
            store.session_for_artifact(&artifact_key)
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

    pub async fn ensure_primary_session(
        &self,
        scope: &crate::session::PrimaryScope,
        replacing: Option<&str>,
        session: AgentSession,
    ) -> StoreResult<AgentSession> {
        let scope = scope.clone();
        let replacing = replacing.map(str::to_string);
        let caller = crate::journal::current_exec_id();
        run_sqlite(&self.sqlite, move |store| {
            store.ensure_primary_session(&scope, replacing.as_deref(), session, caller.as_ref())
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

    pub async fn retarget_unpublished_capture(
        &self,
        artifact_key: &str,
        provider: &str,
        model: Option<&str>,
    ) -> StoreResult<()> {
        let artifact_key = artifact_key.to_owned();
        let provider = provider.to_string();
        let model = model.map(str::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.retarget_unpublished_capture(&artifact_key, &provider, model.as_deref())
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
