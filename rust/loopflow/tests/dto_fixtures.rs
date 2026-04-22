//! Wire-shape fixture tests for DTOs mirrored across Rust / Python / Swift.
//!
//! Each fixture under `tests/fixtures/dto/` is parsed here and in the Python
//! and Swift test suites. If any mirror drifts, one of the three fails.

use std::path::PathBuf;

use serde_json::Value;

fn load_fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/dto")
        .join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|err| {
        panic!("read fixture {}: {err}", path.display());
    });
    serde_json::from_slice(&bytes).expect("parse fixture json")
}

#[test]
fn session_fixture_has_codex_input_supported_true() {
    let session = load_fixture("session.json");
    assert_eq!(session["object"], "session");
    assert_eq!(session["harness"], "codex");
    assert_eq!(session["status"], "active");
    assert_eq!(session["input_supported"], true);
    assert_eq!(session["wave_run_id"], "run-abc");
    assert_eq!(session["provider_session_id"], "provider-xyz");
    assert_eq!(session["config"]["step"], "design");
    assert_eq!(session["config"]["repo_root"], "/tmp/repo");
    assert_eq!(session["config"]["yolo_mode"], false);
}

#[test]
fn session_unsupported_input_fixture_has_input_supported_false() {
    let session = load_fixture("session_unsupported_input.json");
    assert_eq!(session["harness"], "claude");
    assert_eq!(session["status"], "failed");
    assert_eq!(session["input_supported"], false);
    assert!(session["ended_at"].is_string());
}
