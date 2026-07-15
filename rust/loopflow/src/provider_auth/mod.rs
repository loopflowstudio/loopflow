//! Provider auth flows: device-code/browser OAuth for the agent and PM
//! providers, token extraction from vendor CLI artifacts, and store-direct
//! persistence into store provider tokens.
//!
//! Shared home — called in-process by `lf auth`. Auth lifecycle notifications
//! go through [`AuthEventSink`]. Auth is poll-only in the base
//! wave model (see `scratch/eventing.md` §5), so both the CLI and the HTTP
//! routes pass [`no_event_sink`] — the store write is the record; a caller
//! reads it back by querying the provider list.

pub mod credential_socket;

use std::collections::{HashMap, HashSet};
use std::fs;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use once_cell::sync::Lazy;
use regex::Regex;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::provider_auth::credential_socket::{
    AuthStartResponse, CredentialSocketClient, CredentialSocketError,
};
use crate::store::{CredentialType, ProviderToken, SharedStore};

const AUTH_URL_TIMEOUT: Duration = Duration::from_secs(20);
const AUTH_URL_POLL_INTERVAL: Duration = Duration::from_millis(200);
const SOCKET_AUTH_POLL_INTERVAL: Duration = Duration::from_secs(1);
const SOCKET_AUTH_DEFAULT_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const SOCKET_AUTH_MAX_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const OPENCODE_AUTH_POLL_INTERVAL: Duration = Duration::from_secs(1);
const OPENCODE_AUTH_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const OPENCODE_AUTH_URL: &str = "https://opencode.ai/auth";
const LINEAR_OAUTH_AUTHORIZE_URL: &str = "https://linear.app/oauth/authorize";
const LINEAR_OAUTH_TOKEN_URL: &str = "https://api.linear.app/oauth/token";
const LINEAR_OAUTH_REDIRECT_URI: &str = "http://localhost:19222/oauth/callback";
const LINEAR_OAUTH_CALLBACK_ADDR: &str = "127.0.0.1:19222";
const LINEAR_CLIENT_ID_ENV: &str = "LINEAR_CLIENT_ID";
const LINEAR_CLIENT_SECRET_ENV: &str = "LINEAR_CLIENT_SECRET";
const LINEAR_OAUTH_DEFAULT_SCOPE: &str = "read,write";
pub(crate) const TOKEN_REFRESH_LEAD_SECONDS: i64 = 20 * 60;

static USER_CODE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b([A-Z0-9]{4,}(?:-[A-Z0-9]{4,})+)\b").expect("user code regex"));
static EXPIRES_IN_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)expires(?:_in| in)?[^0-9]*(\d{2,6})").expect("expires regex"));
static GH_LOGIN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)logged in to\s+\S+\s+as\s+([a-z0-9-]+)").expect("github login regex")
});
static ANSI_ESCAPE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\x1B\[[0-9;]*[A-Za-z]").expect("ansi escape regex"));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Provider {
    GitHub,
    Claude,
    Codex,
    OpenCodeZen,
    Linear,
    Doppler,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GitHub => "github",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::OpenCodeZen => "opencodezen",
            Self::Linear => "linear",
            Self::Doppler => "doppler",
        }
    }

    pub fn all() -> [Self; 6] {
        [
            Self::GitHub,
            Self::Claude,
            Self::Codex,
            Self::OpenCodeZen,
            Self::Linear,
            Self::Doppler,
        ]
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Self::GitHub => "GitHub",
            Self::Claude => "Claude",
            Self::Codex => "Codex",
            Self::OpenCodeZen => "OpenCode Zen",
            Self::Linear => "Linear",
            Self::Doppler => "Doppler",
        }
    }

    pub fn api_key_env_name(self) -> Option<&'static str> {
        match self {
            Self::Claude => Some("ANTHROPIC_API_KEY"),
            Self::Codex => Some("OPENAI_API_KEY"),
            Self::OpenCodeZen => Some("OPENCODE_API_KEY"),
            Self::GitHub | Self::Linear | Self::Doppler => None,
        }
    }

    pub fn api_key_bills_per_token(self) -> bool {
        matches!(self, Self::Claude | Self::Codex | Self::OpenCodeZen)
    }

    pub fn api_key_configure_error(self) -> Option<&'static str> {
        match self {
            Self::Linear => Some("Linear requires OAuth. Run 'lf auth linear' to connect."),
            _ => None,
        }
    }

    /// Whether this provider can refresh without user interaction.
    pub fn supports_automatic_refresh(self) -> bool {
        matches!(self, Self::GitHub | Self::Codex | Self::Linear)
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Provider {
    type Err = ParseProviderError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "github" | "gh" => Ok(Self::GitHub),
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            "opencodezen" | "opencode" | "zen" | "oc" => Ok(Self::OpenCodeZen),
            "linear" | "lin" => Ok(Self::Linear),
            "doppler" => Ok(Self::Doppler),
            _ => Err(ParseProviderError {
                input: value.trim().to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("unsupported provider: {input}")]
pub struct ParseProviderError {
    input: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthStatus {
    Active { login: Option<String> },
    Pending,
    None,
    Expired,
}

impl AuthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active { .. } => "active",
            Self::Pending => "pending",
            Self::None => "none",
            Self::Expired => "expired",
        }
    }

    pub fn login(&self) -> Option<String> {
        match self {
            Self::Active { login } => login.clone(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthFlowResponse {
    pub provider: Provider,
    pub verification_uri: String,
    pub verification_uri_complete: Option<String>,
    pub user_code: Option<String>,
    pub expires_in: Option<u64>,
}

/// Auth lifecycle notifications. Base callers pass [`no_event_sink`] and rely
/// on the store write; the type is retained as the sink a future narrow auth
/// SSE (see `scratch/eventing.md` §5b) would feed.
#[derive(Debug, Clone)]
pub enum AuthEvent {
    FlowStarted {
        provider: Provider,
        verification_uri: String,
        verification_uri_complete: Option<String>,
    },
    Connected {
        provider: Provider,
        login: Option<String>,
    },
    Failed {
        provider: Provider,
        error: String,
    },
    Disconnected {
        provider: Provider,
    },
}

pub type AuthEventSink = Arc<dyn Fn(AuthEvent) + Send + Sync>;

pub fn no_event_sink() -> AuthEventSink {
    Arc::new(|_| {})
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAuthSnapshot {
    pub provider: Provider,
    pub status: AuthStatus,
    pub expires_at: Option<i64>,
    pub next_refresh_at: Option<i64>,
    pub credential_type: Option<CredentialType>,
}

pub struct AuthFlowHandle {
    pub response: AuthFlowResponse,
    monitor: AuthMonitor,
}

impl std::fmt::Debug for AuthFlowHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthFlowHandle")
            .field("response", &self.response)
            .finish()
    }
}

impl AuthFlowHandle {
    fn new(response: AuthFlowResponse, monitor: JoinHandle<Result<(), AuthError>>) -> Self {
        Self {
            response,
            monitor: AuthMonitor::new(monitor),
        }
    }

    pub async fn wait(self) -> Result<(), AuthError> {
        self.monitor.wait(self.response.provider).await
    }
}

struct AuthMonitor {
    task: Option<JoinHandle<Result<(), AuthError>>>,
}

impl AuthMonitor {
    fn new(task: JoinHandle<Result<(), AuthError>>) -> Self {
        Self { task: Some(task) }
    }

    async fn wait(mut self, provider: Provider) -> Result<(), AuthError> {
        let result = self
            .task
            .as_mut()
            .expect("auth monitor task should exist")
            .await
            .map_err(|error| AuthError::CommandFailed {
                provider,
                message: format!("auth monitor task failed: {error}"),
            });
        self.task.take();
        result?
    }
}

impl Drop for AuthMonitor {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("unsupported provider: {0}")]
    UnsupportedProvider(String),
    #[error("auth flow already in progress for {0}")]
    FlowAlreadyPending(Provider),
    #[error("no pending auth flow for {0}")]
    NoPendingFlow(Provider),
    #[error("{0} auth flow does not accept manual completion")]
    CompletionUnavailable(Provider),
    #[error("{provider} CLI not found: {command}")]
    CommandUnavailable { provider: Provider, command: String },
    #[error("failed to start {provider} auth command: {source}")]
    CommandSpawn {
        provider: Provider,
        #[source]
        source: std::io::Error,
    },
    #[error("{provider} auth command did not emit a verification URL within {timeout_secs}s")]
    MissingVerificationUrl {
        provider: Provider,
        timeout_secs: u64,
    },
    #[error("{provider} auth command failed: {message}")]
    CommandFailed { provider: Provider, message: String },
    #[error("{provider} auth command IO failure: {source}")]
    CommandIo {
        provider: Provider,
        #[source]
        source: std::io::Error,
    },
    #[error("filesystem error: {0}")]
    Filesystem(String),
    #[error("{provider} rejected the authorization code: {message}")]
    CodeExchangeRejected { provider: Provider, message: String },
    #[error("{provider} OAuth request failed: {message}")]
    OAuthRequest { provider: Provider, message: String },
    #[error("credential socket request failed for {provider}: {message}")]
    CredentialSocket { provider: Provider, message: String },
}

#[derive(Debug, Error)]
pub enum TokenRefreshError {
    #[error("{provider} refresh command unavailable: {command}")]
    CommandUnavailable { provider: Provider, command: String },
    #[error("{provider} refresh command failed: {message}")]
    CommandFailed { provider: Provider, message: String },
    #[error("{provider} refresh command IO failure: {source}")]
    CommandIo {
        provider: Provider,
        #[source]
        source: std::io::Error,
    },
    #[error("{provider} token not found after refresh")]
    MissingToken { provider: Provider },
    #[error("{provider} OAuth refresh failed: {reason}")]
    OAuth {
        provider: Provider,
        reason: &'static str,
    },
}

#[async_trait]
trait RefreshCommandRunner: Send + Sync {
    async fn run(
        &self,
        program: &'static str,
        args: &'static [&'static str],
    ) -> Result<std::process::Output, std::io::Error>;
}

#[derive(Debug)]
struct TokioRefreshCommandRunner;

#[async_trait]
impl RefreshCommandRunner for TokioRefreshCommandRunner {
    async fn run(
        &self,
        program: &'static str,
        args: &'static [&'static str],
    ) -> Result<std::process::Output, std::io::Error> {
        let mut command = Command::new(program);
        command.args(args);
        command.output().await
    }
}

#[async_trait]
pub trait AuthBroker: Send + Sync {
    fn provider(&self) -> Provider;
    async fn start_auth(&self) -> Result<AuthFlowHandle, AuthError>;
    async fn check_status(&self) -> Result<AuthStatus, AuthError>;
    async fn disconnect(&self) -> Result<(), AuthError>;
    async fn complete_auth(&self, _code: &str) -> Result<(), AuthError> {
        Err(AuthError::CompletionUnavailable(self.provider()))
    }

    /// Extract a token from CLI artifacts after a successful auth flow.
    async fn extract_token(&self) -> Option<ProviderToken> {
        None
    }
}

#[derive(Debug, Clone)]
pub struct SocketAuthBroker {
    provider_name: Provider,
    client: Arc<CredentialSocketClient>,
}

#[derive(Debug, Deserialize)]
struct OAuthErrorResponse {
    error: Option<String>,
    error_description: Option<String>,
}

// ── Linear OAuth ────────────────────────────────────────────────────

/// Linear OAuth uses a loopback redirect (`http://localhost:19222/oauth/callback`):
/// consent is a single click and the code arrives on a short-lived local listener.
/// Once Linear returns a refresh token, `refresh_pm_oauth_token` renews headlessly
/// with the stored PKCE client ID.
#[derive(Debug, Clone)]
struct LinearOAuthBroker {
    completed_token: Arc<Mutex<Option<ProviderToken>>>,
}

#[derive(Debug, Clone)]
struct LinearOAuthApp {
    client_id: String,
    client_secret: String,
    scope: String,
}

#[derive(Debug, Deserialize)]
struct LinearOAuthTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

impl LinearOAuthBroker {
    fn new() -> Self {
        Self {
            completed_token: Arc::new(Mutex::new(None)),
        }
    }

    async fn oauth_app() -> Result<LinearOAuthApp, AuthError> {
        let (client_id, client_secret) = oauth_client_credentials(
            Provider::Linear,
            LINEAR_CLIENT_ID_ENV,
            LINEAR_CLIENT_SECRET_ENV,
        )
        .await?;

        Ok(LinearOAuthApp {
            client_id,
            client_secret,
            scope: LINEAR_OAUTH_DEFAULT_SCOPE.to_string(),
        })
    }

    fn build_authorization_url(app: &LinearOAuthApp, code_verifier: &str, state: &str) -> String {
        let code_challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));
        let mut url = Url::parse(LINEAR_OAUTH_AUTHORIZE_URL)
            .expect("linear oauth authorize URL should parse");
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("client_id", &app.client_id);
            pairs.append_pair("redirect_uri", LINEAR_OAUTH_REDIRECT_URI);
            pairs.append_pair("response_type", "code");
            pairs.append_pair("state", state);
            pairs.append_pair("scope", &app.scope);
            pairs.append_pair("code_challenge", &code_challenge);
            pairs.append_pair("code_challenge_method", "S256");
            pairs.append_pair("prompt", "consent");
        }
        url.to_string()
    }

    async fn exchange_code(
        app: &LinearOAuthApp,
        code_verifier: &str,
        code: &str,
    ) -> Result<ProviderToken, AuthError> {
        let body = serde_urlencoded::to_string([
            ("grant_type", "authorization_code"),
            ("client_id", app.client_id.as_str()),
            ("client_secret", app.client_secret.as_str()),
            ("redirect_uri", LINEAR_OAUTH_REDIRECT_URI),
            ("code", code),
            ("code_verifier", code_verifier),
        ])
        .map_err(|err| AuthError::OAuthRequest {
            provider: Provider::Linear,
            message: format!("failed to encode token request: {err}"),
        })?;
        let response = reqwest::Client::new()
            .post(LINEAR_OAUTH_TOKEN_URL)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|err| AuthError::OAuthRequest {
                provider: Provider::Linear,
                message: err.to_string(),
            })?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .bytes()
                .await
                .map_err(|err| AuthError::OAuthRequest {
                    provider: Provider::Linear,
                    message: err.to_string(),
                })?;
            let message = oauth_error_message(body.as_ref())
                .unwrap_or_else(|| String::from_utf8_lossy(&body).trim().to_string());
            if status == reqwest::StatusCode::BAD_REQUEST
                || status == reqwest::StatusCode::UNAUTHORIZED
            {
                return Err(AuthError::CodeExchangeRejected {
                    provider: Provider::Linear,
                    message,
                });
            }
            return Err(AuthError::OAuthRequest {
                provider: Provider::Linear,
                message: format!("HTTP {status}: {message}"),
            });
        }

        let payload = response
            .json::<LinearOAuthTokenResponse>()
            .await
            .map_err(|err| AuthError::OAuthRequest {
                provider: Provider::Linear,
                message: format!("failed to decode token response: {err}"),
            })?;

        let expires_at = payload
            .expires_in
            .filter(|seconds| *seconds > 0)
            .map(|seconds| now_unix() + seconds);

        Ok(ProviderToken {
            provider: Provider::Linear.as_str().to_string(),
            access_token: payload.access_token,
            refresh_token: payload.refresh_token,
            oauth_client_id: Some(app.client_id.clone()),
            expires_at,
            login: None,
            updated_at: now_unix(),
            credential_type: CredentialType::OAuth,
        })
    }
}

#[async_trait]
impl AuthBroker for LinearOAuthBroker {
    fn provider(&self) -> Provider {
        Provider::Linear
    }

    async fn start_auth(&self) -> Result<AuthFlowHandle, AuthError> {
        let app = Self::oauth_app().await?;
        let code_verifier = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let state = Uuid::new_v4().to_string();
        let verification_uri = Self::build_authorization_url(&app, &code_verifier, &state);
        let listener = oauth_callback_listener(Provider::Linear, LINEAR_OAUTH_CALLBACK_ADDR)?;
        let completed_token = self.completed_token.clone();
        let monitor = tokio::spawn(monitor_oauth_callback(
            Provider::Linear,
            listener,
            Duration::from_secs(15 * 60),
            "linear OAuth timed out",
            completed_token,
            move |code| {
                let app = app.clone();
                let code_verifier = code_verifier.clone();
                async move { LinearOAuthBroker::exchange_code(&app, &code_verifier, &code).await }
            },
        ));

        let response = AuthFlowResponse {
            provider: Provider::Linear,
            verification_uri_complete: Some(verification_uri.clone()),
            verification_uri,
            user_code: None,
            expires_in: Some(15 * 60),
        };

        Ok(AuthFlowHandle::new(response, monitor))
    }

