use crate::CredentialType;
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
#[non_exhaustive]
pub enum Provider {
    GitHub,
    Claude,
    Codex,
    OpenCodeZen,
    Linear,
    Asana,
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
            Self::Asana => "asana",
            Self::Doppler => "doppler",
        }
    }

    pub fn all() -> [Self; 7] {
        [
            Self::GitHub,
            Self::Claude,
            Self::Codex,
            Self::OpenCodeZen,
            Self::Linear,
            Self::Asana,
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
            Self::Asana => "Asana",
            Self::Doppler => "Doppler",
        }
    }

    pub fn api_key_env_name(self) -> Option<&'static str> {
        match self {
            Self::Claude => Some("ANTHROPIC_API_KEY"),
            Self::Codex => Some("OPENAI_API_KEY"),
            Self::OpenCodeZen => Some("OPENCODE_API_KEY"),
            Self::GitHub | Self::Linear | Self::Asana | Self::Doppler => None,
        }
    }

    pub fn api_key_bills_per_token(self) -> bool {
        matches!(self, Self::Claude | Self::Codex | Self::OpenCodeZen)
    }

    pub fn api_key_configure_error(self) -> Option<&'static str> {
        match self {
            Self::Linear => {
                Some("Linear requires OAuth. Run 'lf machine login linear' to connect.")
            }
            Self::Asana => Some("Asana requires OAuth. Run 'lf machine login asana' to connect."),
            _ => None,
        }
    }

    /// Whether this provider can refresh without user interaction.
    pub fn supports_automatic_refresh(self) -> bool {
        matches!(self, Self::GitHub | Self::Linear | Self::Asana)
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
            "asana" => Ok(Self::Asana),
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
pub struct ProviderAuthSnapshot {
    pub provider: Provider,
    pub status: AuthStatus,
    pub expires_at: Option<i64>,
    pub next_refresh_at: Option<i64>,
    pub credential_type: Option<CredentialType>,
}
