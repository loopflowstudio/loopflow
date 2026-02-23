use crate::lfd::id::LfdId;
use crate::lfd::sessions::types::{PersistedSessionEvent, Session, SessionStatus};
use crate::lfd::store::{SharedStore, StoreResult};

#[derive(Clone, Debug)]
pub struct SessionStore {
    store: SharedStore,
}

impl SessionStore {
    pub fn new(store: SharedStore) -> Self {
        Self { store }
    }

    pub async fn create_session(&self, session: &Session) -> StoreResult<()> {
        self.store.create_session(session).await
    }

    pub async fn get_session(&self, session_id: &LfdId) -> StoreResult<Option<Session>> {
        self.store.get_session(session_id).await
    }

    pub async fn get_active_session_for_wave_run(
        &self,
        wave_run_id: &str,
    ) -> StoreResult<Option<Session>> {
        self.store
            .get_active_session_for_wave_run(wave_run_id)
            .await
    }

    pub async fn update_session_status(
        &self,
        session_id: &LfdId,
        status: SessionStatus,
        ended_at: Option<i64>,
    ) -> StoreResult<()> {
        self.store
            .update_session_status(session_id, status, ended_at)
            .await
    }

    pub async fn append_event(
        &self,
        session_id: &LfdId,
        seq: i64,
        event: &crate::lfd::sessions::types::SessionEvent,
        created_at: i64,
    ) -> StoreResult<()> {
        self.store
            .append_session_event(session_id, seq, event, created_at)
            .await
    }

    pub async fn list_events(
        &self,
        session_id: &LfdId,
        after_seq: Option<i64>,
    ) -> StoreResult<Vec<PersistedSessionEvent>> {
        self.store.list_session_events(session_id, after_seq).await
    }
}