    async fn check_status(&self) -> Result<AuthStatus, AuthError> {
        let token = self.completed_token.lock().await;
        let Some(token) = token.as_ref() else {
            return Ok(AuthStatus::None);
        };
        if token
            .expires_at
            .is_some_and(|expires_at| expires_at <= now_unix())
        {
            return Ok(AuthStatus::Expired);
        }
        Ok(AuthStatus::Active { login: None })
    }

    async fn disconnect(&self) -> Result<(), AuthError> {
        *self.completed_token.lock().await = None;
        Ok(())
    }

    async fn extract_token(&self) -> Option<ProviderToken> {
        self.completed_token.lock().await.clone()
    }
}

impl SocketAuthBroker {
    pub fn new(provider: Provider, client: Arc<CredentialSocketClient>) -> Self {
        Self {
            provider_name: provider,
            client,
        }
    }

    fn map_socket_error(&self, err: CredentialSocketError) -> AuthError {
        AuthError::CredentialSocket {
            provider: self.provider_name,
            message: err.to_string(),
        }
    }
}

#[async_trait]
impl AuthBroker for SocketAuthBroker {
    fn provider(&self) -> Provider {
        self.provider_name
    }

    async fn start_auth(&self) -> Result<AuthFlowHandle, AuthError> {
        let response = self
            .client
            .start_auth(self.provider_name.as_str())
            .await
            .map_err(|err| self.map_socket_error(err))?;
        Ok(socket_auth_flow_handle(
            self.provider_name,
            response,
            self.client.clone(),
        ))
    }

    async fn check_status(&self) -> Result<AuthStatus, AuthError> {
        match self
            .client
            .get_credential(self.provider_name.as_str())
            .await
        {
            Ok(credential) => {
                if credential
                    .expires_at
                    .as_deref()
                    .and_then(parse_expires_at)
                    .is_some_and(|expires_at| expires_at <= now_unix())
                {
                    return Ok(AuthStatus::Expired);
                }
                Ok(AuthStatus::Active {
                    login: credential.login,
                })
            }
            Err(CredentialSocketError::NotFound { .. }) => Ok(AuthStatus::None),
            Err(err) => Err(self.map_socket_error(err)),
        }
    }

    async fn disconnect(&self) -> Result<(), AuthError> {
        self.client
            .disconnect(self.provider_name.as_str())
            .await
            .map_err(|err| self.map_socket_error(err))
    }

    async fn extract_token(&self) -> Option<ProviderToken> {
        let credential = self
            .client
            .get_credential(self.provider_name.as_str())
            .await
            .ok()?;
        Some(ProviderToken {
            provider: self.provider_name.as_str().to_string(),
            access_token: credential.token,
            refresh_token: None,
            oauth_client_id: None,
            expires_at: credential.expires_at.as_deref().and_then(parse_expires_at),
            login: credential.login,
            updated_at: now_unix(),
            credential_type: CredentialType::OAuth,
        })
    }
}

#[derive(Clone)]
pub struct ProviderAuthService {
    brokers: HashMap<Provider, Arc<dyn AuthBroker>>,
    pending: Arc<Mutex<HashMap<Provider, JoinHandle<()>>>>,
    store: Option<SharedStore>,
}

impl std::fmt::Debug for ProviderAuthService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let providers = self.brokers.keys().copied().collect::<Vec<_>>();
        f.debug_struct("ProviderAuthService")
            .field("providers", &providers)
            .finish()
    }
}

impl ProviderAuthService {
    pub fn new(store: SharedStore) -> Self {
        if let Ok(socket_path) = std::env::var("LF_CREDENTIAL_SOCKET") {
            let trimmed = socket_path.trim();
            if !trimmed.is_empty() {
                let client = Arc::new(CredentialSocketClient::new(PathBuf::from(trimmed)));
                return Self::with_brokers_and_store(default_brokers(Some(client)), Some(store));
            }
        }

        Self::with_brokers_and_store(default_brokers(None), Some(store))
    }

    #[cfg(test)]
    pub(crate) fn with_brokers(brokers: Vec<Arc<dyn AuthBroker>>) -> Self {
        Self::with_brokers_and_store(brokers, None)
    }

    fn with_brokers_and_store(
        brokers: Vec<Arc<dyn AuthBroker>>,
        store: Option<SharedStore>,
    ) -> Self {
        let mut by_provider = HashMap::new();
        for broker in brokers {
            by_provider.insert(broker.provider(), broker);
        }
        Self {
            brokers: by_provider,
            pending: Arc::new(Mutex::new(HashMap::new())),
            store,
        }
    }

    pub async fn list_statuses(&self) -> Result<Vec<ProviderAuthSnapshot>, AuthError> {
        self.prune_finished_pending().await;
        let pending_providers = self.pending_providers().await;
        let mut snapshots = Vec::with_capacity(Provider::all().len());

        for provider in Provider::all() {
            if pending_providers.contains(&provider) {
                snapshots.push(ProviderAuthSnapshot {
                    provider,
                    status: AuthStatus::Pending,
                    expires_at: None,
                    next_refresh_at: None,
                    credential_type: None,
                });
                continue;
            }
            snapshots.push(self.resolve_snapshot(provider).await?);
        }

        Ok(snapshots)
    }

    pub async fn status(&self, provider: Provider) -> Result<ProviderAuthSnapshot, AuthError> {
        self.prune_finished_pending().await;
        if self.is_pending(provider).await {
            return Ok(ProviderAuthSnapshot {
                provider,
                status: AuthStatus::Pending,
                expires_at: None,
                next_refresh_at: None,
                credential_type: None,
            });
        }

        self.resolve_snapshot(provider).await
    }

    async fn resolve_snapshot(
        &self,
        provider: Provider,
    ) -> Result<ProviderAuthSnapshot, AuthError> {
        if let Some(store) = &self.store {
            match store.get_provider_token(provider.as_str()).await {
                Ok(Some(token)) => {
                    let status = if token
                        .expires_at
                        .is_some_and(|expires_at| expires_at <= now_unix())
                    {
                        AuthStatus::Expired
                    } else {
                        AuthStatus::Active { login: token.login }
                    };
                    let expires_at = token.expires_at;
                    let next_refresh_at =
                        expires_at.map(|value| value - TOKEN_REFRESH_LEAD_SECONDS);
                    return Ok(ProviderAuthSnapshot {
                        provider,
                        status,
                        expires_at,
                        next_refresh_at,
                        credential_type: Some(token.credential_type),
                    });
                }
                Ok(None) => {}
                Err(err) => {
                    warn!(provider = %provider, error = %err, "failed to load provider token")
                }
            }
        }

        let broker = self.broker(provider)?;
        let status = broker.check_status().await?;
        debug!(provider = %provider, ?status, "broker check_status result (no stored token)");

        // If the CLI reports Active but we have no stored token, auto-persist it.
        // This handles providers where the user logged in outside Loopflow
        // (e.g., `doppler login` in a terminal).
        if matches!(&status, AuthStatus::Active { .. }) {
            if let Some(store) = &self.store {
                if let Some(token) = broker.extract_token().await {
                    info!(provider = %provider, "auto-persisting CLI token");
                    if let Err(err) = store.upsert_provider_token(&token).await {
                        warn!(provider = %provider, error = %err, "failed to auto-persist provider token");
                    }
                    return Ok(ProviderAuthSnapshot {
                        provider,
                        status,
                        expires_at: token.expires_at,
                        next_refresh_at: None,
                        credential_type: Some(token.credential_type),
                    });
                }
                debug!(provider = %provider, "broker reported Active but extract_token returned None");
            }
        }

        Ok(ProviderAuthSnapshot {
            provider,
            status,
            expires_at: None,
            next_refresh_at: None,
            credential_type: None,
        })
    }

    pub async fn start_auth(
        &self,
        provider: Provider,
        events: AuthEventSink,
    ) -> Result<AuthFlowResponse, AuthError> {
        self.prune_finished_pending().await;
        if self.is_pending(provider).await {
            return Err(AuthError::FlowAlreadyPending(provider));
        }

        let broker = self.broker(provider)?;
        let AuthFlowHandle { response, monitor } = broker.start_auth().await?;

        events(AuthEvent::FlowStarted {
            provider,
            verification_uri: response.verification_uri.clone(),
            verification_uri_complete: response.verification_uri_complete.clone(),
        });

        let pending = self.pending.clone();
        let broker_for_task = broker.clone();
        let events_for_task = events.clone();
        let store_for_task = self.store.clone();

        let lifecycle = tokio::spawn(async move {
            let monitor_result = monitor.wait(provider).await;

            match monitor_result {
                Ok(()) => match broker_for_task.check_status().await {
                    Ok(AuthStatus::Active { login }) => {
                        // Extract and persist the token (always as OAuth)
                        if let Some(store) = &store_for_task {
                            // Check if switching from apikey to oauth
                            if let Ok(Some(existing)) =
                                store.get_provider_token(provider.as_str()).await
                            {
                                if existing.credential_type == CredentialType::ApiKey {
                                    tracing::info!(
                                        provider = %provider,
                                        "switched from API key to OAuth (subscription billing)"
                                    );
                                }
                            }
                            if let Some(token) = broker_for_task.extract_token().await {
                                if let Err(err) = store.upsert_provider_token(&token).await {
                                    warn!(provider = %provider, error = %err, "failed to persist provider token");
                                }
                            }
                        }
                        events_for_task(AuthEvent::Connected { provider, login });
                    }
                    Ok(status) => {
                        events_for_task(AuthEvent::Failed {
                            provider,
                            error: format!("completed with status {}", status.as_str()),
                        });
                    }
                    Err(err) => {
                        events_for_task(AuthEvent::Failed {
                            provider,
                            error: err.to_string(),
                        });
                    }
                },
                Err(err) => {
                    events_for_task(AuthEvent::Failed {
                        provider,
                        error: err.to_string(),
                    });
                }
            }

            let mut pending = pending.lock().await;
            pending.remove(&provider);
        });

        self.pending.lock().await.insert(provider, lifecycle);
        Ok(response)
    }

    pub async fn complete_auth(&self, provider: Provider, code: &str) -> Result<(), AuthError> {
        self.broker(provider)?.complete_auth(code).await
    }

    pub async fn disconnect(
        &self,
        provider: Provider,
        events: AuthEventSink,
    ) -> Result<(), AuthError> {
        self.abort_pending(provider).await;
        self.broker(provider)?.disconnect().await?;
        if let Some(store) = &self.store {
            if let Err(err) = store.delete_provider_token(provider.as_str()).await {
                warn!(provider = %provider, error = %err, "failed to delete provider token");
            }
        }
        events(AuthEvent::Disconnected { provider });
        Ok(())
    }

    async fn abort_pending(&self, provider: Provider) {
        let handle = self.pending.lock().await.remove(&provider);
        if let Some(handle) = handle {
            handle.abort();
        }
    }

    async fn prune_finished_pending(&self) {
        let mut pending = self.pending.lock().await;
        pending.retain(|_, handle| !handle.is_finished());
    }

    async fn is_pending(&self, provider: Provider) -> bool {
        self.pending.lock().await.contains_key(&provider)
    }

    async fn pending_providers(&self) -> HashSet<Provider> {
        self.pending.lock().await.keys().copied().collect()
    }

    fn broker(&self, provider: Provider) -> Result<Arc<dyn AuthBroker>, AuthError> {
        self.brokers
            .get(&provider)
            .cloned()
            .ok_or_else(|| AuthError::UnsupportedProvider(provider.to_string()))
    }
}

fn default_brokers(client: Option<Arc<CredentialSocketClient>>) -> Vec<Arc<dyn AuthBroker>> {
    let mut brokers = match client {
        Some(client) => vec![
            Arc::new(SocketAuthBroker::new(Provider::GitHub, client.clone()))
                as Arc<dyn AuthBroker>,
            Arc::new(SocketAuthBroker::new(Provider::Claude, client.clone()))
                as Arc<dyn AuthBroker>,
            Arc::new(SocketAuthBroker::new(Provider::Codex, client)) as Arc<dyn AuthBroker>,
        ],
        None => vec![
            Arc::new(GhAuthBroker::default()) as Arc<dyn AuthBroker>,
            Arc::new(ClaudeAuthBroker::default()) as Arc<dyn AuthBroker>,
            Arc::new(CodexAuthBroker::default()) as Arc<dyn AuthBroker>,
        ],
    };
    brokers.push(Arc::new(OpenCodeZenBroker::default()));
    brokers.push(Arc::new(DopplerAuthBroker));
    brokers.extend(pm_auth_brokers());
    brokers
}

fn pm_auth_brokers() -> [Arc<dyn AuthBroker>; 1] {
    [Arc::new(LinearOAuthBroker::new()) as Arc<dyn AuthBroker>]
}

fn socket_auth_flow_handle(
    provider: Provider,
    response: AuthStartResponse,
    client: Arc<CredentialSocketClient>,
) -> AuthFlowHandle {
    let timeout = response
        .expires_in
        .map(Duration::from_secs)
        .unwrap_or(SOCKET_AUTH_DEFAULT_TIMEOUT)
        .min(SOCKET_AUTH_MAX_TIMEOUT);
    let provider_name = provider.as_str().to_string();
    let flow_response = AuthFlowResponse {
        provider,
        verification_uri: response.verification_uri,
        verification_uri_complete: response.verification_uri_complete,
        user_code: response.user_code,
        expires_in: response.expires_in,
    };
    let monitor = tokio::spawn(async move {
        let started_at = Instant::now();
        loop {
            match client.get_credential(provider_name.as_str()).await {
                Ok(_) => return Ok(()),
                Err(CredentialSocketError::NotFound { .. }) => {
                    if started_at.elapsed() >= timeout {
                        return Err(AuthError::CredentialSocket {
                            provider,
                            message: format!(
                                "timed out waiting for credential after {} seconds",
                                timeout.as_secs()
                            ),
                        });
                    }
                    tokio::time::sleep(SOCKET_AUTH_POLL_INTERVAL).await;
                }
                Err(err) => {
                    return Err(AuthError::CredentialSocket {
                        provider,
                        message: err.to_string(),
                    });
                }
            }
        }
    });
    AuthFlowHandle::new(flow_response, monitor)
}

#[derive(Debug, Clone)]
pub struct GhAuthBroker {
    home_dir: PathBuf,
}

impl Default for GhAuthBroker {
    fn default() -> Self {
        Self {
            home_dir: home_dir_or_cwd(),
        }
    }
}

#[async_trait]
impl AuthBroker for GhAuthBroker {
    fn provider(&self) -> Provider {
        Provider::GitHub
    }

    async fn start_auth(&self) -> Result<AuthFlowHandle, AuthError> {
        let mut command = Command::new("gh");
        command.args([
            "auth",
            "login",
            "--web",
            "--hostname",
            "github.com",
            "--git-protocol",
            "https",
            "--skip-ssh-key",
        ]);
        command.env("GH_BROWSER", "echo");

        start_auth_command(Provider::GitHub, "gh", command, parse_github_auth_line).await
    }

    async fn check_status(&self) -> Result<AuthStatus, AuthError> {
        let mut command = Command::new("gh");
        command.args(["auth", "status", "--hostname", "github.com"]);

        match command.output().await {
            Ok(output) if output.status.success() => {
                let combined = String::from_utf8_lossy(&output.stdout).to_string()
                    + &String::from_utf8_lossy(&output.stderr);
                let login =
                    parse_github_login(&combined).or_else(|| read_github_login(&self.home_dir));
                Ok(AuthStatus::Active { login })
            }
            Ok(_) => Ok(github_status_from_home_dir(&self.home_dir)),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Ok(github_status_from_home_dir(&self.home_dir))
            }
            Err(err) => Err(AuthError::CommandIo {
                provider: Provider::GitHub,
                source: err,
            }),
        }
    }

    async fn disconnect(&self) -> Result<(), AuthError> {
        let mut command = Command::new("gh");
        command.args(["auth", "logout", "--hostname", "github.com"]);
        match command.output().await {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
                let combined_output = format!("{stdout}\n{stderr}");
                if gh_logout_is_already_disconnected(&combined_output) {
                    Ok(())
                } else {
                    let message = if stderr.is_empty() { stdout } else { stderr };
                    Err(AuthError::CommandFailed {
                        provider: Provider::GitHub,
                        message,
                    })
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                Err(AuthError::CommandUnavailable {
                    provider: Provider::GitHub,
                    command: "gh".to_string(),
                })
            }
            Err(err) => Err(AuthError::CommandIo {
                provider: Provider::GitHub,
                source: err,
            }),
        }
    }

    async fn extract_token(&self) -> Option<ProviderToken> {
        extract_github_token(&self.home_dir)
    }
}

