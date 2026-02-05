use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::signal;
use tokio_util::sync::CancellationToken;
use tonic::transport::Server;

mod lfd;

use lfd::executor::WaveExecutor;
use lfd::http::HttpState;
use lfd::output::OutputHub;
use lfd::proto::control::control_service_server::ControlServiceServer;
use lfd::scheduler::Scheduler;
use lfd::server::ControlServer;
use lfd::store::postgres::PostgresStore;
use lfd::store::sqlite::SqliteStore;
use lfd::store::SharedStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    lfd::obs::init_tracing();

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

    let grpc_addr: SocketAddr = std::env::var("LFD_GRPC_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:50051".to_string())
        .parse()?;
    let http_addr: SocketAddr = std::env::var("LFD_HTTP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_string())
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
    let executor = WaveExecutor::new(store.clone(), scheduler.clone(), output.clone());
    let cancel = CancellationToken::new();
    let loop_handles =
        scheduler
            .clone()
            .start_loops(store.clone(), executor.clone(), cancel.clone());

    let grpc_server = ControlServer::new(store.clone(), scheduler.clone(), executor, output);

    let http_state = HttpState {
        store: store.clone(),
        scheduler: scheduler.clone(),
        started_at: time::OffsetDateTime::now_utc(),
    };
    let http_router = lfd::http::router(http_state);

    let grpc_task = tokio::spawn(async move {
        Server::builder()
            .add_service(ControlServiceServer::new(grpc_server))
            .serve(grpc_addr)
            .await
    });

    let http_task = tokio::spawn(async move {
        let listener = tokio::net::TcpListener::bind(http_addr).await?;
        axum::serve(listener, http_router).await
    });

    tokio::select! {
        result = grpc_task => {
            result??;
        }
        result = http_task => {
            result??;
        }
        _ = signal::ctrl_c() => {
            tracing::info!("shutdown signal received");
        }
    }

    cancel.cancel();
    for handle in loop_handles {
        let _ = handle.await;
    }

    Ok(())
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
