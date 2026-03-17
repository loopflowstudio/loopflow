use anyhow::{anyhow, bail, Context, Result};
use reqwest::blocking::{Client, Response};
use reqwest::header::{HeaderValue, AUTHORIZATION};
use serde::de::DeserializeOwned;
use serde::Deserialize;
use std::io::{self, Write};
use std::thread;
use std::time::{Duration, Instant};

use crate::lfd::provider_auth::Provider;

const READY_TIMEOUT: Duration = Duration::from_secs(5);
const READY_POLL_INTERVAL: Duration = Duration::from_millis(200);
const AUTH_POLL_INTERVAL: Duration = Duration::from_secs(1);
const AUTH_POLL_TIMEOUT: Duration = Duration::from_secs(5 * 60);

pub fn run_install_onboarding(no_interactive: bool) -> Result<()> {
    if no_interactive {
        println!("Run `lfq auth status` to connect providers.");
        return Ok(());
    }

    let client = OnboardingClient::new()?;
    if !client.wait_until_ready(READY_TIMEOUT) {
        bail!(
            "lfd did not become ready within {} seconds. Run `lfq auth status` to connect providers once the daemon is running.",
            READY_TIMEOUT.as_secs()
        );
    }

    let initial_statuses = client.list_statuses()?;
    let required_initially_connected = required_providers_connected(&initial_statuses);

    println!();
    println!("Connecting accounts...");
    println!();

    let mut has_agent = try_connect_agent(&client, Provider::Claude)?;
    ensure_required_provider_connected(&client, Provider::GitHub)?;

    if has_agent {
        offer_optional_provider(&client, Provider::Codex)?;
        offer_optional_provider(&client, Provider::OpenCodeZen)?;
    } else {
        has_agent = try_connect_agent(&client, Provider::Codex)?;
        if has_agent {
            offer_optional_provider(&client, Provider::OpenCodeZen)?;
        } else {
            try_connect_agent(&client, Provider::OpenCodeZen)?;
        }
    }

    let final_statuses = client.list_statuses()?;
    if !required_providers_connected(&final_statuses) {
        bail!(
            "required providers are not connected. Run `lfq auth status` and `lfq auth <provider>` to finish setup"
        );
    }

    println!();
    if required_initially_connected {
        println!("All required providers connected. Ready.");
    } else {
        println!("Ready. Run `lf` to start.");
    }

    Ok(())
}

fn try_connect_agent(client: &OnboardingClient, provider: Provider) -> Result<bool> {
    let status = client.provider_status(provider)?;
    if status.is_active() {
        print_connected(provider, &status);
        return Ok(true);
    }

    // Detect API key in environment and warn about billing
    if let Some(env_name) = provider.api_key_env_name() {
        if std::env::var(env_name)
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false)
        {
            println!(
                "  Found {env_name}. API key auth bills per token — OAuth uses your existing subscription. We recommend OAuth."
            );
        }
    }

    if connect_provider(client, provider)? {
        return Ok(true);
    }

    // OAuth failed/skipped — offer API key fallback if available
    if let Some(env_name) = provider.api_key_env_name() {
        if let Ok(api_key) = std::env::var(env_name) {
            if !api_key.trim().is_empty() && prompt_api_key_fallback(provider, env_name)? {
                client.configure_api_key(provider, &api_key)?;
                println!(
                    "  ✓ {} connected via API key (pay-per-token billing)",
                    provider.display_name()
                );
                return Ok(true);
            }
        }
    }

    println!("  ! {} not connected.", provider.display_name());
    Ok(false)
}

fn prompt_api_key_fallback(provider: Provider, env_name: &str) -> Result<bool> {
    print!("  Use {} for {}? [y/N] ", env_name, provider.display_name());
    io::stdout().flush().context("failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read user input")?;

    Ok(is_affirmative_input(&input))
}

fn ensure_required_provider_connected(client: &OnboardingClient, provider: Provider) -> Result<()> {
    let status = client.provider_status(provider)?;
    if status.is_active() {
        print_connected(provider, &status);
        return Ok(());
    }

    if connect_provider(client, provider)? {
        return Ok(());
    }

    bail!(
        "failed to connect required provider {}",
        provider.display_name()
    )
}

fn offer_optional_provider(client: &OnboardingClient, provider: Provider) -> Result<()> {
    let status = client.provider_status(provider)?;
    if status.is_active() {
        print_connected(provider, &status);
        return Ok(());
    }

    if !prompt_optional(provider)? {
        println!("  Skipped");
        return Ok(());
    }

    if !connect_provider(client, provider)? {
        println!("  ! {} not connected.", provider.display_name());
    }

    Ok(())
}

fn connect_provider(client: &OnboardingClient, provider: Provider) -> Result<bool> {
    let start_result = client.start_auth(provider)?;
    let expires_in = match start_result {
        StartAuthResult::Started(flow) => {
            let location = flow
                .verification_uri_complete
                .as_ref()
                .unwrap_or(&flow.verification_uri);
            let code = flow.user_code.unwrap_or_else(|| "(no code)".to_string());
            println!(
                "  {} — go to {} and enter code: {}",
                provider.display_name(),
                location,
                code,
            );
            flow.expires_in
        }
        StartAuthResult::AlreadyPending => {
            println!(
                "  {} — auth already pending, waiting for completion...",
                provider.display_name()
            );
            None
        }
    };

    poll_until_connected(client, provider, expires_in)
}

