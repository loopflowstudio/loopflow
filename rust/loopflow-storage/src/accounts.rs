//! Saved provider accounts, credentials and capacity observations.

use crate::profile::EmailAddress;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CredentialType {
    OAuth,
    ApiKey,
}

impl CredentialType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OAuth => "oauth",
            Self::ApiKey => "apikey",
        }
    }

    pub fn from_db(value: &str) -> Self {
        match value {
            "apikey" => Self::ApiKey,
            _ => Self::OAuth,
        }
    }
}

impl std::fmt::Display for CredentialType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug)]
pub enum ProviderTokenReplacement {
    Replaced,
    Changed(ProviderToken),
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderToken {
    pub provider: String,
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub oauth_client_id: Option<String>,
    pub expires_at: Option<i64>,
    pub login: Option<String>,
    pub updated_at: i64,
    pub credential_type: CredentialType,
}

#[derive(
    schemars::JsonSchema,
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    serde::Serialize,
    serde::Deserialize,
)]
#[serde(transparent)]
pub struct ProviderAccountId(String);

impl ProviderAccountId {
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.is_empty() || value.len() > 63 {
            return Err("account id must be 1-63 characters".to_string());
        }
        let mut chars = value.chars();
        let first = chars
            .next()
            .expect("non-empty account id has a first character");
        if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
            return Err("account id must start with a lowercase letter or number".to_string());
        }
        if !chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
        {
            return Err(
                "account id may contain lowercase letters, numbers, '-' and '_'".to_string(),
            );
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProviderAccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProviderAccount {
    pub provider: String,
    pub account_id: ProviderAccountId,
    pub home: Option<PathBuf>,
    pub login_email: Option<EmailAddress>,
    pub observed_email: Option<String>,
    pub observed_subject: Option<String>,
    pub observed_credential_digest: Option<String>,
    pub observed_plan: Option<String>,
    pub credential_state: CredentialState,
    pub routing_state: RoutingState,
    pub plan: Option<String>,
    pub paid_through: Option<time::Date>,
    pub utilization_percent: Option<u8>,
    pub cooldown_until: Option<i64>,
    pub cooldown_reason: Option<String>,
    pub last_selected_at: Option<i64>,
    pub created_at: i64,
    pub updated_at: i64,
}

impl ProviderAccount {
    pub fn effective_routing_state(&self, today: time::Date) -> RoutingState {
        if self.routing_state == RoutingState::Automatic
            && self.paid_through.is_some_and(|date| date < today)
        {
            RoutingState::ExplicitOnly
        } else {
            self.routing_state
        }
    }

    pub fn eligible_for_automatic_routing(&self, today: time::Date) -> bool {
        self.credential_state == CredentialState::Connected
            && self.effective_routing_state(today) == RoutingState::Automatic
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CredentialState {
    Connected,
    Missing,
}

impl CredentialState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Connected => "connected",
            Self::Missing => "missing",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, String> {
        match value {
            "connected" => Ok(Self::Connected),
            "missing" => Ok(Self::Missing),
            other => Err(format!("unknown credential state '{other}'")),
        }
    }
}

#[derive(
    schemars::JsonSchema, Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum RoutingState {
    Automatic,
    ExplicitOnly,
    Disabled,
}

impl RoutingState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Automatic => "automatic",
            Self::ExplicitOnly => "explicit_only",
            Self::Disabled => "disabled",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, String> {
        match value {
            "automatic" => Ok(Self::Automatic),
            "explicit_only" => Ok(Self::ExplicitOnly),
            "disabled" => Ok(Self::Disabled),
            other => Err(format!("unknown routing state '{other}'")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderAccountSelection {
    pub account: ProviderAccount,
}

#[derive(Debug)]
pub struct AccountCapacity {
    pub utilization_percent: Option<u8>,
    pub cooldown_until: Option<i64>,
    pub cooldown_reason: Option<String>,
    pub windows: Vec<AccountLimitWindow>,
}

/// One observed subscription rate-limit window: how much of the plan's
/// `session`/`weekly`/`weekly:<model>` window an account has consumed.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct AccountLimitWindow {
    pub window: String,
    pub used_percent: u8,
    pub resets_at: Option<i64>,
    pub plan: Option<String>,
    /// The one model this window limits; `None` limits every model.
    pub model: Option<String>,
    /// Set while the window holds launches it affects back.
    pub limited_until: Option<i64>,
}

/// A stored window observation for one managed account.
#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct AccountLimitRow {
    pub provider: String,
    pub account_id: ProviderAccountId,
    pub window: String,
    pub used_percent: u8,
    pub resets_at: Option<i64>,
    pub plan: Option<String>,
    pub observed_at: i64,
    /// 'stream' when a running harness reported it; 'poll' when asked for.
    pub source: String,
    /// The one model this window limits; `None` limits every model.
    pub model: Option<String>,
    /// Set while the window holds launches it affects back.
    pub limited_until: Option<i64>,
}
