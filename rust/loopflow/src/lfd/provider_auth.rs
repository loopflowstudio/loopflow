use std::collections::HashMap;
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
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;

use crate::lfd::events::EventHub;
use crate::lfd::types::Event;

const AUTH_URL_TIMEOUT: Duration = Duration::from_secs(20);
const AUTH_URL_POLL_INTERVAL: Duration = Duration::from_millis(200);

static USER_CODE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)\b([A-Z0-9]{4}(?:-[A-Z0-9]{4})+)\b").expect("user code regex"));
static EXPIRES_IN_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?i)expires(?:_in| in)?[^0-9]*(\d{2,6})").expect("expires regex"));
static GH_LOGIN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)logged in to\s+\S+\s+as\s+([a-z0-9-]+)").expect("github login regex")
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Provider {
    GitHub,
    Claude,
    Codex,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::GitHub => "github",
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    pub fn all() -> [Self; 3] {
        [Self::GitHub, Self::Claude, Self::Codex]
    }
}

impl std::fmt::Display for Provider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Provider {
    type Err = ();

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalized = value.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "github" | "gh" => Ok(Self::GitHub),
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            _ => Err(()),
        }
    }
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
    #[error("home directory not available")]
    MissingHomeDir,
    #[error("filesystem error: {0}")]
    Filesystem(String),
}

#[async_trait]
pub trait AuthBroker: Send + Sync {
    fn provider(&self) -> Provider;
    async fn start_auth(&self) -> Result<AuthFlowHandle, AuthError>;
    async fn check_status(&self) -> Result<AuthStatus, AuthError>;
    async fn disconnect(&self) -> Result<(), AuthError>;
}

#[derive(Clone)]
pub struct ProviderAuthService {
    brokers: HashMap<Provider, Arc<dyn AuthBroker>>,
    pending: Arc<Mutex<HashMap<Provider, JoinHandle<()>>>>,
}

impl std::fmt::Debug for ProviderAuthService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let providers = self.brokers.keys().copied().collect::<Vec<_>>();
        f.debug_struct("ProviderAuthService")
            .field("providers", &providers)
            .finish()
    }
}

impl Default for ProviderAuthService {
    fn default() -> Self {
        Self::new()
    }
}

impl ProviderAuthService {
    pub fn new() -> Self {
        Self::with_brokers(vec![
            Arc::new(GhAuthBroker::default()) as Arc<dyn AuthBroker>,
            Arc::new(ClaudeAuthBroker::default()) as Arc<dyn AuthBroker>,
            Arc::new(CodexAuthBroker::default()) as Arc<dyn AuthBroker>,
        ])
    }

