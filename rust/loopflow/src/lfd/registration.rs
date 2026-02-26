use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::lfd::address::detect_lfd_url;
use crate::lfd::http_client::SafeHttpClient;
use crate::lfd::redaction::sanitize_operator_message;
use crate::lfd::store::SharedStore;
use crate::lfd::types::Wave;
use secrecy::SecretString;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RegistrationPublicSummary {
    pub enabled: bool,
    pub registered: bool,
}

impl RegistrationState {
    pub fn public_summary(&self) -> RegistrationPublicSummary {
        RegistrationPublicSummary {
            enabled: self.enabled,
            registered: self.registered,
        }
    }

    pub fn sanitized(self) -> Self {
        Self {
            last_error: self
                .last_error
                .map(|error| sanitize_operator_message(&error)),
            ..self
        }
    }
}

#[derive(Debug, Deserialize)]
struct RegisterResponse {
    connection_token: SecretString,
    expires_at: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegistrationRepoSummary {
    pub name: String,
    pub wave_count: u32,
}

#[derive(Debug, Clone)]
pub struct RegistrationClient {
    base_url: String,
    http: SafeHttpClient,
    state: Arc<RwLock<RegistrationState>>,
    connection_token: Arc<RwLock<Option<SecretString>>>,
    store: Option<SharedStore>,
    bind_addr: Option<SocketAddr>,
}

impl RegistrationClient {
    pub fn new(base_url: &str) -> Self {
        Self::new_internal(base_url, None, None)
    }

    pub fn with_context(base_url: &str, store: SharedStore, bind_addr: SocketAddr) -> Self {
        Self::new_internal(base_url, Some(store), Some(bind_addr))
    }

    fn new_internal(
        base_url: &str,
        store: Option<SharedStore>,
        bind_addr: Option<SocketAddr>,
    ) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http: SafeHttpClient::new().expect("safe HTTP client should initialize"),
            state: Arc::new(RwLock::new(RegistrationState::default())),
            connection_token: Arc::new(RwLock::new(None)),
            store,
            bind_addr,
        }
    }

    pub async fn status(&self) -> RegistrationState {
        self.state.read().await.clone()
    }

    #[cfg(test)]
    pub async fn set_state_for_test(&self, state: RegistrationState) {
        *self.state.write().await = state;
    }

    pub async fn register(
        &self,
        jwt: &str,
        machine_id: &str,
        machine_name: &str,
    ) -> Result<SecretString, RegistrationError> {
        let url = self.detect_url().await;
        let repos = self.collect_repo_summary().await;
        let payload = serde_json::json!({
            "machine_id": machine_id,
            "machine_name": machine_name,
            "capabilities": ["waves", "terminal"],
            "url": url,
            "repos": repos,
        });

        let response = self
            .post_json(
                "api/v1/daemons/register",
                &payload,
                Some(jwt),
                Duration::from_secs(10),
            )
            .await?;

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
        let url = self.detect_url().await;
        let repos = self.collect_repo_summary().await;
        let payload = serde_json::json!({
            "machine_id": machine_id,
            "url": url,
            "repos": repos,
        });

        let response = self
            .post_json(
                "api/v1/daemons/heartbeat",
                &payload,
                Some(jwt),
                Duration::from_secs(10),
            )
            .await?;

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

        let payload = serde_json::json!({
            "machine_id": machine_id,
        });

        let _ = self
            .post_json(
                "api/v1/daemons/deregister",
                &payload,
                Some(jwt),
                Duration::from_secs(5),
            )
            .await;

        {
            let mut state = self.state.write().await;
            state.registered = false;
        }
        *self.connection_token.write().await = None;
    }

    async fn post_json(
        &self,
        path: &str,
        payload: &serde_json::Value,
        jwt: Option<&str>,
        timeout: Duration,
    ) -> Result<reqwest::Response, RegistrationError> {
        let url = format!("{}/{}", self.base_url, path);
        let mut builder = self
            .http
            .request(reqwest::Method::POST, &url)
            .map_err(|error| RegistrationError::Network(error.to_string()))?;
        if let Some(jwt) = jwt {
            builder = builder.header("Authorization", format!("Bearer {jwt}"));
        }
        self.http
            .send(builder.json(payload).timeout(timeout))
            .await
            .map_err(|error| RegistrationError::Network(error.to_string()))
    }

    async fn detect_url(&self) -> String {
        detect_lfd_url(self.bind_addr()).await
    }

    fn bind_addr(&self) -> SocketAddr {
        self.bind_addr
            .or_else(|| {
                std::env::var("LFD_HTTP_ADDR")
                    .ok()
                    .and_then(|value| value.parse::<SocketAddr>().ok())
            })
            .unwrap_or_else(|| SocketAddr::from(([127, 0, 0, 1], 2486)))
    }

    async fn collect_repo_summary(&self) -> Vec<RegistrationRepoSummary> {
        let Some(store) = &self.store else {
            return Vec::new();
        };

        match store.list_waves(None).await {
            Ok(waves) => summarize_repos(waves),
            Err(error) => {
                tracing::warn!(error = %error, "failed to collect repo summary for registration");
                Vec::new()
            }
        }
    }
}

