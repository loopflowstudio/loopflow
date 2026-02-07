use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::signal;
use tokio_util::sync::CancellationToken;

mod auth;
mod config;
mod credentials;
mod events;
mod executor;
mod http;
mod id;
mod loops;
mod machine_id;
mod obs;
mod output;
mod registration;
mod scheduler;
mod sessions;
mod store;
mod types;

use crate::auth::AuthContext;
use crate::config::LfdConfig;
use crate::events::EventHub;
use crate::registration::{ConnectionValidator, RegistrationClient};
use executor::WaveExecutor;
use http::HttpState;
use output::OutputHub;
use scheduler::Scheduler;
use store::postgres::PostgresStore;
use store::sqlite::SqliteStore;
use store::SharedStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    obs::init_tracing();

    let mut args = std::env::args();
    if let Some(command) = args.nth(1) {
        if command == "migrate" {
            let status_only = args.any(|arg| arg == "--status");
            let database_url = std::env::var("LFD_DATABASE_URL")
                .expect("LFD_DATABASE_URL required for postgres migrations");
            if status_only {
                let version = PostgresStore::migrate_status_async(&database_url).await?;
                println!("schema_version={version}");
            } else {
                let version = PostgresStore::migrate_async(&database_url).await?;
                println!("migrated schema to version {version}");
            }
            return Ok(());
        }
    }

    let http_addr: SocketAddr = std::env::var("LFD_HTTP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:2486".to_string())
        .parse()?;
    let db_path = std::env::var("LFD_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_db_path());
    let storage = std::env::var("LFD_STORAGE").unwrap_or_else(|_| "sqlite".to_string());

    let max_slots = std::env::var("LFD_MAX_SLOTS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(default_max_slots);

    // Load config and set up auth.
    let lfd_config = LfdConfig::load();
    let requires_auth = !http_addr.ip().is_loopback();
    let cancel = CancellationToken::new();

    let (registration_client, auth_context, registration_creds) = if requires_auth {
        setup_registration(&lfd_config, cancel.clone()).await
    } else {
        (None, AuthContext::inactive(), None)
    };

    let store = match storage.as_str() {
        "postgres" => {
            let database_url = std::env::var("LFD_DATABASE_URL")
                .expect("LFD_DATABASE_URL required for postgres storage");
            Arc::new(PostgresStore::connect_async(&database_url).await?) as SharedStore
        }
        _ => Arc::new(SqliteStore::new(&db_path)?) as SharedStore,
    };
    let scheduler = Arc::new(Scheduler::new(max_slots));
    let output = OutputHub::new(2048);
    let event_hub = EventHub::new(1024);
    let executor = WaveExecutor::new(store.clone(), scheduler.clone(), output.clone());
    let loop_handles =
        scheduler
            .clone()
            .start_loops(store.clone(), executor.clone(), cancel.clone());

    let http_state = HttpState {
        store: store.clone(),
        scheduler: scheduler.clone(),
        executor: Arc::new(executor),
        event_hub,
        output_hub: output,
        auth: auth_context,
        registration: registration_client.clone(),
        started_at: time::OffsetDateTime::now_utc(),
    };
    let http_router = http::router(http_state);

    let http_task = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(http_addr).await?;
        tracing::info!(addr = %http_addr, "listening");
        axum::serve(
            listener,
            http_router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
    });

    tokio::select! {
        result = http_task => {
            result??;
        }
        _ = signal::ctrl_c() => {
            tracing::info!("shutdown signal received");
        }
    }

    // Deregister on shutdown.
    if let (Some(client), Some((jwt, mid))) = (&registration_client, &registration_creds) {
        tracing::info!("deregistering from loopflow.studio");
        client.deregister(jwt, mid).await;
    }

    cancel.cancel();
    for handle in loop_handles {
        let _ = handle.await;
    }

    Ok(())
}

/// Set up registration with auth.loopflow.studio.
///
/// Requires a JWT in `~/.lf/credentials.json`. If the JWT is missing or
/// registration fails, lfd exits — you can't serve on a public address
/// without auth.
async fn setup_registration(
    config: &LfdConfig,
    cancel: CancellationToken,
) -> (
    Option<RegistrationClient>,
    AuthContext,
    Option<(String, String)>,
) {
    let Some(jwt) = credentials::load_jwt() else {
        tracing::error!(
            "non-loopback bind address requires auth — \
             add your JWT to ~/.lf/credentials.json"
        );
        std::process::exit(1);
    };

    let mid = machine_id::machine_id();
    let machine_name = machine_id::machine_name();
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

fn default_db_path() -> PathBuf {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    home.join(".lf").join("lfd.db")
}

fn default_max_slots() -> usize {
    std::thread::available_parallelism()
        .map(|count| std::cmp::max(1, count.get() / 2))
        .unwrap_or(1)
}
