//! Wire-shape fixture tests for DTOs mirrored across Rust / Python / Swift.
//!
//! Each fixture under `tests/fixtures/dto/` is parsed here and in the Python
//! and Swift test suites. If any mirror drifts, one of the three fails.

use std::path::PathBuf;

use loopflow::lfd::conversations::turns::{ChatRole, ChatTurn};
use loopflow::lfd::conversations::types::{ConversationItem, Lifecycle};
use loopflow::lfd::http::dto::{CreateSessionRequestDto, SessionDto, UsageReportDto, WaveDto};
use loopflow::wave::state::MindState;
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
    assert_eq!(turn.from.as_deref(), Some("worker"));
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

    // `from` is explicitly Optional: a fixture without the key decodes as
    // None — no default masking (mirrored in Swift's ContractTests).
    let mut without_from = load_fixture("chat_turn.json");
    without_from.as_object_mut().expect("object").remove("from");
    let turn: ChatTurn = serde_json::from_value(without_from).expect("absent from parses");
    assert_eq!(turn.from, None);
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

/// `POST /messages` response — `{turn, state}` (wave/server.rs
/// `PostMessageResponse`). The same fixture Concerto's ContractTests decodes;
/// `turn` must parse as a `ChatTurn` and `state` must be a mind-state name.
#[test]
fn post_message_response_fixture_pins_wave_chat_reply() {
    let value = load_fixture("post_message_response.json");
    let turn: ChatTurn =
        serde_json::from_value(value["turn"].clone()).expect("turn should parse as ChatTurn");
    assert_eq!(turn.id, "turn-4");
    assert_eq!(turn.role, ChatRole::User);
    assert_eq!(turn.status, Lifecycle::Completed);
    assert_eq!(turn.from, None);

    // Round-trip: the reply's turn serializes back to the fixture shape.
    assert_eq!(
        serde_json::to_value(&turn).expect("serialize turn"),
        value["turn"]
    );

    // `state` carries the mind-state name at acceptance; it must be a name
    // MindState actually produces (renaming a state fails here AND in Swift).
    let state = value["state"].as_str().expect("state is a string");
    assert_eq!(
        state,
        MindState::Turning {
            turn_id: turn.id.clone(),
        }
        .name()
    );
}

/// The SSE `state` vocabulary (`idle | turning | interrupting | failed`)
/// crosses the boundary as bare names. The fixture pins the shared list:
/// renaming a `MindState` variant fails here, and Swift's ContractTests pin
/// the same file against `WaveMindState`.
#[test]
fn wave_mind_states_fixture_pins_the_state_vocabulary() {
    let value = load_fixture("wave_mind_states.json");
    let fixture_names: Vec<&str> = value["states"]
        .as_array()
        .expect("states array")
        .iter()
        .map(|name| name.as_str().expect("state name is a string"))
        .collect();

    let variants = [
        MindState::Idle,
        MindState::Turning {
            turn_id: "turn-1".to_string(),
        },
        MindState::Interrupting {
            turn_id: "turn-1".to_string(),
        },
        MindState::Failed {
            reason: "dead".to_string(),
        },
    ];
    let names: Vec<&str> = variants.iter().map(MindState::name).collect();
    assert_eq!(fixture_names, names);
}
