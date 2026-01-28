"""Tests for protocol schema and golden fixtures.

These tests validate that:
1. All fixture files parse as valid JSON
2. Required fields are present in each fixture
3. Enum values match the protocol schema
4. Event payloads follow the expected structure
"""

import json
from pathlib import Path

import pytest

PROTO_ROOT = Path(__file__).parent.parent / "proto"
FIXTURES_ROOT = PROTO_ROOT / "fixtures"


# -----------------------------------------------------------------------------
# Fixture Discovery
# -----------------------------------------------------------------------------


def _load_fixture(path: Path) -> dict:
    """Load and parse a JSON fixture file."""
    return json.loads(path.read_text())


def _discover_fixtures(subdir: str) -> list[tuple[str, Path]]:
    """Discover all JSON fixtures in a subdirectory."""
    fixture_dir = FIXTURES_ROOT / subdir
    if not fixture_dir.exists():
        return []
    return [(f.stem, f) for f in sorted(fixture_dir.glob("*.json"))]


# Parameterized fixtures
EVENT_FIXTURES = _discover_fixtures("events")
REQUEST_FIXTURES = _discover_fixtures("requests")
RESPONSE_FIXTURES = _discover_fixtures("responses")


# -----------------------------------------------------------------------------
# Schema Constants (from proto files)
# -----------------------------------------------------------------------------

# control.proto enums
STIMULUS_KINDS = {
    "STIMULUS_KIND_UNSPECIFIED",
    "STIMULUS_ONCE",
    "STIMULUS_LOOP",
    "STIMULUS_WATCH",
    "STIMULUS_CRON",
}

WAVE_STATUSES = {
    "WAVE_STATUS_UNSPECIFIED",
    "WAVE_IDLE",
    "WAVE_RUNNING",
    "WAVE_WAITING",
    "WAVE_ERROR",
}

WORKTREE_CHANGE_REASONS = {
    "WORKTREE_CHANGE_REASON_UNSPECIFIED",
    "WORKTREE_COMMIT",
    "WORKTREE_CHECKOUT",
    "WORKTREE_CHANGED",
    "WORKTREE_DRAFT_PR_CREATED",
    "WORKTREE_CI_UPDATED",
    "WORKTREE_MERGED",
    "WORKTREE_PR_STATE_CHANGED",
}

# engine.proto enums
DIFF_MODES = {
    "DIFF_MODE_UNSPECIFIED",
    "DIFF_FILES",
    "DIFF_RAW",
    "DIFF_NONE",
}


# -----------------------------------------------------------------------------
# Event Fixture Tests
# -----------------------------------------------------------------------------


@pytest.mark.parametrize("name,path", EVENT_FIXTURES, ids=[n for n, _ in EVENT_FIXTURES])
def test_event_fixture_parses(name: str, path: Path):
    """All event fixtures must be valid JSON."""
    data = _load_fixture(path)
    assert isinstance(data, dict)


@pytest.mark.parametrize("name,path", EVENT_FIXTURES, ids=[n for n, _ in EVENT_FIXTURES])
def test_event_fixture_has_required_fields(name: str, path: Path):
    """All events must have 'event' and 'timestamp' fields."""
    data = _load_fixture(path)
    assert "event" in data, f"Missing 'event' field in {name}"
    assert "timestamp" in data, f"Missing 'timestamp' field in {name}"
    assert isinstance(data["event"], str)
    assert isinstance(data["timestamp"], str)


@pytest.mark.parametrize("name,path", EVENT_FIXTURES, ids=[n for n, _ in EVENT_FIXTURES])
def test_event_fixture_has_payload(name: str, path: Path):
    """Each event should have exactly one payload field matching its type."""
    data = _load_fixture(path)
    event_type = data["event"]

    # Map event types to expected payload keys.
    # Only includes event types that have fixtures—add entries when adding fixtures.
    payload_map = {
        "session.started": "session_started",
        "session.ended": "session_ended",
        "output.line": "output_line",
        "worktree.updated": "worktree_updated",
        "worktree.pruned": "worktree_pruned",
        "wave.created": "wave_created",
        "wave.started": "wave_started",
        "wave.stopped": "wave_stopped",
        "wave.activated": "wave_activated",
        "scheduler.slot.acquired": "scheduler_slot_acquired",
        "scheduler.slot.released": "scheduler_slot_released",
    }

    expected_key = payload_map.get(event_type)
    assert expected_key is not None, f"Unknown event type: {event_type}"
    assert expected_key in data, f"Missing payload '{expected_key}' for event {event_type}"


def test_session_started_event_fields():
    """session.started must have id, step, worktree."""
    data = _load_fixture(FIXTURES_ROOT / "events" / "session_started.json")
    payload = data["session_started"]
    assert "id" in payload
    assert "step" in payload
    assert "worktree" in payload


def test_worktree_updated_event_fields():
    """worktree.updated must have branch, reason, repo."""
    data = _load_fixture(FIXTURES_ROOT / "events" / "worktree_updated.json")
    payload = data["worktree_updated"]
    assert "branch" in payload
    assert "reason" in payload
    assert payload["reason"] in WORKTREE_CHANGE_REASONS
    assert "repo" in payload


def test_wave_activated_event_fields():
    """wave.activated must have wave_id and valid stimulus kind."""
    data = _load_fixture(FIXTURES_ROOT / "events" / "wave_activated.json")
    payload = data["wave_activated"]
    assert "wave_id" in payload
    assert "stimulus" in payload
    assert payload["stimulus"] in STIMULUS_KINDS


