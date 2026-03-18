use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use ipnet::IpNet;
use secrecy::ExposeSecret;
use tokio::signal;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use loopflow::lfd::auth::AuthProvider;
use loopflow::lfd::config::{AuthMode, LfdConfig, StorageType};
use loopflow::lfd::events::EventHub;
use loopflow::lfd::executor::WaveExecutor;
use loopflow::lfd::http::HttpState;
use loopflow::lfd::output::OutputHub;
use loopflow::lfd::provider_auth::ProviderAuthService;
use loopflow::lfd::scheduler::Scheduler;
use loopflow::lfd::security::path_within_root_planned;
use loopflow::lfd::sessions::SessionManager;
use loopflow::lfd::store::{migrate_store, open_store, SharedStore, StorageConfig};

fn env_flag(name: &str) -> bool {
    std::env::var(name)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    loopflow::lfd::obs::init_tracing();

    let args: Vec<String> = std::env::args().collect();
    if let Some(command) = args.get(1).map(String::as_str) {
        let command_args = &args[2..];
        let force = has_flag(command_args, "--force");
        let no_interactive = has_flag(command_args, "--no-interactive");
        match command {
            "migrate" => {
                let status_only = args[2..].iter().any(|arg| arg == "--status");
                let lfd_config = LfdConfig::load().expect("failed to load lfd config");
                let storage_config = storage_config_from_config(&lfd_config)?;
                let version = migrate_store(&storage_config, status_only).await?;
                if status_only {
                    println!("schema_version={version}");
                } else {
                    println!("migrated schema to version {version}");
                }
                return Ok(());
            }
            "install" => return loopflow::lfd::service::install(force, no_interactive),
            "uninstall" => return loopflow::lfd::service::uninstall(),
            "start" => return loopflow::lfd::service::start(force),
            "stop" => return loopflow::lfd::service::stop(),
            "status" => return loopflow::lfd::service::status(),
            _ => {} // fall through to serve
        }
    }

    let lfd_config = LfdConfig::load().expect("failed to load lfd config");
    let allow_insecure_bind = has_flag(&args[1..], "--allow-insecure-bind");

    let http_addr: SocketAddr = std::env::var("LFD_HTTP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:2486".to_string())
        .parse()?;
    let storage_config = storage_config_from_config(&lfd_config)?;

    let max_slots = std::env::var("LFD_MAX_SLOTS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(loopflow::lfd::default_max_slots);

    let cancel = CancellationToken::new();

    if requires_secure_bind(lfd_config.auth.mode)
        && !allow_insecure_bind
        && !http_addr.ip().is_loopback()
        && !is_tailscale_ip(http_addr.ip())
    {
        return Err(format!(
            "refusing insecure non-loopback bind for auth.mode={}: {}. \
             use --allow-insecure-bind to override",
            lfd_config.auth.mode, http_addr
        )
        .into());
    }

    if matches!(&storage_config, StorageConfig::Postgres { .. }) {
        let version = migrate_store(&storage_config, false).await?;
        tracing::info!(schema_version = %version, "postgres schema up to date");
    }

    let store: SharedStore = Arc::new(open_store(&storage_config).await?);

    let is_loopback = http_addr.ip().is_loopback();
    let (auth_provider, registration_client, registration_creds) = loopflow::lfd::setup_auth(
        &lfd_config,
        store.clone(),
        &storage_config,
        http_addr,
        cancel.clone(),
    )
    .await;
    if !is_loopback && matches!(auth_provider, AuthProvider::Local { .. }) {
        tracing::warn!(
            addr = %http_addr,
            "binding to non-loopback address with auth.mode=local; \
             remote requests require the session token"
        );
    }

    if lfd_config.github.webhook_secret.trim().is_empty() {
        tracing::warn!(
            "GitHub webhook secret is not configured — webhook endpoint will reject all requests. \
             Set LFD_GITHUB_WEBHOOK_SECRET or github.webhook_secret in config."
        );
    }

    let scheduler = Arc::new(Scheduler::new(max_slots));
    let output_dir = loopflow::lfd::default_output_dir();
    let max_age =
        std::time::Duration::from_secs(u64::from(lfd_config.output_log_retention_days) * 86400);
    loopflow::lfd::output::prune_output_logs(&output_dir, max_age);
    let output = OutputHub::new(2048, output_dir.clone());
    let event_hub = EventHub::new(1024);
    let session_manager = SessionManager::new_with_scheduler(store.clone(), scheduler.clone());
    let ci_failure_cache = Arc::new(Mutex::new(std::collections::HashSet::new()));
    let executor = WaveExecutor::new(
        store.clone(),
        scheduler.clone(),
        output.clone(),
        event_hub.clone(),
        lfd_config.executor.clone(),
        lfd_config.github.clone(),
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
            if recovery.orphaned_fork_worktrees_removed > 0 {
                tracing::info!(
                    count = recovery.orphaned_fork_worktrees_removed,
                    "removed orphaned fork worktrees on startup"
                );
            }
            if recovery.orphaned_fork_runs_cleaned > 0 {
                tracing::info!(
                    count = recovery.orphaned_fork_runs_cleaned,
                    "cleaned orphaned fork run records on startup"
                );
            }
        }
        Err(err) => tracing::warn!(error = %err, "startup recovery failed"),
    }

    match session_manager.recover_orphaned_sessions().await {
        Ok(recovery) => {
            if recovery.sessions_failed > 0 {
                tracing::info!(
                    count = recovery.sessions_failed,
                    "recovered orphaned sessions from previous lfd"
                );
            }
            if recovery.opencode_servers_reaped > 0 {
                tracing::info!(
                    count = recovery.opencode_servers_reaped,
                    "reaped orphaned OpenCode servers from previous lfd"
                );
            }
            if recovery.reap_errors > 0 {
                tracing::warn!(
                    count = recovery.reap_errors,
                    "encountered errors while reaping orphaned OpenCode servers"
                );
            }
        }
        Err(err) => tracing::warn!(error = %err, "session orphan recovery failed"),
    }

    let loop_handles = scheduler.clone().start_loops(
        store.clone(),
        executor.clone(),
        event_hub.clone(),
        lfd_config.github.clone(),
        cancel.clone(),
    );

    // Hourly output log pruning.
    {
        let prune_dir = output_dir;
        let prune_cancel = cancel.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_secs(3600));
            interval.tick().await; // skip immediate tick (startup prune already ran)
            loop {
                tokio::select! {
                    _ = prune_cancel.cancelled() => break,
                    _ = interval.tick() => {
                        loopflow::lfd::output::prune_output_logs(&prune_dir, max_age);
                    }
                }
            }
        });
    }

    if env_flag("LFD_DISABLE_WORKTREE_JANITOR") {
        tracing::info!("startup worktree janitor disabled by LFD_DISABLE_WORKTREE_JANITOR");
    } else {
        let repo_roots = store
            .list_waves(None)
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|wave| PathBuf::from(wave.repo()))
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
    }

    if let Some(token) = lfd_config.github.token.clone() {
        if let Err(err) = loopflow::lfd::http::routes::hooks::poll_all_waves_ci(
            &store,
            &event_hub,
            token.expose_secret(),
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
        provider_auth: ProviderAuthService::new(store.clone()),
        auth: auth_provider,
        registration: registration_client.clone(),
        started_at: time::OffsetDateTime::now_utc(),
        github: lfd_config.github,
        http_security: lfd_config.http_security,
        auth_failure_throttle: loopflow::lfd::auth::AuthFailureThrottle::new(),
        ci_failure_cache,
        sessions: session_manager,
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

fn storage_config_from_config(
    config: &LfdConfig,
) -> Result<StorageConfig, Box<dyn std::error::Error>> {
    match config.storage {
        StorageType::Sqlite => {
            let db_root = loopflow::lfd::default_db_path()
                .parent()
                .map(std::path::Path::to_path_buf)
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
        StorageType::Postgres => {
            let database_url = std::env::var("LFD_DATABASE_URL").map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    "LFD_DATABASE_URL required for postgres storage",
                )
            })?;
            Ok(StorageConfig::postgres(database_url))
        }
    }
}

