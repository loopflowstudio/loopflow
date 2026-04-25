//! Shared provider credential files — Codex pattern adapted for loopflow.
//!
//! Provider tokens (OAuth access/refresh, or raw API keys) live under
//! `~/.lf/credentials/<provider>.json`, written by `lf auth <provider>` and
//! read by any lfd/Concerto/agent that runs as the same user. The shared
//! location is the source of truth; lfd's sqlite `provider_tokens` table
//! keeps lifecycle metadata (status, connected_at, events) but does not
//! own the token bytes.
//!
//! Splitting storage this way fixes the silo that otherwise appears when
//! multiple lfds run under one user — the CLI-spawned lfd and the Concerto
//! bundled lfd used to each carry their own `provider_tokens` table and
//! neither saw the other's auth. Now the file is the canonical store and
//! every lfd discovers auth on startup.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::lfd::store::{CredentialType, ProviderToken};

/// On-disk shape for `~/.lf/credentials/<provider>.json`. Kept minimal so
/// manual edits are straightforward (useful for debugging, and for tools
/// that provision tokens from a secrets manager).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderCredentialFile {
    pub provider: String,
    pub access_token: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<i64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub login: Option<String>,
    pub updated_at: i64,
    #[serde(default = "default_credential_type")]
    pub credential_type: CredentialType,
}

fn default_credential_type() -> CredentialType {
    CredentialType::OAuth
}

pub fn credentials_dir() -> PathBuf {
    crate::lfd::lf_home_dir().join("credentials")
}

pub fn credential_file_path(provider: &str) -> PathBuf {
    credentials_dir().join(format!("{provider}.json"))
}

/// Read the shared credential for a provider. Returns `None` if the file
/// doesn't exist or fails to parse — callers that want to fall back to
/// the legacy sqlite cache can do so themselves.
pub fn read(provider: &str) -> Option<ProviderToken> {
    read_from(&credentials_dir(), provider)
}

pub(crate) fn read_from(dir: &Path, provider: &str) -> Option<ProviderToken> {
    let path = dir.join(format!("{provider}.json"));
    let content = std::fs::read_to_string(path).ok()?;
    let file: ProviderCredentialFile = serde_json::from_str(&content).ok()?;
    Some(ProviderToken {
        provider: file.provider,
        access_token: file.access_token,
        refresh_token: file.refresh_token,
        expires_at: file.expires_at,
        login: file.login,
        updated_at: file.updated_at,
        credential_type: file.credential_type,
    })
}

/// Write the shared credential for a provider. Atomic: writes to a temp
/// file and renames, so a concurrent reader never sees half a file.
pub fn write(token: &ProviderToken) -> std::io::Result<()> {
    write_to(&credentials_dir(), token)
}

pub(crate) fn write_to(dir: &Path, token: &ProviderToken) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(format!("{}.json", token.provider));
    let tmp_path = path.with_extension("json.tmp");
    let file = ProviderCredentialFile {
        provider: token.provider.clone(),
        access_token: token.access_token.clone(),
        refresh_token: token.refresh_token.clone(),
        expires_at: token.expires_at,
        login: token.login.clone(),
        updated_at: token.updated_at,
        credential_type: token.credential_type,
    };
    let body = serde_json::to_vec_pretty(&file)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
    std::fs::write(&tmp_path, body)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp_path, std::fs::Permissions::from_mode(0o600))?;
    }
    std::fs::rename(&tmp_path, &path)?;
    Ok(())
}

/// Remove the credential file for a provider. No error if already absent.
pub fn delete(provider: &str) -> std::io::Result<()> {
    let path = credential_file_path(provider);
    match std::fs::remove_file(&path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample(provider: &str) -> ProviderToken {
        ProviderToken {
            provider: provider.to_string(),
            access_token: "access-abc".to_string(),
            refresh_token: Some("refresh-xyz".to_string()),
            expires_at: Some(1_800_000_000),
            login: Some("jack@example.com".to_string()),
            updated_at: 1_700_000_000,
            credential_type: CredentialType::OAuth,
        }
    }

    #[test]
    fn round_trip_preserves_all_fields() {
        let dir = TempDir::new().expect("temp dir");
        let token = sample("asana");
        write_to(dir.path(), &token).expect("write succeeds");
        let loaded = read_from(dir.path(), "asana").expect("read succeeds");
        assert_eq!(loaded.provider, "asana");
        assert_eq!(loaded.access_token, "access-abc");
        assert_eq!(loaded.refresh_token.as_deref(), Some("refresh-xyz"));
        assert_eq!(loaded.expires_at, Some(1_800_000_000));
        assert_eq!(loaded.login.as_deref(), Some("jack@example.com"));
        assert_eq!(loaded.credential_type, CredentialType::OAuth);
    }

    #[test]
    fn read_missing_returns_none() {
        let dir = TempDir::new().expect("temp dir");
        assert!(read_from(dir.path(), "asana").is_none());
    }

    #[test]
    fn write_creates_parent_directory() {
        let dir = TempDir::new().expect("temp dir");
        let nested = dir.path().join("nested/credentials");
        assert!(!nested.exists());
        write_to(&nested, &sample("asana")).expect("write succeeds");
        assert!(nested.join("asana.json").exists());
    }
}