# -----------------------------------------------------------------------------
# Request Fixture Tests
# -----------------------------------------------------------------------------


@pytest.mark.parametrize(
    "name,path", REQUEST_FIXTURES, ids=[n for n, _ in REQUEST_FIXTURES]
)
def test_request_fixture_parses(name: str, path: Path):
    """All request fixtures must be valid JSON."""
    data = _load_fixture(path)
    assert isinstance(data, dict)


def test_create_wave_request_fields():
    """create_wave request must have repo."""
    data = _load_fixture(FIXTURES_ROOT / "requests" / "create_wave.json")
    assert "repo" in data


def test_gather_context_request_fields():
    """gather_context request must have repo_root."""
    data = _load_fixture(FIXTURES_ROOT / "requests" / "gather_context.json")
    assert "repo_root" in data


def test_gather_context_diff_mode_valid():
    """gather_context diff_mode must be a valid enum value."""
    data = _load_fixture(FIXTURES_ROOT / "requests" / "gather_context.json")
    if "config" in data and "diff_mode" in data["config"]:
        assert data["config"]["diff_mode"] in DIFF_MODES


# -----------------------------------------------------------------------------
# Response Fixture Tests
# -----------------------------------------------------------------------------


@pytest.mark.parametrize(
    "name,path", RESPONSE_FIXTURES, ids=[n for n, _ in RESPONSE_FIXTURES]
)
def test_response_fixture_parses(name: str, path: Path):
    """All response fixtures must be valid JSON."""
    data = _load_fixture(path)
    assert isinstance(data, dict)


def test_get_health_response_has_protocol_version():
    """get_health response must include protocol_version."""
    data = _load_fixture(FIXTURES_ROOT / "responses" / "get_health.json")
    assert "protocol_version" in data
    pv = data["protocol_version"]
    assert "major" in pv
    assert "minor" in pv
    assert "patch" in pv
    assert isinstance(pv["major"], int)
    assert isinstance(pv["minor"], int)
    assert isinstance(pv["patch"], int)


def test_create_wave_response_has_wave():
    """create_wave response must include wave with required fields."""
    data = _load_fixture(FIXTURES_ROOT / "responses" / "create_wave.json")
    assert "wave" in data
    wave = data["wave"]
    assert "id" in wave
    assert "name" in wave
    assert "repo" in wave
    assert "status" in wave
    assert wave["status"] in WAVE_STATUSES


def test_list_worktrees_response_structure():
    """list_worktrees response must have worktrees array."""
    data = _load_fixture(FIXTURES_ROOT / "responses" / "list_worktrees.json")
    assert "worktrees" in data
    assert isinstance(data["worktrees"], list)
    if data["worktrees"]:
        wt = data["worktrees"][0]
        assert "branch" in wt
        assert "path" in wt


def test_error_detail_structure():
    """ErrorDetail must have code and message."""
    data = _load_fixture(FIXTURES_ROOT / "responses" / "error_detail.json")
    assert "code" in data
    assert "message" in data
    assert isinstance(data["code"], str)
    assert isinstance(data["message"], str)


# -----------------------------------------------------------------------------
# Cross-Fixture Consistency Tests
# -----------------------------------------------------------------------------


def test_event_timestamps_are_iso8601():
    """All event timestamps must be valid ISO 8601 format."""
    from datetime import datetime

    for name, path in EVENT_FIXTURES:
        data = _load_fixture(path)
        ts = data["timestamp"]
        # Should parse without error
        try:
            datetime.fromisoformat(ts.replace("Z", "+00:00"))
        except ValueError:
            pytest.fail(f"Invalid timestamp in {name}: {ts}")


def test_all_fixtures_use_consistent_id_format():
    """IDs should follow consistent naming patterns."""
    # Check wave IDs start with wave_
    wave_response = _load_fixture(FIXTURES_ROOT / "responses" / "create_wave.json")
    assert wave_response["wave"]["id"].startswith("wave_")

    # Check step run IDs start with sr_
    session_event = _load_fixture(FIXTURES_ROOT / "events" / "session_started.json")
    assert session_event["session_started"]["id"].startswith("sr_")


# -----------------------------------------------------------------------------
# Protocol Version Compatibility Tests
# -----------------------------------------------------------------------------


def test_protocol_version_is_1_0_0():
    """Current protocol version should be 1.0.0."""
    data = _load_fixture(FIXTURES_ROOT / "responses" / "get_health.json")
    pv = data["protocol_version"]
    assert pv["major"] == 1
    assert pv["minor"] == 0
    assert pv["patch"] == 0


# -----------------------------------------------------------------------------
# Proto File Existence Tests
# -----------------------------------------------------------------------------


def test_control_proto_exists():
    """Control plane proto file must exist."""
    proto_path = PROTO_ROOT / "loopflow" / "control" / "v1" / "control.proto"
    assert proto_path.exists(), f"Missing proto file: {proto_path}"


def test_engine_proto_exists():
    """Engine proto file must exist."""
    proto_path = PROTO_ROOT / "loopflow" / "engine" / "v1" / "engine.proto"
    assert proto_path.exists(), f"Missing proto file: {proto_path}"


def test_versioning_doc_exists():
    """VERSIONING.md must exist in proto directory."""
    versioning_path = PROTO_ROOT / "VERSIONING.md"
    assert versioning_path.exists(), f"Missing: {versioning_path}"


def test_readme_exists():
    """README.md must exist in proto directory."""
    readme_path = PROTO_ROOT / "README.md"
    assert readme_path.exists(), f"Missing: {readme_path}"
