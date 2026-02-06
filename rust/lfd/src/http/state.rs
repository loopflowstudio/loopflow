use std::sync::Arc;

use time::OffsetDateTime;

use crate::events::EventHub;
use crate::executor::WaveExecutor;
use crate::output::OutputHub;
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
    pub started_at: OffsetDateTime,
}