#[derive(Debug, Clone)]
pub struct ClaudeAuthBroker {
    config_dir: PathBuf,
    keychain_fallback: bool,
}

impl Default for ClaudeAuthBroker {
    fn default() -> Self {
        Self {
            config_dir: home_dir_or_cwd().join(".claude"),
            keychain_fallback: true,
        }
    }
}

impl ClaudeAuthBroker {
    fn for_profile(config_dir: PathBuf) -> Self {
        Self {
            config_dir,
            keychain_fallback: false,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new("claude");
        command.env("CLAUDE_CONFIG_DIR", &self.config_dir);
        command
    }
}

#[async_trait]
impl AuthBroker for ClaudeAuthBroker {
    fn provider(&self) -> Provider {
        Provider::Claude
    }

    async fn start_auth(&self) -> Result<AuthFlowHandle, AuthError> {
        let mut command = self.command();
        command.args(["auth", "login"]);
        command.env("BROWSER", "echo");
        command.env("CLAUDE_BROWSER", "echo");

        start_auth_command(Provider::Claude, "claude", command, parse_generic_auth_line).await
    }

    async fn check_status(&self) -> Result<AuthStatus, AuthError> {
        let mut command = self.command();
        command.args(["auth", "status"]);

        match command.output().await {
            Ok(output) if output.status.success() => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let login = parse_claude_status_login(&stdout);
                Ok(AuthStatus::Active { login })
            }
            Ok(_) => Ok(AuthStatus::None),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(AuthStatus::None),
            Err(err) => Err(AuthError::CommandIo {
                provider: Provider::Claude,
                source: err,
            }),
        }
    }

    async fn disconnect(&self) -> Result<(), AuthError> {
        let mut command = self.command();
        command.args(["auth", "logout"]);

        // Best-effort CLI logout; always clean up auth files regardless
        let _ = command.output().await;
        self.remove_claude_auth_files()
    }

    async fn extract_token(&self) -> Option<ProviderToken> {
        extract_claude_token_from_config_dir(&self.config_dir).or_else(|| {
            if self.keychain_fallback {
                read_claude_keychain_token()
            } else {
                None
            }
        })
    }
}

impl ClaudeAuthBroker {
    fn remove_claude_auth_files(&self) -> Result<(), AuthError> {
        if !self.config_dir.exists() {
            return Ok(());
        }
        for name in &[".credentials.json", "auth.json", "session-cache"] {
            let path = self.config_dir.join(name);
            if path.exists() {
                remove_path(&path)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CodexAuthBroker {
    codex_home: PathBuf,
    force_file_store: bool,
}

impl Default for CodexAuthBroker {
    fn default() -> Self {
        Self {
            codex_home: home_dir_or_cwd().join(".codex"),
            force_file_store: false,
        }
    }
}

impl CodexAuthBroker {
    fn for_profile(codex_home: PathBuf) -> Self {
        Self {
            codex_home,
            force_file_store: true,
        }
    }

    fn command(&self) -> Command {
        let mut command = Command::new("codex");
        command.env("CODEX_HOME", &self.codex_home);
        command
    }

    fn add_file_store_override(&self, command: &mut Command) {
        if self.force_file_store {
            command.args(["-c", "cli_auth_credentials_store=\"file\""]);
        }
    }

    async fn refresh_access_token(&self) -> Result<(), AuthError> {
        let mut command = self.command();
        self.add_file_store_override(&mut command);
        command.arg("app-server");
        refresh_codex_access_token_with_command(&mut command).await
    }
}

#[async_trait]
impl AuthBroker for CodexAuthBroker {
    fn provider(&self) -> Provider {
        Provider::Codex
    }

    async fn start_auth(&self) -> Result<AuthFlowHandle, AuthError> {
        let mut command = self.command();
        self.add_file_store_override(&mut command);
        command.args(["login", "--device-auth"]);

        start_auth_command(Provider::Codex, "codex", command, parse_generic_auth_line).await
    }

    async fn check_status(&self) -> Result<AuthStatus, AuthError> {
        Ok(
            if extract_codex_token_from_home(&self.codex_home).is_some() {
                AuthStatus::Active { login: None }
            } else {
                AuthStatus::None
            },
        )
    }

    async fn disconnect(&self) -> Result<(), AuthError> {
        let mut command = self.command();
        self.add_file_store_override(&mut command);
        command.arg("logout");

        match command.output().await {
            Ok(output) if output.status.success() => Ok(()),
            Ok(_) | Err(_) => {
                let auth_path = self.codex_home.join("auth.json");
                if auth_path.exists() {
                    remove_path(&auth_path)?;
                }
                Ok(())
            }
        }
    }

    async fn extract_token(&self) -> Option<ProviderToken> {
        extract_codex_token_from_home(&self.codex_home)
    }
}

fn provider_account_broker(
    provider: Provider,
    provider_home: PathBuf,
) -> Result<Arc<dyn AuthBroker>, AuthError> {
    match provider {
        Provider::Claude => Ok(Arc::new(ClaudeAuthBroker::for_profile(provider_home))),
        Provider::Codex => Ok(Arc::new(CodexAuthBroker::for_profile(provider_home))),
        _ => Err(AuthError::UnsupportedProvider(provider.to_string())),
    }
}

pub async fn start_provider_account_auth(
    provider: Provider,
    provider_home: PathBuf,
) -> Result<AuthFlowHandle, AuthError> {
    provider_account_broker(provider, provider_home)?
        .start_auth()
        .await
}

pub async fn provider_account_auth_status(
    provider: Provider,
    provider_home: PathBuf,
) -> Result<AuthStatus, AuthError> {
    provider_account_broker(provider, provider_home)?
        .check_status()
        .await
}

pub async fn disconnect_provider_account_auth(
    provider: Provider,
    provider_home: PathBuf,
) -> Result<(), AuthError> {
    provider_account_broker(provider, provider_home)?
        .disconnect()
        .await
}

pub(crate) async fn prepare_provider_account_access_token(
    provider: Provider,
    provider_home: &Path,
) -> Result<Option<String>, AuthError> {
    let token = match provider {
        Provider::Claude => {
            let broker = ClaudeAuthBroker::for_profile(provider_home.to_path_buf());
            if !matches!(broker.check_status().await?, AuthStatus::Active { .. }) {
                return Ok(None);
            }
            broker.extract_token().await
        }
        Provider::Codex => {
            let broker = CodexAuthBroker::for_profile(provider_home.to_path_buf());
            broker.refresh_access_token().await?;
            broker.extract_token().await
        }
        _ => return Err(AuthError::UnsupportedProvider(provider.to_string())),
    };
    let Some(token) = token else {
        return Ok(None);
    };
    if provider_token_refresh_due(&token, now_unix()) {
        return Err(AuthError::CommandFailed {
            provider,
            message: "provider CLI did not produce an access token valid for the forwarding lease"
                .to_string(),
        });
    }
    Ok(Some(token.access_token))
}

#[derive(Debug, Clone)]
pub struct DopplerAuthBroker;

#[async_trait]
impl AuthBroker for DopplerAuthBroker {
    fn provider(&self) -> Provider {
        Provider::Doppler
    }

    async fn start_auth(&self) -> Result<AuthFlowHandle, AuthError> {
        // If already logged in, short-circuit: extract the token and return
        // a pre-completed handle so the lifecycle task picks it up immediately.
        if let Ok(AuthStatus::Active { .. }) = self.check_status().await {
            let response = AuthFlowResponse {
                provider: Provider::Doppler,
                verification_uri: String::new(),
                verification_uri_complete: None,
                user_code: None,
                expires_in: None,
            };
            let monitor = tokio::spawn(async { Ok(()) });
            return Ok(AuthFlowHandle::new(response, monitor));
        }

        let mut command = Command::new("doppler");
        command.args(["login", "--yes", "--scope", "/"]);
        command.env("BROWSER", "echo");

        start_auth_command(
            Provider::Doppler,
            "doppler",
            command,
            parse_generic_auth_line,
        )
        .await
    }

    async fn check_status(&self) -> Result<AuthStatus, AuthError> {
        let mut command = Command::new("doppler");
        command.args(["configure", "get", "token", "--plain"]);

        match command.output().await {
            Ok(output) if output.status.success() => {
                let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if token.is_empty() {
                    Ok(AuthStatus::None)
                } else {
                    Ok(AuthStatus::Active { login: None })
                }
            }
            Ok(_) => Ok(AuthStatus::None),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(AuthStatus::None),
            Err(err) => Err(AuthError::CommandIo {
                provider: Provider::Doppler,
                source: err,
            }),
        }
    }

    async fn disconnect(&self) -> Result<(), AuthError> {
        let mut command = Command::new("doppler");
        command.arg("logout");
        let _ = command.output().await;
        Ok(())
    }

    async fn extract_token(&self) -> Option<ProviderToken> {
        extract_doppler_token().await
    }
}

async fn extract_doppler_token() -> Option<ProviderToken> {
    let output = Command::new("doppler")
        .args(["configure", "get", "token", "--plain"])
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if token.is_empty() {
        return None;
    }
    Some(ProviderToken {
        provider: Provider::Doppler.as_str().to_string(),
        access_token: token,
        refresh_token: None,
        oauth_client_id: None,
        expires_at: None,
        login: None,
        updated_at: crate::store::rows::now_unix(),
        credential_type: CredentialType::OAuth,
    })
}

#[derive(Debug, Clone)]
pub struct OpenCodeZenBroker {
    home_dir: PathBuf,
}

impl Default for OpenCodeZenBroker {
    fn default() -> Self {
        Self {
            home_dir: home_dir_or_cwd(),
        }
    }
}

#[async_trait]
impl AuthBroker for OpenCodeZenBroker {
    fn provider(&self) -> Provider {
        Provider::OpenCodeZen
    }

    async fn start_auth(&self) -> Result<AuthFlowHandle, AuthError> {
        let response = AuthFlowResponse {
            provider: Provider::OpenCodeZen,
            verification_uri: OPENCODE_AUTH_URL.to_string(),
            verification_uri_complete: Some(OPENCODE_AUTH_URL.to_string()),
            user_code: None,
            expires_in: Some(OPENCODE_AUTH_TIMEOUT.as_secs()),
        };
        let home_dir = self.home_dir.clone();
        let monitor = tokio::spawn(async move {
            let started_at = Instant::now();
            loop {
                if extract_opencode_zen_token(&home_dir).is_some() {
                    return Ok(());
                }
                if started_at.elapsed() >= OPENCODE_AUTH_TIMEOUT {
                    return Err(AuthError::CommandFailed {
                        provider: Provider::OpenCodeZen,
                        message: format!(
                            "timed out waiting for opencode auth at {OPENCODE_AUTH_URL}"
                        ),
                    });
                }
                tokio::time::sleep(OPENCODE_AUTH_POLL_INTERVAL).await;
            }
        });
        Ok(AuthFlowHandle::new(response, monitor))
    }

    async fn check_status(&self) -> Result<AuthStatus, AuthError> {
        Ok(
            if let Some(token) = extract_opencode_zen_token(&self.home_dir) {
                AuthStatus::Active { login: token.login }
            } else {
                AuthStatus::None
            },
        )
    }

    async fn disconnect(&self) -> Result<(), AuthError> {
        remove_opencode_zen_auth_entry(&self.home_dir)
    }

    async fn extract_token(&self) -> Option<ProviderToken> {
        extract_opencode_zen_token(&self.home_dir)
    }
}

#[derive(Debug, Default)]
struct AuthFlowBuilder {
    verification_uri: Option<String>,
    verification_uri_complete: Option<String>,
    user_code: Option<String>,
    expires_in: Option<u64>,
    expects_user_code: bool,
}

struct AuthProcessGroup {
    pid: Arc<AtomicU32>,
}

impl AuthProcessGroup {
    fn new(pid: Option<u32>) -> Self {
        let pid = Arc::new(AtomicU32::new(pid.unwrap_or(0)));
        let interrupt_pid = Arc::clone(&pid);
        crate::engine::agent::register_interrupt_cleanup(move || {
            let pid = interrupt_pid.swap(0, Ordering::AcqRel);
            if pid != 0 {
                kill_auth_process_group(pid);
            }
        });
        Self { pid }
    }

    fn complete(&self) {
        self.pid.store(0, Ordering::Release);
    }
}

impl Drop for AuthProcessGroup {
    fn drop(&mut self) {
        let pid = self.pid.swap(0, Ordering::AcqRel);
        if pid != 0 {
            kill_auth_process_group(pid);
        }
    }
}

fn kill_auth_process_group(pid: u32) {
    #[cfg(unix)]
    // SAFETY: the auth command is spawned into a fresh process group whose id
    // is its pid; a negative pid targets that group and no Loopflow process.
    unsafe {
        libc::kill(-(pid as i32), libc::SIGKILL);
    }
    #[cfg(not(unix))]
    crate::engine::platform::kill_process(pid);
}

async fn start_auth_command(
    provider: Provider,
    command_name: &'static str,
    mut command: Command,
    mut parse_line: impl FnMut(&str, &mut AuthFlowBuilder),
) -> Result<AuthFlowHandle, AuthError> {
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);

    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(AuthError::CommandUnavailable {
                provider,
                command: command_name.to_string(),
            });
        }
        Err(err) => {
            return Err(AuthError::CommandSpawn {
                provider,
                source: err,
            });
        }
    };

    let process_group = AuthProcessGroup::new(child.id());
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AuthError::CommandFailed {
            provider,
            message: "missing stdout pipe".to_string(),
        })?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| AuthError::CommandFailed {
            provider,
            message: "missing stderr pipe".to_string(),
        })?;

    let (line_tx, mut line_rx) = mpsc::unbounded_channel::<String>();
    let stdout_task = spawn_line_reader(stdout, line_tx.clone());
    let stderr_task = spawn_line_reader(stderr, line_tx);

    let started_at = Instant::now();
    let mut builder = AuthFlowBuilder::default();

    loop {
        if started_at.elapsed() >= AUTH_URL_TIMEOUT {
            let _ = child.kill().await;
            let _ = child.wait().await;
            drop(process_group);
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(AuthError::MissingVerificationUrl {
                provider,
                timeout_secs: AUTH_URL_TIMEOUT.as_secs(),
            });
        }

        if let Some(status) = child.try_wait().map_err(|err| AuthError::CommandIo {
            provider,
            source: err,
        })? {
            if let Some(response) = build_flow_response(provider, &builder) {
                let monitor = tokio::spawn(async move {
                    let _ = stdout_task.await;
                    let _ = stderr_task.await;
                    process_group.complete();
                    command_exit_result(provider, status)
                });
                return Ok(AuthFlowHandle::new(response, monitor));
            }

            drop(process_group);
            let _ = stdout_task.await;
            let _ = stderr_task.await;
            return Err(AuthError::CommandFailed {
                provider,
                message: format!("auth command exited before emitting verification URL: {status}"),
            });
        }

        if let Ok(Some(line)) = tokio::time::timeout(AUTH_URL_POLL_INTERVAL, line_rx.recv()).await {
            let normalized_line = strip_ansi_escape_codes(&line);
            parse_line(&normalized_line, &mut builder);
            if let Some(response) = build_flow_response(provider, &builder) {
                let monitor = tokio::spawn(async move {
                    let status = child.wait().await.map_err(|err| AuthError::CommandIo {
                        provider,
                        source: err,
                    })?;
                    let _ = stdout_task.await;
                    let _ = stderr_task.await;
                    process_group.complete();
                    command_exit_result(provider, status)
                });
                return Ok(AuthFlowHandle::new(response, monitor));
            }
        }
    }
}

fn command_exit_result(
    provider: Provider,
    status: std::process::ExitStatus,
) -> Result<(), AuthError> {
    if status.success() {
        Ok(())
    } else {
        Err(AuthError::CommandFailed {
            provider,
            message: format!("auth command exited with status {status}"),
        })
    }
}

