//! Secrets provider integration.
//!
//! Doppler is the secrets provider. After OAuth via the `doppler` CLI, the
//! stored token is used to discover projects/configs and fetch secrets.
//! Fetched secrets are matched against known env-var names and persisted
//! through the existing credential-storage path.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use tracing::info;

use crate::lfd::events::EventHub;
use crate::lfd::provider_auth::Provider;
use crate::lfd::store::{CredentialType, ProviderToken, SecretsProviderConfig, SharedStore};
use crate::lfd::types::Event;

// Key mappings: env var name → provider that consumes it.
const KEY_MAPPINGS: &[(&str, Provider)] = &[
    ("ANTHROPIC_API_KEY", Provider::Claude),
    ("OPENAI_API_KEY", Provider::Codex),
];

/// Smart default config preference order.
const PREFERRED_CONFIGS: &[&str] = &["dev", "prd", "prod"];

/// A key supplied by a secrets provider and its mapping status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuppliedKey {
    pub env_name: String,
    pub provider: String,
    pub present: bool,
}

/// A Doppler project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DopplerProject {
    pub slug: String,
    pub name: String,
}

/// A Doppler config within a project.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DopplerConfig {
    pub name: String,
    pub environment: String,
}

/// Status of a connected secrets provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretsProviderStatus {
    pub provider: String,
    pub connected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub project: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<String>,
    pub keys: Vec<SuppliedKey>,
}

impl SecretsProviderStatus {
    pub fn disconnected() -> Self {
        let keys = KEY_MAPPINGS
            .iter()
            .map(|(env, p)| SuppliedKey {
                env_name: env.to_string(),
                provider: p.as_str().to_string(),
                present: false,
            })
            .collect();
        Self {
            provider: String::new(),
            connected: false,
            project: None,
            config: None,
            keys,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("unauthorized — token may be expired")]
    Unauthorized,
    #[error("Doppler is not connected")]
    NotConnected,
}

// -- Doppler API client -----------------------------------------------------

async fn doppler_get<T: serde::de::DeserializeOwned>(
    token: &str,
    url: &str,
) -> Result<T, SecretsError> {
    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| SecretsError::Http(e.to_string()))?;

    if response.status() == reqwest::StatusCode::UNAUTHORIZED {
        return Err(SecretsError::Unauthorized);
    }
    if !response.status().is_success() {
        return Err(SecretsError::Http(format!("status {}", response.status())));
    }

    response
        .json()
        .await
        .map_err(|e| SecretsError::InvalidResponse(e.to_string()))
}

/// List Doppler projects accessible with the stored token.
pub async fn list_projects(store: &SharedStore) -> Result<Vec<DopplerProject>, SecretsError> {
    let token = get_doppler_token(store).await?;

    #[derive(Deserialize)]
    struct Response {
        projects: Vec<DopplerProject>,
    }

    let resp: Response =
        doppler_get(&token, "https://api.doppler.com/v3/projects?per_page=100").await?;
    Ok(resp.projects)
}

/// List configs for a Doppler project.
pub async fn list_configs(
    store: &SharedStore,
    project: &str,
) -> Result<Vec<DopplerConfig>, SecretsError> {
    let token = get_doppler_token(store).await?;

    #[derive(Deserialize)]
    struct Response {
        configs: Vec<DopplerConfig>,
    }

    let url = format!("https://api.doppler.com/v3/configs?project={project}&per_page=100");
    let resp: Response = doppler_get(&token, &url).await?;
    Ok(resp.configs)
}

/// Fetch secrets from Doppler for the given project/config.
pub async fn fetch_secrets(
    token: &str,
    project: &str,
    config: &str,
) -> Result<HashMap<String, String>, SecretsError> {
    let url = format!(
        "https://api.doppler.com/v3/configs/config/secrets/download?project={project}&config={config}&format=json"
    );
    doppler_get(token, &url).await
}

/// Pick the best default config from a list.
pub fn smart_default_config(configs: &[DopplerConfig]) -> Option<&DopplerConfig> {
    for preferred in PREFERRED_CONFIGS {
        if let Some(c) = configs.iter().find(|c| c.name == *preferred) {
            return Some(c);
        }
    }
    configs.first()
}

// -- Sync / clear / status --------------------------------------------------

/// Read the Doppler OAuth token from provider_tokens.
async fn get_doppler_token(store: &SharedStore) -> Result<String, SecretsError> {
    let token = store
        .get_provider_token(Provider::Doppler.as_str())
        .await
        .map_err(|e| SecretsError::Http(e.to_string()))?
        .ok_or(SecretsError::NotConnected)?;
    Ok(token.access_token)
}

