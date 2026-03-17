use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tracing::warn;

use crate::lfd::credential_socket::{
    AuthStartResponse, CredentialSocketClient, CredentialSocketError,
};
use crate::lfd::events::EventHub;
use crate::lfd::store::{CredentialType, ProviderToken, SharedStore};
use crate::lfd::types::Event;

const AUTH_URL_TIMEOUT: Duration = Duration::from_secs(20);
const AUTH_URL_POLL_INTERVAL: Duration = Duration::from_millis(200);
const SOCKET_AUTH_POLL_INTERVAL: Duration = Duration::from_secs(1);
const SOCKET_AUTH_DEFAULT_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const SOCKET_AUTH_MAX_TIMEOUT: Duration = Duration::from_secs(60 * 60);
const OPENCODE_AUTH_POLL_INTERVAL: Duration = Duration::from_secs(1);
const OPENCODE_AUTH_TIMEOUT: Duration = Duration::from_secs(15 * 60);
const OPENCODE_AUTH_URL: &str = "https://opencode.ai/auth";
const TOKEN_REFRESH_LEAD_SECONDS: i64 = 20 * 60;

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
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GitHub => "github",
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::OpenCodeZen => "opencodezen",
        }
    }

    pub fn all() -> [Self; 4] {
        [Self::GitHub, Self::Claude, Self::Codex, Self::OpenCodeZen]
    }

    /// Whether this provider supports CLI-based token refresh.
    /// Providers that only re-read files (Claude, OpenCodeZen) can't self-heal
    /// and require user re-authentication when tokens expire.
    pub fn supports_cli_refresh(self) -> bool {
        matches!(self, Self::GitHub | Self::Codex)
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
    monitor: JoinHandle<Result<(), AuthError>>,
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
        Self { response, monitor }
    }
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("unsupported provider: {0}")]
    UnsupportedProvider(String),
    #[error("auth flow already in progress for {0}")]
    FlowAlreadyPending(Provider),
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
        if let Ok(socket_path) = std::env::var("LFD_CREDENTIAL_SOCKET") {
            let trimmed = socket_path.trim();
            if !trimmed.is_empty() {
                let client = Arc::new(CredentialSocketClient::new(PathBuf::from(trimmed)));
                return Self::with_brokers_and_store(
                    vec![
                        Arc::new(SocketAuthBroker::new(Provider::GitHub, client.clone()))
                            as Arc<dyn AuthBroker>,
                        Arc::new(SocketAuthBroker::new(Provider::Claude, client.clone()))
                            as Arc<dyn AuthBroker>,
                        Arc::new(SocketAuthBroker::new(Provider::Codex, client.clone()))
                            as Arc<dyn AuthBroker>,
                        Arc::new(OpenCodeZenBroker::default()) as Arc<dyn AuthBroker>,
                    ],
                    Some(store),
                );
            }
        }

        Self::with_brokers_and_store(
            vec![
                Arc::new(GhAuthBroker::default()) as Arc<dyn AuthBroker>,
                Arc::new(ClaudeAuthBroker::default()) as Arc<dyn AuthBroker>,
                Arc::new(CodexAuthBroker::default()) as Arc<dyn AuthBroker>,
                Arc::new(OpenCodeZenBroker::default()) as Arc<dyn AuthBroker>,
            ],
            Some(store),
        )
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

        let status = self.broker(provider)?.check_status().await?;
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
        event_hub: EventHub,
    ) -> Result<AuthFlowResponse, AuthError> {
        self.prune_finished_pending().await;
        if self.is_pending(provider).await {
            return Err(AuthError::FlowAlreadyPending(provider));
        }

        let broker = self.broker(provider)?;
        let AuthFlowHandle { response, monitor } = broker.start_auth().await?;

        event_hub.send(Event::auth_flow_started(
            provider,
            response.verification_uri.clone(),
            response.verification_uri_complete.clone(),
        ));

        let pending = self.pending.clone();
        let broker_for_task = broker.clone();
        let event_hub_for_task = event_hub.clone();
        let store_for_task = self.store.clone();

        let lifecycle = tokio::spawn(async move {
            let monitor_result = match monitor.await {
                Ok(result) => result,
                Err(err) => Err(AuthError::CommandFailed {
                    provider,
                    message: format!("auth monitor task failed: {err}"),
                }),
            };

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
                        event_hub_for_task.send(Event::auth_connected(provider, login));
                    }
                    Ok(status) => {
                        event_hub_for_task.send(Event::auth_failed(
                            provider,
                            format!("completed with status {}", status.as_str()),
                        ));
                    }
                    Err(err) => {
                        event_hub_for_task.send(Event::auth_failed(provider, err.to_string()));
                    }
                },
                Err(err) => {
                    event_hub_for_task.send(Event::auth_failed(provider, err.to_string()));
                }
            }

            let mut pending = pending.lock().await;
            pending.remove(&provider);
        });

        self.pending.lock().await.insert(provider, lifecycle);
        Ok(response)
    }

    pub async fn disconnect(
        &self,
        provider: Provider,
        event_hub: EventHub,
    ) -> Result<(), AuthError> {
        self.abort_pending(provider).await;
        self.broker(provider)?.disconnect().await?;
        if let Some(store) = &self.store {
            if let Err(err) = store.delete_provider_token(provider.as_str()).await {
                warn!(provider = %provider, error = %err, "failed to delete provider token");
            }
        }
        event_hub.send(Event::auth_disconnected(provider));
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
    home_dir: PathBuf,
}

