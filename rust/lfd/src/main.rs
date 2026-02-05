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
use crate::executor::WaveExecutor;
use crate::http::HttpState;
use crate::output::OutputHub;
use crate::registration::{ConnectionValidator, RegistrationClient};
use crate::scheduler::Scheduler;
use crate::store::postgres::PostgresStore;
use crate::store::sqlite::SqliteStore;
use crate::store::SharedStore;

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
        .unwrap_or_else(|_| "0.0.0.0:2486".to_string())
        .parse()?;
    let db_path = std::env::var("LFD_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_db_path());
    let storage = std::env::var("LFD_STORAGE").unwrap_or_else(|_| "sqlite".to_string());

    let max_slots = std::env::var("LFD_MAX_SLOTS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(default_max_slots);

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
    let events = EventHub::new(2048);
    let executor = Arc::new(WaveExecutor::new(
        store.clone(),
        scheduler.clone(),
        output.clone(),
    ));
    let cancel = CancellationToken::new();
    let loop_handles =
        scheduler
            .clone()
            .start_loops(store.clone(), (*executor).clone(), cancel.clone());

    // Load config and set up registration
    let lfd_config = LfdConfig::load();
    let (registration_client, auth_context, registration_creds) =
        setup_registration(&lfd_config, cancel.clone()).await;

    let http_state = HttpState {
        store: store.clone(),
        scheduler: scheduler.clone(),
        executor: executor.clone(),
        event_hub: events.clone(),
        output_hub: output.clone(),
        auth: auth_context.clone(),
        started_at: time::OffsetDateTime::now_utc(),
        registration: registration_client.clone(),
    };
    let http_router = http::router(http_state);

    let http_task = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(http_addr).await?;
        tracing::info!(%http_addr, "lfd listening");
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

    // Deregister on shutdown
    if let (Some(client), Some((jwt, machine_id))) = (&registration_client, &registration_creds) {
        tracing::info!("deregistering from loopflow.studio");
        client.deregister(jwt, machine_id).await;
    }

    cancel.cancel();
    for handle in loop_handles {
        let _ = handle.await;
    }

    Ok(())
}

async fn setup_registration(
    config: &LfdConfig,
    cancel: CancellationToken,
) -> (
    Option<RegistrationClient>,
    AuthContext,
    Option<(String, String)>,
) {
    let is_loopflow_studio = config
        .auth
        .provider
        .as_deref()
        .map(|p| p == "loopflow.studio")
        .unwrap_or(false);

    if !is_loopflow_studio {
        tracing::debug!("registration disabled (auth.provider not set to loopflow.studio)");
        return (None, AuthContext::disabled(), None);
    }

    let Some(jwt) = credentials::load_jwt() else {
        tracing::warn!("registration enabled but no JWT found in ~/.lf/credentials.json");
        let client = RegistrationClient::new(&config.auth.base_url);
        client.set_enabled(true).await;
        return (Some(client), AuthContext::disabled(), None);
    };

    let machine_id = machine_id::get_machine_id();
    let machine_name = machine_id::get_machine_name();

    let client = RegistrationClient::new(&config.auth.base_url);
    client.set_enabled(true).await;

    let validator = ConnectionValidator::new(&config.auth.base_url);

    match client.register(&jwt, &machine_id, &machine_name).await {
        Ok(_token) => {
            tracing::info!(machine_name = %machine_name, "registered with loopflow.studio");
            let auth_context = AuthContext::new(true, true, Some(validator));
            client.start_heartbeat(jwt.clone(), machine_id.clone(), cancel);
            (Some(client), auth_context, Some((jwt, machine_id)))
        }
        Err(e) => {
            tracing::warn!(error = %e, "registration failed, continuing without remote access");
            let auth_context = AuthContext::new(true, false, Some(validator));
            (Some(client), auth_context, None)
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
