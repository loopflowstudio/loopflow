"""Wire-shape fixture tests for DTOs mirrored across Rust / Python / Swift.

Each fixture under tests/fixtures/dto/ is parsed here and in the Rust and Swift
test suites. If any mirror drifts, one of the three fails.
"""

import json
from pathlib import Path

from loopflow.models import Session

FIXTURE_DIR = Path(__file__).resolve().parents[2] / "tests" / "fixtures" / "dto"


def _load(name: str) -> dict:
    return json.loads((FIXTURE_DIR / name).read_text())


class TestDTOFixtures:
    def test_session_fixture_parses_with_input_supported_true(self):
        session = Session.model_validate(_load("session.json"))
        assert session.object == "session"
        assert session.harness == "codex"
        assert session.status == "active"
        assert session.input_supported is True
        assert session.wave_run_id == "run-abc"
        assert session.provider_session_id == "provider-xyz"
        assert session.config.agent == "claude-sonnet-4-5-20250929"
        assert session.config.cwd == "/tmp/repo"
        assert session.config.max_turns == 5
        assert session.config.yolo_mode is False

    def test_session_unsupported_input_fixture_parses_with_input_supported_false(self):
        session = Session.model_validate(_load("session_unsupported_input.json"))
        assert session.harness == "claude"
        assert session.status == "failed"
        assert session.input_supported is False
        assert session.ended_at is not None
