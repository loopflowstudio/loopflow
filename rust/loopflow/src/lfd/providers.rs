use serde::Serialize;

#[cfg(test)]
use crate::provider_auth::AuthStatus;
use crate::provider_auth::{Provider, ProviderAuthSnapshot};

// ── Types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    pub models: &'static [ModelInfo],
    pub is_default: bool,
    pub auth_provider: Option<Provider>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    pub is_default: bool,
}

// ── Static catalog ─────────────────────────────────────────────────────

const CLAUDE_MODELS: &[ModelInfo] = &[
    ModelInfo {
        id: "opus",
        display_name: "Claude Opus 4",
        is_default: true,
    },
    ModelInfo {
        id: "sonnet",
        display_name: "Claude Sonnet 4",
        is_default: false,
    },
    ModelInfo {
        id: "haiku",
        display_name: "Claude Haiku 3.5",
        is_default: false,
    },
];

const CODEX_MODELS: &[ModelInfo] = &[ModelInfo {
    id: "codex",
    display_name: "Codex",
    is_default: true,
}];

const OPENCODE_MODELS: &[ModelInfo] = &[ModelInfo {
    id: "opencode",
    display_name: "OpenCode",
    is_default: true,
}];

pub static PROVIDER_CATALOG: &[ProviderInfo] = &[
    ProviderInfo {
        id: "claude",
        display_name: "Claude",
        models: CLAUDE_MODELS,
        is_default: true,
        auth_provider: Some(Provider::Claude),
    },
    ProviderInfo {
        id: "codex",
        display_name: "Codex",
        models: CODEX_MODELS,
        is_default: false,
        auth_provider: Some(Provider::Codex),
    },
    ProviderInfo {
        id: "opencode",
        display_name: "OpenCode",
        models: OPENCODE_MODELS,
        is_default: false,
        auth_provider: None,
    },
];

// ── Auth merging ───────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct ProviderInfoDto {
    pub object: &'static str,
    pub id: &'static str,
    pub display_name: &'static str,
    pub models: &'static [ModelInfo],
    pub is_default: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_status: Option<AuthStatusDto>,
}

#[derive(Debug, Serialize)]
pub struct AuthStatusDto {
    pub status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
}

pub fn merge_auth(
    catalog: &[ProviderInfo],
    snapshots: &[ProviderAuthSnapshot],
) -> Vec<ProviderInfoDto> {
    catalog
        .iter()
        .map(|provider| {
            let auth_status = provider.auth_provider.and_then(|ap| {
                snapshots
                    .iter()
                    .find(|s| s.provider == ap)
                    .map(|s| AuthStatusDto {
                        status: s.status.as_str(),
                        login: s.status.login(),
                    })
            });

            ProviderInfoDto {
                object: "provider",
                id: provider.id,
                display_name: provider.display_name,
                models: provider.models,
                is_default: provider.is_default,
                auth_status,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_has_exactly_one_default() {
        let defaults: Vec<_> = PROVIDER_CATALOG.iter().filter(|p| p.is_default).collect();
        assert_eq!(defaults.len(), 1);
        assert_eq!(defaults[0].id, "claude");
    }

    #[test]
    fn each_provider_has_at_least_one_model() {
        for provider in PROVIDER_CATALOG {
            assert!(!provider.models.is_empty(), "{} has no models", provider.id);
        }
    }

    #[test]
    fn each_provider_has_exactly_one_default_model() {
        for provider in PROVIDER_CATALOG {
            let defaults: Vec<_> = provider.models.iter().filter(|m| m.is_default).collect();
            assert_eq!(
                defaults.len(),
                1,
                "{} should have exactly one default model",
                provider.id
            );
        }
    }

    #[test]
    fn merge_auth_attaches_active_status() {
        let snapshots = vec![ProviderAuthSnapshot {
            provider: Provider::Claude,
            status: AuthStatus::Active {
                login: Some("jack".to_string()),
            },
            expires_at: None,
            next_refresh_at: None,
            credential_type: None,
        }];

        let merged = merge_auth(PROVIDER_CATALOG, &snapshots);
        let claude = merged.iter().find(|p| p.id == "claude").expect("claude");
        let auth = claude.auth_status.as_ref().expect("auth_status");
        assert_eq!(auth.status, "active");
        assert_eq!(auth.login.as_deref(), Some("jack"));
    }

    #[test]
    fn merge_auth_leaves_opencode_without_auth() {
        let snapshots = vec![];
        let merged = merge_auth(PROVIDER_CATALOG, &snapshots);
        let opencode = merged
            .iter()
            .find(|p| p.id == "opencode")
            .expect("opencode");
        assert!(opencode.auth_status.is_none());
    }
}
