pub mod attention;
pub mod auth;
pub mod bridge;
pub(crate) mod client;
pub mod config;
pub mod events;
pub mod executor;
pub mod github;
pub mod http;
pub(crate) mod http_client;
pub mod id;
pub mod journal;
pub mod lf_exec;
pub mod live_pr;
pub mod obs;
pub mod output;
pub mod pm;
pub mod providers;
pub mod queue;
pub(crate) mod redaction;
pub mod security;
pub mod service;
pub(crate) mod session_token;
pub mod triggers;
pub mod types;

use std::path::{Path, PathBuf};

use self::auth::AuthProvider;
use self::config::{AuthConfig, LfdConfig};
use crate::lfd::security::path_within_root_planned;
use crate::lfdb::StorageConfig;
use secrecy::ExposeSecret;

/// Set up bearer-token auth for local and self-hosted remote clients.
pub fn setup_auth(config: &LfdConfig) -> AuthProvider {
    AuthProvider::Bearer {
        session_token: load_or_create_session_token(&config.auth),
    }
}

fn load_or_create_session_token(auth: &AuthConfig) -> secrecy::SecretString {
    if let Some(token) = auth.token.as_ref() {
        if let Err(err) = self::session_token::write(token.expose_secret()) {
            tracing::error!(error = %err, "failed to write session token");
            std::process::exit(1);
        }
        tracing::info!(
            path = %self::session_token::token_path().display(),
            "session token written from override"
        );
        return token.clone();
    }

    match self::session_token::generate_and_write() {
        Ok(token) => {
            tracing::info!(
                path = %self::session_token::token_path().display(),
                "session token written"
            );
            secrecy::SecretString::from(token)
        }
        Err(err) => {
            tracing::error!(error = %err, "failed to write session token");
            std::process::exit(1);
        }
    }
}

pub(crate) fn lf_home_dir() -> PathBuf {
    if let Ok(home) = std::env::var("LF_HOME") {
        return PathBuf::from(home);
    }
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".lf")
}

pub fn default_db_path() -> PathBuf {
    lf_home_dir().join("lfd.db")
}

pub fn storage_config_from_env() -> Result<StorageConfig, std::io::Error> {
    let db_root = default_db_path()
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "failed to resolve sqlite root directory",
            )
        })?;
    std::fs::create_dir_all(&db_root)?;

    let db_candidate = std::env::var("LFD_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("lfd.db"));
    let db_path = if db_candidate.is_absolute() {
        let parent = db_candidate.parent().ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "invalid LFD_DB_PATH: absolute path must include a parent directory",
            )
        })?;
        std::fs::create_dir_all(parent)?;
        db_candidate
    } else {
        path_within_root_planned(&db_root, &db_candidate).map_err(|err| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                format!("invalid LFD_DB_PATH: {err}"),
            )
        })?
    };

    Ok(StorageConfig::sqlite(db_path))
}

pub fn default_output_dir() -> PathBuf {
    lf_home_dir().join("output")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lf_home_dir_uses_env_var() {
        // LF_HOME is process-global and the run ledger resolves it at write
        // time — mutations must hold the shared env lock.
        let _guard = crate::journal::test_env_lock();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().to_path_buf();

        // Save and restore to avoid polluting other tests.
        let prev = std::env::var("LF_HOME").ok();
        std::env::set_var("LF_HOME", &path);
        assert_eq!(lf_home_dir(), path);

        std::env::remove_var("LF_HOME");
        let default = lf_home_dir();
        assert!(default.ends_with(".lf"));

        if let Some(val) = prev {
            std::env::set_var("LF_HOME", val);
        }
    }
}