impl Default for ClaudeAuthBroker {
    fn default() -> Self {
        Self {
            home_dir: home_dir_or_cwd(),
        }
    }
}

#[async_trait]
impl AuthBroker for ClaudeAuthBroker {
    fn provider(&self) -> Provider {
        Provider::Claude
    }

    async fn start_auth(&self) -> Result<AuthFlowHandle, AuthError> {
        let mut command = Command::new("claude");
        command.args(["auth", "login"]);
        command.env("BROWSER", "echo");
        command.env("CLAUDE_BROWSER", "echo");

        start_auth_command(Provider::Claude, "claude", command, parse_generic_auth_line).await
    }

    async fn check_status(&self) -> Result<AuthStatus, AuthError> {
        let mut command = Command::new("claude");
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
        let mut command = Command::new("claude");
        command.args(["auth", "logout"]);

        // Best-effort CLI logout; always clean up auth files regardless
        let _ = command.output().await;
        self.remove_claude_auth_files()
    }

    async fn extract_token(&self) -> Option<ProviderToken> {
        extract_claude_token(&self.home_dir)
    }
}

impl ClaudeAuthBroker {
    fn remove_claude_auth_files(&self) -> Result<(), AuthError> {
        let claude_dir = self.home_dir.join(".claude");
        if !claude_dir.exists() {
            return Ok(());
        }
        for name in &["auth.json", "session-cache"] {
            let path = claude_dir.join(name);
            if path.exists() {
                remove_path(&path)?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct CodexAuthBroker {
    home_dir: PathBuf,
}

impl Default for CodexAuthBroker {
    fn default() -> Self {
        Self {
            home_dir: home_dir_or_cwd(),
        }
    }
}

#[async_trait]
impl AuthBroker for CodexAuthBroker {
    fn provider(&self) -> Provider {
        Provider::Codex
    }

    async fn start_auth(&self) -> Result<AuthFlowHandle, AuthError> {
        let mut command = Command::new("codex");
        command.args(["login", "--device-auth"]);

        start_auth_command(Provider::Codex, "codex", command, parse_generic_auth_line).await
    }

    async fn check_status(&self) -> Result<AuthStatus, AuthError> {
        Ok(if extract_codex_token(&self.home_dir).is_some() {
            AuthStatus::Active { login: None }
        } else {
            AuthStatus::None
        })
    }

    async fn disconnect(&self) -> Result<(), AuthError> {
        let mut command = Command::new("codex");
        command.arg("logout");

        match command.output().await {
            Ok(output) if output.status.success() => Ok(()),
            Ok(_) | Err(_) => {
                let codex_dir = self.home_dir.join(".codex");
                if codex_dir.exists() {
                    remove_path(&codex_dir)?;
                }
                Ok(())
            }
        }
    }

    async fn extract_token(&self) -> Option<ProviderToken> {
        extract_codex_token(&self.home_dir)
    }
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
}

async fn start_auth_command(
    provider: Provider,
    command_name: &'static str,
    mut command: Command,
    mut parse_line: impl FnMut(&str, &mut AuthFlowBuilder),
) -> Result<AuthFlowHandle, AuthError> {
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    command.stdin(Stdio::null());

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
                    command_exit_result(provider, status)
                });
                return Ok(AuthFlowHandle::new(response, monitor));
            }

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
    if let Some(url) = extract_url(line) {
        if builder.verification_uri_complete.is_none() {
            builder.verification_uri_complete = Some(url.clone());
        }
        if builder.verification_uri.is_none() {
            builder.verification_uri = Some(strip_query(&url).to_string());
        }
    }

    if builder.user_code.is_none() {
        builder.user_code = extract_user_code(line);
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
            .and_then(extract_user_code);
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
        expires_at,
        login,
        updated_at: now_unix(),
        credential_type: CredentialType::OAuth,
    })
}

fn extract_claude_token(home_dir: &Path) -> Option<ProviderToken> {
    let cred_path = home_dir.join(".claude/.credentials.json");
    let content = fs::read_to_string(cred_path).ok()?;
    let json: serde_json::Value = serde_json::from_str(&content).ok()?;
    let token = json.get("accessToken")?.as_str()?;
    let expires_at = read_json_expires_at(
        &json,
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
        expires_at,
        login: None,
        updated_at: now_unix(),
        credential_type: CredentialType::OAuth,
    })
}

