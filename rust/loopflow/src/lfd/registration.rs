use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::lfd::http_client::SafeHttpClient;

const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Default, Serialize)]
pub struct RegistrationState {
    pub enabled: bool,
    pub registered: bool,
    pub expires_at: Option<f64>,
    pub last_error: Option<String>,
    pub last_heartbeat: Option<f64>,
    pub machine_id: Option<String>,
    pub machine_name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct RegisterResponse {
    connection_token: String,
    expires_at: Option<f64>,
}

#[derive(Debug, Clone)]
pub struct RegistrationClient {
    base_url: String,
    http: SafeHttpClient,
    state: Arc<RwLock<RegistrationState>>,
    connection_token: Arc<RwLock<Option<String>>>,
}

impl RegistrationClient {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http: SafeHttpClient::new().expect("safe HTTP client should initialize"),
            state: Arc::new(RwLock::new(RegistrationState::default())),
            connection_token: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn status(&self) -> RegistrationState {
        self.state.read().await.clone()
    }

    pub async fn register(
        &self,
        jwt: &str,
        machine_id: &str,
        machine_name: &str,
    ) -> Result<String, RegistrationError> {
        let url = format!("{}/api/v1/daemons/register", self.base_url);

        let payload = serde_json::json!({
            "machine_id": machine_id,
            "machine_name": machine_name,
            "capabilities": ["waves", "terminal"],
        });

        let response = self
            .http
            .send(
                self.http
                    .request(reqwest::Method::POST, &url)
                    .map_err(|e| RegistrationError::Network(e.to_string()))?
                    .header("Authorization", format!("Bearer {jwt}"))
                    .json(&payload)
                    .timeout(Duration::from_secs(10)),
            )
            .await
            .map_err(|e| RegistrationError::Network(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            return Err(RegistrationError::Http(status));
        }

        let data: RegisterResponse = response
            .json()
            .await
            .map_err(|e| RegistrationError::Parse(e.to_string()))?;

        {
            let mut state = self.state.write().await;
            state.enabled = true;
            state.registered = true;
            state.expires_at = data.expires_at;
            state.last_error = None;
            state.machine_id = Some(machine_id.to_string());
            state.machine_name = Some(machine_name.to_string());
        }

        *self.connection_token.write().await = Some(data.connection_token.clone());
        Ok(data.connection_token)
    }

    pub fn start_heartbeat(
        &self,
        jwt: String,
        machine_id: String,
        cancel: CancellationToken,
    ) -> JoinHandle<()> {
        let client = self.clone();
        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(HEARTBEAT_INTERVAL) => {
                        if let Err(e) = client.send_heartbeat(&jwt, &machine_id).await {
                            tracing::warn!(error = %e, "registration heartbeat failed");
                            client.state.write().await.last_error = Some(e.to_string());
                        }
                    }
                    _ = cancel.cancelled() => {
                        break;
                    }
                }
            }
        })
    }

    async fn send_heartbeat(&self, jwt: &str, machine_id: &str) -> Result<(), RegistrationError> {
        let url = format!("{}/api/v1/daemons/heartbeat", self.base_url);

        let payload = serde_json::json!({
            "machine_id": machine_id,
        });

        let response = self
            .http
            .send(
                self.http
                    .request(reqwest::Method::POST, &url)
                    .map_err(|e| RegistrationError::Network(e.to_string()))?
                    .header("Authorization", format!("Bearer {jwt}"))
                    .json(&payload)
                    .timeout(Duration::from_secs(10)),
            )
            .await
            .map_err(|e| RegistrationError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Err(RegistrationError::Http(response.status().as_u16()));
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);

        self.state.write().await.last_heartbeat = Some(now);
        Ok(())
    }

    pub async fn deregister(&self, jwt: &str, machine_id: &str) {
        let registered = self.state.read().await.registered;
        if !registered {
            return;
        }

        let url = format!("{}/api/v1/daemons/deregister", self.base_url);

        let payload = serde_json::json!({
            "machine_id": machine_id,
        });

        if let Ok(builder) = self.http.request(reqwest::Method::POST, &url) {
            let _ = self
                .http
                .send(
                    builder
                        .header("Authorization", format!("Bearer {jwt}"))
                        .json(&payload)
                        .timeout(Duration::from_secs(5)),
                )
                .await;
        }

        {
            let mut state = self.state.write().await;
            state.registered = false;
        }
        *self.connection_token.write().await = None;
    }
}

#[derive(Debug, Clone)]
pub struct ConnectionValidator {
    base_url: String,
    http: SafeHttpClient,
    cache: Arc<RwLock<std::collections::HashMap<String, (bool, Instant)>>>,
}

impl ConnectionValidator {
    pub fn new(base_url: &str) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http: SafeHttpClient::new().expect("safe HTTP client should initialize"),
            cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        }
    }

    pub async fn validate(&self, token: &str) -> bool {
        if token.is_empty() {
            return false;
        }

        {
            let cache = self.cache.read().await;
            if let Some((valid, expires)) = cache.get(token) {
                if Instant::now() < *expires {
                    return *valid;
                }
            }
        }

        let valid = self.validate_remote(token).await.unwrap_or(false);

        {
            let mut cache = self.cache.write().await;
            cache.insert(
                token.to_string(),
                (valid, Instant::now() + Duration::from_secs(60)),
            );
        }

        valid
    }

    async fn validate_remote(&self, token: &str) -> Result<bool, RegistrationError> {
        let url = format!("{}/api/v1/daemons/validate-connection", self.base_url);

        let payload = serde_json::json!({
            "connection_token": token,
        });

        let response = self
            .http
            .send(
                self.http
                    .request(reqwest::Method::POST, &url)
                    .map_err(|e| RegistrationError::Network(e.to_string()))?
                    .json(&payload)
                    .timeout(Duration::from_secs(5)),
            )
            .await
            .map_err(|e| RegistrationError::Network(e.to_string()))?;

        if !response.status().is_success() {
            return Ok(false);
        }

        #[derive(Deserialize)]
        struct ValidateResponse {
            valid: Option<bool>,
        }

        let data: ValidateResponse = response
            .json()
            .await
            .map_err(|e| RegistrationError::Parse(e.to_string()))?;

        Ok(data.valid.unwrap_or(false))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum RegistrationError {
    #[error("network error: {0}")]
    Network(String),
    #[error("HTTP error: {0}")]
    Http(u16),
    #[error("parse error: {0}")]
    Parse(String),
}
