//! Personal routing profiles that bind repositories, browser identities, and
//! reusable provider accounts.

use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::path::PathBuf;

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
pub struct LocalChromeProfile {
    pub directory: String,
    pub label: String,
}

#[derive(Debug, serde::Deserialize)]
struct ChromeLocalState {
    profile: ChromeProfileCatalog,
}

#[derive(Debug, serde::Deserialize)]
struct ChromeProfileCatalog {
    info_cache: HashMap<String, ChromeProfileInfo>,
}

#[derive(Debug, serde::Deserialize)]
struct ChromeProfileInfo {
    name: String,
    user_name: Option<String>,
}

pub fn resolve_local_chrome_profile(requested: &str) -> Result<LocalChromeProfile, String> {
    let requested = requested.trim();
    if requested.is_empty() {
        return Err("Chrome profile cannot be empty".to_string());
    }
    let local_state_path = chrome_local_state_path()?;
    let bytes = fs::read(&local_state_path)
        .map_err(|error| format!("read {}: {error}", local_state_path.display()))?;
    let local_state = serde_json::from_slice::<ChromeLocalState>(&bytes)
        .map_err(|error| format!("parse {}: {error}", local_state_path.display()))?;
    select_chrome_profile(local_state.profile.info_cache, requested)
}

fn select_chrome_profile(
    profiles: HashMap<String, ChromeProfileInfo>,
    requested: &str,
) -> Result<LocalChromeProfile, String> {
    let mut matches = profiles
        .into_iter()
        .filter(|(directory, profile)| {
            directory.eq_ignore_ascii_case(requested)
                || profile.name.eq_ignore_ascii_case(requested)
                || profile
                    .user_name
                    .as_deref()
                    .is_some_and(|login| login.eq_ignore_ascii_case(requested))
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(format!(
            "Chrome profile '{}' matched {} profiles",
            requested,
            matches.len()
        ));
    }
    let (directory, profile) = matches
        .pop()
        .expect("one matched Chrome profile should exist");
    let label = profile
        .user_name
        .filter(|login| !login.trim().is_empty())
        .unwrap_or(profile.name);
    validate_chrome_profile_identifier(&directory)?;
    validate_chrome_profile_identifier(&label)?;
    Ok(LocalChromeProfile { directory, label })
}

fn validate_chrome_profile_identifier(value: &str) -> Result<(), String> {
    if value.is_empty()
        || value.len() > 128
        || !value.chars().all(|character| {
            character.is_ascii_alphanumeric()
                || matches!(character, '@' | '.' | '_' | '+' | '-' | ' ')
        })
    {
        return Err("unsupported Chrome profile identifier".to_string());
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn chrome_local_state_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "cannot resolve home directory".to_string())?;
    Ok(home.join("Library/Application Support/Google/Chrome/Local State"))
}

#[cfg(not(target_os = "macos"))]
fn chrome_local_state_path() -> Result<PathBuf, String> {
    Err("Chrome profile discovery is currently supported on macOS only".to_string())
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
    use std::collections::HashMap;

    use super::{
        select_chrome_profile, ChromeProfileInfo, EmailAddress, LocalChromeProfile, ProfileId,
    };

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

    #[test]
    fn chrome_profiles_resolve_by_signed_in_email() {
        let profiles = HashMap::from([
            (
                "Profile 3".to_string(),
                ChromeProfileInfo {
                    name: "jackstah".to_string(),
                    user_name: Some("jackstah@example.com".to_string()),
                },
            ),
            (
                "Profile 7".to_string(),
                ChromeProfileInfo {
                    name: "Loopflow".to_string(),
                    user_name: Some("jack@example.com".to_string()),
                },
            ),
        ]);

        assert_eq!(
            select_chrome_profile(profiles, "jack@example.com").unwrap(),
            LocalChromeProfile {
                directory: "Profile 7".to_string(),
                label: "jack@example.com".to_string(),
            }
        );
    }
}
