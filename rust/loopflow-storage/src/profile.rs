use crate::provider::Provider;
use crate::ProviderAccountId;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(transparent)]
pub struct ProfileId(String);

impl ProfileId {
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.is_empty()
            || value.len() > 128
            || !value.chars().all(|character| {
                character.is_ascii_alphanumeric()
                    || matches!(character, '@' | '.' | '_' | '+' | '-' | ' ')
            })
        {
            return Err("profile id must be 1-128 safe printable characters".to_string());
        }
        Ok(Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ProfileId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(schemars::JsonSchema, Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EmailAddress(String);

impl EmailAddress {
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim().to_ascii_lowercase();
        let Some((local, domain)) = value.split_once('@') else {
            return Err("email address must contain '@'".to_string());
        };
        if local.is_empty()
            || domain.is_empty()
            || domain.contains('@')
            || value
                .chars()
                .any(|character| character.is_whitespace() || character.is_control())
        {
            return Err("invalid email address".to_string());
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EmailAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct AccessProfile {
    pub id: ProfileId,
    pub chrome_directory: String,
    pub expected_login: Option<EmailAddress>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(
    Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, schemars::JsonSchema,
)]
pub struct AuthBrowserBinding {
    pub provider: Provider,
    pub account_id: Option<ProviderAccountId>,
    pub position: usize,
    pub profile_id: ProfileId,
}
