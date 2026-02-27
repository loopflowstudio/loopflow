use serde::Serialize;

use crate::lfd::provider_auth::Provider;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct ModelInfo {
    pub id: &'static str,
    pub display_name: &'static str,
    pub provider: Provider,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cost_rates: Option<CostRates>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct CostRates {
    pub input_per_mtok: f64,
    pub output_per_mtok: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_read_per_mtok: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cache_write_per_mtok: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProviderInfo {
    pub provider: Provider,
    pub auth_status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    pub billing: &'static str,
    pub models: Vec<ModelInfo>,
}

const CLAUDE_MODELS: [ModelInfo; 3] = [
    ModelInfo {
        id: "claude-opus-4-6",
        display_name: "Claude Opus 4.6",
        provider: Provider::Claude,
        cost_rates: None,
    },
    ModelInfo {
        id: "claude-sonnet-4",
        display_name: "Claude Sonnet 4",
        provider: Provider::Claude,
        cost_rates: None,
    },
    ModelInfo {
        id: "claude-haiku-4-5",
        display_name: "Claude Haiku 4.5",
        provider: Provider::Claude,
        cost_rates: None,
    },
];

const CODEX_MODELS: [ModelInfo; 1] = [ModelInfo {
    id: "gpt-5.1-codex",
    display_name: "GPT-5.1 Codex",
    provider: Provider::Codex,
    cost_rates: None,
}];

const OPENCODE_ZEN_MODELS: [ModelInfo; 2] = [
    ModelInfo {
        id: "opencode/kimi-k2.5",
        display_name: "Kimi K2.5",
        provider: Provider::OpenCodeZen,
        cost_rates: None,
    },
    ModelInfo {
        id: "moonshotai/kimi-k2",
        display_name: "Kimi K2",
        provider: Provider::OpenCodeZen,
        cost_rates: None,
    },
];

pub fn model_capable_providers() -> [Provider; 3] {
    [Provider::Claude, Provider::Codex, Provider::OpenCodeZen]
}

pub fn billing_for_provider(provider: Provider) -> &'static str {
    match provider {
        Provider::Claude | Provider::Codex => "subscription",
        Provider::OpenCodeZen => "per_token",
        Provider::GitHub => "auth_only",
    }
}

pub fn models_for_provider(provider: Provider) -> &'static [ModelInfo] {
    match provider {
        Provider::Claude => &CLAUDE_MODELS,
        Provider::Codex => &CODEX_MODELS,
        Provider::OpenCodeZen => &OPENCODE_ZEN_MODELS,
        Provider::GitHub => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_capable_provider_list_excludes_github() {
        let providers = model_capable_providers();
        assert_eq!(
            providers,
            [Provider::Claude, Provider::Codex, Provider::OpenCodeZen]
        );
    }

    #[test]
    fn provider_model_registry_returns_expected_sets() {
        assert_eq!(models_for_provider(Provider::Claude).len(), 3);
        assert_eq!(models_for_provider(Provider::Codex).len(), 1);
        assert_eq!(models_for_provider(Provider::OpenCodeZen).len(), 2);
        assert!(models_for_provider(Provider::GitHub).is_empty());
    }
}
