use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};
use time::OffsetDateTime;

use crate::engine::platform::open_url;
use crate::lf::AuthCommand;
use crate::lfdb::{open_store, CredentialType, ProviderToken, SharedStore};
use crate::provider_auth::{
    no_event_sink, AuthStatus, Provider, ProviderAuthService, ProviderAuthSnapshot,
};

const AUTH_STATUS_POLL_TIMEOUT: Duration = Duration::from_secs(180);
const AUTH_STATUS_POLL_INTERVAL: Duration = Duration::from_secs(1);

pub fn run(cmd: &AuthCommand) -> Result<()> {
    let rt = tokio::runtime::Runtime::new().context("failed to create async runtime")?;
    rt.block_on(run_async(cmd))
}

async fn run_async(cmd: &AuthCommand) -> Result<()> {
    match cmd {
        AuthCommand::Status { provider } => status(provider.as_deref()).await,
        AuthCommand::Disconnect { provider } => disconnect(provider).await,
        AuthCommand::Configure { provider } => configure(provider).await,
        AuthCommand::Connect { provider } => connect(provider).await,
        AuthCommand::External(args) => {
            let provider = args
                .first()
                .ok_or_else(|| anyhow!("usage: lf auth <provider>"))?;
            if args.len() > 1 {
                return Err(anyhow!(
                    "unexpected auth arguments: {}",
                    args[1..].join(" ")
                ));
            }
            connect(provider).await
        }
    }
}

async fn status(provider: Option<&str>) -> Result<()> {
    let service = local_auth_service().await?;
    match provider {
        Some(raw) => {
            let snapshot = service.status(parse_provider(raw)?).await?;
            println!("{}", format_snapshot(&snapshot));
        }
        None => {
            for snapshot in service.list_statuses().await? {
                println!("{}", format_snapshot(&snapshot));
            }
        }
    }
    Ok(())
}

async fn connect(raw_provider: &str) -> Result<()> {
    let provider = parse_provider(raw_provider)?;
    let service = local_auth_service().await?;
    let flow = service.start_auth(provider, no_event_sink()).await?;

    let verification_url = flow
        .verification_uri_complete
        .clone()
        .unwrap_or_else(|| flow.verification_uri.clone());
    println!(
        "Opening {} auth in your browser...",
        provider.display_name()
    );
    println!("If the browser does not open, visit:\n{verification_url}");
    open_url(&verification_url);

    wait_for_active_status(&service, provider, flow.expires_in).await
}

async fn disconnect(raw_provider: &str) -> Result<()> {
    let provider = parse_provider(raw_provider)?;
    let service = local_auth_service().await?;
    service.disconnect(provider, no_event_sink()).await?;
    let snapshot = service.status(provider).await?;
    if matches!(snapshot.status, AuthStatus::None) {
        println!("Disconnected {}", provider.display_name());
    } else {
        println!("Updated {}", format_snapshot(&snapshot));
    }
    Ok(())
}

async fn configure(raw_provider: &str) -> Result<()> {
    let provider = parse_provider(raw_provider)?;
    if let Some(message) = provider.api_key_configure_error() {
        return Err(anyhow!(message));
    }
    let env_name = provider
        .api_key_env_name()
        .ok_or_else(|| anyhow!("{} does not support API key auth", provider.display_name()))?;
    let api_key = std::env::var(env_name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            anyhow!(
                "{env_name} is not set. Export it, then run `lf auth configure {}`.",
                provider.as_str()
            )
        })?;

    let store = local_store().await?;
    store
        .upsert_provider_token(&ProviderToken {
            provider: provider.as_str().to_string(),
            access_token: api_key,
            refresh_token: None,
            oauth_client_id: None,
            expires_at: None,
            login: None,
            updated_at: OffsetDateTime::now_utc().unix_timestamp(),
            credential_type: CredentialType::ApiKey,
        })
        .await?;

    if provider.api_key_bills_per_token() {
        println!("API key auth bills per token. OAuth uses your existing subscription.");
    }
    println!("Stored {} API key", provider.display_name());
    Ok(())
}

async fn local_auth_service() -> Result<ProviderAuthService> {
    Ok(ProviderAuthService::new(local_store().await?))
}

async fn local_store() -> Result<SharedStore> {
    let cfg = crate::lfd::storage_config_from_env()
        .context("failed to resolve local lfd credential store")?;
    let store = open_store(&cfg)
        .await
        .map_err(|err| anyhow!("failed to open local lfd credential store: {err}"))?;
    Ok(Arc::new(store))
}