/// Sync secrets from Doppler into provider tokens.
///
/// Returns the list of keys that were found and synced.
pub async fn sync_secrets(
    store: &SharedStore,
    config: &SecretsProviderConfig,
    event_hub: Option<&EventHub>,
) -> Result<Vec<SuppliedKey>, SecretsError> {
    let token = get_doppler_token(store).await?;
    let project = config.project.as_deref().unwrap_or_default();
    let config_name = config.config.as_deref().unwrap_or_default();

    let secrets = fetch_secrets(&token, project, config_name).await?;

    let mut supplied_keys = Vec::new();

    for &(env_name, target_provider) in KEY_MAPPINGS {
        let value = secrets.get(env_name);
        supplied_keys.push(SuppliedKey {
            env_name: env_name.to_string(),
            provider: target_provider.as_str().to_string(),
            present: value.is_some(),
        });

        if let Some(value) = value {
            let provider_token = ProviderToken {
                provider: target_provider.as_str().to_string(),
                access_token: value.clone(),
                refresh_token: None,
                expires_at: None,
                login: Some("via doppler".to_string()),
                updated_at: crate::lfd::store::rows::now_unix(),
                credential_type: CredentialType::ApiKey,
            };
            if let Err(err) = store.upsert_provider_token(&provider_token).await {
                tracing::warn!(
                    provider = target_provider.as_str(),
                    error = %err,
                    "failed to store synced credential"
                );
            } else {
                info!(
                    provider = target_provider.as_str(),
                    env = env_name,
                    "synced credential from secrets provider"
                );
                if let Some(hub) = event_hub {
                    hub.send(Event::auth_connected(
                        target_provider,
                        Some("via doppler".to_string()),
                    ));
                }
            }
        }
    }

    if let Some(hub) = event_hub {
        hub.send(Event::secrets_synced("doppler".to_string()));
    }

    Ok(supplied_keys)
}

/// Remove credentials that were supplied by a secrets provider.
pub async fn clear_secrets_credentials(store: &SharedStore, event_hub: Option<&EventHub>) {
    for &(_, target_provider) in KEY_MAPPINGS {
        // Only clear if the credential was supplied by a secrets provider
        // (login starts with "via ").
        if let Ok(Some(token)) = store.get_provider_token(target_provider.as_str()).await {
            if token
                .login
                .as_deref()
                .is_some_and(|l| l.starts_with("via "))
            {
                if let Err(err) = store.delete_provider_token(target_provider.as_str()).await {
                    tracing::warn!(
                        provider = target_provider.as_str(),
                        error = %err,
                        "failed to clear secrets-supplied credential"
                    );
                } else {
                    info!(
                        provider = target_provider.as_str(),
                        "cleared secrets-supplied credential"
                    );
                    if let Some(hub) = event_hub {
                        hub.send(Event::auth_disconnected(target_provider));
                    }
                }
            }
        }
    }
}