fn spawn_line_reader<R>(reader: R, tx: mpsc::UnboundedSender<String>) -> JoinHandle<()>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if tx.send(line).is_err() {
                return;
            }
        }
    })
}

fn parse_github_auth_line(line: &str, builder: &mut AuthFlowBuilder) {
    if let Some(url) = extract_url(line) {
        if !url.contains("github.com/login/device") {
            return;
        }

        if url.contains("user_code=") {
            builder.verification_uri_complete = Some(url.clone());
            builder.verification_uri = Some(strip_query(&url).to_string());
            if builder.user_code.is_none() {
                builder.user_code = extract_user_code(&url);
            }
        } else {
            builder.verification_uri = Some(url);
        }
    }

    if builder.user_code.is_none() {
        builder.user_code = extract_user_code(line);
    }
}

fn parse_generic_auth_line(line: &str, builder: &mut AuthFlowBuilder) {
    let url = extract_url(line);
    if let Some(url) = &url {
        if builder.verification_uri_complete.is_none() {
            builder.verification_uri_complete = Some(url.clone());
        }
        if builder.verification_uri.is_none() {
            builder.verification_uri = Some(strip_query(url).to_string());
        }
        if builder.user_code.is_none() {
            builder.user_code = extract_user_code_from_url(url);
        }
    }

    if builder.user_code.is_none() {
        let text = url
            .as_deref()
            .map_or_else(|| line.to_string(), |url| line.replacen(url, "", 1));
        let asks_for_code = text.to_ascii_lowercase().contains("code");
        if asks_for_code || builder.expects_user_code {
            builder.user_code = extract_user_code(&text);
        }
        builder.expects_user_code =
            builder.user_code.is_none() && (builder.expects_user_code || asks_for_code);
    }

    if builder.expires_in.is_none() {
        builder.expires_in = extract_expires_in(line);
    }
}

fn build_flow_response(provider: Provider, builder: &AuthFlowBuilder) -> Option<AuthFlowResponse> {
    let verification_uri = builder.verification_uri.clone().or_else(|| {
        builder
            .verification_uri_complete
            .clone()
            .map(|url| strip_query(&url).to_string())
    })?;

    let mut verification_uri_complete = builder.verification_uri_complete.clone();
    let mut user_code = builder.user_code.clone();

    if user_code.is_none() {
        user_code = verification_uri_complete
            .as_deref()
            .and_then(extract_user_code_from_url);
    }

    if verification_uri_complete.is_none() {
        if let Some(code) = &user_code {
            verification_uri_complete = Some(format!("{verification_uri}?user_code={code}"));
        }
    }

    Some(AuthFlowResponse {
        provider,
        verification_uri,
        verification_uri_complete,
        user_code,
        expires_in: builder.expires_in,
    })
}

fn extract_url(line: &str) -> Option<String> {
    line.split_whitespace().find_map(|token| {
        let trimmed = token
            .trim_matches(|ch: char| "'\"()[]{}<>,;".contains(ch))
            .trim_end_matches('.')
            .trim_end_matches(',');
        if trimmed.starts_with("https://") || trimmed.starts_with("http://") {
            Some(trimmed.to_string())
        } else {
            None
        }
    })
}

fn strip_ansi_escape_codes(line: &str) -> String {
    ANSI_ESCAPE_RE.replace_all(line, "").to_string()
}

fn strip_query(url: &str) -> &str {
    url.split_once('?').map_or(url, |(base, _)| base)
}

fn extract_user_code(line: &str) -> Option<String> {
    USER_CODE_RE
        .captures(line)
        .and_then(|capture| capture.get(1))
        .map(|value| value.as_str().to_ascii_uppercase())
}

fn extract_user_code_from_url(url: &str) -> Option<String> {
    Url::parse(url)
        .ok()?
        .query_pairs()
        .find_map(|(name, value)| {
            (name == "user_code" && !value.trim().is_empty()).then(|| value.to_ascii_uppercase())
        })
}

fn extract_expires_in(line: &str) -> Option<u64> {
    EXPIRES_IN_RE
        .captures(line)
        .and_then(|capture| capture.get(1))
        .and_then(|value| value.as_str().parse::<u64>().ok())
}

fn parse_claude_status_login(output: &str) -> Option<String> {
    let json: serde_json::Value = serde_json::from_str(output).ok()?;
    if json.get("loggedIn")?.as_bool()? {
        json.get("email")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    } else {
        None
    }
}

fn parse_github_login(output: &str) -> Option<String> {
    GH_LOGIN_RE
        .captures(output)
        .and_then(|capture| capture.get(1))
        .map(|value| value.as_str().to_string())
}

fn github_status_from_home_dir(home_dir: &Path) -> AuthStatus {
    if let Some(login) = read_github_login(home_dir) {
        AuthStatus::Active { login: Some(login) }
    } else {
        AuthStatus::None
    }
}

fn gh_logout_is_already_disconnected(output: &str) -> bool {
    output.to_ascii_lowercase().contains("not logged in")
}

fn read_github_login(home_dir: &Path) -> Option<String> {
    let hosts_path = home_dir.join(".config/gh/hosts.yml");
    let content = fs::read_to_string(hosts_path).ok()?;
    let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content).ok()?;
    let entry = yaml.get("github.com")?;

    if let Some(login) = entry.get("user").and_then(serde_yaml_ng::Value::as_str) {
        return Some(login.to_string());
    }

    Some("github".to_string())
}

fn remove_path(path: &Path) -> Result<(), AuthError> {
    if path.is_dir() {
        fs::remove_dir_all(path)
            .map_err(|err| AuthError::Filesystem(format!("remove {}: {err}", path.display())))
    } else {
        fs::remove_file(path)
            .map_err(|err| AuthError::Filesystem(format!("remove {}: {err}", path.display())))
    }
}

fn home_dir_or_cwd() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("."))
}

fn now_unix() -> i64 {
    time::OffsetDateTime::now_utc().unix_timestamp()
}

fn extract_github_token(home_dir: &Path) -> Option<ProviderToken> {
    let hosts_path = home_dir.join(".config/gh/hosts.yml");
    let content = fs::read_to_string(hosts_path).ok()?;
    let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content).ok()?;
    let entry = yaml.get("github.com")?;
    let token = entry
        .get("oauth_token")
        .and_then(serde_yaml_ng::Value::as_str)?;
    let login = entry
        .get("user")
        .and_then(serde_yaml_ng::Value::as_str)
        .map(String::from);
    let expires_at = entry
        .get("oauth_token_expires_at")
        .or_else(|| entry.get("expires_at"))
        .and_then(parse_expires_at_yaml_value);
    Some(ProviderToken {
        provider: "github".to_string(),
        access_token: token.to_string(),
        refresh_token: None,
        oauth_client_id: None,
        expires_at,
        login,
        updated_at: now_unix(),
        credential_type: CredentialType::OAuth,
    })
}

pub(crate) fn extract_claude_token(home_dir: &Path) -> Option<ProviderToken> {
    let config_dir = home_dir.join(".claude");
    if let Some(token) = extract_claude_token_from_config_dir(&config_dir) {
        return Some(token);
    }
    // The file is absent on machines where Claude Code stashed its OAuth blob in
    // the macOS keychain (service "Claude Code-credentials", JSON under
    // `.claudeAiOauth`). Fall back to reading it there.
    read_claude_keychain_token()
}

fn extract_claude_token_from_config_dir(config_dir: &Path) -> Option<ProviderToken> {
    let cred_path = config_dir.join(".credentials.json");
    if let Ok(content) = fs::read_to_string(cred_path) {
        if let Some(token) = claude_token_from_credentials_json(&content) {
            return Some(token);
        }
    }
    None
}

/// Parse a `.claude/.credentials.json` payload. The access token sits at the top
/// level (`accessToken`); the keychain blob nests it under `claudeAiOauth`, so
/// accept either shape.
fn claude_token_from_credentials_json(content: &str) -> Option<ProviderToken> {
    let json: serde_json::Value = serde_json::from_str(content).ok()?;
    let node = json.get("claudeAiOauth").unwrap_or(&json);
    let token = node.get("accessToken")?.as_str()?;
    let expires_at = read_json_expires_at(
        node,
        &[
            "expiresAt",
            "expires_at",
            "accessTokenExpiresAt",
            "access_token_expires_at",
        ],
    );
    Some(ProviderToken {
        provider: "claude".to_string(),
        access_token: token.to_string(),
        refresh_token: None,
        oauth_client_id: None,
        expires_at,
        login: None,
        updated_at: now_unix(),
        credential_type: CredentialType::OAuth,
    })
}

#[cfg(target_os = "macos")]
fn read_claude_keychain_token() -> Option<ProviderToken> {
    let output = std::process::Command::new("security")
        .args([
            "find-generic-password",
            "-s",
            "Claude Code-credentials",
            "-w",
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let blob = String::from_utf8(output.stdout).ok()?;
    claude_token_from_credentials_json(blob.trim())
}

#[cfg(not(target_os = "macos"))]
fn read_claude_keychain_token() -> Option<ProviderToken> {
    None
}

fn extract_codex_token(home_dir: &Path) -> Option<ProviderToken> {
    extract_codex_token_from_home(&home_dir.join(".codex"))
}

fn extract_codex_token_from_home(codex_home: &Path) -> Option<ProviderToken> {
    let auth_path = codex_home.join("auth.json");
    let content = fs::read_to_string(auth_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    // Store OAuth access tokens only.
    // Never capture manual API keys from Codex auth state.
    let token = json
        .get("access_token")
        .and_then(|v| v.as_str())
        .or_else(|| {
            json.get("tokens")
                .and_then(|value| value.get("access_token"))
                .and_then(|v| v.as_str())
        })?;
    let expires_at = read_json_expires_at(
        &json,
        &[
            "expires_at",
            "expiresAt",
            "accessTokenExpiresAt",
            "access_token_expires_at",
        ],
    );
    Some(ProviderToken {
        provider: "codex".to_string(),
        access_token: token.to_string(),
        refresh_token: None,
        oauth_client_id: None,
        expires_at,
        login: None,
        updated_at: now_unix(),
        credential_type: CredentialType::OAuth,
    })
}

async fn refresh_codex_access_token(codex_home: &Path) -> Result<(), AuthError> {
    let broker = CodexAuthBroker::for_profile(codex_home.to_path_buf());
    broker.refresh_access_token().await
}

async fn refresh_codex_access_token_with_command(command: &mut Command) -> Result<(), AuthError> {
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(unix)]
    command.process_group(0);

    let mut child = command.spawn().map_err(|source| {
        if source.kind() == std::io::ErrorKind::NotFound {
            AuthError::CommandUnavailable {
                provider: Provider::Codex,
                command: "codex".to_string(),
            }
        } else {
            AuthError::CommandSpawn {
                provider: Provider::Codex,
                source,
            }
        }
    })?;
    let _process_group = CodexRefreshProcessGroup::new(child.id());
    let mut stdin = child.stdin.take().ok_or_else(|| AuthError::CommandFailed {
        provider: Provider::Codex,
        message: "app-server did not expose stdin".to_string(),
    })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| AuthError::CommandFailed {
            provider: Provider::Codex,
            message: "app-server did not expose stdout".to_string(),
        })?;
    let mut stdout = BufReader::new(stdout);

    write_codex_auth_request(
        &mut stdin,
        &serde_json::json!({
            "id": 1,
            "method": "initialize",
            "params": {
                "clientInfo": {
                    "name": "loopflow",
                    "title": "loopflow",
                    "version": env!("CARGO_PKG_VERSION"),
                }
            }
        }),
    )
    .await?;
    read_codex_auth_response(&mut stdout, 1).await?;
    write_codex_auth_request(&mut stdin, &serde_json::json!({"method": "initialized"})).await?;
    write_codex_auth_request(
        &mut stdin,
        &serde_json::json!({
            "id": 2,
            "method": "account/read",
            "params": {"refreshToken": true},
        }),
    )
    .await?;
    read_codex_auth_response(&mut stdout, 2).await
}

async fn write_codex_auth_request(
    stdin: &mut tokio::process::ChildStdin,
    request: &serde_json::Value,
) -> Result<(), AuthError> {
    let mut line = serde_json::to_vec(request).map_err(|error| AuthError::CommandFailed {
        provider: Provider::Codex,
        message: format!("failed to encode app-server request: {error}"),
    })?;
    line.push(b'\n');
    stdin
        .write_all(&line)
        .await
        .map_err(|source| AuthError::CommandIo {
            provider: Provider::Codex,
            source,
        })?;
    stdin.flush().await.map_err(|source| AuthError::CommandIo {
        provider: Provider::Codex,
        source,
    })
}

async fn read_codex_auth_response(
    stdout: &mut BufReader<tokio::process::ChildStdout>,
    request_id: i64,
) -> Result<(), AuthError> {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let mut line = String::new();
            let bytes =
                stdout
                    .read_line(&mut line)
                    .await
                    .map_err(|source| AuthError::CommandIo {
                        provider: Provider::Codex,
                        source,
                    })?;
            if bytes == 0 {
                return Err(AuthError::CommandFailed {
                    provider: Provider::Codex,
                    message: "app-server disconnected before refreshing auth".to_string(),
                });
            }
            let Ok(message) = serde_json::from_str::<serde_json::Value>(&line) else {
                continue;
            };
            if message.get("id").and_then(serde_json::Value::as_i64) != Some(request_id) {
                continue;
            }
            if message.get("error").is_some() {
                return Err(AuthError::CommandFailed {
                    provider: Provider::Codex,
                    message: "app-server rejected the proactive token refresh".to_string(),
                });
            }
            return Ok(());
        }
    })
    .await
    .map_err(|_| AuthError::CommandFailed {
        provider: Provider::Codex,
        message: "timed out waiting for app-server auth refresh".to_string(),
    })?
}

struct CodexRefreshProcessGroup {
    pid: Option<u32>,
}

impl CodexRefreshProcessGroup {
    fn new(pid: Option<u32>) -> Self {
        Self { pid }
    }
}

impl Drop for CodexRefreshProcessGroup {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(pid) = self.pid {
            // SAFETY: the child was spawned into a fresh process group whose
            // id is its pid; killing the group also reaps npm-shim descendants.
            unsafe {
                libc::kill(-(pid as i32), libc::SIGKILL);
            }
        }
    }
}

pub(crate) fn extract_codex_access_token(home_dir: &Path) -> Option<String> {
    extract_codex_token(home_dir).map(|token| token.access_token)
}

/// Canonical key under which OpenCode stores credentials in auth.json.
/// Schema: `{"opencode": {"type": "api", "key": "...", "email": "..."}}`
const OPENCODE_AUTH_KEY: &str = "opencode";

fn opencode_auth_path(home_dir: &Path) -> PathBuf {
    home_dir.join(".local/share/opencode/auth.json")
}

fn extract_opencode_zen_token(home_dir: &Path) -> Option<ProviderToken> {
    let (access_token, login, expires_at) = if let Some(key) = read_nonempty_env("OPENCODE_API_KEY")
    {
        (key, None, None)
    } else {
        let (key, login, expires_at) = read_opencode_credential(home_dir)?;
        (key, login, expires_at)
    };
    Some(ProviderToken {
        provider: Provider::OpenCodeZen.as_str().to_string(),
        access_token,
        refresh_token: None,
        oauth_client_id: None,
        expires_at,
        login,
        updated_at: now_unix(),
        credential_type: CredentialType::OAuth,
    })
}

async fn oauth_client_credentials(
    provider: Provider,
    client_id_env: &'static str,
    client_secret_env: &'static str,
) -> Result<(String, String), AuthError> {
    oauth_client_credentials_with_doppler_runner(
        provider,
        client_id_env,
        client_secret_env,
        fetch_doppler_secret,
    )
    .await
}

