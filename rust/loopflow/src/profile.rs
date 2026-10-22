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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
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
    select_chrome_profile(local_chrome_profiles()?, requested)
}

fn select_chrome_profile(
    profiles: Vec<LocalChromeProfile>,
    requested: &str,
) -> Result<LocalChromeProfile, String> {
    let matches = profiles
        .iter()
        .filter(|profile| {
            profile.directory.eq_ignore_ascii_case(requested)
                || profile.name.eq_ignore_ascii_case(requested)
                || profile
                    .login
                    .as_deref()
                    .is_some_and(|login| login.eq_ignore_ascii_case(requested))
        })
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(format!(
            "Chrome profile '{requested}' matched {} profiles. Choices: {}",
            matches.len(),
            profiles
                .iter()
                .map(|p| format!("{} ({})", p.name, p.directory))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    validate_chrome_profile_identifier(&matches[0].directory)?;
    Ok(matches[0].clone())
}

pub fn local_chrome_profiles() -> Result<Vec<LocalChromeProfile>, String> {
    let path = chrome_local_state_path()?;
    let bytes = fs::read(&path).map_err(|error| format!("read Chrome profiles: {error}"))?;
    let state = serde_json::from_slice::<ChromeLocalState>(&bytes)
        .map_err(|_| "invalid Chrome profile catalog".to_string())?;
    let mut profiles = Vec::new();
    for (directory, profile) in state.profile.info_cache {
        profiles.push(LocalChromeProfile {
            directory,
            name: profile.name,
            login: profile.user_name.filter(|login| !login.trim().is_empty()),
        });
    }
    profiles.sort_by(|a, b| a.directory.cmp(&b.directory));
    Ok(profiles)
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

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AccessProfile {
    pub id: ProfileId,
    pub chrome_directory: String,
    pub expected_login: Option<EmailAddress>,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AuthBrowserBinding {
    pub provider: Provider,
    pub account_id: Option<ProviderAccountId>,
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
    use super::{select_chrome_profile, EmailAddress, LocalChromeProfile, ProfileId};

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
        let profiles = vec![
            LocalChromeProfile {
                directory: "Profile 3".to_string(),
                name: "Personal".to_string(),
                login: Some("personal@example.com".to_string()),
            },
            LocalChromeProfile {
                directory: "Profile 7".to_string(),
                name: "Loopflow".to_string(),
                login: Some("jack@example.com".to_string()),
            },
        ];

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
