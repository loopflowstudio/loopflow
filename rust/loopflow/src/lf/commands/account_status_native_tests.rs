use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use axum::{
    routing::{get, post},
    Json, Router,
};
use secrecy::{ExposeSecret, SecretString};
use serde_json::json;

use crate::provider_auth::{
    native_keychain_tests as native, read_claude_login, write_claude_login, Provider,
};
use crate::store::{ProviderAccountId, StorageConfig};

#[test]
#[ignore = "requires an unlocked macOS Keychain; invoke explicitly through test_network.py"]
fn native_refresh_replaces_exact_bytes_and_preserves_failed_saves() {
    let Some(root) = std::env::var_os(native::CHILD_ROOT).map(PathBuf::from) else {
        native::run_cases(
            concat!(
                module_path!(),
                "::native_refresh_replaces_exact_bytes_and_preserves_failed_saves"
            )
            .strip_prefix("loopflow::")
            .unwrap(),
        );
        return;
    };
    let _env = crate::journal::test_env_lock();
    native::disable_keychain_ui();
    let home = root.join("native");
    native::select_native_home(home.clone());
    std::env::set_var("LF_HOME", root.join("machine"));
    std::env::set_var("CLAUDE_CONFIG_DIR", &home);
    for name in [
        "HTTP_PROXY",
        "HTTPS_PROXY",
        "ALL_PROXY",
        "http_proxy",
        "https_proxy",
        "all_proxy",
    ] {
        std::env::remove_var(name);
    }
    let (size, invalid, unicode) = native::child_case(&root);
    let (old, _, access) = native::credentials(size, unicode);
    let mut observed = native::Observation {
        native_read: read_claude_login(&home)
            .ok()
            .flatten()
            .is_some_and(|value| value.expose_secret() == &old),
        ..Default::default()
    };
    if invalid {
        observed.unavailable = write_claude_login(&home, &SecretString::new("{".into())).is_err();
        observed.no_reconnect = true;
    } else {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(async {
            let requests = Arc::new(AtomicUsize::new(0));
            let count = requests.clone();
            let app = Router::new()
                .route("/token", post(move || {
                    count.fetch_add(1, Ordering::SeqCst);
                    let access = access.clone();
                    async move { Json(json!({"access_token": access})) }
                }))
                .route("/usage", get(|| async { Json(json!({"limits":[{"group":"session", "percent":25}]})) }))
                .route("/profile", get(|| async { Json(json!({"account":{"email":"fixture@example.com", "uuid":"fixture-user"}})) }));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let endpoint = format!("http://{}", listener.local_addr().unwrap());
            for (name, route) in [("TOKEN", "token"), ("USAGE", "usage"), ("PROFILE", "profile")] {
                std::env::set_var(format!("LF_TEST_CLAUDE_{name}_URL"), format!("{endpoint}/{route}"));
            }
            let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let store = Arc::new(crate::store::open_ephemeral_store(&StorageConfig::sqlite(root.join("store.db"))).await.unwrap());
            // A managed copy of the same login makes the normal account report
            // select its active native home through claude_credential_home.
            let profile = root.join("profile");
            std::fs::create_dir(&profile).unwrap();
            std::fs::write(profile.join(".credentials.json"), &old).unwrap();
            let account = crate::provider_account::new_account(Provider::Claude,
                ProviderAccountId::parse("fixture").unwrap(), profile,
                Some(crate::profile::EmailAddress::parse("fixture@example.com").unwrap()));
            store.upsert_provider_account(&account).await.unwrap();
            let rows = super::managed_rows(&store, Some(Provider::Claude), true).await.unwrap();
            assert_eq!(rows.len(), 1);
            let row = &rows[0];
            observed.accepted = row.verification == super::Verification::Accepted;
            observed.unavailable = row.verification == super::Verification::Unavailable;
            observed.persistence_diagnostic = row.diagnostic.as_ref().is_some_and(|d|
                d.starts_with("failed to persist refreshed credential:") &&
                !d.contains("fixture-refresh") && !d.contains("fixture-old"));
            observed.no_reconnect = row.recovery.is_none();
            observed.identity_and_usage = row.login.as_deref() == Some("fixture@example.com")
                && row.windows.iter().any(|w| w.window == "session" && w.used_percent == 25);
            observed.refresh_requests = requests.load(Ordering::SeqCst);
            server.abort();
            let _ = server.await;
        });
    }
    observed.credential_file_absent = !home.join(".credentials.json").exists();
    std::fs::write(
        root.join("observation.json"),
        serde_json::to_vec(&observed).unwrap(),
    )
    .unwrap();
}
