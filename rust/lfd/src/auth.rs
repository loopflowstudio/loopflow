use std::sync::Arc;

use tokio::sync::RwLock;

use crate::registration::ConnectionValidator;

/// Shared auth context for HTTP API.
#[derive(Clone)]
pub struct AuthContext {
    pub enabled: Arc<RwLock<bool>>,
    pub registered: Arc<RwLock<bool>>,
    pub validator: Option<ConnectionValidator>,
}

impl AuthContext {
    pub fn new(validator: Option<ConnectionValidator>) -> Self {
        Self {
            enabled: Arc::new(RwLock::new(false)),
            registered: Arc::new(RwLock::new(false)),
            validator,
        }
    }

    pub fn disabled() -> Self {
        Self {
            enabled: Arc::new(RwLock::new(false)),
            registered: Arc::new(RwLock::new(false)),
            validator: None,
        }
    }

    pub async fn set_state(&self, enabled: bool, registered: bool) {
        *self.enabled.write().await = enabled;
        *self.registered.write().await = registered;
    }
}
