use std::sync::Arc;

use time::OffsetDateTime;
use tokio::sync::Mutex;

use crate::lfd::auth::{AuthFailureThrottle, AuthProvider};
use crate::lfd::config::{GitHubConfig, HttpSecurityConfig};
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::output::OutputHub;
use crate::lfdb::SharedStore;
use crate::provider_auth::ProviderAuthService;

#[derive(Clone)]
pub struct HttpState {
    pub store: SharedStore,
    pub executor: Arc<WaveExecutor>,
    pub event_hub: EventHub,
    #[allow(dead_code)] // Reserved for output streaming endpoints.
    pub output_hub: OutputHub,
    pub provider_auth: ProviderAuthService,
    pub auth: AuthProvider,
    pub started_at: OffsetDateTime,
    pub github: GitHubConfig,
    pub http_security: HttpSecurityConfig,
    pub auth_failure_throttle: AuthFailureThrottle,
    pub ci_failure_cache: Arc<Mutex<std::collections::HashSet<String>>>,
}
