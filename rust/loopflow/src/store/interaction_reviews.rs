use crate::child_session::{ChildCommand, ChildCommandSource, ChildWriteLease};
use crate::id::WaveId;
use crate::interaction_review::{
    InteractionReview, InteractionReviewDisposition, InteractionReviewId,
};
use crate::project_session::ProjectSessionId;
use crate::task::{TaskSession, TaskSessionId};

use super::{run_sqlite, Store, StoreResult};

impl Store {
    pub(crate) async fn open_interaction_review(
        &self,
        session: &TaskSession,
        review: &InteractionReview,
        lease: &ChildWriteLease,
    ) -> StoreResult<(InteractionReview, bool)> {
        let session = session.clone();
        let review = review.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.open_interaction_review(&session, &review, &lease)
        })
        .await
    }

    pub async fn get_interaction_review(
        &self,
        review_id: &InteractionReviewId,
    ) -> StoreResult<Option<InteractionReview>> {
        let review_id = review_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.get_interaction_review(&review_id)
        })
        .await
    }

    pub async fn interaction_review_at(
        &self,
        task_session_id: &TaskSessionId,
        phase_epoch: u32,
        phase_iteration: u32,
        step_index: u32,
    ) -> StoreResult<Option<InteractionReview>> {
        let task_session_id = task_session_id.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.interaction_review_at(&task_session_id, phase_epoch, phase_iteration, step_index)
        })
        .await
    }

    pub async fn list_interaction_reviews(
        &self,
        wave_id: Option<&WaveId>,
    ) -> StoreResult<Vec<InteractionReview>> {
        let wave_id = wave_id.cloned();
        run_sqlite(&self.sqlite, move |store| {
            store.list_interaction_reviews(wave_id.as_ref())
        })
        .await
    }

    pub(crate) async fn send_project_interaction_review_message(
        &self,
        review_id: &InteractionReviewId,
        project_session_id: &ProjectSessionId,
        lease: &ChildWriteLease,
        text: &str,
    ) -> StoreResult<ChildCommand> {
        let review_id = review_id.clone();
        let project_session_id = project_session_id.clone();
        let lease = lease.clone();
        let text = text.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.send_project_interaction_review_message(
                &review_id,
                &project_session_id,
                &lease,
                &text,
            )
        })
        .await
    }

    pub(crate) async fn activate_human_interaction_review(
        &self,
        session: &TaskSession,
        review_id: &InteractionReviewId,
        lease: &ChildWriteLease,
    ) -> StoreResult<InteractionReview> {
        let session = session.clone();
        let review_id = review_id.clone();
        let lease = lease.clone();
        run_sqlite(&self.sqlite, move |store| {
            store.activate_human_interaction_review(&session, &review_id, &lease)
        })
        .await
    }

    pub(crate) async fn send_human_interaction_review_message(
        &self,
        review_id: &InteractionReviewId,
        source: ChildCommandSource,
        text: &str,
    ) -> StoreResult<ChildCommand> {
        let review_id = review_id.clone();
        let text = text.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.send_human_interaction_review_message(&review_id, source, &text)
        })
        .await
    }

    pub(crate) async fn reply_to_interaction_review(
        &self,
        review_id: &InteractionReviewId,
        task_session_id: &TaskSessionId,
        lease: &ChildWriteLease,
        text: &str,
    ) -> StoreResult<()> {
        let review_id = review_id.clone();
        let task_session_id = task_session_id.clone();
        let lease = lease.clone();
        let text = text.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.reply_to_interaction_review(&review_id, &task_session_id, &lease, &text)
        })
        .await
    }

    pub(crate) async fn complete_project_interaction_review(
        &self,
        review_id: &InteractionReviewId,
        project_session_id: &ProjectSessionId,
        lease: &ChildWriteLease,
        disposition: InteractionReviewDisposition,
        outcome: &str,
    ) -> StoreResult<(InteractionReview, bool)> {
        let review_id = review_id.clone();
        let project_session_id = project_session_id.clone();
        let lease = lease.clone();
        let outcome = outcome.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.complete_project_interaction_review(
                &review_id,
                &project_session_id,
                &lease,
                disposition,
                &outcome,
            )
        })
        .await
    }

    pub(crate) async fn complete_human_interaction_review(
        &self,
        review_id: &InteractionReviewId,
        disposition: InteractionReviewDisposition,
        outcome: &str,
    ) -> StoreResult<(InteractionReview, bool)> {
        let review_id = review_id.clone();
        let outcome = outcome.to_string();
        run_sqlite(&self.sqlite, move |store| {
            store.complete_human_interaction_review(&review_id, disposition, &outcome)
        })
        .await
    }
}
