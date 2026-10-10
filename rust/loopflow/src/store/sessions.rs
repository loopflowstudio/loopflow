use crate::durable::TaskId;
use crate::session::{LfSession, TitleSource};

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub(crate) async fn resume_candidates(&self) -> StoreResult<Vec<(LfSession, Option<i64>)>> {
        run_sqlite(&self.sqlite, |store| store.resume_candidates()).await
    }
    pub(crate) async fn session_summaries(
        &self,
        filter: &crate::session::SessionFilter,
        now: i64,
    ) -> StoreResult<Vec<crate::session::SessionSummary>> {
        let filter = filter.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.session_summaries(&filter, now)
        })
        .await
    }

    pub async fn session(&self, id: &str) -> StoreResult<Option<LfSession>> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.session(&id)).await
    }

    pub async fn session_for_artifact(&self, artifact_key: &str) -> StoreResult<Option<LfSession>> {
        let artifact_key = artifact_key.to_owned();
        run_sqlite(&self.sqlite, move |store| {
            store.session_for_artifact(&artifact_key)
        })
        .await
    }

    pub async fn create_session(&self, session: LfSession) -> StoreResult<LfSession> {
        let caller = crate::journal::current_lf_process_id();
        run_sqlite(&self.sqlite, move |store| {
            store.create_session(session, caller.as_ref())
        })
        .await
    }

    pub(crate) async fn task_conversations(
        &self,
        task: &crate::durable::TaskId,
    ) -> StoreResult<Vec<(LfSession, Option<i64>)>> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| store.task_conversations(&task)).await
    }

    pub(crate) async fn task_primary(
        &self,
        task: &crate::durable::TaskId,
    ) -> StoreResult<Option<LfSession>> {
        let task = task.clone();
        run_sqlite(&self.sqlite, move |store| store.task_primary(&task)).await
    }

    pub(crate) async fn choose_task_primary(
        &self,
        task: &crate::durable::TaskId,
        session: &str,
    ) -> StoreResult<LfSession> {
        let (task, session) = (task.clone(), session.to_string());
        run_sqlite(&self.sqlite, move |store| {
            store.choose_task_primary(&task, &session)
        })
        .await
    }

    pub async fn ensure_primary_session(
        &self,
        scope: &crate::session::PrimaryScope,
        replacing: Option<&str>,
        session: LfSession,
    ) -> StoreResult<LfSession> {
        let scope = scope.clone();
        let replacing = replacing.map(str::to_string);
        let caller = crate::journal::current_lf_process_id();
        run_sqlite(&self.sqlite, move |store| {
            store.ensure_primary_session(&scope, replacing.as_deref(), session, caller.as_ref())
        })
        .await
    }

    pub async fn replace_session_input(
        &self,
        expected_capture: Option<i64>,
        session: LfSession,
    ) -> StoreResult<LfSession> {
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
    ) -> StoreResult<LfSession> {
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
    ) -> StoreResult<Vec<LfSession>> {
        let filter = filter.clone();
        run_sqlite(&self.sqlite, move |store| store.sessions(&filter)).await
    }

    pub async fn session_inputs(&self, id: &str) -> StoreResult<Vec<String>> {
        let id = id.to_string();
        run_sqlite(&self.sqlite, move |store| store.session_inputs(&id)).await
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
}

impl Store {
    pub async fn flow_inventory(
        &self,
        filter: &crate::durable::FlowProcessFilter,
        after: Option<&str>,
        limit: std::num::NonZeroU32,
    ) -> StoreResult<crate::durable::FlowProcessPage> {
        let filter = filter.clone();
        let after = after.map(str::to_owned);
        run_sqlite(&self.sqlite, move |store| {
            store.flow_inventory(&filter, after.as_deref(), limit)
        })
        .await
    }

    /// One Flow by driver Process id or unique prefix, drawn from its Processes.
    pub async fn flow_detail(
        &self,
        selector: &str,
    ) -> StoreResult<Option<crate::durable::FlowProcessDetail>> {
        let selector = selector.to_string();
        run_sqlite(&self.sqlite, move |store| {
            Ok(store
                .flow_process(&selector)?
                .map(|(flow, entry)| flow.detail(entry)))
        })
        .await
    }
}
