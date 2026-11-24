use crate::support;
use std::process::{Command, Output, Stdio};

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
    let opencode = |output: Output| -> serde_json::Value {
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&output.stdout).unwrap();
        rows.into_iter()
            .find(|row| row["provider"] == "opencodezen")
            .unwrap()
    };
    run(&["machine", "login", "opencode", "--api-key"]);
    let row = opencode(run(&["machine", "logins", "--json"]));
    assert_eq!(row["state"], "signed_in");
    let rt = tokio::runtime::Runtime::new().unwrap();
    let store = rt
        .block_on(loopflow_execution::store::open_existing_store())
        .unwrap()
        .unwrap();
    assert!(rt
        .block_on(store.get_provider_token("opencodezen"))
        .unwrap()
        .is_some());
    run(&["machine", "logout", "opencode"]);
    assert!(rt
        .block_on(store.get_provider_token("opencodezen"))
        .unwrap()
        .is_none());
    let all = run(&["machine", "logins", "--json"]);
    let rows: Vec<serde_json::Value> = serde_json::from_slice(&all.stdout).unwrap();
    let providers: Vec<_> = rows
        .iter()
        .map(|row| row["provider"].as_str().unwrap())
        .collect();
    assert_eq!(
        providers,
        [
            "claude",
            "codex",
            "github",
            "opencodezen",
            "linear",
            "asana",
            "doppler"
        ]
    );
    assert_eq!(opencode(all)["state"], "signed_out");
}