fn poll_until_connected(
    client: &OnboardingClient,
    provider: Provider,
    expires_in: Option<u64>,
) -> Result<bool> {
    let timeout = expires_in
        .map(Duration::from_secs)
        .map(|duration| duration.min(AUTH_POLL_TIMEOUT))
        .unwrap_or(AUTH_POLL_TIMEOUT);
    let deadline = Instant::now() + timeout;

    while Instant::now() < deadline {
        let status = client.provider_status(provider)?;
        if status.is_active() {
            print_connected(provider, &status);
            return Ok(true);
        }

        if status.status == "expired" {
            println!("  ! {} authentication expired.", provider.display_name());
            return Ok(false);
        }

        thread::sleep(AUTH_POLL_INTERVAL);
    }

    println!(
        "  ! Timed out waiting for {} authentication.",
        provider.display_name()
    );
    Ok(false)
}

fn print_connected(provider: Provider, status: &AuthProviderStatus) {
    let name = provider.display_name();
    let login = status
        .login
        .as_deref()
        .map(|login| format_login(provider, login))
        .unwrap_or_else(|| "authenticated".to_string());
    println!("  ✓ {name} connected as {login}");
}

fn prompt_optional(provider: Provider) -> Result<bool> {
    print!(
        "  {} (optional) — press Enter to skip, or 'y' to connect: ",
        provider.display_name()
    );
    io::stdout().flush().context("failed to flush stdout")?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read user input")?;

    Ok(is_affirmative_input(&input))
}

fn required_providers_connected(statuses: &[AuthProviderStatus]) -> bool {
    let github_connected = provider_is_active(statuses, Provider::GitHub);
    let has_agent = provider_is_active(statuses, Provider::Claude)
        || provider_is_active(statuses, Provider::Codex)
        || provider_is_active(statuses, Provider::OpenCodeZen);
    github_connected && has_agent
}

fn provider_is_active(statuses: &[AuthProviderStatus], provider: Provider) -> bool {
    statuses
        .iter()
        .any(|status| status.provider == provider.as_str() && status.is_active())
}

fn is_affirmative_input(input: &str) -> bool {
    matches!(input.trim().to_ascii_lowercase().as_str(), "y" | "yes")
}

fn format_login(provider: Provider, login: &str) -> String {
    if provider == Provider::GitHub {
        format!("@{login}")
    } else {
        login.to_string()
    }
}

fn resolve_base_url() -> String {
    if let Ok(raw) = std::env::var("LFD_URL") {
        return normalize_base_url(&raw);
    }

    if let Ok(raw) = std::env::var("LFD_HTTP_ADDR") {
        return normalize_base_url(&raw);
    }

    "http://127.0.0.1:2486".to_string()
}

fn normalize_base_url(raw: &str) -> String {
    let trimmed = raw.trim().trim_end_matches('/');
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        trimmed.to_string()
    } else {
        format!("http://{trimmed}")
    }
}

fn auth_header() -> Option<String> {
    for key in ["LFD_TOKEN", "LFD_AUTH_TOKEN"] {
        if let Ok(value) = std::env::var(key) {
            let token = value.trim();
            if !token.is_empty() {
                return Some(token.to_string());
            }
        }
    }

    let path = crate::lfd::session_token::token_path();
    let token = std::fs::read_to_string(path).ok()?;
    let trimmed = token.trim();
    if trimmed.is_empty() {
        return None;
    }
    Some(trimmed.to_string())
}

#[derive(Debug)]
struct OnboardingClient {
    base_url: String,
    client: Client,
}

impl OnboardingClient {
    fn new() -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .context("failed to create onboarding HTTP client")?;

