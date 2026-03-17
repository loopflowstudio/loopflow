pub mod address;
pub mod attention;
pub mod auth;
pub mod config;
pub mod credential_socket;
pub mod credentials;
pub mod events;
pub mod executor;
pub mod github;
pub mod http;
pub mod http_client;
pub mod id;
pub mod live_pr;
pub mod machine_id;
pub mod obs;
pub mod output;
pub mod pm;
pub mod provider_auth;
pub mod providers;
pub mod queue;
pub mod redaction;
pub mod registration;
pub mod scheduler;
pub mod security;
pub mod service;
pub mod session_token;
pub mod sessions;
pub mod store;
pub mod token_ledger;
pub mod triggers;
pub mod types;

use std::net::SocketAddr;
use std::path::PathBuf;

use secrecy::ExposeSecret;
use tokio_util::sync::CancellationToken;

use self::auth::AuthProvider;
use self::config::{AuthConfig, AuthMode, LfdConfig};
use self::registration::RegistrationClient;
use self::store::{SharedStore, StorageConfig};
use self::token_ledger::TokenLedger;

/// Set up auth provider based on config.
///
/// Returns the auth provider, an optional registration client (for Studio
/// auth status/deregistration), and optional credentials for deregistration.
pub async fn setup_auth(
    config: &LfdConfig,
    store: SharedStore,
    storage_config: &StorageConfig,
    http_addr: SocketAddr,
    cancel: CancellationToken,
) -> (
    AuthProvider,
    Option<RegistrationClient>,
    Option<(String, String)>,
) {
    match config.auth.mode {
        AuthMode::Local => (
            AuthProvider::Local {
                session_token: load_or_create_session_token(&config.auth),
            },
            None,
            None,
        ),
        AuthMode::Studio => {
            setup_studio_registration(config, store, storage_config, http_addr, cancel).await
        }
    }
}

async fn setup_studio_registration(
    config: &LfdConfig,
    store: SharedStore,
    storage_config: &StorageConfig,
    http_addr: SocketAddr,
    cancel: CancellationToken,
) -> (
    AuthProvider,
    Option<RegistrationClient>,
    Option<(String, String)>,
) {
    let local_token = load_or_create_session_token(&config.auth);

    let Some(jwt) = self::credentials::load_jwt() else {
        tracing::error!("auth.mode=studio requires a JWT in ~/.lf/credentials.json");
        std::process::exit(1);
    };

    let path = match storage_config {
        StorageConfig::Sqlite { path } => path.clone(),
        StorageConfig::Postgres { .. } => crate::lfd::lf_home_dir().join("connection_tokens.db"),
    };
    let ledger = TokenLedger::new(path.clone())
        .await
        .unwrap_or_else(|error| {
            tracing::error!(error = %error, "failed to initialize connection token ledger");
            std::process::exit(1);
        });

    let prune_ledger = ledger.clone();
    let prune_cancel = cancel.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(300));
        interval.tick().await;
        loop {
            tokio::select! {
                _ = prune_cancel.cancelled() => break,
                _ = interval.tick() => {
                    if let Err(error) = prune_ledger.prune().await {
                        tracing::warn!(error = %error, "connection token prune failed");
                    }
                }
            }
        }
    });

    let mid = self::machine_id::machine_id();
    let machine_name = self::machine_id::machine_name();
    let base_url = &config.auth.base_url;
    let client =
        RegistrationClient::with_context_and_ledger(base_url, store, http_addr, ledger.clone());

    match client.register(&jwt, &mid, &machine_name).await {
        Ok(_) => {
            let status = client.status().await;
            tracing::info!(
                machine_name = %machine_name,
                owner_sub = ?status.owner_sub,
                "registered with studio"
            );
            let auth = AuthProvider::Studio {
                local_token,
                ledger,
            };
            client.start_heartbeat(jwt.clone(), mid.clone(), cancel);
            (auth, Some(client), Some((jwt, mid)))
        }
        Err(error) => {
            tracing::error!(error = %error, "studio registration failed");
            std::process::exit(1);
        }
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
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".lf")
}

pub fn default_db_path() -> PathBuf {
    lf_home_dir().join("lfd.db")
}

pub fn default_output_dir() -> PathBuf {
    lf_home_dir().join("output")
}

pub fn default_max_slots() -> usize {
    std::thread::available_parallelism()
        .map(|count| std::cmp::max(1, count.get() / 2))
        .unwrap_or(1)
}
