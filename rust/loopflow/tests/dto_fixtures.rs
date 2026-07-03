//! Wire-shape fixture tests for DTOs mirrored across Rust / Python / Swift.
//!
//! Each fixture under `tests/fixtures/dto/` is parsed here and in the Python
//! and Swift test suites. If any mirror drifts, one of the three fails.

use std::path::PathBuf;

use loopflow::lfd::http::dto::{CreateSessionRequestDto, SessionDto};
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
fn session_fixture_pins_palette_shape() {
    let session = load_fixture("session.json");
    assert_eq!(session["object"], "session");
    let session: SessionDto =
        serde_json::from_value(session).expect("session fixture should parse");
    assert_eq!(session.step, "ship");
    assert_eq!(session.agent, "codex");
    assert_eq!(session.source, "palette");
    assert_eq!(session.session_use, "palette");
    assert_eq!(session.status, "running");
    assert_eq!(session.run_id, None);
    assert_eq!(session.parent_session_id, None);
    assert!(session.argv.contains(&"-m".to_string()));
}

#[test]
fn create_session_request_fixture_pins_required_fields() {
    let request: CreateSessionRequestDto =
        serde_json::from_value(load_fixture("create_session_request.json"))
            .expect("create request fixture should parse");
    assert_eq!(request.flow, "ship");
    assert_eq!(request.worktree, "/tmp/repo.Desktop");
    assert_eq!(request.agent, "codex");
}
