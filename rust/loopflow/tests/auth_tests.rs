use crate::support;
use std::process::{Command, Stdio};

#[test]
fn service_auth_survives_subscription_removal() {
    let home = tempfile::tempdir().unwrap();
    let _env = support::EnvGuard::with_lf_home(&[], home.path());
    let run = |args: &[&str]| {
        let output = Command::new(env!("CARGO_BIN_EXE_lf"))
            .args(args)
            .env("HOME", home.path())
            .env("OPENCODE_API_KEY", "fixture-service-key")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        output
    };
    run(&["account", "connect", "opencode", "--api-key"]);
    let output = run(&["account", "opencode", "--json"]);
    let status: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(status["accounts"][0]["provider"], "opencodezen");
    assert_eq!(status["accounts"][0]["state"], "active");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = rt
        .block_on(loopflow_execution::store::open_existing_store())
        .unwrap()
        .unwrap();
    assert!(rt
        .block_on(store.get_provider_token("opencodezen"))
        .unwrap()
        .is_some());
    run(&["account", "disconnect", "opencode"]);
    assert!(rt
        .block_on(store.get_provider_token("opencodezen"))
        .unwrap()
        .is_none());
    let all = run(&["account", "--json"]);
    let status: serde_json::Value = serde_json::from_slice(&all.stdout).unwrap();
    let providers: Vec<_> = status["accounts"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["provider"].as_str().unwrap())
        .collect();
    assert_eq!(
        providers,
        ["github", "opencodezen", "linear", "asana", "doppler"]
    );
}
