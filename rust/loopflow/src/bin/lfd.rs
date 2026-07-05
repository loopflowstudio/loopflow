use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use tokio::signal;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use loopflow::lfd::config::LfdConfig;
use loopflow::lfd::events::EventHub;
use loopflow::lfd::executor::WaveExecutor;
use loopflow::lfd::http::HttpState;
use loopflow::lfd::output::OutputHub;
use loopflow::lfd::security::path_within_root_planned;
use loopflow::lfdb::{migrate_store, open_store, SharedStore, StorageConfig};
use loopflow::provider_auth::ProviderAuthService;

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

fn maybe_spawn_parent_watch() {
    let Some(parent_pid) = std::env::var("LFD_PARENT_PID")
        .ok()
        .and_then(|value| value.parse::<i32>().ok())
    else {
        return;
    };

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let status = Command::new("/bin/kill")
                .args(["-0", &parent_pid.to_string()])
                .status();
            if status.as_ref().is_ok_and(|status| status.success()) {
                continue;
            }
            tracing::info!(parent_pid, "bundled parent exited; shutting down lfd");
            std::process::exit(0);
        }
    });
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    loopflow::lfd::obs::init_tracing();

    let args: Vec<String> = std::env::args().collect();
    if args
        .get(1)
        .is_some_and(|arg| arg == "--version" || arg == "-V")
    {
        println!("lfd {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if let Some(command) = args.get(1).map(String::as_str) {
        let command_args = &args[2..];
        let force = has_flag(command_args, "--force");
        let no_interactive = has_flag(command_args, "--no-interactive");
        match command {
            "migrate" => {
                let status_only = args[2..].iter().any(|arg| arg == "--status");
                let storage_config = storage_config_from_env()?;
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
    maybe_spawn_parent_watch();

    let http_addr: SocketAddr = std::env::var("LFD_HTTP_ADDR")
        .unwrap_or_else(|_| "127.0.0.1:2486".to_string())
        .parse()?;
    let storage_config = storage_config_from_env()?;

    let cancel = CancellationToken::new();

    if !allow_insecure_bind && !http_addr.ip().is_loopback() && lfd_config.auth.token.is_none() {
        return Err(format!(
            "refusing non-loopback bind without LFD_AUTH_TOKEN/auth.token: {http_addr}. \
             Set a bearer token from Doppler or pass --allow-insecure-bind for local-network experiments"
        )
        .into());
    }

    let store: SharedStore = Arc::new(open_store(&storage_config).await?);

    let is_loopback = http_addr.ip().is_loopback();
    let auth_provider = loopflow::lfd::setup_auth(&lfd_config);
    if !is_loopback {
        tracing::warn!(
            addr = %http_addr,
            "binding to non-loopback address; remote requests require the bearer token"
        );
    }

    if lfd_config.github.webhook_secret.trim().is_empty() {
        tracing::warn!(
            "GitHub webhook secret is not configured — webhook endpoint will reject all requests. \
             Set LFD_GITHUB_WEBHOOK_SECRET or github.webhook_secret in config."
        );
    }

    let output_dir = loopflow::lfd::default_output_dir();
    let max_age =
        std::time::Duration::from_secs(u64::from(lfd_config.output_log_retention_days) * 86400);
    loopflow::lfd::output::prune_output_logs(&output_dir, max_age);
    let output = OutputHub::new(2048, output_dir.clone());
    let event_hub = EventHub::new(1024);
    let ci_failure_cache = Arc::new(Mutex::new(std::collections::HashSet::new()));
    let executor = WaveExecutor::new(store.clone(), event_hub.clone());

    // Boot registry hygiene: runs left mid-flight by a dead daemon are
    // failed, and terminal sessions whose tmux sessions exited while lfd was
    // down are closed.
    match store.fail_orphaned_runs().await {
        Ok(count) if count > 0 => {
            tracing::info!(count, "cleaned up orphaned runs from previous lfd");
        }
        Ok(_) => {}
        Err(err) => tracing::warn!(error = %err, "orphaned run cleanup failed"),
    }

    match executor.reconcile_sessions().await {
        Ok(completed) if completed > 0 => {
            tracing::info!(
                count = completed,
                "reconciled terminal sessions whose tmux sessions exited while lfd was down"
            );
        }
        Ok(_) => {}
        Err(err) => tracing::warn!(error = %err, "terminal session reconcile failed"),
    }

    // The one surviving background loop besides the push bridge: provider
    // token refresh. Activation/watch/cron/recovery organs died in the
    // collapse — webhooks exec `lf`, and cron lives in the wave's mind.
    let token_refresh_handle = loopflow::lfd::triggers::spawn_token_refresh(
        store.clone(),
        event_hub.clone(),
        cancel.clone(),
    );
    let journal_handle =
        loopflow::lfd::journal::spawn(store.clone(), event_hub.clone(), cancel.clone());

    // The push bridge: mutations happen in other processes now (lf wave
    // registers store-direct, lf q writes runs, lf op writes attention) —
    // poll the store and relay their effects to /ws subscribers.
    {
        let bridge = loopflow::lfd::bridge::StoreBridge::new(store.clone(), event_hub.clone());
        let bridge_cancel = cancel.clone();
        tokio::spawn(async move {
            tokio::select! {
                _ = bridge_cancel.cancelled() => {}
                _ = bridge.run(loopflow::lfd::bridge::POLL_CADENCE) => {}
            }
        });
    }

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

    let http_state = HttpState {
        store: store.clone(),
        executor: Arc::new(executor),
        event_hub,
        output_hub: output,
        provider_auth: ProviderAuthService::new(store.clone()),
        auth: auth_provider,
        started_at: time::OffsetDateTime::now_utc(),
        github: lfd_config.github,
        http_security: lfd_config.http_security,
        auth_failure_throttle: loopflow::lfd::auth::AuthFailureThrottle::new(),
        ci_failure_cache,
    };
    let http_router = loopflow::lfd::http::router(http_state);

    tracing::info!("gatekeeper: reads, push, webhook ingress; mutations exec lf");
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

    cancel.cancel();
    let _ = token_refresh_handle.await;
    let _ = journal_handle.await;

    Ok(())
}

fn storage_config_from_env() -> Result<StorageConfig, Box<dyn std::error::Error>> {
    if std::env::var_os("LFD_DATABASE_URL").is_some() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "LFD_DATABASE_URL was removed; lfd uses sqlite via LFD_DB_PATH",
        )
        .into());
    }

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

fn has_flag(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

#[cfg(test)]
mod tests {
    use super::{has_flag, storage_config_from_env, StorageConfig};
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

        let config = storage_config_from_env().expect("sqlite default should parse");
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

        let config = storage_config_from_env().expect("sqlite config should parse");
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

        let config = storage_config_from_env().expect("sqlite config should parse");
        match config {
            StorageConfig::Sqlite { path, .. } => {
                assert_eq!(path, absolute);
            }
            StorageConfig::Postgres { .. } => panic!("expected sqlite storage"),
        }
    }

    #[test]
    fn storage_config_rejects_database_url() {
        let _lock = env_lock().lock().expect("env lock poisoned");
        let _guard = EnvGuard::snapshot(&["HOME", "LFD_DB_PATH", "LFD_DATABASE_URL"]);
        let _home = setup_sqlite_env();
        std::env::set_var("LFD_DATABASE_URL", "postgres://example");

        let err = storage_config_from_env().expect_err("postgres env removed");
        assert_eq!(
            err.to_string(),
            "LFD_DATABASE_URL was removed; lfd uses sqlite via LFD_DB_PATH"
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