        Ok(Self {
            base_url: resolve_base_url(),
            client,
        })
    }

    fn wait_until_ready(&self, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            match self
                .client
                .get(format!("{}/health", self.base_url))
                .send()
                .map(|response| response.status().is_success())
            {
                Ok(true) => return true,
                Ok(false) | Err(_) => thread::sleep(READY_POLL_INTERVAL),
            }
        }
        false
    }

    fn list_statuses(&self) -> Result<Vec<AuthProviderStatus>> {
        let response: AuthProvidersResponse = self.get_json("/v0/auth")?;
        Ok(response.providers)
    }

    fn provider_status(&self, provider: Provider) -> Result<AuthProviderStatus> {
        self.get_json(&format!("/v0/auth/{}", provider.as_str()))
    }

    fn start_auth(&self, provider: Provider) -> Result<StartAuthResult> {
        let request = self
            .client
            .post(format!("{}/v0/auth/{}", self.base_url, provider.as_str()));
        let response = self
            .authorize(request)
            .send()
            .with_context(|| format!("failed to start {} auth flow", provider.display_name()))?;

        if response.status() == reqwest::StatusCode::CONFLICT {
            return Ok(StartAuthResult::AlreadyPending);
        }

        if !response.status().is_success() {
            return Err(response_error(response, "start auth"));
        }

        let flow: AuthFlowResponse = response
            .json()
            .context("failed to parse start auth response")?;
        Ok(StartAuthResult::Started(flow))
    }

    fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let request = self.client.get(format!("{}{}", self.base_url, path));
        let response = self
            .authorize(request)
            .send()
            .with_context(|| format!("request to {path} failed"))?;

        if !response.status().is_success() {
            return Err(response_error(response, path));
        }

        response
            .json()
            .with_context(|| format!("failed to parse response body for {path}"))
    }

    fn configure_api_key(&self, provider: Provider, api_key: &str) -> Result<()> {
        let body = serde_json::json!({ "api_key": api_key });
        let request = self.client.put(format!(
            "{}/v0/auth/{}/credential",
            self.base_url,
            provider.as_str()
        ));
        let response = self
            .authorize(request)
            .json(&body)
            .send()
            .with_context(|| format!("failed to configure {} API key", provider.display_name()))?;

        if !response.status().is_success() {
            return Err(response_error(response, "configure API key"));
        }
        Ok(())
    }

    fn authorize(
        &self,
        request: reqwest::blocking::RequestBuilder,
    ) -> reqwest::blocking::RequestBuilder {
        let Some(token) = auth_header() else {
            return request;
        };
        let value = format!("Bearer {token}");
        let Ok(header) = HeaderValue::from_str(&value) else {
            return request;
        };
        request.header(AUTHORIZATION, header)
    }
}

fn response_error(response: Response, context: &str) -> anyhow::Error {
    let status = response.status();
    let body = response.text().unwrap_or_default();

    if let Ok(parsed) = serde_json::from_str::<ApiErrorResponse>(&body) {
        return anyhow!(
            "{context} failed with HTTP {}: {}",
            status.as_u16(),
            parsed.error.message
        );
    }

    let trimmed = body.trim();
    if trimmed.is_empty() {
        anyhow!("{context} failed with HTTP {}", status.as_u16())
    } else {
        anyhow!("{context} failed with HTTP {}: {trimmed}", status.as_u16())
    }
}

enum StartAuthResult {
    Started(AuthFlowResponse),
    AlreadyPending,
}

#[derive(Debug, Deserialize)]
struct AuthProvidersResponse {
    providers: Vec<AuthProviderStatus>,
}

#[derive(Debug, Deserialize, Clone)]
struct AuthProviderStatus {
    provider: String,
    status: String,
    login: Option<String>,
    #[allow(dead_code)]
    expires_at: Option<String>,
    #[allow(dead_code)]
    next_refresh_at: Option<String>,
}

impl AuthProviderStatus {
    fn is_active(&self) -> bool {
        self.status == "active"
    }
}

#[derive(Debug, Deserialize)]
struct AuthFlowResponse {
    #[allow(dead_code)]
    provider: String,
    verification_uri: String,
    verification_uri_complete: Option<String>,
    user_code: Option<String>,
    expires_in: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ApiErrorResponse {
    error: ApiErrorBody,
}

#[derive(Debug, Deserialize)]
struct ApiErrorBody {
    message: String,
}

#[cfg(test)]
mod tests {
    use super::{
        is_affirmative_input, normalize_base_url, required_providers_connected, AuthProviderStatus,
    };

    fn active(provider: &str) -> AuthProviderStatus {
        AuthProviderStatus {
            provider: provider.to_string(),
            status: "active".to_string(),
            login: None,
            expires_at: None,
            next_refresh_at: None,
        }
    }

    fn none(provider: &str) -> AuthProviderStatus {
        AuthProviderStatus {
            provider: provider.to_string(),
            status: "none".to_string(),
            login: None,
            expires_at: None,
            next_refresh_at: None,
        }
    }

    #[test]
    fn required_providers_need_github_and_any_agent() {
        assert!(required_providers_connected(&[
            active("github"),
            active("claude")
        ]));
        assert!(required_providers_connected(&[
            active("github"),
            active("codex")
        ]));
        assert!(required_providers_connected(&[
            active("github"),
            active("opencodezen")
        ]));
        assert!(!required_providers_connected(&[
            active("claude"),
            none("github")
        ]));
        assert!(!required_providers_connected(&[
            active("github"),
            none("claude")
        ]));
    }

    #[test]
    fn affirmative_input_accepts_yes_variants() {
        assert!(is_affirmative_input("y"));
        assert!(is_affirmative_input("YES"));
        assert!(!is_affirmative_input(""));
        assert!(!is_affirmative_input("n"));
    }

    #[test]
    fn normalize_base_url_supports_host_port_and_urls() {
        assert_eq!(
            normalize_base_url("127.0.0.1:2486"),
            "http://127.0.0.1:2486"
        );
        assert_eq!(
            normalize_base_url("http://127.0.0.1:2486/"),
            "http://127.0.0.1:2486"
        );
    }
}