fn requires_secure_bind(mode: AuthMode) -> bool {
    matches!(mode, AuthMode::Studio)
}

fn is_tailscale_ip(ip: std::net::IpAddr) -> bool {
    let cidr: IpNet = "100.64.0.0/10".parse().expect("valid tailscale cidr");
    cidr.contains(&ip)
}

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

#[cfg(test)]
mod tests {
    use super::{
        has_flag, is_tailscale_ip, requires_secure_bind, storage_config_from_config, AuthMode,
        LfdConfig, StorageConfig, StorageType,
    };
    use std::ffi::OsString;
    use std::net::IpAddr;
    use std::sync::{Mutex, OnceLock};
    use tempfile::{tempdir, TempDir};

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

    fn setup_sqlite_env() -> TempDir {
        let home = tempdir().expect("tempdir");
        std::env::set_var("HOME", home.path());
        std::env::remove_var("LFD_DB_PATH");
        std::env::remove_var("LFD_DATABASE_URL");
        home
    }

    #[test]
    fn storage_config_defaults_to_sqlite() {
        let _lock = env_lock().lock().expect("env lock poisoned");
        let _guard = EnvGuard::snapshot(&["HOME", "LFD_DB_PATH", "LFD_DATABASE_URL"]);
        let home = setup_sqlite_env();

        let config =
            storage_config_from_config(&LfdConfig::default()).expect("sqlite default should parse");
        match config {
            StorageConfig::Sqlite { path, .. } => {
                let expected_root = home
                    .path()
                    .join(".lf")
                    .canonicalize()
                    .expect("canonical root");
                assert_eq!(path, expected_root.join("lfd.db"))
            }
            StorageConfig::Postgres { .. } => panic!("expected sqlite storage"),
        }
    }