async fn wait_for_active_status(
    service: &ProviderAuthService,
    provider: Provider,
    expires_in: Option<u64>,
) -> Result<()> {
    let timeout = expires_in
        .map(Duration::from_secs)
        .unwrap_or(AUTH_STATUS_POLL_TIMEOUT);
    let deadline = std::time::Instant::now() + timeout;

    loop {
        let snapshot = service.status(provider).await?;
        match snapshot.status {
            AuthStatus::Active { login } => {
                if provider == Provider::GitHub {
                    if let Some(login) = login {
                        println!("Authenticated as @{login}");
                    } else {
                        println!("Authenticated");
                    }
                } else {
                    println!("Authenticated");
                }
                return Ok(());
            }
            AuthStatus::Expired => {
                println!(
                    "Authentication expired. Run `lf auth {}` again.",
                    provider.as_str()
                );
                return Ok(());
            }
            AuthStatus::Pending | AuthStatus::None => {
                if std::time::Instant::now() >= deadline {
                    println!(
                        "Authentication still pending. Finish the browser flow, then run `lf auth status {}`.",
                        provider.as_str()
                    );
                    return Ok(());
                }
                tokio::time::sleep(AUTH_STATUS_POLL_INTERVAL).await;
            }
        }
    }
}

fn parse_provider(raw: &str) -> Result<Provider> {
    raw.parse::<Provider>()
        .map_err(|_| anyhow!("unknown provider: {raw}"))
}

fn format_snapshot(snapshot: &ProviderAuthSnapshot) -> String {
    let provider = format!("{:<12}", snapshot.provider.display_name());
    let credential_type = snapshot.credential_type.unwrap_or(CredentialType::OAuth);

    match snapshot.status.clone() {
        AuthStatus::Active { login } => {
            let status = if credential_type == CredentialType::ApiKey {
                "apikey"
            } else {
                "oauth"
            };
            let mut details = Vec::new();
            if let Some(login) = login {
                if snapshot.provider == Provider::GitHub {
                    details.push(format!("@{login}"));
                } else {
                    details.push(login);
                }
            } else {
                details.push("authenticated".to_string());
            }

            let now = OffsetDateTime::now_utc().unix_timestamp();
            if let Some(expires_at) = snapshot.expires_at {
                let delta = expires_at - now;
                if delta <= 0 {
                    details.push("expired".to_string());
                } else {
                    details.push(format!("expires {}", format_relative_delta(delta)));
                }
            }
            if let Some(next_refresh_at) = snapshot.next_refresh_at {
                let delta = next_refresh_at - now;
                if delta <= 0 {
                    details.push("refreshing soon".to_string());
                } else {
                    details.push(format!("refresh in {}", format_relative_delta(delta)));
                }
            }
            if credential_type == CredentialType::ApiKey
                && snapshot.provider.api_key_bills_per_token()
            {
                details.push("pay-per-token".to_string());
            }

            format!("{provider} {status:<8} {}", details.join(" · "))
        }
        AuthStatus::Pending => {
            format!(
                "{provider} {:<8} waiting for browser confirmation",
                "pending"
            )
        }
        AuthStatus::Expired => format!("{provider} {:<8} expired", "expired"),
        AuthStatus::None => format!("{provider} {:<8} not connected", "none"),
    }
}

fn format_relative_delta(seconds: i64) -> String {
    let total_seconds = seconds.max(0);
    if total_seconds < 60 {
        return format!("{total_seconds}s");
    }
    if total_seconds < 3600 {
        return format!("{}m", total_seconds / 60);
    }
    if total_seconds < 86_400 {
        return format!("{}h", total_seconds / 3600);
    }
    format!("{}d", total_seconds / 86_400)
}

#[cfg(test)]
mod tests {
    use time::OffsetDateTime;

    use crate::lfdb::CredentialType;
    use crate::provider_auth::{AuthStatus, Provider, ProviderAuthSnapshot};

    use super::{format_relative_delta, format_snapshot};

    #[test]
    fn format_snapshot_shows_login_and_expiry() {
        let now = OffsetDateTime::now_utc().unix_timestamp();
        let rendered = format_snapshot(&ProviderAuthSnapshot {
            provider: Provider::GitHub,
            status: AuthStatus::Active {
                login: Some("jackdanger".to_string()),
            },
            expires_at: Some(now + 7200),
            next_refresh_at: None,
            credential_type: Some(CredentialType::OAuth),
        });

        assert!(rendered.contains("GitHub"));
        assert!(rendered.contains("oauth"));
        assert!(rendered.contains("@jackdanger"));
        assert!(rendered.contains("expires 1h") || rendered.contains("expires 2h"));
    }

    #[test]
    fn format_snapshot_shows_disconnected_providers() {
        let rendered = format_snapshot(&ProviderAuthSnapshot {
            provider: Provider::Linear,
            status: AuthStatus::None,
            expires_at: None,
            next_refresh_at: None,
            credential_type: None,
        });

        assert!(rendered.contains("Linear"));
        assert!(rendered.contains("not connected"));
    }

    #[test]
    fn format_relative_delta_uses_human_units() {
        assert_eq!(format_relative_delta(42), "42s");
        assert_eq!(format_relative_delta(180), "3m");
        assert_eq!(format_relative_delta(7_200), "2h");
        assert_eq!(format_relative_delta(172_800), "2d");
    }
}