    fn with_brokers(brokers: Vec<Arc<dyn AuthBroker>>) -> Self {
        let mut by_provider = HashMap::new();
        for broker in brokers {
            by_provider.insert(broker.provider(), broker);
        }
        Self {
            brokers: by_provider,
            pending: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn list_statuses(&self) -> Result<Vec<ProviderAuthSnapshot>, AuthError> {
        self.prune_finished_pending().await;
        let pending = self.pending.lock().await;
        let mut snapshots = Vec::with_capacity(Provider::all().len());

        for provider in Provider::all() {
            if pending.contains_key(&provider) {
                snapshots.push(ProviderAuthSnapshot {
                    provider,
                    status: AuthStatus::Pending,
                });
                continue;
            }
            let status = self.broker(provider)?.check_status().await?;
            snapshots.push(ProviderAuthSnapshot { provider, status });
        }

        Ok(snapshots)
    }

    pub async fn status(&self, provider: Provider) -> Result<ProviderAuthSnapshot, AuthError> {
        self.prune_finished_pending().await;
        if self.pending.lock().await.contains_key(&provider) {
            return Ok(ProviderAuthSnapshot {
                provider,
                status: AuthStatus::Pending,
            });
        }

        let status = self.broker(provider)?.check_status().await?;
        Ok(ProviderAuthSnapshot { provider, status })
    }

    pub async fn start_auth(
        &self,
        provider: Provider,
        event_hub: EventHub,
    ) -> Result<AuthFlowResponse, AuthError> {
        self.prune_finished_pending().await;
        if self.pending.lock().await.contains_key(&provider) {
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

    fn broker(&self, provider: Provider) -> Result<Arc<dyn AuthBroker>, AuthError> {
        self.brokers
            .get(&provider)
            .cloned()
            .ok_or_else(|| AuthError::UnsupportedProvider(provider.to_string()))
    }
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
            Ok(_) => {
                if let Some(login) = read_github_login(&self.home_dir) {
                    Ok(AuthStatus::Active { login: Some(login) })
                } else {
                    Ok(AuthStatus::None)
                }
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                if let Some(login) = read_github_login(&self.home_dir) {
                    Ok(AuthStatus::Active { login: Some(login) })
                } else {
                    Ok(AuthStatus::None)
                }
            }
            Err(err) => Err(AuthError::CommandIo {
                provider: Provider::GitHub,
                source: err,
            }),
        }
    }

    async fn disconnect(&self) -> Result<(), AuthError> {
        let mut command = Command::new("gh");
        command.args(["auth", "logout", "--hostname", "github.com", "--yes"]);
        match command.output().await {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => Err(AuthError::CommandFailed {
                provider: Provider::GitHub,
                message: String::from_utf8_lossy(&output.stderr).trim().to_string(),
            }),
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
        command.arg("login");
        command.env("BROWSER", "echo");
        command.env("CLAUDE_BROWSER", "echo");

        start_auth_command(Provider::Claude, "claude", command, parse_generic_auth_line).await
    }

    async fn check_status(&self) -> Result<AuthStatus, AuthError> {
        let claude_dir = self.home_dir.join(".claude");
        if has_auth_like_entries(&claude_dir)? {
            Ok(AuthStatus::Active { login: None })
        } else {
            Ok(AuthStatus::None)
        }
    }

    async fn disconnect(&self) -> Result<(), AuthError> {
        let claude_dir = self.home_dir.join(".claude");
        if !claude_dir.exists() {
            return Ok(());
        }

        let entries = fs::read_dir(&claude_dir).map_err(|err| {
            AuthError::Filesystem(format!("read {}: {err}", claude_dir.display()))
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            let file_name = entry
                .file_name()
                .to_string_lossy()
                .to_string()
                .to_ascii_lowercase();
            if !is_auth_like_name(&file_name) {
                continue;
            }
            remove_path(&path)?;
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
        let codex_dir = self.home_dir.join(".codex");
        if directory_has_entries(&codex_dir)? {
            Ok(AuthStatus::Active { login: None })
        } else {
            Ok(AuthStatus::None)
        }
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
            parse_line(&line, &mut builder);
            if build_flow_response(provider, &builder).is_some() {
                let response = build_flow_response(provider, &builder).expect("response exists");
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

fn parse_github_login(output: &str) -> Option<String> {
    GH_LOGIN_RE
        .captures(output)
        .and_then(|capture| capture.get(1))
        .map(|value| value.as_str().to_string())
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

fn directory_has_entries(path: &Path) -> Result<bool, AuthError> {
    if !path.exists() {
        return Ok(false);
    }

    let mut entries = fs::read_dir(path)
        .map_err(|err| AuthError::Filesystem(format!("read {}: {err}", path.display())))?;
    Ok(entries.next().is_some())
}

fn has_auth_like_entries(path: &Path) -> Result<bool, AuthError> {
    if !path.exists() {
        return Ok(false);
    }

    for entry in fs::read_dir(path)
        .map_err(|err| AuthError::Filesystem(format!("read {}: {err}", path.display())))?
    {
        let entry = entry.map_err(|err| {
            AuthError::Filesystem(format!("read {} entry: {err}", path.display()))
        })?;
        let name = entry.file_name().to_string_lossy().to_ascii_lowercase();

        if is_auth_like_name(&name) {
            return Ok(true);
        }
    }

    Ok(false)
}

fn is_auth_like_name(name: &str) -> bool {
    ["auth", "token", "credential", "oauth", "session"]
        .iter()
        .any(|needle| name.contains(needle))
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn provider_parses_aliases() {
        assert_eq!("github".parse::<Provider>(), Ok(Provider::GitHub));
        assert_eq!("gh".parse::<Provider>(), Ok(Provider::GitHub));
        assert_eq!("CLAUDE".parse::<Provider>(), Ok(Provider::Claude));
        assert_eq!("codex".parse::<Provider>(), Ok(Provider::Codex));
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
    fn claude_status_detects_auth_like_entries() {
        let temp = tempdir().expect("tempdir");
        let claude = temp.path().join(".claude");
        fs::create_dir_all(&claude).expect("claude dir");
        fs::write(claude.join("credentials.json"), "{}").expect("credentials file");

        assert!(has_auth_like_entries(&claude).expect("status"));
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
}
