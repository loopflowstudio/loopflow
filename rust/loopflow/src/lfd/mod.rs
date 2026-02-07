pub mod auth;
pub mod config;
pub mod credentials;
pub mod events;
pub mod executor;
pub mod http;
pub mod id;
pub mod loops;
pub mod machine_id;
pub mod obs;
pub mod output;
pub mod registration;
pub mod scheduler;
pub mod sessions;
pub mod store;
pub mod types;

use std::path::PathBuf;

use tokio_util::sync::CancellationToken;

use self::auth::AuthContext;
use self::config::LfdConfig;
use self::registration::{ConnectionValidator, RegistrationClient};

/// Set up registration with auth.loopflow.studio.
///
/// Requires a JWT in `~/.lf/credentials.json`. If the JWT is missing or
/// registration fails, lfd exits — you can't serve on a public address
/// without auth.
pub async fn setup_registration(
    config: &LfdConfig,
    cancel: CancellationToken,
) -> (
    Option<RegistrationClient>,
    AuthContext,
    Option<(String, String)>,
) {
    let Some(jwt) = self::credentials::load_jwt() else {
        tracing::error!(
            "non-loopback bind address requires auth — \
             add your JWT to ~/.lf/credentials.json"
        );
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
            let auth = AuthContext::new(true, validator);
            client.start_heartbeat(jwt.clone(), mid.clone(), cancel);
            (Some(client), auth, Some((jwt, mid)))
        }
        Err(e) => {
            tracing::error!(error = %e, "registration with loopflow.studio failed");
            std::process::exit(1);
        }
    }
}

pub fn default_db_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".lf").join("lfd.db")
}

pub fn default_max_slots() -> usize {
    std::thread::available_parallelism()
        .map(|count| std::cmp::max(1, count.get() / 2))
        .unwrap_or(1)
}
