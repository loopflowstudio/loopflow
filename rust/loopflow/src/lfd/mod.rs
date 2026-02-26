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
pub mod provider_auth;
pub mod queue;
pub mod redaction;
pub mod registration;
pub mod scheduler;
pub mod security;
pub mod service;
pub mod session_token;
pub mod sessions;
pub mod store;
pub mod triggers;
pub mod types;

use std::path::PathBuf;

use tokio_util::sync::CancellationToken;

use self::auth::AuthProvider;
use self::config::LfdConfig;
use self::registration::{ConnectionValidator, RegistrationClient};

/// Set up auth provider based on config.
///
/// Returns the auth provider, an optional registration client (for Studio
/// provider status/deregistration), and optional credentials for deregistration.
pub async fn setup_auth(
    config: &LfdConfig,
    cancel: CancellationToken,
) -> (
    AuthProvider,
    Option<RegistrationClient>,
    Option<(String, String)>,
) {
    match config.auth.provider.as_str() {
        "local" => (setup_local_auth(), None, None),
        "static" => {
            let token = config
                .auth
                .token
                .as_ref()
                .unwrap_or_else(|| {
                    tracing::error!(
                        "auth.provider=static requires auth.token in config or LFD_AUTH_TOKEN env"
                    );
                    std::process::exit(1);
                })
                .clone();
            tracing::info!("static token auth configured");
            (AuthProvider::Static { token }, None, None)
        }
        "loopflow.studio" => setup_studio_registration(config, cancel).await,
        other => {
            tracing::error!(provider = other, "unknown auth provider");
            std::process::exit(1);
        }
    }
}

pub fn setup_local_auth() -> AuthProvider {
    let session_token = match self::session_token::generate_and_write() {
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
    };

    AuthProvider::Local { session_token }
}

async fn setup_studio_registration(
    config: &LfdConfig,
    cancel: CancellationToken,
) -> (
    AuthProvider,
    Option<RegistrationClient>,
    Option<(String, String)>,
) {
    let Some(jwt) = self::credentials::load_jwt() else {
        tracing::error!("auth.provider=loopflow.studio requires a JWT in ~/.lf/credentials.json");
        std::process::exit(1);
    };

    let mid = self::machine_id::machine_id();
    let machine_name = self::machine_id::machine_name();
    let base_url = &config.auth.base_url;

    let client = RegistrationClient::new(base_url);
    let validator = ConnectionValidator::new(base_url);

    match client.register(&jwt, &mid, &machine_name).await {
        Ok(_token) => {
            tracing::info!(machine_name = %machine_name, "registered with loopflow.studio");
            let auth = AuthProvider::Studio { validator };
            client.start_heartbeat(jwt.clone(), mid.clone(), cancel);
            (auth, Some(client), Some((jwt, mid)))
        }
        Err(e) => {
            tracing::error!(error = %e, "registration with loopflow.studio failed");
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