fn extract_codex_token(home_dir: &Path) -> Option<ProviderToken> {
    let auth_path = home_dir.join(".codex/auth.json");
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
        expires_at,
        login: None,
        updated_at: now_unix(),
        credential_type: CredentialType::OAuth,
    })
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
        expires_at,
        login,
        updated_at: now_unix(),
        credential_type: CredentialType::OAuth,
    })
}

fn read_nonempty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
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

pub async fn refresh_provider_token(
    provider: Provider,
) -> Result<ProviderToken, TokenRefreshError> {
    refresh_provider_token_with_runner(provider, &home_dir_or_cwd(), &TokioRefreshCommandRunner)
        .await
}

async fn refresh_provider_token_with_runner(
    provider: Provider,
    home_dir: &Path,
    runner: &dyn RefreshCommandRunner,
) -> Result<ProviderToken, TokenRefreshError> {
    match provider {
        Provider::GitHub => refresh_github_token(home_dir, runner).await,
        Provider::Claude => refresh_claude_token(home_dir),
        Provider::Codex => refresh_codex_token(home_dir, runner).await,
        Provider::OpenCodeZen => {
            extract_opencode_zen_token(home_dir).ok_or(TokenRefreshError::MissingToken {
                provider: Provider::OpenCodeZen,
            })
        }
    }
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

async fn refresh_codex_token(
    home_dir: &Path,
    runner: &dyn RefreshCommandRunner,
) -> Result<ProviderToken, TokenRefreshError> {
    let _ = run_refresh_command(
        Provider::Codex,
        "codex",
        &["login", "--refresh"],
        runner,
        false,
    )
    .await;

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
        ("codex", CredentialType::OAuth) => None, // Codex OAuth env injection unsupported
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
pub async fn provider_env_vars(store: &crate::lfd::store::Store) -> Vec<(String, String)> {
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
    use std::os::unix::net::UnixListener;
    use std::os::unix::process::ExitStatusExt;
    use std::sync::Mutex as StdMutex;
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
        assert!("gemini".parse::<Provider>().is_err());
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

        let broker = ClaudeAuthBroker {
            home_dir: temp.path().to_path_buf(),
        };
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

        tokio::time::timeout(Duration::from_secs(3), handle.monitor)
            .await
            .expect("monitor should complete")
            .expect("monitor task should join")
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
        let events = EventHub::new(32);
        service
            .start_auth(Provider::GitHub, events)
            .await
            .expect("start auth");

        let status = service.status(Provider::GitHub).await.expect("status");
        assert_eq!(status.status, AuthStatus::Pending);
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
        let store = crate::lfd::store::open_store(&crate::lfd::store::StorageConfig::sqlite(
            tmp.path().join("lfd.db"),
        ))
        .await
        .expect("open sqlite store");
        store
            .upsert_provider_token(&ProviderToken {
                provider: "opencodezen".to_string(),
                access_token: "opencode-key".to_string(),
                refresh_token: None,
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
    async fn refresh_codex_token_falls_back_to_file_when_command_missing() {
        let tmp = tempdir().expect("tempdir");
        let codex_dir = tmp.path().join(".codex");
        fs::create_dir_all(&codex_dir).expect("create codex dir");
        fs::write(
            codex_dir.join("auth.json"),
            r#"{"access_token":"oauth-token"}"#,
        )
        .expect("write auth json");
        let runner = FakeRefreshRunner::new(vec![Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "codex not installed",
        ))]);

        let token = refresh_provider_token_with_runner(Provider::Codex, tmp.path(), &runner).await;
        let token = token.expect("fallback file refresh should succeed");
        assert_eq!(token.provider, "codex");
        assert_eq!(token.access_token, "oauth-token");
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
            expires_at: None,
            login: None,
            updated_at: now_unix(),
            credential_type,
        }
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
    fn env_var_for_token_codex_oauth_returns_none() {
        let token = make_token("codex", CredentialType::OAuth);
        assert!(env_var_for_token(&token).is_none());
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
        let store = crate::lfd::store::open_store(&crate::lfd::store::StorageConfig::sqlite(
            tmp.path().join("lfd.db"),
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

        // Codex with OAuth (should produce no env var)
        store
            .upsert_provider_token(&make_token("codex", CredentialType::OAuth))
            .await
            .expect("upsert codex oauth");

        let vars = provider_env_vars(&store).await;

        assert!(vars.iter().any(|(n, _)| n == "ANTHROPIC_API_KEY"));
        assert!(vars.iter().any(|(n, _)| n == "GH_TOKEN"));
        assert!(!vars.iter().any(|(n, _)| n == "CLAUDE_CODE_OAUTH_TOKEN"));
        assert!(!vars.iter().any(|(n, _)| n == "OPENAI_API_KEY"));
    }
}