/// Build status from stored config and Doppler auth state.
pub async fn secrets_status(store: &SharedStore) -> SecretsProviderStatus {
    // Check if Doppler is authed
    let doppler_connected = store
        .get_provider_token(Provider::Doppler.as_str())
        .await
        .ok()
        .flatten()
        .is_some();

    if !doppler_connected {
        return SecretsProviderStatus::disconnected();
    }

    let configs = store
        .list_secrets_provider_configs()
        .await
        .unwrap_or_default();

    let config = configs.into_iter().next();

    let mut keys: Vec<SuppliedKey> = Vec::new();
    for &(env_name, target_provider) in KEY_MAPPINGS {
        let present = store
            .get_provider_token(target_provider.as_str())
            .await
            .ok()
            .flatten()
            .is_some_and(|t| t.login.as_deref().is_some_and(|l| l.starts_with("via ")));
        keys.push(SuppliedKey {
            env_name: env_name.to_string(),
            provider: target_provider.as_str().to_string(),
            present,
        });
    }

    SecretsProviderStatus {
        provider: "doppler".to_string(),
        connected: true,
        project: config.as_ref().and_then(|c| c.project.clone()),
        config: config.as_ref().and_then(|c| c.config.clone()),
        keys,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lfd::id::LfdId;
    use crate::lfd::store::StorageConfig;
    use std::sync::Arc;

    async fn test_store() -> SharedStore {
        let db_path = std::env::temp_dir().join(format!("lfd-secrets-test-{}.db", LfdId::new()));
        let config = StorageConfig::sqlite(db_path);
        Arc::new(
            crate::lfd::store::open_store(&config)
                .await
                .expect("sqlite store should initialize"),
        )
    }

    /// Store a fake Doppler token so sync_secrets can read it.
    async fn store_doppler_token(store: &SharedStore, token: &str) {
        store
            .upsert_provider_token(&ProviderToken {
                provider: "doppler".to_string(),
                access_token: token.to_string(),
                refresh_token: None,
                expires_at: None,
                login: None,
                updated_at: 0,
                credential_type: CredentialType::OAuth,
            })
            .await
            .expect("store doppler token");
    }

    #[tokio::test]
    async fn clear_removes_only_secrets_supplied_credentials() {
        let store = test_store().await;

        // Store a token supplied by secrets provider
        store
            .upsert_provider_token(&ProviderToken {
                provider: "claude".to_string(),
                access_token: "sk-from-secrets".to_string(),
                refresh_token: None,
                expires_at: None,
                login: Some("via doppler".to_string()),
                updated_at: 0,
                credential_type: CredentialType::ApiKey,
            })
            .await
            .expect("upsert");

        // Store a token NOT from secrets provider
        store
            .upsert_provider_token(&ProviderToken {
                provider: "codex".to_string(),
                access_token: "sk-manual".to_string(),
                refresh_token: None,
                expires_at: None,
                login: Some("user@example.com".to_string()),
                updated_at: 0,
                credential_type: CredentialType::ApiKey,
            })
            .await
            .expect("upsert");

        clear_secrets_credentials(&store, None).await;

        // Claude token (from secrets) should be cleared
        let claude = store
            .get_provider_token("claude")
            .await
            .expect("store read");
        assert!(claude.is_none());

        // Codex token (manually set) should remain
        let codex = store
            .get_provider_token("codex")
            .await
            .expect("store read")
            .expect("codex token should remain");
        assert_eq!(codex.access_token, "sk-manual");
    }

    #[tokio::test]
    async fn status_reflects_connected_doppler_with_config() {
        let store = test_store().await;

        store_doppler_token(&store, "dp.ct.test-token").await;

        store
            .upsert_secrets_provider_config(&SecretsProviderConfig {
                provider: "doppler".to_string(),
                project: Some("myproject".to_string()),
                config: Some("dev".to_string()),
                updated_at: 0,
            })
            .await
            .expect("upsert config");

        store
            .upsert_provider_token(&ProviderToken {
                provider: "claude".to_string(),
                access_token: "sk-test".to_string(),
                refresh_token: None,
                expires_at: None,
                login: Some("via doppler".to_string()),
                updated_at: 0,
                credential_type: CredentialType::ApiKey,
            })
            .await
            .expect("upsert token");

        let status = secrets_status(&store).await;
        assert!(status.connected);
        assert_eq!(status.provider, "doppler");
        assert_eq!(status.project.as_deref(), Some("myproject"));

        let claude_key = status.keys.iter().find(|k| k.provider == "claude");
        let codex_key = status.keys.iter().find(|k| k.provider == "codex");
        assert!(claude_key.unwrap().present);
        assert!(!codex_key.unwrap().present);
    }

    #[tokio::test]
    async fn status_returns_disconnected_when_no_doppler_token() {
        let store = test_store().await;
        let status = secrets_status(&store).await;
        assert!(!status.connected);
        assert!(status.keys.iter().all(|k| !k.present));
    }

    #[tokio::test]
    async fn status_connected_but_no_config_selected() {
        let store = test_store().await;
        store_doppler_token(&store, "dp.ct.test-token").await;

        let status = secrets_status(&store).await;
        assert!(status.connected);
        assert!(status.project.is_none());
        assert!(status.config.is_none());
    }

    #[test]
    fn smart_default_prefers_dev() {
        let configs = vec![
            DopplerConfig {
                name: "prod".into(),
                environment: "production".into(),
            },
            DopplerConfig {
                name: "dev".into(),
                environment: "development".into(),
            },
            DopplerConfig {
                name: "staging".into(),
                environment: "staging".into(),
            },
        ];
        assert_eq!(smart_default_config(&configs).unwrap().name, "dev");
    }

    #[test]
    fn smart_default_falls_back_to_prod() {
        let configs = vec![
            DopplerConfig {
                name: "prod".into(),
                environment: "production".into(),
            },
            DopplerConfig {
                name: "staging".into(),
                environment: "staging".into(),
            },
        ];
        assert_eq!(smart_default_config(&configs).unwrap().name, "prod");
    }

    #[test]
    fn smart_default_falls_back_to_first() {
        let configs = vec![DopplerConfig {
            name: "custom".into(),
            environment: "custom".into(),
        }];
        assert_eq!(smart_default_config(&configs).unwrap().name, "custom");
    }

    #[test]
    fn smart_default_empty_returns_none() {
        let configs: Vec<DopplerConfig> = vec![];
        assert!(smart_default_config(&configs).is_none());
    }
}
