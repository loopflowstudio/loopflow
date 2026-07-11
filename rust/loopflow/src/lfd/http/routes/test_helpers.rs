use crate::lfd::auth::{AuthFailureThrottle, AuthProvider};
use crate::lfd::config::{GitHubConfig, HttpSecurityConfig};
use crate::lfd::session_supervisor::SessionSupervisor;
use crate::lfd::http::state::HttpState;
use crate::lfdb::{open_store, StorageConfig};
use crate::provider_auth::ProviderAuthService;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use tempfile::tempdir;
use time::OffsetDateTime;
use tokio::sync::Mutex;

pub async fn test_http_state() -> HttpState {
    let tmp = tempdir().expect("tempdir");
    let db_path = tmp.path().join("lfd.db");
    let store: crate::lfdb::SharedStore = Arc::new(
        open_store(&StorageConfig::sqlite(db_path))
            .await
            .expect("open sqlite store"),
    );
    let session_supervisor = Arc::new(SessionSupervisor::new(store.clone()));

    HttpState {
        store: store.clone(),
        session_supervisor,
        provider_auth: ProviderAuthService::new(store),
        auth: AuthProvider::Bearer {
            session_token: secrecy::SecretString::from("test-token".to_string()),
        },
        started_at: OffsetDateTime::now_utc(),
        github: GitHubConfig::default(),
        http_security: HttpSecurityConfig::default(),
        auth_failure_throttle: AuthFailureThrottle::new(),
        ci_failure_cache: Arc::new(Mutex::new(std::collections::HashSet::new())),
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
