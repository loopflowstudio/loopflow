use std::sync::Arc;

use time::OffsetDateTime;

use crate::auth::AuthContext;
use crate::events::EventHub;
use crate::executor::WaveExecutor;
use crate::output::OutputHub;
use crate::registration::RegistrationClient;
use crate::scheduler::Scheduler;
use crate::store::SharedStore;

#[derive(Clone)]
pub struct HttpState {
    pub store: SharedStore,
    pub scheduler: Arc<Scheduler>,
    pub executor: Arc<WaveExecutor>,
    pub event_hub: EventHub,
    #[allow(dead_code)] // Reserved for output streaming endpoints.
    pub output_hub: OutputHub,
    pub auth: AuthContext,
    pub registration: Option<RegistrationClient>,
    pub started_at: OffsetDateTime,
}
