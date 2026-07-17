//! Chrome access venues and account-first provider routing.

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalChromeProfile {
    pub directory: String,
    pub name: String,
    pub login: Option<String>,
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
    let login = profile.user_name.filter(|login| !login.trim().is_empty());
    validate_chrome_profile_identifier(&directory)?;
    validate_chrome_profile_identifier(&profile.name)?;
    if let Some(login) = &login {
        validate_chrome_profile_identifier(login)?;
    }
    Ok(LocalChromeProfile {
        directory,
        name: profile.name,
        login,
    })
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

#[cfg(any(target_os = "macos", test))]
fn chrome_local_state_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or_else(|| "cannot resolve home directory".to_string())?;
    Ok(home.join("Library/Application Support/Google/Chrome/Local State"))
}

#[cfg(all(not(target_os = "macos"), not(test)))]
fn chrome_local_state_path() -> Result<PathBuf, String> {
    Err("Chrome profile discovery is currently supported on macOS only".to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccessProfile {
    pub id: ProfileId,
    pub chrome_directory: String,
    pub expected_login: EmailAddress,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AccountAccessProfile {
    pub provider: Provider,
    pub account_id: ProviderAccountId,
    pub position: usize,
    pub profile_id: ProfileId,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum RouteScope {
    Repo(RepoId),
    Default,
}

impl RouteScope {
    pub fn kind(&self) -> &'static str {
        match self {
            Self::Repo(_) => "repo",
            Self::Default => "default",
        }
    }

    pub fn id(&self) -> &str {
        match self {
            Self::Repo(repo_id) => repo_id.as_str(),
            Self::Default => "",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderRoute {
    pub scope: RouteScope,
    pub provider: Provider,
    pub accounts: Vec<ProviderAccountId>,
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
    fn profile_ids_are_safe_printable_slugs() {
        assert_eq!(
            ProfileId::parse(" Profile 3 ").unwrap().as_str(),
            "Profile 3"
        );
        assert!(ProfileId::parse("Loopflow").is_ok());
        assert!(ProfileId::parse("../loopflow").is_err());
    }

    #[test]
    fn email_addresses_are_normalized() {
        assert_eq!(
            EmailAddress::parse(" Primary@Example.com ")
                .unwrap()
                .as_str(),
            "primary@example.com"
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
                name: "Loopflow".to_string(),
                login: Some("jack@example.com".to_string()),
            }
        );
    }
}
