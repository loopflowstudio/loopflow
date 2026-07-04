//! Wire-shape fixture tests for DTOs mirrored across Rust / Python / Swift.
//!
//! Each fixture under `tests/fixtures/dto/` is parsed here and in the Python
//! and Swift test suites. If any mirror drifts, one of the three fails.

use std::path::PathBuf;

use loopflow::lfd::conversations::turns::{ChatRole, ChatTurn};
use loopflow::lfd::conversations::types::{ConversationItem, Lifecycle};
use loopflow::lfd::http::dto::{
    CreateSessionRequestDto, RunWorkerRequestDto, SessionDto, UsageReportDto, WaveDto,
    WorkerPlacementDto,
};
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
fn chat_turn_fixture_pins_wave_chat_shape() {
    // The same fixture Concerto's ContractTests decodes; if the wire shape drifts
    // between Rust and Swift, one of the two fails.
    let turn: ChatTurn =
        serde_json::from_value(load_fixture("chat_turn.json")).expect("chat turn should parse");

    assert_eq!(turn.id, "turn-3");
    assert_eq!(turn.role, ChatRole::Assistant);
    assert_eq!(turn.status, Lifecycle::Running);
    assert_eq!(turn.items.len(), 6);
    assert!(matches!(turn.items[0], ConversationItem::Command { .. }));
    assert!(matches!(turn.items[1], ConversationItem::File { .. }));
    assert!(matches!(turn.items[2], ConversationItem::Message { .. }));
    assert!(matches!(turn.items[3], ConversationItem::Thought { .. }));
    assert!(matches!(turn.items[4], ConversationItem::Tool { .. }));

    // The interrupted state and explicit-null optionals are pinned on the wire.
    assert!(matches!(
        turn.items[5],
        ConversationItem::Command {
            status: Lifecycle::Interrupted,
            output: None,
            exit_code: None,
            duration_ms: None,
            ..
        }
    ));

    // Absent optionals serialize as explicit null, not omitted keys.
    let value = serde_json::to_value(&turn).expect("serialize chat turn");
    assert!(value["items"][5]["output"].is_null());
    assert!(value["items"][5]
        .as_object()
        .expect("object")
        .contains_key("output"));
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

#[test]
fn run_worker_request_fixture_pins_placement_shape() {
    let request: RunWorkerRequestDto =
        serde_json::from_value(load_fixture("run_worker_request.json"))
            .expect("run worker request fixture should parse");
    assert_eq!(request.flow, "implement");
    assert_eq!(request.task, "Add the workers endpoint.");
    assert_eq!(
        request.parent_session_id.as_deref(),
        Some("lfdsession_01HNX7XYZ0AZ1B2C3D4E5F6G7H")
    );
    assert_eq!(
        request.placement,
        WorkerPlacementDto::Stack {
            parent_run_id: "8f14e45f-ceea-467f-a34e-b5a1c3d4e5f6".to_string(),
        }
    );

    // Round-trip keeps `stack` as a flat field pair: placement + parent_run_id.
    let value = serde_json::to_value(&request).expect("serialize request");
    assert_eq!(value["placement"], "stack");
    assert_eq!(
        value["parent_run_id"],
        "8f14e45f-ceea-467f-a34e-b5a1c3d4e5f6"
    );

    // Unit placements are the bare tag; the field is required, never defaulted.
    let fresh: RunWorkerRequestDto = serde_json::from_value(serde_json::json!({
        "flow": "implement",
        "task": "t",
        "parent_session_id": null,
        "placement": "fresh",
    }))
    .expect("fresh placement should parse");
    assert_eq!(fresh.placement, WorkerPlacementDto::Fresh);

    let missing = serde_json::from_value::<RunWorkerRequestDto>(serde_json::json!({
        "flow": "implement",
        "task": "t",
    }));
    assert!(missing.is_err(), "placement is required on the wire");
}

#[test]
fn usage_report_fixture_pins_repo_provider_shape() {
    let value = load_fixture("usage_report.json");
    assert_eq!(value["object"], "usage_report");
    let report: UsageReportDto =
        serde_json::from_value(value).expect("usage report fixture should parse");

    assert_eq!(report.by_repo_provider.len(), 3);
    let loopflow_claude = report
        .by_repo_provider
        .iter()
        .find(|row| row.repo.as_deref() == Some("/Users/jack/src/loopflow"))
        .expect("loopflow/claude row");
    assert_eq!(loopflow_claude.provider, "claude");
    assert_eq!(loopflow_claude.input_tokens, 400);
    assert_eq!(loopflow_claude.cache_read_tokens, 15);

    // repo is explicitly Optional: a null repo round-trips as None.
    let unattributed = report
        .by_repo_provider
        .iter()
        .find(|row| row.repo.is_none())
        .expect("null-repo row");
    assert_eq!(unattributed.provider, "claude");

    assert_eq!(report.by_wave_provider.len(), 1);
    assert_eq!(
        report.by_wave_provider[0].wave_id,
        "lfdwave_01HNX7XYZ0AZ1B2C3D4E5F6G7H"
    );
    assert_eq!(report.by_provider.len(), 2);

    // Round-trip: serializing back preserves the explicit null repo.
    let roundtrip = serde_json::to_value(&report).expect("serialize usage report");
    assert!(roundtrip["by_repo_provider"]
        .as_array()
        .expect("array")
        .iter()
        .any(|row| row["repo"].is_null()));
}
