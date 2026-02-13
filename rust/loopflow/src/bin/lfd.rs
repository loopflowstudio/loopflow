use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::signal;
use tokio_util::sync::CancellationToken;

use loopflow::lfd::auth::AuthContext;
use loopflow::lfd::config::LfdConfig;
use loopflow::lfd::events::EventHub;
use loopflow::lfd::executor::WaveExecutor;
use loopflow::lfd::http::HttpState;
use loopflow::lfd::output::OutputHub;
use loopflow::lfd::scheduler::Scheduler;
use loopflow::lfd::store::postgres::PostgresStore;
use loopflow::lfd::store::sqlite::SqliteStore;
use loopflow::lfd::store::SharedStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    loopflow::lfd::obs::init_tracing();

    let mut args = std::env::args();
    if let Some(command) = args.nth(1) {
        match command.as_str() {
            "migrate" => {
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
            "install" => return loopflow::lfd::service::install(),
            "uninstall" => return loopflow::lfd::service::uninstall(),
            "start" => return loopflow::lfd::service::start(),
            "stop" => return loopflow::lfd::service::stop(),
            "status" => return loopflow::lfd::service::status(),
            _ => {} // fall through to serve
        }
    }

    let http_addr: SocketAddr = std::env::var("LFD_HTTP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:2486".to_string())
        .parse()?;
    let db_path = std::env::var("LFD_DB_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| loopflow::lfd::default_db_path());
    let storage = std::env::var("LFD_STORAGE").unwrap_or_else(|_| "sqlite".to_string());

    let max_slots = std::env::var("LFD_MAX_SLOTS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(loopflow::lfd::default_max_slots);

    // Load config and set up auth.
    let lfd_config = LfdConfig::load();
    let requires_auth = !http_addr.ip().is_loopback();
    let cancel = CancellationToken::new();

    let (registration_client, auth_context, registration_creds) = if requires_auth {
        loopflow::lfd::setup_registration(&lfd_config, cancel.clone()).await
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
    // Clean up orphaned runs from a previous lfd process.
    match store.fail_orphaned_runs() {
        Ok(0) => {}
        Ok(n) => tracing::info!(count = n, "cleaned up orphaned runs from previous lfd"),
        Err(err) => tracing::warn!(error = %err, "failed to clean up orphaned runs"),
    }

    let scheduler = Arc::new(Scheduler::new(max_slots));
    let output = OutputHub::new(2048, loopflow::lfd::default_output_dir());
    let event_hub = EventHub::new(1024);
    let executor = WaveExecutor::new(
        store.clone(),
        scheduler.clone(),
        output.clone(),
        event_hub.clone(),
        lfd_config.executor.clone(),
    )?;
    let loop_handles = scheduler.clone().start_loops(
        store.clone(),
        executor.clone(),
        event_hub.clone(),
        cancel.clone(),
    );

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
    let http_router = loopflow::lfd::http::router(http_state);

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
