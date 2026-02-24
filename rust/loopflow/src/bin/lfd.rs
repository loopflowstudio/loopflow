use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::signal;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use loopflow::lfd::auth::AuthProvider;
use loopflow::lfd::config::{LfdConfig, StorageType};
use loopflow::lfd::events::EventHub;
use loopflow::lfd::executor::WaveExecutor;
use loopflow::lfd::http::HttpState;
use loopflow::lfd::output::OutputHub;
use loopflow::lfd::scheduler::Scheduler;
use loopflow::lfd::secret_string::SecretString;
use loopflow::lfd::security::path_within_root_planned;
use loopflow::lfd::sessions::SessionManager;
use loopflow::lfd::store::{migrate_store, open_store, SharedStore, StorageConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    loopflow::lfd::obs::init_tracing();

    let args: Vec<String> = std::env::args().collect();
    if let Some(command) = args.get(1).map(String::as_str) {
        let force = args[2..].iter().any(|arg| arg == "--force");
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
            "install" => return loopflow::lfd::service::install(force),
            "uninstall" => return loopflow::lfd::service::uninstall(),
            "start" => return loopflow::lfd::service::start(force),
            "stop" => return loopflow::lfd::service::stop(),
            "status" => return loopflow::lfd::service::status(),
            "token" => {
                if matches!(args.get(2).map(String::as_str), Some("rotate")) {
                    let token = rotate_static_auth_token();
                    println!("{}", format_token_rotation_output(token.as_str()));
                    return Ok(());
                }
                return Err("unknown token subcommand; expected `lfd token rotate`"
                    .to_string()
                    .into());
            }
            _ => {} // fall through to serve
        }
    }

    let lfd_config = LfdConfig::load().expect("failed to load lfd config");

    let http_addr: SocketAddr = std::env::var("LFD_HTTP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:2486".to_string())
        .parse()?;
    let storage_config = storage_config_from_config(&lfd_config)?;

    let max_slots = std::env::var("LFD_MAX_SLOTS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or_else(loopflow::lfd::default_max_slots);

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
                 remote requests require the session token"
            );
        }
        (provider, client, creds)
    };

    let session_token = if matches!(&auth_provider, AuthProvider::Local) {
        Some(generate_session_token_or_exit())
    } else {
        None
    };

    if matches!(&storage_config, StorageConfig::Postgres { .. }) {
        let version = migrate_store(&storage_config, false).await?;
        tracing::info!(schema_version = %version, "postgres schema up to date");
    }

    let store: SharedStore = Arc::new(open_store(&storage_config).await?);

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

    let loop_handles = scheduler.clone().start_loops(
        store.clone(),
        executor.clone(),
        event_hub.clone(),
        lfd_config.github.clone(),
        cancel.clone(),
    );

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
        session_token,
        registration: registration_client.clone(),
        started_at: time::OffsetDateTime::now_utc(),
        github: lfd_config.github,
        http_security: lfd_config.http_security,
        auth_failure_throttle: loopflow::lfd::auth::AuthFailureThrottle::new(),
        ci_failure_cache,
        sessions: SessionManager::new(store),
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
            let db_path = path_within_root_planned(&db_root, &db_candidate).map_err(|err| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidInput,
                    format!("invalid LFD_DB_PATH: {err}"),
                )
            })?;
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

fn generate_session_token_or_exit() -> SecretString {
    match loopflow::lfd::session_token::generate_and_write() {
        Ok(token) => {
            tracing::info!(
                path = %loopflow::lfd::session_token::token_path().display(),
                "session token written"
            );
            SecretString::from(token)
        }
        Err(err) => {
            tracing::error!(error = %err, "failed to write session token");
            std::process::exit(1);
        }
    }
}

fn rotate_static_auth_token() -> String {
    loopflow::lfd::session_token::generate()
}

fn format_token_rotation_output(token: &str) -> String {
    format!(
        "new static auth token (shown once):\n{token}\n\nnext steps:\n1. update LFD_AUTH_TOKEN in your secret source (.env, secret manager, systemd/launchd env)\n2. restart lfd\n3. verify old token is rejected and new token is accepted",
    )
}

#[cfg(test)]
mod tests {
    use super::{
        format_token_rotation_output, rotate_static_auth_token, storage_config_from_config,
        LfdConfig, StorageConfig, StorageType,
    };
    use std::ffi::OsString;
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
    fn storage_config_rejects_absolute_db_path_override_for_sqlite() {
        let _lock = env_lock().lock().expect("env lock poisoned");
        let _guard = EnvGuard::snapshot(&["HOME", "LFD_DB_PATH", "LFD_DATABASE_URL"]);
        let _home = setup_sqlite_env();
        std::env::set_var("LFD_DB_PATH", "/tmp/custom.db");

        let err = storage_config_from_config(&LfdConfig::default())
            .expect_err("absolute sqlite path should fail");
        assert!(err.to_string().contains("invalid LFD_DB_PATH"));
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
    fn rotate_static_auth_token_returns_hex_32_byte_token() {
        let token = rotate_static_auth_token();
        assert_eq!(token.len(), 64);
        assert!(token.chars().all(|ch| ch.is_ascii_hexdigit()));
    }

    #[test]
    fn format_token_rotation_output_prints_token_once_with_runbook_steps() {
        let output = format_token_rotation_output("abc123");
        assert_eq!(output.matches("abc123").count(), 1);
        assert!(output.contains("update LFD_AUTH_TOKEN"));
        assert!(output.contains("restart lfd"));
        assert!(output.contains("verify old token is rejected and new token is accepted"));
    }
}
