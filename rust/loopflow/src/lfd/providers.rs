use serde::Serialize;

#[cfg(test)]
use crate::lfd::provider_auth::AuthStatus;
use crate::lfd::provider_auth::{Provider, ProviderAuthSnapshot};

// ── Types ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ProviderInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    pub models: &'static [ModelInfo],
    pub is_default: bool,
    pub auth_provider: Option<Provider>,
    /// Known API rates for models used through this harness.
    /// Empty for subscription harnesses (Claude, Codex).
    pub model_rates: &'static [ModelRate],
}

#[derive(Debug, Clone, Serialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    pub is_default: bool,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CostRates {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    pub cache_read_per_mtok: f64,
    pub cache_write_per_mtok: f64,
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
        model_rates: &[],
    },
    ProviderInfo {
        id: "codex",
        display_name: "Codex",
        models: CODEX_MODELS,
        is_default: false,
        auth_provider: Some(Provider::Codex),
        model_rates: &[],
    },
    ProviderInfo {
        id: "opencode",
        display_name: "OpenCode",
        models: OPENCODE_MODELS,
        is_default: false,
        auth_provider: None,
        model_rates: OPENCODE_MODEL_RATES,
    },
];

// ── Model rate lookup ─────────────────────────────────────────────────
//
// Per-harness rate tables. Subscription harnesses (Claude, Codex) have
// no rates. OpenCode hits real APIs with real per-token billing.

#[derive(Debug, Clone)]
pub struct ModelRate {
    pub model_prefix: &'static str,
    pub rates: CostRates,
}

const OPENCODE_MODEL_RATES: &[ModelRate] = &[
    // Moonshot — Kimi K2
    ModelRate {
        model_prefix: "kimi-k2",
        rates: CostRates {
            input_per_mtok: 0.60,
            output_per_mtok: 2.50,
            cache_read_per_mtok: 0.15,
            cache_write_per_mtok: 0.60,
        },
    },
    // Alibaba — Qwen3
    ModelRate {
        model_prefix: "qwen3-coder",
        rates: CostRates {
            input_per_mtok: 0.22,
            output_per_mtok: 1.0,
            cache_read_per_mtok: 0.22,
            cache_write_per_mtok: 0.22,
        },
    },
    ModelRate {
        model_prefix: "qwen3-max",
        rates: CostRates {
            input_per_mtok: 1.20,
            output_per_mtok: 6.0,
            cache_read_per_mtok: 1.20,
            cache_write_per_mtok: 1.20,
        },
    },
];

/// Look up cost rates for a model within a specific harness.
/// Matches by prefix so versioned model IDs (e.g. "claude-sonnet-4-20250514")
/// resolve to the base model's rates. Returns None for subscription
/// harnesses or unknown models.
pub fn lookup_cost_rates(harness: &str, model: &str) -> Option<CostRates> {
    let provider = PROVIDER_CATALOG.iter().find(|p| p.id == harness)?;
    provider
        .model_rates
        .iter()
        .find(|mr| model.starts_with(mr.model_prefix))
        .map(|mr| mr.rates)
}

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

    #[test]
    fn opencode_has_rates_for_known_models() {
        assert!(lookup_cost_rates("opencode", "kimi-k2").is_some());
        assert!(lookup_cost_rates("opencode", "kimi-k2-0711").is_some());
        assert!(lookup_cost_rates("opencode", "qwen3-coder").is_some());
        assert!(lookup_cost_rates("opencode", "qwen3-max").is_some());
    }

    #[test]
    fn subscription_harnesses_have_no_rates() {
        assert!(lookup_cost_rates("claude", "anything").is_none());
        assert!(lookup_cost_rates("codex", "anything").is_none());
    }

    #[test]
    fn unknown_model_returns_none() {
        assert!(lookup_cost_rates("opencode", "unknown-model-v1").is_none());
    }

    #[test]
    fn unknown_harness_returns_none() {
        assert!(lookup_cost_rates("nonexistent", "kimi-k2").is_none());
    }
}
