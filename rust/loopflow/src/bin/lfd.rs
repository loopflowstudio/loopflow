use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::signal;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use loopflow::lfd::auth::AuthProvider;
use loopflow::lfd::config::LfdConfig;
use loopflow::lfd::events::EventHub;
use loopflow::lfd::executor::WaveExecutor;
use loopflow::lfd::http::HttpState;
use loopflow::lfd::output::OutputHub;
use loopflow::lfd::scheduler::Scheduler;
use loopflow::lfd::store::{migrate_store, open_store, SharedStore, StorageConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    loopflow::lfd::obs::init_tracing();

    let mut args = std::env::args();
    if let Some(command) = args.nth(1) {
        match command.as_str() {
            "migrate" => {
                let status_only = args.any(|arg| arg == "--status");
                let storage_config = storage_config_from_env()?;
                let version = migrate_store(&storage_config, status_only).await?;
                if status_only {
                    println!("schema_version={version}");
                } else {
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
    let storage_config = storage_config_from_env()?;

    let max_slots = std::env::var("LFD_MAX_SLOTS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(loopflow::lfd::default_max_slots);

    // Load config and set up auth.
    let lfd_config = LfdConfig::load().expect("failed to load lfd config");
    let cancel = CancellationToken::new();

    let is_loopback = http_addr.ip().is_loopback();
    let (auth_provider, registration_client, registration_creds) = if is_loopback {
        // Loopback bind — local provider, no registration needed.
        (AuthProvider::Local, None, None)
    } else {
        let (provider, client, creds) =
            loopflow::lfd::setup_auth(&lfd_config, cancel.clone()).await;
        if matches!(provider, AuthProvider::Local) {
            tracing::warn!(
                addr = %http_addr,
                "binding to non-loopback address with auth.provider=local; \
                 remote connections will be rejected"
            );
        }
        (provider, client, creds)
    };

    if matches!(&storage_config, StorageConfig::Postgres { .. }) {
        let version = migrate_store(&storage_config, false).await?;
        tracing::info!(schema_version = %version, "postgres schema up to date");
    }

    let store: SharedStore = open_store(&storage_config).await?.into_shared();

    let scheduler = Arc::new(Scheduler::new(max_slots));
    let output = OutputHub::new(2048, loopflow::lfd::default_output_dir());
    let event_hub = EventHub::new(1024);
    let ci_failure_cache = Arc::new(Mutex::new(std::collections::HashSet::new()));
    let executor = WaveExecutor::new(
        store.clone(),
        scheduler.clone(),
        output.clone(),
        event_hub.clone(),
        lfd_config.executor.clone(),
    )?;

    match executor.recover_startup().await {
        Ok(recovery) => {
            if recovery.orphaned_runs_failed > 0 {
                tracing::info!(
                    count = recovery.orphaned_runs_failed,
                    "cleaned up orphaned runs from previous lfd"
                );
            }
            if recovery.rehydrated_agents > 0 {
                tracing::info!(
                    count = recovery.rehydrated_agents,
                    "reattached running docker agents after restart"
                );
            }
            if recovery.lost_agents_failed > 0 {
                tracing::warn!(
                    count = recovery.lost_agents_failed,
                    "marked running agents failed because containers were missing"
                );
            }
            if recovery.orphaned_containers_removed > 0 {
                tracing::info!(
                    count = recovery.orphaned_containers_removed,
                    "removed orphaned managed docker containers on startup"
                );
            }
        }
        Err(err) => tracing::warn!(error = %err, "startup recovery failed"),
    }

    let loop_handles = scheduler.clone().start_loops(
        store.clone(),
        executor.clone(),
        event_hub.clone(),
        cancel.clone(),
    );

    let repo_roots = store
        .list_waves(None)
        .unwrap_or_default()
        .into_iter()
        .map(|wave| PathBuf::from(wave.repo))
        .collect::<Vec<_>>();
    match executor.run_worktree_janitor(&repo_roots).await {
        Ok(report) => {
            if report.removed > 0 || report.errors > 0 {
                tracing::info!(
                    removed = report.removed,
                    active = report.active,
                    errors = report.errors,
                    "startup worktree janitor finished"
                );
            }
        }
        Err(err) => tracing::warn!(error = %err, "startup worktree janitor failed"),
    }

    if let Some(token) = lfd_config.github.token.clone() {
        if let Err(err) = loopflow::lfd::http::routes::hooks::poll_all_waves_ci(
            &store,
            &event_hub,
            &token,
            &ci_failure_cache,
        )
        .await
        {
            tracing::warn!(error = %err, "startup CI poll failed");
        }
    } else {
        tracing::warn!("LFD_GITHUB_TOKEN not set; skipping startup CI poll");
    }

    let http_state = HttpState {
        store: store.clone(),
        scheduler: scheduler.clone(),
        executor: Arc::new(executor),
        event_hub,
        output_hub: output,
        auth: auth_provider,
        registration: registration_client.clone(),
        started_at: time::OffsetDateTime::now_utc(),
        github: lfd_config.github,
        ci_failure_cache,
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

fn storage_config_from_env() -> Result<StorageConfig, Box<dyn std::error::Error>> {
    let storage = std::env::var("LFD_STORAGE").unwrap_or_else(|_| "sqlite".to_string());
    if storage.eq_ignore_ascii_case("postgres") {
        let database_url = std::env::var("LFD_DATABASE_URL").map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "LFD_DATABASE_URL required for postgres storage",
            )
        })?;
        return Ok(StorageConfig::postgres(database_url));
    }

    if storage.eq_ignore_ascii_case("sqlite") {
        let db_path = std::env::var("LFD_DB_PATH")
            .map(PathBuf::from)
            .unwrap_or_else(|_| loopflow::lfd::default_db_path());
        return Ok(StorageConfig::sqlite(db_path));
    }

    Err(std::io::Error::new(
        std::io::ErrorKind::InvalidInput,
        format!("invalid LFD_STORAGE value `{storage}`; expected `sqlite` or `postgres`"),
    )
    .into())
}

#[cfg(test)]
mod tests {
    use super::{storage_config_from_env, StorageConfig};
    use std::ffi::OsString;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct EnvGuard {
        vars: Vec<(&'static str, Option<OsString>)>,
    }

    impl EnvGuard {
        fn snapshot(vars: &[&'static str]) -> Self {
            Self {
                vars: vars
                    .iter()
                    .map(|name| (*name, std::env::var_os(name)))
                    .collect(),
            }
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (name, value) in &self.vars {
                if let Some(value) = value {
                    std::env::set_var(name, value);
                } else {
                    std::env::remove_var(name);
                }
            }
        }
    }

    #[test]
    fn storage_config_defaults_to_sqlite() {
        let _lock = env_lock().lock().expect("env lock poisoned");
        let _guard = EnvGuard::snapshot(&["LFD_STORAGE", "LFD_DB_PATH", "LFD_DATABASE_URL"]);
        std::env::remove_var("LFD_STORAGE");
        std::env::remove_var("LFD_DB_PATH");
        std::env::remove_var("LFD_DATABASE_URL");

        let config = storage_config_from_env().expect("sqlite default should parse");
        assert!(matches!(config, StorageConfig::Sqlite { .. }));
    }

    #[test]
    fn storage_config_rejects_unknown_storage() {
        let _lock = env_lock().lock().expect("env lock poisoned");
        let _guard = EnvGuard::snapshot(&["LFD_STORAGE", "LFD_DB_PATH", "LFD_DATABASE_URL"]);
        std::env::set_var("LFD_STORAGE", "mysql");
        std::env::remove_var("LFD_DB_PATH");
        std::env::remove_var("LFD_DATABASE_URL");

        let err = storage_config_from_env().expect_err("unknown storage should error");
        assert_eq!(
            err.to_string(),
            "invalid LFD_STORAGE value `mysql`; expected `sqlite` or `postgres`"
        );
    }

    #[test]
    fn storage_config_requires_database_url_for_postgres() {
        let _lock = env_lock().lock().expect("env lock poisoned");
        let _guard = EnvGuard::snapshot(&["LFD_STORAGE", "LFD_DB_PATH", "LFD_DATABASE_URL"]);
        std::env::set_var("LFD_STORAGE", "postgres");
        std::env::remove_var("LFD_DB_PATH");
        std::env::remove_var("LFD_DATABASE_URL");

        let err = storage_config_from_env().expect_err("postgres should require database url");
        assert_eq!(
            err.to_string(),
            "LFD_DATABASE_URL required for postgres storage"
        );
    }
}