async fn oauth_client_credentials_with_doppler_runner<F, Fut>(
    provider: Provider,
    client_id_env: &'static str,
    client_secret_env: &'static str,
    mut fetch_secret: F,
) -> Result<(String, String), AuthError>
where
    F: FnMut(&'static str) -> Fut,
    Fut: Future<Output = Option<String>>,
{
    let missing_credentials = || AuthError::CommandUnavailable {
        provider,
        command: format!(
            "set {client_id_env} and {client_secret_env}, or Doppler, to enable {provider} OAuth"
        ),
    };
    let client_id = read_oauth_client_credential(client_id_env, &mut fetch_secret)
        .await
        .ok_or_else(missing_credentials)?;
    let client_secret = read_oauth_client_credential(client_secret_env, &mut fetch_secret)
        .await
        .ok_or_else(missing_credentials)?;
    Ok((client_id, client_secret))
}

async fn read_oauth_client_credential<F, Fut>(
    name: &'static str,
    fetch_secret: &mut F,
) -> Option<String>
where
    F: FnMut(&'static str) -> Fut,
    Fut: Future<Output = Option<String>>,
{
    if let Some(value) = read_nonempty_env(name) {
        return Some(value);
    }
    fetch_secret(name).await
}

async fn fetch_doppler_secret(name: &'static str) -> Option<String> {
    let output = Command::new("doppler")
        .args(["secrets", "get", name, "--plain"])
        .output()
        .await
        .ok()?;
    if !output.status.success() {
        return None;
    }
    read_nonempty_value(&String::from_utf8_lossy(&output.stdout))
}

fn read_nonempty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .and_then(|value| read_nonempty_value(&value))
}

fn read_nonempty_value(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

/// Read OpenCode credential from `~/.local/share/opencode/auth.json`.
/// Returns `(api_key, optional_login, optional_expires_at)`.
fn read_opencode_credential(home_dir: &Path) -> Option<(String, Option<String>, Option<i64>)> {
    let auth_path = opencode_auth_path(home_dir);
    let content = fs::read_to_string(auth_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let entry = json.as_object()?.get(OPENCODE_AUTH_KEY)?.as_object()?;
    let key = entry.get("key").and_then(|v| v.as_str())?.trim();
    if key.is_empty() {
        return None;
    }
    let login = entry
        .get("email")
        .and_then(|v| v.as_str())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(String::from);
    let expires_at = entry
        .get("expires_at")
        .or_else(|| entry.get("expiresAt"))
        .and_then(parse_expires_at_json_value);
    Some((key.to_string(), login, expires_at))
}

fn parse_expiry_from_parts(
    i: Option<i64>,
    u: Option<u64>,
    f: Option<f64>,
    s: Option<&str>,
) -> Option<i64> {
    if let Some(seconds) = i.and_then(normalize_epoch_seconds) {
        return Some(seconds);
    }
    if let Some(seconds) = u
        .and_then(|raw| i64::try_from(raw).ok())
        .and_then(normalize_epoch_seconds)
    {
        return Some(seconds);
    }
    if let Some(seconds) = f.and_then(normalize_epoch_seconds_f64) {
        return Some(seconds);
    }
    s.and_then(parse_expires_at)
}

fn parse_expires_at_yaml_value(value: &serde_yaml_ng::Value) -> Option<i64> {
    parse_expiry_from_parts(
        value.as_i64(),
        value.as_u64(),
        value.as_f64(),
        value.as_str(),
    )
}

fn read_json_expires_at(json: &serde_json::Value, keys: &[&str]) -> Option<i64> {
    let object = json.as_object()?;
    keys.iter()
        .filter_map(|key| object.get(*key))
        .find_map(parse_expires_at_json_value)
}

fn parse_expires_at_json_value(value: &serde_json::Value) -> Option<i64> {
    parse_expiry_from_parts(
        value.as_i64(),
        value.as_u64(),
        value.as_f64(),
        value.as_str(),
    )
}

fn parse_expires_at(raw: &str) -> Option<i64> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(seconds) = trimmed.parse::<i64>() {
        return normalize_epoch_seconds(seconds);
    }
    if let Ok(seconds) = trimmed.parse::<f64>() {
        return normalize_epoch_seconds_f64(seconds);
    }
    time::OffsetDateTime::parse(trimmed, &Rfc3339)
        .ok()
        .map(|timestamp| timestamp.unix_timestamp())
}

fn normalize_epoch_seconds(seconds: i64) -> Option<i64> {
    if seconds <= 0 {
        return None;
    }
    if seconds > 100_000_000_000 {
        return Some(seconds / 1000);
    }
    Some(seconds)
}

fn normalize_epoch_seconds_f64(seconds: f64) -> Option<i64> {
    if !seconds.is_finite() || seconds <= 0.0 {
        return None;
    }
    normalize_epoch_seconds(seconds.floor() as i64)
}

fn oauth_error_message(body: &[u8]) -> Option<String> {
    let payload = serde_json::from_slice::<OAuthErrorResponse>(body).ok()?;
    let description = payload
        .error_description
        .filter(|value| !value.trim().is_empty());
    let error = payload.error.filter(|value| !value.trim().is_empty());
    match (error, description) {
        (Some(error), Some(description)) => Some(format!("{error}: {description}")),
        (Some(error), None) => Some(error),
        (None, Some(description)) => Some(description),
        (None, None) => None,
    }
}

fn remove_opencode_zen_auth_entry(home_dir: &Path) -> Result<(), AuthError> {
    let auth_path = opencode_auth_path(home_dir);
    if !auth_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(&auth_path)
        .map_err(|err| AuthError::Filesystem(format!("read {}: {err}", auth_path.display())))?;
    let mut json: serde_json::Value = serde_json::from_str(&content).map_err(|err| {
        AuthError::Filesystem(format!(
            "parse {} as JSON for opencode credentials: {err}",
            auth_path.display()
        ))
    })?;

    let Some(object) = json.as_object_mut() else {
        return Ok(());
    };

    if object.remove(OPENCODE_AUTH_KEY).is_none() {
        return Ok(());
    }

    if object.is_empty() {
        fs::remove_file(&auth_path).map_err(|err| {
            AuthError::Filesystem(format!("remove {}: {err}", auth_path.display()))
        })?;
        return Ok(());
    }

    let rendered = serde_json::to_string_pretty(&json).map_err(|err| {
        AuthError::Filesystem(format!(
            "serialize {} after removing opencode auth: {err}",
            auth_path.display()
        ))
    })?;
    fs::write(&auth_path, rendered)
        .map_err(|err| AuthError::Filesystem(format!("write {}: {err}", auth_path.display())))
}

async fn refresh_provider_token(provider: Provider) -> Result<ProviderToken, TokenRefreshError> {
    refresh_provider_token_with_runner(provider, &home_dir_or_cwd(), &TokioRefreshCommandRunner)
        .await
}

pub async fn refresh_stored_provider_token(
    provider: Provider,
    current_token: &ProviderToken,
) -> Result<ProviderToken, TokenRefreshError> {
    let mut refreshed = match provider {
        Provider::Linear => {
            let refresh_token = current_token
                .refresh_token
                .as_deref()
                .filter(|token| !token.trim().is_empty())
                .ok_or(TokenRefreshError::OAuth {
                    provider,
                    reason: "stored credential has no refresh token",
                })?;
            refresh_pm_oauth_token(
                provider,
                refresh_token,
                current_token.oauth_client_id.as_deref(),
            )
            .await
            .map_err(|error| TokenRefreshError::OAuth {
                provider,
                reason: pm_refresh_failure_reason(&error),
            })?
        }
        _ => refresh_provider_token(provider).await?,
    };
    preserve_provider_token_metadata(&mut refreshed, current_token);
    Ok(refreshed)
}

pub(crate) fn preserve_provider_token_metadata(
    refreshed: &mut ProviderToken,
    current_token: &ProviderToken,
) {
    if refreshed.refresh_token.is_none() {
        refreshed.refresh_token = current_token.refresh_token.clone();
    }
    if refreshed.oauth_client_id.is_none() {
        refreshed.oauth_client_id = current_token.oauth_client_id.clone();
    }
    if refreshed.login.is_none() {
        refreshed.login = current_token.login.clone();
    }
}

pub(crate) fn provider_token_refresh_due(token: &ProviderToken, now: i64) -> bool {
    token.credential_type == CredentialType::OAuth
        && token
            .expires_at
            .is_some_and(|expires_at| expires_at <= now + TOKEN_REFRESH_LEAD_SECONDS)
}

async fn refresh_provider_token_with_runner(
    provider: Provider,
    home_dir: &Path,
    runner: &dyn RefreshCommandRunner,
) -> Result<ProviderToken, TokenRefreshError> {
    match provider {
        Provider::GitHub => refresh_github_token(home_dir, runner).await,
        Provider::Claude => refresh_claude_token(home_dir),
        Provider::Codex => refresh_codex_token(home_dir).await,
        Provider::OpenCodeZen => {
            extract_opencode_zen_token(home_dir).ok_or(TokenRefreshError::MissingToken {
                provider: Provider::OpenCodeZen,
            })
        }
        Provider::Linear | Provider::Doppler => Err(TokenRefreshError::MissingToken { provider }),
    }
}

#[derive(Debug, Clone, Copy)]
struct PmOAuthEndpoint {
    token_url: &'static str,
    client_id_env: &'static str,
    client_secret_env: &'static str,
}

fn pm_oauth_endpoint(provider: Provider) -> Option<PmOAuthEndpoint> {
    match provider {
        Provider::Linear => Some(PmOAuthEndpoint {
            token_url: LINEAR_OAUTH_TOKEN_URL,
            client_id_env: LINEAR_CLIENT_ID_ENV,
            client_secret_env: LINEAR_CLIENT_SECRET_ENV,
        }),
        _ => None,
    }
}

/// Bind a short-lived loopback listener for an OAuth redirect callback.
fn oauth_callback_listener(
    provider: Provider,
    address: &str,
) -> Result<tokio::net::TcpListener, AuthError> {
    let listener = std::net::TcpListener::bind(address).map_err(|err| AuthError::OAuthRequest {
        provider,
        message: format!("failed to bind {address} for OAuth callback: {err}"),
    })?;
    listener.set_nonblocking(true).ok();
    tokio::net::TcpListener::from_std(listener).map_err(|err| AuthError::OAuthRequest {
        provider,
        message: format!("failed to create async listener: {err}"),
    })
}

/// Wait for the browser to hit the loopback redirect, exchange the returned code
/// for a token, and stash it for the broker's `extract_token`.
async fn monitor_oauth_callback<F, Fut>(
    provider: Provider,
    listener: tokio::net::TcpListener,
    timeout: Duration,
    timeout_message: &'static str,
    completed_token: Arc<Mutex<Option<ProviderToken>>>,
    exchange_code: F,
) -> Result<(), AuthError>
where
    F: Fn(String) -> Fut + Send + 'static,
    Fut: Future<Output = Result<ProviderToken, AuthError>> + Send,
{
    let deadline = Instant::now() + timeout;
    loop {
        if Instant::now() >= deadline {
            return Err(AuthError::CommandFailed {
                provider,
                message: timeout_message.to_string(),
            });
        }

        let accept = tokio::time::timeout(Duration::from_secs(1), listener.accept()).await;
        let (stream, _) = match accept {
            Ok(Ok(conn)) => conn,
            Ok(Err(_)) | Err(_) => continue,
        };
        let Some(code) = read_oauth_callback_code(stream).await else {
            continue;
        };

        match exchange_code(code).await {
            Ok(token) => {
                *completed_token.lock().await = Some(token);
                return Ok(());
            }
            Err(err) => {
                return Err(AuthError::CommandFailed {
                    provider,
                    message: err.to_string(),
                });
            }
        }
    }
}

async fn read_oauth_callback_code(stream: tokio::net::TcpStream) -> Option<String> {
    let (mut reader, mut writer) = stream.into_split();
    let mut buf = vec![0u8; 4096];
    let n = tokio::io::AsyncReadExt::read(&mut reader, &mut buf)
        .await
        .ok()?;
    let request = String::from_utf8_lossy(&buf[..n]);
    let code = extract_oauth_code_from_request(&request);

    let response_body = if code.is_some() {
        "Authenticated! You can close this tab."
    } else {
        "Authentication failed — no code found."
    };
    let http_response = format!(
        "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nConnection: close\r\n\r\n<html><body><h2>{response_body}</h2></body></html>"
    );
    let _ = tokio::io::AsyncWriteExt::write_all(&mut writer, http_response.as_bytes()).await;
    let _ = tokio::io::AsyncWriteExt::shutdown(&mut writer).await;
    code
}

fn extract_oauth_code_from_request(request: &str) -> Option<String> {
    let first_line = request.lines().next()?;
    let path = first_line.split_whitespace().nth(1)?;
    let query = path.split_once('?')?.1;
    for pair in query.split('&') {
        if let Some(value) = pair.strip_prefix("code=") {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[derive(Debug, Deserialize)]
struct OAuthRefreshResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
}

fn pm_refresh_failure_reason(error: &AuthError) -> &'static str {
    match error {
        AuthError::CommandUnavailable { .. } => "OAuth client configuration is unavailable",
        AuthError::OAuthRequest { .. } => {
            "the token endpoint rejected or could not complete the request"
        }
        _ => "the refresh request could not be completed",
    }
}

fn encode_pm_refresh_request(
    provider: Provider,
    client_id: &str,
    client_secret: Option<&str>,
    refresh_token: &str,
) -> Result<String, AuthError> {
    let mut params = vec![
        ("grant_type", "refresh_token"),
        ("client_id", client_id),
        ("refresh_token", refresh_token),
    ];
    if let Some(client_secret) = client_secret {
        params.push(("client_secret", client_secret));
    }
    serde_urlencoded::to_string(params).map_err(|err| AuthError::OAuthRequest {
        provider,
        message: format!("failed to encode refresh request: {err}"),
    })
}

/// Exchange a stored refresh token for a fresh access token via the PM provider's
/// OAuth `grant_type=refresh_token` endpoint. Linear PKCE grants reuse their
/// stored client ID; legacy rows fall back to the configured client credentials.
///
/// The returned token carries `login: None`; callers should preserve the prior login.
///
/// # Errors
/// Returns `AuthError::UnsupportedProvider` for non-PM providers,
/// `AuthError::CommandUnavailable` when client credentials are absent, and
/// `AuthError::OAuthRequest` when the network request or token endpoint rejects it.
pub async fn refresh_pm_oauth_token(
    provider: Provider,
    refresh_token: &str,
    stored_client_id: Option<&str>,
) -> Result<ProviderToken, AuthError> {
    let endpoint = pm_oauth_endpoint(provider)
        .ok_or_else(|| AuthError::UnsupportedProvider(provider.to_string()))?;
    let (client_id, client_secret) = match stored_client_id {
        Some(client_id) if !client_id.trim().is_empty() => (client_id.trim().to_string(), None),
        _ => {
            let (client_id, client_secret) = oauth_client_credentials(
                provider,
                endpoint.client_id_env,
                endpoint.client_secret_env,
            )
            .await?;
            (client_id, Some(client_secret))
        }
    };
    let body = encode_pm_refresh_request(
        provider,
        &client_id,
        client_secret.as_deref(),
        refresh_token,
    )?;

    let response = reqwest::Client::new()
        .post(endpoint.token_url)
        .header("content-type", "application/x-www-form-urlencoded")
        .body(body)
        .send()
        .await
        .map_err(|err| AuthError::OAuthRequest {
            provider,
            message: err.to_string(),
        })?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response
            .bytes()
            .await
            .map_err(|err| AuthError::OAuthRequest {
                provider,
                message: err.to_string(),
            })?;
        let message = oauth_error_message(body.as_ref())
            .unwrap_or_else(|| String::from_utf8_lossy(&body).trim().to_string());
        return Err(AuthError::OAuthRequest {
            provider,
            message: format!("HTTP {status}: {message}"),
        });
    }

    let payload = response
        .json::<OAuthRefreshResponse>()
        .await
        .map_err(|err| AuthError::OAuthRequest {
            provider,
            message: format!("failed to decode refresh response: {err}"),
        })?;

    let expires_at = payload
        .expires_in
        .filter(|seconds| *seconds > 0)
        .map(|seconds| now_unix() + seconds);

    Ok(ProviderToken {
        provider: provider.as_str().to_string(),
        access_token: payload.access_token,
        refresh_token: payload.refresh_token,
        oauth_client_id: Some(client_id),
        expires_at,
        login: None,
        updated_at: now_unix(),
        credential_type: CredentialType::OAuth,
    })
}

async fn refresh_github_token(
    home_dir: &Path,
    runner: &dyn RefreshCommandRunner,
) -> Result<ProviderToken, TokenRefreshError> {
    run_refresh_command(
        Provider::GitHub,
        "gh",
        &["auth", "refresh", "--hostname", "github.com"],
        runner,
        true,
    )
    .await?;

    extract_github_token(home_dir).ok_or(TokenRefreshError::MissingToken {
        provider: Provider::GitHub,
    })
}

fn refresh_claude_token(home_dir: &Path) -> Result<ProviderToken, TokenRefreshError> {
    extract_claude_token(home_dir).ok_or(TokenRefreshError::MissingToken {
        provider: Provider::Claude,
    })
}

async fn refresh_codex_token(home_dir: &Path) -> Result<ProviderToken, TokenRefreshError> {
    let codex_home = home_dir.join(".codex");
    refresh_codex_access_token(&codex_home)
        .await
        .map_err(|error| match error {
            AuthError::CommandUnavailable { command, .. } => {
                TokenRefreshError::CommandUnavailable {
                    provider: Provider::Codex,
                    command,
                }
            }
            AuthError::CommandIo { source, .. } | AuthError::CommandSpawn { source, .. } => {
                TokenRefreshError::CommandIo {
                    provider: Provider::Codex,
                    source,
                }
            }
            error => TokenRefreshError::CommandFailed {
                provider: Provider::Codex,
                message: error.to_string(),
            },
        })?;
    extract_codex_token(home_dir).ok_or(TokenRefreshError::MissingToken {
        provider: Provider::Codex,
    })
}

async fn run_refresh_command(
    provider: Provider,
    program: &'static str,
    args: &'static [&'static str],
    runner: &dyn RefreshCommandRunner,
    fail_on_command_error: bool,
) -> Result<(), TokenRefreshError> {
    match runner.run(program, args).await {
        Ok(output) if output.status.success() => Ok(()),
        Ok(output) if fail_on_command_error => Err(TokenRefreshError::CommandFailed {
            provider,
            message: summarize_command_output(&output),
        }),
        Ok(_) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound && fail_on_command_error => {
            Err(TokenRefreshError::CommandUnavailable {
                provider,
                command: program.to_string(),
            })
        }
        Err(err) if fail_on_command_error => Err(TokenRefreshError::CommandIo {
            provider,
            source: err,
        }),
        Err(_) => Ok(()),
    }
}

fn summarize_command_output(output: &std::process::Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if !stderr.is_empty() {
        return stderr;
    }
    if !stdout.is_empty() {
        return stdout;
    }
    format!("exit status {}", output.status)
}

fn normalize_program_name(program: &str) -> String {
    std::path::Path::new(program)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(program)
        .trim()
        .to_ascii_lowercase()
}

fn normalize_env_name(name: &str) -> String {
    name.trim().to_ascii_uppercase()
}

pub fn provider_env_allowed_for_program(program: &str, env_name: &str) -> bool {
    match env_name {
        "GH_TOKEN" => true,
        "CLAUDE_CODE_OAUTH_TOKEN" => normalize_program_name(program) == "claude",
        "CODEX_ACCESS_TOKEN" => normalize_program_name(program) == "codex",
        "ANTHROPIC_API_KEY" => normalize_program_name(program) == "claude",
        "OPENAI_API_KEY" => normalize_program_name(program) == "codex",
        "OPENCODE_API_KEY" => normalize_program_name(program) == "opencode",
        _ => false,
    }
}

const API_KEY_ENV_NAMES: &[&str] = &[
    "ANTHROPIC_API_KEY",
    "OPENAI_API_KEY",
    "CODEX_API_KEY",
    "GEMINI_API_KEY",
    "OPENCODE_API_KEY",
    "MOONSHOT_API_KEY",
];

pub fn is_api_key_env_name(name: &str) -> bool {
    let normalized = normalize_env_name(name);
    API_KEY_ENV_NAMES.iter().any(|item| *item == normalized)
}

pub fn api_key_env_allowed_for_program(program: &str, env_name: &str) -> bool {
    let env_name = normalize_env_name(env_name);
    if !is_api_key_env_name(&env_name) {
        return true;
    }
    if normalize_program_name(program) != "opencode" {
        return false;
    }
    matches!(env_name.as_str(), "OPENCODE_API_KEY" | "MOONSHOT_API_KEY")
}

pub fn api_key_env_names() -> &'static [&'static str] {
    API_KEY_ENV_NAMES
}

