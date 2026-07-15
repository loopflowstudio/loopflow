//! Personal routing profiles that bind repositories, browser identities, and
//! reusable provider accounts.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::provider_auth::Provider;
use crate::repository::RepoId;
use crate::store::ProviderAccountId;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfileId(String);

impl ProfileId {
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.is_empty() || value.len() > 63 {
            return Err("profile id must be 1-63 characters".to_string());
        }
        let mut chars = value.chars();
        let first = chars
            .next()
            .expect("non-empty profile id has a first character");
        if !first.is_ascii_lowercase() && !first.is_ascii_digit() {
            return Err("profile id must start with a lowercase letter or number".to_string());
        }
        if !chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-' || ch == '_')
        {
            return Err(
                "profile id may contain lowercase letters, numbers, '-' and '_'".to_string(),
            );
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
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
            || value.chars().any(char::is_whitespace)
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HostId(String);

impl HostId {
    pub fn parse(value: &str) -> Result<Self, String> {
        let value = value.trim();
        if value.is_empty() || value.len() > 255 || value.chars().any(char::is_control) {
            return Err("host id must be 1-255 printable characters".to_string());
        }
        Ok(Self(value.to_string()))
    }

    pub fn local() -> Result<Self, String> {
        Self::parse(&gethostname::gethostname().to_string_lossy())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for HostId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub id: ProfileId,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChromeProfileBinding {
    pub profile_id: ProfileId,
    pub host_id: HostId,
    pub chrome_directory: String,
    pub google_email: EmailAddress,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileProviderAccount {
    pub profile_id: ProfileId,
    pub provider: Provider,
    pub account_id: ProviderAccountId,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderProfileCandidate {
    pub profile_id: ProfileId,
    pub account_id: ProviderAccountId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoProfileRoute {
    pub repo_id: RepoId,
    pub default_profile: ProfileId,
    pub backup_profiles: Vec<ProfileId>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[cfg(test)]
mod tests {
    use super::{EmailAddress, ProfileId};

    #[test]
    fn profile_ids_are_path_safe() {
        assert_eq!(
            ProfileId::parse("loopflow-eng").unwrap().as_str(),
            "loopflow-eng"
        );
        assert!(ProfileId::parse("Loopflow").is_err());
        assert!(ProfileId::parse("../loopflow").is_err());
    }

    #[test]
    fn email_addresses_are_normalized() {
        assert_eq!(
            EmailAddress::parse(" Jack@Loopflow.Studio ")
                .unwrap()
                .as_str(),
            "jack@loopflow.studio"
        );
        assert!(EmailAddress::parse("not-an-email").is_err());
    }
}
