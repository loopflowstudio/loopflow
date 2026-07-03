//! Wire-shape fixture tests for DTOs mirrored across Rust / Python / Swift.
//!
//! Each fixture under `tests/fixtures/dto/` is parsed here and in the Python
//! and Swift test suites. If any mirror drifts, one of the three fails.

use std::path::PathBuf;

use loopflow::lfd::http::dto::{CreateSessionRequestDto, SessionDto, WaveDto};
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

fn load_top_fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures")
        .join(name);
    let bytes = std::fs::read(&path).unwrap_or_else(|err| {
        panic!("read fixture {}: {err}", path.display());
    });
    serde_json::from_slice(&bytes).expect("parse fixture json")
}

#[test]
fn wave_fixture_nests_repo_work() {
    let wave: WaveDto =
        serde_json::from_value(load_top_fixture("wave.json")).expect("wave fixture should parse");

    assert_eq!(wave.id, "wave_abc123");
    assert_eq!(wave.name, "engbot");
    assert_eq!(wave.primary_flow, "build");
    assert_eq!(wave.status, "running");
    assert_eq!(wave.parent_wave_id.as_deref(), Some("wave_parent999"));
    assert_eq!(
        wave.cloud_session_url.as_deref(),
        Some("https://claude.ai/session/engbot")
    );

    assert_eq!(wave.repos.len(), 1);
    let repo = &wave.repos[0];
    assert_eq!(repo.repo, "/home/user/project");
    assert_eq!(repo.status, "running");
    assert_eq!(repo.iteration, 3);
    assert_eq!(
        repo.local_worktree.as_deref(),
        Some("/home/user/project/.claude/worktrees/engbot")
    );
    assert_eq!(repo.remote_branch.as_deref(), Some("engbot/build-3"));
    assert_eq!(repo.open_pr_count, 1);
    assert_eq!(repo.commits.len(), 1);
    assert_eq!(repo.commits[0].sha, "abc1234");
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
