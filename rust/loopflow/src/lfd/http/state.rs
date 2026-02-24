use std::sync::Arc;

use time::OffsetDateTime;
use tokio::sync::Mutex;

use crate::lfd::auth::{AuthFailureThrottle, AuthProvider};
use crate::lfd::config::{GitHubConfig, HttpSecurityConfig};
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::output::OutputHub;
use crate::lfd::registration::RegistrationClient;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::sessions::SessionManager;
use crate::lfd::store::SharedStore;

#[derive(Clone)]
pub struct HttpState {
    pub store: SharedStore,
    pub scheduler: Arc<Scheduler>,
    pub executor: Arc<WaveExecutor>,
    pub event_hub: EventHub,
    #[allow(dead_code)] // Reserved for output streaming endpoints.
    pub output_hub: OutputHub,
    pub auth: AuthProvider,
    pub session_token: Option<String>,
    pub registration: Option<RegistrationClient>,
    pub started_at: OffsetDateTime,
    pub github: GitHubConfig,
    pub http_security: HttpSecurityConfig,
    pub auth_failure_throttle: AuthFailureThrottle,
    pub ci_failure_cache: Arc<Mutex<std::collections::HashSet<String>>>,
    pub sessions: SessionManager,
}