/// Return the env var name and value for a given provider token, based on its
/// credential_type. This is the single decision point for executors.
pub fn env_var_for_token(token: &ProviderToken) -> Option<(String, String)> {
    match (token.provider.as_str(), token.credential_type) {
        ("github", _) => Some(("GH_TOKEN".to_string(), token.access_token.clone())),
        ("claude", CredentialType::OAuth) => Some((
            "CLAUDE_CODE_OAUTH_TOKEN".to_string(),
            token.access_token.clone(),
        )),
        ("claude", CredentialType::ApiKey) => {
            Some(("ANTHROPIC_API_KEY".to_string(), token.access_token.clone()))
        }
        ("codex", CredentialType::OAuth) => {
            Some(("CODEX_ACCESS_TOKEN".to_string(), token.access_token.clone()))
        }
        ("codex", CredentialType::ApiKey) => {
            Some(("OPENAI_API_KEY".to_string(), token.access_token.clone()))
        }
        ("opencodezen", _) => Some(("OPENCODE_API_KEY".to_string(), token.access_token.clone())),
        _ => None,
    }
}

/// Build env vars for all stored provider tokens. Used by executors to inject
/// credentials into agent processes. The env var chosen depends on the token's
/// credential_type (oauth vs apikey).
pub async fn provider_env_vars(store: &crate::store::Store) -> Vec<(String, String)> {
    let tokens = match store.list_provider_tokens().await {
        Ok(tokens) => tokens,
        Err(_) => return Vec::new(),
    };
    let mut vars = Vec::new();
    for token in tokens {
        if let Some(pair) = env_var_for_token(&token) {
            vars.push(pair);
        }
    }
    vars
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::io::{Read, Write};
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::net::UnixListener;
    use std::os::unix::process::ExitStatusExt;
    use std::sync::{Mutex as StdMutex, OnceLock};
    use std::thread;

    use super::*;
    use tempfile::tempdir;

    #[derive(Debug)]
    struct FakeRefreshRunner {
        responses: StdMutex<VecDeque<Result<std::process::Output, std::io::Error>>>,
    }

    impl FakeRefreshRunner {
        fn new(responses: Vec<Result<std::process::Output, std::io::Error>>) -> Self {
            Self {
                responses: StdMutex::new(VecDeque::from(responses)),
            }
        }
    }

    struct EnvGuard {
        vars: Vec<(&'static str, Option<std::ffi::OsString>)>,
    }

    impl EnvGuard {
        fn snapshot(vars: &[&'static str]) -> Self {
            Self {
                vars: vars
                    .iter()
                    .map(|name| (*name, std::env::var_os(name)))
                    .collect(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, value) in &self.vars {
                if let Some(value) = value {
                    std::env::set_var(name, value);
                } else {
                    std::env::remove_var(name);
                }
            }
        }
    }

    fn env_lock() -> &'static StdMutex<()> {
        static LOCK: OnceLock<StdMutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| StdMutex::new(()))
    }

    #[async_trait]
    impl RefreshCommandRunner for FakeRefreshRunner {
        async fn run(
            &self,
            _program: &'static str,
            _args: &'static [&'static str],
        ) -> Result<std::process::Output, std::io::Error> {
            self.responses
                .lock()
                .expect("runner mutex poisoned")
                .pop_front()
                .unwrap_or_else(|| {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::NotFound,
                        "no queued command response",
                    ))
                })
        }
    }

    fn command_output(status_code: i32, stdout: &str, stderr: &str) -> std::process::Output {
        std::process::Output {
            status: std::process::ExitStatus::from_raw(status_code << 8),
            stdout: stdout.as_bytes().to_vec(),
            stderr: stderr.as_bytes().to_vec(),
        }
    }

    fn start_sequenced_socket_server(
        socket_path: PathBuf,
        statuses: Vec<u16>,
    ) -> thread::JoinHandle<()> {
        let _ = std::fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path).expect("bind unix listener");
        thread::spawn(move || {
            for status in statuses {
                let (mut stream, _) = listener.accept().expect("accept unix connection");
                let mut request = [0_u8; 4096];
                let _ = stream.read(&mut request);

                let response = match status {
                    200 => {
                        let body = r#"{"token":"abc123","login":"jack","expires_at":null}"#;
                        format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                            body.len(),
                            body
                        )
                    }
                    404 => "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n".to_string(),
                    other => format!("HTTP/1.1 {other} Error\r\nContent-Length: 0\r\n\r\n"),
                };

                stream
                    .write_all(response.as_bytes())
                    .expect("write HTTP response");
            }
        })
    }

    fn start_socket_server_with_body(socket_path: PathBuf, body: &str) -> thread::JoinHandle<()> {
        let _ = std::fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path).expect("bind unix listener");
        let response_body = body.to_string();
        thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept unix connection");
            let mut request = [0_u8; 4096];
            let _ = stream.read(&mut request);

            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                response_body.len(),
                response_body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write HTTP response");
        })
    }

    #[test]
    fn provider_parses_aliases() {
        assert_eq!("github".parse::<Provider>(), Ok(Provider::GitHub));
        assert_eq!("gh".parse::<Provider>(), Ok(Provider::GitHub));
        assert_eq!("CLAUDE".parse::<Provider>(), Ok(Provider::Claude));
        assert_eq!("codex".parse::<Provider>(), Ok(Provider::Codex));
        assert_eq!("opencodezen".parse::<Provider>(), Ok(Provider::OpenCodeZen));
        assert_eq!("zen".parse::<Provider>(), Ok(Provider::OpenCodeZen));
        assert_eq!("oc".parse::<Provider>(), Ok(Provider::OpenCodeZen));
        assert_eq!("linear".parse::<Provider>(), Ok(Provider::Linear));
        assert_eq!("lin".parse::<Provider>(), Ok(Provider::Linear));
        assert!("gemini".parse::<Provider>().is_err());
    }

    #[test]
    fn provider_api_key_billing_only_marks_metered_agent_providers() {
        assert!(!Provider::GitHub.api_key_bills_per_token());
        assert!(Provider::Claude.api_key_bills_per_token());
        assert!(Provider::Codex.api_key_bills_per_token());
        assert!(Provider::OpenCodeZen.api_key_bills_per_token());
        assert!(!Provider::Linear.api_key_bills_per_token());
    }

    #[test]
    fn github_parser_extracts_url_and_code() {
        let mut builder = AuthFlowBuilder::default();
        parse_github_auth_line(
            "Open this URL https://github.com/login/device and enter code ABCD-1234",
            &mut builder,
        );

        let response = build_flow_response(Provider::GitHub, &builder).expect("response");
        assert_eq!(response.verification_uri, "https://github.com/login/device");
        assert_eq!(response.user_code, Some("ABCD-1234".to_string()));
        assert!(response
            .verification_uri_complete
            .expect("complete url")
            .contains("user_code=ABCD-1234"));
    }

    #[test]
    fn generic_parser_extracts_complete_url() {
        let mut builder = AuthFlowBuilder::default();
        parse_generic_auth_line(
            "Visit https://example.com/device?user_code=QWER-9876 to continue",
            &mut builder,
        );

        let response = build_flow_response(Provider::Codex, &builder).expect("response");
        assert_eq!(response.verification_uri, "https://example.com/device");
        assert_eq!(
            response.verification_uri_complete,
            Some("https://example.com/device?user_code=QWER-9876".to_string())
        );
        assert_eq!(response.user_code, Some("QWER-9876".to_string()));
    }

    #[test]
    fn claude_callback_url_does_not_invent_a_user_code() {
        let mut builder = AuthFlowBuilder::default();
        parse_generic_auth_line(
            "https://claude.com/cai/oauth/authorize?code=true&client_id=9d1c250a-e61b-44d9-88ed-5944d1962f5e&response_type=code&redirect_uri=https%3A%2F%2Fplatform.claude.com%2Foauth%2Fcode%2Fcallback&code_challenge=challenge&state=state",
            &mut builder,
        );

        let response = build_flow_response(Provider::Claude, &builder).expect("response");
        assert_eq!(
            response.verification_uri,
            "https://claude.com/cai/oauth/authorize"
        );
        assert_eq!(response.user_code, None);
    }

    #[tokio::test]
    async fn cancelling_auth_wait_kills_the_provider_process_group() {
        let tmp = tempdir().expect("tempdir");
        let pid_path = tmp.path().join("auth.pid");
        let mut command = Command::new("sh");
        command.env("AUTH_PID_PATH", &pid_path).args([
            "-c",
            "echo $$ > \"$AUTH_PID_PATH\"; echo https://example.com/oauth/authorize; sleep 30",
        ]);

        let handle = start_auth_command(Provider::Claude, "sh", command, parse_generic_auth_line)
            .await
            .expect("start fake auth command");
        let pid: i32 = fs::read_to_string(pid_path)
            .expect("read auth pid")
            .trim()
            .parse()
            .expect("parse auth pid");
        let wait = tokio::spawn(handle.wait());
        tokio::task::yield_now().await;
        wait.abort();
        let _ = wait.await;

        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                // SAFETY: signal 0 is an existence probe and uses no pointers.
                if unsafe { libc::kill(pid, 0) } != 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("auth process should exit when its handle is dropped");
    }

    #[test]
    fn generic_parser_handles_ansi_wrapped_url_and_variable_user_code() {
        let mut builder = AuthFlowBuilder::default();
        let code_line = "Enter this one-time code \u{1b}[90m(expires in 15 minutes)\u{1b}[0m\n\u{1b}[94m1XH6-DG19Y\u{1b}[0m";
        parse_generic_auth_line(&strip_ansi_escape_codes(code_line), &mut builder);
        parse_generic_auth_line(
            &strip_ansi_escape_codes("\u{1b}[94mhttps://auth.openai.com/codex/device\u{1b}[0m"),
            &mut builder,
        );

        let response = build_flow_response(Provider::Codex, &builder).expect("response");
        assert_eq!(
            response.verification_uri,
            "https://auth.openai.com/codex/device"
        );
        assert_eq!(response.user_code, Some("1XH6-DG19Y".to_string()));
    }

    #[test]
    fn github_parser_handles_code_and_url_on_separate_lines() {
        let mut builder = AuthFlowBuilder::default();
        parse_github_auth_line("! First copy your one-time code: 09FB-AAD5", &mut builder);
        parse_github_auth_line(
            "Open this URL to continue in your web browser: https://github.com/login/device",
            &mut builder,
        );

        let response = build_flow_response(Provider::GitHub, &builder).expect("response");
        assert_eq!(response.user_code, Some("09FB-AAD5".to_string()));
        assert_eq!(response.verification_uri, "https://github.com/login/device");
    }

    #[test]
    fn read_github_login_parses_hosts_yml() {
        let temp = tempdir().expect("tempdir");
        let hosts = temp.path().join(".config/gh");
        fs::create_dir_all(&hosts).expect("hosts dir");
        fs::write(
            hosts.join("hosts.yml"),
            "github.com:\n  user: jackdanger\n  oauth_token: test\n",
        )
        .expect("hosts file");

        assert_eq!(
            read_github_login(temp.path()),
            Some("jackdanger".to_string())
        );
    }

    #[test]
    fn claude_status_parses_login_from_json() {
        let output = r#"{"loggedIn":true,"authMethod":"claude.ai","email":"user@example.com"}"#;
        assert_eq!(
            parse_claude_status_login(output),
            Some("user@example.com".to_string())
        );

        let not_logged_in = r#"{"loggedIn":false}"#;
        assert_eq!(parse_claude_status_login(not_logged_in), None);

        assert_eq!(parse_claude_status_login("not json"), None);
    }

    #[test]
    fn gh_logout_detects_already_disconnected_message() {
        assert!(gh_logout_is_already_disconnected(
            "not logged in to any hosts"
        ));
        assert!(!gh_logout_is_already_disconnected("fatal: unknown host"));
    }

    // Requires local claude CLI; keep ignored.
    #[tokio::test]
    #[ignore]
    async fn claude_disconnect_keeps_settings_and_removes_auth_entries() {
        let temp = tempdir().expect("tempdir");
        let claude_dir = temp.path().join(".claude");
        fs::create_dir_all(&claude_dir).expect("claude dir");
        fs::write(claude_dir.join("settings.json"), "{\"theme\":\"dark\"}").expect("settings");
        fs::write(claude_dir.join("auth.json"), "{\"token\":\"abc\"}").expect("auth file");
        fs::create_dir_all(claude_dir.join("session-cache")).expect("session dir");
        fs::write(claude_dir.join("session-cache").join("entry"), "cached").expect("session entry");

        let broker = ClaudeAuthBroker::for_profile(temp.path().join(".claude"));
        broker.disconnect().await.expect("disconnect");

        assert!(claude_dir.join("settings.json").exists());
        assert!(!claude_dir.join("auth.json").exists());
        assert!(!claude_dir.join("session-cache").exists());
    }

    #[tokio::test]
    async fn socket_auth_monitor_waits_until_credential_exists() {
        let temp = tempdir().expect("tempdir");
        let socket_path = temp.path().join("credentials.sock");
        let server = start_sequenced_socket_server(socket_path.clone(), vec![404, 200]);

        let response = AuthStartResponse {
            verification_uri: "https://github.com/login/device".to_string(),
            verification_uri_complete: None,
            user_code: None,
            expires_in: Some(30),
        };
        let client = Arc::new(CredentialSocketClient::new(socket_path));
        let handle = socket_auth_flow_handle(Provider::GitHub, response, client);

        tokio::time::timeout(Duration::from_secs(3), handle.wait())
            .await
            .expect("monitor should complete")
            .expect("credential should eventually be detected");

        server.join().expect("server join");
    }

    #[tokio::test]
    async fn service_reports_pending_when_flow_running() {
        #[derive(Debug)]
        struct FakeBroker;

        #[async_trait]
        impl AuthBroker for FakeBroker {
            fn provider(&self) -> Provider {
                Provider::GitHub
            }

            async fn start_auth(&self) -> Result<AuthFlowHandle, AuthError> {
                let response = AuthFlowResponse {
                    provider: Provider::GitHub,
                    verification_uri: "https://github.com/login/device".to_string(),
                    verification_uri_complete: None,
                    user_code: Some("ABCD-1234".to_string()),
                    expires_in: Some(900),
                };
                let monitor = tokio::spawn(async move {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    Ok(())
                });
                Ok(AuthFlowHandle::new(response, monitor))
            }

            async fn check_status(&self) -> Result<AuthStatus, AuthError> {
                Ok(AuthStatus::Active {
                    login: Some("jackdanger".to_string()),
                })
            }

            async fn disconnect(&self) -> Result<(), AuthError> {
                Ok(())
            }
        }

        let service = ProviderAuthService::with_brokers(vec![Arc::new(FakeBroker)]);
        service
            .start_auth(Provider::GitHub, no_event_sink())
            .await
            .expect("start auth");

        let status = service.status(Provider::GitHub).await.expect("status");
        assert_eq!(status.status, AuthStatus::Pending);
    }

    #[derive(Debug, Clone)]
    struct CompletingBroker {
        provider: Provider,
        token: ProviderToken,
    }

    #[async_trait]
    impl AuthBroker for CompletingBroker {
        fn provider(&self) -> Provider {
            self.provider
        }

        async fn start_auth(&self) -> Result<AuthFlowHandle, AuthError> {
            let response = AuthFlowResponse {
                provider: self.provider,
                verification_uri: "https://github.com/login/device".to_string(),
                verification_uri_complete: None,
                user_code: Some("ABCD-1234".to_string()),
                expires_in: Some(900),
            };
            let monitor = tokio::spawn(async { Ok(()) });
            Ok(AuthFlowHandle::new(response, monitor))
        }

        async fn check_status(&self) -> Result<AuthStatus, AuthError> {
            Ok(AuthStatus::Active {
                login: self.token.login.clone(),
            })
        }

        async fn disconnect(&self) -> Result<(), AuthError> {
            Ok(())
        }

        async fn extract_token(&self) -> Option<ProviderToken> {
            Some(self.token.clone())
        }
    }

    async fn temp_sqlite_store() -> SharedStore {
        let db_path =
            std::env::temp_dir().join(format!("provider-auth-test-{}.db", Uuid::new_v4().simple()));
        Arc::new(
            crate::store::open_store(&crate::store::StorageConfig::sqlite(db_path))
                .await
                .expect("open sqlite store"),
        )
    }

    #[tokio::test]
    async fn start_auth_persists_extracted_token_and_reports_events() {
        let store = temp_sqlite_store().await;
        let token = ProviderToken {
            provider: "github".to_string(),
            access_token: "gho_flow123".to_string(),
            refresh_token: None,
            oauth_client_id: None,
            expires_at: Some(now_unix() + 3600),
            login: Some("jackdanger".to_string()),
            updated_at: now_unix(),
            credential_type: CredentialType::OAuth,
        };
        let broker = CompletingBroker {
            provider: Provider::GitHub,
            token: token.clone(),
        };
        let service = ProviderAuthService::with_brokers_and_store(
            vec![Arc::new(broker)],
            Some(store.clone()),
        );

        let events: Arc<StdMutex<Vec<AuthEvent>>> = Arc::new(StdMutex::new(Vec::new()));
        let events_for_sink = events.clone();
        let sink: AuthEventSink = Arc::new(move |event| {
            events_for_sink.lock().expect("events mutex").push(event);
        });

        service
            .start_auth(Provider::GitHub, sink)
            .await
            .expect("start auth");

        // Wait for the background lifecycle to persist the token.
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if !service.is_pending(Provider::GitHub).await {
                break;
            }
            assert!(Instant::now() < deadline, "auth lifecycle did not finish");
            tokio::time::sleep(Duration::from_millis(10)).await;
        }

        let stored = store
            .get_provider_token("github")
            .await
            .expect("get token")
            .expect("token persisted by flow");
        assert_eq!(stored.access_token, "gho_flow123");
        assert_eq!(stored.login.as_deref(), Some("jackdanger"));
        assert_eq!(stored.credential_type, CredentialType::OAuth);

        let events = events.lock().expect("events mutex");
        assert!(matches!(
            events.first(),
            Some(AuthEvent::FlowStarted { .. })
        ));
        assert!(events
            .iter()
            .any(|event| matches!(event, AuthEvent::Connected { .. })));
    }

    #[tokio::test]
    async fn status_reads_expiry_from_stored_token() {
        let store = temp_sqlite_store().await;
        let expires_at = now_unix() + 7200;
        store
            .upsert_provider_token(&ProviderToken {
                provider: "linear".to_string(),
                access_token: "linear-token".to_string(),
                refresh_token: Some("linear-refresh".to_string()),
                oauth_client_id: Some("linear-client".to_string()),
                expires_at: Some(expires_at),
                login: Some("jack@loopflow.studio".to_string()),
                updated_at: now_unix(),
                credential_type: CredentialType::OAuth,
            })
            .await
            .expect("upsert token");
        let service = ProviderAuthService::with_brokers_and_store(Vec::new(), Some(store));

        let snapshot = service.status(Provider::Linear).await.expect("status");
        assert_eq!(
            snapshot.status,
            AuthStatus::Active {
                login: Some("jack@loopflow.studio".to_string())
            }
        );
        assert_eq!(snapshot.expires_at, Some(expires_at));
        assert_eq!(
            snapshot.next_refresh_at,
            Some(expires_at - TOKEN_REFRESH_LEAD_SECONDS)
        );
        assert_eq!(snapshot.credential_type, Some(CredentialType::OAuth));
    }

    #[tokio::test]
    async fn status_marks_stored_token_expired() {
        let store = temp_sqlite_store().await;
        store
            .upsert_provider_token(&ProviderToken {
                provider: "claude".to_string(),
                access_token: "stale".to_string(),
                refresh_token: None,
                oauth_client_id: None,
                expires_at: Some(now_unix() - 60),
                login: None,
                updated_at: now_unix(),
                credential_type: CredentialType::OAuth,
            })
            .await
            .expect("upsert token");
        let service = ProviderAuthService::with_brokers_and_store(Vec::new(), Some(store));

        let snapshot = service.status(Provider::Claude).await.expect("status");
        assert_eq!(snapshot.status, AuthStatus::Expired);
    }

    #[test]
    fn api_key_env_filter_is_harness_specific() {
        assert!(api_key_env_allowed_for_program(
            "opencode",
            "OPENCODE_API_KEY"
        ));
        assert!(api_key_env_allowed_for_program(
            "/usr/local/bin/opencode",
            "MOONSHOT_API_KEY"
        ));
        assert!(!api_key_env_allowed_for_program(
            "opencode",
            "ANTHROPIC_API_KEY"
        ));
        assert!(!api_key_env_allowed_for_program(
            "opencode",
            "OPENAI_API_KEY"
        ));
        assert!(!api_key_env_allowed_for_program(
            "claude",
            "OPENCODE_API_KEY"
        ));
        assert!(!api_key_env_allowed_for_program(
            "codex",
            "MOONSHOT_API_KEY"
        ));
    }

    #[test]
    fn provider_env_allowed_is_harness_specific() {
        assert!(provider_env_allowed_for_program("claude", "GH_TOKEN"));
        assert!(provider_env_allowed_for_program("codex", "GH_TOKEN"));
        assert!(provider_env_allowed_for_program(
            "claude",
            "CLAUDE_CODE_OAUTH_TOKEN"
        ));
        assert!(provider_env_allowed_for_program(
            "codex",
            "CODEX_ACCESS_TOKEN"
        ));
        assert!(provider_env_allowed_for_program(
            "opencode",
            "OPENCODE_API_KEY"
        ));
        assert!(!provider_env_allowed_for_program(
            "codex",
            "CLAUDE_CODE_OAUTH_TOKEN"
        ));
        assert!(!provider_env_allowed_for_program(
            "claude",
            "OPENCODE_API_KEY"
        ));
    }

    #[test]
    fn extract_codex_token_ignores_manual_api_keys() {
        let tmp = tempdir().expect("tempdir");
        let codex_dir = tmp.path().join(".codex");
        fs::create_dir_all(&codex_dir).expect("create codex dir");
        fs::write(
            codex_dir.join("auth.json"),
            r#"{"api_key":"sk-live-manual-key"}"#,
        )
        .expect("write auth json");

        let token = extract_codex_token(tmp.path());
        assert!(token.is_none());
    }

    #[test]
    fn extract_codex_token_reads_oauth_access_token() {
        let tmp = tempdir().expect("tempdir");
        let codex_dir = tmp.path().join(".codex");
        fs::create_dir_all(&codex_dir).expect("create codex dir");
        fs::write(
            codex_dir.join("auth.json"),
            r#"{"access_token":"oauth-access-token"}"#,
        )
        .expect("write auth json");

        let token = extract_codex_token(tmp.path()).expect("oauth token should load");
        assert_eq!(token.provider, "codex");
        assert_eq!(token.access_token, "oauth-access-token");
    }

    #[test]
    fn extract_codex_token_reads_nested_chatgpt_access_token() {
        let tmp = tempdir().expect("tempdir");
        let codex_dir = tmp.path().join(".codex");
        fs::create_dir_all(&codex_dir).expect("create codex dir");
        fs::write(
            codex_dir.join("auth.json"),
            r#"{"auth_mode":"chatgpt","tokens":{"access_token":"nested-oauth-token"}}"#,
        )
        .expect("write auth json");

        let token = extract_codex_token(tmp.path()).expect("oauth token should load");
        assert_eq!(token.provider, "codex");
        assert_eq!(token.access_token, "nested-oauth-token");
    }

    #[test]
    fn extract_codex_token_parses_rfc3339_expiry() {
        let tmp = tempdir().expect("tempdir");
        let codex_dir = tmp.path().join(".codex");
        fs::create_dir_all(&codex_dir).expect("create codex dir");
        fs::write(
            codex_dir.join("auth.json"),
            r#"{"access_token":"oauth-access-token","expires_at":"2030-01-01T00:00:00Z"}"#,
        )
        .expect("write auth json");

        let token = extract_codex_token(tmp.path()).expect("oauth token should load");
        assert_eq!(token.expires_at, Some(1_893_456_000));
    }

    #[test]
    fn extract_claude_token_parses_epoch_millis_expiry() {
        let tmp = tempdir().expect("tempdir");
        let claude_dir = tmp.path().join(".claude");
        fs::create_dir_all(&claude_dir).expect("create claude dir");
        fs::write(
            claude_dir.join(".credentials.json"),
            r#"{"accessToken":"claude-token","expiresAt":"1893456000000"}"#,
        )
        .expect("write credentials");

        let token = extract_claude_token(tmp.path()).expect("claude token should load");
        assert_eq!(token.expires_at, Some(1_893_456_000));
    }

    #[test]
    fn linear_oauth_authorization_url_uses_loopback_redirect_and_pkce() {
        let app = LinearOAuthApp {
            client_id: "client-123".to_string(),
            client_secret: "secret-456".to_string(),
            scope: LINEAR_OAUTH_DEFAULT_SCOPE.to_string(),
        };

        let url = LinearOAuthBroker::build_authorization_url(&app, "verifier-123", "state-abc");
        let parsed = Url::parse(&url).expect("linear oauth url should parse");
        let query = parsed.query_pairs().collect::<HashMap<_, _>>();

        assert_eq!(
            parsed.as_str().split('?').next(),
            Some(LINEAR_OAUTH_AUTHORIZE_URL)
        );
        assert_eq!(
            query.get("client_id").map(|value| value.as_ref()),
            Some("client-123")
        );
        assert_eq!(
            query.get("redirect_uri").map(|value| value.as_ref()),
            Some(LINEAR_OAUTH_REDIRECT_URI)
        );
        assert_eq!(
            query.get("scope").map(|value| value.as_ref()),
            Some("read,write")
        );
        assert!(query.contains_key("code_challenge"));
        assert_eq!(
            query
                .get("code_challenge_method")
                .map(|value| value.as_ref()),
            Some("S256")
        );
    }

    #[test]
    fn linear_provider_parses_and_supports_oauth_refresh() {
        assert_eq!("linear".parse::<Provider>(), Ok(Provider::Linear));
        assert_eq!("lin".parse::<Provider>(), Ok(Provider::Linear));
        assert_eq!(Provider::Linear.as_str(), "linear");
        assert!(!Provider::Linear.api_key_bills_per_token());
        assert_eq!(
            Provider::Linear.api_key_configure_error(),
            Some("Linear requires OAuth. Run 'lf auth linear' to connect.")
        );
        // Refresh is wired: Linear resolves a PM OAuth endpoint.
        assert!(pm_oauth_endpoint(Provider::Linear).is_some());
        assert!(Provider::Linear.supports_automatic_refresh());
    }

    #[test]
    fn linear_broker_registered_in_default_brokers() {
        let has_linear = default_brokers(None)
            .iter()
            .any(|broker| broker.provider() == Provider::Linear);
        assert!(has_linear, "`lf auth linear` needs a registered broker");
    }

    #[tokio::test(flavor = "current_thread")]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn oauth_client_credentials_prefers_env_over_doppler() {
        const CLIENT_ID_ENV: &str = "LOOPFLOW_TEST_LINEAR_CLIENT_ID";
        const CLIENT_SECRET_ENV: &str = "LOOPFLOW_TEST_LINEAR_CLIENT_SECRET";

        let _lock = env_lock().lock().expect("env lock");
        let _env = EnvGuard::snapshot(&[CLIENT_ID_ENV, CLIENT_SECRET_ENV]);
        std::env::set_var(CLIENT_ID_ENV, " env-client ");
        std::env::set_var(CLIENT_SECRET_ENV, " env-secret ");

        let mut doppler_calls = Vec::new();
        let credentials = oauth_client_credentials_with_doppler_runner(
            Provider::Linear,
            CLIENT_ID_ENV,
            CLIENT_SECRET_ENV,
            |name| {
                doppler_calls.push(name);
                std::future::ready(None)
            },
        )
        .await
        .expect("env credentials should resolve");

        assert_eq!(
            credentials,
            ("env-client".to_string(), "env-secret".to_string())
        );
        assert!(doppler_calls.is_empty());
    }

    #[tokio::test(flavor = "current_thread")]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn oauth_client_credentials_falls_back_to_doppler() {
        const CLIENT_ID_ENV: &str = "LOOPFLOW_TEST_LINEAR_CLIENT_ID";
        const CLIENT_SECRET_ENV: &str = "LOOPFLOW_TEST_LINEAR_CLIENT_SECRET";

        let _lock = env_lock().lock().expect("env lock");
        let _env = EnvGuard::snapshot(&[CLIENT_ID_ENV, CLIENT_SECRET_ENV]);
        std::env::remove_var(CLIENT_ID_ENV);
        std::env::remove_var(CLIENT_SECRET_ENV);

        let mut doppler_calls = Vec::new();
        let credentials = oauth_client_credentials_with_doppler_runner(
            Provider::Linear,
            CLIENT_ID_ENV,
            CLIENT_SECRET_ENV,
            |name| {
                doppler_calls.push(name);
                std::future::ready(match name {
                    CLIENT_ID_ENV => Some("doppler-client".to_string()),
                    CLIENT_SECRET_ENV => Some("doppler-secret".to_string()),
                    _ => None,
                })
            },
        )
        .await
        .expect("doppler credentials should resolve");

        assert_eq!(
            credentials,
            ("doppler-client".to_string(), "doppler-secret".to_string())
        );
        assert_eq!(doppler_calls, vec![CLIENT_ID_ENV, CLIENT_SECRET_ENV]);
    }

    #[tokio::test(flavor = "current_thread")]
    #[allow(clippy::await_holding_lock)] // the env lock is the test serializer
    async fn oauth_client_credentials_returns_unavailable_when_env_and_doppler_miss() {
        const CLIENT_ID_ENV: &str = "LOOPFLOW_TEST_LINEAR_CLIENT_ID";
        const CLIENT_SECRET_ENV: &str = "LOOPFLOW_TEST_LINEAR_CLIENT_SECRET";

        let _lock = env_lock().lock().expect("env lock");
        let _env = EnvGuard::snapshot(&[CLIENT_ID_ENV, CLIENT_SECRET_ENV]);
        std::env::remove_var(CLIENT_ID_ENV);
        std::env::remove_var(CLIENT_SECRET_ENV);

        let result = oauth_client_credentials_with_doppler_runner(
            Provider::Linear,
            CLIENT_ID_ENV,
            CLIENT_SECRET_ENV,
            |_| std::future::ready(None),
        )
        .await;

        let Err(AuthError::CommandUnavailable { provider, command }) = result else {
            panic!("expected missing OAuth client credentials");
        };
        assert_eq!(provider, Provider::Linear);
        assert!(command.contains(CLIENT_ID_ENV));
        assert!(command.contains(CLIENT_SECRET_ENV));
        assert!(command.contains("Doppler"));
    }

    #[test]
    fn oauth_error_message_prefers_description() {
        let message = oauth_error_message(
            br#"{"error":"invalid_grant","error_description":"authorization code expired"}"#,
        )
        .expect("oauth error should parse");

        assert_eq!(message, "invalid_grant: authorization code expired");
    }

    #[test]
    fn pm_oauth_endpoint_maps_only_pm_providers() {
        assert_eq!(
            pm_oauth_endpoint(Provider::Linear).map(|e| e.token_url),
            Some(LINEAR_OAUTH_TOKEN_URL)
        );
        assert!(pm_oauth_endpoint(Provider::GitHub).is_none());
    }

    #[test]
    fn pkce_refresh_request_uses_client_id_without_secret() {
        let body =
            encode_pm_refresh_request(Provider::Linear, "linear-client", None, "refresh-token")
                .expect("encode refresh request");
        let params: HashMap<String, String> =
            serde_urlencoded::from_str(&body).expect("decode refresh request");

        assert_eq!(
            params.get("grant_type").map(String::as_str),
            Some("refresh_token")
        );
        assert_eq!(
            params.get("client_id").map(String::as_str),
            Some("linear-client")
        );
        assert_eq!(
            params.get("refresh_token").map(String::as_str),
            Some("refresh-token")
        );
        assert!(!params.contains_key("client_secret"));
    }

    #[test]
    fn refreshed_token_preserves_or_rotates_grant_metadata() {
        let current = ProviderToken {
            provider: "linear".to_string(),
            access_token: "old-access".to_string(),
            refresh_token: Some("old-refresh".to_string()),
            oauth_client_id: Some("linear-client".to_string()),
            expires_at: Some(now_unix()),
            login: Some("user@example.com".to_string()),
            updated_at: now_unix(),
            credential_type: CredentialType::OAuth,
        };
        let mut omitted = ProviderToken {
            access_token: "new-access".to_string(),
            refresh_token: None,
            oauth_client_id: None,
            login: None,
            ..current.clone()
        };
        preserve_provider_token_metadata(&mut omitted, &current);
        assert_eq!(omitted.refresh_token.as_deref(), Some("old-refresh"));
        assert_eq!(omitted.oauth_client_id.as_deref(), Some("linear-client"));
        assert_eq!(omitted.login.as_deref(), Some("user@example.com"));

        let mut rotated = ProviderToken {
            refresh_token: Some("new-refresh".to_string()),
            ..omitted
        };
        preserve_provider_token_metadata(&mut rotated, &current);
        assert_eq!(rotated.refresh_token.as_deref(), Some("new-refresh"));
    }

    #[test]
    fn oauth_tokens_refresh_before_expiry_but_api_keys_do_not() {
        let now = now_unix();
        let mut token = make_token("linear", CredentialType::OAuth);
        token.expires_at = Some(now + TOKEN_REFRESH_LEAD_SECONDS);
        assert!(provider_token_refresh_due(&token, now));

        token.expires_at = Some(now + TOKEN_REFRESH_LEAD_SECONDS + 1);
        assert!(!provider_token_refresh_due(&token, now));

        token.credential_type = CredentialType::ApiKey;
        token.expires_at = Some(now - 1);
        assert!(!provider_token_refresh_due(&token, now));
    }

    #[tokio::test]
    async fn linear_refresh_without_refresh_token_is_actionable_and_secret_free() {
        let token = make_token("linear", CredentialType::OAuth);
        let error = refresh_stored_provider_token(Provider::Linear, &token)
            .await
            .expect_err("missing refresh token should fail");

        assert_eq!(
            error.to_string(),
            "linear OAuth refresh failed: stored credential has no refresh token"
        );
    }

    #[tokio::test]
    async fn socket_broker_extract_token_parses_expiry() {
        let tmp = tempdir().expect("tempdir");
        let socket_path = tmp.path().join("credentials.sock");
        let server = start_socket_server_with_body(
            socket_path.clone(),
            r#"{"token":"abc123","login":"jack","expires_at":"2030-01-01T00:00:00Z"}"#,
        );
        let broker = SocketAuthBroker::new(
            Provider::GitHub,
            Arc::new(CredentialSocketClient::new(socket_path)),
        );

        let token = broker.extract_token().await.expect("token should extract");

        assert_eq!(token.provider, "github");
        assert_eq!(token.access_token, "abc123");
        assert_eq!(token.login, Some("jack".to_string()));
        assert_eq!(token.expires_at, Some(1_893_456_000));
        server.join().expect("server join");
    }

    #[test]
    fn extract_opencode_zen_token_reads_auth_file() {
        let tmp = tempdir().expect("tempdir");
        let auth_path = tmp.path().join(".local/share/opencode/auth.json");
        fs::create_dir_all(auth_path.parent().expect("auth parent")).expect("auth parent dir");
        fs::write(
            &auth_path,
            r#"{"opencode":{"type":"api","key":"opencode-file-key","email":"user@example.com"}}"#,
        )
        .expect("write opencode auth json");

        let token = extract_opencode_zen_token(tmp.path()).expect("file token should load");
        assert_eq!(token.provider, "opencodezen");
        assert_eq!(token.access_token, "opencode-file-key");
        assert_eq!(token.login.as_deref(), Some("user@example.com"));
    }

    #[test]
    fn remove_opencode_zen_auth_entry_deletes_provider_key() {
        let tmp = tempdir().expect("tempdir");
        let auth_path = tmp.path().join(".local/share/opencode/auth.json");
        fs::create_dir_all(auth_path.parent().expect("auth parent")).expect("auth parent dir");
        fs::write(
            &auth_path,
            r#"{"opencode":{"type":"api","key":"opencode-file-key"},"anthropic":{"type":"api","key":"anthropic-key"}}"#,
        )
        .expect("write opencode auth json");

        remove_opencode_zen_auth_entry(tmp.path()).expect("remove zen auth entry");

        let updated = fs::read_to_string(&auth_path).expect("read updated auth file");
        let json: serde_json::Value = serde_json::from_str(&updated).expect("parse updated auth");
        let object = json.as_object().expect("auth root object");
        assert!(!object.contains_key("opencode"));
        assert!(object.contains_key("anthropic"));
    }

    #[tokio::test]
    async fn provider_env_vars_includes_opencode_zen_token_for_opencode_harness() {
        let tmp = tempdir().expect("tempdir");
        let store = crate::store::open_store(&crate::store::StorageConfig::sqlite(
            tmp.path().join("loopflow.db"),
        ))
        .await
        .expect("open sqlite store");
        store
            .upsert_provider_token(&ProviderToken {
                provider: "opencodezen".to_string(),
                access_token: "opencode-key".to_string(),
                refresh_token: None,
                oauth_client_id: None,
                expires_at: None,
                login: Some("user@example.com".to_string()),
                updated_at: now_unix(),
                credential_type: CredentialType::OAuth,
            })
            .await
            .expect("upsert provider token");

        let env_vars = provider_env_vars(&store).await;
        assert!(env_vars
            .iter()
            .any(|(name, value)| name == "OPENCODE_API_KEY" && value == "opencode-key"));
    }

    #[tokio::test]
    async fn codex_refresh_uses_app_server_managed_auth_flow() {
        let tmp = tempdir().expect("tempdir");
        let script = tmp.path().join("codex-app-server");
        let trace = tmp.path().join("requests.jsonl");
        fs::write(
            &script,
            r#"#!/bin/sh
trace="$1"
IFS= read -r line
printf '%s\n' "$line" >> "$trace"
printf '{"id":1,"result":{}}\n'
IFS= read -r line
printf '%s\n' "$line" >> "$trace"
IFS= read -r line
printf '%s\n' "$line" >> "$trace"
printf '{"id":2,"result":{"account":null}}\n'
"#,
        )
        .expect("write fake app-server");
        let mut permissions = fs::metadata(&script)
            .expect("script metadata")
            .permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&script, permissions).expect("make script executable");

        let mut command = Command::new(&script);
        command.arg(&trace);
        refresh_codex_access_token_with_command(&mut command)
            .await
            .expect("refresh through fake app-server");

        let requests = fs::read_to_string(trace).expect("read request trace");
        let requests = requests
            .lines()
            .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("request json"))
            .collect::<Vec<_>>();
        assert_eq!(requests[0]["method"], "initialize");
        assert_eq!(requests[1]["method"], "initialized");
        assert_eq!(requests[2]["method"], "account/read");
        assert_eq!(requests[2]["params"]["refreshToken"], true);
    }

    #[tokio::test]
    async fn codex_refresh_reports_missing_cli() {
        let tmp = tempdir().expect("tempdir");
        let mut command = Command::new(tmp.path().join("missing-codex"));
        let error = refresh_codex_access_token_with_command(&mut command)
            .await
            .expect_err("missing app-server should fail");
        assert!(matches!(error, AuthError::CommandUnavailable { .. }));
    }

    #[tokio::test]
    async fn refresh_github_token_requires_successful_refresh_command() {
        let tmp = tempdir().expect("tempdir");
        let hosts = tmp.path().join(".config/gh");
        fs::create_dir_all(&hosts).expect("hosts dir");
        fs::write(
            hosts.join("hosts.yml"),
            "github.com:\n  user: jackdanger\n  oauth_token: refreshed\n",
        )
        .expect("write hosts");
        let runner = FakeRefreshRunner::new(vec![Ok(command_output(1, "", "refresh failed"))]);

        let result =
            refresh_provider_token_with_runner(Provider::GitHub, tmp.path(), &runner).await;

        assert!(matches!(
            result,
            Err(TokenRefreshError::CommandFailed {
                provider: Provider::GitHub,
                ..
            })
        ));
    }

    #[tokio::test]
    async fn refresh_github_token_extracts_updated_token_after_refresh() {
        let tmp = tempdir().expect("tempdir");
        let hosts = tmp.path().join(".config/gh");
        fs::create_dir_all(&hosts).expect("hosts dir");
        fs::write(
            hosts.join("hosts.yml"),
            "github.com:\n  user: jackdanger\n  oauth_token: refreshed-token\n",
        )
        .expect("write hosts");
        let runner = FakeRefreshRunner::new(vec![Ok(command_output(0, "ok", ""))]);

        let token = refresh_provider_token_with_runner(Provider::GitHub, tmp.path(), &runner).await;
        let token = token.expect("github refresh should succeed");
        assert_eq!(token.provider, "github");
        assert_eq!(token.access_token, "refreshed-token");
        assert_eq!(token.login, Some("jackdanger".to_string()));
    }

    fn make_token(provider: &str, credential_type: CredentialType) -> ProviderToken {
        ProviderToken {
            provider: provider.to_string(),
            access_token: "test-token".to_string(),
            refresh_token: None,
            oauth_client_id: None,
            expires_at: None,
            login: None,
            updated_at: now_unix(),
            credential_type,
        }
    }

    #[test]
    fn pm_providers_do_not_support_api_key_env_auth() {
        assert_eq!(Provider::Linear.api_key_env_name(), None);
    }

    #[test]
    fn pm_provider_configure_errors_point_to_oauth() {
        assert_eq!(
            Provider::Linear.api_key_configure_error(),
            Some("Linear requires OAuth. Run 'lf auth linear' to connect.")
        );
        assert_eq!(Provider::Claude.api_key_configure_error(), None);
    }

    #[test]
    fn env_var_for_token_claude_oauth_returns_oauth_token() {
        let token = make_token("claude", CredentialType::OAuth);
        let (name, _) = env_var_for_token(&token).expect("should produce env var");
        assert_eq!(name, "CLAUDE_CODE_OAUTH_TOKEN");
    }

    #[test]
    fn env_var_for_token_claude_apikey_returns_api_key() {
        let token = make_token("claude", CredentialType::ApiKey);
        let (name, _) = env_var_for_token(&token).expect("should produce env var");
        assert_eq!(name, "ANTHROPIC_API_KEY");
    }

    #[test]
    fn env_var_for_token_codex_oauth_returns_access_token() {
        let token = make_token("codex", CredentialType::OAuth);
        let (name, _) = env_var_for_token(&token).expect("should produce env var");
        assert_eq!(name, "CODEX_ACCESS_TOKEN");
    }

    #[test]
    fn env_var_for_token_codex_apikey_returns_openai_key() {
        let token = make_token("codex", CredentialType::ApiKey);
        let (name, _) = env_var_for_token(&token).expect("should produce env var");
        assert_eq!(name, "OPENAI_API_KEY");
    }

    #[test]
    fn env_var_for_token_github_always_returns_gh_token() {
        for ct in [CredentialType::OAuth, CredentialType::ApiKey] {
            let token = make_token("github", ct);
            let (name, _) = env_var_for_token(&token).expect("should produce env var");
            assert_eq!(name, "GH_TOKEN");
        }
    }

    #[test]
    fn env_var_for_token_opencodezen_always_returns_opencode_key() {
        for ct in [CredentialType::OAuth, CredentialType::ApiKey] {
            let token = make_token("opencodezen", ct);
            let (name, _) = env_var_for_token(&token).expect("should produce env var");
            assert_eq!(name, "OPENCODE_API_KEY");
        }
    }

    #[tokio::test]
    async fn provider_env_vars_returns_correct_vars_for_mixed_credential_types() {
        let tmp = tempdir().expect("tempdir");
        let store = crate::store::open_store(&crate::store::StorageConfig::sqlite(
            tmp.path().join("loopflow.db"),
        ))
        .await
        .expect("open sqlite store");

        // Claude with API key
        store
            .upsert_provider_token(&make_token("claude", CredentialType::ApiKey))
            .await
            .expect("upsert claude apikey");

        // GitHub with OAuth
        store
            .upsert_provider_token(&make_token("github", CredentialType::OAuth))
            .await
            .expect("upsert github oauth");

        // Codex with OAuth uses the supported process-lifetime access token.
        store
            .upsert_provider_token(&make_token("codex", CredentialType::OAuth))
            .await
            .expect("upsert codex oauth");

        let vars = provider_env_vars(&store).await;

        assert!(vars.iter().any(|(n, _)| n == "ANTHROPIC_API_KEY"));
        assert!(vars.iter().any(|(n, _)| n == "GH_TOKEN"));
        assert!(!vars.iter().any(|(n, _)| n == "CLAUDE_CODE_OAUTH_TOKEN"));
        assert!(vars.iter().any(|(n, _)| n == "CODEX_ACCESS_TOKEN"));
        assert!(!vars.iter().any(|(n, _)| n == "OPENAI_API_KEY"));
    }
}
