use crate::lfd::auth::{AuthFailureThrottle, AuthProvider};
use crate::lfd::config::{ExecutorConfig, GitHubConfig, HttpSecurityConfig};
use crate::lfd::events::EventHub;
use crate::lfd::executor::WaveExecutor;
use crate::lfd::http::state::HttpState;
use crate::lfd::output::OutputHub;
use crate::lfd::provider_auth::ProviderAuthService;
use crate::lfd::scheduler::Scheduler;
use crate::lfd::sessions::SessionManager;
use crate::lfd::store::{open_store, StorageConfig};
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tempfile::tempdir;
use time::OffsetDateTime;
use tokio::sync::Mutex;

pub async fn test_http_state() -> HttpState {
    let tmp = tempdir().expect("tempdir");
    let db_path = tmp.path().join("lfd.db");
    let store: crate::lfd::store::SharedStore = Arc::new(
        open_store(&StorageConfig::sqlite(db_path))
            .await
            .expect("open sqlite store"),
    );
    let scheduler = Arc::new(Scheduler::new(1));
    let output_hub = OutputHub::new(128, tmp.path().join("output"));
    let event_hub = EventHub::new(128);
    let sessions = SessionManager::new(store.clone());
    let executor = Arc::new(
        WaveExecutor::new(
            store.clone(),
            scheduler.clone(),
            output_hub.clone(),
            event_hub.clone(),
            ExecutorConfig::default(),
            GitHubConfig::default(),
        )
        .expect("build executor"),
    );

    HttpState {
        store: store.clone(),
        scheduler,
        executor,
        event_hub,
        output_hub,
        provider_auth: ProviderAuthService::new(store),
        auth: AuthProvider::Local {
            session_token: secrecy::SecretString::from("test-token".to_string()),
        },
        registration: None,
        started_at: OffsetDateTime::now_utc(),
        github: GitHubConfig::default(),
        http_security: HttpSecurityConfig::default(),
        auth_failure_throttle: AuthFailureThrottle::new(),
        ci_failure_cache: Arc::new(Mutex::new(std::collections::HashSet::new())),
        sessions,
    }
}

pub fn init_git_repo(path: &Path) {
    std::fs::create_dir_all(path).expect("create repo directory");
    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(path)
            .status()
            .unwrap_or_else(|e| panic!("git {}: {e}", args[0]));
        assert!(status.success(), "git {} failed", args[0]);
    };
    git(&["init", "-b", "main"]);
    git(&["config", "user.email", "test@example.com"]);
    git(&["config", "user.name", "Test User"]);
    std::fs::write(path.join("README.md"), "seed").expect("write seed file");
    git(&["add", "."]);
    git(&["commit", "-m", "init"]);
}
