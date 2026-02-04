use std::sync::Arc;

use tokio::sync::RwLock;
use tonic::{Request, Status};

use crate::registration::ConnectionValidator;

/// Shared registration context for gRPC auth.
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

    /// Check if a request should be authenticated.
    /// Returns Ok(()) if allowed, Err(Status) if denied.
    pub async fn check_request<T>(&self, request: &Request<T>) -> Result<(), Status> {
        let enabled = *self.enabled.read().await;
        let registered = *self.registered.read().await;

        // If registration is not enabled or not registered, allow all requests
        if !enabled || !registered {
            return Ok(());
        }

        // Registration is enabled and registered, require connection token
        let Some(validator) = &self.validator else {
            return Ok(());
        };

        let token = extract_connection_token(request);
        let Some(token) = token else {
            return Err(Status::unauthenticated("missing connection token"));
        };

        if validator.validate(&token).await {
            Ok(())
        } else {
            Err(Status::unauthenticated("invalid connection token"))
        }
    }
}

fn extract_connection_token<T>(request: &Request<T>) -> Option<String> {
    // Try x-loopflow-connection-token header first
    if let Some(value) = request.metadata().get("x-loopflow-connection-token") {
        if let Ok(token) = value.to_str() {
            return Some(token.to_string());
        }
    }

    // Try connection-token header
    if let Some(value) = request.metadata().get("connection-token") {
        if let Ok(token) = value.to_str() {
            return Some(token.to_string());
        }
    }

    // Try Authorization: Bearer header
    if let Some(value) = request.metadata().get("authorization") {
        if let Ok(auth) = value.to_str() {
            if let Some(token) = auth.strip_prefix("Bearer ") {
                return Some(token.trim().to_string());
            }
            if let Some(token) = auth.strip_prefix("bearer ") {
                return Some(token.trim().to_string());
            }
        }
    }

    None
}