fn summarize_repos(waves: Vec<Wave>) -> Vec<RegistrationRepoSummary> {
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();

    for wave in waves {
        let repo_name = Path::new(wave.repo())
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .map(std::string::ToString::to_string)
            .unwrap_or_else(|| wave.repo().clone());
        *counts.entry(repo_name).or_insert(0) += 1;
    }

    counts
        .into_iter()
        .map(|(name, wave_count)| RegistrationRepoSummary { name, wave_count })
        .collect()
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
        let payload = serde_json::json!({
            "connection_token": token,
        });

        let response = self
            .post_json(
                "api/v1/daemons/validate-connection",
                &payload,
                Duration::from_secs(5),
            )
            .await?;

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

    async fn post_json(
        &self,
        path: &str,
        payload: &serde_json::Value,
        timeout: Duration,
    ) -> Result<reqwest::Response, RegistrationError> {
        let url = format!("{}/{}", self.base_url, path);
        self.http
            .send(
                self.http
                    .request(reqwest::Method::POST, &url)
                    .map_err(|error| RegistrationError::Network(error.to_string()))?
                    .json(payload)
                    .timeout(timeout),
            )
            .await
            .map_err(|error| RegistrationError::Network(error.to_string()))
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

#[cfg(test)]
mod tests {
    use super::{
        summarize_repos, RegistrationClient, RegistrationPublicSummary, RegistrationRepoSummary,
        RegistrationState,
    };
    use std::sync::Arc;

    use axum::extract::Json;
    use axum::routing::post;
    use axum::Router;
    use tokio::net::TcpListener;
    use tokio::sync::Mutex;

    use crate::lfd::id::LfdId;
    use crate::lfd::store::{open_store, SharedStore, StorageConfig};
    use crate::lfd::types::Wave;

    #[test]
    fn public_summary_only_contains_enabled_and_registered() {
        let state = RegistrationState {
            enabled: true,
            registered: true,
            machine_id: Some("machine-id".to_string()),
            machine_name: Some("machine-name".to_string()),
            ..RegistrationState::default()
        };

        assert_eq!(
            state.public_summary(),
            RegistrationPublicSummary {
                enabled: true,
                registered: true,
            }
        );
    }

    #[test]
    fn sanitized_redacts_last_error() {
        let state = RegistrationState {
            last_error: Some(
                "request failed for Bearer abcdef0123456789abcdef0123456789 at /tmp/wave"
                    .to_string(),
            ),
            ..RegistrationState::default()
        };
        let sanitized = state.sanitized();
        let last_error = sanitized.last_error.expect("sanitized error");
        assert!(!last_error.contains("abcdef0123456789abcdef0123456789"));
        assert!(!last_error.contains("/tmp/wave"));
        assert!(last_error.contains("[REDACTED_TOKEN]"));
        assert!(last_error.contains("[REDACTED_PATH]"));
    }

    #[test]
    fn summarize_repos_counts_wave_names() {
        let waves = vec![
            Wave::new(LfdId::new(), "one".to_string(), "/tmp/repo-a".to_string()),
            Wave::new(LfdId::new(), "two".to_string(), "/tmp/repo-a".to_string()),
            Wave::new(LfdId::new(), "three".to_string(), "/tmp/repo-b".to_string()),
        ];

        let repos = summarize_repos(waves);
        assert_eq!(
            repos,
            vec![
                RegistrationRepoSummary {
                    name: "repo-a".to_string(),
                    wave_count: 2,
                },
                RegistrationRepoSummary {
                    name: "repo-b".to_string(),
                    wave_count: 1,
                },
            ]
        );
    }

    #[tokio::test]
    async fn register_and_heartbeat_send_url_and_repos() {
        let recorded_payloads = Arc::new(Mutex::new(Vec::<(String, serde_json::Value)>::new()));

        let register_payloads = recorded_payloads.clone();
        let heartbeat_payloads = recorded_payloads.clone();
        let app = Router::new()
            .route(
                "/api/v1/daemons/register",
                post(move |Json(payload): Json<serde_json::Value>| {
                    let register_payloads = register_payloads.clone();
                    async move {
                        register_payloads
                            .lock()
                            .await
                            .push(("register".to_string(), payload));
                        Json(serde_json::json!({
                            "connection_token": "token",
                            "expires_at": null
                        }))
                    }
                }),
            )
            .route(
                "/api/v1/daemons/heartbeat",
                post(move |Json(payload): Json<serde_json::Value>| {
                    let heartbeat_payloads = heartbeat_payloads.clone();
                    async move {
                        heartbeat_payloads
                            .lock()
                            .await
                            .push(("heartbeat".to_string(), payload));
                        Json(serde_json::json!({"ok": true}))
                    }
                }),
            );

        let base_url = spawn_server(app).await;
        let (store, _tmp) = seeded_store().await;
        let client = RegistrationClient::with_context(
            &base_url,
            store,
            "127.0.0.1:2486".parse().expect("socket addr"),
        );

        client
            .register("jwt-token", "machine-1", "devbox")
            .await
            .expect("register should succeed");
        client
            .send_heartbeat("jwt-token", "machine-1")
            .await
            .expect("heartbeat should succeed");

        let payloads = recorded_payloads.lock().await.clone();
        assert_eq!(payloads.len(), 2);

        for (_, payload) in payloads {
            let url = payload["url"].as_str().expect("payload should include url");
            assert!(url.starts_with("http://"));
            assert!(url.ends_with(":2486"));

            let repos = payload["repos"]
                .as_array()
                .expect("payload should include repos");
            assert!(!repos.is_empty());
            assert!(repos
                .iter()
                .any(|repo| repo["name"] == "repo-a" && repo["wave_count"] == 2));
        }
    }

    async fn spawn_server(app: Router) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let addr = listener.local_addr().expect("listener addr");
        let _server = tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve test app");
        });
        format!("http://{addr}")
    }

    async fn seeded_store() -> (SharedStore, tempfile::TempDir) {
        let tmp = tempfile::tempdir().expect("tempdir");
        let db_path = tmp.path().join("lfd.db");
        let store: SharedStore = Arc::new(
            open_store(&StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        );

        store
            .create_wave(&Wave::new(
                LfdId::new(),
                "wave-a1".to_string(),
                "/tmp/repo-a".to_string(),
            ))
            .await
            .expect("create wave");
        store
            .create_wave(&Wave::new(
                LfdId::new(),
                "wave-a2".to_string(),
                "/tmp/repo-a".to_string(),
            ))
            .await
            .expect("create wave");
        store
            .create_wave(&Wave::new(
                LfdId::new(),
                "wave-b1".to_string(),
                "/tmp/repo-b".to_string(),
            ))
            .await
            .expect("create wave");

        (store, tmp)
    }
}
