use crate::child_session::ChildRef;
use crate::durable::{
    Author, Basis, BoundarySeed, ControlCtx, Send, SendId, SendState, SteerId, SteerReceipt,
    ToolResponseReceipt, ToolResponseWrite, WorkRef,
};

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub async fn work_for_child(&self, target: &ChildRef) -> StoreResult<WorkRef> {
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| store.work_for_child(&target)).await
    }

    pub async fn current_epoch(&self, work: &WorkRef) -> StoreResult<crate::durable::Epoch> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.current_epoch(&work)).await
    }

    pub async fn boundary_seed(&self, work: &WorkRef) -> StoreResult<BoundarySeed> {
        let work = work.clone();
        run_sqlite(&self.sqlite, move |store| store.boundary_seed(&work)).await
    }

    pub(crate) async fn boundary_seed_for_child(
        &self,
        target: &ChildRef,
    ) -> StoreResult<BoundarySeed> {
        let target = target.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.boundary_seed_for_child(&target)
        })
        .await
    }

    pub async fn steer(
        &self,
        context: &ControlCtx<'_>,
        work: &WorkRef,
        text: &str,
        if_basis: Option<&Basis>,
    ) -> StoreResult<SteerReceipt> {
        let author = match context {
            ControlCtx::User(_) => Author::User,
            ControlCtx::Run(lease) => Author::Run(lease.run_id.clone()),
        };
        self.append_steer(work, author, text, if_basis).await
    }

    pub(crate) async fn append_steer(
        &self,
        work: &WorkRef,
        author: Author,
        text: &str,
        if_basis: Option<&Basis>,
    ) -> StoreResult<SteerReceipt> {
        let work = work.clone();
        let text = text.to_string();
        let if_basis = if_basis.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.append_steer(&work, &author, &text, if_basis.as_ref())
        })
        .await
    }

    pub async fn write_tool_response(
        &self,
        work: &WorkRef,
        write: ToolResponseWrite,
        if_basis: Option<&Basis>,
    ) -> StoreResult<(ToolResponseReceipt, bool)> {
        let work = work.clone();
        let if_basis = if_basis.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.write_tool_response(&work, &write, if_basis.as_ref())
        })
        .await
    }

    pub async fn tool_response(
        &self,
        work: &WorkRef,
        request_id: &str,
    ) -> StoreResult<Option<ToolResponseReceipt>> {
        let work = work.clone();
        let request_id = request_id.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.tool_response(&work, &request_id)
        })
        .await
    }

    pub async fn begin_live_send(
        &self,
        steer_id: &SteerId,
        turn_id: &str,
    ) -> StoreResult<Option<Send>> {
        let steer_id = steer_id.clone();
        let turn_id = turn_id.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.begin_live_send(&steer_id, &turn_id)
        })
        .await
    }

    pub async fn finish_send(
        &self,
        send_id: &SendId,
        state: SendState,
        provider_turn_id: Option<&str>,
        reason: Option<&str>,
    ) -> StoreResult<Send> {
        let send_id = send_id.clone();
        let provider_turn_id = provider_turn_id.map(ToString::to_string);
        let reason = reason.map(ToString::to_string);
        run_sqlite(&self.sqlite, move |store| {
            store.finish_send(
                &send_id,
                state,
                provider_turn_id.as_deref(),
                reason.as_deref(),
            )
        })
        .await
    }

    pub async fn validate_completion_basis(
        &self,
        work: &WorkRef,
        basis: &Basis,
    ) -> StoreResult<()> {
        let work = work.clone();
        let basis = basis.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.validate_completion_basis(&work, &basis)
        })
        .await
    }
}
