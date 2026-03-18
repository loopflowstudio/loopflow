//! Secrets provider integration.
//!
//! `SecretsProvider` is the boundary trait. `DopplerSecretsProvider` is the
//! first implementation. After connect or refresh, fetched secrets are matched
//! against known env-var names and persisted through the existing
//! credential-storage path.

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

/// A key supplied by a secrets provider and its mapping status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SuppliedKey {
    pub env_name: String,
    pub provider: String,
    pub present: bool,
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

/// Provider-agnostic secrets-fetching boundary.
#[async_trait::async_trait]
pub trait SecretsProvider: Send + Sync {
    fn name(&self) -> &str;

    /// Fetch all secrets from the provider. Returns env-var name → value.
    async fn fetch_secrets(
        &self,
        token: &str,
        project: &str,
        config: &str,
    ) -> Result<HashMap<String, String>, SecretsError>;
}

#[derive(Debug, thiserror::Error)]
pub enum SecretsError {
    #[error("HTTP request failed: {0}")]
    Http(String),
    #[error("invalid response: {0}")]
    InvalidResponse(String),
    #[error("unauthorized — token may be expired")]
    Unauthorized,
}

/// Doppler secrets provider implementation.
pub struct DopplerSecretsProvider;

#[async_trait::async_trait]
impl SecretsProvider for DopplerSecretsProvider {
    fn name(&self) -> &str {
        "doppler"
    }

    async fn fetch_secrets(
        &self,
        token: &str,
        project: &str,
        config: &str,
    ) -> Result<HashMap<String, String>, SecretsError> {
        let url = format!(
            "https://api.doppler.com/v3/configs/config/secrets/download?project={project}&config={config}&format=json"
        );
        let client = reqwest::Client::new();
        let response = client
            .get(&url)
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

        let secrets: HashMap<String, String> = response
            .json()
            .await
            .map_err(|e| SecretsError::InvalidResponse(e.to_string()))?;
        Ok(secrets)
    }
}

/// Sync secrets from the configured provider into provider tokens.
///
/// Returns the list of keys that were found and synced.
pub async fn sync_secrets(
    store: &SharedStore,
    provider: &dyn SecretsProvider,
    config: &SecretsProviderConfig,
    event_hub: Option<&EventHub>,
) -> Result<Vec<SuppliedKey>, SecretsError> {
    let project = config.project.as_deref().unwrap_or_default();
    let config_name = config.config.as_deref().unwrap_or_default();

    let secrets = provider
        .fetch_secrets(&config.access_token, project, config_name)
        .await?;

    let mut supplied_keys = Vec::new();

    for &(env_name, target_provider) in KEY_MAPPINGS {
        let value = secrets.get(env_name);
        supplied_keys.push(SuppliedKey {
            env_name: env_name.to_string(),
            provider: target_provider.as_str().to_string(),
            present: value.is_some(),
        });

        if let Some(value) = value {
            let token = ProviderToken {
                provider: target_provider.as_str().to_string(),
                access_token: value.clone(),
                refresh_token: None,
                expires_at: None,
                login: Some(format!("via {}", provider.name())),
                updated_at: crate::lfd::store::rows::now_unix(),
                credential_type: CredentialType::ApiKey,
            };
            if let Err(err) = store.upsert_provider_token(&token).await {
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
                        Some(format!("via {}", provider.name())),
                    ));
                }
            }
        }
    }

    if let Some(hub) = event_hub {
        hub.send(Event::secrets_synced(provider.name().to_string()));
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

/// Build status from stored config.
pub async fn secrets_status(store: &SharedStore) -> SecretsProviderStatus {
    let configs = store
        .list_secrets_provider_configs()
        .await
        .unwrap_or_default();

    let Some(config) = configs.into_iter().next() else {
        return SecretsProviderStatus::disconnected();
    };

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
        provider: config.provider,
        connected: true,
        project: config.project,
        config: config.config,
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

    struct MockSecretsProvider {
        secrets: HashMap<String, String>,
    }

    #[async_trait::async_trait]
    impl SecretsProvider for MockSecretsProvider {
        fn name(&self) -> &str {
            "mock"
        }

        async fn fetch_secrets(
            &self,
            _token: &str,
            _project: &str,
            _config: &str,
        ) -> Result<HashMap<String, String>, SecretsError> {
            Ok(self.secrets.clone())
        }
    }

    #[tokio::test]
    async fn sync_populates_matching_provider_tokens() {
        let store = test_store().await;
        let provider = MockSecretsProvider {
            secrets: HashMap::from([
                ("ANTHROPIC_API_KEY".to_string(), "sk-ant-test".to_string()),
                ("OPENAI_API_KEY".to_string(), "sk-oai-test".to_string()),
                ("UNRELATED_KEY".to_string(), "ignored".to_string()),
            ]),
        };

        let config = SecretsProviderConfig {
            provider: "mock".to_string(),
            access_token: "dp-token".to_string(),
            project: Some("myproject".to_string()),
            config: Some("dev".to_string()),
            updated_at: 0,
        };

        let keys = sync_secrets(&store, &provider, &config, None)
            .await
            .expect("sync should succeed");

        assert_eq!(keys.len(), 2);
        assert!(keys.iter().all(|k| k.present));

        let claude_token = store
            .get_provider_token("claude")
            .await
            .expect("store read")
            .expect("claude token should exist");
        assert_eq!(claude_token.access_token, "sk-ant-test");
        assert_eq!(claude_token.credential_type, CredentialType::ApiKey);
        assert_eq!(claude_token.login.as_deref(), Some("via mock"));

        let codex_token = store
            .get_provider_token("codex")
            .await
            .expect("store read")
            .expect("codex token should exist");
        assert_eq!(codex_token.access_token, "sk-oai-test");
    }

    #[tokio::test]
    async fn sync_marks_missing_keys_as_not_present() {
        let store = test_store().await;
        let provider = MockSecretsProvider {
            secrets: HashMap::from([("ANTHROPIC_API_KEY".to_string(), "sk-ant-test".to_string())]),
        };

        let config = SecretsProviderConfig {
            provider: "mock".to_string(),
            access_token: "dp-token".to_string(),
            project: Some("myproject".to_string()),
            config: Some("dev".to_string()),
            updated_at: 0,
        };

        let keys = sync_secrets(&store, &provider, &config, None)
            .await
            .expect("sync should succeed");

        let claude_key = keys.iter().find(|k| k.env_name == "ANTHROPIC_API_KEY");
        let codex_key = keys.iter().find(|k| k.env_name == "OPENAI_API_KEY");
        assert!(claude_key.unwrap().present);
        assert!(!codex_key.unwrap().present);
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
                login: Some("via mock".to_string()),
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
    async fn status_reflects_connected_provider_with_keys() {
        let store = test_store().await;

        store
            .upsert_secrets_provider_config(&SecretsProviderConfig {
                provider: "doppler".to_string(),
                access_token: "dp-token".to_string(),
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
    async fn status_returns_disconnected_when_no_config() {
        let store = test_store().await;
        let status = secrets_status(&store).await;
        assert!(!status.connected);
        assert!(status.keys.iter().all(|k| !k.present));
    }
}