    #[test]
    fn secure_bind_required_for_studio() {
        assert!(requires_secure_bind(AuthMode::Studio));
        assert!(!requires_secure_bind(AuthMode::Local));
    }

    #[test]
    fn tailscale_range_is_detected() {
        assert!(is_tailscale_ip(IpAddr::from([100, 64, 10, 3])));
        assert!(is_tailscale_ip(IpAddr::from([100, 127, 255, 254])));
        assert!(!is_tailscale_ip(IpAddr::from([100, 128, 0, 1])));
        assert!(!is_tailscale_ip(IpAddr::from([192, 168, 1, 10])));
    }

    #[test]
    fn storage_config_honors_relative_db_path_override_for_sqlite() {
        let _lock = env_lock().lock().expect("env lock poisoned");
        let _guard = EnvGuard::snapshot(&["HOME", "LFD_DB_PATH", "LFD_DATABASE_URL"]);
        let home = setup_sqlite_env();
        std::fs::create_dir_all(home.path().join(".lf").join("db")).expect("create db dir");
        std::env::set_var("LFD_DB_PATH", "db/custom.db");

        let config =
            storage_config_from_config(&LfdConfig::default()).expect("sqlite config should parse");
        match config {
            StorageConfig::Sqlite { path, .. } => {
                let expected_root = home
                    .path()
                    .join(".lf")
                    .canonicalize()
                    .expect("canonical root");
                assert_eq!(path, expected_root.join("db").join("custom.db"))
            }
            StorageConfig::Postgres { .. } => panic!("expected sqlite storage"),
        }
    }

    #[test]
    fn storage_config_honors_absolute_db_path_override_for_sqlite() {
        let _lock = env_lock().lock().expect("env lock poisoned");
        let _guard = EnvGuard::snapshot(&["HOME", "LFD_DB_PATH", "LFD_DATABASE_URL"]);
        let _home = setup_sqlite_env();
        let absolute = tempdir()
            .expect("tempdir")
            .path()
            .join("custom")
            .join("lfd.db");
        std::env::set_var("LFD_DB_PATH", &absolute);

        let config =
            storage_config_from_config(&LfdConfig::default()).expect("sqlite config should parse");
        match config {
            StorageConfig::Sqlite { path, .. } => {
                assert_eq!(path, absolute);
            }
            StorageConfig::Postgres { .. } => panic!("expected sqlite storage"),
        }
    }

    #[test]
    fn storage_config_requires_database_url_for_postgres() {
        let _lock = env_lock().lock().expect("env lock poisoned");
        let _guard = EnvGuard::snapshot(&["HOME", "LFD_DB_PATH", "LFD_DATABASE_URL"]);
        std::env::remove_var("LFD_DB_PATH");
        std::env::remove_var("LFD_DATABASE_URL");

        let config = LfdConfig {
            storage: StorageType::Postgres,
            ..LfdConfig::default()
        };
        let err =
            storage_config_from_config(&config).expect_err("postgres should require database url");
        assert_eq!(
            err.to_string(),
            "LFD_DATABASE_URL required for postgres storage"
        );
    }

    #[test]
    fn has_flag_matches_exact_flag() {
        let args = vec![
            "--force".to_string(),
            "--no-interactive".to_string(),
            "value".to_string(),
        ];
        assert!(has_flag(&args, "--force"));
        assert!(has_flag(&args, "--no-interactive"));
        assert!(!has_flag(&args, "--missing"));
    }
}
